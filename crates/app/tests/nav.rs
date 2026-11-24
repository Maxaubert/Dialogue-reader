//! Navigation through the Slint testing backend: no window, no input to the desktop.
//!
//! There is no nav bar: the game name opens the switcher (also Ctrl K), the gear opens Settings, and sub-pages
//! close with the back chevron or Esc. Most tests run with instant motion. The `motion_*` tests keep the real
//! page push and move the testing clock by hand, to check fast input and that only one page takes input.

use std::time::Duration;

use dr_app::{MainWindow, OverlayPreview, Page};
use i_slint_backend_testing::{AccessibleRole, ElementHandle, ElementQuery};
use slint::platform::{Key, PointerEventButton, WindowEvent};
use slint::{ComponentHandle, Model, SharedString};

/// The main window with the fake data. `motion`: the real page push (the clock moves only in `wait`).
fn window_with(motion: bool) -> MainWindow {
    i_slint_backend_testing::init_no_event_loop();
    dr_app::set_instant_motion(!motion);
    let w = dr_app::build_main_window().expect("window");
    w.window().set_size(slint::LogicalSize::new(1440.0, 900.0));
    w.show().expect("show");
    w
}

fn window() -> MainWindow {
    window_with(false)
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

fn ctrl_k(w: &MainWindow) {
    w.window().dispatch_event(WindowEvent::KeyPressed {
        text: Key::Control.into(),
    });
    key(w, "k");
    w.window().dispatch_event(WindowEvent::KeyReleased {
        text: Key::Control.into(),
    });
}

/// Moves the testing clock by `ms`, in small steps (animations and timers run as in a real window).
fn wait(ms: u64) {
    let mut left = ms;
    while left > 0 {
        let step = left.min(10);
        i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(step));
        left -= step;
    }
}

/// Long enough for any page change to end (the back push is 80 + 200 ms, forward 80 + 260 ms).
fn settle() {
    wait(600);
}

/// The pages in the accessibility tree: the page slots that can be seen (any opacity above 0).
fn pages(w: &MainWindow) -> Vec<String> {
    let mut v: Vec<String> = ElementQuery::from_root(w)
        .match_descendants()
        .match_accessible_role(AccessibleRole::TabPanel)
        .find_all()
        .into_iter()
        .filter_map(|e| e.accessible_label().map(|l| l.to_string()))
        .collect();
    // one page can be found once per element of its slot: count each page once
    v.sort();
    v.dedup();
    v
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
    // Welcome has no footer: no Back to press there
    assert!(
        ElementHandle::find_by_accessible_label(&w, "Back")
            .next()
            .is_none()
    );
}

#[test]
fn finish_lands_on_overview_and_sub_pages_open_and_close() {
    let w = window();
    w.set_page(Page::Mode);
    press(&w, "Finish", AccessibleRole::Button);
    assert_eq!(w.get_page(), Page::Overview);
    press(&w, "Edit zones", AccessibleRole::Button);
    assert_eq!(w.get_page(), Page::Zones);
    press(&w, "Back", AccessibleRole::Button);
    assert_eq!(w.get_page(), Page::Overview);
    press(&w, "Settings", AccessibleRole::Button);
    assert_eq!(w.get_page(), Page::Settings);
    press(&w, "Back", AccessibleRole::Button);
    assert_eq!(w.get_page(), Page::Overview);
    press(&w, "Show all", AccessibleRole::Button);
    assert_eq!(w.get_page(), Page::Activity);
    press(&w, "Back", AccessibleRole::Button);
    assert_eq!(w.get_page(), Page::Overview);
}

#[test]
fn the_gear_is_on_the_overview_only() {
    let w = window();
    w.set_page(Page::Overview);
    let gear = |w: &MainWindow| {
        ElementHandle::find_by_accessible_label(w, "Settings")
            .filter(|e| e.accessible_role() == Some(AccessibleRole::Button))
            .count()
    };
    assert_eq!(gear(&w), 1);
    press(&w, "Settings", AccessibleRole::Button);
    assert_eq!(gear(&w), 0, "Settings has back, not the gear");
    // and setup has neither the game name nor the gear
    w.set_page(Page::Keys);
    assert_eq!(gear(&w), 0);
    assert!(
        ElementHandle::find_by_accessible_label(&w, "Switch game")
            .next()
            .is_none()
    );
}

