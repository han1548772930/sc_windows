use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
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
pub type FrameSinkReady = Box<dyn Fn() -> bool + Send>;

/// Single-slot mailbox for the frame hand-off.
///
/// Each frame is published into one shared slot, overwriting whatever the consumer has not yet
/// taken, and the producer never blocks waiting for it. A bounded channel would instead stall
/// the grabber whenever the matcher runs long.
struct FrameSlot {
    slot: Mutex<Option<Result<CapturedScrollFrame, String>>>,
    available: Condvar,
    closed: AtomicBool,
}

impl FrameSlot {
    fn new() -> Self {
        Self {
            slot: Mutex::new(None),
            available: Condvar::new(),
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
                self.available.notify_all();
                Ok(())
            }
            Err(_) => Err(()),
        }
    }

    fn take(&self) -> Option<Result<CapturedScrollFrame, String>> {
        self.slot.lock().ok().and_then(|mut slot| slot.take())
    }

    fn wait_and_take(&self) -> Option<Result<CapturedScrollFrame, String>> {
        let mut slot = self.slot.lock().ok()?;
        while slot.is_none() && !self.closed.load(Ordering::Acquire) {
            slot = self.available.wait(slot).ok()?;
        }
        slot.take()
    }

    fn wake(&self) {
        self.available.notify_all();
    }

    fn close(&self) {
        self.closed.store(true, Ordering::Release);
        if let Ok(mut slot) = self.slot.lock() {
            *slot = None;
        }
        self.available.notify_all();
    }
}

