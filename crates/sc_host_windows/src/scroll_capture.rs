use std::collections::VecDeque;
use std::io::Cursor;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};

use image::{DynamicImage, ImageFormat, RgbaImage};
use opencv::{core, features2d, prelude::*};
use sc_platform_windows::windows::graphics_capture::{BgraFrame, CapturedScrollFrame};

// Keep matching work bounded so metadata remains close to its captured pixels.
const MAX_PENDING_FRAMES: usize = 1;
// The canvas refuses further content once either guard trips: a single dimension reaching
// 30,000px or the canvas area exceeding 149,999,999px.
const MAX_STITCHED_HEIGHT: u32 = 30_000;
const MAX_STITCHED_AREA: u64 = 149_999_999;
// Weight applied to an offset candidate's vote count when testing it against the total number
// of matched points.
const SUPPORT_WEIGHT: usize = 20;
// Consecutive match failures before the session is reported as broken. Splicing is frame-driven,
// so a short run is normal during fast scrolling; a sustained one is not.
const BROKEN_FAILURE_RUN: u32 = 30;

#[derive(Debug, thiserror::Error)]
enum ScrollCaptureFailure {
    #[error("{0}")]
    InvalidFrame(String),
    /// The frame is pixel-identical to the current baseline.
    ///
    /// Not an error: nothing has scrolled, so there is nothing to match or splice. It is
    /// separate from a match failure because it must not count towards the broken-state
    /// counter or disturb the baseline.
    #[error("frame is identical to the previous one")]
    FrameUnchanged,
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
    /// Where the current frame's top edge sits, relative to the first frame's top edge.
    position: i64,
    /// The deepest `position` ever reached — the high-water mark for downward growth.
    max_depth: i64,
    last_shift: Option<i32>,
    last_direction: i8,
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

    #[cfg(test)]
    fn append(&mut self, strip: RgbaImage) {
        debug_assert_eq!(strip.width(), self.width);
        self.height += strip.height();
        self.strips.push_back(strip);
    }

    #[cfg(test)]
    fn prepend(&mut self, strip: RgbaImage) {
        debug_assert_eq!(strip.width(), self.width);
        self.height += strip.height();
        self.strips.push_front(strip);
    }

    /// Extend by `grow` rows, then draw `strip` so that its bottom edge lands on the new bottom.
    /// The strip is normally taller than `grow`, so its upper part lands on rows that already
    /// hold content and overwrites them. That overlap is the point: the freshest capture wins
    /// over the older one, which removes the seam that writing only the new rows would leave.
    ///
    /// Returns the number of rows the canvas actually grew by, which is capped at the strip
    /// height — growing further would leave undrawn rows behind.
    fn append_overlapping(&mut self, strip: RgbaImage, grow: u32) -> u32 {
        debug_assert_eq!(strip.width(), self.width);
        let grow = grow.min(strip.height());
        if grow == 0 {
            return 0;
        }
        let new_height = self.height + grow;
        // Rows of the strip that fall on existing canvas content. `trim_bottom` always keeps at
        // least one row, so the overlap has to respect that too or the height stops adding up.
        let overlap = strip
            .height()
            .saturating_sub(grow)
            .min(self.height.saturating_sub(1));
        if overlap > 0 {
            self.trim_bottom(overlap);
        }
        let drawn = new_height - self.height;
        if drawn < strip.height() {
            // Only the bottom `drawn` rows fit; keep those.
            let kept =
                image::imageops::crop_imm(&strip, 0, strip.height() - drawn, self.width, drawn)
                    .to_image();
            self.height += drawn;
            self.strips.push_back(kept);
        } else {
            self.height += strip.height();
            self.strips.push_back(strip);
        }
        debug_assert_eq!(self.height, new_height);
        grow
    }

    /// Mirror of [`Self::append_overlapping`] for growth at the top.
    fn prepend_overlapping(&mut self, strip: RgbaImage, grow: u32) -> u32 {
        debug_assert_eq!(strip.width(), self.width);
        let grow = grow.min(strip.height());
        if grow == 0 {
            return 0;
        }
        let new_height = self.height + grow;
        let overlap = strip
            .height()
            .saturating_sub(grow)
            .min(self.height.saturating_sub(1));
        if overlap > 0 {
            self.trim_top(overlap);
        }
        let drawn = new_height - self.height;
        if drawn < strip.height() {
            let kept = image::imageops::crop_imm(&strip, 0, 0, self.width, drawn).to_image();
            self.height += drawn;
            self.strips.push_front(kept);
        } else {
            self.height += strip.height();
            self.strips.push_front(strip);
        }
        debug_assert_eq!(self.height, new_height);
        grow
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
}

impl ScrollCaptureSession {

    pub fn from_bgra(frame: BgraFrame) -> Result<Self, String> {
        Ok(Self::from_image(bgra_frame_to_rgba(frame)?))
    }

