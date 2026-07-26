use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
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

/// Consumer invoked by the grab worker for every captured frame.
///
/// Frames must reach the matcher at capture rate. Routing them through the UI's `WM_TIMER`
/// instead caps delivery at the ~64Hz rate at which Windows synthesises that message (it is the
/// lowest-priority message and is only generated when the queue is otherwise empty), which
/// spaces matched frames far enough apart to exceed the 60%-frame-height limit. This sink lets
/// the grab thread hand frames to the matcher directly.
///
/// The sink takes the frame by value and hands it back when it cannot accept it, so a frame is
/// never copied just to offer it.
pub type FrameSink = Box<dyn Fn(CapturedScrollFrame) -> Option<CapturedScrollFrame> + Send>;

/// Single-slot mailbox for the frame hand-off.
///
/// Each frame is published into one shared slot, overwriting whatever the consumer has not yet
/// taken, and the producer never blocks waiting for it. A bounded channel would instead stall
/// the grabber whenever the matcher runs long.
struct FrameSlot {
    slot: Mutex<Option<Result<CapturedScrollFrame, String>>>,
    closed: AtomicBool,
}

impl FrameSlot {
    fn new() -> Self {
        Self {
            slot: Mutex::new(None),
            closed: AtomicBool::new(false),
        }
    }

    /// Publish a frame, discarding any frame the consumer has not collected yet.
    fn replace(&self, frame: Result<CapturedScrollFrame, String>) -> Result<(), ()> {
        if self.closed.load(Ordering::Acquire) {
            return Err(());
        }
        match self.slot.lock() {
            Ok(mut slot) => {
                *slot = Some(frame);
                Ok(())
            }
            Err(_) => Err(()),
        }
    }

    fn take(&self) -> Option<Result<CapturedScrollFrame, String>> {
        self.slot.lock().ok().and_then(|mut slot| slot.take())
    }

    fn close(&self) {
        self.closed.store(true, Ordering::Release);
        if let Ok(mut slot) = self.slot.lock() {
            *slot = None;
        }
    }
}

