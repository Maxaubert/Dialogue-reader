//! The in-game overlays in the view model (M4) on the Slint testing backend: no window shows on the desktop. A
//! runtime on fake parts reads a fake game; keys are fed as the hook hands them (`Runtime::feed_key`). Covers the
//! pill, the quick menu (open, keys, Esc, the voice list, Speed, Stop, Quit twice), holding reading while a menu is
//! open, and zone edit: a new zone is saved to the profile and the reader uses it at once.

mod common;

use std::sync::Arc;
use std::time::{Duration, Instant};

use common::{FakeDesktop, fake_voices, real_window_with, temp_paths};
use dr_app::overlay::Overlays;
use dr_app::overlay::quick_menu as qm;
use dr_app::vm::reading;
use dr_app::{Kind as UiKind, MainWindow, Page};
use dr_core::{Ocr, OcrLine, Rect};
use dr_runtime::capture::{FrameSource, Next, OwnedFrame};
use dr_runtime::keys::{Action, Nav, Overlay};
use dr_runtime::runtime::{Parts, Runtime};
use dr_runtime::{Kind, Profile, ZoneCfg, profile};

struct Still(bool, f64);

impl FrameSource for Still {
    fn next(&mut self, wait: Duration) -> Next {
        if !self.0 {
            self.0 = true;
            return Next::Frame(OwnedFrame {
                t_ms: 0.0,
                w: 640,
                h: 360,
                rgba: [40u8, 40, 40, 255].repeat(640 * 360),
            });
        }
        std::thread::sleep(wait);
        self.1 += wait.as_secs_f64() * 1000.0;
        Next::Same(self.1)
    }
}