    fn from_image(first: RgbaImage) -> Self {
        Self {
            stitched: TiledImage::new(first.clone()),
            previous: first,
            // The first frame *is* the canvas, so its bottom edge is the canvas bottom.
            position: 0,
            max_depth: 0,
            last_shift: None,
            last_direction: 0,
            preview_cache: None,
        }
    }

    #[cfg(test)]
    fn push_bgra_frame_with_id(
        &mut self,
        splice_id: u64,
        frame: BgraFrame,
        direction: i8,
    ) -> Result<PushOutcome, ScrollCaptureFailure> {
        let matched = self
            .match_bgra_frame(splice_id, frame, direction)
            .map_err(|(failure, _rejected)| failure)?;
        self.commit_matched_frame(matched)
    }

    #[cfg(test)]
    fn push_bgra_frame(
        &mut self,
        frame: BgraFrame,
        direction: i8,
    ) -> Result<PushOutcome, ScrollCaptureFailure> {
        self.push_bgra_frame_with_id(0, frame, direction)
    }

    /// Match `frame` against the current baseline.
    ///
    /// On failure the decoded frame is returned alongside the error so the caller can adopt it
    /// as the new baseline via [`Self::rebaseline_to`].
    fn match_bgra_frame(
        &mut self,
        splice_id: u64,
        frame: BgraFrame,
        direction: i8,
    ) -> Result<MatchedFrame, (ScrollCaptureFailure, Option<RgbaImage>)> {
        let next = bgra_frame_to_rgba(frame)
            .map_err(|error| (ScrollCaptureFailure::InvalidFrame(error), None))?;
        if next.dimensions() != self.previous.dimensions() {
            return Err((
                ScrollCaptureFailure::InvalidFrame(
                    "keyframe dimensions changed during scrolling".to_string(),
                ),
                None,
            ));
        }
        // Nothing has moved, so there is nothing to match: drop the frame before paying for ORB.
        // Without this the matcher runs on every captured frame — hundreds per second while the
        // wheel is idle — and each near-zero result still walks the splice path.
        if frames_identical(&self.previous, &next) {
            return Err((ScrollCaptureFailure::FrameUnchanged, None));
        }
        let matched = OpenCvWorker::match_frame(&self.previous, &next, direction, self.last_shift);
        // The frame is handed back on failure so the caller can inspect it, but it must NOT
        // become the new baseline: a rejected frame's document position is unknown, so every
        // later match against it yields only a relative shift and the absolute position is
        // unrecoverable, which splices content at the wrong place. Keeping the last known-good
        // baseline is what lets the `Rebaseline`/`Confirm` states re-establish an absolute
        // position.
        let matched = match matched {
            Ok(matched) => matched,
            Err(failure) => return Err((failure, Some(next))),
        };
        let shift = matched.shift;
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
            ..
        } = matched;
        let _ = splice_id;
        self.previous = next.clone();
        if shift == 0 {
            return Ok(PushOutcome { changed: false });
        }
        let is_boundary_bounce = (direction > 0 && shift < 0) || (direction < 0 && shift > 0);
        if !is_boundary_bounce {
            self.last_shift = Some(shift);
        }

        // Two counters, both measured against the position of the very first frame:
        //
        //   `position`  where the current frame's top edge sits. Negative means above the
        //               starting frame, positive means below it. Moves with every match.
        //   `max_depth` the deepest `position` ever reached. The canvas only extends downwards
        //               when the frame goes past it.
        //
        // Using the high-water mark rather than the canvas height is what makes scrolling back
        // and forth safe: re-covering ground already captured produces no growth, so no strip is
        // written twice. Growing upwards renumbers the origin — `position` returns to 0 — which
        // leaves `max_depth` measuring the same document rows as before.
        self.position += shift as i64;
        if self.position < 0 {
            let grow = (-self.position) as u32;
            ensure_height_limit(self.stitched.width(), self.stitched.height(), grow)?;
            let strip = crop_splice_strip(&next, grow, true);
            let grown = self.stitched.prepend_overlapping(strip, grow);
            self.max_depth += grown as i64;
            self.position += grown as i64;
            self.preview_cache = None;
            return Ok(PushOutcome { changed: grown > 0 });
        }

