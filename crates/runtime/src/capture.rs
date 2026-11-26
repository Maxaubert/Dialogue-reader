//! Frames for the reader thread (spec 2.2): a [`FrameSource`] gives the newest frame, or says that nothing new
//! came.
//!
//! - [`WgcSource`]: Windows Graphics Capture on the game window, at most one frame every [`INTERVAL`] (40 ms, the
//!   `Manual` ring step). The capture thread copies each frame into a "newest frame" slot; an older frame that
//!   the reader did not take yet is replaced, so a slow OCR call never builds a backlog. The session is a
//!   [`crate::live::Live`] with a full-size sink, so the live view of the same window attaches to it instead of
//!   starting a second session ([`WgcSource::attacher`]).
//! - [`ScriptSource`]: frames made on demand at given times (the headless runtime test plays arena scenes
//!   through it), in lockstep with the reader.
//!
//! WGC sends a frame only when the window changed. A dialogue box that appears and then stays still sends one
//! frame. So the source also answers [`Next::Same`] with the time now: the reader feeds the frame it kept again,
//! and `dr_core::Pipeline` can confirm the line.

use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

/// At most one frame this often (25 a second).
pub const INTERVAL: Duration = Duration::from_millis(40);

/// One frame, tight RGBA rows (`w * h * 4` bytes). `t_ms` is on the source's clock.
pub struct OwnedFrame {
    pub t_ms: f64,
    pub w: u32,
    pub h: u32,
    pub rgba: Vec<u8>,
}

/// The answer of [`FrameSource::next`].
pub enum Next {
    /// A new frame.
    Frame(OwnedFrame),
    /// Nothing new by the time `t_ms` (the screen did not change): the last frame still shows.
    Same(f64),
    /// No frame follows (the window closed, the capture failed, or a scripted source ran out).
    Ended(String),
}

pub trait FrameSource: Send {
    /// Waits up to `wait` for a frame newer than the last one given.
    fn next(&mut self, wait: Duration) -> Next;
    /// Gives a frame's buffer back for reuse.
    fn recycle(&mut self, _rgba: Vec<u8>) {}
}

/// The slot between the capture thread and the reader.
#[derive(Default)]
struct SlotData {
    newest: Option<(u32, u32, Vec<u8>, Instant)>,
    spare: Option<Vec<u8>>,
    ended: Option<String>,
}

#[derive(Default)]
struct Slot {
    data: Mutex<SlotData>,
    cv: Condvar,
}

impl Slot {
    fn lock(&self) -> std::sync::MutexGuard<'_, SlotData> {
        self.data.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// Runs on the capture thread.
#[cfg(windows)]
struct SlotSink(Arc<Slot>);

#[cfg(windows)]
impl crate::live::FullSink for SlotSink {
    fn buffer(&mut self) -> Vec<u8> {
        self.0.lock().spare.take().unwrap_or_default()
    }

    fn frame(&mut self, w: u32, h: u32, rgba: Vec<u8>) {
        let mut d = self.0.lock();
        // a frame the reader did not take yet is replaced; its buffer is the next spare
        if let Some((_, _, old, _)) = d.newest.replace((w, h, rgba, Instant::now())) {
            d.spare = Some(old);
        }
        drop(d);
        self.0.cv.notify_one();
    }

    fn ended(&mut self, why: &str) {
        self.0.lock().ended = Some(why.to_string());
        self.0.cv.notify_one();
    }
}

/// Frames of a game window through Windows Graphics Capture.
#[cfg(windows)]
pub struct WgcSource {
    slot: Arc<Slot>,
    live: crate::live::Live,
    clock: Instant,
}

#[cfg(windows)]
impl WgcSource {
    /// Starts capturing `hwnd` (`HWND.0 as isize`). Returns at once; a start Windows refuses comes as
    /// [`Next::Ended`]. Err only for a window that is already gone.
    pub fn start(hwnd: isize) -> Result<WgcSource, String> {
        let slot = Arc::new(Slot::default());
        let live = crate::live::Live::start_full(hwnd, INTERVAL, Box::new(SlotSink(slot.clone())))?;
        Ok(WgcSource {
            slot,
            live,
            clock: Instant::now(),
        })
    }

    /// Attaches live views (the stages of the app) to this session.
    pub fn attacher(&self) -> crate::live::Attacher {
        self.live.attacher()
    }

