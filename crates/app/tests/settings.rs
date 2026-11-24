//! Settings (M5) through the real view model on a temp data folder and a fake desktop: the Startup and In-game
//! overlay rows are saved to settings.json and read back, Start with Windows goes to the desktop (never the real
//! registry here), Check now opens the releases page, and a game is exported and imported again. Nothing shows on
//! screen, no dialog opens.

mod common;

use common::*;
use dr_app::Page;
use dr_runtime::settings::{self, PillAt};
use dr_runtime::{Profile, profile};
use i_slint_backend_testing::{AccessibleRole, ElementHandle};
use slint::Model;

fn one_game(paths: &dr_runtime::Paths) {
    let mut p = Profile::new("test-game", "Test Game", "TestGame.exe");
    p.zones = vec![dr_runtime::ZoneCfg {
        name: "Subtitles".into(),
        kind: dr_runtime::Kind::Dialogue,
        on: true,
        rect: [100.0, 800.0, 1200.0, 120.0],
        base: [SHOT_W, SHOT_H],
    }];
    profile::save(paths, &p).unwrap();
}

fn toggle(w: &dr_app::MainWindow, label: &str) {
    let t: Vec<ElementHandle> = ElementHandle::find_by_accessible_label(w, label)
        .filter(|e| e.accessible_role() == Some(AccessibleRole::Checkbox))
        .collect();
    assert_eq!(t.len(), 1, "one toggle {label}");
    t[0].invoke_accessible_default_action();
}

#[test]
fn settings_are_saved_and_read_back() {
    let (_d, paths) = temp_paths();
    one_game(&paths);
    let desk = FakeDesktop::new();
    let shell = desk.shell.clone();
    let (w, vm) = real_window(&paths, desk);
    w.set_page(Page::Settings);
    // the spec defaults, not the mockup's
    assert!(!w.get_start_with_windows());
    assert!(w.get_open_minimized());
    assert!(w.get_show_in_taskbar());
    assert_eq!(w.get_version().as_str(), env!("CARGO_PKG_VERSION"));
    assert!(!w.get_show_auto_update());
    assert_eq!(w.get_update_note().as_str(), dr_app::vm::prefs::UPDATE_NOTE);

    toggle(&w, "Start with Windows");
    assert!(
        shell.autostart.get(),
        "Start with Windows went to the desktop"
    );
    toggle(&w, "Open minimized");
    toggle(&w, "Show in taskbar");
    toggle(&w, "Show speaker name");
    w.set_pill_position(1);
    w.invoke_settings_changed();

    let saved = settings::load(&paths).settings;
    assert!(saved.start_with_windows);
    assert!(!saved.open_minimized);
    assert!(!saved.show_in_taskbar);
    assert!(!saved.show_speaker);
    assert_eq!(saved.pill_position, PillAt::BottomCenter);
    assert_eq!(vm.borrow().prefs(), &saved);
    drop((w, vm));

    // a new start shows them
    let desk = FakeDesktop::new();
    desk.shell.autostart.set(true);
    let (w, _vm) = real_window(&paths, desk);
    assert!(w.get_start_with_windows());
    assert!(!w.get_open_minimized());
    assert!(!w.get_show_in_taskbar());
    assert!(!w.get_show_speaker());
    assert_eq!(w.get_pill_position(), 1);
}

#[test]
fn the_registry_decides_start_with_windows() {
    let (_d, paths) = temp_paths();
    one_game(&paths);
    // settings.json says on, but the Run value is gone (removed by hand, or by an uninstall)
    let s = settings::Settings {
        start_with_windows: true,
        ..Default::default()
    };
    settings::save(&paths, &s).unwrap();
    let desk = FakeDesktop::new();
    let (w, _vm) = real_window(&paths, desk);
    assert!(!w.get_start_with_windows());
}

#[test]
fn a_failed_start_with_windows_change_is_undone_and_said() {
    let (_d, paths) = temp_paths();
    one_game(&paths);
    let desk = FakeDesktop::new();
    let shell = desk.shell.clone();
    shell.autostart_fail.set(true);
    let (w, _vm) = real_window(&paths, desk);
    w.set_page(Page::Settings);
    toggle(&w, "Start with Windows");
    assert!(!w.get_start_with_windows(), "the toggle goes back");
    assert!(!shell.autostart.get());
    assert!(
        w.get_notice()
            .contains("Start with Windows could not be changed"),
        "{}",
        w.get_notice()
    );
}

#[test]
fn a_broken_settings_file_is_said_and_kept() {
    let (_d, paths) = temp_paths();
    one_game(&paths);
    std::fs::create_dir_all(paths.root()).unwrap();
    std::fs::write(paths.settings(), "{ broken").unwrap();
    let (w, _vm) = real_window(&paths, FakeDesktop::new());
    assert!(
        w.get_notice().contains("settings.json.bad"),
        "{}",
        w.get_notice()
    );
    assert!(paths.root().join("settings.json.bad").exists());
}

