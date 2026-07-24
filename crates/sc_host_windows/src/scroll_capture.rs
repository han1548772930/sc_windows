use std::collections::VecDeque;
use std::io::Cursor;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};

use image::{DynamicImage, GenericImageView, ImageFormat, RgbaImage};
use opencv::{core, features2d, prelude::*};
use sc_platform_windows::windows::graphics_capture::{BgraFrame, CapturedScrollFrame};

// Keep matching work bounded so metadata remains close to its captured pixels.
const MAX_PENDING_FRAMES: usize = 1;
const MAX_STITCHED_HEIGHT: u32 = 50_000;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SplicePipelineState {
    Moving,
    Paused,
    Rebaseline,
    Confirm,
    Splicing,
}

#[derive(Debug, thiserror::Error)]
enum ScrollCaptureFailure {
    #[error("{0}")]
    InvalidFrame(String),
    #[error("scrolling frame could not be matched: {0}")]
    MatchLost(String),
    #[error("scrolling screenshot reached the maximum height of {limit}px")]
    MaximumLength { limit: u32 },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ScrollCaptureState {
    Broken { consecutive_failures: u32 },
    Recovered,
    MaximumLength { limit: u32 },
}

fn bgra_frame_to_rgba(mut frame: BgraFrame) -> Result<RgbaImage, String> {
    let expected_len = frame.width as usize * frame.height as usize * 4;
    if frame.pixels.len() != expected_len {
        return Err("invalid scrolling frame pixel buffer size".to_string());
    }
    for pixel in frame.pixels.chunks_exact_mut(4) {
        pixel.swap(0, 2);
        pixel[3] = 255;
    }
    RgbaImage::from_raw(frame.width, frame.height, frame.pixels)
        .ok_or_else(|| "failed to construct scrolling frame pixel buffer".to_string())
}

fn rgba_to_bgra_frame(image: RgbaImage) -> BgraFrame {
    let (width, height) = image.dimensions();
    let mut pixels = image.into_raw();
    for pixel in pixels.chunks_exact_mut(4) {
        pixel.swap(0, 2);
    }
    BgraFrame {
        width,
        height,
        pixels,
    }
}

#[derive(Clone)]
pub struct ScrollCaptureSession {
    stitched: TiledImage,
    previous: RgbaImage,
    current_offset: i64,
    min_offset: i64,
    max_bottom: i64,
    last_shift: Option<i32>,
    last_direction: i8,
    top_inset: Option<u32>,
    bottom_inset: Option<u32>,
    native_scroll_position: Option<i32>,
    native_scroll_scale: Option<f64>,
    preview_cache: Option<PreviewCache>,
}

#[derive(Clone)]
struct TiledImage {
    width: u32,
    height: u32,
    strips: VecDeque<RgbaImage>,
}

impl TiledImage {
    fn new(image: RgbaImage) -> Self {
        let (width, height) = image.dimensions();
        Self {
            width,
            height,
            strips: VecDeque::from([image]),
        }
    }

    fn width(&self) -> u32 {
        self.width
    }

    fn height(&self) -> u32 {
        self.height
    }

    fn trim_top(&mut self, rows: u32) {
        let mut rows = rows.min(self.height.saturating_sub(1));
        self.height -= rows;
        while rows > 0 {
            let strip = self.strips.pop_front().expect("tiled image has strips");
            if rows < strip.height() {
                self.strips.push_front(
                    image::imageops::crop_imm(
                        &strip,
                        0,
                        rows,
                        strip.width(),
                        strip.height() - rows,
                    )
                    .to_image(),
                );
                break;
            }
            rows -= strip.height();
        }
    }

    fn trim_bottom(&mut self, rows: u32) {
        let mut rows = rows.min(self.height.saturating_sub(1));
        self.height -= rows;
        while rows > 0 {
            let strip = self.strips.pop_back().expect("tiled image has strips");
            if rows < strip.height() {
                self.strips.push_back(
                    image::imageops::crop_imm(&strip, 0, 0, strip.width(), strip.height() - rows)
                        .to_image(),
                );
                break;
            }
            rows -= strip.height();
        }
    }

    fn prepend(&mut self, strip: RgbaImage) {
        debug_assert_eq!(strip.width(), self.width);
        self.height += strip.height();
        self.strips.push_front(strip);
    }

    fn append(&mut self, strip: RgbaImage) {
        debug_assert_eq!(strip.width(), self.width);
        self.height += strip.height();
        self.strips.push_back(strip);
    }

    fn crop_rows(&self, start: u32, height: u32) -> RgbaImage {
        let mut output = RgbaImage::new(self.width, height);
        let end = start.saturating_add(height);
        let mut strip_top = 0u32;
        for strip in &self.strips {
            let strip_bottom = strip_top + strip.height();
            let copy_top = start.max(strip_top);
            let copy_bottom = end.min(strip_bottom);
            if copy_top < copy_bottom {
                let source_y = copy_top - strip_top;
                let target_y = copy_top - start;
                let part = image::imageops::crop_imm(
                    strip,
                    0,
                    source_y,
                    self.width,
                    copy_bottom - copy_top,
                );
                image::imageops::replace(&mut output, &part.to_image(), 0, target_y as i64);
            }
            strip_top = strip_bottom;
            if strip_top >= end {
                break;
            }
        }
        output
    }

    fn to_image(&self) -> RgbaImage {
        self.crop_rows(0, self.height)
    }