/// Reads nothing, or the same line in every crop.
struct Text(Option<&'static str>);

impl Ocr for Text {
    fn recognize(&mut self, w: u32, h: u32, _rgba: &[u8]) -> Result<Vec<OcrLine>, String> {
        Ok(self
            .0
            .map(|t| OcrLine {
                text: t.into(),
                rect: Rect::new(2.0, 2.0, w as f32 - 4.0, h as f32 - 4.0),
            })
            .into_iter()
            .collect())
    }
}

struct App {
    w: MainWindow,
    vm: dr_app::vm::Shared,
    paths: dr_runtime::Paths,
    _dir: tempfile::TempDir,
}

fn app() -> App {
    app_reading(None)
}

/// The app on a game whose zones show `line` (or nothing).
fn app_reading(line: Option<&'static str>) -> App {
    app_on(line, FakeDesktop::new())
}

/// `app_reading` on this fake desktop.
fn app_on(line: Option<&'static str>, desktop: FakeDesktop) -> App {
    let (dir, paths) = temp_paths();
    let mut p = Profile::new("test-game", "Test Game", "TestGame.exe");
    p.zones.push(ZoneCfg {
        name: "Subtitles".into(),
        kind: Kind::Dialogue,
        on: true,
        rect: [40.0, 260.0, 560.0, 60.0],
        base: [640, 360],
    });
    p.voice.speed = 1.0;
    profile::save(&paths, &p).unwrap();
    let voices = fake_voices();
    let speech = voices.speech.as_ref().unwrap().sender();
    let (w, vm) = real_window_with(&paths, desktop, voices);
    let parts = Parts {
        ocr: Arc::new(move |_lang: &str| Ok(Box::new(Text(line)) as Box<dyn Ocr>)),
        source: Arc::new(|_hwnd| Ok((Box::new(Still(false, 0.0)) as Box<dyn FrameSource>, None))),
        hooks: false,
        checks: false,
        speech,
    };
    dr_app::vm::attach_runtime(&vm, Runtime::new(parts), None);
    dr_app::vm::attach_overlays(&vm, Overlays::new(false).expect("overlay windows"));
    let a = App {
        w,
        vm,
        paths,
        _dir: dir,
    };
    assert!(a.until(|a| a.w.get_status_text().as_str() == reading::STATUS_READING));
    a
}

impl App {
    fn until(&self, done: impl Fn(&App) -> bool) -> bool {
        let end = Instant::now() + Duration::from_secs(10);
        while Instant::now() < end {
            self.vm.borrow_mut().reading_tick();
            if done(self) {
                return true;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        done(self)
    }

    /// A key as the hook hands it: the overlay changes on the hook side, then the event is taken.
    fn key(&self, a: Action) {
        self.vm.borrow_mut().runtime_mut().unwrap().feed_key(a);
        self.vm.borrow_mut().reading_tick();
    }

    fn nav(&self, n: Nav) {
        self.key(Action::Nav(n));
    }

    fn shown(&self) -> Overlay {
        self.vm.borrow().overlays().unwrap().shown
    }

    fn pill_shown(&self) -> bool {
        self.vm.borrow().overlays().unwrap().pill_shown
    }

    fn hook(&self) -> Overlay {
        self.vm.borrow().runtime().unwrap().overlay()
    }

    fn held(&self) -> bool {
        self.vm.borrow().runtime().unwrap().is_held()
    }

    fn saved(&self) -> Profile {
        let (all, _) = profile::load_all(&self.paths);
        all.into_iter().find(|p| p.id == "test-game").unwrap()
    }

    fn menu(&self) -> dr_app::QuickMenuWindow {
        self.vm.borrow().overlays().unwrap().menu.clone_strong()
    }
}

use slint::ComponentHandle;

#[test]
fn the_pill_shows_while_the_game_is_read_and_hides_for_the_menu() {
    let a = app();
    assert!(a.pill_shown());
    {
        let vm = a.vm.borrow();
        let p = &vm.overlays().unwrap().pill;
        assert!(p.get_ok(), "green dot while reading");
        assert!(!p.get_speaking());
    }
    a.key(Action::Menu);
    assert_eq!(a.shown(), Overlay::Menu);
    assert!(!a.pill_shown(), "the pill hides while the menu is open");
    assert!(a.held(), "reading waits while the menu is open");
    assert!(
        !a.vm.borrow().runtime().unwrap().is_paused(),
        "not the user's Pause"
    );
    // Settings > Position > Off hides it
    a.nav(Nav::Esc);
    assert!(a.pill_shown());
    a.w.set_pill_position(2);
    a.vm.borrow_mut().reading_tick();
    assert!(!a.pill_shown());
}

#[test]
fn the_pill_wave_lights_while_a_line_is_spoken() {
    let a = app_reading(Some("Hello there, friend."));
    let speaking = |a: &App| a.vm.borrow().overlays().unwrap().pill.get_speaking();
    assert!(a.until(speaking), "the wave never lit");
    assert!(a.pill_shown());
    // the line has no speaker zone: no name shows
    assert_eq!(
        a.vm.borrow()
            .overlays()
            .unwrap()
            .pill
            .get_speaker()
            .as_str(),
        ""
    );
    // muted from the quick menu, the wave stays dark
    a.key(Action::Menu);
    for _ in 0..qm::MUTE {
        a.nav(Nav::Down);
    }
    a.nav(Nav::Enter);
    assert!(a.menu().get_muted());
    a.nav(Nav::Esc);
}

#[test]
fn the_menu_opens_moves_and_closes_with_esc() {
    let a = app();
    a.key(Action::Menu);
    assert_eq!(a.shown(), Overlay::Menu);
    let m = a.menu();
    assert_eq!(m.get_game().as_str(), "Test Game");
    assert_eq!(m.get_highlight(), 0);
    a.nav(Nav::Down);
    a.nav(Nav::Down);
    assert_eq!(m.get_highlight(), 2);
    a.nav(Nav::Up);
    a.nav(Nav::Up);
    a.nav(Nav::Up);
    assert_eq!(m.get_highlight(), qm::QUIT as i32, "wraps to Quit");
    // Esc closes it at once on the hook side, and the windows follow
    a.nav(Nav::Esc);
    assert_eq!(a.hook(), Overlay::None);
    assert_eq!(a.shown(), Overlay::None);
    assert!(!a.held());
    // the quick menu key opens it again at the top; pressed again it closes
    a.key(Action::Menu);
    assert_eq!(m.get_highlight(), 0);
    a.key(Action::Menu);
    assert_eq!(a.shown(), Overlay::None);
}

#[test]
fn the_menu_opens_only_over_the_game_in_front() {
    let a = app();
    a.vm.borrow_mut()
        .overlays_mut()
        .unwrap()
        .fake_game
        .as_mut()
        .unwrap()
        .front = false;
    a.key(Action::Menu);
    assert_eq!(a.shown(), Overlay::None);
    assert_eq!(
        a.hook(),
        Overlay::None,
        "the hook gives the keys back to the game"
    );
    assert!(!a.pill_shown(), "no pill over another app");
}

#[test]
fn the_menu_closes_when_the_game_loses_the_front() {
    let a = app();
    a.key(Action::Menu);
    assert_eq!(a.shown(), Overlay::Menu);
    a.vm.borrow_mut()
        .overlays_mut()
        .unwrap()
        .fake_game
        .as_mut()
        .unwrap()
        .front = false;
    a.vm.borrow_mut().reading_tick();
    assert_eq!(a.shown(), Overlay::None);
    assert_eq!(a.hook(), Overlay::None);
}

#[test]
fn speed_stop_and_mode_change_the_game() {
    let a = app();
    a.key(Action::Menu);
    for _ in 0..qm::SPEED {
        a.nav(Nav::Down);
    }
    a.nav(Nav::Right);
    assert_eq!(a.menu().get_speed(), 1.1);
    assert_eq!(a.saved().voice.speed, 1.1, "saved at once");
    // Stop (play/pause) is the user's Pause
    for _ in qm::STOP..qm::SPEED {
        a.nav(Nav::Up);
    }
    a.nav(Nav::Enter);
    assert!(a.vm.borrow().runtime().unwrap().is_paused());
    assert!(a.menu().get_stopped());
    // Mode: On key press
    for _ in qm::STOP..qm::MODE {
        a.nav(Nav::Down);
    }
    a.nav(Nav::Right);
    assert_eq!(a.saved().mode, dr_runtime::Mode::Manual);
    assert!(!a.w.get_automatic());
    // closing the menu keeps the user's Pause
    a.nav(Nav::Esc);
    assert!(a.vm.borrow().runtime().unwrap().is_paused());
    assert!(!a.held());
}

#[test]
fn esc_in_the_voice_list_goes_back_and_enter_picks_a_voice() {
    let a = app();
    let before = a.saved().voice.id;
    a.key(Action::Menu);
    for _ in 0..qm::VOICE {
        a.nav(Nav::Down);
    }
    a.nav(Nav::Right);
    let m = a.menu();
    assert!(m.get_voice_open());
    // Esc only closes the list: the hook was told so
    a.nav(Nav::Esc);
    assert_eq!(a.hook(), Overlay::Menu);
    assert_eq!(a.shown(), Overlay::Menu);
    assert!(!m.get_voice_open());
    a.nav(Nav::Right);
    a.nav(Nav::Down);
    a.nav(Nav::Enter);
    assert!(!m.get_voice_open());
    assert_eq!(m.get_voice().as_str(), "Zira");
    assert_ne!(a.saved().voice.id, before, "the chosen voice is saved");
}

#[test]
fn quit_needs_two_presses() {
    let a = app();
    a.key(Action::Menu);
    a.nav(Nav::Up);
    assert_eq!(a.menu().get_highlight(), qm::QUIT as i32);
    a.nav(Nav::Enter);
    assert!(a.menu().get_quit_armed(), "the first press arms it");
    assert_eq!(a.shown(), Overlay::Menu);
    a.nav(Nav::Down);
    assert!(!a.menu().get_quit_armed());
    a.nav(Nav::Up);
    a.nav(Nav::Enter);
    a.nav(Nav::Enter);
    // the app quits: every overlay is gone
    assert_eq!(a.shown(), Overlay::None);
    assert!(!a.pill_shown());
}

#[test]
fn settings_opens_the_main_window_on_settings() {
    let a = app();
    a.key(Action::Menu);
    for _ in 0..qm::SETTINGS {
        a.nav(Nav::Down);
    }
    a.nav(Nav::Enter);
    assert_eq!(a.shown(), Overlay::None);
    assert_eq!(a.w.get_page(), Page::Settings);
}

#[test]
fn zone_edit_saves_a_new_zone_and_the_reader_uses_it_at_once() {
    let a = app();
    a.key(Action::Menu);
    a.nav(Nav::Enter);
    assert_eq!(a.shown(), Overlay::Zones);
    assert_eq!(a.hook(), Overlay::Zones);
    assert!(a.held());
    // the frozen screenshot (1920 x 1080) is the profile's picture now; the old zone kept its place on it
    let p = a.saved();
    assert_eq!(p.zones.len(), 1);
    assert_eq!(p.zones[0].base, [1920, 1080]);
    let z = a.vm.borrow().overlays().unwrap().zones.clone_strong();
    use slint::Model;
    assert_eq!(z.get_zones().row_count(), 1);
    // draw a box and pick its kind (the window does the drawing; fake game: 1920 x 1080 at 100 %)
    z.invoke_commit(UiKind::Speaker, 100.0, 50.0, 300.0, 40.0);
    let p = a.saved();
    assert_eq!(p.zones.len(), 2);
    assert_eq!(p.zones[1].kind, Kind::Speaker);
    assert_eq!(p.zones[1].rect, [100.0, 50.0, 300.0, 40.0]);
    // the reader got it at once
    let zones = a.vm.borrow().runtime().unwrap().zones();
    assert_eq!(zones.len(), 2, "{zones:?}");
    assert!(
        zones
            .iter()
            .any(|z| z.rect == Rect::new(100.0, 50.0, 300.0, 40.0))
    );
    assert_eq!(z.get_zones().row_count(), 2);
    assert_eq!(z.get_selected(), 1);
    // the kind picker under the selected zone changes its kind, saved
    z.invoke_set_kind(UiKind::Pickups);
    assert_eq!(a.saved().zones[1].kind, Kind::Notification);
    assert_eq!(z.get_zones().row_data(1).unwrap().kind, UiKind::Pickups);
    // Delete removes the selected zone, saved
    a.nav(Nav::Delete);
    assert_eq!(a.saved().zones.len(), 1);
    assert_eq!(a.vm.borrow().runtime().unwrap().zones().len(), 1);
    // a click picks a zone (the old one, scaled to the new picture: x 120 to 1800, y 780 to 960)
    assert_eq!(z.invoke_pick(200.0, 800.0), 0);
    assert_eq!(z.invoke_pick(5.0, 5.0), -1);
    // Esc drops a box being drawn first, then finishes
    z.set_draft(true);
    a.nav(Nav::Esc);
    assert!(!z.get_draft());
    assert_eq!(a.shown(), Overlay::Zones);
    a.nav(Nav::Esc);
    assert_eq!(a.shown(), Overlay::None);
    assert_eq!(a.hook(), Overlay::None);
    assert!(!a.held());
    assert!(a.pill_shown());
}

/// The live view of the game failed: zone edit saves a new screenshot of the game as its picture, and the
/// Overview and Zones keep the failure note and show the time of the new screenshot.
#[test]
fn a_zone_edit_shot_keeps_the_live_failure_note_and_shows_its_time() {
    use dr_app::LiveView;
    use slint::ComponentHandle;
    let d = FakeDesktop::new();
    d.live.fail.set(true);
    let a = app_on(None, d);
    a.vm.borrow_mut().live_tick();
    let note = |a: &App| a.w.global::<LiveView>().get_note().to_string();
    assert_eq!(note(&a), dr_app::vm::NOTE_FAILED);
    let png = a.paths.profile_png("test-game");
    let before = std::fs::metadata(&png).and_then(|m| m.modified()).ok();
    a.key(Action::Menu);
    a.nav(Nav::Enter);
    assert_eq!(a.shown(), Overlay::Zones);
    // the new screenshot is saved
    assert_eq!(a.w.get_notice(), "");
    let after = std::fs::metadata(&png).and_then(|m| m.modified()).ok();
    assert!(after.is_some() && after != before, "{before:?} {after:?}");
    assert_eq!(note(&a), dr_app::vm::NOTE_FAILED);
    // "21:30": the time of the new screenshot (TEXT.md)
    let t = a.w.get_shot_time();
    let b = t.as_bytes();
    assert!(
        b.len() == 5
            && b.iter().enumerate().all(|(i, c)| if i == 2 {
                *c == b':'
            } else {
                c.is_ascii_digit()
            }),
        "{t}"
    );
    a.nav(Nav::Esc);
    a.w.set_page(Page::Zones);
    a.vm.borrow_mut().live_tick();
    assert_eq!(note(&a), dr_app::vm::NOTE_FAILED);
}

/// Zone edit with one new zone drawn at (100, 50) 300 x 40 and selected (fake game: 1920 x 1080 at 100 %, so window
/// px are capture px).
fn zone_edit_with_a_box() -> (App, dr_app::ZoneEditWindow) {
    let a = app();
    a.key(Action::Menu);
    a.nav(Nav::Enter);
    assert_eq!(a.shown(), Overlay::Zones);
    let z = a.vm.borrow().overlays().unwrap().zones.clone_strong();
    z.invoke_commit(UiKind::Speaker, 100.0, 50.0, 300.0, 40.0);
    assert_eq!(z.get_selected(), 1);
    (a, z)
}

fn edit_rect(z: &dr_app::ZoneEditWindow, i: usize) -> [f32; 4] {
    use slint::Model;
    let r = z.get_zones().row_data(i).unwrap();
    [r.x, r.y, r.w, r.h]
}

/// Max, 2026-11-24: the selected zone in zone edit has resize handles, as on Zones. A drag of a corner resizes it;
/// it is saved on the release and the reader uses it at once.
#[test]
fn zone_edit_resizes_the_selected_zone_by_a_corner() {
    let (a, z) = zone_edit_with_a_box();
    // the bottom right handle (4) follows the pointer; nothing is saved before the release
    assert!(z.invoke_resize_start(4));
    z.invoke_resize_move(500.0, 150.0);
    assert_eq!(edit_rect(&z, 1), [100.0, 50.0, 400.0, 100.0]);
    assert_eq!(a.saved().zones[1].rect, [100.0, 50.0, 300.0, 40.0]);
    z.invoke_resize_end(500.0, 150.0);
    assert_eq!(a.saved().zones[1].rect, [100.0, 50.0, 400.0, 100.0]);
    let zones = a.vm.borrow().runtime().unwrap().zones();
    assert!(
        zones
            .iter()
            .any(|z| z.rect == Rect::new(100.0, 50.0, 400.0, 100.0)),
        "{zones:?}"
    );
    // the top left handle (0) past the other corner: the zone keeps its smallest size
    assert!(z.invoke_resize_start(0));
    z.invoke_resize_end(900.0, 900.0);
    let [x, y, w, h] = a.saved().zones[1].rect;
    assert_eq!((x + w, y + h), (500.0, 150.0), "the other corner stays");
    assert!(
        w >= dr_app::geom::MIN_W - 0.01 && h >= dr_app::geom::MIN_H - 0.01,
        "{w} x {h}"
    );
    // no zone selected: nothing to resize
    assert_eq!(z.invoke_pick(5.0, 5.0), -1);
    assert!(!z.invoke_resize_start(4));
}

/// Esc during a resize puts the zone back and keeps zone edit open; the next Esc finishes.
#[test]
fn esc_during_a_zone_edit_resize_puts_the_zone_back() {
    let (a, z) = zone_edit_with_a_box();
    assert!(z.invoke_resize_start(4));
    z.set_resizing(true);
    z.invoke_resize_move(700.0, 300.0);
    assert_eq!(edit_rect(&z, 1), [100.0, 50.0, 600.0, 250.0]);
    // Delete does not remove a zone being resized
    a.nav(Nav::Delete);
    use slint::Model;
    assert_eq!(z.get_zones().row_count(), 2);
    a.nav(Nav::Esc);
    assert_eq!(a.shown(), Overlay::Zones, "zone edit stays open");
    assert!(!z.get_resizing());
    assert_eq!(edit_rect(&z, 1), [100.0, 50.0, 300.0, 40.0]);
    assert_eq!(a.saved().zones[1].rect, [100.0, 50.0, 300.0, 40.0]);
    // a late release changes nothing
    z.invoke_resize_end(700.0, 300.0);
    assert_eq!(a.saved().zones[1].rect, [100.0, 50.0, 300.0, 40.0]);
    a.nav(Nav::Esc);
    assert_eq!(a.shown(), Overlay::None);
}

/// The handles take real pointer events: a press on the bottom right handle, a move and a release resize the zone.
/// A right press during the drag cancels it, and a press away from the handles still draws.
#[test]
fn zone_edit_handles_take_the_pointer() {
    use slint::LogicalPosition;
    use slint::platform::{PointerEventButton, WindowEvent};
    let (a, z) = zone_edit_with_a_box();
    let win = z.window();
    let at = |x: f32, y: f32| LogicalPosition::new(x, y);
    let press = |p, b| {
        win.dispatch_event(WindowEvent::PointerPressed {
            position: p,
            button: b,
        })
    };
    let release = |p, b| {
        win.dispatch_event(WindowEvent::PointerReleased {
            position: p,
            button: b,
        })
    };
    let mv = |p| win.dispatch_event(WindowEvent::PointerMoved { position: p });
    // the box ends at (400, 90)
    mv(at(401.0, 91.0));
    press(at(401.0, 91.0), PointerEventButton::Left);
    assert!(z.get_resizing());
    mv(at(450.0, 120.0));
    assert_eq!(edit_rect(&z, 1), [100.0, 50.0, 350.0, 70.0]);
    release(at(450.0, 120.0), PointerEventButton::Left);
    assert!(!z.get_resizing());
    assert_eq!(a.saved().zones[1].rect, [100.0, 50.0, 350.0, 70.0]);
    assert_eq!(z.get_selected(), 1, "still selected");
    // a right press during the drag puts it back
    press(at(450.0, 120.0), PointerEventButton::Left);
    assert!(z.get_resizing());
    mv(at(600.0, 200.0));
    press(at(600.0, 200.0), PointerEventButton::Right);
    assert!(!z.get_resizing());
    assert_eq!(edit_rect(&z, 1), [100.0, 50.0, 350.0, 70.0]);
    release(at(600.0, 200.0), PointerEventButton::Left);
    assert_eq!(a.saved().zones[1].rect, [100.0, 50.0, 350.0, 70.0]);
    // a press away from the handles still draws a new box
    mv(at(800.0, 400.0));
    press(at(800.0, 400.0), PointerEventButton::Left);
    assert!(!z.get_resizing());
    mv(at(900.0, 480.0));
    release(at(900.0, 480.0), PointerEventButton::Left);
    assert!(z.get_draft());
}

/// Where Done is in zone edit (the bar's place).
fn done_at(z: &dr_app::ZoneEditWindow) -> (i32, i32) {
    slint::platform::update_timers_and_animations();
    let d = i_slint_backend_testing::ElementHandle::find_by_accessible_label(z, "Done")
        .find(|e| e.accessible_role() == Some(i_slint_backend_testing::AccessibleRole::Button))
        .expect("Done");
    let p = d.absolute_position();
    (p.x.round() as i32, p.y.round() as i32)
}

/// Max, 2026-11-24: the zone edit bar jumped back and forth while a zone sat at the edge of its home spot. It now
/// leaves a place when the selection comes within 8 px, and comes back only when the selection is 24 px clear.
#[test]
fn the_zone_edit_bar_does_not_jump_at_the_edge_of_home() {
    let a = app();
    a.key(Action::Menu);
    a.nav(Nav::Enter);
    let z = a.vm.borrow().overlays().unwrap().zones.clone_strong();
    // a box under the middle of the bar's home (top centre, y 24 to 72), its tag above it
    z.invoke_commit(UiKind::Speaker, 800.0, 200.0, 300.0, 40.0);
    let home = done_at(&z);
    let mut places = vec![home];
    let mut step = |y: f32| {
        z.invoke_resize_move(900.0, y);
        let at = done_at(&z);
        if places.last() != Some(&at) {
            places.push(at);
        }
    };
    // the top edge goes up 1 px at a time until the tag is under the bar, and back down
    assert!(z.invoke_resize_start(1));
    for y in (90..=200).rev() {
        step(y as f32);
    }
    // a box that sits at the edge (moving by a few px) does not move the bar
    for y in [110, 112, 109, 113, 110, 114, 108] {
        step(y as f32);
    }
    for y in 90..=200 {
        step(y as f32);
    }
    z.invoke_resize_end(900.0, 200.0);
    assert_eq!(places.len(), 3, "out once and home once: {places:?}");
    assert_ne!(places[1], home);
    assert_eq!(places[2], home);
}

#[test]
fn the_quick_menu_key_goes_from_zone_edit_back_to_the_menu() {
    let a = app();
    a.key(Action::Menu);
    a.nav(Nav::Enter);
    assert_eq!(a.shown(), Overlay::Zones);
    a.key(Action::Menu);
    assert_eq!(a.shown(), Overlay::Menu);
    assert_eq!(a.menu().get_highlight(), 0);
}

/// A slider of the open menu, "Speed" or "Volume" (its accessible value is its place in percent).
fn slider(m: &dr_app::QuickMenuWindow, label: &str) -> i_slint_backend_testing::ElementHandle {
    i_slint_backend_testing::ElementHandle::find_by_accessible_label(m, label)
        .find(|e| e.accessible_role() == Some(i_slint_backend_testing::AccessibleRole::Slider))
        .unwrap_or_else(|| panic!("no {label} slider"))
}

/// A Mode button of the open menu ("Automatic" or "On key press").
fn mode_button(m: &dr_app::QuickMenuWindow, label: &str) -> i_slint_backend_testing::ElementHandle {
    i_slint_backend_testing::ElementHandle::find_by_accessible_label(m, label)
        .find(|e| e.accessible_role() == Some(i_slint_backend_testing::AccessibleRole::Tab))
        .unwrap_or_else(|| panic!("no Mode button {label:?}"))
}

#[test]
fn volume_and_mode_follow_the_keys_after_a_mouse_click() {
    use slint::platform::PointerEventButton;
    let a = app();
    a.key(Action::Menu);
    let m = a.menu();
    // a click in the middle of the slider: 50 %, saved when let go
    slider(&m, "Volume").mock_single_click(PointerEventButton::Left);
    assert_eq!(
        slider(&m, "Volume").accessible_value().unwrap().as_str(),
        "50"
    );
    assert!((a.saved().voice.volume - 0.5).abs() < 0.01);
    // then Right on Volume: the slider shows the new value
    for _ in 0..qm::VOLUME {
        a.nav(Nav::Down);
    }
    a.nav(Nav::Right);
    assert!((a.saved().voice.volume - 0.55).abs() < 0.01);
    assert_eq!(
        slider(&m, "Volume").accessible_value().unwrap().as_str(),
        "55"
    );
    // a click on "On key press", then Left on Mode: "Automatic" shows as chosen
    mode_button(&m, "On key press").mock_single_click(PointerEventButton::Left);
    assert_eq!(a.saved().mode, dr_runtime::Mode::Manual);
    assert_eq!(
        mode_button(&m, "On key press").accessible_item_selected(),
        Some(true)
    );
    a.nav(Nav::Down);
    a.nav(Nav::Left);
    assert_eq!(a.saved().mode, dr_runtime::Mode::Auto);
    assert_eq!(
        mode_button(&m, "Automatic").accessible_item_selected(),
        Some(true)
    );
    // and on the next open, the menu shows what the profile has
    a.nav(Nav::Esc);
    a.key(Action::Menu);
    assert_eq!(
        slider(&m, "Volume").accessible_value().unwrap().as_str(),
        "55"
    );
    assert_eq!(
        mode_button(&m, "Automatic").accessible_item_selected(),
        Some(true)
    );
}

#[test]
fn the_speed_slider_follows_the_keys_and_the_mouse() {
    use slint::platform::PointerEventButton;
    let a = app();
    a.key(Action::Menu);
    let m = a.menu();
    // 1.0x on the 0.5x to 2.0x range: a third of the way
    assert_eq!(
        slider(&m, "Speed").accessible_value().unwrap().as_str(),
        "33"
    );
    // Right on Speed: 1.1x, the knob follows, saved at once
    for _ in 0..qm::SPEED {
        a.nav(Nav::Down);
    }
    a.nav(Nav::Right);
    assert_eq!(m.get_speed(), 1.1);
    assert_eq!(
        slider(&m, "Speed").accessible_value().unwrap().as_str(),
        "40"
    );
    assert_eq!(a.saved().voice.speed, 1.1);
    // a click in the middle: 1.25x snaps to the 0.1 steps of the keys, saved when let go
    slider(&m, "Speed").mock_single_click(PointerEventButton::Left);
    let s = m.get_speed();
    assert!((s - 1.3).abs() < 1e-4 || (s - 1.2).abs() < 1e-4, "{s}");
    assert_eq!(a.saved().voice.speed, s);
    // and Left after the click steps from there
    a.nav(Nav::Left);
    assert!((m.get_speed() - (s - 0.1)).abs() < 1e-4);
    assert!((a.saved().voice.speed - (s - 0.1)).abs() < 1e-4);
}

/// Moves the testing clock by `ms`, in small steps (timers and animations run as in a real window).
fn wait(ms: u64) {
    let mut left = ms;
    while left > 0 {
        let step = left.min(10);
        i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(step));
        left -= step;
    }
}

#[test]
fn a_hide_waits_for_the_fade_and_a_new_show_keeps_the_window() {
    use dr_app::overlay::place::{GameWin, PillAt};
    // the testing backend for this thread, then overlay windows with the real motion
    let _a = app();
    dr_app::set_instant_motion(false);
    dr_app::set_reduce_motion(Some(false));
    let mut o = Overlays::new(false).expect("overlay windows");
    dr_app::set_instant_motion(true);
    dr_app::set_reduce_motion(None);
    let g = GameWin::fake(1920, 1080);
    let shown = |w: &slint::Window| w.is_visible();
    o.pill_on(true, Some(&g), PillAt::TopRight);
    assert!(shown(o.pill.window()));
    // hidden: the window stays while it fades (200 ms), then goes
    o.pill_on(false, Some(&g), PillAt::TopRight);
    assert!(!o.pill_shown);
    assert!(shown(o.pill.window()), "gone before the fade");
    wait(100);
    assert!(shown(o.pill.window()));
    wait(300);
    assert!(!shown(o.pill.window()), "still there after the fade");
    // shown again while it fades: the hide that was waiting does not take it away
    o.pill_on(true, Some(&g), PillAt::TopRight);
    o.pill_on(false, Some(&g), PillAt::TopRight);
    wait(100);
    o.pill_on(true, Some(&g), PillAt::TopRight);
    wait(400);
    assert!(
        shown(o.pill.window()),
        "a new show was undone by the old hide"
    );
    // the menu and zone edit work the same way
    assert!(o.menu_on(&g));
    o.shown = Overlay::Menu;
    o.menu_off();
    assert!(shown(o.menu.window()));
    wait(300);
    assert!(!shown(o.menu.window()));
    assert!(o.zones_on(&g));
    o.zones_off();
    assert!(shown(o.zones.window()));
    wait(300);
    assert!(!shown(o.zones.window()));
}

#[test]
fn a_menu_action_does_not_reopen_a_menu_the_hook_closed() {
    let a = app();
    a.key(Action::Menu);
    assert_eq!(a.shown(), Overlay::Menu);
    // the quick menu key closes the menu on the hook thread while the UI is still working on a Speed change
    a.vm.borrow_mut()
        .runtime_mut()
        .unwrap()
        .feed_key(Action::Menu);
    assert_eq!(a.hook(), Overlay::None);
    a.vm.borrow_mut().menu_out(qm::Out::Speed(1.2));
    assert_eq!(a.hook(), Overlay::None, "the menu stays closed");
    assert!(a.until(|a| a.shown() == Overlay::None));
    assert!(a.until(|a| !a.held()));
}
