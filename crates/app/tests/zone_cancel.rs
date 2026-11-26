//! A left drag on the stage (draw, move, resize) and the other mouse buttons, through simulated pointer events on
//! the Slint testing backend (Max, 2026-11-22: a right click froze the box, and the release put it at the
//! cursor). A right click or Esc cancels the drag and puts the zone back; other buttons never end or freeze it.

mod common;

use common::*;
use dr_app::MainWindow;
use dr_runtime::{Paths, Profile, Shot, ZoneCfg, profile};
use slint::platform::{Key, PointerEventButton, WindowEvent};
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

fn near(a: [f32; 4], b: [f32; 4]) -> bool {
    a.iter().zip(b).all(|(x, y)| (x - y).abs() < 2.0)
}

fn rect(w: &MainWindow, i: usize) -> [f32; 4] {
    let z = w.get_zones().row_data(i).unwrap();
    [z.x, z.y, z.w, z.h]
}

fn moved(w: &MainWindow, s: &StageAt, at: (f32, f32)) {
    w.window().dispatch_event(WindowEvent::PointerMoved {
        position: s.at(at.0, at.1),
    });
}

fn button(w: &MainWindow, s: &StageAt, at: (f32, f32), b: PointerEventButton, down: bool) {
    let position = s.at(at.0, at.1);
    w.window().dispatch_event(if down {
        WindowEvent::PointerPressed {
            position,
            button: b,
        }
    } else {
        WindowEvent::PointerReleased {
            position,
            button: b,
        }
    });
}

/// The Zones tab of a saved profile with one zone, and its stage.
fn zones_tab(paths: &Paths) -> (MainWindow, dr_app::vm::Shared, StageAt) {
    saved(paths);
    let (w, vm) = real_window(paths, FakeDesktop::new());
    go_zones(&w);
    let s = stage(&w, SHOT_W as f32, SHOT_H as f32);
    (w, vm, s)
}

#[test]
fn right_click_while_drawing_discards_the_new_zone() {
    let (_dir, paths) = temp_paths();
    let (w, _vm, s) = zones_tab(&paths);
    draw_tool(&w, true);
    moved(&w, &s, (100.0, 100.0));
    button(&w, &s, (100.0, 100.0), PointerEventButton::Left, true);
    moved(&w, &s, (300.0, 200.0));
    assert!(w.get_drafting(), "the box follows the mouse");
    button(&w, &s, (300.0, 200.0), PointerEventButton::Right, true);
    assert!(!w.get_drafting(), "the right press cancels at once");
    button(&w, &s, (300.0, 200.0), PointerEventButton::Right, false);
    moved(&w, &s, (600.0, 400.0));
    assert!(
        !w.get_drafting(),
        "no box comes back while the left button is still down"
    );
    button(&w, &s, (600.0, 400.0), PointerEventButton::Left, false);
    assert_eq!(w.get_zones().row_count(), 1, "no zone was added");
    assert_eq!(on_disk(&paths).zones.len(), 1);

    // the next drag draws as usual
    drag(&w, &s, (100.0, 100.0), (500.0, 300.0));
    assert_eq!(w.get_zones().row_count(), 2);
    assert!(near(rect(&w, 1), [100.0, 100.0, 400.0, 200.0]));
}

#[test]
fn right_click_while_moving_puts_the_zone_back() {
    let (_dir, paths) = temp_paths();
    let (w, _vm, s) = zones_tab(&paths);
    draw_tool(&w, false);
    let before = rect(&w, 0);
    moved(&w, &s, (900.0, 950.0));
    button(&w, &s, (900.0, 950.0), PointerEventButton::Left, true);
    moved(&w, &s, (800.0, 700.0));
    assert!(
        near(rect(&w, 0), [100.0, 650.0, 1500.0, 100.0]),
        "it follows"
    );
    button(&w, &s, (800.0, 700.0), PointerEventButton::Right, true);
    button(&w, &s, (800.0, 700.0), PointerEventButton::Right, false);
    assert_eq!(rect(&w, 0), before, "back where it was");
    moved(&w, &s, (700.0, 500.0));
    button(&w, &s, (700.0, 500.0), PointerEventButton::Left, false);
    assert_eq!(
        rect(&w, 0),
        before,
        "the release does not put it at the cursor"
    );
    assert!(near(on_disk(&paths).zones[0].rect, before));
    // a cancel is no change: Ctrl+Z has nothing of it to undo
    ctrl_key(&w, "z");
    assert_eq!(rect(&w, 0), before);
}