    fn resize_box(&self, width: u32, height: u32) -> RgbaImage {
        let mut output = RgbaImage::new(width, height);
        let mut strip_index = 0usize;
        let mut strip_top = 0u32;
        for target_y in 0..height {
            let source_y0 = (target_y as u64 * self.height as u64 / height as u64) as u32;
            let source_y1 = (((target_y + 1) as u64 * self.height as u64).div_ceil(height as u64))
                .max(source_y0 as u64 + 1)
                .min(self.height as u64) as u32;
            while strip_index + 1 < self.strips.len()
                && source_y0 >= strip_top + self.strips[strip_index].height()
            {
                strip_top += self.strips[strip_index].height();
                strip_index += 1;
            }
            for target_x in 0..width {
                let source_x0 = (target_x as u64 * self.width as u64 / width as u64) as u32;
                let source_x1 = (((target_x + 1) as u64 * self.width as u64).div_ceil(width as u64))
                    .max(source_x0 as u64 + 1)
                    .min(self.width as u64) as u32;
                let mut channels = [0u64; 4];
                let mut samples = 0u64;
                let mut row_strip_index = strip_index;
                let mut row_strip_top = strip_top;
                for source_y in source_y0..source_y1 {
                    while row_strip_index + 1 < self.strips.len()
                        && source_y >= row_strip_top + self.strips[row_strip_index].height()
                    {
                        row_strip_top += self.strips[row_strip_index].height();
                        row_strip_index += 1;
                    }
                    let strip = &self.strips[row_strip_index];
                    let strip_y = source_y - row_strip_top;
                    for source_x in source_x0..source_x1 {
                        let pixel = strip.get_pixel(source_x, strip_y).0;
                        for channel in 0..4 {
                            channels[channel] += pixel[channel] as u64;
                        }
                        samples += 1;
                    }
                }
                output.put_pixel(
                    target_x,
                    target_y,
                    image::Rgba(channels.map(|value| (value / samples) as u8)),
                );
            }
        }
        output
    }
}

fn overlap_error(
    left: &RgbaImage,
    left_y: u32,
    right: &RgbaImage,
    right_y: u32,
    height: u32,
) -> u64 {
    // Keep matching resilient to fixed scrollbars/toolbars and animated blocks:
    // discard the noisiest horizontal cell in each row band, then the noisiest
    // row band. The remaining cells describe the stable document content.
    let mut cell_error = [[0u64; 3]; 4];
    let mut cell_samples = [[0u64; 3]; 4];
    for offset_y in (0..height).step_by(4) {
        let row_band = ((offset_y as u64 * 4 / height.max(1) as u64) as usize).min(3);
        for x in (0..left.width().min(right.width())).step_by(8) {
            let column_band =
                ((x as u64 * 3 / left.width().min(right.width()).max(1) as u64) as usize).min(2);
            let old = left.get_pixel(x, left_y + offset_y).0;
            let new = right.get_pixel(x, right_y + offset_y).0;
            if x < 8 {
                continue;
            }
            let old_neighbor = left.get_pixel(x - 8, left_y + offset_y).0;
            let new_neighbor = right.get_pixel(x - 8, right_y + offset_y).0;
            let detail = (0..3)
                .map(|channel| {
                    old[channel].abs_diff(old_neighbor[channel]) as u64
                        + new[channel].abs_diff(new_neighbor[channel]) as u64
                })
                .sum::<u64>();
            if detail < 12 {
                continue;
            }
            for channel in 0..3 {
                cell_error[row_band][column_band] += old[channel].abs_diff(new[channel]) as u64;
                cell_samples[row_band][column_band] += 1;
            }
        }
    }
    let mut row_scores = Vec::with_capacity(4);
    for row_band in 0..4 {
        let mut column_scores: Vec<_> = cell_error[row_band]
            .into_iter()
            .zip(cell_samples[row_band])
            .filter_map(|(error, samples)| (samples > 0).then(|| error / samples))
            .collect();
        if !column_scores.is_empty() {
            column_scores.sort_unstable();
            if column_scores.len() > 1 {
                column_scores.pop();
            }
            let count = column_scores.len() as u64;
            row_scores.push(column_scores.into_iter().sum::<u64>() / count);
        }
    }
    if row_scores.len() < 2 {
        return u64::MAX;
    }
    row_scores.sort_unstable();
    if row_scores.len() > 1 {
        row_scores.pop();
    }
    let count = row_scores.len() as u64;
    row_scores.into_iter().sum::<u64>() / count
}

#[cfg(test)]
impl PartialEq<RgbaImage> for TiledImage {
    fn eq(&self, other: &RgbaImage) -> bool {
        self.to_image() == *other
    }
}

#[cfg(test)]
impl std::fmt::Debug for TiledImage {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("TiledImage")
            .field("width", &self.width)
            .field("height", &self.height)
            .field("strips", &self.strips.len())
            .finish()
    }
}

#[derive(Clone)]
struct PreviewCache {
    image: RgbaImage,
    source_height: u32,
    max_width: u32,
    max_height: u32,
    scale: f64,
}

#[derive(Debug)]
pub struct PushOutcome {
    pub changed: bool,
}

struct MatchedFrame {
    splice_id: u64,
    next: RgbaImage,
    direction: i8,
    shift: i32,
    top_inset: Option<u32>,
    bottom_inset: Option<u32>,
}

impl ScrollCaptureSession {
    fn rebaseline(
        &mut self,
        frame: BgraFrame,
        native_scroll_position: Option<i32>,
    ) -> Result<(), String> {
        self.previous = bgra_frame_to_rgba(frame).map_err(|error| error.to_string())?;
        self.native_scroll_position = native_scroll_position;
        self.last_shift = None;
        Ok(())
    }
    pub fn from_bgra(frame: BgraFrame) -> Result<Self, String> {
        Ok(Self::from_image(bgra_frame_to_rgba(frame)?))
    }

    fn from_image(first: RgbaImage) -> Self {
        let initial_height = first.height() as i64;
        Self {
            stitched: TiledImage::new(first.clone()),
            previous: first,
            current_offset: 0,
            min_offset: 0,
            max_bottom: initial_height,
            last_shift: None,
            last_direction: 0,
            top_inset: None,
            bottom_inset: None,
            native_scroll_position: None,
            native_scroll_scale: None,
            preview_cache: None,
        }
    }

    #[cfg(test)]
    fn push_bgra_frame_with_id(
        &mut self,
        splice_id: u64,
        frame: BgraFrame,
        direction: i8,
        native_scroll_position: Option<i32>,
    ) -> Result<PushOutcome, ScrollCaptureFailure> {
        let matched = self.match_bgra_frame(splice_id, frame, direction, native_scroll_position)?;
        self.commit_matched_frame(matched)
    }

    #[cfg(test)]
    fn push_bgra_frame(
        &mut self,
        frame: BgraFrame,
        direction: i8,
        native_scroll_position: Option<i32>,
    ) -> Result<PushOutcome, ScrollCaptureFailure> {
        self.push_bgra_frame_with_id(0, frame, direction, native_scroll_position)
    }

    fn match_bgra_frame(
        &mut self,
        splice_id: u64,
        frame: BgraFrame,
        direction: i8,
        native_scroll_position: Option<i32>,
    ) -> Result<MatchedFrame, ScrollCaptureFailure> {
        let next = bgra_frame_to_rgba(frame).map_err(ScrollCaptureFailure::InvalidFrame)?;
        if next.dimensions() != self.previous.dimensions() {
            return Err(ScrollCaptureFailure::InvalidFrame(
                "keyframe dimensions changed during scrolling".to_string(),
            ));
        }
        let native_delta = native_scroll_position
            .zip(self.native_scroll_position)
            .map(|(current, previous)| current.saturating_sub(previous));
        self.native_scroll_position = native_scroll_position;
        let shift = match native_delta.filter(|shift| shift.unsigned_abs() > 0) {
            Some(native_delta) => {
                let shift = self.native_scroll_scale.map_or(native_delta, |scale| {
                    (native_delta as f64 * scale).round() as i32
                });
                let sign_ok = (direction >= 0 && shift >= 0) || (direction <= 0 && shift <= 0);
                let overlap_error = validated_shift_error(&self.previous, &next, shift);
                if sign_ok && overlap_error == Some(0) {
                    shift
                } else {
                    let visual_shift = OpenCvWorker::match_frame_shift(
                        &self.previous,
                        &next,
                        direction,
                        self.last_shift,
                    )?;
                    if visual_shift.signum() == native_delta.signum() {
                        let observed_scale = visual_shift as f64 / native_delta as f64;
                        if observed_scale.is_finite() {
                            self.native_scroll_scale =
                                Some(self.native_scroll_scale.map_or(observed_scale, |old| {
                                    old * 0.75 + observed_scale * 0.25
                                }));
                        }
                    }
                    visual_shift
                }
            }
            None => {
                OpenCvWorker::match_frame_shift(&self.previous, &next, direction, self.last_shift)?
            }
        };
        if direction != 0 && self.last_direction != 0 && direction != self.last_direction {
            self.last_shift = self.last_shift.map(|last| -last);
        }
        if direction != 0 {
            self.last_direction = direction;
        }
        Ok(MatchedFrame {
            splice_id,
            next,
            direction,
            shift,
            top_inset: self.top_inset,
            bottom_inset: self.bottom_inset,
        })
    }