#[test]
fn a_change_that_cannot_be_saved_is_said() {
    let (_d, paths) = temp_paths();
    one_game(&paths);
    std::fs::create_dir_all(paths.root()).unwrap();
    // from a newer DialogReader: left alone
    let newer = "{\"version\": 99, \"show_speaker\": false}";
    std::fs::write(paths.settings(), newer).unwrap();
    let (w, _vm) = real_window(&paths, FakeDesktop::new());
    w.set_page(Page::Settings);
    toggle(&w, "Show speaker name");
    assert_eq!(
        w.get_notice().as_str(),
        dr_app::vm::prefs::NOT_SAVED,
        "the toggle must not look saved"
    );
    assert_eq!(std::fs::read_to_string(paths.settings()).unwrap(), newer);
}

#[test]
fn a_file_too_big_to_be_a_game_is_refused() {
    let (d, paths) = temp_paths();
    one_game(&paths);
    let (_w, vm) = real_window(&paths, FakeDesktop::new());
    let big = d.path().join("big.json");
    std::fs::write(&big, vec![b' '; 2 * 1024 * 1024]).unwrap();
    let e = vm.borrow_mut().import_from(&big).unwrap_err();
    assert!(e.contains("too big"), "{e}");
}

#[test]
fn check_now_opens_the_releases_page() {
    let (_d, paths) = temp_paths();
    one_game(&paths);
    let desk = FakeDesktop::new();
    let shell = desk.shell.clone();
    let (w, _vm) = real_window(&paths, desk);
    w.set_page(Page::Settings);
    w.invoke_settings_jump(4);
    press(&w, "Check now", AccessibleRole::Button);
    assert_eq!(
        shell.urls.borrow().as_slice(),
        [dr_app::vm::prefs::RELEASES_URL.to_string()]
    );
}

#[test]
fn a_game_is_exported_and_imported_again() {
    let (dir, paths) = temp_paths();
    one_game(&paths);
    let desk = FakeDesktop::new();
    let shell = desk.shell.clone();
    let (w, vm) = real_window(&paths, desk);
    w.set_page(Page::Settings);
    w.invoke_settings_jump(5);
    press(&w, "Export", AccessibleRole::Button);
    assert_eq!(
        shell.picks.borrow().as_slice(),
        [Some("Test Game.json".to_string())]
    );
    let file = dir.path().join("exported").join("Test Game.json");
    w.invoke_file_picked(true, file.to_string_lossy().as_ref().into());
    assert!(
        w.get_notice().contains("was exported"),
        "{}",
        w.get_notice()
    );
    let exported = Profile::from_json(&std::fs::read_to_string(&file).unwrap()).unwrap();
    assert_eq!(exported.name, "Test Game");
    assert_eq!(exported.zones.len(), 1);

    w.set_page(Page::Settings);
    w.invoke_settings_jump(5);
    press(&w, "Import", AccessibleRole::Button);
    assert_eq!(shell.picks.borrow().len(), 2);
    assert_eq!(shell.picks.borrow()[1], None);
    w.invoke_file_picked(false, file.to_string_lossy().as_ref().into());
    assert_eq!(w.get_notice().as_str(), "");
    assert_eq!(w.get_page(), Page::Overview);
    // a second game under a new id; the first one is not overwritten
    assert_eq!(w.get_games().row_count(), 2);
    let ids = profile::ids_on_disk(&paths);
    assert_eq!(ids.len(), 2, "{ids:?}");
    assert!(ids.iter().any(|i| i == "test-game"));
    let new_id = ids.iter().find(|i| *i != "test-game").unwrap();
    let imported = profile::load(&paths.profile_json(new_id)).unwrap();
    assert_eq!(imported.zones, exported.zones);
    assert!(!vm.borrow().prefs().start_with_windows);
}

#[test]
fn import_on_the_welcome_page_adds_the_first_game() {
    let (dir, paths) = temp_paths();
    let file = dir.path().join("game.json");
    let mut p = Profile::new("whatever", "Imported Game", "Imported.exe");
    p.zones = Vec::new();
    std::fs::write(&file, p.to_json()).unwrap();
    let desk = FakeDesktop::new();
    let shell = desk.shell.clone();
    let (w, _vm) = real_window(&paths, desk);
    assert_eq!(w.get_page(), Page::Welcome);
    press(&w, "Import settings", AccessibleRole::Button);
    assert_eq!(shell.picks.borrow().as_slice(), [None]);
    w.invoke_file_picked(false, file.to_string_lossy().as_ref().into());
    assert_eq!(w.get_page(), Page::Overview);
    assert_eq!(
        profile::ids_on_disk(&paths),
        vec!["imported-game".to_string()]
    );
}

#[test]
fn a_file_that_is_not_a_game_is_refused() {
    let (dir, paths) = temp_paths();
    one_game(&paths);
    let file = dir.path().join("notes.json");
    std::fs::write(&file, "[1, 2, 3]").unwrap();
    let (w, _vm) = real_window(&paths, FakeDesktop::new());
    w.invoke_file_picked(false, file.to_string_lossy().as_ref().into());
    assert!(
        w.get_notice().contains("not a DialogReader game"),
        "{}",
        w.get_notice()
    );
    assert_eq!(profile::ids_on_disk(&paths), vec!["test-game".to_string()]);
}