/// Window capture used by long screenshots.
pub struct GraphicsCaptureSource {
    frames: Arc<FrameSlot>,
    excluded: Arc<Mutex<Option<Vec<usize>>>>,
    excluded_requested: Arc<AtomicU64>,
    excluded_applied: Arc<AtomicU64>,
    stop: Arc<AtomicBool>,
    delivery_enabled: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
    dispatch_thread: Option<std::thread::JoinHandle<()>>,
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
        ready: FrameSinkReady,
        sink: FrameSink,
    ) -> Result<Self, String> {
        Self::start(selection, excluded, Some((ready, sink)))
    }

    fn start(
        selection: Rect,
        excluded: Vec<windows::Win32::Foundation::HWND>,
        sink: Option<(FrameSinkReady, FrameSink)>,
    ) -> Result<Self, String> {
        // Publish into a single slot; never block on the consumer.
        let frames = Arc::new(FrameSlot::new());
        let worker_frames = frames.clone();
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = stop.clone();
        let delivery_enabled = Arc::new(AtomicBool::new(sink.is_none()));
        // `HWND` is not `Send`; move the handles across as plain addresses and rebuild them on
        // the worker thread, which is the only thread that touches the magnifier.
        let excluded: Vec<usize> = excluded
            .into_iter()
            .map(|window| window.0 as usize)
            .collect();
        let pending_excluded = Arc::new(Mutex::new(Some(excluded)));
        let worker_excluded = pending_excluded.clone();
        let excluded_requested = Arc::new(AtomicU64::new(1));
        let excluded_applied = Arc::new(AtomicU64::new(0));
        let worker_excluded_requested = excluded_requested.clone();
        let worker_excluded_applied = excluded_applied.clone();
        let thread = std::thread::Builder::new()
            .name("longscreenshoter-grab-worker".to_string())
            .spawn(move || {
                run_grab_worker(
                    selection,
                    worker_excluded,
                    worker_excluded_requested,
                    worker_excluded_applied,
                    worker_frames,
                    thread_stop,
                )
            })
            .map_err(|error| format!("failed to start grab worker: {error}"))?;
        let dispatch_thread = sink.map(|(ready, sink)| {
            let dispatch_frames = frames.clone();
            let dispatch_stop = stop.clone();
            let dispatch_enabled = delivery_enabled.clone();
            std::thread::Builder::new()
                .name("longscreenshoter-frame-dispatch".to_string())
                .spawn(move || {
                    while !dispatch_stop.load(Ordering::Acquire) {
                        if !dispatch_enabled.load(Ordering::Acquire) {
                            std::thread::sleep(Duration::from_millis(1));
                            continue;
                        }
                        // WeChat's consumer does not take the shared slot while OpenCVWorker is
                        // processing. GrabWorker keeps overwriting it, and only once the matcher
                        // is ready does the consumer take the then-current latest frame.
                        if !ready() {
                            std::thread::sleep(Duration::from_millis(1));
                            continue;
                        }
                        let Some(frame) = dispatch_frames.wait_and_take() else {
                            break;
                        };
                        match frame {
                            Ok(frame) => {
                                if let Some(frame) = sink(frame) {
                                    // Readiness can only change between the pre-check and submit
                                    // during shutdown. Keep the frame available in that rare case.
                                    let _ = dispatch_frames.replace(Ok(frame));
                                    std::thread::sleep(Duration::from_millis(1));
                                }
                            }
                            Err(error) => {
                                if dispatch_frames.replace(Err(error)).is_err() {
                                    break;
                                }
                                break;
                            }
                        }
                    }
                })
                .expect("failed to start scrolling frame dispatcher")
        });
        Ok(Self {
            frames,
            excluded: pending_excluded,
            excluded_requested,
            excluded_applied,
            stop,
            delivery_enabled,
            thread: Some(thread),
            dispatch_thread,
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
            self.excluded_requested.fetch_add(1, Ordering::AcqRel);
        }
    }

    /// Wait until the grab worker has applied the most recently published filter list.
    pub fn wait_for_excluded_windows(&self, timeout: Duration) -> bool {
        let requested = self.excluded_requested.load(Ordering::Acquire);
        let started = Instant::now();
        while self.excluded_applied.load(Ordering::Acquire) < requested {
            if started.elapsed() >= timeout {
                return false;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        true
    }

    pub fn enable_delivery(&self) {
        // Do not deliver a frame captured while the preview was not yet excluded.
        let _ = self.frames.take();
        self.delivery_enabled.store(true, Ordering::Release);
        self.frames.wake();
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
        if let Some(thread) = self.dispatch_thread.take() {
            let _ = thread.join();
        }
    }
}

fn run_grab_worker(
    selection: Rect,
    excluded: Arc<Mutex<Option<Vec<usize>>>>,
    excluded_requested: Arc<AtomicU64>,
    excluded_applied: Arc<AtomicU64>,
    frames: Arc<FrameSlot>,
    stop: Arc<AtomicBool>,
) {
    // The magnifier windows and their scaling callback belong to this thread.
    let magnifier = match super::magnifier_capture::MagnifierCapture::new(selection) {
        Ok(magnifier) => {
            eprintln!("[scroll capture] capture path: magnifier (Magnification API)");
            magnifier
        }
        Err(error) => {
            let _ = frames.replace(Err(format!(
                "Magnification API initialization failed: {error}"
            )));
            return;
        }
    };

    let mut stats_started = Instant::now();
    let mut stats_frames = 0u32;
    let mut stats_busy = Duration::ZERO;
    let mut applied_excluded = Vec::new();
    while !stop.load(Ordering::Acquire) {
        let started = Instant::now();
        // Pick up a newly published exclusion list (the preview windows appear after start).
        let pending = excluded.lock().ok().and_then(|mut slot| slot.take());
        if let Some(pending) = pending {
            let mut filter_applied = true;
            if pending != applied_excluded {
                let handles: Vec<windows::Win32::Foundation::HWND> = pending
                    .iter()
                    .copied()
                    .map(|window| {
                        windows::Win32::Foundation::HWND(window as *mut core::ffi::c_void)
                    })
                    .collect();
                if let Err(error) = magnifier.set_excluded_windows(&handles) {
                    eprintln!("[scroll capture] MagSetWindowFilterList unavailable: {error}");
                    filter_applied = false;
                } else {
                    applied_excluded = pending;
                }
            }
            if filter_applied {
                excluded_applied.store(
                    excluded_requested.load(Ordering::Acquire),
                    Ordering::Release,
                );
            }
        }
        let result =
            magnifier
                .capture(selection)
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
        // The producer always replaces the shared slot and never waits for the consumer.
        if frames.replace(result).is_err() {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn captured(seed: u8) -> CapturedScrollFrame {
        CapturedScrollFrame {
            frame: BgraFrame {
                width: 1,
                height: 1,
                pixels: vec![seed, seed, seed, 255],
            },
            captured_at: Instant::now(),
            native_scroll_position: None,
            wheel_sequence: 0,
            discontinuity: false,
        }
    }

    #[test]
    fn frame_slot_keeps_only_the_latest_frame() {
        let slot = FrameSlot::new();
        slot.replace(Ok(captured(1))).unwrap();
        slot.replace(Ok(captured(2))).unwrap();

        let frame = slot.take().unwrap().unwrap();
        assert_eq!(frame.frame.pixels[0], 2);
        assert!(slot.take().is_none());
    }

    #[test]
    fn closing_frame_slot_wakes_waiting_consumer() {
        let slot = Arc::new(FrameSlot::new());
        let waiting_slot = slot.clone();
        let waiter = std::thread::spawn(move || waiting_slot.wait_and_take());
        std::thread::yield_now();
        slot.close();

        assert!(waiter.join().unwrap().is_none());
    }
}
