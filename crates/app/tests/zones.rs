//! Zone drawing on the Zones tab (11) through simulated pointer events on the Slint testing backend: drag to
//! add, click to select, drag a corner, Delete, kind change, rename, undo. Each change is saved to the profile
//! in a temp data folder.

mod common;

use common::*;
use dr_app::{Kind, Page};
use dr_runtime::{Paths, Profile, Shot, ZoneCfg, profile};
use i_slint_backend_testing::{AccessibleRole, ElementHandle};
use slint::platform::{PointerEventButton, WindowEvent};
use slint::{ComponentHandle, Model};

/// A saved profile for "Test Game" with one dialogue zone, and its 1920 x 1080 screenshot.
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

fn near(a: [f32; 4], b: [f32; 4]) -> bool {
    a.iter().zip(b).all(|(x, y)| (x - y).abs() < 2.0)
}

#[test]
fn draw_select_resize_delete_and_kind() {
    let (_dir, paths) = temp_paths();
    saved(&paths);
    let (w, _vm) = real_window(&paths, FakeDesktop::new());
    assert_eq!(w.get_page(), Page::Overview);
    go_zones(&w);
    assert_eq!(w.get_page(), Page::Zones);
    assert_eq!(w.get_zones().row_count(), 1);
    let s = stage(&w, SHOT_W as f32, SHOT_H as f32);

    // drag to add (draw tool)
    press(&w, "Draw zone", AccessibleRole::Tab);
    assert!(w.get_zones_draw());
    drag(&w, &s, (100.0, 100.0), (500.0, 300.0));
    assert_eq!(w.get_zones().row_count(), 2);
    assert_eq!(w.get_zones_selected(), 1);
    let p = on_disk(&paths);
    assert_eq!(p.zones.len(), 2, "saved at once");
    assert!(
        near(p.zones[1].rect, [100.0, 100.0, 400.0, 200.0]),
        "{:?}",
        p.zones[1].rect
    );
    assert_eq!(p.zones[1].base, [SHOT_W, SHOT_H]);

    // click to select (select tool)
    press(&w, "Select", AccessibleRole::Tab);
    click(&w, &s, (900.0, 950.0));
    assert_eq!(w.get_zones_selected(), 0);
    // click on empty space: nothing selected
    click(&w, &s, (1800.0, 500.0));
    assert_eq!(w.get_zones_selected(), -1);
    click(&w, &s, (300.0, 200.0));
    assert_eq!(w.get_zones_selected(), 1);

    // drag the bottom right corner of zone 1 from (500, 300) to (600, 400)
    drag(&w, &s, (500.0, 300.0), (600.0, 400.0));
    let z = w.get_zones().row_data(1).unwrap();
    assert!(
        near([z.x, z.y, z.w, z.h], [100.0, 100.0, 500.0, 300.0]),
        "{z:?}"
    );
    assert!(near(
        on_disk(&paths).zones[1].rect,
        [100.0, 100.0, 500.0, 300.0]
    ));

    // drag inside a zone moves it
    drag(&w, &s, (300.0, 200.0), (400.0, 260.0));
    let z = w.get_zones().row_data(1).unwrap();
    assert!(
        near([z.x, z.y, z.w, z.h], [200.0, 160.0, 500.0, 300.0]),
        "{z:?}"
    );

    // kind change of the selected zone
    press(&w, "Pickups", AccessibleRole::Tab);
    assert_eq!(w.get_zones().row_data(1).unwrap().kind, Kind::Pickups);
    assert_eq!(w.get_zone_kind(), Kind::Pickups);
    assert_eq!(
        on_disk(&paths).zones[1].kind,
        dr_runtime::Kind::Notification
    );

    // rename the selected zone: typed text waits, Enter (rename-done) saves it
    w.invoke_zone_rename(1, "Item pop-ups".into());
    assert_eq!(on_disk(&paths).zones[1].name, "Dialogue");
    w.invoke_zone_rename_done();
    assert_eq!(w.get_zones().row_data(1).unwrap().name, "Item pop-ups");
    assert_eq!(on_disk(&paths).zones[1].name, "Item pop-ups");

    // Delete removes it, Ctrl+Z brings it back
    key(&w, slint::platform::Key::Delete);
    assert_eq!(w.get_zones().row_count(), 1);
    assert_eq!(on_disk(&paths).zones.len(), 1);
    ctrl_key(&w, "z");
    assert_eq!(w.get_zones().row_count(), 2);
    assert_eq!(on_disk(&paths).zones.len(), 2);
    assert_eq!(w.get_zones().row_data(1).unwrap().name, "Item pop-ups");

    // the Delete zone button does the same
    press(&w, "Delete zone", AccessibleRole::Button);
    assert_eq!(w.get_zones().row_count(), 1);
}

