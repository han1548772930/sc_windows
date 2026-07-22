use std::io::Cursor;

use image::{DynamicImage, ImageFormat, RgbaImage};

const MATCH_THRESHOLD: f32 = 18.0;
const NORMAL_MIN_OVERLAP_DIVISOR: u32 = 2;
const FAST_SCROLL_MIN_OVERLAP_DIVISOR: u32 = 8;
pub const MAX_STITCH_HEIGHT: u32 = 30_000;

fn capped_growth(current_height: u32, requested_height: u32) -> u32 {
    requested_height.min(MAX_STITCH_HEIGHT.saturating_sub(current_height))
}

pub struct ScrollCaptureSession {
    selection: sc_app::selection::RectI32,
    stitched: RgbaImage,
    previous: RgbaImage,
    current_offset: i64,
    min_offset: i64,
    max_bottom: i64,
    gesture_checkpoint: Option<GestureCheckpoint>,
}

#[derive(Clone, Copy)]
struct GestureCheckpoint {
    current_offset: i64,
    min_offset: i64,
    max_bottom: i64,
}

pub struct PushOutcome {
    pub changed: bool,
    pub finished: bool,
}

impl ScrollCaptureSession {
    pub fn new(selection: sc_app::selection::RectI32, bmp: &[u8]) -> Result<Self, String> {
        let first = image::load_from_memory_with_format(bmp, ImageFormat::Bmp)
            .map_err(|e| format!("无法读取首帧: {e}"))?
            .to_rgba8();
        let initial_height = first.height() as i64;
        Ok(Self {
            selection,
            stitched: first.clone(),
            previous: first,
            current_offset: 0,
            min_offset: 0,
            max_bottom: initial_height,
            gesture_checkpoint: None,
        })
    }

    pub fn selection(&self) -> sc_app::selection::RectI32 {
        self.selection
    }

    pub fn begin_gesture(&mut self) {
        self.gesture_checkpoint = Some(GestureCheckpoint {
            current_offset: self.current_offset,
            min_offset: self.min_offset,
            max_bottom: self.max_bottom,
        });
    }

    pub fn finish_gesture(&mut self) -> bool {
        let Some(checkpoint) = self.gesture_checkpoint.take() else {
            return false;
        };
        if (self.current_offset - checkpoint.current_offset).abs() > 2 {
            return false;
        }

        let remove_top = (checkpoint.min_offset - self.min_offset).max(0) as u32;
        let remove_bottom = (self.max_bottom - checkpoint.max_bottom).max(0) as u32;
        if remove_top == 0 && remove_bottom == 0 {
            return false;
        }
        let retained_height = self
            .stitched
            .height()
            .saturating_sub(remove_top + remove_bottom);
        self.stitched = image::imageops::crop_imm(
            &self.stitched,
            0,
            remove_top,
            self.stitched.width(),
            retained_height,
        )
        .to_image();
        self.current_offset = checkpoint.current_offset;
        self.min_offset = checkpoint.min_offset;
        self.max_bottom = checkpoint.max_bottom;
        true
    }

