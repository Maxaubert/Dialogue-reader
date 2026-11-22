//! The live view on the stages (src/vm/live.rs) with a fake desktop: it starts on pages with a stage and stops
//! on the others, frames replace the saved screenshot, zones follow a game that changed size, and a minimized,
//! closed or uncapturable game shows the saved screenshot with a note. The Slint testing backend has no event
//! loop, so each test hands frames over itself (`live_arrived`) and runs the check (`live_tick`).

mod common;

use common::*;
use dr_app::vm::{NOTE_CLOSED, NOTE_FAILED, NOTE_LIVE, NOTE_MINIMIZED};
use dr_app::{LiveView, MainWindow, Page};
use dr_runtime::{Paths, Profile, Shot, ZoneCfg, profile};
use i_slint_backend_testing::AccessibleRole;
use slint::{ComponentHandle, Model};

fn saved(paths: &Paths) {
    let mut p = Profile::new("test-game", "Test Game", "TestGame.exe");
    p.zones = vec![ZoneCfg {
        name: "Subtitles".into(),
        kind: dr_runtime::Kind::Dialogue,
        on: true,
        rect: [200.0, 900.0, 1500.0, 100.0],
        base: [SHOT_W, SHOT_H],
    }];
    profile::save(paths, &p).unwrap();
    Shot::filled(SHOT_W, SHOT_H, [40, 50, 70, 255])
        .save_png(&paths.profile_png("test-game"))
        .unwrap();
}

fn on_disk(paths: &Paths) -> Profile {
    profile::load(&paths.profile_json("test-game")).unwrap()
}

fn rect(w: &MainWindow, i: usize) -> [f32; 4] {
    let z = w.get_zones().row_data(i).unwrap();
    [z.x, z.y, z.w, z.h]
}

fn near(a: [f32; 4], b: [f32; 4]) -> bool {
    a.iter().zip(b).all(|(x, y)| (x - y).abs() < 2.0)
}

struct Setup {
    _dir: tempfile::TempDir,
    paths: Paths,
    w: MainWindow,
    vm: dr_app::vm::Shared,
    live: FakeLive,
}

fn open() -> Setup {
    open_with(|_| {})
}

/// `before` sets up the fake live view before the window opens (Overview starts the live view at once).
fn open_with(before: impl FnOnce(&FakeLive)) -> Setup {
    let (dir, paths) = temp_paths();
    saved(&paths);
    let d = FakeDesktop::new();
    let live = d.live.clone();
    before(&live);
    let (w, vm) = real_window(&paths, d);
    Setup {
        _dir: dir,
        paths,
        w,
        vm,
        live,
    }
}

impl Setup {
    fn tick(&self) {
        self.vm.borrow_mut().live_tick();
    }
    fn arrived(&self) {
        self.vm.borrow_mut().live_arrived();
    }
    fn on(&self) -> bool {
        self.w.global::<LiveView>().get_on()
    }
    fn note(&self) -> String {
        self.w.global::<LiveView>().get_note().into()
    }
}