/// The "Zone name" text field of the selected row.
fn name_field(w: &dr_app::MainWindow) -> ElementHandle {
    ElementHandle::find_by_accessible_label(w, "Zone name")
        .next()
        .expect("a name field in the selected row")
}

#[test]
fn typed_rename_is_one_change_and_one_undo_step() {
    let (_dir, paths) = temp_paths();
    saved(&paths);
    let (w, _vm) = real_window(&paths, FakeDesktop::new());
    go_zones(&w);
    w.invoke_zone_select(0);
    w.set_zones_selected(0);
    let f = name_field(&w);
    // click into the field, select all, type a new name
    let at = f.absolute_position();
    let size = f.size();
    let p = slint::LogicalPosition::new(at.x + size.width / 2.0, at.y + size.height / 2.0);
    let win = w.window();
    win.dispatch_event(WindowEvent::PointerPressed {
        position: p,
        button: PointerEventButton::Left,
    });
    win.dispatch_event(WindowEvent::PointerReleased {
        position: p,
        button: PointerEventButton::Left,
    });
    ctrl_key(&w, "a");
    for c in "Lines".chars() {
        key(&w, c.to_string());
    }
    assert_eq!(f.accessible_value().unwrap_or_default(), "Lines");
    // not saved while typing, and the model keeps the old name
    assert_eq!(on_disk(&paths).zones[0].name, "Subtitles");
    assert_eq!(w.get_zones().row_data(0).unwrap().name, "Subtitles");
    key(&w, slint::platform::Key::Return);
    assert_eq!(w.get_zones().row_data(0).unwrap().name, "Lines");
    assert_eq!(on_disk(&paths).zones[0].name, "Lines");
    // Enter gave the keys back to the window: Ctrl+Z is the zone undo, one step for the whole old name, and the
    // field follows
    ctrl_key(&w, "z");
    assert_eq!(w.get_zones().row_data(0).unwrap().name, "Subtitles");
    assert_eq!(on_disk(&paths).zones[0].name, "Subtitles");
    slint::platform::update_timers_and_animations();
    assert_eq!(
        name_field(&w).accessible_value().unwrap_or_default(),
        "Subtitles"
    );
}

#[test]
fn typed_rename_is_saved_when_another_zone_is_clicked() {
    let (_dir, paths) = temp_paths();
    saved(&paths);
    let (w, _vm) = real_window(&paths, FakeDesktop::new());
    go_zones(&w);
    w.invoke_zone_rename(0, "Lines".into());
    // a click on the stage (empty space) commits it before anything else
    let s = stage(&w, SHOT_W as f32, SHOT_H as f32);
    click(&w, &s, (1800.0, 300.0));
    assert_eq!(on_disk(&paths).zones[0].name, "Lines");
    // a tab change (the field goes away with no focus event) saves it too
    w.invoke_zone_rename(0, "Speech".into());
    go_overview(&w);
    assert_eq!(on_disk(&paths).zones[0].name, "Speech");
}

#[test]
fn delete_while_dragging_a_zone_does_not_crash() {
    let (_dir, paths) = temp_paths();
    saved(&paths);
    let (w, _vm) = real_window(&paths, FakeDesktop::new());
    go_zones(&w);
    press(&w, "Select", AccessibleRole::Tab);
    let s = stage(&w, SHOT_W as f32, SHOT_H as f32);
    let win = w.window();
    win.dispatch_event(WindowEvent::PointerMoved {
        position: s.at(900.0, 950.0),
    });
    win.dispatch_event(WindowEvent::PointerPressed {
        position: s.at(900.0, 950.0),
        button: PointerEventButton::Left,
    });
    assert_eq!(w.get_zones_selected(), 0);
    // the mouse still holds the only zone: Delete, then move and let go
    key(&w, slint::platform::Key::Delete);
    assert_eq!(w.get_zones().row_count(), 0);
    win.dispatch_event(WindowEvent::PointerMoved {
        position: s.at(1000.0, 700.0),
    });
    win.dispatch_event(WindowEvent::PointerReleased {
        position: s.at(1000.0, 700.0),
        button: PointerEventButton::Left,
    });
    assert_eq!(w.get_zones().row_count(), 0);
    assert!(on_disk(&paths).zones.is_empty());
}