    fn commit_matched_frame(
        &mut self,
        matched: MatchedFrame,
    ) -> Result<PushOutcome, ScrollCaptureFailure> {
        let MatchedFrame {
            splice_id,
            next,
            direction,
            shift,
            top_inset,
            bottom_inset,
        } = matched;
        let _ = splice_id;
        if self.top_inset != top_inset
            && let Some(inset) = top_inset
        {
            self.stitched.trim_top(inset);
            self.min_offset += inset as i64;
            self.preview_cache = None;
        }
        if self.bottom_inset != bottom_inset
            && let Some(inset) = bottom_inset
        {
            self.stitched.trim_bottom(inset);
            self.max_bottom -= inset as i64;
            self.preview_cache = None;
        }
        self.top_inset = top_inset;
        self.bottom_inset = bottom_inset;
        self.current_offset += shift as i64;
        self.previous = next.clone();
        if shift == 0 {
            return Ok(PushOutcome { changed: false });
        }
        let is_boundary_bounce = (direction > 0 && shift < 0) || (direction < 0 && shift > 0);
        if !is_boundary_bounce {
            self.last_shift = Some(shift);
        }

        let top_inset = self.top_inset.unwrap_or(0);
        let bottom_inset = self.bottom_inset.unwrap_or(0);
        let height = next.height() - top_inset - bottom_inset;
        let new_top = self.current_offset + top_inset as i64;
        let new_bottom = new_top + height as i64;
        if new_top < self.min_offset {
            let requested_height = (self.min_offset - new_top).min(height as i64) as u32;
            ensure_height_limit(self.stitched.height(), requested_height)?;
            let added_height = requested_height;
            let head = image::imageops::crop_imm(&next, 0, top_inset, next.width(), added_height);
            self.stitched.prepend(head.to_image());
            self.preview_cache = None;
            self.min_offset -= added_height as i64;
            return Ok(PushOutcome { changed: true });
        }

        let requested_height = (new_bottom - self.max_bottom).clamp(0, height as i64) as u32;
        if requested_height == 0 {
            return Ok(PushOutcome { changed: false });
        }
        ensure_height_limit(self.stitched.height(), requested_height)?;
        let content =
            image::imageops::crop_imm(&next, 0, top_inset, next.width(), height).to_image();
        // The OpenCV worker has validated this overlap. Only append rows outside
        // the already committed document range.
        let added_height = requested_height;
        self.current_offset =
            self.max_bottom - height as i64 + added_height as i64 - top_inset as i64;
        if added_height == 0 {
            return Ok(PushOutcome { changed: false });
        }
        let tail = image::imageops::crop_imm(
            &content,
            0,
            height - added_height,
            content.width(),
            added_height,
        );
        self.stitched.append(tail.to_image());
        self.max_bottom += added_height as i64;
        Ok(PushOutcome { changed: true })
    }

    fn advance_match_state(&mut self, matched: &MatchedFrame) {
        self.current_offset += matched.shift as i64;
        self.confirm_match_state(matched);
    }

    fn confirm_match_state(&mut self, matched: &MatchedFrame) {
        self.previous = matched.next.clone();
        if matched.shift != 0 {
            let is_boundary_bounce = (matched.direction > 0 && matched.shift < 0)
                || (matched.direction < 0 && matched.shift > 0);
            if !is_boundary_bounce {
                self.last_shift = Some(matched.shift);
            }
        }
        self.top_inset = matched.top_inset;
        self.bottom_inset = matched.bottom_inset;
    }

    pub fn bmp_data(&self) -> Result<Vec<u8>, String> {
        let mut output = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(self.stitched.to_image())
            .write_to(&mut output, ImageFormat::Bmp)
            .map_err(|e| format!("failed to encode scrolling screenshot: {e}"))?;
        Ok(output.into_inner())
    }

    pub fn preview_frame(&mut self, max_width: u32, max_height: u32) -> BgraFrame {
        let width_scale = max_width as f64 / self.stitched.width().max(1) as f64;
        let height_scale = max_height as f64 / self.stitched.height().max(1) as f64;
        let scale = width_scale.min(height_scale).min(1.0);
        let width = (self.stitched.width() as f64 * scale).round().max(1.0) as u32;
        let height = (self.stitched.height() as f64 * scale).round().max(1.0) as u32;
        let can_append = self.preview_cache.as_ref().is_some_and(|cache| {
            cache.max_width == max_width
                && cache.max_height == max_height
                && (cache.scale - scale).abs() < f64::EPSILON
                && cache.source_height < self.stitched.height()
                && cache.image.width() == width
                && cache.image.height() < height
        });
        if can_append {
            let cache = self.preview_cache.as_mut().expect("preview cache checked");
            let source_height = self.stitched.height() - cache.source_height;
            let target_height = height - cache.image.height();
            let strip = self.stitched.crop_rows(cache.source_height, source_height);
            let strip = image::imageops::resize(
                &strip,
                width,
                target_height,
                image::imageops::FilterType::Triangle,
            );
            let mut pixels = std::mem::take(&mut cache.image).into_raw();
            pixels.extend_from_slice(strip.as_raw());
            cache.image = RgbaImage::from_raw(width, height, pixels)
                .expect("incremental preview dimensions are consistent");
            cache.source_height = self.stitched.height();
            return rgba_to_bgra_frame(cache.image.clone());
        }

        let preview = self.stitched.resize_box(width, height);
        self.preview_cache = Some(PreviewCache {
            image: preview.clone(),
            source_height: self.stitched.height(),
            max_width,
            max_height,
            scale,
        });
        rgba_to_bgra_frame(preview)
    }
}

fn validated_shift_error(previous: &RgbaImage, next: &RgbaImage, shift: i32) -> Option<u64> {
    let displacement = shift.unsigned_abs();
    let overlap = previous
        .height()
        .min(next.height())
        .checked_sub(displacement)?;
    if overlap < 32 {
        return None;
    }
    Some(if shift > 0 {
        overlap_error(previous, displacement, next, 0, overlap)
    } else {
        overlap_error(previous, 0, next, displacement, overlap)
    })
}

fn ensure_height_limit(current_height: u32, added_height: u32) -> Result<(), ScrollCaptureFailure> {
    if current_height.saturating_add(added_height) > MAX_STITCHED_HEIGHT {
        return Err(ScrollCaptureFailure::MaximumLength {
            limit: MAX_STITCHED_HEIGHT,
        });
    }
    Ok(())
}

struct OpenCvWorker;