#[test]
fn esc_closes_sub_pages() {
    let w = window();
    w.set_page(Page::Overview);
    for open in ["Edit zones", "Show all", "Settings"] {
        press(&w, open, AccessibleRole::Button);
        assert_ne!(w.get_page(), Page::Overview, "{open}");
        key(&w, Key::Escape);
        assert_eq!(w.get_page(), Page::Overview, "Esc after {open}");
    }
    // Esc on the Overview itself changes nothing
    key(&w, Key::Escape);
    assert_eq!(w.get_page(), Page::Overview);
}

#[test]
fn esc_during_key_capture_keeps_the_page() {
    let w = window();
    w.set_page(Page::Overview);
    press(&w, "Settings", AccessibleRole::Button);
    // the key hook takes Esc as Cancel; the page must not close under it
    w.set_capturing_key(0);
    key(&w, Key::Escape);
    assert_eq!(w.get_page(), Page::Settings);
    w.set_capturing_key(-1);
    key(&w, Key::Escape);
    assert_eq!(w.get_page(), Page::Overview);
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
    ctrl_k(&w);
    assert!(w.get_switcher_open());
    key(&w, Key::Escape);
    assert!(!w.get_switcher_open());
    // not in setup: there is no game yet
    w.set_page(Page::Keys);
    ctrl_k(&w);
    assert!(!w.get_switcher_open());
}