    /// Reports whether the stitched image changed and whether its height limit was reached.
    pub fn push_frame(&mut self, bmp: &[u8], direction: i8) -> Result<PushOutcome, String> {
        if self.stitched.height() >= MAX_STITCH_HEIGHT {
            return Ok(PushOutcome {
                changed: false,
                finished: true,
            });
        }
        let next = image::load_from_memory_with_format(bmp, ImageFormat::Bmp)
            .map_err(|e| format!("无法读取滚动帧: {e}"))?
            .to_rgba8();
        if next.dimensions() != self.previous.dimensions() {
            return Err("滚动过程中选区尺寸发生变化".to_string());
        }

        let Some((shift, score)) = find_vertical_shift(&self.previous, &next, direction) else {
            return Ok(PushOutcome {
                changed: false,
                finished: false,
            });
        };
        if score > MATCH_THRESHOLD {
            return Ok(PushOutcome {
                changed: false,
                finished: false,
            });
        }

        self.current_offset += shift as i64;
        self.previous = next.clone();
        if shift == 0 {
            return Ok(PushOutcome {
                changed: false,
                finished: false,
            });
        }

        let height = next.height();
        let new_top = self.current_offset;
        let new_bottom = new_top + height as i64;
        if new_top < self.min_offset {
            let requested_height = (self.min_offset - new_top).min(height as i64) as u32;
            let added_height = capped_growth(self.stitched.height(), requested_height);
            if added_height == 0 {
                return Ok(PushOutcome {
                    changed: false,
                    finished: self.stitched.height() >= MAX_STITCH_HEIGHT,
                });
            }
            let mut combined =
                RgbaImage::new(self.stitched.width(), self.stitched.height() + added_height);
            let head = image::imageops::crop_imm(&next, 0, 0, next.width(), added_height);
            image::imageops::replace(&mut combined, &head.to_image(), 0, 0);
            image::imageops::replace(&mut combined, &self.stitched, 0, added_height as i64);
            self.stitched = combined;
            self.min_offset -= added_height as i64;
            return Ok(PushOutcome {
                changed: true,
                finished: self.stitched.height() >= MAX_STITCH_HEIGHT,
            });
        }

        let requested_height = (new_bottom - self.max_bottom).clamp(0, height as i64) as u32;
        let added_height = capped_growth(self.stitched.height(), requested_height);
        if added_height == 0 {
            return Ok(PushOutcome {
                changed: false,
                finished: self.stitched.height() >= MAX_STITCH_HEIGHT,
            });
        }
        let tail =
            image::imageops::crop_imm(&next, 0, height - added_height, next.width(), added_height);
        let width = self.stitched.width();
        let old_height = self.stitched.height();
        let mut pixels = std::mem::take(&mut self.stitched).into_raw();
        pixels.extend_from_slice(tail.to_image().as_raw());
        self.stitched = RgbaImage::from_raw(width, old_height + added_height, pixels)
            .ok_or_else(|| "无法扩展滚动截图像素缓冲区".to_string())?;
        self.max_bottom += added_height as i64;
        Ok(PushOutcome {
            changed: true,
            finished: self.stitched.height() >= MAX_STITCH_HEIGHT,
        })
    }

    pub fn bmp_data(&self) -> Result<Vec<u8>, String> {
        let mut output = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(self.stitched.clone())
            .write_to(&mut output, ImageFormat::Bmp)
            .map_err(|e| format!("无法编码滚动截图: {e}"))?;
        Ok(output.into_inner())
    }

    pub fn preview_bmp_data(&self, max_width: u32, max_height: u32) -> Result<Vec<u8>, String> {
        let width_scale = max_width as f64 / self.stitched.width().max(1) as f64;
        let height_scale = max_height as f64 / self.stitched.height().max(1) as f64;
        let scale = width_scale.min(height_scale).min(1.0);
        let width = (self.stitched.width() as f64 * scale).round().max(1.0) as u32;
        let height = (self.stitched.height() as f64 * scale).round().max(1.0) as u32;
        let preview = image::imageops::resize(
            &self.stitched,
            width,
            height,
            image::imageops::FilterType::Lanczos3,
        );
        let mut output = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(preview)
            .write_to(&mut output, ImageFormat::Bmp)
            .map_err(|e| format!("无法编码滚动截图预览: {e}"))?;
        Ok(output.into_inner())
    }
}

fn find_vertical_shift(
    previous: &RgbaImage,
    next: &RgbaImage,
    direction: i8,
) -> Option<(i32, f32)> {
    if previous.dimensions() == next.dimensions() && previous.as_raw() == next.as_raw() {
        return Some((0, 0.0));
    }
    find_vertical_shift_with_overlap(
        previous,
        next,
        direction,
        NORMAL_MIN_OVERLAP_DIVISOR,
        true,
        MATCH_THRESHOLD,
    )
    .or_else(|| {
        // A fast gesture can move more than half a viewport between samples. Recover only when
        // multiple horizontal regions independently agree on the same displacement. The narrow
        // single-band fallback is deliberately disabled here because repeated chat rows make it
        // unsafe for large jumps.
        find_vertical_shift_with_overlap(
            previous,
            next,
            direction,
            FAST_SCROLL_MIN_OVERLAP_DIVISOR,
            false,
            6.0,
        )
    })
}

