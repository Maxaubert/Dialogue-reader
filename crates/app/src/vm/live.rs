//! The live view of the game on the stages of pages 03, 10 and 11 (Max, 2026-11-22: "there is no live view").
//!
//! - A [`Desktop::live`] session (Windows Graphics Capture on its own thread, `dr_runtime::live`) makes each frame
//!   the stage's size in physical pixels on that thread. The sink below copies it into a `SharedPixelBuffer`,
//!   keeps only the newest one in a slot, and wakes the UI thread once (`invoke_from_event_loop`) until the UI has
//!   taken it. About 15 frames a second; the UI thread only wraps the buffer in an `Image`.
//! - It runs only while a page with a real stage shows (03 with a picked window, 10 and 11 with a current game)
//!   and the app window is not minimized or hidden. A UI timer checks that twice a second; a page change checks
//!   at once.
//! - Overview (10) runs it only while the app window has the focus (Max, 2026-11-23): when he goes back to the
//!   game, the session stops and the last frame stays on the stage (no switch to the saved screenshot, the note
//!   stays). It starts again when the app gets the focus back. Pages 03 and 11 do not care about the focus.
//! - Why the saved screenshot shows is said in the line under the stage, never on the picture. "Live" itself is
//!   not shown anywhere (Max, 2026-11-24): the picture moving says it.
//! - Zones stay in capture pixels. While frames show, the editor uses the capture size (`Editor::rescale` when
//!   the game changes size, after a drag ends). When the live view stops, it goes back to the saved screenshot's
//!   size, and a small note on the stage says why the saved screenshot shows. A frame of a new size that comes
//!   during a drag waits in `pending` and shows when the drag ends.
//! - The game's window is searched on another thread (the window list takes about 10 ms).
//! - The start returns at once (`dr_runtime::live` builds the session on a helper thread). A start Windows
//!   refuses comes back as the end of the session: "no live picture" while the window is open, "closed" if not.
//! - There is no "Retake screenshot" (Max, 2026-11-24): the saved still changes at setup and with the in-game
//!   zone edit only.
//! - While the reader captures the same window (M3b), the live view attaches to the reader's capture session
//!   instead of starting a second one on that window (`dr_runtime::live::Attacher`). When the reader starts or
//!   stops, the live view moves to or from it on the next check, and the last frame stays on the stage meanwhile.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use dr_runtime::live::{LiveFrame, LiveSink};
use dr_runtime::windows_list::WinInfo;
use slint::{ComponentHandle, Image, Rgba8Pixel, SharedPixelBuffer};

use super::{Page, Vm, profile_matches};
use crate::{LiveView, MainWindow};

/// A running session. Dropping it stops it: no frame reaches its sink after the drop.
pub trait LiveRun {
    /// The stage changed size (physical pixels).
    fn set_target(&self, w: u32, h: u32);
}

impl LiveRun for dr_runtime::live::Live {
    fn set_target(&self, w: u32, h: u32) {
        dr_runtime::live::Live::set_target(self, w, h);
    }
}

impl LiveRun for dr_runtime::live::Attached {
    fn set_target(&self, w: u32, h: u32) {
        dr_runtime::live::Attached::set_target(self, w, h);
    }
}

/// How often the UI checks whether the live view should run.
pub const TICK: Duration = Duration::from_millis(500);
/// Frames of the live view at most this often (also when attached to the reader's session).
const INTERVAL: Duration = Duration::from_millis(66);
/// A failed start is tried again after this long.
const RETRY: Duration = Duration::from_secs(3);
/// No frame for this long: check whether the game was minimized. A game that does not change sends no frames,
/// which is fine.
const QUIET: Duration = Duration::from_millis(1500);

pub const NOTE_LIVE: &str = "Live";
pub const NOTE_MINIMIZED: &str = "Saved screenshot · the game is minimized";
pub const NOTE_CLOSED: &str = "Saved screenshot · the game is closed";
pub const NOTE_FAILED: &str = "Saved screenshot · no live picture from this window";