/// Window capture used by long screenshots.
pub struct GraphicsCaptureSource {
    frames: Arc<FrameSlot>,
    excluded: Arc<Mutex<Option<Vec<usize>>>>,
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl GraphicsCaptureSource {
    pub fn new(selection: Rect) -> Result<Self, String> {
        Self::with_excluded_windows(selection, Vec::new())
    }

    /// Start the grab worker, excluding `excluded` windows from the captured image.
    ///
    /// The list is forwarded to `MagSetWindowFilterList`; see [`super::magnifier_capture`].
    pub fn with_excluded_windows(
        selection: Rect,
        excluded: Vec<windows::Win32::Foundation::HWND>,
    ) -> Result<Self, String> {
        Self::start(selection, excluded, None)
    }

    /// Start the grab worker and deliver every captured frame straight to `sink`.
    ///
    /// The sink runs on the grab thread and returns `false` once it no longer wants frames.
    /// Frames it accepts are not published to the mailbox, so [`Self::try_next_frame`] only
    /// returns what the sink declined.
    pub fn with_frame_sink(
        selection: Rect,
        excluded: Vec<windows::Win32::Foundation::HWND>,
        sink: FrameSink,
    ) -> Result<Self, String> {
        Self::start(selection, excluded, Some(sink))
    }

    fn start(
        selection: Rect,
        excluded: Vec<windows::Win32::Foundation::HWND>,
        sink: Option<FrameSink>,
    ) -> Result<Self, String> {
        // Publish into a single slot; never block on the consumer.
        let frames = Arc::new(FrameSlot::new());
        let worker_frames = frames.clone();
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = stop.clone();
        // `HWND` is not `Send`; move the handles across as plain addresses and rebuild them on
        // the worker thread, which is the only thread that touches the magnifier.
        let excluded: Vec<usize> = excluded.into_iter().map(|window| window.0 as usize).collect();
        let pending_excluded = Arc::new(Mutex::new(Some(excluded)));
        let worker_excluded = pending_excluded.clone();
        let thread = std::thread::Builder::new()
            .name("longscreenshoter-grab-worker".to_string())
            .spawn(move || {
                run_grab_worker(selection, worker_excluded, worker_frames, thread_stop, sink)
            })
            .map_err(|error| format!("failed to start grab worker: {error}"))?;
        Ok(Self {
            frames,
            excluded: pending_excluded,
            stop,
            thread: Some(thread),
        })
    }

    /// Replace the excluded-window list; the grab worker applies it before its next capture.
    ///
    /// The long-screenshot preview windows are created after capture has already started, so
    /// their handles only become known later. Hence a slot the worker re-reads rather than a
    /// construction-time argument.
    pub fn set_excluded_windows(&self, excluded: Vec<usize>) {
        if let Ok(mut slot) = self.excluded.lock() {
            *slot = Some(excluded);
        }
    }

    pub fn try_next_frame(&self) -> Result<Option<CapturedScrollFrame>, String> {
        match self.frames.take() {
            Some(Ok(frame)) => Ok(Some(frame)),
            Some(Err(error)) => Err(error),
            None => Ok(None),
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
        Err("capture did not produce an initial frame".to_string())
    }
}

impl Drop for GraphicsCaptureSource {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        self.frames.close();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn run_grab_worker(
    selection: Rect,
    excluded: Arc<Mutex<Option<Vec<usize>>>>,
    frames: Arc<FrameSlot>,
    stop: Arc<AtomicBool>,
    sink: Option<FrameSink>,
) {
    // The magnifier windows and their scaling callback belong to this thread.
    let magnifier = match super::magnifier_capture::MagnifierCapture::new(selection) {
        Ok(magnifier) => {
            eprintln!("[scroll capture] capture path: magnifier (Magnification API)");
            Some(magnifier)
        }
        Err(error) => {
            // GDI fallback for when the magnifier is unavailable.
            eprintln!("[scroll capture] magnifier unavailable, falling back to GDI: {error}");
            None
        }
    };

    let mut stats_started = Instant::now();
    let mut stats_frames = 0u32;
    let mut stats_busy = Duration::ZERO;
    while !stop.load(Ordering::Acquire) {
        let started = Instant::now();
        // Pick up a newly published exclusion list (the preview windows appear after start).
        if let Some(magnifier) = magnifier.as_ref() {
            let pending = excluded.lock().ok().and_then(|mut slot| slot.take());
            if let Some(pending) = pending {
                let handles: Vec<windows::Win32::Foundation::HWND> = pending
                    .into_iter()
                    .map(|window| {
                        windows::Win32::Foundation::HWND(window as *mut core::ffi::c_void)
                    })
                    .collect();
                if let Err(error) = magnifier.set_excluded_windows(&handles) {
                    eprintln!("[scroll capture] MagSetWindowFilterList unavailable: {error}");
                }
            }
        }
        let result = match magnifier.as_ref() {
            Some(magnifier) => magnifier.capture(selection),
            None => super::gdi::capture_screen_region_to_bgra(selection),
        }
        .map(|(width, height, pixels)| CapturedScrollFrame {
            frame: BgraFrame {
                width,
                height,
                pixels,
            },
            captured_at: started,
            native_scroll_position: None,
            wheel_sequence: 0,
            discontinuity: false,
        });
        // Hand the frame straight to the matcher when a sink is installed, rather than routing
        // it through the UI message loop, which Windows caps at ~64Hz. A frame the sink declines
        // (matcher still busy) falls through to the mailbox, where it is coalesced latest-wins.
        let pending = match result {
            Ok(frame) => match sink.as_ref() {
                Some(sink) => sink(frame).map(Ok),
                None => Some(Ok(frame)),
            },
            Err(error) => Some(Err(error)),
        };
        if let Some(pending) = pending
            && frames.replace(pending).is_err()
        {
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