fn find_vertical_shift_with_overlap(
    previous: &RgbaImage,
    next: &RgbaImage,
    direction: i8,
    min_overlap_divisor: u32,
    allow_single_fallback: bool,
    max_score: f32,
) -> Option<(i32, f32)> {
    let height = previous.height();
    let width = previous.width();
    if height < 8 || width == 0 {
        return None;
    }

    let x_margin = (width / 20).max(1);
    let usable_width = width.saturating_sub(x_margin * 2);
    let band_count = (usable_width / 4).clamp(1, 5);
    let mut matches = Vec::new();
    for band in 0..band_count {
        let x_start = x_margin + band * usable_width / band_count;
        let x_end = x_margin + (band + 1) * usable_width / band_count;
        if let Some(result) = best_shift_for_band(
            previous,
            next,
            x_start,
            x_end,
            direction,
            min_overlap_divisor,
        ) {
            matches.push(result);
        }
    }
    matches.retain(|(_, score)| *score <= max_score);
    if matches.is_empty() {
        return None;
    }
    matches.sort_by_key(|(shift, _)| *shift);
    let strongest = matches.iter().copied().min_by(|a, b| a.1.total_cmp(&b.1));
    let median_shift = matches[matches.len() / 2].0;
    let mut consensus: Vec<_> = matches
        .into_iter()
        .filter(|(shift, score)| (shift - median_shift).abs() <= 4 && *score <= max_score)
        .collect();
    let required = if band_count >= 3 { 2 } else { 1 };
    if consensus.len() < required {
        return allow_single_fallback
            .then_some(strongest)
            .flatten()
            .filter(|(shift, score)| shift.unsigned_abs() <= height * 3 / 4 && *score <= 10.0);
    }
    consensus.sort_by(|a, b| a.1.total_cmp(&b.1));
    let shift = {
        let mut shifts: Vec<_> = consensus.iter().map(|(shift, _)| *shift).collect();
        shifts.sort_unstable();
        shifts[shifts.len() / 2]
    };
    let score = consensus[consensus.len() / 2].1;
    if shift.unsigned_abs() > height * 3 / 4 && score > 8.0 {
        None
    } else {
        Some((shift, score))
    }
}