#[test]
fn right_click_while_resizing_puts_the_size_back() {
    let (_dir, paths) = temp_paths();
    let (w, _vm, s) = zones_tab(&paths);
    draw_tool(&w, false);
    w.invoke_zone_select(0);
    let before = rect(&w, 0);
    // the bottom right handle of Subtitles is at (1700, 1000)
    moved(&w, &s, (1700.0, 1000.0));
    button(&w, &s, (1700.0, 1000.0), PointerEventButton::Left, true);
    moved(&w, &s, (1800.0, 1050.0));
    assert!(near(rect(&w, 0), [200.0, 900.0, 1600.0, 150.0]));
    button(&w, &s, (1800.0, 1050.0), PointerEventButton::Right, true);
    button(&w, &s, (1800.0, 1050.0), PointerEventButton::Right, false);
    moved(&w, &s, (1500.0, 980.0));
    button(&w, &s, (1500.0, 980.0), PointerEventButton::Left, false);
    assert_eq!(rect(&w, 0), before);
    assert!(near(on_disk(&paths).zones[0].rect, before));
}

#[test]
fn esc_while_drawing_or_moving_cancels() {
    let (_dir, paths) = temp_paths();
    let (w, _vm, s) = zones_tab(&paths);
    draw_tool(&w, true);
    moved(&w, &s, (100.0, 100.0));
    button(&w, &s, (100.0, 100.0), PointerEventButton::Left, true);
    moved(&w, &s, (300.0, 200.0));
    key(&w, Key::Escape);
    assert!(!w.get_drafting());
    moved(&w, &s, (600.0, 400.0));
    button(&w, &s, (600.0, 400.0), PointerEventButton::Left, false);
    assert_eq!(w.get_zones().row_count(), 1);

    draw_tool(&w, false);
    let before = rect(&w, 0);
    moved(&w, &s, (900.0, 950.0));
    button(&w, &s, (900.0, 950.0), PointerEventButton::Left, true);
    moved(&w, &s, (800.0, 700.0));
    key(&w, Key::Escape);
    assert_eq!(rect(&w, 0), before);
    button(&w, &s, (800.0, 700.0), PointerEventButton::Left, false);
    assert_eq!(rect(&w, 0), before);
    assert!(near(on_disk(&paths).zones[0].rect, before));
}

#[test]
fn middle_click_never_ends_or_freezes_a_drag() {
    let (_dir, paths) = temp_paths();
    let (w, _vm, s) = zones_tab(&paths);
    draw_tool(&w, true);
    moved(&w, &s, (100.0, 100.0));
    button(&w, &s, (100.0, 100.0), PointerEventButton::Left, true);
    moved(&w, &s, (300.0, 200.0));
    button(&w, &s, (300.0, 200.0), PointerEventButton::Middle, true);
    button(&w, &s, (300.0, 200.0), PointerEventButton::Middle, false);
    moved(&w, &s, (500.0, 300.0));
    assert!(w.get_drafting());
    let d = w.get_draft();
    assert!(
        near([d.x, d.y, d.w, d.h], [100.0, 100.0, 400.0, 200.0]),
        "the box still follows: {d:?}"
    );
    button(&w, &s, (500.0, 300.0), PointerEventButton::Left, false);
    assert_eq!(w.get_zones().row_count(), 2);
    assert!(near(rect(&w, 1), [100.0, 100.0, 400.0, 200.0]));
}