#[test]
fn runs_on_pages_with_a_stage_and_stops_on_the_others() {
    let s = open();
    assert_eq!(s.w.get_page(), Page::Overview);
    s.tick();
    assert_eq!(s.live.starts.get(), 1, "Overview has a stage");
    assert_eq!(*s.live.title.borrow(), "Test Game");
    assert!(!s.on(), "the saved screenshot shows until the first frame");

    s.live.frame(SHOT_W, SHOT_H, 640, 360);
    s.arrived();
    assert!(s.on());
    assert!(s.vm.borrow().live_showing());
    assert_eq!(s.note(), NOTE_LIVE);
    // the "Live" chip: small, in the top right corner of the stage
    let st =
        i_slint_backend_testing::ElementHandle::find_by_accessible_label(&s.w, "Screenshot").next();
    let chip = i_slint_backend_testing::ElementHandle::find_by_accessible_label(&s.w, "Live view")
        .next()
        .expect("a chip on the stage");
    assert_eq!(chip.accessible_value().unwrap_or_default(), NOTE_LIVE);
    let (cp, cs) = (chip.absolute_position(), chip.size());
    assert!(
        cs.width > 20.0 && cs.width < 120.0,
        "chip width {}",
        cs.width
    );
    if let Some(st) = st {
        // Overview's stage is not interactive; the chip sits near the right edge of whichever stage shows
        let (sp, ss) = (st.absolute_position(), st.size());
        assert!((sp.x + ss.width - (cp.x + cs.width) - 12.0).abs() < 2.0);
    }
    let g = s.w.global::<LiveView>();
    assert_eq!(g.get_frame().size().width, 640);
    assert_eq!((g.get_w(), g.get_h()), (SHOT_W as f32, SHOT_H as f32));

    // Zones keeps the same session
    press(&s.w, "Zones", AccessibleRole::Tab);
    s.tick();
    assert_eq!((s.live.starts.get(), s.live.stops.get()), (1, 0));
    assert_eq!(s.w.get_shot_info(), "Live · 1920 × 1080 · Fit to window");

    // Settings has no stage: it stops, and the saved screenshot is back
    press(&s.w, "Settings", AccessibleRole::Tab);
    s.tick();
    assert_eq!(s.live.stops.get(), 1);
    assert!(!s.vm.borrow().live_running());
    assert!(!s.on());
    assert_eq!(s.note(), "");
    // a frame of the stopped session that was still on its way is dropped
    s.live.frame(SHOT_W, SHOT_H, 640, 360);
    s.arrived();
    assert!(!s.on());

    press(&s.w, "Zones", AccessibleRole::Tab);
    s.tick();
    assert_eq!(s.live.starts.get(), 2);
    assert_eq!(
        s.w.get_shot_info(),
        "Screenshot · 1920 × 1080 · Fit to window"
    );
}

#[test]
fn the_page_change_alone_starts_it() {
    // no tick by hand: the `changed page` callback runs the check
    let s = open();
    press(&s.w, "Settings", AccessibleRole::Tab);
    slint::platform::update_timers_and_animations();
    s.tick();
    assert!(!s.vm.borrow().live_running());
    press(&s.w, "Zones", AccessibleRole::Tab);
    slint::platform::update_timers_and_animations();
    assert!(s.vm.borrow().live_running());
}

#[test]
fn a_hidden_or_minimized_app_stops_it() {
    let s = open();
    s.tick();
    assert!(s.vm.borrow().live_running());
    s.w.hide().unwrap();
    s.tick();
    assert!(!s.vm.borrow().live_running(), "hidden to the tray");
    s.w.show().unwrap();
    s.tick();
    assert!(s.vm.borrow().live_running());
    s.w.window().set_minimized(true);
    assert!(s.w.window().is_minimized());
    s.tick();
    assert!(!s.vm.borrow().live_running(), "minimized");
    s.w.window().set_minimized(false);
    s.tick();
    assert!(s.vm.borrow().live_running());
}

#[test]
fn zones_follow_a_game_that_changed_size() {
    let s = open();
    press(&s.w, "Zones", AccessibleRole::Tab);
    s.tick();
    // the game now runs at 3840 x 2160: zones keep their place on the picture, in its pixels
    s.live.frame(3840, 2160, 1280, 720);
    s.arrived();
    assert!(
        near(rect(&s.w, 0), [400.0, 1800.0, 3000.0, 200.0]),
        "{:?}",
        rect(&s.w, 0)
    );
    assert_eq!(s.w.get_shot_info(), "Live · 3840 × 2160 · Fit to window");
    // the size change alone is not saved
    assert_eq!(on_disk(&s.paths).zones[0].base, [SHOT_W, SHOT_H]);

    // a drag on the live picture: stage coordinates map to capture pixels
    press(&s.w, "Draw zone", AccessibleRole::Tab);
    let st = stage(&s.w, 3840.0, 2160.0);
    drag(&s.w, &st, (200.0, 200.0), (1000.0, 600.0));
    assert_eq!(s.w.get_zones().row_count(), 2);
    let p = on_disk(&s.paths);
    assert_eq!(p.zones[1].base, [3840, 2160], "saved in capture pixels");
    assert!(
        near(p.zones[1].rect, [200.0, 200.0, 800.0, 400.0]),
        "{:?}",
        p.zones[1].rect
    );

    // the live view stops: back to the saved 1920 x 1080 screenshot, zones with it
    press(&s.w, "Settings", AccessibleRole::Tab);
    s.tick();
    press(&s.w, "Zones", AccessibleRole::Tab);
    assert!(
        near(rect(&s.w, 0), [200.0, 900.0, 1500.0, 100.0]),
        "{:?}",
        rect(&s.w, 0)
    );
    assert!(
        near(rect(&s.w, 1), [100.0, 100.0, 400.0, 200.0]),
        "{:?}",
        rect(&s.w, 1)
    );
}

