//! M2 done-when: a profile made through onboarding is saved, survives a restart, and its zones load as
//! `dr_core::Zone`s with the right size on a frame of another size. Runs on the Slint testing backend with a
//! temp data folder and a fake desktop: no window, no real %APPDATA%, no capture of the screen.

mod common;

use common::*;
use dr_app::Page;
use dr_runtime::profile;
use i_slint_backend_testing::{AccessibleRole, ElementHandle};
use slint::platform::WindowEvent;
use slint::{ComponentHandle, Model};

#[test]
fn onboarding_saves_a_profile_that_survives_a_restart() {
    let (_dir, paths) = temp_paths();
    let (w, vm) = real_window(&paths, FakeDesktop::new());
    // no profile yet: onboarding
    assert_eq!(w.get_page(), Page::Welcome);

    press(&w, "Get started", AccessibleRole::Button);
    assert_eq!(w.get_page(), Page::PickWindow);
    // the game first, the other window below
    assert_eq!(w.get_games_running().row_count(), 1);
    assert_eq!(w.get_games_running().row_data(0).unwrap().name, "Test Game");
    assert_eq!(
        w.get_games_running().row_data(0).unwrap().meta,
        "1920 × 1080 · borderless"
    );
    assert_eq!(w.get_other_windows().row_count(), 1);
    assert_eq!(w.get_picked_window(), 0);

    // Continue takes the screenshot
    press(&w, "Continue", AccessibleRole::Button);
    assert_eq!(w.get_page(), Page::DrawZones);
    assert_eq!(
        (w.get_setup_shot_w(), w.get_setup_shot_h()),
        (SHOT_W as f32, SHOT_H as f32)
    );
    assert!(w.get_setup_shot().size().width == SHOT_W);
    assert_eq!(w.get_setup_zones().row_count(), 0);

    // draw two zones, make the second a speaker zone
    let s = stage(&w, SHOT_W as f32, SHOT_H as f32);
    drag(&w, &s, (200.0, 900.0), (1700.0, 1000.0));
    assert_eq!(w.get_setup_zones().row_count(), 1);
    press(&w, "Speaker", AccessibleRole::Tab);
    drag(&w, &s, (800.0, 840.0), (1100.0, 890.0));
    assert_eq!(w.get_setup_zones().row_count(), 2);
    let z1 = w.get_setup_zones().row_data(1).unwrap();
    assert_eq!(z1.kind, dr_app::Kind::Speaker);
    assert_eq!(z1.name, "Speaker");

    for expected in [Page::Keys, Page::Voice, Page::Mode] {
        press(&w, "Continue", AccessibleRole::Button);
        assert_eq!(w.get_page(), expected);
    }
    press(&w, "On key press", AccessibleRole::RadioButton);
    press(&w, "Finish", AccessibleRole::Button);
    assert_eq!(w.get_page(), Page::Overview);
    assert_eq!(w.get_notice(), "");

    // saved: JSON and screenshot
    let json = paths.profile_json("test-game");
    assert!(json.exists(), "{} missing", json.display());
    assert!(paths.profile_png("test-game").exists());
    let p = profile::load(&json).unwrap();
    assert_eq!(p.name, "Test Game");
    assert_eq!(p.matches.exe, "TestGame.exe");
    assert_eq!(p.mode, dr_runtime::Mode::Manual);
    assert_eq!(p.keys.read, "Mouse 4");
    assert_eq!(p.zones.len(), 2);
    assert_eq!(p.zones[0].base, [SHOT_W, SHOT_H]);
    let [x, y, zw, zh] = p.zones[0].rect;
    for (a, b) in [(x, 200.0), (y, 900.0), (zw, 1500.0), (zh, 100.0)] {
        assert!((a - b).abs() < 2.0, "zone rect {:?}", p.zones[0].rect);
    }
    assert_eq!(p.zones[1].kind, dr_runtime::Kind::Speaker);

    // restart: a new window on the same folder opens the profile, not onboarding
    drop(w);
    drop(vm);
    let (w2, _vm2) = real_window(&paths, FakeDesktop::new());
    assert_eq!(w2.get_page(), Page::Overview);
    assert_eq!(w2.get_games().row_count(), 1);
    assert_eq!(w2.get_games().row_data(0).unwrap().name, "Test Game");
    assert!(w2.get_games().row_data(0).unwrap().running);
    assert_eq!(w2.get_zones().row_count(), 2);
    assert_eq!(w2.get_shot_w(), SHOT_W as f32);
    assert!(!w2.get_automatic(), "the mode comes from the profile");
    let z0 = w2.get_zones().row_data(0).unwrap();
    assert!((z0.x - 200.0).abs() < 2.0 && (z0.w - 1500.0).abs() < 2.0);

    // the zones as dr-core gets them, on a 4K frame: twice the size
    let p = profile::load(&json).unwrap();
    let core = p.core_zones();
    assert_eq!(core.len(), 2);
    let r = core[0].rect_in(3840, 2160);
    assert!(
        (r.x - 400.0).abs() < 4.0 && (r.y - 1800.0).abs() < 4.0,
        "{r:?}"
    );
    assert!(
        (r.w - 3000.0).abs() < 4.0 && (r.h - 200.0).abs() < 4.0,
        "{r:?}"
    );
    assert_eq!(core[1].kind, dr_core::ZoneKind::Speaker);
}

