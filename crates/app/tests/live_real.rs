//! The whole live view path with a real capture: Windows Graphics Capture of a test window left of every monitor
//! (it repaints all the time) -> `dr_runtime::live` -> the app's sink -> `invoke_from_event_loop` -> `LiveView` on
//! the stage, with the game's window searched on another thread. Runs the Slint testing backend WITH an event loop (its own test binary: the platform is set once per
//! process). Nothing shows on the user's screen.

#![cfg(windows)]

use std::cell::Cell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use dr_app::vm::{Desktop, LiveRun, Source};
use dr_app::{LiveView, Page};
use dr_runtime::live::LiveSink;
use dr_runtime::testwin::{Kind, TestWindow};
use dr_runtime::windows_list::{WinInfo, WinMode};
use dr_runtime::{Paths, Profile, Shot, ZoneCfg, profile};
use slint::ComponentHandle;

/// One game: the test window.
struct OneWindow(WinInfo);

impl Desktop for OneWindow {
    fn windows(&self, _exes: &[String]) -> Vec<WinInfo> {
        vec![self.0.clone()]
    }
    fn screenshot(&self, w: &WinInfo) -> Result<Shot, String> {
        dr_runtime::screenshot::capture(w.hwnd)
    }
    fn is_minimized(&self, w: &WinInfo) -> bool {
        dr_runtime::windows_list::is_minimized(w.hwnd)
    }
    fn is_open(&self, w: &WinInfo) -> bool {
        dr_runtime::windows_list::is_open(w.hwnd)
    }
    // the answer comes from another thread, as on the real desktop
    fn windows_quick_then(&self, _exes: Vec<String>, done: Box<dyn FnOnce(Vec<WinInfo>) + Send>) {
        let list = vec![self.0.clone()];
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(30));
            done(list);
        });
    }
    fn live(
        &self,
        w: &WinInfo,
        max_w: u32,
        max_h: u32,
        sink: Box<dyn LiveSink>,
    ) -> Result<Box<dyn LiveRun>, String> {
        dr_app::vm::RealDesktop.live(w, max_w, max_h, sink)
    }
}

#[test]
fn real_frames_reach_the_stage_and_stop_with_the_page() {
    i_slint_backend_testing::init_integration_test_with_system_time();
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::at(dir.path().join("DialogReader"));
    let mut p = Profile::new("test-game", "Test Game", "TestGame.exe");
    p.zones = vec![ZoneCfg {
        name: "Subtitles".into(),
        kind: dr_runtime::Kind::Dialogue,
        on: true,
        rect: [100.0, 300.0, 400.0, 40.0],
        base: [640, 360],
    }];
    profile::save(&paths, &p).unwrap();
    // the saved screenshot is smaller than the game now runs
    Shot::filled(320, 180, [40, 50, 70, 255])
        .save_png(&paths.profile_png("test-game"))
        .unwrap();

    let game = TestWindow::spawn(
        Kind::OffscreenMoving {
            w: 640,
            h: 360,
            every_ms: 20,
        },
        None,
    )
    .unwrap();
    let info = WinInfo {
        hwnd: game.hwnd,
        pid: std::process::id(),
        title: "Test Game".into(),
        exe: "TestGame.exe".into(),
        exe_path: None,
        width: 640,
        height: 360,
        mode: WinMode::Windowed,
        minimized: false,
        game: true,
        icon: None,
    };
    let (w, vm) = dr_app::build_window(Source::Real {
        paths: paths.clone(),
        desktop: Box::new(OneWindow(info)),
        // no voices and no speech thread: nothing can play
        voices: dr_app::vm::voices::Voices::none(),
    })
    .unwrap();
    w.window().set_size(slint::LogicalSize::new(1440.0, 900.0));
    w.show().unwrap();
    w.set_page(Page::Zones);

    // what the loop saw: frame size on the stage, then whether it stopped
    let seen: Rc<Cell<Option<(f32, f32, u32)>>> = Rc::default();
    let stopped = Rc::new(Cell::new(false));
    let start = Instant::now();
    let timer = slint::Timer::default();
    {
        let (w, vm, seen, stopped) = (w.as_weak(), vm.clone(), seen.clone(), stopped.clone());
        timer.start(
            slint::TimerMode::Repeated,
            Duration::from_millis(50),
            move || {
                let Some(w) = w.upgrade() else { return };
                let g = w.global::<LiveView>();
                if seen.get().is_none() && g.get_on() {
                    seen.set(Some((g.get_w(), g.get_h(), g.get_frame().size().width)));
                    w.set_page(Page::Settings);
                } else if seen.get().is_some() && !vm.borrow().live_running() {
                    stopped.set(true);
                    let _ = slint::quit_event_loop();
                }
                if start.elapsed() > Duration::from_secs(10) {
                    let _ = slint::quit_event_loop();
                }
            },
        );
    }
    slint::run_event_loop().unwrap();
    drop(timer);

    let (lw, lh, fw) = seen.get().expect("no live frame reached the stage in 10 s");
    assert_eq!(
        (lw, lh),
        (640.0, 360.0),
        "the stage maps zones with the capture size"
    );
    assert!(fw > 0 && fw <= 640, "frame width {fw}");
    assert!(stopped.get(), "the live view did not stop on Settings");
    assert!(!w.global::<LiveView>().get_on());
    drop(vm);
    drop(w);
    drop(game);
}