#[test]
fn a_size_change_waits_for_the_end_of_a_drag() {
    let s = open();
    press(&s.w, "Zones", AccessibleRole::Tab);
    s.tick();
    s.live.frame(SHOT_W, SHOT_H, 640, 360);
    s.arrived();
    press(&s.w, "Draw zone", AccessibleRole::Tab);
    let st = stage(&s.w, SHOT_W as f32, SHOT_H as f32);
    let win = s.w.window();
    use slint::platform::{PointerEventButton, WindowEvent};
    win.dispatch_event(WindowEvent::PointerPressed {
        position: st.at(100.0, 100.0),
        button: PointerEventButton::Left,
    });
    win.dispatch_event(WindowEvent::PointerMoved {
        position: st.at(300.0, 200.0),
    });
    s.live.frame(3840, 2160, 1280, 720);
    s.arrived();
    assert_eq!(
        s.w.global::<LiveView>().get_w(),
        SHOT_W as f32,
        "the old picture stays"
    );
    win.dispatch_event(WindowEvent::PointerReleased {
        position: st.at(500.0, 300.0),
        button: PointerEventButton::Left,
    });
    let p = on_disk(&s.paths);
    assert!(
        near(p.zones[1].rect, [100.0, 100.0, 400.0, 200.0]),
        "{:?}",
        p.zones[1].rect
    );
    assert_eq!(p.zones[1].base, [SHOT_W, SHOT_H]);
    // the frame that waited shows when the drag ends, with no further frame (a paused game sends none)
    assert_eq!(s.w.global::<LiveView>().get_w(), 3840.0);
    assert!(near(rect(&s.w, 1), [200.0, 200.0, 800.0, 400.0]));
}

#[test]
fn a_size_change_during_a_cancelled_drag_shows_after_the_cancel() {
    let s = open();
    press(&s.w, "Zones", AccessibleRole::Tab);
    s.tick();
    s.live.frame(SHOT_W, SHOT_H, 640, 360);
    s.arrived();
    press(&s.w, "Draw zone", AccessibleRole::Tab);
    let st = stage(&s.w, SHOT_W as f32, SHOT_H as f32);
    let win = s.w.window();
    use slint::platform::{PointerEventButton, WindowEvent};
    win.dispatch_event(WindowEvent::PointerPressed {
        position: st.at(100.0, 100.0),
        button: PointerEventButton::Left,
    });
    win.dispatch_event(WindowEvent::PointerMoved {
        position: st.at(300.0, 200.0),
    });
    s.live.frame(2560, 1440, 1280, 720);
    s.arrived();
    assert_eq!(s.w.global::<LiveView>().get_w(), SHOT_W as f32);
    key(&s.w, slint::platform::Key::Escape);
    assert_eq!(s.w.global::<LiveView>().get_w(), 2560.0);
    assert_eq!(s.w.get_zones().row_count(), 1);
}

#[test]
fn a_minimized_game_shows_the_saved_screenshot_with_a_note() {
    let s = open_with(|l| l.minimized.set(true));
    s.tick();
    assert!(!s.vm.borrow().live_running());
    assert!(!s.on());
    assert_eq!(s.note(), NOTE_MINIMIZED);
    // restored: the next try (a few seconds later) starts it; a page change tries at once
    s.live.minimized.set(false);
    press(&s.w, "Settings", AccessibleRole::Tab);
    s.tick();
    press(&s.w, "Zones", AccessibleRole::Tab);
    s.tick();
    assert!(s.vm.borrow().live_running());
    assert_eq!(s.note(), "", "no note until a frame comes");
}

