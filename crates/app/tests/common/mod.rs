//! Shared test helpers: a fake desktop (windows and screenshots without touching the real desktop), a temp
//! data folder, and pointer and key input through the Slint testing backend. Nothing shows on screen.

#![allow(dead_code)]

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use dr_app::MainWindow;
use dr_app::vm::voices::Voices;
use dr_app::vm::{Desktop, LiveRun, Source};
use dr_runtime::live::{LiveFrame, LiveSink};
use dr_runtime::speech::{NullSink, Speech, VoiceSettings};
use dr_runtime::windows_list::{WinInfo, WinMode};
use dr_runtime::{Paths, Shot};
use i_slint_backend_testing::{AccessibleRole, ElementHandle};
use slint::platform::{PointerEventButton, WindowEvent};
use slint::{ComponentHandle, LogicalPosition, SharedString};

pub const SHOT_W: u32 = 1920;
pub const SHOT_H: u32 = 1080;

/// Windows and screenshots from memory. `fail` makes the next screenshots fail. `front` holds windows that
/// open later: the next list shows them first (a test fills it after the window is built). `live` is the live
/// view: the test pushes frames into its sink.
pub struct FakeDesktop {
    pub windows: Vec<WinInfo>,
    pub fail: Rc<Cell<bool>>,
    pub front: Rc<RefCell<Vec<WinInfo>>>,
    /// How often "Add voices" asked to open the Windows speech settings.
    pub opened_settings: Rc<Cell<u32>>,
    pub live: FakeLive,
}

/// The live view of the fake desktop: what was started and stopped, and the sink of the newest session.
#[derive(Clone, Default)]
pub struct FakeLive {
    pub sink: Rc<RefCell<Option<Box<dyn LiveSink>>>>,
    pub starts: Rc<Cell<u32>>,
    pub stops: Rc<Cell<u32>>,
    /// Target size of the newest session (physical px).
    pub target: Rc<Cell<(u32, u32)>>,
    /// The title of the window of the newest session.
    pub title: Rc<RefCell<String>>,
    /// The next starts fail.
    pub fail: Rc<Cell<bool>>,
    /// The windows are minimized, or closed.
    pub minimized: Rc<Cell<bool>>,
    pub closed: Rc<Cell<bool>>,
    /// The window searches wait here until the test answers them (`answer`), as the real one runs on another
    /// thread. Off: they answer at once.
    pub defer: Rc<Cell<bool>>,
    pub searches: Rc<RefCell<Vec<Search>>>,
}

/// A window search the test answers later: the list it will get, and where the answer goes.
pub type Search = (Vec<WinInfo>, Box<dyn FnOnce(Vec<WinInfo>) + Send>);

impl FakeLive {
    /// One frame of a `src_w` x `src_h` capture, made `w` x `h`, as the capture thread hands it over.
    pub fn frame(&self, src_w: u32, src_h: u32, w: u32, h: u32) {
        let rgba = [90u8, 120, 160, 255].repeat((w * h) as usize);
        if let Some(s) = self.sink.borrow_mut().as_mut() {
            s.frame(&LiveFrame {
                src_w,
                src_h,
                w,
                h,
                rgba: &rgba,
            });
        }
    }

    /// Answers the oldest waiting window search.
    pub fn answer(&self) {
        let s = self.searches.borrow_mut().remove(0);
        (s.1)(s.0);
    }

    pub fn end(&self, why: &str) {
        if let Some(s) = self.sink.borrow_mut().as_mut() {
            s.ended(why);
        }
    }
}

struct FakeRun {
    stops: Rc<Cell<u32>>,
    target: Rc<Cell<(u32, u32)>>,
}

impl LiveRun for FakeRun {
    fn set_target(&self, w: u32, h: u32) {
        self.target.set((w, h));
    }
}

impl Drop for FakeRun {
    fn drop(&mut self) {
        self.stops.set(self.stops.get() + 1);
    }
}

