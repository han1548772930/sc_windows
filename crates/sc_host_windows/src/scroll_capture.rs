use std::io::Cursor;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};

use image::{DynamicImage, ImageFormat, RgbaImage};
use imageproc::template_matching::{MatchTemplateMethod, match_template_parallel};

const MATCH_THRESHOLD: f32 = 18.0;
const NORMAL_MIN_OVERLAP_DIVISOR: u32 = 2;
const FAST_SCROLL_MIN_OVERLAP_DIVISOR: u32 = 8;
const MAX_TRANSIENT_MATCH_FAILURES: u8 = 8;
pub const MAX_STITCH_HEIGHT: u32 = 30_000;

fn capped_growth(current_height: u32, requested_height: u32) -> u32 {
    requested_height.min(MAX_STITCH_HEIGHT.saturating_sub(current_height))
}

pub struct ScrollCaptureSession {
    stitched: RgbaImage,
    previous: RgbaImage,
    current_offset: i64,
    min_offset: i64,
    max_bottom: i64,
    last_shift: Option<i32>,
    last_direction: i8,
    gesture_checkpoint: Option<GestureCheckpoint>,
}

#[derive(Clone, Copy)]
struct GestureCheckpoint {
    current_offset: i64,
    min_offset: i64,
    max_bottom: i64,
}

#[derive(Debug)]
pub struct PushOutcome {
    pub changed: bool,
    pub finished: bool,
}

impl ScrollCaptureSession {
    pub fn new(_selection: sc_app::selection::RectI32, bmp: &[u8]) -> Result<Self, String> {
        let first = image::load_from_memory_with_format(bmp, ImageFormat::Bmp)
            .map_err(|e| format!("无法读取首帧: {e}"))?
            .to_rgba8();
        let initial_height = first.height() as i64;
        Ok(Self {
            stitched: first.clone(),
            previous: first,
            current_offset: 0,
            min_offset: 0,
            max_bottom: initial_height,
            last_shift: None,
            last_direction: 0,
            gesture_checkpoint: None,
        })
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

        if direction != 0 && self.last_direction != 0 && direction != self.last_direction {
            self.last_shift = self.last_shift.map(|shift| -shift);
        }
        if direction != 0 {
            self.last_direction = direction;
        }

        let directed_match =
            find_vertical_shift_with_hint(&self.previous, &next, direction, self.last_shift);
        let bounce_match = || {
            (direction != 0)
                .then(|| find_vertical_shift_with_hint(&self.previous, &next, 0, None))
                .flatten()
                .filter(|(shift, score)| {
                    let opposite = (direction > 0 && *shift < 0) || (direction < 0 && *shift > 0);
                    opposite && *score <= 6.0
                })
        };
        let Some((shift, score)) = directed_match.or_else(bounce_match) else {
            let direction_label = match direction {
                -1 => "滚轮向上",
                1 => "滚轮向下",
                _ => "方向未知",
            };
            return Err(format!(
                "重叠验证未通过：没有候选位移同时满足方向、最小重叠、唯一性和像素误差要求。方向={direction_label}，最后成功位移={:?}px，选区={}x{}。当前帧未写入长图",
                self.last_shift,
                self.previous.width(),
                self.previous.height()
            ));
        };
        if score > MATCH_THRESHOLD {
            return Err(format!(
                "像素误差验证未通过：候选位移={shift}px，实测误差={score:.2}，允许上限={MATCH_THRESHOLD:.2}。当前帧未写入长图"
            ));
        }

        self.current_offset += shift as i64;
        self.previous = next.clone();
        if shift == 0 {
            return Ok(PushOutcome {
                changed: false,
                finished: false,
            });
        }
        let is_boundary_bounce = (direction > 0 && shift < 0) || (direction < 0 && shift > 0);
        if !is_boundary_bounce {
            self.last_shift = Some(shift);
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
            image::imageops::FilterType::Triangle,
        );
        let mut output = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(preview)
            .write_to(&mut output, ImageFormat::Bmp)
            .map_err(|e| format!("无法编码滚动截图预览: {e}"))?;
        Ok(output.into_inner())
    }
}