#[test]
fn a_closed_game_shows_the_saved_screenshot_with_a_note() {
    let s = open();
    press(&s.w, "Zones", AccessibleRole::Tab);
    s.tick();
    s.live.frame(3840, 2160, 1280, 720);
    s.arrived();
    assert!(s.on());
    // the capture ends: the window closed
    s.live.closed.set(true);
    s.live.end("the window closed");
    s.arrived();
    assert!(!s.on());
    assert!(!s.vm.borrow().live_running());
    assert_eq!(s.note(), NOTE_CLOSED);
    assert!(
        near(rect(&s.w, 0), [200.0, 900.0, 1500.0, 100.0]),
        "zones back on the screenshot"
    );
    assert_eq!(
        s.w.get_shot_info(),
        "Screenshot · 1920 × 1080 · Fit to window"
    );

    // not running when the page opens
    press(&s.w, "Settings", AccessibleRole::Tab);
    s.tick();
    press(&s.w, "Zones", AccessibleRole::Tab);
    s.tick();
    assert!(!s.vm.borrow().live_running());
    assert_eq!(s.note(), NOTE_CLOSED);
}

#[test]
fn a_capture_that_fails_shows_the_saved_screenshot_with_a_note() {
    let s = open_with(|l| l.fail.set(true));
    s.tick();
    assert!(!s.vm.borrow().live_running());
    assert_eq!(s.note(), NOTE_FAILED);
    assert!(!s.on());
}

#[test]
fn an_end_while_the_game_is_still_open_is_a_failure_not_a_close() {
    // a start Windows refused (it comes back as an end, the start runs on another thread) or a frame it could
    // not read: the window is still there, so the note does not say it closed
    let s = open();
    s.tick();
    s.live.frame(SHOT_W, SHOT_H, 640, 360);
    s.arrived();
    s.live.end("could not read the captured frame");
    s.arrived();
    assert!(!s.on());
    assert!(!s.vm.borrow().live_running());
    assert_eq!(s.note(), NOTE_FAILED);
    assert_eq!(
        s.w.global::<LiveView>().get_frame().size().width,
        0,
        "the last frame is let go"
    );
    // the retry waits a few seconds; the note stays meanwhile
    s.tick();
    assert!(!s.vm.borrow().live_running());
    assert_eq!(s.note(), NOTE_FAILED);
    // a page change tries at once, and a frame brings the live picture back
    press(&s.w, "Settings", AccessibleRole::Tab);
    s.tick();
    press(&s.w, "Overview", AccessibleRole::Tab);
    s.tick();
    assert!(s.vm.borrow().live_running());
    s.live.frame(SHOT_W, SHOT_H, 640, 360);
    s.arrived();
    assert_eq!(s.note(), NOTE_LIVE);
}

#[test]
fn the_game_window_is_searched_off_the_ui_thread() {
    // the real window list takes about 10 ms: the check asks for it and goes on; the answer starts the view
    let s = open_with(|l| l.defer.set(true));
    s.tick();
    assert!(!s.vm.borrow().live_running(), "no start before the answer");
    assert_eq!(s.live.searches.borrow().len(), 1);
    s.tick();
    assert_eq!(s.live.searches.borrow().len(), 1, "one search at a time");
    assert_eq!(s.note(), "", "no note while it searches");
    s.live.answer();
    s.tick();
    assert!(s.vm.borrow().live_running());
    assert_eq!(*s.live.title.borrow(), "Test Game");

    // an answer for the page before is not used
    press(&s.w, "Settings", AccessibleRole::Tab);
    s.tick();
    press(&s.w, "Zones", AccessibleRole::Tab);
    s.tick();
    assert!(!s.vm.borrow().live_running());
    assert_eq!(s.live.searches.borrow().len(), 1);
    press(&s.w, "Settings", AccessibleRole::Tab);
    s.tick();
    s.live.answer();
    press(&s.w, "Zones", AccessibleRole::Tab);
    s.tick();
    assert!(!s.vm.borrow().live_running(), "the old answer is dropped");
    assert_eq!(s.live.searches.borrow().len(), 1, "a new search runs");
    s.live.answer();
    s.tick();
    assert!(s.vm.borrow().live_running());

    // a closed game: the answer has no window, and the note says so
    press(&s.w, "Settings", AccessibleRole::Tab);
    s.tick();
    s.live.closed.set(true);
    press(&s.w, "Zones", AccessibleRole::Tab);
    s.tick();
    s.live.answer();
    s.tick();
    assert!(!s.vm.borrow().live_running());
    assert_eq!(s.note(), NOTE_CLOSED);
}