impl OpenCvWorker {
    fn match_frame_shift(
        previous: &RgbaImage,
        next: &RgbaImage,
        _direction: i8,
        _previous_shift: Option<i32>,
    ) -> Result<i32, ScrollCaptureFailure> {
        if previous.dimensions() != next.dimensions() {
            return Err(ScrollCaptureFailure::MatchLost(
                "frame dimensions changed".to_string(),
            ));
        }
        let width = previous.width();
        let height = previous.height();
        if width < 48 || height < 72 {
            return Err(ScrollCaptureFailure::InvalidFrame(
                "capture region is too small for scrolling matching".to_string(),
            ));
        }

        let identical_top = identical_edge_rows(previous, next, false);
        let identical_bottom = identical_edge_rows(previous, next, true);
        let crop_top = identical_top.saturating_sub(31);
        let crop_bottom = identical_bottom.saturating_sub(31);
        if crop_top.saturating_add(crop_bottom) >= height {
            return Ok(0);
        }
        let crop_height = height - crop_top - crop_bottom;
        let previous_mat = gray_mat(previous, crop_top, crop_height)?;
        let next_mat = gray_mat(next, crop_top, crop_height)?;
        let mut orb = features2d::ORB::create(
            2000,
            1.2,
            8,
            31,
            0,
            2,
            features2d::ORB_ScoreType::HARRIS_SCORE,
            31,
            20,
        )
        .map_err(opencv_failure)?;
        let mut previous_keypoints = core::Vector::<core::KeyPoint>::new();
        let mut previous_descriptors = Mat::default();
        orb.detect_and_compute_def(
            &previous_mat,
            &Mat::default(),
            &mut previous_keypoints,
            &mut previous_descriptors,
        )
        .map_err(opencv_failure)?;
        let mut next_keypoints = core::Vector::<core::KeyPoint>::new();
        let mut next_descriptors = Mat::default();
        orb.detect_and_compute_def(
            &next_mat,
            &Mat::default(),
            &mut next_keypoints,
            &mut next_descriptors,
        )
        .map_err(opencv_failure)?;
        if previous_descriptors.empty() || next_descriptors.empty() {
            return Err(ScrollCaptureFailure::MatchLost(
                "WeChat-style ORB descriptors are empty".to_string(),
            ));
        }

        let matcher =
            features2d::BFMatcher::create(core::NORM_L2SQR, false).map_err(opencv_failure)?;
        let mut matches = core::Vector::<core::Vector<core::DMatch>>::new();
        matcher
            .knn_train_match_def(&previous_descriptors, &next_descriptors, &mut matches, 5)
            .map_err(opencv_failure)?;

        let mut offsets = Vec::new();
        for group in matches {
            if group.len() < 2 {
                continue;
            }
            let best = group.get(0).map_err(opencv_failure)?;
            let second = group.get(1).map_err(opencv_failure)?;
            if best.distance >= second.distance * 0.75 {
                continue;
            }
            for matched in [best, second] {
                if matched.distance > 20.0 {
                    continue;
                }
                let previous_point = previous_keypoints
                    .get(matched.query_idx as usize)
                    .map_err(opencv_failure)?
                    .pt();
                let next_point = next_keypoints
                    .get(matched.train_idx as usize)
                    .map_err(opencv_failure)?
                    .pt();
                if (previous_point.x.round() as i32 - next_point.x.round() as i32).abs() > 4 {
                    continue;
                }
                let mut offset = previous_point.y.round() as i32 - next_point.y.round() as i32;
                if offset.abs() < 2 {
                    offset = 0;
                }
                offsets.push(offset);
            }
        }
        if offsets.is_empty() {
            return Err(ScrollCaptureFailure::MatchLost(
                "WeChat-style ORB matching produced no aligned points".to_string(),
            ));
        }
        let mut histogram = std::collections::BTreeMap::<i32, usize>::new();
        for &offset in &offsets {
            *histogram.entry(offset).or_default() += 1;
        }
        let (shift, _support) = histogram
            .iter()
            .map(|(&candidate, _)| {
                let support = offsets
                    .iter()
                    .filter(|&&offset| (offset - candidate).abs() <= 1)
                    .count();
                ((candidate, support), candidate)
            })
            .max_by_key(|((_, support), _)| *support)
            .map(|((candidate, support), _)| (candidate, support))
            .ok_or_else(|| {
                ScrollCaptureFailure::MatchLost(
                    "WeChat-style ORB offset histogram is empty".to_string(),
                )
            })?;
        if shift.unsigned_abs() > height.saturating_mul(3) / 5 {
            return Err(ScrollCaptureFailure::MatchLost(format!(
                "offset {shift}px exceeds WeChat's 60% frame-height limit"
            )));
        }
        Ok(shift)
    }
}

fn identical_edge_rows(left: &RgbaImage, right: &RgbaImage, from_bottom: bool) -> u32 {
    let rows: Box<dyn Iterator<Item = u32>> = if from_bottom {
        Box::new((0..left.height()).rev())
    } else {
        Box::new(0..left.height())
    };
    rows.take_while(|&row| {
        left.view(0, row, left.width(), 1).to_image()
            == right.view(0, row, right.width(), 1).to_image()
    })
    .count() as u32
}

fn gray_mat(image: &RgbaImage, top: u32, height: u32) -> Result<Mat, ScrollCaptureFailure> {
    let mut gray = Vec::with_capacity((image.width() * height) as usize);
    for row in top..top + height {
        for column in 0..image.width() {
            let p = image.get_pixel(column, row).0;
            gray.push(((p[0] as u32 * 77 + p[1] as u32 * 150 + p[2] as u32 * 29) >> 8) as u8);
        }
    }
    Mat::from_slice(&gray)
        .and_then(|mat| mat.reshape(1, height as i32)?.try_clone())
        .map_err(opencv_failure)
}

fn opencv_failure(error: opencv::Error) -> ScrollCaptureFailure {
    ScrollCaptureFailure::MatchLost(format!("OpenCV error: {error}"))
}

enum WorkerCommand {
    Frame {
        splice_id: u64,
        frame: CapturedScrollFrame,
        direction: i8,
        preview_size: Option<(u32, u32)>,
        queued_at: Instant,
    },
    FinishGesture {
        preview_size: (u32, u32),
    },
    Export(Sender<Result<Vec<u8>, String>>),
    Stop,
}

enum SpliceCommand {
    Rebaseline {
        frame: BgraFrame,
        native_scroll_position: Option<i32>,
    },
    Frame {
        matched: MatchedFrame,
        preview_size: Option<(u32, u32)>,
    },
    FinishGesture {
        preview_size: (u32, u32),
    },
    Export(Sender<Result<Vec<u8>, String>>),
    Stop,
}

pub enum ScrollCaptureEvent {
    Preview(BgraFrame),
    FrameAccepted,
    FrameDiscarded,
    GestureFinished,
    StateChanged(ScrollCaptureState),
}

pub struct ScrollCaptureWorker {
    selection: sc_app::selection::RectI32,
    commands: Sender<WorkerCommand>,
    events: Receiver<ScrollCaptureEvent>,
    thread: Option<std::thread::JoinHandle<()>>,
    pending_frames: Arc<AtomicUsize>,
    next_splice_id: AtomicU64,
}

impl ScrollCaptureWorker {
    pub fn from_bgra(
        selection: sc_app::selection::RectI32,
        frame: BgraFrame,
        native_scroll_position: Option<i32>,
    ) -> Result<Self, String> {
        let mut session = ScrollCaptureSession::from_bgra(frame)?;
        session.native_scroll_position = native_scroll_position;
        Self::spawn(selection, session)
    }

    fn spawn(
        selection: sc_app::selection::RectI32,
        session: ScrollCaptureSession,
    ) -> Result<Self, String> {
        let (command_tx, command_rx) = mpsc::channel();
        let (event_tx, event_rx) = mpsc::channel();
        let pending_frames = Arc::new(AtomicUsize::new(0));
        let worker_pending_frames = pending_frames.clone();
        let thread = std::thread::Builder::new()
            .name("scroll-capture-worker".to_string())
            .spawn(move || {
                run_scroll_capture_worker(session, command_rx, event_tx, worker_pending_frames)
            })
            .map_err(|error| format!("failed to start scrolling capture worker: {error}"))?;
        Ok(Self {
            selection,
            commands: command_tx,
            events: event_rx,
            thread: Some(thread),
            pending_frames,
            next_splice_id: AtomicU64::new(1),
        })
    }