/// The newest frame waiting for the UI, or the end of the session.
#[derive(Default)]
struct SlotData {
    /// Session the contents belong to.
    session: u64,
    frame: Option<(u32, u32, SharedPixelBuffer<Rgba8Pixel>)>,
    ended: Option<String>,
    /// A wake-up is on its way to the UI thread.
    posted: bool,
}

#[derive(Default)]
pub struct Slot(Mutex<SlotData>);

/// Runs on the capture thread.
struct Sink {
    slot: Arc<Slot>,
    session: u64,
    w: slint::Weak<MainWindow>,
}

impl Sink {
    fn put(&self, f: impl FnOnce(&mut SlotData)) {
        let Ok(mut d) = self.slot.0.lock() else {
            return;
        };
        if d.session != self.session {
            return;
        }
        f(&mut d);
        if d.posted {
            return;
        }
        d.posted = true;
        drop(d);
        // no event loop (the tests): nothing is woken, and the test takes the slot itself
        let _ = self
            .w
            .upgrade_in_event_loop(|w| w.global::<LiveView>().invoke_arrived());
    }
}

impl LiveSink for Sink {
    fn frame(&mut self, f: &LiveFrame) {
        let mut buf = SharedPixelBuffer::<Rgba8Pixel>::new(f.w, f.h);
        let bytes = buf.make_mut_bytes();
        if bytes.len() != f.rgba.len() {
            return;
        }
        bytes.copy_from_slice(f.rgba);
        self.put(|d| d.frame = Some((f.src_w, f.src_h, buf)));
    }

    fn ended(&mut self, why: &str) {
        self.put(|d| d.ended = Some(why.to_string()));
    }
}

/// The answer of a window search: its number and the window list.
type Found = (u64, Vec<WinInfo>);

/// The answer of `live_window`.
enum Look {
    Found(WinInfo),
    /// No such window (the game is closed).
    Gone,
    /// The window list is still being read.
    Wait,
}

/// What the live view is for: the setup window (page 03) or the current game (10, 11).
#[derive(Clone, Copy, Debug, PartialEq)]
enum For {
    Setup,
    Game(usize),
}

pub struct LiveState {
    run: Option<Box<dyn LiveRun>>,
    /// `run` is attached to the reader's capture session (not a session of its own).
    attached: bool,
    slot: Arc<Slot>,
    session: u64,
    of: Option<For>,
    win: Option<WinInfo>,
    /// A frame is on the stage now.
    showing: bool,
    /// A frame of a new capture size that came during a zone drag: it shows when the drag ends (a paused game
    /// may send no other frame).
    pending: Option<(u32, u32, SharedPixelBuffer<Rgba8Pixel>)>,
    last_frame: Option<Instant>,
    started: Option<Instant>,
    /// No new start before this (after a failure).
    next_try: Option<Instant>,
    /// The stage size in logical pixels, as the stage reported it. Shared with the stage-size callback, which
    /// stores it even while the view model is busy.
    stage: std::rc::Rc<std::cell::Cell<(f32, f32)>>,
    /// The stage size in physical pixels the session was given.
    target: (u32, u32),
    note: String,
    /// The app window has the focus (winit `Focused`, see `crate::focus_lost` and `crate::focus_gained`).
    focused: bool,
    /// The note of a failed start, kept until the page or game changes.
    failed: Option<(For, &'static str)>,
    /// The search for the game's window (the window list takes about 10 ms, so it runs on another thread):
    /// the number of the newest search, whether it still runs, and its result.
    lookup: u64,
    looking: bool,
    found: Arc<Mutex<Option<Found>>>,
    pub(super) timer: slint::Timer,
}

impl Default for LiveState {
    fn default() -> Self {
        LiveState {
            run: None,
            attached: false,
            slot: Arc::new(Slot::default()),
            session: 0,
            of: None,
            win: None,
            showing: false,
            pending: None,
            last_frame: None,
            started: None,
            next_try: None,
            stage: std::rc::Rc::new(std::cell::Cell::new((1280.0, 720.0))),
            target: (1280, 720),
            note: String::new(),
            focused: true,
            failed: None,
            lookup: 0,
            looking: false,
            found: Arc::new(Mutex::new(None)),
            timer: slint::Timer::default(),
        }
    }
}

impl Vm {
    /// A live view session runs (tests).
    pub fn live_running(&self) -> bool {
        self.live.run.is_some()
    }