enum WorkerCommand {
    BeginGesture,
    Frame {
        bmp: Vec<u8>,
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

pub enum ScrollCaptureEvent {
    Preview(Vec<u8>),
    Finished,
    Error(String),
}

pub struct ScrollCaptureWorker {
    selection: sc_app::selection::RectI32,
    commands: Sender<WorkerCommand>,
    events: Receiver<ScrollCaptureEvent>,
    thread: Option<std::thread::JoinHandle<()>>,
    pending_frames: Arc<AtomicUsize>,
}

impl ScrollCaptureWorker {
    pub fn new(selection: sc_app::selection::RectI32, bmp: &[u8]) -> Result<Self, String> {
        let session = ScrollCaptureSession::new(selection, bmp)?;
        let (command_tx, command_rx) = mpsc::channel();
        let (event_tx, event_rx) = mpsc::channel();
        let pending_frames = Arc::new(AtomicUsize::new(0));
        let worker_pending_frames = pending_frames.clone();
        let thread = std::thread::Builder::new()
            .name("scroll-capture-worker".to_string())
            .spawn(move || {
                run_scroll_capture_worker(session, command_rx, event_tx, worker_pending_frames)
            })
            .map_err(|error| format!("无法启动滚动截图后台线程: {error}"))?;
        Ok(Self {
            selection,
            commands: command_tx,
            events: event_rx,
            thread: Some(thread),
            pending_frames,
        })
    }

    pub fn selection(&self) -> sc_app::selection::RectI32 {
        self.selection
    }

    pub fn begin_gesture(&self) -> Result<(), String> {
        self.send(WorkerCommand::BeginGesture)
    }

    pub fn push_frame(
        &self,
        bmp: Vec<u8>,
        direction: i8,
        preview_size: Option<(u32, u32)>,
    ) -> Result<(), String> {
        self.pending_frames.fetch_add(1, Ordering::AcqRel);
        if let Err(error) = self.send(WorkerCommand::Frame {
            bmp,
            direction,
            preview_size,
            queued_at: Instant::now(),
        }) {
            self.pending_frames.fetch_sub(1, Ordering::AcqRel);
            return Err(error);
        }
        Ok(())
    }

    pub fn pending_frames(&self) -> usize {
        self.pending_frames.load(Ordering::Acquire)
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
            .map_err(|_| "滚动截图后台线程已停止".to_string())?
    }

    fn send(&self, command: WorkerCommand) -> Result<(), String> {
        self.commands
            .send(command)
            .map_err(|_| "滚动截图后台线程已停止".to_string())
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
    let mut preview_dirty = false;
    let mut last_lag_warning = None;
    let mut terminal_error: Option<String> = None;
    let mut pending_match_error: Option<String> = None;
    let mut consecutive_match_failures = 0u8;
    while let Ok(command) = commands.recv() {
        if let Some(error) = terminal_error.as_ref() {
            match command {
                WorkerCommand::Export(response) => {
                    let _ = response.send(Err(error.clone()));
                }
                WorkerCommand::Frame { .. } => {
                    pending_frames.fetch_sub(1, Ordering::AcqRel);
                }
                WorkerCommand::Stop => break,
                _ => {}
            }
            continue;
        }
        match command {
            WorkerCommand::BeginGesture => session.begin_gesture(),
            WorkerCommand::Frame {
                bmp,
                direction,
                preview_size,
                queued_at,
            } => {
                let queue_delay = queued_at.elapsed();
                if queue_delay >= Duration::from_millis(200)
                    && last_lag_warning
                        .is_none_or(|last: Instant| last.elapsed() >= Duration::from_secs(1))
                {
                    eprintln!(
                        "[滚动截图] 后台队列延迟过高: {}ms，右侧预览可能暂时落后",
                        queue_delay.as_millis()
                    );
                    last_lag_warning = Some(Instant::now());
                }
                match session.push_frame(&bmp, direction) {
                    Ok(outcome) => {
                        if consecutive_match_failures > 0 {
                            eprintln!(
                                "[滚动截图] 已从运动中间帧恢复: 连续失败={}，后续帧已通过严格匹配",
                                consecutive_match_failures
                            );
                            consecutive_match_failures = 0;
                            pending_match_error = None;
                        }
                        preview_dirty |= outcome.changed;
                        if outcome.finished {
                            let _ = events.send(ScrollCaptureEvent::Finished);
                        }
                        if preview_dirty {
                            if let Some((width, height)) = preview_size {
                                match session.preview_bmp_data(width, height) {
                                    Ok(preview) => {
                                        preview_dirty = false;
                                        let _ = events.send(ScrollCaptureEvent::Preview(preview));
                                    }
                                    Err(error) => {
                                        let _ = events.send(ScrollCaptureEvent::Error(error));
                                    }
                                }
                            }
                        }
                    }
                    Err(error) => {
                        consecutive_match_failures = consecutive_match_failures.saturating_add(1);
                        pending_match_error = Some(error.clone());
                        if consecutive_match_failures == 1 {
                            eprintln!(
                                "[滚动截图] 帧验证未通过，继续恢复 ({consecutive_match_failures}/{MAX_TRANSIENT_MATCH_FAILURES}): {error}"
                            );
                        }
                        if consecutive_match_failures >= MAX_TRANSIENT_MATCH_FAILURES {
                            let terminal = format!(
                                "处理终止：连续 {consecutive_match_failures} 帧未能从最后正确锚点恢复。最后一次验证详情：{error}"
                            );
                            terminal_error = Some(terminal.clone());
                            let _ = events.send(ScrollCaptureEvent::Error(terminal));
                        }
                    }
                }
                pending_frames.fetch_sub(1, Ordering::AcqRel);
            }
            WorkerCommand::FinishGesture { preview_size } => {
                if let Some(error) = pending_match_error.take() {
                    let terminal = format!(
                        "处理终止：滚动已经停止，但待恢复帧仍未通过验证。最后一次验证详情：{error}"
                    );
                    terminal_error = Some(terminal.clone());
                    let _ = events.send(ScrollCaptureEvent::Error(terminal));
                    continue;
                }
                preview_dirty |= session.finish_gesture();
                if preview_dirty {
                    match session.preview_bmp_data(preview_size.0, preview_size.1) {
                        Ok(preview) => {
                            preview_dirty = false;
                            let _ = events.send(ScrollCaptureEvent::Preview(preview));
                        }
                        Err(error) => {
                            let _ = events.send(ScrollCaptureEvent::Error(error));
                        }
                    }
                }
            }
            WorkerCommand::Export(response) => {
                let result = pending_match_error
                    .clone()
                    .map_or_else(|| session.bmp_data(), Err);
                let _ = response.send(result);
            }
            WorkerCommand::Stop => break,
        }
    }
}

#[cfg(test)]
fn find_vertical_shift(
    previous: &RgbaImage,
    next: &RgbaImage,
    direction: i8,
) -> Option<(i32, f32)> {
    find_vertical_shift_with_hint(previous, next, direction, None)
}

fn find_vertical_shift_with_hint(
    previous: &RgbaImage,
    next: &RgbaImage,
    direction: i8,
    expected_shift: Option<i32>,
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
        expected_shift,
    )
    .or_else(|| {
        // Narrow chat columns can spread too little texture across each independent band even
        // though the complete content area has enough evidence. Recheck the full usable width
        // with the normal overlap bound and a strict score before considering a large jump.
        let width = previous.width();
        let x_margin = (width / 20).max(1);
        best_shift_for_band(
            previous,
            next,
            x_margin,
            width.saturating_sub(x_margin),
            direction,
            NORMAL_MIN_OVERLAP_DIVISOR,
            expected_shift,
        )
        .filter(|(shift, score)| {
            shift.unsigned_abs() <= previous.height() * 3 / 4 && *score <= 10.0
        })
    })
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
            expected_shift,
        )
    })
    .or_else(|| find_vertical_shift_by_template(previous, next, direction, expected_shift))
}

