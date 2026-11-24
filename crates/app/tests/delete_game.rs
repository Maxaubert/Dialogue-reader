//! Delete a game from the game switcher (Max, 2026-11-24) on a temp data folder: it asks first, Esc cancels,
//! and a confirmed delete removes the profile, its screenshot and its activity log. The current game makes way
//! for the next one; with no game left the app goes to Welcome.

mod common;

use common::*;
use dr_app::{MainWindow, Page};
use dr_runtime::{Paths, Profile, Shot, profile};
use i_slint_backend_testing::{AccessibleRole, ElementHandle};
use slint::Model;
use slint::platform::Key;

const IDS: [&str; 3] = ["alpha", "beta", "gamma"];

/// Three games with a screenshot and an activity log each.
fn saved(paths: &Paths) {
    for id in IDS {
        let name = format!("{}{} Game", id[..1].to_uppercase(), &id[1..]);
        profile::save(paths, &Profile::new(id, &name, &format!("{id}.exe"))).unwrap();
        Shot::filled(64, 36, [40, 50, 70, 255])
            .save_png(&paths.profile_png(id))
            .unwrap();
        let log = paths.activity_log(id);
        std::fs::create_dir_all(log.parent().unwrap()).unwrap();
        std::fs::write(&log, "").unwrap();
    }
}

fn names(w: &MainWindow) -> Vec<String> {
    w.get_games().iter().map(|g| g.name.to_string()).collect()
}

fn current(w: &MainWindow) -> String {
    names(w)[w.get_current_game() as usize].clone()
}

fn id_of(name: &str) -> String {
    name.split(' ').next().unwrap().to_lowercase()
}

fn files_of(paths: &Paths, id: &str) -> [bool; 3] {
    [
        paths.profile_json(id).exists(),
        paths.profile_png(id).exists(),
        paths.activity_log(id).exists(),
    ]
}

fn asks(w: &MainWindow, name: &str) -> bool {
    ElementHandle::find_by_accessible_label(w, &format!("Confirm delete {name}"))
        .any(|e| e.accessible_role() == Some(AccessibleRole::Button))
}

#[test]
fn delete_asks_first_and_esc_cancels() {
    let (_dir, paths) = temp_paths();
    saved(&paths);
    let (w, _vm) = real_window(&paths, FakeDesktop::new());
    assert_eq!(w.get_page(), Page::Overview);
    ctrl_key(&w, "k");
    assert!(w.get_switcher_open());
    // the highlight starts on the first game that is not the current one; its row has the trash button
    let other = names(&w)[1].clone();
    press(&w, &format!("Delete {other}"), AccessibleRole::Button);
    assert!(asks(&w, &other));
    assert_eq!(
        files_of(&paths, &id_of(&other)),
        [true; 3],
        "nothing gone yet"
    );
    // Esc cancels the ask only; the next Esc closes the switcher
    key(&w, Key::Escape);
    assert!(!asks(&w, &other));
    assert!(w.get_switcher_open());
    // the Delete key asks too; moving to another row cancels it
    key(&w, Key::Delete);
    assert!(asks(&w, &other));
    key(&w, Key::DownArrow);
    assert!(!asks(&w, &other));
    key(&w, Key::Escape);
    assert!(!w.get_switcher_open());
    assert_eq!(names(&w).len(), 3);
    for id in IDS {
        assert_eq!(files_of(&paths, id), [true; 3]);
    }
}

#[test]
fn deleting_games_down_to_none() {
    let (_dir, paths) = temp_paths();
    saved(&paths);
    let (w, vm) = real_window(&paths, FakeDesktop::new());
    let first = current(&w);

    // another game: it goes, with its files; the current game stays
    ctrl_key(&w, "k");
    let other = names(&w)[1].clone();
    key(&w, Key::Delete);
    key(&w, Key::Return);
    assert_eq!(files_of(&paths, &id_of(&other)), [false; 3]);
    assert_eq!(names(&w).len(), 2);
    assert!(!names(&w).contains(&other));
    assert_eq!(current(&w), first);
    assert!(w.get_switcher_open(), "the switcher stays open");
    assert!(
        profile::ids_on_disk(&paths)
            .iter()
            .all(|i| *i != id_of(&other))
    );

    // the current game (by mouse: trash, then Delete): the next one becomes current
    key(&w, Key::UpArrow);
    press(&w, &format!("Delete {first}"), AccessibleRole::Button);
    press(
        &w,
        &format!("Confirm delete {first}"),
        AccessibleRole::Button,
    );
    assert_eq!(files_of(&paths, &id_of(&first)), [false; 3]);
    let left = names(&w);
    assert_eq!(left.len(), 1);
    assert_eq!(current(&w), left[0]);
    assert_eq!(w.get_page(), Page::Overview);
    // late speech events of the deleted game do not make its log again
    vm.borrow_mut().poll_test();
    assert!(!paths.activity_log(&id_of(&first)).exists());

    // the last game: Welcome
    key(&w, Key::Delete);
    key(&w, Key::Return);
    assert!(names(&w).is_empty());
    assert!(!w.get_switcher_open());
    assert_eq!(w.get_page(), Page::Welcome);
    vm.borrow_mut().poll_test();
    for id in IDS {
        assert_eq!(files_of(&paths, id), [false; 3], "{id}");
    }
    assert!(profile::ids_on_disk(&paths).is_empty());
}
