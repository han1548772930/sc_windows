use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{
    Arc,
    mpsc::{self, Receiver, SyncSender},
};
use std::time::{Duration, Instant};

use sc_drawing::Rect;

#[derive(Clone, Debug)]
pub struct BgraFrame {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

#[derive(Clone, Debug)]
pub struct CapturedScrollFrame {
    pub frame: BgraFrame,
    pub captured_at: Instant,
    pub native_scroll_position: Option<i32>,
    pub wheel_sequence: u64,
    pub discontinuity: bool,
}

/// Window capture used by long screenshots.
pub struct GraphicsCaptureSource {
    frames: Receiver<Result<CapturedScrollFrame, String>>,
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl GraphicsCaptureSource {
    pub fn new(selection: Rect) -> Result<Self, String> {
        // Preserve WeChat's ordered single-worker pipeline without allowing
        // this faster BitBlt producer to accumulate an unbounded frame queue.
        let (frame_tx, frame_rx) = mpsc::sync_channel(1);
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = stop.clone();
        let thread = std::thread::Builder::new()
            .name("longscreenshoter-grab-worker".to_string())
            .spawn(move || run_gdi_grab_worker(selection, frame_tx, thread_stop))
            .map_err(|error| format!("failed to start GDI grab worker: {error}"))?;
        Ok(Self {
            frames: frame_rx,
            stop,
            thread: Some(thread),
        })
    }

    pub fn try_next_frame(&self) -> Result<Option<CapturedScrollFrame>, String> {
        match self.frames.try_recv() {
            Ok(Ok(frame)) => Ok(Some(frame)),
            Ok(Err(error)) => Err(error),
            Err(mpsc::TryRecvError::Empty) => Ok(None),
            Err(mpsc::TryRecvError::Disconnected) => Err("GDI grab worker stopped".to_string()),
        }
    }

    pub fn wait_for_first_frame(&self, timeout: Duration) -> Result<CapturedScrollFrame, String> {
        let started = Instant::now();
        while started.elapsed() < timeout {
            if let Some(frame) = self.try_next_frame()? {
                return Ok(frame);
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        Err("GDI capture did not produce an initial frame".to_string())
    }
}

impl Drop for GraphicsCaptureSource {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            while !thread.is_finished() {
                while self.frames.try_recv().is_ok() {}
                std::thread::yield_now();
            }
            let _ = thread.join();
        }
    }
}

fn run_gdi_grab_worker(
    selection: Rect,
    frames: SyncSender<Result<CapturedScrollFrame, String>>,
    stop: Arc<AtomicBool>,
) {
    let mut stats_started = Instant::now();
    let mut stats_frames = 0u32;
    let mut stats_busy = Duration::ZERO;
    while !stop.load(Ordering::Acquire) {
        let started = Instant::now();
        let result = super::gdi::capture_screen_region_to_bmp(selection).and_then(|bmp| {
            let image = image::load_from_memory(&bmp)
                .map_err(|error| format!("GDI BMP decode failed: {error}"))?
                .to_rgba8();
            let (width, height) = image.dimensions();
            let mut pixels = image.into_raw();
            for pixel in pixels.chunks_exact_mut(4) {
                pixel.swap(0, 2);
            }
            Ok(CapturedScrollFrame {
                frame: BgraFrame {
                    width,
                    height,
                    pixels,
                },
                captured_at: started,
                native_scroll_position: None,
                wheel_sequence: 0,
                discontinuity: false,
            })
        });
        if frames.send(result).is_err() {
            break;
        }
        stats_frames = stats_frames.saturating_add(1);
        stats_busy += started.elapsed();
        if stats_started.elapsed() >= Duration::from_secs(1) {
            let elapsed = stats_started.elapsed();
            eprintln!(
                "[scroll capture] grab stats: fps={:.1}, avg={:.2}ms, busy={:.1}%",
                stats_frames as f64 / elapsed.as_secs_f64(),
                stats_busy.as_secs_f64() * 1000.0 / stats_frames.max(1) as f64,
                stats_busy.as_secs_f64() / elapsed.as_secs_f64() * 100.0
            );
            stats_started = Instant::now();
            stats_frames = 0;
            stats_busy = Duration::ZERO;
        }
    }
}