#[test]
fn the_frames_fit_the_stage() {
    let s = open();
    press(&s.w, "Zones", AccessibleRole::Tab);
    // the stage reported its size when it was built (logical px times the scale factor, 1 here)
    let st = i_slint_backend_testing::ElementHandle::find_by_accessible_label(&s.w, "Screenshot")
        .next()
        .unwrap();
    let size = st.size();
    s.tick();
    let (tw, th) = s.live.target.get();
    assert_eq!(
        (tw, th),
        (size.width.round() as u32, size.height.round() as u32)
    );
    // a bigger window: a bigger stage, a new target for the running session
    s.w.window()
        .set_size(slint::LogicalSize::new(1800.0, 1100.0));
    slint::platform::update_timers_and_animations();
    let size2 = st.size();
    assert!(size2.width > size.width);
    s.vm.borrow_mut().live_stage_size(size2.width, size2.height);
    assert_eq!(s.live.target.get().0, size2.width.round() as u32);
    // another monitor at 225 %: the next check gives the session a target in its physical pixels
    s.w.window()
        .dispatch_event(slint::platform::WindowEvent::ScaleFactorChanged { scale_factor: 2.25 });
    s.tick();
    let (tw, _) = s.live.target.get();
    assert!(
        (tw as f32 - size2.width * 2.25).abs() < 2.0,
        "{tw} for {} logical px",
        size2.width
    );
}

#[test]
fn setup_page_shows_the_picked_window_live() {
    let (_dir, paths) = temp_paths();
    let d = FakeDesktop::new();
    let live = d.live.clone();
    let (w, vm) = real_window(&paths, d);
    press(&w, "Get started", AccessibleRole::Button);
    press(&w, "Continue", AccessibleRole::Button);
    assert_eq!(w.get_page(), Page::DrawZones);
    vm.borrow_mut().live_tick();
    assert!(vm.borrow().live_running());
    assert_eq!(*live.title.borrow(), "Test Game");
    live.frame(2560, 1440, 872, 490);
    vm.borrow_mut().live_arrived();
    assert!(w.global::<LiveView>().get_on());
    // a zone drawn on the live picture is in its pixels
    let st = stage(&w, 2560.0, 1440.0);
    drag(&w, &st, (200.0, 1200.0), (2300.0, 1350.0));
    let z = w.get_setup_zones().row_data(0).unwrap();
    assert!(
        near([z.x, z.y, z.w, z.h], [200.0, 1200.0, 2100.0, 150.0]),
        "{z:?}"
    );
    // Continue to the keys page: no stage, it stops; the zone goes back onto the 1920 x 1080 screenshot
    press(&w, "Continue", AccessibleRole::Button);
    vm.borrow_mut().live_tick();
    assert!(!vm.borrow().live_running());
    let z = w.get_setup_zones().row_data(0).unwrap();
    assert!(
        near([z.x, z.y, z.w, z.h], [150.0, 900.0, 1575.0, 112.5]),
        "{z:?}"
    );
}

#[test]
fn a_game_minimized_while_live_shows_the_saved_screenshot_until_it_is_back() {
    let s = open();
    press(&s.w, "Zones", AccessibleRole::Tab);
    s.tick();
    s.live.frame(SHOT_W, SHOT_H, 640, 360);
    s.arrived();
    assert!(s.on());
    // a game that does not change sends no frames: that alone keeps the live picture
    std::thread::sleep(std::time::Duration::from_millis(1600));
    s.tick();
    assert!(s.on());
    // minimized: no frames and the window says so
    s.live.minimized.set(true);
    s.tick();
    assert!(!s.on());
    assert_eq!(s.note(), NOTE_MINIMIZED);
    assert!(
        s.vm.borrow().live_running(),
        "the session waits for the game"
    );
    // restored: frames come again
    s.live.minimized.set(false);
    s.live.frame(SHOT_W, SHOT_H, 640, 360);
    s.arrived();
    assert!(s.on());
    assert_eq!(s.note(), NOTE_LIVE);
}
