//! Shared test helpers: a fake desktop (windows and screenshots without touching the real desktop), a temp
//! data folder, and pointer and key input through the Slint testing backend. Nothing shows on screen.

#![allow(dead_code)]

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use dr_app::MainWindow;
use dr_app::vm::voices::Voices;
use dr_app::vm::{Desktop, Source};
use dr_runtime::speech::{NullSink, Speech, VoiceSettings};
use dr_runtime::windows_list::{WinInfo, WinMode};
use dr_runtime::{Paths, Shot};
use i_slint_backend_testing::{AccessibleRole, ElementHandle};
use slint::platform::{PointerEventButton, WindowEvent};
use slint::{ComponentHandle, LogicalPosition, SharedString};

pub const SHOT_W: u32 = 1920;
pub const SHOT_H: u32 = 1080;

/// Windows and screenshots from memory. `fail` makes the next screenshots fail. `front` holds windows that
/// open later: the next list shows them first (a test fills it after the window is built).
pub struct FakeDesktop {
    pub windows: Vec<WinInfo>,
    pub fail: Rc<Cell<bool>>,
    pub front: Rc<RefCell<Vec<WinInfo>>>,
    /// How often "Add voices" asked to open the Windows speech settings.
    pub opened_settings: Rc<Cell<u32>>,
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
    let tts = dr_tts::FakeTts::new();
    let list = dr_tts::Tts::voices(&tts);
    let speech = Speech::start(
        Box::new(tts),
        || Box::new(NullSink::timed(1.0)),
        VoiceSettings::default(),
    );
    Voices {
        list,
        problem: None,
        speech: Some(speech),
        log: None,
    }
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
        voices,
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