    /// Live frames are on the stage (tests).
    pub fn live_showing(&self) -> bool {
        self.live.showing
    }

    /// The app window got (`true`) or lost the focus. Overview pauses its live view without it.
    pub fn live_focus(&mut self, focused: bool) {
        if self.live.focused != focused {
            self.live.focused = focused;
            self.live_tick();
        }
    }

    /// The page on screen wants a live view but must not run one now: Overview while the app window has no
    /// focus (the game is in front). The last frame stays.
    fn live_paused(&self, w: &MainWindow) -> bool {
        !self.live.focused && w.get_page() == Page::Overview
    }

    /// What the page on screen wants a live view of, if anything.
    fn live_wanted(&self, w: &MainWindow) -> Option<For> {
        self.real.as_ref()?;
        let win = w.window();
        if win.is_minimized() || !win.is_visible() {
            return None;
        }
        match w.get_page() {
            Page::DrawZones if self.setup_win.is_some() => Some(For::Setup),
            Page::Zones | Page::Overview => self.current.map(For::Game),
            _ => None,
        }
    }

    /// The window to capture for `of`: the picked window, or a running window of the current game. For a game
    /// the window list is read on another thread (`Desktop::windows_quick_then`); until it answers this gives
    /// `Look::Wait`, and the answer runs the check again.
    fn live_window(&mut self, of: For) -> Look {
        match of {
            For::Setup => self.setup_win.clone().map_or(Look::Gone, Look::Found),
            For::Game(i) => {
                if !self.live.looking {
                    self.live.lookup += 1;
                    self.live.looking = true;
                    let (id, found) = (self.live.lookup, self.live.found.clone());
                    let weak = self.win().map(|w| w.as_weak());
                    let Some(real) = self.real.as_ref() else {
                        return Look::Gone;
                    };
                    real.desktop.windows_quick_then(
                        self.profile_exes(),
                        Box::new(move |list| {
                            if let Ok(mut f) = found.lock() {
                                *f = Some((id, list));
                            }
                            // no event loop (the tests): the next check picks it up
                            if let Some(weak) = weak {
                                let _ = weak
                                    .upgrade_in_event_loop(|w| w.invoke_page_shown(w.get_page()));
                            }
                        }),
                    );
                }
                let id = self.live.lookup;
                let list = self
                    .live
                    .found
                    .lock()
                    .ok()
                    .and_then(|mut f| f.take_if(|(n, _)| *n == id))
                    .map(|(_, list)| list);
                let Some(list) = list else {
                    return Look::Wait;
                };
                self.live.looking = false;
                let Some(g) = self.games.get(i) else {
                    return Look::Gone;
                };
                list.into_iter()
                    .find(|w| profile_matches(&g.profile, w))
                    .map_or(Look::Gone, Look::Found)
            }
        }
    }

    /// The stage changed size (logical px).
    pub fn live_stage_size(&mut self, w: f32, h: f32) {
        self.live.stage.set((w, h));
        self.live_target();
    }

    /// The stage size in physical pixels to the session: after the stage or the scale factor changed (the window
    /// moved to another monitor).
    fn live_target(&mut self) {
        let (w, h) = self.live.stage.get();
        let k = self.win().map_or(1.0, |w| w.window().scale_factor());
        let t = (
            ((w * k).round() as u32).max(1),
            ((h * k).round() as u32).max(1),
        );
        if t != self.live.target {
            self.live.target = t;
            if let Some(r) = &self.live.run {
                r.set_target(t.0, t.1);
            }
        }
    }