#[test]
fn a_drag_without_its_grab_is_cancelled_when_the_mouse_leaves_the_stage() {
    // after a middle click the stage no longer holds the mouse, so a left release outside it would be lost
    let (_dir, paths) = temp_paths();
    let (w, _vm, s) = zones_tab(&paths);
    draw_tool(&w, true);
    moved(&w, &s, (100.0, 100.0));
    button(&w, &s, (100.0, 100.0), PointerEventButton::Left, true);
    moved(&w, &s, (300.0, 200.0));
    button(&w, &s, (300.0, 200.0), PointerEventButton::Middle, true);
    button(&w, &s, (300.0, 200.0), PointerEventButton::Middle, false);
    // far right of the window, off the stage
    let out = slint::LogicalPosition::new(1430.0, 450.0);
    w.window()
        .dispatch_event(WindowEvent::PointerMoved { position: out });
    slint::platform::update_timers_and_animations();
    assert!(!w.get_drafting());
    w.window().dispatch_event(WindowEvent::PointerReleased {
        position: out,
        button: PointerEventButton::Left,
    });
    assert_eq!(w.get_zones().row_count(), 1);
    // and a normal drag that leaves the stage keeps its grab and still draws: the hover check runs after the
    // move off the stage (`changed` handlers are deferred) and must not cancel it
    moved(&w, &s, (1700.0, 100.0));
    button(&w, &s, (1700.0, 100.0), PointerEventButton::Left, true);
    moved(&w, &s, (1800.0, 200.0));
    w.window()
        .dispatch_event(WindowEvent::PointerMoved { position: out });
    slint::platform::update_timers_and_animations();
    assert!(
        w.get_drafting(),
        "a drag with its grab survives leaving the stage"
    );
    moved(&w, &s, (2100.0, 300.0));
    button(&w, &s, (2100.0, 300.0), PointerEventButton::Left, false);
    assert_eq!(w.get_zones().row_count(), 2);
    assert!(near(rect(&w, 1), [1700.0, 100.0, 220.0, 200.0]));
}

#[test]
fn focus_loss_during_a_drag_cancels_it() {
    // Alt+Tab while a zone is held: the left release may never come, so the zone must not keep following the
    // mouse, and a later release (or press) must not put it at the cursor
    let (_dir, paths) = temp_paths();
    let (w, vm, s) = zones_tab(&paths);
    draw_tool(&w, false);
    let before = rect(&w, 0);
    moved(&w, &s, (900.0, 950.0));
    button(&w, &s, (900.0, 950.0), PointerEventButton::Left, true);
    moved(&w, &s, (800.0, 700.0));
    assert_ne!(rect(&w, 0), before, "it follows");
    dr_app::focus_lost(&w, &vm);
    slint::platform::update_timers_and_animations();
    assert_eq!(rect(&w, 0), before, "back where it was");
    // the mouse comes back with no button held: the zone stays
    moved(&w, &s, (600.0, 500.0));
    assert_eq!(rect(&w, 0), before, "it no longer follows");
    button(&w, &s, (600.0, 500.0), PointerEventButton::Left, false);
    assert_eq!(rect(&w, 0), before);
    assert!(near(on_disk(&paths).zones[0].rect, before));
    ctrl_key(&w, "z");
    assert_eq!(rect(&w, 0), before, "a cancel is no change");

    // a draw: the box goes, and the next drag draws as usual
    draw_tool(&w, true);
    moved(&w, &s, (100.0, 100.0));
    button(&w, &s, (100.0, 100.0), PointerEventButton::Left, true);
    moved(&w, &s, (300.0, 200.0));
    assert!(w.get_drafting());
    dr_app::focus_lost(&w, &vm);
    slint::platform::update_timers_and_animations();
    assert!(!w.get_drafting());
    moved(&w, &s, (500.0, 300.0));
    assert!(!w.get_drafting());
    button(&w, &s, (500.0, 300.0), PointerEventButton::Left, false);
    assert_eq!(w.get_zones().row_count(), 1);
    drag(&w, &s, (100.0, 100.0), (500.0, 300.0));
    assert_eq!(w.get_zones().row_count(), 2);
}

#[test]
fn esc_without_a_drag_is_left_alone() {
    // Esc with no drag is not taken by the zone editor (the window may use it for something else)
    let (_dir, paths) = temp_paths();
    let (w, vm, _s) = zones_tab(&paths);
    assert!(!vm.borrow_mut().cancel_zone_drag());
    key(&w, Key::Escape);
    assert_eq!(w.get_zones().row_count(), 1);
}