#[test]
fn game_name_opens_the_switcher_and_enter_switches() {
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
fn gear_then_ctrl_k_opens_the_switcher_over_settings() {
    let w = window();
    w.set_page(Page::Overview);
    press(&w, "Settings", AccessibleRole::Button);
    ctrl_k(&w);
    assert!(w.get_switcher_open());
    assert_eq!(w.get_page(), Page::Settings);
    // the first Esc closes the switcher only, the second one Settings
    key(&w, Key::Escape);
    assert!(!w.get_switcher_open());
    assert_eq!(w.get_page(), Page::Settings);
    key(&w, Key::Escape);
    assert_eq!(w.get_page(), Page::Overview);
}

#[test]
fn zone_toggle_changes_the_view_model() {
    let w = window();
    w.set_page(Page::Overview);
    assert!(!w.get_zones().row_data(3).unwrap().on, "prompts start off");
    // the Prompts chip switches every prompt zone (the only one here is row 3)
    press(&w, "Prompts", AccessibleRole::Checkbox);
    assert!(w.get_zones().row_data(3).unwrap().on);
    press(&w, "Prompts", AccessibleRole::Checkbox);
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

// ---- with the real page push ----

#[test]
fn motion_launch_shows_the_page_without_a_push() {
    let w = window_with(true);
    // Rust sets the first page before any move in the UI: it shows at once
    w.set_page(Page::Overview);
    assert_eq!(pages(&w), ["Overview page"]);
}

#[test]
fn motion_triple_continue_ends_on_one_page() {
    let w = window_with(true);
    press(&w, "Get started", AccessibleRole::Button);
    // the footer fades in after the 80 ms wait of the push
    wait(100);
    for _ in 0..3 {
        press(&w, "Continue", AccessibleRole::Button);
        wait(30);
    }
    assert_eq!(w.get_page(), Page::Voice);
    settle();
    assert_eq!(pages(&w), ["Voice page"], "no ghost page is left");
    // the pages that left are gone, with their text fields
    assert!(
        ElementHandle::find_by_accessible_label(&w, "Search windows")
            .next()
            .is_none()
    );
}

#[test]
fn motion_continue_then_back_ends_where_it_started() {
    let w = window_with(true);
    w.set_page(Page::PickWindow);
    press(&w, "Continue", AccessibleRole::Button);
    wait(50);
    press(&w, "Back", AccessibleRole::Button);
    assert_eq!(w.get_page(), Page::PickWindow);
    settle();
    assert_eq!(pages(&w), ["Game page"]);
}

#[test]
fn motion_gear_then_esc() {
    let w = window_with(true);
    w.set_page(Page::Overview);
    press(&w, "Settings", AccessibleRole::Button);
    // 40 ms in: Overview fades out, Settings waits for its 80 ms
    wait(40);
    assert_eq!(pages(&w), ["Overview page"]);
    settle();
    assert_eq!(pages(&w), ["Settings page"]);
    key(&w, Key::Escape);
    assert_eq!(w.get_page(), Page::Overview);
    settle();
    assert_eq!(pages(&w), ["Overview page"]);
}

#[test]
fn motion_gear_then_ctrl_k() {
    let w = window_with(true);
    w.set_page(Page::Overview);
    press(&w, "Settings", AccessibleRole::Button);
    wait(20);
    ctrl_k(&w);
    assert!(w.get_switcher_open());
    settle();
    assert_eq!(w.get_page(), Page::Settings);
    assert_eq!(pages(&w), ["Settings page"]);
}

#[test]
fn motion_switch_game_rolls_the_name_in_240_ms() {
    let w = window_with(true);
    w.set_page(Page::Overview);
    press(&w, "Switch game", AccessibleRole::Button);
    key(&w, Key::DownArrow);
    key(&w, Key::Return);
    // the game changes at once; the strip shows the old name while it rolls out
    assert_eq!(w.get_current_game(), 2);
    let name = |w: &MainWindow| {
        let g = w.get_games();
        g.row_data(w.get_current_game() as usize).unwrap().name
    };
    let shown = |w: &MainWindow, n: &str| {
        ElementHandle::find_by_accessible_label(w, n)
            .filter(|e| e.accessible_role() == Some(AccessibleRole::Text))
            .count()
    };
    let new = name(&w);
    // the strip still shows the old name: the new one is not there yet
    let during = shown(&w, &new);
    wait(240);
    assert_eq!(shown(&w, &new), during + 1, "the new name rolled in");
}

#[test]
fn motion_only_one_page_takes_input_while_pages_change() {
    let w = window_with(true);
    w.set_page(Page::Overview);
    press(&w, "Settings", AccessibleRole::Button);
    settle();
    let toggle = ElementHandle::find_by_accessible_label(&w, "Start with Windows")
        .find(|e| e.accessible_role() == Some(AccessibleRole::Checkbox))
        .expect("the Start with Windows switch");
    let at = toggle.absolute_position();
    let size = toggle.size();
    let p = slint::LogicalPosition::new(at.x + size.width / 2.0, at.y + size.height / 2.0);
    let before = w.get_start_with_windows();
    press(&w, "Back", AccessibleRole::Button);
    // 40 ms in: Settings still fades out under the pointer, the Overview waits for its turn
    wait(40);
    assert_eq!(pages(&w), ["Settings page"]);
    let win = w.window();
    win.dispatch_event(WindowEvent::PointerMoved { position: p });
    win.dispatch_event(WindowEvent::PointerPressed {
        position: p,
        button: PointerEventButton::Left,
    });
    win.dispatch_event(WindowEvent::PointerReleased {
        position: p,
        button: PointerEventButton::Left,
    });
    assert_eq!(
        w.get_start_with_windows(),
        before,
        "the page that leaves takes no click"
    );
    settle();
    assert_eq!(w.get_page(), Page::Overview);
    assert_eq!(pages(&w), ["Overview page"]);
}

#[test]
fn enter_walks_the_setup_but_not_during_key_capture() {
    let w = window();
    assert_eq!(w.get_page(), Page::Welcome);
    // Enter is Get started on Welcome, then Continue
    for expected in [Page::PickWindow, Page::DrawZones, Page::Keys] {
        key(&w, Key::Return);
        assert_eq!(w.get_page(), expected);
    }
    // a click on a keycap starts key capture: Enter waits for the key hook
    press(&w, "Read key: Mouse 4", AccessibleRole::Button);
    assert_eq!(w.get_capturing_key(), 0);
    key(&w, Key::Return);
    assert_eq!(w.get_page(), Page::Keys);
    // a second click on the keycap ends the capture
    press(&w, "Press a key", AccessibleRole::Button);
    assert_eq!(w.get_capturing_key(), -1);
    for expected in [Page::Voice, Page::Mode, Page::Overview] {
        key(&w, Key::Return);
        assert_eq!(w.get_page(), expected);
    }
    // Enter does nothing on the main pages
    key(&w, Key::Return);
    assert_eq!(w.get_page(), Page::Overview);
}

#[test]
fn motion_finish_fades_the_setup_into_the_overview() {
    let w = window_with(true);
    w.set_page(Page::Voice);
    press(&w, "Continue", AccessibleRole::Button);
    settle();
    press(&w, "Finish", AccessibleRole::Button);
    assert_eq!(w.get_page(), Page::Overview);
    // 100 ms in: the setup and its footer still fade out (140 ms), the Overview waits 80 ms, then comes in
    wait(100);
    assert!(
        ElementHandle::find_by_accessible_label(&w, "Finish")
            .next()
            .is_some(),
        "the footer fades out, it does not vanish"
    );
    settle();
    assert_eq!(pages(&w), ["Overview page"]);
    assert!(
        ElementHandle::find_by_accessible_label(&w, "Finish")
            .next()
            .is_none(),
        "no footer is left"
    );
    // the footer took no click while it faded: Back is gone with it
    assert!(
        ElementHandle::find_by_accessible_label(&w, "Back")
            .next()
            .is_none()
    );
}