    /// Starts, keeps or stops the live view for the page on screen. Runs on a page change and on the timer.
    pub fn live_tick(&mut self) {
        let Some(w) = self.win() else { return };
        self.live_target();
        let want = self.live_wanted(&w);
        if want != self.live.of {
            self.live_stop();
            self.live.next_try = None;
            if self.live.failed.is_some_and(|(f, _)| Some(f) != want) {
                self.live.failed = None;
            }
        }
        let Some(of) = want else {
            self.live_note(&w, "");
            return;
        };
        if self.live_paused(&w) {
            self.live_pause();
            return;
        }
        if self.live.run.is_none() {
            if self.live.next_try.is_some_and(|t| Instant::now() < t) {
                return;
            }
            self.live_start(&w, of);
            return;
        }
        // the reader started or stopped capturing this window: move to its session, or to one of our own
        let shared = self
            .live
            .win
            .as_ref()
            .is_some_and(|win| self.reading_attacher(win.hwnd).is_some());
        if shared != self.live.attached {
            self.live_pause();
            self.live_start(&w, of);
            return;
        }
        // running: a quiet game is fine, a minimized or closed one shows the saved screenshot
        let quiet_since = self.live.last_frame.or(self.live.started);
        if quiet_since.is_some_and(|t| t.elapsed() >= QUIET) {
            let why = match (&self.real, &self.live.win) {
                (Some(real), Some(win)) if real.desktop.is_minimized(win) => Some(NOTE_MINIMIZED),
                (Some(real), Some(win)) if !real.desktop.is_open(win) => Some(NOTE_CLOSED),
                _ => None,
            };
            if let Some(why) = why {
                self.live_fallback(&w, why);
            }
        }
    }

    fn live_start(&mut self, w: &MainWindow, of: For) {
        let win = match self.live_window(of) {
            Look::Found(win) => win,
            Look::Wait => {
                self.live.of = Some(of);
                return;
            }
            Look::Gone => {
                self.live.of = Some(of);
                self.live.next_try = Some(Instant::now() + RETRY);
                self.live_fallback(w, NOTE_CLOSED);
                return;
            }
        };
        let Some(real) = &self.real else { return };
        if !real.desktop.is_open(&win) {
            self.live.of = Some(of);
            self.live.next_try = Some(Instant::now() + RETRY);
            self.live_fallback(w, NOTE_CLOSED);
            return;
        }
        if real.desktop.is_minimized(&win) {
            self.live.of = Some(of);
            self.live.next_try = Some(Instant::now() + RETRY);
            self.live_fallback(w, NOTE_MINIMIZED);
            return;
        }
        self.live.session += 1;
        let session = self.live.session;
        if let Ok(mut d) = self.live.slot.0.lock() {
            *d = SlotData {
                session,
                ..SlotData::default()
            };
        }
        let sink = || Sink {
            slot: self.live.slot.clone(),
            session,
            w: w.as_weak(),
        };
        let (tw, th) = self.live.target;
        // the reader captures this window already: one session serves both
        let attached = self
            .reading_attacher(win.hwnd)
            .and_then(|a| a.attach(tw, th, INTERVAL, Box::new(sink())))
            .map(|a| Box::new(a) as Box<dyn LiveRun>);
        let is_attached = attached.is_some();
        let run = match attached {
            Some(a) => Ok(a),
            None => real.desktop.live(&win, tw, th, Box::new(sink())),
        };
        match run {
            Ok(run) => {
                self.live.run = Some(run);
                self.live.attached = is_attached;
                self.live.of = Some(of);
                self.live.win = Some(win);
                self.live.started = Some(Instant::now());
                self.live.last_frame = None;
                // the saved screenshot (and the note of a failure before, if any) shows until the first frame
                // comes: a start that fails again then does not make the note blink
                if !self.live.showing {
                    self.live_note(w, "");
                }
            }
            Err(_) => {
                self.live.of = Some(of);
                self.live.next_try = Some(Instant::now() + RETRY);
                self.live.failed = Some((of, NOTE_FAILED));
                self.live_fallback(w, NOTE_FAILED);
            }
        }
    }

    /// Ends the session and shows the saved screenshot again (no note).
    pub fn live_stop(&mut self) {
        self.live.run = None;
        self.live.session += 1;
        if let Ok(mut d) = self.live.slot.0.lock() {
            d.session = self.live.session;
            d.frame = None;
            d.ended = None;
        }
        let of = self.live.of.take();
        self.live.pending = None;
        // a window search still running is for the page or game before: its answer is not used
        self.live.looking = false;
        self.live.lookup += 1;
        self.live.win = None;
        self.live.last_frame = None;
        self.live.started = None;
        if let Some(w) = self.win() {
            self.live_hide(&w, of);
            self.live_note(&w, "");
        }
    }