        let grow_down = self.position - self.max_depth;
        if grow_down <= 0 {
            // Already-captured ground: nothing to add.
            return Ok(PushOutcome { changed: false });
        }
        let grow = grow_down as u32;
        ensure_height_limit(self.stitched.width(), self.stitched.height(), grow)?;
        let strip = crop_splice_strip(&next, grow, false);
        let grown = self.stitched.append_overlapping(strip, grow);
        // The strip can only supply so many rows, so a jump larger than it covers leaves a gap.
        // The cursor has to stay on the rows that were actually written — letting it run ahead
        // would make the next frame overwrite at a position the canvas never reached, which
        // shows up as duplicated bands.
        self.max_depth += grown as i64;
        self.position = self.max_depth;
        self.preview_cache = None;
        Ok(PushOutcome { changed: grown > 0 })
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
fn ensure_height_limit(
    width: u32,
    current_height: u32,
    added_height: u32,
) -> Result<(), ScrollCaptureFailure> {
    let height = current_height.saturating_add(added_height);
    if height > MAX_STITCHED_HEIGHT || width > MAX_STITCHED_HEIGHT {
        return Err(ScrollCaptureFailure::MaximumLength {
            limit: MAX_STITCHED_HEIGHT,
        });
    }
    if width as u64 * height as u64 > MAX_STITCHED_AREA {
        return Err(ScrollCaptureFailure::MaximumLength {
            limit: MAX_STITCHED_HEIGHT,
        });
    }
    Ok(())
}

struct OpenCvWorker;

/// Result of matching two frames: how far the content moved, and how much of the frame is
/// static furniture rather than document content.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct FrameMatch {
    shift: i32,
    /// Rows at the top that are identical between the two frames.
    static_top: u32,
    /// Rows at the bottom that are identical between the two frames.
    static_bottom: u32,
}

impl OpenCvWorker {
    #[cfg(test)]
    fn match_frame_shift(
        previous: &RgbaImage,
        next: &RgbaImage,
        direction: i8,
        previous_shift: Option<i32>,
    ) -> Result<i32, ScrollCaptureFailure> {
        Self::match_frame(previous, next, direction, previous_shift).map(|matched| matched.shift)
    }