    pub fn selection(&self) -> sc_app::selection::RectI32 {
        self.selection
    }

    pub fn push_frame(
        &self,
        frame: CapturedScrollFrame,
        direction: i8,
        preview_size: Option<(u32, u32)>,
    ) -> Result<bool, String> {
        if self
            .pending_frames
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |pending| {
                (pending < MAX_PENDING_FRAMES).then_some(pending + 1)
            })
            .is_err()
        {
            eprintln!(
                "[scroll capture] worker queue full, pending={}",
                self.pending_frames()
            );
            return Ok(false);
        }
        if let Err(error) = self.send(WorkerCommand::Frame {
            splice_id: self.next_splice_id.fetch_add(1, Ordering::Relaxed),
            frame,
            direction,
            preview_size,
            queued_at: Instant::now(),
        }) {
            self.pending_frames.fetch_sub(1, Ordering::AcqRel);
            return Err(error);
        }
        Ok(true)
    }

    pub fn pending_frames(&self) -> usize {
        self.pending_frames.load(Ordering::Acquire)
    }

    pub fn can_accept_frame(&self) -> bool {
        self.pending_frames() < MAX_PENDING_FRAMES
    }

    pub fn finish_gesture(&self, preview_size: (u32, u32)) -> Result<(), String> {
        self.send(WorkerCommand::FinishGesture { preview_size })
    }

    pub fn poll_event(&self) -> Option<ScrollCaptureEvent> {
        self.events.try_recv().ok()
    }

    pub fn bmp_data(&self) -> Result<Vec<u8>, String> {
        let (response_tx, response_rx) = mpsc::channel();
        self.send(WorkerCommand::Export(response_tx))?;
        response_rx
            .recv()
            .map_err(|_| "scrolling capture worker stopped".to_string())?
    }

    fn send(&self, command: WorkerCommand) -> Result<(), String> {
        self.commands
            .send(command)
            .map_err(|_| "scrolling capture worker stopped".to_string())
    }
}

impl Drop for ScrollCaptureWorker {
    fn drop(&mut self) {
        let _ = self.commands.send(WorkerCommand::Stop);
        // Do not block the UI while an already queued frame is finishing.
        self.thread.take();
    }
}

fn run_scroll_capture_worker(
    mut session: ScrollCaptureSession,
    commands: Receiver<WorkerCommand>,
    events: Sender<ScrollCaptureEvent>,
    pending_frames: Arc<AtomicUsize>,
) {
    let splice_session = session.clone();
    let (splice_tx, splice_rx) = mpsc::channel();
    let splice_events = events.clone();
    let splice_pending_frames = pending_frames.clone();
    let splice_thread = std::thread::Builder::new()
        .name("scroll-capture-splice-worker".to_string())
        .spawn(move || {
            run_splice_worker(
                splice_session,
                splice_rx,
                splice_events,
                splice_pending_frames,
            )
        })
        .expect("failed to start scroll capture splice worker");
    let mut last_lag_warning = None;
    let mut consecutive_failures = 0u32;
    let mut stats_started = Instant::now();
    let mut stats_busy = Duration::ZERO;
    let mut stats_frames = 0u32;
    let mut stats_failures = 0u32;
    let mut expected_splice_id = 1u64;
    let mut pipeline_state = SplicePipelineState::Paused;
    let mut broken_visible = false;
    let mut last_wheel_sequence = 0u64;
    while let Ok(command) = commands.recv() {
        match command {
            WorkerCommand::Frame {
                splice_id,
                frame,
                direction,
                preview_size,
                queued_at,
            } => {
                if splice_id != expected_splice_id {
                    eprintln!(
                        "[scroll capture] discarded out-of-order splice_id={splice_id}, expected={expected_splice_id}"
                    );
                    pending_frames.fetch_sub(1, Ordering::AcqRel);
                    continue;
                }
                expected_splice_id = expected_splice_id.saturating_add(1);
                if frame.discontinuity {
                    pipeline_state = SplicePipelineState::Rebaseline;
                    let splice_frame = frame.frame.clone();
                    if let Err(error) =
                        session.rebaseline(frame.frame, frame.native_scroll_position)
                    {
                        eprintln!("[scroll capture] rebaseline failed: {error}");
                    }
                    let _ = splice_tx.send(SpliceCommand::Rebaseline {
                        frame: splice_frame,
                        native_scroll_position: frame.native_scroll_position,
                    });
                    pending_frames.fetch_sub(1, Ordering::AcqRel);
                    continue;
                }
                let wheel_changed = frame.wheel_sequence != last_wheel_sequence;
                last_wheel_sequence = frame.wheel_sequence;
                if !matches!(
                    pipeline_state,
                    SplicePipelineState::Rebaseline | SplicePipelineState::Confirm
                ) {
                    pipeline_state = if wheel_changed {
                        SplicePipelineState::Moving
                    } else {
                        SplicePipelineState::Paused
                    };
                }
                let queue_delay = queued_at.elapsed();
                if queue_delay >= Duration::from_millis(200)
                    && last_lag_warning
                        .is_none_or(|last: Instant| last.elapsed() >= Duration::from_secs(1))
                {
                    eprintln!(
                        "[scroll capture] worker queue delay: {}ms; preview may lag",
                        queue_delay.as_millis()
                    );
                    last_lag_warning = Some(Instant::now());
                }
                let processing_started = Instant::now();
                let native_scroll_position = frame.native_scroll_position;
                let previous_native_scroll_position = session.native_scroll_position;
                if pipeline_state == SplicePipelineState::Rebaseline {
                    pipeline_state = SplicePipelineState::Confirm;
                }
                let result = session.match_bgra_frame(
                    splice_id,
                    frame.frame,
                    direction,
                    native_scroll_position,
                );
                stats_busy += processing_started.elapsed();
                stats_frames = stats_frames.saturating_add(1);
                match result {
                    Ok(matched) => {
                        if matches!(
                            pipeline_state,
                            SplicePipelineState::Rebaseline | SplicePipelineState::Confirm
                        ) {
                            if broken_visible {
                                let _ = events.send(ScrollCaptureEvent::StateChanged(
                                    ScrollCaptureState::Recovered,
                                ));
                                broken_visible = false;
                            }
                            consecutive_failures = 0;
                            session.advance_match_state(&matched);
                            pipeline_state = SplicePipelineState::Splicing;
                            if splice_tx
                                .send(SpliceCommand::Frame {
                                    matched,
                                    preview_size,
                                })
                                .is_err()
                            {
                                pending_frames.fetch_sub(1, Ordering::AcqRel);
                                break;
                            }
                        } else {
                            consecutive_failures = 0;
                            session.advance_match_state(&matched);
                            pipeline_state = SplicePipelineState::Splicing;
                            if splice_tx
                                .send(SpliceCommand::Frame {
                                    matched,
                                    preview_size,
                                })
                                .is_err()
                            {
                                pending_frames.fetch_sub(1, Ordering::AcqRel);
                                break;
                            }
                        }
                    }
                    Err(failure) => {
                        stats_failures = stats_failures.saturating_add(1);
                        let _ = events.send(ScrollCaptureEvent::FrameDiscarded);
                        pending_frames.fetch_sub(1, Ordering::AcqRel);
                        match failure {
                            ScrollCaptureFailure::MaximumLength { limit } => {
                                let _ = events.send(ScrollCaptureEvent::StateChanged(
                                    ScrollCaptureState::MaximumLength { limit },
                                ));
                            }
                            failure => {
                                consecutive_failures = consecutive_failures.saturating_add(1);
                                eprintln!("[scroll capture] rejected frame: {failure}");
                                pipeline_state = SplicePipelineState::Rebaseline;
                                session.native_scroll_position = previous_native_scroll_position;
                            }
                        }
                    }
                }
                if stats_started.elapsed() >= Duration::from_secs(1) {
                    let elapsed = stats_started.elapsed();
                    let elapsed_ms = elapsed.as_secs_f64() * 1000.0;
                    let average_ms = stats_busy.as_secs_f64() * 1000.0 / stats_frames.max(1) as f64;
                    let busy_percent = stats_busy.as_secs_f64() / elapsed.as_secs_f64() * 100.0;
                    eprintln!(
                        "[scroll capture] opencv stats: fps={:.1}, avg={average_ms:.2}ms, busy={busy_percent:.1}%, failures={stats_failures}, pending={}",
                        stats_frames as f64 * 1000.0 / elapsed_ms,
                        pending_frames.load(Ordering::Acquire)
                    );
                    stats_started = Instant::now();
                    stats_busy = Duration::ZERO;
                    stats_frames = 0;
                    stats_failures = 0;
                }
            }
            WorkerCommand::FinishGesture { preview_size } => {
                if consecutive_failures > 0
                    && matches!(
                        pipeline_state,
                        SplicePipelineState::Rebaseline | SplicePipelineState::Confirm
                    )
                    && !broken_visible
                {
                    let _ = events.send(ScrollCaptureEvent::StateChanged(
                        ScrollCaptureState::Broken {
                            consecutive_failures,
                        },
                    ));
                    broken_visible = true;
                }
                let _ = splice_tx.send(SpliceCommand::FinishGesture { preview_size });
            }
            WorkerCommand::Export(response) => {
                let _ = splice_tx.send(SpliceCommand::Export(response));
            }
            WorkerCommand::Stop => {
                let _ = splice_tx.send(SpliceCommand::Stop);
                break;
            }
        }
    }
    let _ = splice_thread.join();
}