    /// Ends the session but keeps what the stage shows: the last frame (or the saved screenshot) and the note. The
    /// next start (when the app has the focus again) goes on from there without a flicker.
    fn live_pause(&mut self) {
        if self.live.run.is_none() && !self.live.looking {
            return;
        }
        self.live.run = None;
        self.live.session += 1;
        if let Ok(mut d) = self.live.slot.0.lock() {
            d.session = self.live.session;
            d.frame = None;
            d.ended = None;
        }
        self.live.pending = None;
        // a window search still running is not used: the next start asks again
        self.live.looking = false;
        self.live.lookup += 1;
        self.live.last_frame = None;
        self.live.started = None;
        self.live.next_try = None;
    }

    /// Shows the saved screenshot with a note that says why. The session (if any) keeps running: a game that is
    /// opened again sends frames again.
    fn live_fallback(&mut self, w: &MainWindow, note: &str) {
        let of = self.live.of;
        self.live.pending = None;
        self.live_hide(w, of);
        self.live_note(w, note);
    }

    /// Back to the saved screenshot: the stage shows it, the editor uses its size again.
    fn live_hide(&mut self, w: &MainWindow, of: Option<For>) {
        if !self.live.showing {
            return;
        }
        self.live.showing = false;
        let g = w.global::<LiveView>();
        g.set_on(false);
        // the last frame (up to about 18 MB) is not kept behind the saved screenshot
        g.set_frame(Image::default());
        let setup = of == Some(For::Setup);
        if let Some((sw, sh)) = self.still_size(setup) {
            let page = if setup { Page::DrawZones } else { Page::Zones };
            let e = self.editor(page);
            if (e.img_w, e.img_h) != (sw, sh) {
                e.rescale(sw, sh);
                self.after(page, false);
            }
        }
        if !setup {
            self.push_shot_time();
        }
    }

    fn live_note(&mut self, w: &MainWindow, note: &str) {
        let note = if note.is_empty() {
            self.live
                .failed
                .filter(|(f, _)| Some(*f) == self.live.of)
                .map_or("", |(_, n)| n)
        } else {
            note
        };
        if self.live.note != note {
            self.live.note = note.to_string();
            let g = w.global::<LiveView>();
            g.set_note(note.into());
            g.set_note_live(note == NOTE_LIVE);
            if self.live.of != Some(For::Setup) {
                self.push_shot_time();
            }
        }
    }

    /// Size of the saved screenshot the stage falls back to.
    fn still_size(&self, setup: bool) -> Option<(f32, f32)> {
        if setup {
            return self
                .setup_shot
                .as_ref()
                .map(|s| (s.width as f32, s.height as f32));
        }
        let g = self.games.get(self.current?)?;
        let (w, h) = g
            .shot_size
            .or_else(|| g.profile.zones.first().map(|z| (z.base[0], z.base[1])))?;
        Some((w as f32, h as f32))
    }