/// Strict fallback for text-heavy windows. `imageproc` supplies the motion candidates, but a
/// candidate is accepted only when independent vertical anchors agree, the peak is unique, and
/// the original RGB overlap also matches. This avoids treating repeated chat rows as movement.
fn find_vertical_shift_by_template(
    previous: &RgbaImage,
    next: &RgbaImage,
    direction: i8,
    expected_shift: Option<i32>,
) -> Option<(i32, f32)> {
    let (width, height) = previous.dimensions();
    if previous.dimensions() != next.dimensions() || width < 16 || height < 40 {
        return None;
    }

    // Compress only horizontally. Keeping the original vertical resolution preserves an exact
    // pixel displacement while making parallel template matching cheap enough for the worker.
    let match_width = width.min(96).max(8);
    let previous_gray = image::imageops::resize(
        &DynamicImage::ImageRgba8(previous.clone()).to_luma8(),
        match_width,
        height,
        image::imageops::FilterType::Triangle,
    );
    let next_gray = image::imageops::resize(
        &DynamicImage::ImageRgba8(next.clone()).to_luma8(),
        match_width,
        height,
        image::imageops::FilterType::Triangle,
    );
    let template_height = (height / 5).clamp(16, height - 1);
    let template_width = match_width - 2;
    let max_shift = (height * 7 / 8) as i32;
    let anchors = [height / 8, height * 2 / 5, height * 3 / 4];
    let mut anchor_matches = Vec::new();

    for anchor in anchors {
        let anchor = anchor.min(height - template_height);
        let template =
            image::imageops::crop_imm(&previous_gray, 1, anchor, template_width, template_height)
                .to_image();
        let scores = match_template_parallel(
            &next_gray,
            &template,
            MatchTemplateMethod::SumOfSquaredErrorsNormalized,
        );
        let mut candidates = Vec::new();
        for y in 0..scores.height() {
            let shift = anchor as i32 - y as i32;
            if shift.abs() > max_shift
                || (direction > 0 && shift < 0)
                || (direction < 0 && shift > 0)
            {
                continue;
            }
            // x=1 means zero horizontal displacement after taking the one-pixel side margins.
            candidates.push((shift, scores.get_pixel(1, y).0[0]));
        }
        candidates.sort_by(|a, b| {
            a.1.total_cmp(&b.1).then_with(|| {
                expected_shift.map_or(a.0.abs().cmp(&b.0.abs()), |hint| {
                    (a.0 - hint).abs().cmp(&(b.0 - hint).abs())
                })
            })
        });
        let best = candidates.first().copied()?;
        if !best.1.is_finite() || best.1 > 0.012 {
            continue;
        }
        // A repeated row can create another equally good peak. Motion history may order close
        // candidates, but it must never make an ambiguous candidate acceptable.
        let ambiguous = candidates
            .iter()
            .skip(1)
            .any(|candidate| (candidate.0 - best.0).abs() > 3 && candidate.1 <= best.1 + 0.0025);
        if !ambiguous {
            anchor_matches.push(best);
        }
    }

    if anchor_matches.len() < 2 {
        return None;
    }
    anchor_matches.sort_by_key(|candidate| candidate.0);
    let shift = anchor_matches[anchor_matches.len() / 2].0;
    let agreeing: Vec<_> = anchor_matches
        .iter()
        .filter(|candidate| (candidate.0 - shift).abs() <= 2)
        .collect();
    if agreeing.len() < 2 {
        return None;
    }
    let rgb_score = verify_vertical_shift_rgb(previous, next, shift)?;
    (rgb_score <= 10.0).then_some((shift, rgb_score))
}