fn run_splice_worker(
    mut session: ScrollCaptureSession,
    commands: Receiver<SpliceCommand>,
    events: Sender<ScrollCaptureEvent>,
    pending_frames: Arc<AtomicUsize>,
) {
    let mut preview_dirty = false;
    let mut last_splice_id = 0u64;
    let mut stats_started = Instant::now();
    let mut stats_busy = Duration::ZERO;
    let mut stats_frames = 0u32;
    while let Ok(command) = commands.recv() {
        match command {
            SpliceCommand::Rebaseline {
                frame,
                native_scroll_position,
            } => {
                if let Err(error) = session.rebaseline(frame, native_scroll_position) {
                    eprintln!("[scroll capture] splice rebaseline failed: {error}");
                }
            }
            SpliceCommand::Frame {
                matched,
                preview_size,
            } => {
                if matched.splice_id <= last_splice_id {
                    eprintln!(
                        "[scroll capture] splice worker discarded stale splice_id={}, last={last_splice_id}",
                        matched.splice_id
                    );
                    pending_frames.fetch_sub(1, Ordering::AcqRel);
                    continue;
                }
                last_splice_id = matched.splice_id;
                let processing_started = Instant::now();
                match session.commit_matched_frame(matched) {
                    Ok(outcome) => {
                        preview_dirty |= outcome.changed;
                        let _ = events.send(ScrollCaptureEvent::FrameAccepted);
                        if preview_dirty && let Some((width, height)) = preview_size {
                            let preview = session.preview_frame(width, height);
                            preview_dirty = false;
                            let _ = events.send(ScrollCaptureEvent::Preview(preview));
                        }
                    }
                    Err(ScrollCaptureFailure::MaximumLength { limit }) => {
                        let _ = events.send(ScrollCaptureEvent::StateChanged(
                            ScrollCaptureState::MaximumLength { limit },
                        ));
                    }
                    Err(failure) => {
                        eprintln!("[scroll capture] splice worker rejected frame: {failure}");
                        let _ = events.send(ScrollCaptureEvent::FrameDiscarded);
                    }
                }
                stats_busy += processing_started.elapsed();
                stats_frames = stats_frames.saturating_add(1);
                pending_frames.fetch_sub(1, Ordering::AcqRel);
                if stats_started.elapsed() >= Duration::from_secs(1) {
                    let elapsed = stats_started.elapsed();
                    let average_ms = stats_busy.as_secs_f64() * 1000.0 / stats_frames.max(1) as f64;
                    let busy_percent = stats_busy.as_secs_f64() / elapsed.as_secs_f64() * 100.0;
                    eprintln!(
                        "[scroll capture] splice stats: fps={:.1}, avg={average_ms:.2}ms, busy={busy_percent:.1}%, last_splice_id={last_splice_id}",
                        stats_frames as f64 / elapsed.as_secs_f64()
                    );
                    stats_started = Instant::now();
                    stats_busy = Duration::ZERO;
                    stats_frames = 0;
                }
            }
            SpliceCommand::FinishGesture { preview_size } => {
                if preview_dirty {
                    let preview = session.preview_frame(preview_size.0, preview_size.1);
                    preview_dirty = false;
                    let _ = events.send(ScrollCaptureEvent::Preview(preview));
                }
                let _ = events.send(ScrollCaptureEvent::GestureFinished);
            }
            SpliceCommand::Export(response) => {
                let _ = response.send(session.bmp_data());
            }
            SpliceCommand::Stop => break,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    fn frame(width: u32, height: u32, seed: u8) -> BgraFrame {
        let image = RgbaImage::from_fn(width, height, |x, y| {
            let value = seed.wrapping_add((x * 17 + y * 31) as u8);
            Rgba([value, value.wrapping_add(23), value.wrapping_add(71), 255])
        });
        rgba_to_bgra_frame(image)
    }

    fn document(width: u32, height: u32) -> RgbaImage {
        RgbaImage::from_fn(width, height, |x, y| {
            let mut hash = x.wrapping_mul(0x9e37_79b9) ^ y.wrapping_mul(0x85eb_ca6b);
            hash ^= hash >> 16;
            hash = hash.wrapping_mul(0x7feb_352d);
            hash ^= hash >> 15;
            let value = hash as u8;
            Rgba([value, value.wrapping_add(23), value.wrapping_add(71), 255])
        })
    }

    fn document_frame(document: &RgbaImage, top: u32, height: u32) -> BgraFrame {
        rgba_to_bgra_frame(
            image::imageops::crop_imm(document, 0, top, document.width(), height).to_image(),
        )
    }

    fn frame_with_band_shifts(width: u32, height: u32, shifts: [u32; 3]) -> RgbaImage {
        RgbaImage::from_fn(width, height, |x, y| {
            let band = (x as u64 * 3 / width as u64).min(2) as usize;
            let source_y = y + shifts[band];
            let value = (x * 17 + source_y * 31) as u8;
            Rgba([value, value.wrapping_add(23), value.wrapping_add(71), 255])
        })
    }

    fn captured(frame: BgraFrame, native_scroll_position: Option<i32>) -> CapturedScrollFrame {
        CapturedScrollFrame {
            frame,
            captured_at: Instant::now(),
            native_scroll_position,
            wheel_sequence: 1,
            discontinuity: false,
        }
    }

    fn wait_for_frame_event(worker: &ScrollCaptureWorker) -> bool {
        let started = Instant::now();
        loop {
            match worker.poll_event() {
                Some(ScrollCaptureEvent::FrameAccepted) => return true,
                Some(ScrollCaptureEvent::FrameDiscarded) => return false,
                Some(_) | None if started.elapsed() < Duration::from_secs(1) => {
                    std::thread::yield_now();
                }
                _ => panic!("worker did not finish the submitted frame"),
            }
        }
    }

    #[test]
    fn opencv_shift_appends_only_new_rows() {
        let source = document(80, 180);
        let mut session = ScrollCaptureSession::from_bgra(document_frame(&source, 0, 100)).unwrap();
        let outcome = session
            .push_bgra_frame(document_frame(&source, 35, 100), 1, None)
            .unwrap();
        assert!(outcome.changed);
        assert_eq!(session.stitched.height(), 135);
        assert_eq!(session.current_offset, 35);
        assert_eq!(
            session.stitched.to_image(),
            image::imageops::crop_imm(&source, 0, 0, 80, 135).to_image()
        );
    }

    #[test]
    fn native_scroll_offset_drives_incremental_splice() {
        let source = document(80, 180);
        let mut session = ScrollCaptureSession::from_bgra(document_frame(&source, 0, 100)).unwrap();
        session
            .push_bgra_frame(document_frame(&source, 0, 100), 1, Some(0))
            .unwrap();
        let outcome = session
            .push_bgra_frame(document_frame(&source, 20, 100), 1, Some(20))
            .unwrap();
        assert!(outcome.changed);
        assert_eq!(session.current_offset, 20);
        assert_eq!(session.stitched.height(), 120);
    }

    #[test]
    fn rebaseline_and_confirmation_do_not_advance_canvas() {
        let source = document(80, 220);
        let mut session = ScrollCaptureSession::from_bgra(document_frame(&source, 0, 100)).unwrap();

        session
            .rebaseline(document_frame(&source, 40, 100), Some(40))
            .unwrap();
        assert_eq!(session.current_offset, 0);

        let confirmation = session
            .match_bgra_frame(1, document_frame(&source, 60, 100), 1, Some(60))
            .unwrap();
        session.confirm_match_state(&confirmation);
        assert_eq!(session.current_offset, 0);

        let resumed = session
            .match_bgra_frame(2, document_frame(&source, 80, 100), 1, Some(80))
            .unwrap();
        let outcome = session.commit_matched_frame(resumed).unwrap();
        assert!(outcome.changed);
        assert_eq!(session.current_offset, 20);
        assert_eq!(session.stitched.height(), 120);
    }

    #[test]
    fn visual_match_corrects_non_pixel_native_scroll_units() {
        let source = document(160, 320);
        let mut session = ScrollCaptureSession::from_bgra(document_frame(&source, 0, 180)).unwrap();
        session
            .push_bgra_frame(document_frame(&source, 0, 180), 1, Some(0))
            .unwrap();
        let outcome = session
            .push_bgra_frame(document_frame(&source, 40, 180), 1, Some(20))
            .unwrap();
        assert!(outcome.changed);
        assert_eq!(session.current_offset, 40);
        assert_eq!(session.stitched.height(), 220);
        assert_eq!(session.native_scroll_scale, Some(2.0));
        session
            .push_bgra_frame(document_frame(&source, 80, 180), 1, Some(40))
            .unwrap();
        assert_eq!(session.current_offset, 80);
        assert_eq!(session.stitched.height(), 260);
    }

    #[test]
    fn missing_native_position_does_not_reuse_a_stale_baseline() {
        let source = document(80, 200);
        let mut session = ScrollCaptureSession::from_bgra(document_frame(&source, 0, 100)).unwrap();
        session
            .push_bgra_frame(document_frame(&source, 0, 100), 1, Some(0))
            .unwrap();
        session
            .push_bgra_frame(document_frame(&source, 20, 100), 1, None)
            .unwrap();
        session
            .push_bgra_frame(document_frame(&source, 40, 100), 1, None)
            .unwrap();
        assert_eq!(session.current_offset, 40);
        assert_eq!(session.stitched.height(), 140);
    }

    #[test]
    fn opencv_shift_prepends_only_new_rows() {
        let source = document(160, 300);
        let mut session =
            ScrollCaptureSession::from_bgra(document_frame(&source, 50, 180)).unwrap();
        let outcome = session
            .push_bgra_frame(document_frame(&source, 0, 180), -1, None)
            .unwrap();
        assert!(outcome.changed);
        assert_eq!(session.stitched.height(), 230);
        assert_eq!(session.current_offset, -50);
        assert_eq!(
            session.stitched.to_image(),
            image::imageops::crop_imm(&source, 0, 0, 160, 230).to_image()
        );
    }

    #[test]
    fn reversing_scroll_never_removes_captured_top_or_bottom_rows() {
        let source = document(160, 320);
        let mut session =
            ScrollCaptureSession::from_bgra(document_frame(&source, 50, 180)).unwrap();
        session.native_scroll_position = Some(50);
        session
            .push_bgra_frame(document_frame(&source, 0, 180), -1, Some(0))
            .unwrap();
        session
            .push_bgra_frame(document_frame(&source, 50, 180), 1, Some(50))
            .unwrap();
        session
            .push_bgra_frame(document_frame(&source, 100, 180), 1, Some(100))
            .unwrap();

        assert_eq!(session.stitched.height(), 280);
        assert_eq!(
            session.stitched.to_image(),
            image::imageops::crop_imm(&source, 0, 0, 160, 280).to_image()
        );
    }

    #[test]
    fn returning_to_captured_bottom_does_not_append_it_again() {
        let source = document(80, 200);
        let mut session = ScrollCaptureSession::from_bgra(document_frame(&source, 0, 100)).unwrap();
        session
            .push_bgra_frame(document_frame(&source, 20, 100), 1, None)
            .unwrap();
        session
            .push_bgra_frame(document_frame(&source, 40, 100), 1, Some(40))
            .unwrap();
        session
            .push_bgra_frame(document_frame(&source, 20, 100), -1, None)
            .unwrap();
        session
            .push_bgra_frame(document_frame(&source, 40, 100), 1, None)
            .unwrap();

        assert_eq!(session.stitched.height(), 140);
        assert_eq!(
            session.stitched.to_image(),
            image::imageops::crop_imm(&source, 0, 0, 80, 140).to_image()
        );
    }

    #[test]
    fn opencv_finds_shift_without_external_motion_hint() {
        let source = document(80, 180);
        let first = document_frame(&source, 0, 100);
        let mut session = ScrollCaptureSession::from_bgra(first).unwrap();
        let outcome = session
            .push_bgra_frame(document_frame(&source, 35, 100), 1, None)
            .unwrap();

        assert!(outcome.changed);
        assert_eq!(session.current_offset, 35);
        assert_eq!(session.stitched.height(), 135);
        assert_eq!(
            session.stitched.to_image(),
            image::imageops::crop_imm(&source, 0, 0, 80, 135).to_image()
        );
    }

    #[test]
    fn opencv_rejects_frames_when_independent_bands_disagree() {
        let previous = frame_with_band_shifts(90, 100, [0, 0, 0]);
        let next = frame_with_band_shifts(90, 100, [12, 28, 44]);

        let result = OpenCvWorker::match_frame_shift(&previous, &next, 1, None);

        assert!(matches!(result, Err(ScrollCaptureFailure::MatchLost(_))));
    }

    #[test]
    fn preview_cache_incrementally_tracks_appended_strip() {
        let source = document(60, 140);
        let mut session = ScrollCaptureSession::from_bgra(document_frame(&source, 0, 100)).unwrap();
        session.native_scroll_position = Some(0);
        let initial = session.preview_frame(60, 200);
        assert_eq!((initial.width, initial.height), (60, 100));
        session
            .push_bgra_frame(document_frame(&source, 40, 100), 1, Some(40))
            .unwrap();
        let extended = session.preview_frame(60, 200);
        assert_eq!((extended.width, extended.height), (60, 140));
        assert_eq!(session.preview_cache.as_ref().unwrap().source_height, 140);
    }

    #[test]
    fn preview_is_bounded_by_both_dimensions() {
        let mut session = ScrollCaptureSession::from_bgra(frame(100, 600, 1)).unwrap();
        let preview = session.preview_frame(80, 200);
        assert_eq!((preview.width, preview.height), (33, 200));
    }

    #[test]
    fn tiled_preview_samples_across_appended_strips() {
        let mut tiled = TiledImage::new(RgbaImage::from_pixel(4, 2, Rgba([255, 0, 0, 255])));
        tiled.append(RgbaImage::from_pixel(4, 2, Rgba([0, 0, 255, 255])));
        let preview = tiled.resize_box(2, 4);
        assert_eq!(*preview.get_pixel(0, 0), Rgba([255, 0, 0, 255]));
        assert_eq!(*preview.get_pixel(0, 3), Rgba([0, 0, 255, 255]));
    }

    #[test]
    fn box_preview_preserves_thin_rows_while_long_image_grows() {
        let mut source = RgbaImage::from_pixel(4, 100, Rgba([255, 255, 255, 255]));
        for x in 0..4 {
            source.put_pixel(x, 5, Rgba([0, 0, 0, 255]));
        }
        let tiled = TiledImage::new(source);
        let preview = tiled.resize_box(2, 10);
        assert!(preview.get_pixel(0, 0).0[0] < 255);
    }

    #[test]
    fn box_preview_handles_overlapping_rounding_across_prepended_strips() {
        let mut tiled =
            TiledImage::new(RgbaImage::from_pixel(614, 383, Rgba([255, 255, 255, 255])));
        tiled.prepend(RgbaImage::from_pixel(614, 29, Rgba([0, 0, 255, 255])));
        tiled.prepend(RgbaImage::from_pixel(614, 18, Rgba([255, 0, 0, 255])));

        let preview = tiled.resize_box(264, 185);
        assert_eq!(preview.dimensions(), (264, 185));
        assert!(preview.get_pixel(0, 0).0[0] > preview.get_pixel(0, 0).0[2]);
    }

    #[test]
    fn worker_confirms_accepted_frame() {
        let selection = sc_app::selection::RectI32::from_points(0, 0, 60, 100);
        let source = document(60, 140);
        let worker =
            ScrollCaptureWorker::from_bgra(selection, document_frame(&source, 0, 100), Some(0))
                .unwrap();
        assert!(
            worker
                .push_frame(
                    captured(document_frame(&source, 20, 100), Some(20)),
                    1,
                    None
                )
                .unwrap()
        );

        let started = Instant::now();
        loop {
            match worker.poll_event() {
                Some(ScrollCaptureEvent::FrameAccepted) => break,
                Some(_) | None if started.elapsed() < Duration::from_secs(1) => {
                    std::thread::yield_now();
                }
                _ => panic!("worker did not confirm the submitted frame"),
            }
        }
    }

    #[test]
    fn export_waits_for_ordered_splice_commands() {
        let selection = sc_app::selection::RectI32 {
            left: 0,
            top: 0,
            right: 60,
            bottom: 100,
        };
        let source = document(60, 180);
        let worker =
            ScrollCaptureWorker::from_bgra(selection, document_frame(&source, 0, 100), Some(0))
                .unwrap();

        for top in [20, 40] {
            assert!(
                worker
                    .push_frame(
                        captured(document_frame(&source, top, 100), Some(top as i32)),
                        1,
                        None,
                    )
                    .unwrap()
            );
            let started = Instant::now();
            loop {
                match worker.poll_event() {
                    Some(ScrollCaptureEvent::FrameAccepted) => break,
                    Some(_) | None if started.elapsed() < Duration::from_secs(1) => {
                        std::thread::yield_now();
                    }
                    _ => panic!("splice worker did not confirm frame at {top}"),
                }
            }
        }

        let bmp = worker.bmp_data().unwrap();
        let exported = image::load_from_memory(&bmp).unwrap();
        assert_eq!(exported.width(), 60);
        assert_eq!(exported.height(), 140);
    }

    #[test]
    fn discontinuity_rebaselines_and_resumes_on_the_next_continuous_frame() {
        let selection = sc_app::selection::RectI32::from_points(0, 0, 60, 100);
        let source = document(60, 220);
        let worker =
            ScrollCaptureWorker::from_bgra(selection, document_frame(&source, 0, 100), Some(0))
                .unwrap();

        let mut gap = captured(document_frame(&source, 20, 100), Some(20));
        gap.discontinuity = true;
        assert!(worker.push_frame(gap, 1, None).unwrap());
        while worker.pending_frames() != 0 {
            std::thread::yield_now();
        }

        assert!(
            worker
                .push_frame(
                    captured(document_frame(&source, 40, 100), Some(40)),
                    1,
                    None,
                )
                .unwrap()
        );
        assert!(wait_for_frame_event(&worker));
        let resumed = image::load_from_memory(&worker.bmp_data().unwrap()).unwrap();
        assert_eq!(resumed.height(), 120);
    }

    #[test]
    fn rejected_frame_keeps_the_last_accepted_anchor() {
        let selection = sc_app::selection::RectI32::from_points(0, 0, 160, 180);
        let source = document(160, 600);
        let worker =
            ScrollCaptureWorker::from_bgra(selection, document_frame(&source, 0, 180), None)
                .unwrap();

        assert!(
            worker
                .push_frame(captured(document_frame(&source, 300, 180), None), 1, None)
                .unwrap()
        );
        assert!(!wait_for_frame_event(&worker));

        assert!(
            worker
                .push_frame(captured(document_frame(&source, 40, 180), None), 1, None)
                .unwrap()
        );
        assert!(wait_for_frame_event(&worker));
        let recovered = image::load_from_memory(&worker.bmp_data().unwrap()).unwrap();
        assert_eq!(recovered.height(), 220);
        assert_eq!(
            recovered.to_rgba8(),
            image::imageops::crop_imm(&source, 0, 0, 160, 220).to_image()
        );
    }
}