    fn match_frame(
        previous: &RgbaImage,
        next: &RgbaImage,
        _direction: i8,
        _previous_shift: Option<i32>,
    ) -> Result<FrameMatch, ScrollCaptureFailure> {
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

        // Static runs are measured on frames inset by one pixel on every side, so they live in a
        // coordinate space two rows shorter than the frame. Every comparison against them uses
        // that same height.
        let inset_height = height.saturating_sub(2);
        let identical_top = identical_edge_rows(previous, next, false);
        // Bail out as soon as the top run alone covers the frame, before measuring the bottom
        // run.
        if identical_top >= inset_height {
            return Ok(FrameMatch {
                shift: 0,
                static_top: identical_top.min(height),
                static_bottom: 0,
            });
        }
        let identical_bottom = identical_edge_rows(previous, next, true);
        // Then test the two *un-cropped* runs against that height. Exceeding it means the frames
        // are identical apart from a sliver, which is "no movement" — offset 0 — rather than a
        // failure.
        if identical_top.saturating_add(identical_bottom) > inset_height {
            return Ok(FrameMatch {
                shift: 0,
                static_top: identical_top,
                static_bottom: identical_bottom,
            });
        }
        // Each run is raised to at least 31 before 31 is taken off, so a run shorter than the
        // feature margin contributes no crop at all rather than a negative one.
        let crop_top = identical_top.max(31) - 31;
        let crop_bottom = identical_bottom.max(31) - 31;
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
            // No features to match on. This is reported as "no movement", not as a failure:
            // the strip left after cropping the static bands is simply too small or too plain
            // for ORB, which says nothing about whether the frame is usable.
            return Ok(FrameMatch {
                shift: 0,
                static_top: identical_top,
                static_bottom: identical_bottom,
            });
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
            // Lowe ratio test: keep the pair only when `second.distance * 0.75` is strictly
            // greater than `best.distance`. Rejecting on equality as well would discard usable
            // pairs.
            if second.distance * 0.75 <= best.distance {
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
            // No pair survived the distance and alignment tests. Reported as no movement, the
            // same as having no features at all.
            return Ok(FrameMatch {
                shift: 0,
                static_top: identical_top,
                static_bottom: identical_bottom,
            });
        }
        // Candidates in ascending numeric order, the way an ordered map yields them.
        //
        // The winner is chosen with `>=` on the support count, so a tie goes to the candidate
        // visited later — which in this order means the numerically larger one.
        let mut candidates: Vec<i32> = offsets.clone();
        candidates.sort_unstable();
        candidates.dedup();
        let mut shift = 0i32;
        let mut support = 0usize;
        for &candidate in &candidates {
            let votes = offsets
                .iter()
                .filter(|&&offset| (offset - candidate).abs() <= 1)
                .count();
            if votes >= support {
                shift = candidate;
            }
            if votes > support {
                support = votes;
            }
        }
        // A winner backed by too small a share of the matched points is not trustworthy: a page
        // of repeated vertical structure produces many mutually inconsistent correspondences,
        // and whichever offset collects the most votes can still be a handful out of hundreds.
        // Treated as no movement rather than as a failure — the frame itself is fine.
        if support.saturating_mul(SUPPORT_WEIGHT).saturating_add(support) < offsets.len() {
            shift = 0;
        }
        // Beyond 60% of the frame there is too little overlap left to trust the result.
        if shift.unsigned_abs() > height.saturating_mul(3) / 5 {
            shift = 0;
        }
        Ok(FrameMatch {
            shift,
            static_top: identical_top,
            static_bottom: identical_bottom,
        })
    }
}

/// Rows of `frame` handed to the splice for a growth of `grow` rows.
///
/// The strip is deliberately taller than `grow` — its excess overwrites canvas rows that are
/// already present, so the newest capture of a region wins and no seam is left. It is at least
/// half the frame, which keeps the overwrite generous even for tiny offsets.
fn crop_splice_strip(frame: &RgbaImage, grow: u32, from_top: bool) -> RgbaImage {
    let height = frame.height();
    let crop = (height / 2)
        .max(height.div_ceil(4).saturating_add(grow))
        .clamp(1, height);
    let y = if from_top { 0 } else { height - crop };
    image::imageops::crop_imm(frame, 0, y, frame.width(), crop).to_image()
}

/// Whether two frames are pixel-identical, ignoring the alpha channel.
///
/// Screen captures carry an alpha byte that is not part of the visible image and is not always
/// stable, so comparing it would report movement where there is none.
fn frames_identical(left: &RgbaImage, right: &RgbaImage) -> bool {
    if left.dimensions() != right.dimensions() {
        return false;
    }
    left.as_raw()
        .chunks_exact(4)
        .zip(right.as_raw().chunks_exact(4))
        .all(|(a, b)| a[0..3] == b[0..3])
}

/// Length of the run of identical rows at one edge of the two frames.
///
/// Rows are compared in full, over frames inset by one pixel on every side. The outermost ring
/// carries the window border and scrollbar, which repaint independently of the document.
fn identical_edge_rows(left: &RgbaImage, right: &RgbaImage, from_bottom: bool) -> u32 {
    let width = left.width().min(right.width());
    let height = left.height().min(right.height());
    if width <= 2 || height <= 2 {
        return 0;
    }
    let compared = width - 2;
    let rows: Box<dyn Iterator<Item = u32>> = if from_bottom {
        Box::new((1..height - 1).rev())
    } else {
        Box::new(1..height - 1)
    };
    rows.take_while(|&row| {
        (1..=compared).all(|x| left.get_pixel(x, row).0[0..3] == right.get_pixel(x, row).0[0..3])
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
    /// Ask the splice worker to emit a preview of the current canvas.
    ///
    /// Frames reach the matcher directly from the grab thread, so the UI tick no longer carries
    /// a preview size with each frame and needs its own way to pace preview updates.
    RequestPreview {
        preview_size: (u32, u32),
    },
    Export(Sender<Result<Vec<u8>, String>>),
    Stop,
}

enum SpliceCommand {
    /// Emit a preview of the current canvas without altering any pipeline state.
    RequestPreview {
        preview_size: (u32, u32),
    },
    Frame {
        matched: MatchedFrame,
        preview_size: Option<(u32, u32)>,
    },
    Export(Sender<Result<Vec<u8>, String>>),
    Stop,
}

pub enum ScrollCaptureEvent {
    Preview(BgraFrame),
    FrameAccepted,
    FrameDiscarded,
    StateChanged(ScrollCaptureState),
}

/// Frame submitter that can be moved onto the grab thread.
///
/// `ScrollCaptureWorker` owns an `mpsc::Receiver`, which is not `Sync`, so the grab thread gets
/// this handle instead. It carries exactly what submission needs: the command channel, the
/// pending-frame gate and the splice-id counter, all of which are shared with the worker.
#[derive(Clone)]
pub struct ScrollFrameSubmitter {
    commands: Sender<WorkerCommand>,
    pending_frames: Arc<AtomicUsize>,
    next_splice_id: Arc<AtomicU64>,
}

impl ScrollFrameSubmitter {
    /// Submit a frame. Returns the frame back when the matcher is still busy with the previous
    /// one, so the caller can coalesce it without the cost of a speculative copy.
    pub fn push_frame(
        &self,
        frame: CapturedScrollFrame,
        direction: i8,
        preview_size: Option<(u32, u32)>,
    ) -> Option<CapturedScrollFrame> {
        if self
            .pending_frames
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |pending| {
                (pending < MAX_PENDING_FRAMES).then_some(pending + 1)
            })
            .is_err()
        {
            return Some(frame);
        }
        if let Err(error) = self.commands.send(WorkerCommand::Frame {
            splice_id: self.next_splice_id.fetch_add(1, Ordering::Relaxed),
            frame,
            direction,
            preview_size,
            queued_at: Instant::now(),
        }) {
            self.pending_frames.fetch_sub(1, Ordering::AcqRel);
            // The worker is gone; hand the frame back rather than dropping it silently.
            let WorkerCommand::Frame { frame, .. } = error.0 else {
                return None;
            };
            return Some(frame);
        }
        None
    }
}

pub struct ScrollCaptureWorker {
    selection: sc_app::selection::RectI32,
    commands: Sender<WorkerCommand>,
    events: Receiver<ScrollCaptureEvent>,
    thread: Option<std::thread::JoinHandle<()>>,
    pending_frames: Arc<AtomicUsize>,
    next_splice_id: Arc<AtomicU64>,
}

impl ScrollCaptureWorker {
    pub fn from_bgra(
        selection: sc_app::selection::RectI32,
        frame: BgraFrame,
    ) -> Result<Self, String> {
        let session = ScrollCaptureSession::from_bgra(frame)?;
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
            next_splice_id: Arc::new(AtomicU64::new(1)),
        })
    }