    /// Takes the newest frame (or the end) from the capture thread and shows it. Called on the UI thread when
    /// the sink wakes it; tests call it directly.
    pub fn live_arrived(&mut self) {
        let Some(w) = self.win() else { return };
        let (frame, ended) = {
            let Ok(mut d) = self.live.slot.0.lock() else {
                return;
            };
            d.posted = false;
            if d.session != self.live.session {
                return;
            }
            (d.frame.take(), d.ended.take())
        };
        let Some(of) = self.live.of.filter(|_| self.live.run.is_some()) else {
            return;
        };
        if ended.as_deref() == Some(dr_runtime::live::STOPPED) {
            // the reader's session it was attached to stopped: a session of its own starts at the next check,
            // and the last frame stays meanwhile
            self.live_pause();
            return;
        }
        if ended.is_some() {
            self.live.run = None;
            self.live.pending = None;
            self.live.next_try = Some(Instant::now() + RETRY);
            // the window closed, or the capture failed (a start Windows refused, a frame it could not read)
            let open = match (&self.real, &self.live.win) {
                (Some(real), Some(win)) => real.desktop.is_open(win),
                _ => false,
            };
            let note = if open {
                self.live.failed = Some((of, NOTE_FAILED));
                NOTE_FAILED
            } else {
                NOTE_CLOSED
            };
            self.live_fallback(&w, note);
            return;
        }
        // the newest frame wins; without one, a frame that waited for a drag to end is tried again
        let frame = match frame {
            Some(f) => {
                self.live.pending = None;
                Some(f)
            }
            None => self.live.pending.take(),
        };
        let Some((src_w, src_h, buf)) = frame else {
            return;
        };
        let setup = of == For::Setup;
        let page = if setup { Page::DrawZones } else { Page::Zones };
        let (sw, sh) = (src_w as f32, src_h as f32);
        let e = self.editor(page);
        if (e.img_w, e.img_h) != (sw, sh) {
            if e.dragging() {
                // the game changed size during a drag: keep the old picture until the drag ends, then this one
                self.live.pending = Some((src_w, src_h, buf));
                return;
            }
            e.rescale(sw, sh);
            self.after(page, false);
        }
        self.live.last_frame = Some(Instant::now());
        self.live.failed = None;
        let g = w.global::<LiveView>();
        g.set_frame(Image::from_rgba8(buf));
        g.set_w(sw);
        g.set_h(sh);
        if !self.live.showing {
            self.live.showing = true;
            g.set_on(true);
        }
        if !setup {
            self.push_shot_time();
        }
        self.live_note(&w, NOTE_LIVE);
    }

    /// A zone drag ended: a frame that waited for it shows now.
    pub(super) fn live_flush(&mut self) {
        if self.live.pending.is_some() {
            self.live_arrived();
        }
    }

    /// The time of the current game's saved screenshot ("21:30") in the Zones toolbar while the saved
    /// screenshot shows (TEXT.md); empty while the game shows live.
    pub(super) fn push_shot_time(&self) {
        let Some(w) = self.win() else { return };
        let t = if self.live.showing {
            String::new()
        } else {
            self.real
                .as_ref()
                .zip(self.current.and_then(|i| self.games.get(i)))
                .and_then(|(r, g)| std::fs::metadata(r.paths.profile_png(&g.profile.id)).ok())
                .and_then(|m| m.modified().ok())
                .map_or(String::new(), clock_of)
        };
        w.set_shot_time(t.into());
    }
}

/// "21:30": the local hours and minutes of a file time or a capture time.
pub fn clock_of(t: std::time::SystemTime) -> String {
    let ms = t
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64);
    let s = dr_runtime::activity::local_time(ms);
    s.get(..5).unwrap_or(&s).to_string()
}

/// Wires the live view: the stage size, frames, page changes and the timer.
pub fn wire(w: &MainWindow, vm: &super::Shared) {
    if vm.borrow().real.is_none() {
        return;
    }
    let g = w.global::<LiveView>();
    // These can come while the view model is busy (a page built during one of its calls): then the next tick
    // does it.
    let v = std::rc::Rc::downgrade(vm);
    let stage = vm.borrow().live.stage.clone();
    g.on_stage_size(move |sw, sh| {
        stage.set((sw, sh));
        if let Some(vm) = v.upgrade()
            && let Ok(mut vm) = vm.try_borrow_mut()
        {
            vm.live_target();
        }
    });
    let v = std::rc::Rc::downgrade(vm);
    g.on_arrived(move || {
        if let Some(vm) = v.upgrade()
            && let Ok(mut vm) = vm.try_borrow_mut()
        {
            vm.live_arrived();
        }
    });
    let v = std::rc::Rc::downgrade(vm);
    w.on_page_shown(move |_| {
        if let Some(vm) = v.upgrade()
            && let Ok(mut vm) = vm.try_borrow_mut()
        {
            vm.live_tick();
        }
    });
    let v = std::rc::Rc::downgrade(vm);
    vm.borrow()
        .live
        .timer
        .start(slint::TimerMode::Repeated, TICK, move || {
            if let Some(vm) = v.upgrade()
                && let Ok(mut vm) = vm.try_borrow_mut()
            {
                vm.live_tick();
                // a wake-up lost while the view model was busy is picked up here
                vm.live_arrived();
            }
        });
}