fn verify_vertical_shift_rgb(previous: &RgbaImage, next: &RgbaImage, shift: i32) -> Option<f32> {
    let (width, height) = previous.dimensions();
    let y_start = 0i32.max(-shift);
    let y_end = (height as i32).min(height as i32 - shift);
    if y_end - y_start < (height / 8) as i32 {
        return None;
    }
    let x_step = (width / 96).max(1) as usize;
    let y_step = ((y_end - y_start) as u32 / 160).max(1) as usize;
    let mut error = 0u64;
    let mut evidence = 0u64;
    for y in (y_start..y_end).step_by(y_step) {
        let old_y = y + shift;
        for x in (1..width).step_by(x_step) {
            let a = previous.get_pixel(x, old_y as u32).0;
            let b = next.get_pixel(x, y as u32).0;
            let neighbor = previous.get_pixel(x - 1, old_y as u32).0;
            let texture = a[0].abs_diff(neighbor[0]) as u32
                + a[1].abs_diff(neighbor[1]) as u32
                + a[2].abs_diff(neighbor[2]) as u32;
            if texture >= 6 {
                error += a[0].abs_diff(b[0]) as u64
                    + a[1].abs_diff(b[1]) as u64
                    + a[2].abs_diff(b[2]) as u64;
                evidence += 3;
            }
        }
    }
    (evidence >= 48).then_some(error as f32 / evidence as f32)
}