#[test]
fn failed_screenshot_keeps_page_02_and_says_why() {
    let (_dir, paths) = temp_paths();
    let desktop = FakeDesktop::new();
    let fail = desktop.fail.clone();
    let (w, _vm) = real_window(&paths, desktop);
    press(&w, "Get started", AccessibleRole::Button);
    fail.set(true);
    press(&w, "Continue", AccessibleRole::Button);
    assert_eq!(w.get_page(), Page::PickWindow);
    assert!(
        w.get_notice().contains("No screenshot of Test Game"),
        "{}",
        w.get_notice()
    );
    fail.set(false);
    press(&w, "Continue", AccessibleRole::Button);
    assert_eq!(w.get_page(), Page::DrawZones);
    assert_eq!(w.get_notice(), "");
}

#[test]
fn search_and_other_windows_can_be_picked() {
    let (_dir, paths) = temp_paths();
    let (w, _vm) = real_window(&paths, FakeDesktop::new());
    press(&w, "Get started", AccessibleRole::Button);
    // an other window is picked by its row
    press(&w, "Notes", AccessibleRole::Button);
    assert_eq!(w.get_picked_window(), 1);
    w.invoke_windows_search("note".into());
    assert_eq!(w.get_games_running().row_count(), 0);
    assert_eq!(w.get_other_windows().row_count(), 1);
    press(&w, "Continue", AccessibleRole::Button);
    assert_eq!(w.get_page(), Page::DrawZones);
}

/// Onboarding up to Finish with one zone drawn.
fn finish_onboarding(w: &dr_app::MainWindow) {
    press(w, "Get started", AccessibleRole::Button);
    press(w, "Continue", AccessibleRole::Button);
    assert_eq!(w.get_page(), Page::DrawZones);
    let s = stage(w, SHOT_W as f32, SHOT_H as f32);
    drag(w, &s, (200.0, 900.0), (1700.0, 1000.0));
    for _ in 0..3 {
        press(w, "Continue", AccessibleRole::Button);
    }
    press(w, "Finish", AccessibleRole::Button);
    assert_eq!(w.get_page(), Page::Overview);
}

#[test]
fn a_file_that_was_not_loaded_is_never_overwritten() {
    let (_dir, paths) = temp_paths();
    std::fs::create_dir_all(paths.profiles()).unwrap();
    // made by a newer DialogReader, with the id the new game would get
    let newer = r#"{"version": 7, "id": "test-game", "name": "Test Game"}"#;
    std::fs::write(paths.profile_json("Test-Game"), newer).unwrap();
    let (w, _vm) = real_window(&paths, FakeDesktop::new());
    assert!(w.get_notice().contains("newer"), "{}", w.get_notice());
    finish_onboarding(&w);
    assert_eq!(
        std::fs::read_to_string(paths.profile_json("test-game")).unwrap(),
        newer
    );
    let p = profile::load(&paths.profile_json("test-game-2")).unwrap();
    assert_eq!(p.id, "test-game-2");
    assert_eq!(p.zones.len(), 1);
}

#[test]
fn add_game_can_go_back_and_refuses_a_second_profile_for_the_same_window() {
    let (_dir, paths) = temp_paths();
    let (w, _vm) = real_window(&paths, FakeDesktop::new());
    finish_onboarding(&w);
    // Add game from the switcher, then Back on page 02: the main pages again, the game unchanged
    w.invoke_switcher_opened();
    w.invoke_add_game();
    assert_eq!(w.get_page(), Page::PickWindow);
    press(&w, "Back", AccessibleRole::Button);
    assert_eq!(w.get_page(), Page::Overview);
    assert_eq!(w.get_games().row_count(), 1);
    // the running game already has a profile: Continue says so and stays
    w.invoke_add_game();
    assert_eq!(w.get_picked_window(), 0);
    press(&w, "Continue", AccessibleRole::Button);
    assert_eq!(w.get_page(), Page::PickWindow);
    assert!(
        w.get_notice().contains("Test Game already has a profile"),
        "{}",
        w.get_notice()
    );
    // another window can still be set up; its page 03 starts with the Dialogue kind
    w.set_zone_kind(dr_app::Kind::Ignore);
    press(&w, "Notes", AccessibleRole::Button);
    press(&w, "Continue", AccessibleRole::Button);
    assert_eq!(w.get_page(), Page::DrawZones);
    assert_eq!(w.get_zone_kind(), dr_app::Kind::Dialogue);
}

