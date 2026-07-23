use std::collections::VecDeque;
use std::io::Cursor;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};

use image::{DynamicImage, ImageFormat, RgbaImage};
use sc_platform_windows::windows::graphics_capture::BgraFrame;

const MAX_PENDING_FRAMES: usize = 2;

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

pub struct ScrollCaptureSession {
    stitched: TiledImage,
    previous: RgbaImage,
    current_offset: i64,
    min_offset: i64,
    max_bottom: i64,
    last_shift: Option<i32>,
    last_direction: i8,
    preview_cache: Option<PreviewCache>,
}

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

impl ScrollCaptureSession {
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
            preview_cache: None,
        }
    }

    fn push_bgra_frame_gpu(
        &mut self,
        frame: BgraFrame,
        direction: i8,
        shift: i32,
    ) -> Result<PushOutcome, String> {
        let next = bgra_frame_to_rgba(frame)?;
        if next.dimensions() != self.previous.dimensions() {
            return Err("GPU keyframe dimensions changed during scrolling".to_string());
        }
        if direction != 0 && self.last_direction != 0 && direction != self.last_direction {
            self.last_shift = self.last_shift.map(|last| -last);
        }
        if direction != 0 {
            self.last_direction = direction;
        }
        self.commit_image(next, direction, shift)
    }

    fn commit_image(
        &mut self,
        next: RgbaImage,
        direction: i8,
        shift: i32,
    ) -> Result<PushOutcome, String> {
        self.current_offset += shift as i64;
        self.previous = next.clone();
        if shift == 0 {
            eprintln!(
                "[scroll capture] unchanged frame, range={}..{}, height={}",
                self.min_offset,
                self.max_bottom,
                self.stitched.height()
            );
            return Ok(PushOutcome { changed: false });
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
            let added_height = requested_height;
            let head = image::imageops::crop_imm(&next, 0, 0, next.width(), added_height);
            self.stitched.prepend(head.to_image());
            self.preview_cache = None;
            self.min_offset -= added_height as i64;
            eprintln!(
                "[scroll capture] prepend={}px, shift={}px, range={}..{}, height={}",
                added_height,
                shift,
                self.min_offset,
                self.max_bottom,
                self.stitched.height()
            );
            return Ok(PushOutcome { changed: true });
        }

        let requested_height = (new_bottom - self.max_bottom).clamp(0, height as i64) as u32;
        let added_height = requested_height;
        if added_height == 0 {
            return Ok(PushOutcome { changed: false });
        }
        let tail =
            image::imageops::crop_imm(&next, 0, height - added_height, next.width(), added_height);
        self.stitched.append(tail.to_image());
        self.max_bottom += added_height as i64;
        eprintln!(
            "[scroll capture] append={}px, shift={}px, range={}..{}, height={}",
            added_height,
            shift,
            self.min_offset,
            self.max_bottom,
            self.stitched.height()
        );
        Ok(PushOutcome { changed: true })
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
        let scale = width_scale.min(1.0);
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

        let preview = image::imageops::resize(
            &self.stitched.to_image(),
            width,
            height,
            image::imageops::FilterType::Triangle,
        );
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

enum WorkerCommand {
    Frame {
        frame: BgraFrame,
        direction: i8,
        gpu_shift: i32,
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
    Preview(BgraFrame),
    FrameAccepted,
    FrameDiscarded,
    GestureFinished,
}

pub struct ScrollCaptureWorker {
    selection: sc_app::selection::RectI32,
    commands: Sender<WorkerCommand>,
    events: Receiver<ScrollCaptureEvent>,
    thread: Option<std::thread::JoinHandle<()>>,
    pending_frames: Arc<AtomicUsize>,
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
        })
    }

    pub fn selection(&self) -> sc_app::selection::RectI32 {
        self.selection
    }

    pub fn push_gpu_keyframe(
        &self,
        frame: BgraFrame,
        direction: i8,
        shift: i32,
        _stable: bool,
        preview_size: Option<(u32, u32)>,
    ) -> Result<bool, String> {
        self.push_frame_command(frame, direction, shift, preview_size)
    }

    fn push_frame_command(
        &self,
        frame: BgraFrame,
        direction: i8,
        gpu_shift: i32,
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
            frame,
            direction,
            gpu_shift,
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
    let mut preview_dirty = false;
    let mut last_lag_warning = None;
    while let Ok(command) = commands.recv() {
        match command {
            WorkerCommand::Frame {
                frame,
                direction,
                gpu_shift,
                preview_size,
                queued_at,
            } => {
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
                let result = session.push_bgra_frame_gpu(frame, direction, gpu_shift);
                match result {
                    Ok(outcome) => {
                        let _ = events.send(ScrollCaptureEvent::FrameAccepted);
                        preview_dirty |= outcome.changed;
                        if preview_dirty {
                            if let Some((width, height)) = preview_size {
                                let preview = session.preview_frame(width, height);
                                preview_dirty = false;
                                let _ = events.send(ScrollCaptureEvent::Preview(preview));
                            }
                        }
                    }
                    Err(_) => {
                        let _ = events.send(ScrollCaptureEvent::FrameDiscarded);
                    }
                }
                pending_frames.fetch_sub(1, Ordering::AcqRel);
            }
            WorkerCommand::FinishGesture { preview_size } => {
                eprintln!(
                    "[scroll capture] gesture settled, preview_dirty={}, preview={}x{}",
                    preview_dirty, preview_size.0, preview_size.1
                );
                if preview_dirty {
                    let preview = session.preview_frame(preview_size.0, preview_size.1);
                    preview_dirty = false;
                    let _ = events.send(ScrollCaptureEvent::Preview(preview));
                }
                let _ = events.send(ScrollCaptureEvent::GestureFinished);
            }
            WorkerCommand::Export(response) => {
                let _ = response.send(session.bmp_data());
            }
            WorkerCommand::Stop => break,
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
            let value = (x * 17 + y * 31) as u8;
            Rgba([value, value.wrapping_add(23), value.wrapping_add(71), 255])
        })
    }

    fn document_frame(document: &RgbaImage, top: u32, height: u32) -> BgraFrame {
        rgba_to_bgra_frame(
            image::imageops::crop_imm(document, 0, top, document.width(), height).to_image(),
        )
    }

    #[test]
    fn gpu_shift_appends_only_new_rows() {
        let source = document(80, 180);
        let mut session = ScrollCaptureSession::from_bgra(document_frame(&source, 0, 100)).unwrap();
        let outcome = session
            .push_bgra_frame_gpu(document_frame(&source, 35, 100), 1, 35)
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
    fn gpu_shift_prepends_only_new_rows() {
        let source = document(80, 180);
        let mut session =
            ScrollCaptureSession::from_bgra(document_frame(&source, 42, 100)).unwrap();
        let outcome = session
            .push_bgra_frame_gpu(document_frame(&source, 0, 100), -1, -42)
            .unwrap();
        assert!(outcome.changed);
        assert_eq!(session.stitched.height(), 142);
        assert_eq!(session.current_offset, -42);
        assert_eq!(
            session.stitched.to_image(),
            image::imageops::crop_imm(&source, 0, 0, 80, 142).to_image()
        );
    }

    #[test]
    fn reversing_scroll_never_removes_captured_top_or_bottom_rows() {
        let source = document(80, 220);
        let mut session =
            ScrollCaptureSession::from_bgra(document_frame(&source, 50, 100)).unwrap();
        session
            .push_bgra_frame_gpu(document_frame(&source, 0, 100), -1, -50)
            .unwrap();
        session
            .push_bgra_frame_gpu(document_frame(&source, 50, 100), 1, 50)
            .unwrap();
        session
            .push_bgra_frame_gpu(document_frame(&source, 100, 100), 1, 50)
            .unwrap();

        assert_eq!(session.stitched.height(), 200);
        assert_eq!(
            session.stitched.to_image(),
            image::imageops::crop_imm(&source, 0, 0, 80, 200).to_image()
        );
    }

    #[test]
    fn preview_cache_incrementally_tracks_appended_strip() {
        let mut session = ScrollCaptureSession::from_bgra(frame(60, 100, 1)).unwrap();
        let initial = session.preview_frame(60, 200);
        assert_eq!((initial.width, initial.height), (60, 100));
        session
            .push_bgra_frame_gpu(frame(60, 100, 2), 1, 40)
            .unwrap();
        let extended = session.preview_frame(60, 200);
        assert_eq!((extended.width, extended.height), (60, 140));
        assert_eq!(session.preview_cache.as_ref().unwrap().source_height, 140);
    }
}