fn find_vertical_shift_with_overlap(
    previous: &RgbaImage,
    next: &RgbaImage,
    direction: i8,
    min_overlap_divisor: u32,
    allow_single_fallback: bool,
    max_score: f32,
    expected_shift: Option<i32>,
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
            expected_shift,
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
    let required = if min_overlap_divisor == FAST_SCROLL_MIN_OVERLAP_DIVISOR {
        // With only a narrow vertical overlap, two matching regions are too easy to obtain from
        // repeated rows. Require a majority of the sampled width before accepting a large jump.
        (band_count / 2 + 1) as usize
    } else if band_count >= 3 {
        2
    } else {
        1
    };
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
    expected_shift: Option<i32>,
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
    let mut best = candidates.first().copied()?;
    // Repeated chat rows often produce another distant displacement with almost the same score.
    // Such a result has no unique geometric solution and must not be stitched.
    if candidates
        .iter()
        .skip(1)
        .any(|candidate| (candidate.0 - best.0).abs() > 4 && candidate.1 <= best.1 + 1.5)
    {
        let expected = expected_shift.filter(|expected| {
            *expected != 0
                && (direction == 0
                    || (direction > 0 && *expected > 0)
                    || (direction < 0 && *expected < 0))
        })?;
        best = candidates
            .iter()
            .copied()
            .filter(|candidate| candidate.1 <= best.1 + 1.5)
            .min_by_key(|candidate| (candidate.0 - expected).abs())?;
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
    fn template_fallback_finds_a_94_pixel_text_shift() {
        let document = RgbaImage::from_fn(477, 520, |x, y| {
            let row = y % 29;
            if (5..18).contains(&row) && (24..450).contains(&x) {
                let hash = x
                    .wrapping_mul(17)
                    .wrapping_add(y.wrapping_mul(y.wrapping_add(31)))
                    .wrapping_add((x ^ y).wrapping_mul(13));
                let ink = hash % 101 < 19;
                if ink {
                    Rgba([(35 + y % 17) as u8, 38, 42, 255])
                } else {
                    Rgba([244, 246, 248, 255])
                }
            } else {
                Rgba([252, 252, 252, 255])
            }
        });
        let first = image::imageops::crop_imm(&document, 0, 94, 477, 335).to_image();
        let second = image::imageops::crop_imm(&document, 0, 0, 477, 335).to_image();
        let (shift, score) = find_vertical_shift_by_template(&first, &second, -1, Some(-90))
            .expect("the strict template fallback should recover the known displacement");
        assert_eq!(shift, -94);
        assert!(score <= 10.0);
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
    fn unmatched_frame_fails_without_modifying_the_stitch() {
        fn bmp(image: RgbaImage) -> Vec<u8> {
            let mut output = Cursor::new(Vec::new());
            DynamicImage::ImageRgba8(image)
                .write_to(&mut output, ImageFormat::Bmp)
                .unwrap();
            output.into_inner()
        }

        let first = RgbaImage::from_pixel(120, 100, Rgba([220, 20, 20, 255]));
        let unrelated = RgbaImage::from_pixel(120, 100, Rgba([20, 220, 20, 255]));
        let selection = sc_app::selection::RectI32 {
            left: 0,
            top: 0,
            right: 120,
            bottom: 100,
        };
        let mut session = ScrollCaptureSession::new(selection, &bmp(first.clone())).unwrap();
        let error = session.push_frame(&bmp(unrelated), 1).unwrap_err();
        assert!(error.contains("当前帧未写入长图"));
        assert_eq!(session.stitched, first);
        assert_eq!(session.current_offset, 0);
    }

    #[test]
    fn worker_export_cannot_bypass_a_failed_queued_frame() {
        fn bmp(image: RgbaImage) -> Vec<u8> {
            let mut output = Cursor::new(Vec::new());
            DynamicImage::ImageRgba8(image)
                .write_to(&mut output, ImageFormat::Bmp)
                .unwrap();
            output.into_inner()
        }

        let first = RgbaImage::from_pixel(120, 100, Rgba([220, 20, 20, 255]));
        let unrelated = RgbaImage::from_pixel(120, 100, Rgba([20, 220, 20, 255]));
        let selection = sc_app::selection::RectI32 {
            left: 0,
            top: 0,
            right: 120,
            bottom: 100,
        };
        let worker = ScrollCaptureWorker::new(selection, &bmp(first)).unwrap();
        worker.push_frame(bmp(unrelated), 1, None).unwrap();
        let error = worker.bmp_data().unwrap_err();
        assert!(error.contains("当前帧未写入长图"));
    }

    #[test]
    fn worker_recovers_from_one_unusable_motion_frame() {
        fn bmp(image: RgbaImage) -> Vec<u8> {
            let mut output = Cursor::new(Vec::new());
            DynamicImage::ImageRgba8(image)
                .write_to(&mut output, ImageFormat::Bmp)
                .unwrap();
            output.into_inner()
        }

        let document = RgbaImage::from_fn(120, 180, |x, y| {
            let value = x.wrapping_mul(31).wrapping_add(y.wrapping_mul(67));
            Rgba([value as u8, (value >> 2) as u8, (value >> 5) as u8, 255])
        });
        let first = image::imageops::crop_imm(&document, 0, 0, 120, 100).to_image();
        let recovered = image::imageops::crop_imm(&document, 0, 35, 120, 100).to_image();
        let unusable = RgbaImage::from_pixel(120, 100, Rgba([240, 20, 180, 255]));
        let selection = sc_app::selection::RectI32 {
            left: 0,
            top: 0,
            right: 120,
            bottom: 100,
        };
        let worker = ScrollCaptureWorker::new(selection, &bmp(first)).unwrap();
        worker.push_frame(bmp(unusable), 1, None).unwrap();
        worker.push_frame(bmp(recovered), 1, None).unwrap();
        assert!(worker.bmp_data().is_ok());
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
    fn prior_motion_resolves_repeated_row_ambiguity() {
        let document = RgbaImage::from_fn(160, 260, |x, y| {
            let row = y % 20;
            if row < 10 {
                Rgba([(x * 7) as u8, (row * 19) as u8, 80, 255])
            } else {
                Rgba([240, (x * 11) as u8, (row * 13) as u8, 255])
            }
        });
        let first = image::imageops::crop_imm(&document, 0, 0, 160, 100).to_image();
        let second = image::imageops::crop_imm(&document, 0, 10, 160, 100).to_image();
        assert!(find_vertical_shift(&first, &second, 1).is_none());
        let (shift, score) = find_vertical_shift_with_hint(&first, &second, 1, Some(10)).unwrap();
        assert_eq!(shift, 10);
        assert_eq!(score, 0.0);
    }

    #[test]
    fn reversing_direction_flips_the_motion_hint_for_repeated_rows() {
        fn bmp(image: RgbaImage) -> Vec<u8> {
            let mut output = Cursor::new(Vec::new());
            DynamicImage::ImageRgba8(image)
                .write_to(&mut output, ImageFormat::Bmp)
                .unwrap();
            output.into_inner()
        }

        let document = RgbaImage::from_fn(160, 180, |x, y| {
            let row = y % 20;
            if row < 10 {
                Rgba([(x * 7) as u8, (row * 19) as u8, 80, 255])
            } else {
                Rgba([240, (x * 11) as u8, (row * 13) as u8, 255])
            }
        });
        let frame = |y| image::imageops::crop_imm(&document, 0, y, 160, 100).to_image();
        let selection = sc_app::selection::RectI32 {
            left: 0,
            top: 0,
            right: 160,
            bottom: 100,
        };
        let mut session = ScrollCaptureSession::new(selection, &bmp(frame(10))).unwrap();
        session.last_shift = Some(10);
        session.last_direction = 1;

        let outcome = session.push_frame(&bmp(frame(0)), -1).unwrap();
        assert!(outcome.changed);
        assert_eq!(session.last_shift, Some(-10));
        assert_eq!(session.min_offset, -10);
    }

    #[test]
    fn scrolling_back_past_the_start_prepends_new_content() {
        fn bmp(image: RgbaImage) -> Vec<u8> {
            let mut output = Cursor::new(Vec::new());
            DynamicImage::ImageRgba8(image)
                .write_to(&mut output, ImageFormat::Bmp)
                .unwrap();
            output.into_inner()
        }

        let document = RgbaImage::from_fn(120, 260, |x, y| {
            Rgba([(y * 3) as u8, (x * 7) as u8, (x + y * 5) as u8, 255])
        });
        let frame = |y| image::imageops::crop_imm(&document, 0, y, 120, 100).to_image();
        let selection = sc_app::selection::RectI32 {
            left: 0,
            top: 0,
            right: 120,
            bottom: 100,
        };
        let mut session = ScrollCaptureSession::new(selection, &bmp(frame(80))).unwrap();
        assert!(session.push_frame(&bmp(frame(160)), 1).unwrap().changed);
        assert!(!session.push_frame(&bmp(frame(80)), -1).unwrap().changed);
        assert!(session.push_frame(&bmp(frame(40)), -1).unwrap().changed);
        assert!(session.push_frame(&bmp(frame(0)), -1).unwrap().changed);
        assert_eq!(session.stitched, document);
    }

    #[test]
    fn rejects_a_large_shift_without_a_majority_of_horizontal_bands() {
        let document = RgbaImage::from_fn(250, 280, |x, y| {
            if (62..97).contains(&x) || (152..187).contains(&x) {
                let value = x
                    .wrapping_mul(37)
                    .wrapping_add(y.wrapping_mul(71))
                    .wrapping_add(x.wrapping_mul(y));
                Rgba([value as u8, (value >> 3) as u8, (value >> 5) as u8, 255])
            } else {
                Rgba([252, 252, 252, 255])
            }
        });
        let first = image::imageops::crop_imm(&document, 0, 0, 250, 100).to_image();
        let second = image::imageops::crop_imm(&document, 0, 78, 250, 100).to_image();
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
    fn boundary_bounce_is_detected_while_wheel_direction_is_stale() {
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
        assert!(!session.push_frame(&bmp(frame(0)), 1).unwrap().changed);
        assert_eq!(session.last_shift, Some(30));
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