#[test]
fn overview_mode_button_is_saved() {
    let (_dir, paths) = temp_paths();
    saved(&paths);
    let (w, _vm) = real_window(&paths, FakeDesktop::new());
    assert_eq!(on_disk(&paths).mode, dr_runtime::Mode::Auto);
    press(&w, "Mode", AccessibleRole::Combobox);
    assert!(!w.get_automatic());
    assert_eq!(on_disk(&paths).mode, dr_runtime::Mode::Manual);
}

#[test]
fn overview_toggle_is_saved() {
    let (_dir, paths) = temp_paths();
    saved(&paths);
    let (w, _vm) = real_window(&paths, FakeDesktop::new());
    assert!(on_disk(&paths).zones[0].on);
    press(&w, "Subtitles", AccessibleRole::Checkbox);
    assert!(!w.get_zones().row_data(0).unwrap().on);
    let p = on_disk(&paths);
    assert!(!p.zones[0].on);
    assert!(p.core_zones().is_empty(), "a switched-off zone is not read");
}

#[test]
fn draw_zones_page_in_setup() {
    let (_dir, paths) = temp_paths();
    let (w, _vm) = real_window(&paths, FakeDesktop::new());
    press(&w, "Get started", AccessibleRole::Button);
    press(&w, "Continue", AccessibleRole::Button);
    assert_eq!(w.get_page(), Page::DrawZones);
    assert!(w.get_setup_draw(), "page 03 starts with the draw tool");
    let s = stage(&w, SHOT_W as f32, SHOT_H as f32);
    drag(&w, &s, (100.0, 100.0), (400.0, 200.0));
    // a drag too small for a zone adds nothing
    drag(&w, &s, (600.0, 600.0), (603.0, 602.0));
    assert_eq!(w.get_setup_zones().row_count(), 1);
    // setup zones are not saved before Finish
    assert!(
        !paths.profiles().exists()
            || std::fs::read_dir(paths.profiles())
                .unwrap()
                .next()
                .is_none()
    );
    // a click on empty space cleared the selection; a click on the zone selects it, Backspace removes it
    assert_eq!(w.get_setup_selected(), -1);
    click(&w, &s, (250.0, 150.0));
    assert_eq!(w.get_setup_selected(), 0);
    key(&w, slint::platform::Key::Backspace);
    assert_eq!(w.get_setup_zones().row_count(), 0);
}

#[test]
fn tool_choice_survives_other_actions() {
    let (_dir, paths) = temp_paths();
    saved(&paths);
    let (w, _vm) = real_window(&paths, FakeDesktop::new());
    go_zones(&w);
    press(&w, "Draw zone", AccessibleRole::Tab);
    assert!(w.get_zones_draw());
    // picking a kind, clicking a row and switching a zone keep the draw tool
    press(&w, "Speaker", AccessibleRole::Tab);
    w.invoke_zone_select(0);
    press(&w, "Subtitles", AccessibleRole::Checkbox);
    assert!(w.get_zones_draw(), "the tool flipped back to Select");
    // so a drag that starts on a zone draws a new one instead of moving it
    let s = stage(&w, SHOT_W as f32, SHOT_H as f32);
    drag(&w, &s, (300.0, 920.0), (700.0, 1060.0));
    assert_eq!(w.get_zones().row_count(), 2);
    // selecting the Subtitles row made its kind (Dialogue) the kind for new zones
    assert_eq!(w.get_zones().row_data(1).unwrap().kind, Kind::Dialogue);
    let z0 = w.get_zones().row_data(0).unwrap();
    assert!(
        near([z0.x, z0.y, z0.w, z0.h], [200.0, 900.0, 1500.0, 100.0]),
        "{z0:?}"
    );
}