#[test]
fn games_that_share_an_exe_get_one_profile_each() {
    let (_dir, paths) = temp_paths();
    let mut d = FakeDesktop::new();
    d.windows = vec![win("Emu - Game A", "emu.exe", true)];
    let front = d.front.clone();
    let (w, _vm) = real_window(&paths, d);
    finish_onboarding(&w);
    let a = profile::load(&paths.profile_json("emu-game-a")).unwrap();
    assert_eq!(
        a.matches.title_contains, None,
        "the first game of an exe matches by exe"
    );

    // a second game in the same emulator: allowed, and saved with its window title
    front
        .borrow_mut()
        .push(win("Emu - Game B", "emu.exe", true));
    w.invoke_add_game();
    press(&w, "Emu - Game B", AccessibleRole::Button);
    press(&w, "Continue", AccessibleRole::Button);
    assert_eq!(w.get_page(), Page::DrawZones, "{}", w.get_notice());
    for _ in 0..3 {
        press(&w, "Continue", AccessibleRole::Button);
    }
    press(&w, "Finish", AccessibleRole::Button);
    let b = profile::load(&paths.profile_json("emu-game-b")).unwrap();
    assert_eq!(b.matches.exe, "emu.exe");
    assert_eq!(b.matches.title_contains.as_deref(), Some("Emu - Game B"));
    // the first game gets its title too (its name, the title at its setup): it no longer takes every window
    let a = profile::load(&paths.profile_json("emu-game-a")).unwrap();
    assert_eq!(a.matches.title_contains.as_deref(), Some("Emu - Game A"));
    let running = |w: &dr_app::MainWindow, name: &str| {
        w.get_games()
            .iter()
            .find(|g| g.name == name)
            .unwrap_or_else(|| panic!("no game {name}"))
            .running
    };
    // both windows run: each game is running
    assert!(running(&w, "Emu - Game A") && running(&w, "Emu - Game B"));

    // only game B runs: game A is not running, although its exe-only match fits that window
    drop(w);
    let mut d = FakeDesktop::new();
    d.windows = vec![win("Emu - Game B", "emu.exe", true)];
    let (w, _vm) = real_window(&paths, d);
    assert!(running(&w, "Emu - Game B"));
    assert!(!running(&w, "Emu - Game A"));
    // game B's window is refused as a duplicate
    w.invoke_add_game();
    press(&w, "Continue", AccessibleRole::Button);
    assert!(
        w.get_notice()
            .contains("Emu - Game B already has a profile"),
        "{}",
        w.get_notice()
    );
}

#[test]
fn an_unreadable_exe_is_matched_by_title_only() {
    let (_dir, paths) = temp_paths();
    let mut d = FakeDesktop::new();
    // a protected game (no exe) and another unreadable window
    d.windows = vec![win("Hidden Game", "", true), win("Other", "", false)];
    let (w, _vm) = real_window(&paths, d);
    finish_onboarding(&w);
    let p = profile::load(&paths.profile_json("hidden-game")).unwrap();
    assert_eq!(p.matches.exe, "");
    assert_eq!(p.matches.title_contains.as_deref(), Some("Hidden Game"));
    assert!(w.get_games().row_data(0).unwrap().running);

    // only the unreadable "Other" window runs now: the game is not running
    drop(w);
    let mut d = FakeDesktop::new();
    d.windows = vec![win("Other", "", false)];
    let (w, _vm) = real_window(&paths, d);
    assert!(!w.get_games().row_data(0).unwrap().running);
    w.invoke_retake_shot();
    assert!(
        w.get_notice().contains("Hidden Game is not running"),
        "{}",
        w.get_notice()
    );
}

#[test]
fn a_minimized_window_is_refused_at_once() {
    let (_dir, paths) = temp_paths();
    let mut d = FakeDesktop::new();
    d.windows[0].minimized = true;
    let (w, _vm) = real_window(&paths, d);
    press(&w, "Get started", AccessibleRole::Button);
    press(&w, "Continue", AccessibleRole::Button);
    assert_eq!(w.get_page(), Page::PickWindow);
    assert!(w.get_notice().contains("minimized"), "{}", w.get_notice());
}