    fn ms(&self, t: Instant) -> f64 {
        t.saturating_duration_since(self.clock).as_secs_f64() * 1000.0
    }
}

#[cfg(windows)]
impl FrameSource for WgcSource {
    fn next(&mut self, wait: Duration) -> Next {
        let end = Instant::now() + wait;
        let mut d = self.slot.lock();
        loop {
            if let Some((w, h, rgba, at)) = d.newest.take() {
                drop(d);
                return Next::Frame(OwnedFrame {
                    t_ms: self.ms(at),
                    w,
                    h,
                    rgba,
                });
            }
            if let Some(why) = &d.ended {
                return Next::Ended(why.clone());
            }
            let now = Instant::now();
            if now >= end {
                drop(d);
                return Next::Same(self.ms(now));
            }
            d = self
                .slot
                .cv
                .wait_timeout(d, end - now)
                .unwrap_or_else(|e| e.into_inner())
                .0;
        }
    }

    fn recycle(&mut self, rgba: Vec<u8>) {
        let mut d = self.slot.lock();
        if d.spare.is_none() {
            d.spare = Some(rgba);
        }
    }
}

/// Makes one frame for a time (ms): `None` when the picture did not change since the last call.
pub type Render = Box<dyn FnMut(f64) -> Option<(u32, u32, Vec<u8>)> + Send>;

/// Frames on a fixed time grid, made on demand: each [`FrameSource::next`] call is the next step of `step_ms`,
/// in lockstep with the reader (nothing is dropped, nothing waits for a clock). For tests.
pub struct ScriptSource {
    render: Render,
    t_ms: f64,
    step_ms: f64,
    /// After this time, [`Next::Ended`].
    end_ms: f64,
    gate: Option<Arc<Gate>>,
}

/// Lets a test hold a [`ScriptSource`] before a time (the test then checks the runtime at that point).
#[derive(Default)]
pub struct Gate {
    until: Mutex<f64>,
    cv: Condvar,
    /// The source's time now.
    at: Mutex<f64>,
}

impl Gate {
    pub fn new(until_ms: f64) -> Arc<Gate> {
        Arc::new(Gate {
            until: Mutex::new(until_ms),
            ..Gate::default()
        })
    }

    /// Lets the source run on to `until_ms`.
    pub fn open_until(&self, until_ms: f64) {
        *self.until.lock().unwrap_or_else(|e| e.into_inner()) = until_ms;
        self.cv.notify_all();
    }

    /// The source's time now (the last step it gave).
    pub fn at(&self) -> f64 {
        *self.at.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Waits up to `limit` until the source reached `t_ms`.
    pub fn wait_at(&self, t_ms: f64, limit: Duration) -> bool {
        let end = Instant::now() + limit;
        while Instant::now() < end {
            if self.at() >= t_ms {
                return true;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        self.at() >= t_ms
    }
}

impl ScriptSource {
    /// Frames from `render` at 0, `step_ms`, 2 `step_ms` ... up to `end_ms`.
    pub fn new(render: Render, step_ms: f64, end_ms: f64) -> ScriptSource {
        ScriptSource {
            render,
            t_ms: -step_ms,
            step_ms,
            end_ms,
            gate: None,
        }
    }

    /// The source stops at the gate's time until the gate opens further.
    pub fn with_gate(mut self, gate: Arc<Gate>) -> ScriptSource {
        self.gate = Some(gate);
        self
    }
}

impl FrameSource for ScriptSource {
    fn next(&mut self, wait: Duration) -> Next {
        let t = self.t_ms + self.step_ms;
        if t > self.end_ms {
            return Next::Ended("the script ended".into());
        }
        if let Some(g) = &self.gate {
            let until = g.until.lock().unwrap_or_else(|e| e.into_inner());
            if t > *until {
                // held: nothing new yet (the reader goes on with what it has, the time does not move)
                let _ = g.cv.wait_timeout(until, wait);
                return Next::Same(self.t_ms.max(0.0));
            }
        }
        self.t_ms = t;
        if let Some(g) = &self.gate {
            *g.at.lock().unwrap_or_else(|e| e.into_inner()) = t;
        }
        match (self.render)(t) {
            Some((w, h, rgba)) => Next::Frame(OwnedFrame {
                t_ms: t,
                w,
                h,
                rgba,
            }),
            None => Next::Same(t),
        }
    }
}