fn best_shift_for_band(
    previous: &RgbaImage,
    next: &RgbaImage,
    x_start: u32,
    x_end: u32,
    direction: i8,
    min_overlap_divisor: u32,
) -> Option<(i32, f32)> {
    let height = previous.height();
    let min_overlap = (height / min_overlap_divisor).max(4);
    let max_shift = height.saturating_sub(min_overlap).max(1);
    let top_exclusion = (height / 10).max(2);
    let bottom_exclusion = (height / 20).max(1);
    let band_width = x_end.saturating_sub(x_start).max(1);
    let mut candidates = Vec::new();
    let max_shift = max_shift as i32;
    let content_start = top_exclusion as i32;
    let content_end = (height - bottom_exclusion) as i32;
    for shift in -max_shift..=max_shift {
        // A wheel event can arrive before the target window has painted its scroll animation.
        // Keep zero in the search so an unchanged frame is not forced into a false shift.
        if (direction > 0 && shift < 0) || (direction < 0 && shift > 0) {
            continue;
        }
        let y_start = content_start.max(content_start - shift);
        let y_end = content_end.min(content_end - shift);
        if y_end - y_start <= 2 {
            continue;
        }
        let compare_height = (y_end - y_start) as u32;
        // Chat windows contain large blank areas and repeated bubble shapes. Denser sampling
        // keeps text, avatars and timestamps in the score instead of matching only whitespace.
        let samples_y = compare_height.min(72);
        let samples_x = band_width.min(24);
        let mut error = 0u64;
        let mut count = 0u64;
        for sy in 0..samples_y {
            let new_y = y_start + (sy * compare_height / samples_y) as i32;
            let old_y = new_y + shift;
            for sx in 0..samples_x {
                let x = x_start + sx * band_width / samples_x;
                let a = previous.get_pixel(x, old_y as u32).0;
                let b = next.get_pixel(x, new_y as u32).0;
                let neighbor = previous
                    .get_pixel(x.saturating_sub(1), (old_y - 1).max(0) as u32)
                    .0;
                let texture = a[0].abs_diff(neighbor[0]) as u32
                    + a[1].abs_diff(neighbor[1]) as u32
                    + a[2].abs_diff(neighbor[2]) as u32;
                if texture >= 6 {
                    error += a[0].abs_diff(b[0]) as u64;
                    error += a[1].abs_diff(b[1]) as u64;
                    error += a[2].abs_diff(b[2]) as u64;
                    count += 3;
                }
            }
        }
        let minimum_evidence = (samples_y as u64 * samples_x as u64 * 3 / 50).max(18);
        if count < minimum_evidence {
            continue;
        }
        let score = error as f32 / count as f32;
        candidates.push((shift, score));
    }
    candidates.sort_by(|a, b| a.1.total_cmp(&b.1).then_with(|| a.0.abs().cmp(&b.0.abs())));
    let best = candidates.first().copied()?;
    // Repeated chat rows often produce another distant displacement with almost the same score.
    // Such a result has no unique geometric solution and must not be stitched.
    if candidates
        .iter()
        .skip(1)
        .any(|candidate| (candidate.0 - best.0).abs() > 4 && candidate.1 <= best.1 + 1.5)
    {
        return None;
    }
    Some(best)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    #[test]
    fn growth_is_clamped_at_the_maximum_stitch_height() {
        assert_eq!(capped_growth(MAX_STITCH_HEIGHT - 10, 40), 10);
        assert_eq!(capped_growth(MAX_STITCH_HEIGHT, 40), 0);
        assert_eq!(capped_growth(MAX_STITCH_HEIGHT - 10, 5), 5);
    }

    #[test]
    fn finds_known_vertical_overlap() {
        let document = RgbaImage::from_fn(12, 30, |x, y| {
            Rgba([(y * 7) as u8, (x * 13) as u8, (x + y) as u8, 255])
        });
        let first = image::imageops::crop_imm(&document, 0, 0, 12, 20).to_image();
        let second = image::imageops::crop_imm(&document, 0, 10, 12, 20).to_image();
        let (shift, score) = find_vertical_shift(&first, &second, 1).unwrap();
        assert_eq!(shift, 10);
        assert_eq!(score, 0.0);
    }

    #[test]
    fn unchanged_repeated_content_resolves_to_zero_shift() {
        let frame = RgbaImage::from_fn(160, 180, |x, y| {
            let row = y % 36;
            if (8..28).contains(&row) && (20..140).contains(&x) {
                Rgba([235, 235, 235, 255])
            } else if row == 12 && x % 11 < 4 {
                Rgba([80, 80, 80, 255])
            } else {
                Rgba([255, 255, 255, 255])
            }
        });
        let (shift, score) = find_vertical_shift(&frame, &frame, 1).unwrap();
        assert_eq!(shift, 0);
        assert_eq!(score, 0.0);
    }

    #[test]
    fn never_accepts_a_shift_without_enough_overlap() {
        let document = RgbaImage::from_fn(40, 260, |x, y| {
            Rgba([(y * 7) as u8, (x * 13) as u8, (x + y * 3) as u8, 255])
        });
        let first = image::imageops::crop_imm(&document, 0, 0, 40, 100).to_image();
        let second = image::imageops::crop_imm(&document, 0, 80, 40, 100).to_image();
        if let Some((shift, _)) = find_vertical_shift(&first, &second, 1) {
            assert!(shift <= 87);
        }
    }

    #[test]
    fn recovers_a_large_shift_when_multiple_bands_agree() {
        let document = RgbaImage::from_fn(160, 280, |x, y| {
            let value = x
                .wrapping_mul(37)
                .wrapping_add(y.wrapping_mul(71))
                .wrapping_add(x.wrapping_mul(y).wrapping_mul(3));
            Rgba([value as u8, (value >> 3) as u8, (value >> 7) as u8, 255])
        });
        let first = image::imageops::crop_imm(&document, 0, 0, 160, 100).to_image();
        let second = image::imageops::crop_imm(&document, 0, 78, 160, 100).to_image();
        let (shift, score) = find_vertical_shift(&first, &second, 1).unwrap();
        assert_eq!(shift, 78);
        assert_eq!(score, 0.0);
    }

    #[test]
    fn rejects_a_large_shift_supported_by_only_one_narrow_band() {
        let document = RgbaImage::from_fn(240, 280, |x, y| {
            if (104..136).contains(&x) {
                let value = x.wrapping_mul(37).wrapping_add(y.wrapping_mul(71));
                Rgba([value as u8, (value >> 3) as u8, (value >> 5) as u8, 255])
            } else {
                Rgba([252, 252, 252, 255])
            }
        });
        let first = image::imageops::crop_imm(&document, 0, 0, 240, 100).to_image();
        let second = image::imageops::crop_imm(&document, 0, 78, 240, 100).to_image();
        assert!(find_vertical_shift(&first, &second, 1).is_none());
    }

    #[test]
    fn matches_a_narrow_content_column_on_a_white_page() {
        let document = RgbaImage::from_fn(200, 300, |x, y| {
            if (70..130).contains(&x) && y % 17 < 5 {
                Rgba([(y * 3) as u8, (x * 5) as u8, (x + y) as u8, 255])
            } else {
                Rgba([255, 255, 255, 255])
            }
        });
        let first = image::imageops::crop_imm(&document, 0, 0, 200, 200).to_image();
        let second = image::imageops::crop_imm(&document, 0, 40, 200, 200).to_image();
        let (shift, score) = find_vertical_shift(&first, &second, 1).unwrap();
        assert_eq!(shift, 40);
        assert!(score <= MATCH_THRESHOLD);
    }

    #[test]
    fn accepts_one_strong_narrow_band_when_other_bands_are_blank() {
        let document = RgbaImage::from_fn(240, 320, |x, y| {
            if (108..124).contains(&x) && y % 19 < 7 {
                Rgba([(y * 5) as u8, (x * 3) as u8, (x + y) as u8, 255])
            } else {
                Rgba([252, 252, 252, 255])
            }
        });
        let first = image::imageops::crop_imm(&document, 0, 0, 240, 220).to_image();
        let second = image::imageops::crop_imm(&document, 0, 35, 240, 220).to_image();
        let (shift, score) = find_vertical_shift(&first, &second, 1).unwrap();
        assert_eq!(shift, 35);
        assert!(score <= 10.0);
    }

    #[test]
    fn scrolling_back_through_captured_content_does_not_append_it_again() {
        fn bmp(image: RgbaImage) -> Vec<u8> {
            let mut output = Cursor::new(Vec::new());
            DynamicImage::ImageRgba8(image)
                .write_to(&mut output, ImageFormat::Bmp)
                .unwrap();
            output.into_inner()
        }

        let document = RgbaImage::from_fn(120, 260, |x, y| {
            Rgba([(y * 3) as u8, (x * 5) as u8, (x + y * 2) as u8, 255])
        });
        let frame = |y| image::imageops::crop_imm(&document, 0, y, 120, 100).to_image();
        let selection = sc_app::selection::RectI32 {
            left: 0,
            top: 0,
            right: 120,
            bottom: 100,
        };
        let mut session = ScrollCaptureSession::new(selection, &bmp(frame(0))).unwrap();
        assert!(session.push_frame(&bmp(frame(40)), 1).unwrap().changed);
        assert_eq!(session.stitched.height(), 140);
        assert!(!session.push_frame(&bmp(frame(20)), -1).unwrap().changed);
        assert!(!session.push_frame(&bmp(frame(40)), 1).unwrap().changed);
        assert_eq!(session.stitched.height(), 140);
        assert!(session.push_frame(&bmp(frame(60)), 1).unwrap().changed);
        assert_eq!(session.stitched.height(), 160);
    }

    #[test]
    fn bottom_bounce_rolls_back_temporary_growth() {
        fn bmp(image: RgbaImage) -> Vec<u8> {
            let mut output = Cursor::new(Vec::new());
            DynamicImage::ImageRgba8(image)
                .write_to(&mut output, ImageFormat::Bmp)
                .unwrap();
            output.into_inner()
        }

        let document = RgbaImage::from_fn(120, 220, |x, y| {
            Rgba([(y * 3) as u8, (x * 5) as u8, (x + y * 2) as u8, 255])
        });
        let frame = |y| image::imageops::crop_imm(&document, 0, y, 120, 100).to_image();
        let selection = sc_app::selection::RectI32 {
            left: 0,
            top: 0,
            right: 120,
            bottom: 100,
        };
        let mut session = ScrollCaptureSession::new(selection, &bmp(frame(0))).unwrap();
        session.begin_gesture();
        assert!(session.push_frame(&bmp(frame(30)), 1).unwrap().changed);
        assert!(!session.push_frame(&bmp(frame(0)), 0).unwrap().changed);
        assert!(session.finish_gesture());
        assert_eq!(session.stitched, frame(0));
    }

    #[test]
    fn previously_unseen_upward_content_is_prepended() {
        fn bmp(image: RgbaImage) -> Vec<u8> {
            let mut output = Cursor::new(Vec::new());
            DynamicImage::ImageRgba8(image)
                .write_to(&mut output, ImageFormat::Bmp)
                .unwrap();
            output.into_inner()
        }
        let document = RgbaImage::from_fn(100, 240, |x, y| {
            Rgba([(y * 3) as u8, (x * 7) as u8, (x + y) as u8, 255])
        });
        let frame = |y| image::imageops::crop_imm(&document, 0, y, 100, 100).to_image();
        let selection = sc_app::selection::RectI32 {
            left: 0,
            top: 0,
            right: 100,
            bottom: 100,
        };
        let mut session = ScrollCaptureSession::new(selection, &bmp(frame(40))).unwrap();
        assert!(session.push_frame(&bmp(frame(20)), -1).unwrap().changed);
        assert_eq!(session.stitched.height(), 120);
        assert_eq!(session.min_offset, -20);
        let expected = image::imageops::crop_imm(&document, 0, 20, 100, 120).to_image();
        assert_eq!(session.stitched, expected);
    }

    #[test]
    fn capture_does_not_stop_after_thirty_frames() {
        fn bmp(image: RgbaImage) -> Vec<u8> {
            let mut output = Cursor::new(Vec::new());
            DynamicImage::ImageRgba8(image)
                .write_to(&mut output, ImageFormat::Bmp)
                .unwrap();
            output.into_inner()
        }
        let document = RgbaImage::from_fn(80, 500, |x, y| {
            Rgba([(y * 3) as u8, (x * 7) as u8, (x + y * 2) as u8, 255])
        });
        let frame = |y| image::imageops::crop_imm(&document, 0, y, 80, 100).to_image();
        let selection = sc_app::selection::RectI32 {
            left: 0,
            top: 0,
            right: 80,
            bottom: 100,
        };
        let mut session = ScrollCaptureSession::new(selection, &bmp(frame(0))).unwrap();
        for index in 1..=40 {
            let outcome = session.push_frame(&bmp(frame(index * 5)), 1).unwrap();
            assert!(!outcome.finished);
        }
        assert_eq!(session.stitched.height(), 300);
    }
}