pub fn win(title: &str, exe: &str, game: bool) -> WinInfo {
    WinInfo {
        hwnd: 1,
        pid: 1,
        title: title.into(),
        exe: exe.into(),
        exe_path: None,
        width: SHOT_W,
        height: SHOT_H,
        mode: if game {
            WinMode::Borderless
        } else {
            WinMode::Windowed
        },
        minimized: false,
        game,
        icon: None,
    }
}

impl FakeDesktop {
    pub fn new() -> FakeDesktop {
        FakeDesktop {
            windows: vec![
                win("Test Game", "TestGame.exe", true),
                win("Notes", "notepad.exe", false),
            ],
            fail: Rc::new(Cell::new(false)),
            front: Rc::new(RefCell::new(Vec::new())),
            opened_settings: Rc::new(Cell::new(0)),
            live: FakeLive::default(),
        }
    }
}

impl Desktop for FakeDesktop {
    fn windows(&self, profile_exes: &[String]) -> Vec<WinInfo> {
        let mut v = self.front.borrow().clone();
        v.extend(self.windows.iter().cloned());
        for w in &mut v {
            if profile_exes.iter().any(|e| e.eq_ignore_ascii_case(&w.exe)) {
                w.game = true;
            }
        }
        v
    }

    fn open_speech_settings(&self) {
        self.opened_settings.set(self.opened_settings.get() + 1);
    }

    fn is_minimized(&self, w: &WinInfo) -> bool {
        w.minimized || self.live.minimized.get()
    }

    fn is_open(&self, _w: &WinInfo) -> bool {
        !self.live.closed.get()
    }

    fn windows_quick(&self, profile_exes: &[String]) -> Vec<WinInfo> {
        if self.live.closed.get() {
            return Vec::new();
        }
        self.windows(profile_exes)
    }

    fn windows_quick_then(
        &self,
        profile_exes: Vec<String>,
        done: Box<dyn FnOnce(Vec<WinInfo>) + Send>,
    ) {
        let list = self.windows_quick(&profile_exes);
        if self.live.defer.get() {
            self.live.searches.borrow_mut().push((list, done));
        } else {
            done(list);
        }
    }

    fn live(
        &self,
        w: &WinInfo,
        max_w: u32,
        max_h: u32,
        sink: Box<dyn LiveSink>,
    ) -> Result<Box<dyn LiveRun>, String> {
        if self.live.fail.get() {
            return Err("Windows could not capture this window".into());
        }
        self.live.starts.set(self.live.starts.get() + 1);
        self.live.target.set((max_w, max_h));
        *self.live.title.borrow_mut() = w.title.clone();
        *self.live.sink.borrow_mut() = Some(sink);
        Ok(Box::new(FakeRun {
            stops: self.live.stops.clone(),
            target: self.live.target.clone(),
        }))
    }

    fn screenshot(&self, w: &WinInfo) -> Result<Shot, String> {
        if self.fail.get() {
            return Err("the window gave no picture".into());
        }
        // a vertical gradient, so a broken picture is not one flat color
        let mut s = Shot::filled(w.width, w.height, [30, 40, 60, 255]);
        for y in 0..w.height {
            let v = (y * 255 / w.height) as u8;
            for x in (0..w.width).step_by(7) {
                let i = ((y * w.width + x) * 4) as usize;
                s.rgba[i] = v;
            }
        }
        Ok(s)
    }
}

pub fn temp_paths() -> (tempfile::TempDir, Paths) {
    let dir = tempfile::tempdir().unwrap();
    let p = Paths::at(dir.path().join("DialogReader"));
    (dir, p)
}

/// Two fake voices ("David", "Zira") and a speech thread with a fake engine and a null sink: nothing plays.
pub fn fake_voices() -> Voices {
    // Windows voices only (no Kokoro), through the same engine router as the app
    let engines = || dr_tts::Engines::new(None, Some(Box::new(dr_tts::FakeTts::new())));
    let mut v = Voices::of(&engines(), 2, None);
    v.speech = Some(Speech::start(
        Box::new(engines()),
        || Box::new(NullSink::timed(1.0)),
        VoiceSettings::default(),
    ));
    v
}