#[test]
fn nothing_is_picked_when_no_game_runs() {
    let (_dir, paths) = temp_paths();
    let mut d = FakeDesktop::new();
    d.windows.remove(0);
    let front = d.front.clone();
    let (w, _vm) = real_window(&paths, d);
    press(&w, "Get started", AccessibleRole::Button);
    assert_eq!(w.get_picked_window(), -1);
    press(&w, "Continue", AccessibleRole::Button);
    assert!(w.get_notice().contains("Pick a window first"));
    // a picked window stays picked through Refresh, also when a new window opens in front of it
    press(&w, "Notes", AccessibleRole::Button);
    assert_eq!(w.get_picked_window(), 0);
    front
        .borrow_mut()
        .push(win("Calculator", "calc.exe", false));
    press(&w, "Refresh", AccessibleRole::Button);
    assert_eq!(w.get_other_windows().row_count(), 2);
    let picked = w.get_picked_window();
    assert_eq!(picked, 1);
    assert_eq!(
        w.get_other_windows()
            .row_data(picked as usize)
            .unwrap()
            .name,
        "Notes"
    );
}

#[test]
fn page_02_wraps_games_and_scrolls_a_long_list() {
    let (_dir, paths) = temp_paths();
    let mut d = FakeDesktop::new();
    d.windows = (1..=5)
        .map(|i| win(&format!("Game {i}"), &format!("g{i}.exe"), true))
        .chain((1..=30).map(|i| win(&format!("Window {i}"), &format!("w{i}.exe"), false)))
        .collect();
    let (w, _vm) = real_window(&paths, d);
    press(&w, "Get started", AccessibleRole::Button);
    let at = |label: &str| {
        let e: Vec<ElementHandle> = ElementHandle::find_by_accessible_label(&w, label)
            .filter(|e| e.accessible_role() == Some(AccessibleRole::Button))
            .collect();
        e.first()
            .unwrap_or_else(|| panic!("no row {label}"))
            .absolute_position()
    };
    // three games to a row: game 4 starts a second row under game 1
    let (g1, g3, g4) = (at("Game 1"), at("Game 3"), at("Game 4"));
    assert_eq!(g1.y, g3.y);
    assert!(g4.y > g1.y && (g4.x - g1.x).abs() < 1.0, "{g1:?} {g4:?}");
    // the last rows are below the window (the testing backend finds only rows on screen); a wheel turn
    // scrolls them into view
    let shown = |label: &str| {
        ElementHandle::find_by_accessible_label(&w, label)
            .any(|e| e.accessible_role() == Some(AccessibleRole::Button))
    };
    assert!(shown("Window 1"));
    assert!(!shown("Window 30"));
    w.window().dispatch_event(WindowEvent::PointerScrolled {
        position: slint::LogicalPosition::new(720.0, 500.0),
        delta_x: 0.0,
        delta_y: -4000.0,
    });
    assert!(shown("Window 30"));
    let last = at("Window 30");
    assert!(last.y > 0.0 && last.y < 900.0, "{last:?}");
    assert!(!shown("Game 1"));
}

#[test]
fn voice_speed_and_volume_are_saved() {
    let (_dir, paths) = temp_paths();
    let (w, _vm) = real_window(&paths, FakeDesktop::new());
    w.set_voice_speed(0.6);
    w.set_voice_volume(0.5);
    w.set_picked_voice(1);
    finish_onboarding(&w);
    let p = profile::load(&paths.profile_json("test-game")).unwrap();
    assert!((p.voice.speed - 1.2).abs() < 1e-4, "{}", p.voice.speed);
    assert!((p.voice.volume - 0.5).abs() < 1e-4);
    assert_eq!(
        (p.voice.engine.as_str(), p.voice.id.as_str()),
        ("windows", "fake-zira")
    );
}

#[test]
fn broken_profile_is_kept_and_reported() {
    let (_dir, paths) = temp_paths();
    std::fs::create_dir_all(paths.profiles()).unwrap();
    std::fs::write(paths.profile_json("old"), "{ broken").unwrap();
    let (w, _vm) = real_window(&paths, FakeDesktop::new());
    assert_eq!(w.get_page(), Page::Welcome);
    assert!(w.get_notice().contains("old.json"), "{}", w.get_notice());
    assert!(paths.profiles().join("old.json.bad").exists());
}