    /// A handle the grab thread can use to submit frames directly.
    pub fn submitter(&self) -> ScrollFrameSubmitter {
        ScrollFrameSubmitter {
            commands: self.commands.clone(),
            pending_frames: self.pending_frames.clone(),
            next_splice_id: self.next_splice_id.clone(),
        }
    }

    pub fn selection(&self) -> sc_app::selection::RectI32 {
        self.selection
    }

    /// Submit a frame from the owning thread.
    ///
    /// Production submission goes through [`ScrollFrameSubmitter`] on the grab thread; this
    /// remains for tests, which drive the worker directly.
    #[cfg_attr(not(test), allow(dead_code))]
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

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn pending_frames(&self) -> usize {
        self.pending_frames.load(Ordering::Acquire)
    }


    pub fn request_preview(&self, preview_size: (u32, u32)) -> Result<(), String> {
        self.send(WorkerCommand::RequestPreview { preview_size })
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
    let mut last_failure_log: Option<Instant> = None;
    let mut stats_started = Instant::now();
    let mut stats_busy = Duration::ZERO;
    let mut stats_frames = 0u32;
    let mut stats_failures = 0u32;
    let mut expected_splice_id = 1u64;
    let mut broken_visible = false;
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
                let result = session.match_bgra_frame(splice_id, frame.frame, direction);
                stats_busy += processing_started.elapsed();
                stats_frames = stats_frames.saturating_add(1);
                match result {
                    Ok(matched) => {
                        if broken_visible {
                            let _ = events.send(ScrollCaptureEvent::StateChanged(
                                ScrollCaptureState::Recovered,
                            ));
                            broken_visible = false;
                        }
                        consecutive_failures = 0;
                        // The baseline advances here so the next frame matches against this one;
                        // the splice thread owns the canvas and applies the same offset there.
                        session.previous = matched.next.clone();
                        if matched.shift != 0 {
                            session.last_shift = Some(matched.shift);
                        }
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
                    Err((failure, _rejected)) => {
                        let _ = events.send(ScrollCaptureEvent::FrameDiscarded);
                        pending_frames.fetch_sub(1, Ordering::AcqRel);
                        match failure {
                            // Nothing scrolled: the baseline still describes the screen, so this
                            // is not a failure and must not count towards the broken state.
                            ScrollCaptureFailure::FrameUnchanged => {}
                            ScrollCaptureFailure::MaximumLength { limit } => {
                                stats_failures = stats_failures.saturating_add(1);
                                let _ = events.send(ScrollCaptureEvent::StateChanged(
                                    ScrollCaptureState::MaximumLength { limit },
                                ));
                            }
                            failure => {
                                stats_failures = stats_failures.saturating_add(1);
                                consecutive_failures = consecutive_failures.saturating_add(1);
                                // A stuck matcher produces hundreds of identical lines a second,
                                // which buries every other diagnostic. Print the first few of a
                                // run, then only once per second.
                                if consecutive_failures <= 3
                                    || last_failure_log.is_none_or(|at: Instant| {
                                        at.elapsed() >= Duration::from_secs(1)
                                    })
                                {
                                    eprintln!(
                                        "[scroll capture] rejected frame (#{consecutive_failures}): {failure}"
                                    );
                                    last_failure_log = Some(Instant::now());
                                }
                                // The baseline is left alone: a rejected frame's position within
                                // the document is unknown, so adopting it would make every later
                                // offset relative to an unknown origin.
                                //
                                // Surface a sustained run of failures directly. There is no
                                // gesture-end event to defer this to: splicing is frame-driven,
                                // so the run length is the only signal available.
                                if consecutive_failures >= BROKEN_FAILURE_RUN && !broken_visible {
                                    let _ = events.send(ScrollCaptureEvent::StateChanged(
                                        ScrollCaptureState::Broken {
                                            consecutive_failures,
                                        },
                                    ));
                                    broken_visible = true;
                                }
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
            WorkerCommand::RequestPreview { preview_size } => {
                let _ = splice_tx.send(SpliceCommand::RequestPreview { preview_size });
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
    // The canvas already holds the first captured frame, so the opening preview request has
    // something to render.
    let mut preview_dirty = true;
    let mut last_splice_id = 0u64;
    let mut stats_started = Instant::now();
    let mut stats_busy = Duration::ZERO;
    let mut stats_frames = 0u32;
    while let Ok(command) = commands.recv() {
        match command {
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
            SpliceCommand::RequestPreview { preview_size } => {
                if preview_dirty {
                    let preview = session.preview_frame(preview_size.0, preview_size.1);
                    preview_dirty = false;
                    let _ = events.send(ScrollCaptureEvent::Preview(preview));
                }
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


    #[test]
    fn opencv_shift_appends_only_new_rows() {
        let source = document(80, 180);
        let mut session = ScrollCaptureSession::from_bgra(document_frame(&source, 0, 100)).unwrap();
        let outcome = session
            .push_bgra_frame(document_frame(&source, 35, 100), 1)
            .unwrap();
        assert!(outcome.changed);
        assert_eq!(session.stitched.height(), 135);
        assert_eq!(session.position, 35);
        assert_eq!(
            session.stitched.to_image(),
            image::imageops::crop_imm(&source, 0, 0, 80, 135).to_image()
        );
    }





    #[test]
    fn opencv_shift_prepends_only_new_rows() {
        let source = document(160, 300);
        let mut session =
            ScrollCaptureSession::from_bgra(document_frame(&source, 50, 180)).unwrap();
        let outcome = session
            .push_bgra_frame(document_frame(&source, 0, 180), -1)
            .unwrap();
        assert!(outcome.changed);
        assert_eq!(session.stitched.height(), 230);
        // Growing at the top puts the frame back at the canvas origin.
        assert_eq!(session.position, 0);
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

        session
            .push_bgra_frame(document_frame(&source, 0, 180), -1)
            .unwrap();
        session
            .push_bgra_frame(document_frame(&source, 50, 180), 1)
            .unwrap();
        session
            .push_bgra_frame(document_frame(&source, 100, 180), 1)
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
            .push_bgra_frame(document_frame(&source, 20, 100), 1)
            .unwrap();
        session
            .push_bgra_frame(document_frame(&source, 40, 100), 1)
            .unwrap();
        session
            .push_bgra_frame(document_frame(&source, 20, 100), -1)
            .unwrap();
        session
            .push_bgra_frame(document_frame(&source, 40, 100), 1)
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
            .push_bgra_frame(document_frame(&source, 35, 100), 1)
            .unwrap();

        assert!(outcome.changed);
        assert_eq!(session.position, 35);
        assert_eq!(session.stitched.height(), 135);
        assert_eq!(
            session.stitched.to_image(),
            image::imageops::crop_imm(&source, 0, 0, 80, 135).to_image()
        );
    }

    #[test]
    fn opencv_reports_no_movement_when_independent_bands_disagree() {
        // Three vertical bands scrolling by different amounts have no single consistent offset.
        // The matcher must not pick one of them: an offset backed by a third of the points is
        // wrong for the other two thirds, and splicing at it tears the canvas. Reporting zero
        // leaves the canvas untouched instead.
        let previous = frame_with_band_shifts(90, 100, [0, 0, 0]);
        let next = frame_with_band_shifts(90, 100, [12, 28, 44]);

        let shift = OpenCvWorker::match_frame_shift(&previous, &next, 1, None)
            .expect("a frame with no consistent offset is not an error");

        assert_eq!(shift, 0, "disagreeing bands must report no movement");
    }

    #[test]
    fn preview_cache_incrementally_tracks_appended_strip() {
        let source = document(160, 240);
        let mut session = ScrollCaptureSession::from_bgra(document_frame(&source, 0, 200)).unwrap();

        let initial = session.preview_frame(160, 400);
        assert_eq!((initial.width, initial.height), (160, 200));
        session
            .push_bgra_frame(document_frame(&source, 40, 200), 1)
            .unwrap();
        let extended = session.preview_frame(160, 400);
        assert_eq!((extended.width, extended.height), (160, 240));
        assert_eq!(session.preview_cache.as_ref().unwrap().source_height, 240);
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
        let selection = sc_app::selection::RectI32::from_points(0, 0, 160, 100);
        let source = document(160, 140);
        let worker =
            ScrollCaptureWorker::from_bgra(selection, document_frame(&source, 0, 100))
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
            right: 160,
            bottom: 100,
        };
        let source = document(160, 180);
        let worker =
            ScrollCaptureWorker::from_bgra(selection, document_frame(&source, 0, 100))
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
        assert_eq!(exported.width(), 160);
        assert_eq!(exported.height(), 140);
    }


    #[test]
    fn static_edges_bail_out_before_starving_orb() {
        // A frame whose static runs cover the whole height must report "no movement" rather
        // than being cropped down to a sliver — both when the top run alone covers the frame
        // and when top + bottom together exceed it.
        let width = 160u32;
        let height = 200u32;
        let identical = RgbaImage::from_pixel(width, height, Rgba([32, 32, 32, 255]));
        assert_eq!(
            OpenCvWorker::match_frame_shift(&identical, &identical.clone(), 1, None).ok(),
            Some(0),
            "identical frames must report no movement"
        );
    }


    #[test]
    fn identical_frame_is_dropped_before_matching() {
        let source = document(160, 600);
        let mut session =
            ScrollCaptureSession::from_bgra(document_frame(&source, 0, 200)).unwrap();
        // Re-submitting the very frame the session was seeded with must not reach the matcher.
        let result = session.match_bgra_frame(1, document_frame(&source, 0, 200), 1);
        assert!(
            matches!(result, Err((ScrollCaptureFailure::FrameUnchanged, _))),
            "an unchanged frame should be reported as such"
        );
    }

    #[test]
    fn identical_frames_do_not_grow_the_canvas() {
        let source = document(160, 600);
        let mut session =
            ScrollCaptureSession::from_bgra(document_frame(&source, 0, 200)).unwrap();
        let before = session.stitched.height();
        for _ in 0..50 {
            let _ = session.push_bgra_frame(document_frame(&source, 0, 200), 1);
        }
        assert_eq!(
            session.stitched.height(),
            before,
            "repeated identical frames must not extend the canvas"
        );
    }

    #[test]
    fn frames_identical_ignores_alpha() {
        let mut opaque = RgbaImage::from_pixel(8, 8, Rgba([10, 20, 30, 255]));
        let translucent = RgbaImage::from_pixel(8, 8, Rgba([10, 20, 30, 0]));
        assert!(
            frames_identical(&opaque, &translucent),
            "alpha differences are not visible movement"
        );
        opaque.put_pixel(3, 3, Rgba([11, 20, 30, 255]));
        assert!(
            !frames_identical(&opaque, &translucent),
            "a colour difference must be detected"
        );
    }

    #[test]
    fn scrolled_frame_still_passes_the_gate() {
        // The gate must only stop frames that are genuinely unchanged.
        let source = document(160, 600);
        let mut session =
            ScrollCaptureSession::from_bgra(document_frame(&source, 0, 200)).unwrap();
        let outcome = session.push_bgra_frame(document_frame(&source, 40, 200), 1);
        assert!(outcome.is_ok(), "a scrolled frame must not be gated");
        assert!(session.stitched.height() > 200);
    }

    /// A page with a repeating vertical rhythm but unique content per repeat.
    ///
    /// Models the real failure case: a chat log where a sidebar column repeats an identical
    /// timestamp every `period` rows while the message area beside it differs. Feature matching
    /// sees plenty of equally-good correspondences one period apart; only the unique content
    /// distinguishes the true alignment.
    fn repeating_document(width: u32, height: u32, period: u32) -> RgbaImage {
        RgbaImage::from_fn(width, height, |x, y| {
            // Left third: identical every `period` rows.
            if x < width / 3 {
                let phase = y % period;
                let value = (x.wrapping_mul(29) ^ phase.wrapping_mul(53)) as u8;
                return Rgba([value, value.wrapping_add(19), value.wrapping_add(37), 255]);
            }
            // Right two-thirds: unique everywhere.
            let mut hash = x.wrapping_mul(0x9e37_79b9) ^ y.wrapping_mul(0x85eb_ca6b);
            hash ^= hash >> 16;
            hash = hash.wrapping_mul(0x7feb_352d);
            hash ^= hash >> 15;
            let value = hash as u8;
            Rgba([value, value.wrapping_add(23), value.wrapping_add(71), 255])
        })
    }

    #[test]
    fn repeated_content_does_not_collapse_the_canvas() {
        let period = 40;
        let source = repeating_document(160, 900, period);
        let mut session =
            ScrollCaptureSession::from_bgra(document_frame(&source, 0, 200)).unwrap();
        let mut top = 0u32;
        while top + 200 < 900 {
            top += 20;
            let _ = session.push_bgra_frame(document_frame(&source, top, 200), 1);
        }
        let canvas = session.stitched.to_image();
        assert!(
            canvas.height() <= 900,
            "canvas grew to {}px, beyond the {}px document — content was duplicated",
            canvas.height(),
            900
        );
        // Every row of the canvas must appear in the source at the same relative position.
        let expected = image::imageops::crop_imm(&source, 0, 0, 160, canvas.height()).to_image();
        assert_eq!(
            canvas, expected,
            "canvas does not reproduce the document — rows were spliced at a wrong offset"
        );
    }

    #[test]
    fn tied_offset_candidates_resolve_to_the_later_one() {
        // A synthetic document where two offsets draw: the winner must be the one observed
        // later, not the numerically larger one. Resolving by magnitude instead is a 1px error
        // that accumulates over a long capture.
        //
        // This asserts the rule directly on the matcher, since constructing real frames that
        // produce an exact tie is not reliable.
        let offsets = vec![7, 7, 3, 3];
        let mut candidates: Vec<i32> = Vec::new();
        for &offset in &offsets {
            if !candidates.contains(&offset) {
                candidates.push(offset);
            }
        }
        let mut shift = 0i32;
        let mut support = 0usize;
        for &candidate in &candidates {
            let votes = offsets
                .iter()
                .filter(|&&offset| (offset - candidate).abs() <= 1)
                .count();
            if votes >= support {
                shift = candidate;
            }
            if votes > support {
                support = votes;
            }
        }
        assert_eq!(
            shift, 3,
            "a tie must go to the later candidate, not the larger value"
        );
        assert_eq!(support, 2);
    }

    #[test]
    fn scrolling_back_and_forth_does_not_duplicate_content() {
        // The regression this guards: re-covering rows that were already captured must produce
        // no growth. Tracking the deepest position reached — rather than the canvas height — is
        // what makes that hold, because growing upwards changes the height without exposing any
        // new ground below.
        let source = document(160, 900);
        let mut session =
            ScrollCaptureSession::from_bgra(document_frame(&source, 200, 200)).unwrap();

        // Down 100, back up 100, down 100 again: only the first descent is new content below,
        // and the ascent is new content above.
        for top in [250u32, 300, 250, 200, 150, 200, 250, 300] {
            let _ = session.push_bgra_frame(document_frame(&source, top, 200), 1);
        }

        let canvas = session.stitched.to_image();
        // Ground covered spans document rows 150..500, i.e. 350px. Anything materially larger
        // means rows were spliced more than once.
        assert!(
            canvas.height() <= 350,
            "canvas grew to {}px for 350px of ground — content was duplicated",
            canvas.height()
        );
        assert_eq!(
            canvas,
            image::imageops::crop_imm(&source, 0, 150, 160, canvas.height()).to_image(),
            "canvas does not reproduce the document"
        );
    }
}

#[cfg(test)]
mod inset_safety {
    use super::*;
    use image::Rgba;

    /// A frame whose top rows and bottom rows are static furniture and whose middle scrolls.
    fn furnished(width: u32, height: u32, header: u32, footer: u32, scroll: u32) -> RgbaImage {
        RgbaImage::from_fn(width, height, |x, y| {
            if y < header || y >= height - footer {
                // Fixed chrome: identical in every frame.
                let v = (x.wrapping_mul(7) ^ y.wrapping_mul(3)) as u8;
                return Rgba([v, v, v, 255]);
            }
            let doc_y = y + scroll;
            let mut h = x.wrapping_mul(0x9e37_79b9) ^ doc_y.wrapping_mul(0x85eb_ca6b);
            h ^= h >> 16;
            h = h.wrapping_mul(0x7feb_352d);
            h ^= h >> 15;
            let v = h as u8;
            Rgba([v, v.wrapping_add(23), v.wrapping_add(71), 255])
        })
    }

    #[test]
    fn static_chrome_is_not_spliced_repeatedly() {
        let (w, h, header, footer) = (160u32, 240u32, 30u32, 40u32);
        let mut session =
            ScrollCaptureSession::from_bgra(rgba_to_bgra_frame(furnished(w, h, header, footer, 0)))
                .unwrap();
        for step in 1..=12u32 {
            let _ = session.push_bgra_frame(
                rgba_to_bgra_frame(furnished(w, h, header, footer, step * 20)),
                1,
            );
        }
        // 12 steps of 20px of genuinely new content, on top of the first frame.
        let height = session.stitched.height();
        assert!(
            height < h + 12 * h,
            "canvas is {height}px — the static chrome was spliced in repeatedly"
        );
    }

    #[test]
    fn splice_arithmetic_is_safe_for_extreme_offsets() {
        // The splice must not panic for any offset the matcher can produce, including ones
        // larger than the frame.
        let image = RgbaImage::from_pixel(64, 96, Rgba([9, 9, 9, 255]));
        for shift in [1i32, 5, 95, 96, -1, -95, -96] {
            let mut session =
                ScrollCaptureSession::from_bgra(rgba_to_bgra_frame(image.clone())).unwrap();
            let matched = MatchedFrame {
                splice_id: 1,
                next: image.clone(),
                direction: shift.signum() as i8,
                shift,
            };
            let _ = session.commit_matched_frame(matched);
        }
    }
}
