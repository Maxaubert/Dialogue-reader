//! Navigation through the Slint testing backend: no window, no input to the desktop.

use dr_app::{MainWindow, OverlayPreview, Page};
use i_slint_backend_testing::{AccessibleRole, ElementHandle};
use slint::platform::{Key, WindowEvent};
use slint::{ComponentHandle, Model, SharedString};

fn window() -> MainWindow {
    i_slint_backend_testing::init_no_event_loop();
    let w = dr_app::build_main_window().expect("window");
    w.show().expect("show");
    w
}

/// Presses the element with this label and role (buttons, tabs, toggles).
fn press(w: &MainWindow, label: &str, role: AccessibleRole) {
    let hits: Vec<ElementHandle> = ElementHandle::find_by_accessible_label(w, label)
        .filter(|e| e.accessible_role() == Some(role))
        .collect();
    let e = hits
        .first()
        .unwrap_or_else(|| panic!("no {role:?} labelled {label:?}"));
    e.invoke_accessible_default_action();
}

fn key(w: &MainWindow, text: impl Into<SharedString>) {
    let text: SharedString = text.into();
    w.window()
        .dispatch_event(WindowEvent::KeyPressed { text: text.clone() });
    w.window().dispatch_event(WindowEvent::KeyReleased { text });
}

#[test]
fn onboarding_walks_forward_and_back() {
    let w = window();
    assert_eq!(w.get_page(), Page::Welcome);
    press(&w, "Get started", AccessibleRole::Button);
    assert_eq!(w.get_page(), Page::PickWindow);
    for expected in [Page::DrawZones, Page::Keys, Page::Voice, Page::Mode] {
        press(&w, "Continue", AccessibleRole::Button);
        assert_eq!(w.get_page(), expected);
    }
    for expected in [
        Page::Voice,
        Page::Keys,
        Page::DrawZones,
        Page::PickWindow,
        Page::Welcome,
    ] {
        press(&w, "Back", AccessibleRole::Button);
        assert_eq!(w.get_page(), expected);
    }
}

#[test]
fn finish_lands_on_overview_and_tabs_switch() {
    let w = window();
    w.set_page(Page::Mode);
    press(&w, "Finish", AccessibleRole::Button);
    assert_eq!(w.get_page(), Page::Overview);
    press(&w, "Zones", AccessibleRole::Tab);
    assert_eq!(w.get_page(), Page::Zones);
    press(&w, "Settings", AccessibleRole::Tab);
    assert_eq!(w.get_page(), Page::Settings);
    press(&w, "Overview", AccessibleRole::Tab);
    assert_eq!(w.get_page(), Page::Overview);
    press(&w, "Show all", AccessibleRole::Button);
    assert_eq!(w.get_page(), Page::Activity);
}

#[test]
fn picking_a_window_and_a_mode() {
    let w = window();
    w.set_page(Page::PickWindow);
    press(&w, "Satisfactory", AccessibleRole::Button);
    assert_eq!(w.get_picked_window(), 1);
    w.set_page(Page::Mode);
    assert!(w.get_automatic());
    press(&w, "On key press", AccessibleRole::RadioButton);
    assert!(!w.get_automatic());
}

#[test]
fn ctrl_k_opens_the_game_switcher_and_esc_closes_it() {
    let w = window();
    w.set_page(Page::Overview);
    assert!(!w.get_switcher_open());
    w.window().dispatch_event(WindowEvent::KeyPressed {
        text: Key::Control.into(),
    });
    key(&w, "k");
    w.window().dispatch_event(WindowEvent::KeyReleased {
        text: Key::Control.into(),
    });
    assert!(w.get_switcher_open());
    key(&w, Key::Escape);
    assert!(!w.get_switcher_open());
}

#[test]
fn game_button_opens_the_switcher_and_enter_switches() {
    let w = window();
    w.set_page(Page::Overview);
    press(&w, "Switch game", AccessibleRole::Button);
    assert!(w.get_switcher_open());
    key(&w, Key::DownArrow);
    key(&w, Key::Return);
    assert!(!w.get_switcher_open());
    assert_eq!(w.get_current_game(), 2);
}

#[test]
fn zone_toggle_changes_the_view_model() {
    let w = window();
    w.set_page(Page::Overview);
    assert!(!w.get_zones().row_data(3).unwrap().on, "prompts start off");
    press(&w, "Button prompts", AccessibleRole::Checkbox);
    assert!(w.get_zones().row_data(3).unwrap().on);
    // the Zones tab shows the same model
    w.set_page(Page::Zones);
    press(&w, "Button prompts", AccessibleRole::Checkbox);
    assert!(!w.get_zones().row_data(3).unwrap().on);
}

#[test]
fn quick_menu_keys_and_esc() {
    let w = window();
    w.set_page(Page::Overview);
    w.set_overlay(OverlayPreview::QuickMenu);
    // Enter on the first row, Manage zones, opens zone edit
    key(&w, Key::Return);
    assert_eq!(w.get_overlay(), OverlayPreview::ZoneEdit);
    w.set_overlay(OverlayPreview::QuickMenu);
    key(&w, Key::Escape);
    assert_eq!(w.get_overlay(), OverlayPreview::None);
}

#[test]
fn zone_edit_delete_esc_and_kind_pick() {
    let w = window();
    w.set_overlay(OverlayPreview::ZoneEdit);
    assert_eq!(w.get_edit_zones().row_count(), 5);
    assert_eq!(w.get_edit_selected(), 1);
    key(&w, Key::Delete);
    assert_eq!(w.get_edit_zones().row_count(), 4);
    assert_eq!(w.get_edit_selected(), -1);
    // the drawn zone waits for a kind; picking Speaker adds it
    assert!(w.get_edit_draft());
    press(&w, "Speaker", AccessibleRole::Button);
    assert_eq!(w.get_edit_zones().row_count(), 5);
    assert!(!w.get_edit_draft());
    let added = w.get_edit_zones().row_data(4).unwrap();
    assert_eq!(added.kind, dr_app::Kind::Speaker);
    assert_eq!((added.w, added.h), (360.0, 112.0));
    key(&w, Key::Escape);
    assert_eq!(w.get_overlay(), OverlayPreview::None);
}