/// A window on a temp data folder and a fake desktop, at 1440 x 900, with no event loop.
pub fn real_window(paths: &Paths, desktop: FakeDesktop) -> (MainWindow, dr_app::vm::Shared) {
    real_window_with(paths, desktop, fake_voices())
}

/// [`real_window`] with these voices.
pub fn real_window_with(
    paths: &Paths,
    desktop: FakeDesktop,
    voices: Voices,
) -> (MainWindow, dr_app::vm::Shared) {
    thread_local!(static INIT: Cell<bool> = const { Cell::new(false) });
    // the testing platform can be set once per thread
    if !INIT.with(|i| i.replace(true)) {
        i_slint_backend_testing::init_no_event_loop();
    }
    let (w, vm) = dr_app::build_window(Source::Real {
        paths: paths.clone(),
        desktop: Box::new(desktop),
        voices: Box::new(voices),
    })
    .expect("window");
    w.window().set_size(slint::LogicalSize::new(1440.0, 900.0));
    w.show().expect("show");
    (w, vm)
}

/// Presses the element with this label and role (buttons, tabs, toggles).
pub fn press(w: &MainWindow, label: &str, role: AccessibleRole) {
    let hits: Vec<ElementHandle> = ElementHandle::find_by_accessible_label(w, label)
        .filter(|e| e.accessible_role() == Some(role))
        .collect();
    let e = hits
        .first()
        .unwrap_or_else(|| panic!("no {role:?} labelled {label:?}"));
    e.invoke_accessible_default_action();
}

pub fn key(w: &MainWindow, text: impl Into<SharedString>) {
    let text: SharedString = text.into();
    w.window()
        .dispatch_event(WindowEvent::KeyPressed { text: text.clone() });
    w.window().dispatch_event(WindowEvent::KeyReleased { text });
}

pub fn ctrl_key(w: &MainWindow, text: &str) {
    use slint::platform::Key;
    w.window().dispatch_event(WindowEvent::KeyPressed {
        text: Key::Control.into(),
    });
    key(w, text);
    w.window().dispatch_event(WindowEvent::KeyReleased {
        text: Key::Control.into(),
    });
}

/// The stage on screen: maps image pixels of a `img_w` x `img_h` screenshot to window positions ("contain").
pub struct StageAt {
    pub x: f32,
    pub y: f32,
    pub scale: f32,
}

pub fn stage(w: &MainWindow, img_w: f32, img_h: f32) -> StageAt {
    let st: Vec<ElementHandle> = ElementHandle::find_by_accessible_label(w, "Screenshot").collect();
    let st = st.first().expect("an interactive stage on this page");
    let (pos, size) = (st.absolute_position(), st.size());
    let scale = (size.width / img_w).min(size.height / img_h);
    StageAt {
        x: pos.x + (size.width - img_w * scale) / 2.0,
        y: pos.y + (size.height - img_h * scale) / 2.0,
        scale,
    }
}

impl StageAt {
    pub fn at(&self, ix: f32, iy: f32) -> LogicalPosition {
        LogicalPosition::new(self.x + ix * self.scale, self.y + iy * self.scale)
    }
}

/// A left-button drag from one image point to another, in steps.
pub fn drag(w: &MainWindow, s: &StageAt, from: (f32, f32), to: (f32, f32)) {
    let win = w.window();
    win.dispatch_event(WindowEvent::PointerMoved {
        position: s.at(from.0, from.1),
    });
    win.dispatch_event(WindowEvent::PointerPressed {
        position: s.at(from.0, from.1),
        button: PointerEventButton::Left,
    });
    for k in 1..=4 {
        let t = k as f32 / 4.0;
        let p = s.at(from.0 + (to.0 - from.0) * t, from.1 + (to.1 - from.1) * t);
        win.dispatch_event(WindowEvent::PointerMoved { position: p });
    }
    win.dispatch_event(WindowEvent::PointerReleased {
        position: s.at(to.0, to.1),
        button: PointerEventButton::Left,
    });
}

pub fn click(w: &MainWindow, s: &StageAt, at: (f32, f32)) {
    drag(w, s, at, at);
}
