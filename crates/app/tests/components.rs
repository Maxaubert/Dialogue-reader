//! The shared parts of the d-black design (ui/components/base.slint), pressed on the component sheet
//! (ui/components/sheet.slint) through the Slint testing backend. No window, nothing on screen.

use dr_app::MainWindow;
use i_slint_backend_testing::{AccessibleRole, ElementHandle};
use slint::ComponentHandle;

fn sheet() -> MainWindow {
    i_slint_backend_testing::init_no_event_loop();
    dr_app::set_instant_motion(true);
    let w = dr_app::build_main_window().expect("window");
    w.window().set_size(slint::LogicalSize::new(1440.0, 900.0));
    w.show().expect("show");
    // Welcome under the sheet: no list rows of its own
    w.set_sheet(true);
    w
}

fn find(w: &MainWindow, label: &str, role: AccessibleRole) -> Vec<ElementHandle> {
    ElementHandle::find_by_accessible_label(w, label)
        .filter(|e| e.accessible_role() == Some(role))
        .collect()
}

#[test]
fn a_list_row_says_its_full_text_once() {
    let w = sheet();
    // the grey speaker drawn over the white text is not in the tree; the whole line is, once
    for full in [
        "Dutch: We ride at first light. Nobody stays behind.",
        "Hosea: If we do not get the wagons through the pass before dark, we lose the whole camp to the cold, and the wolves that follow it down from the ridge will be at the horses before morning.",
    ] {
        assert_eq!(find(&w, full, AccessibleRole::Text).len(), 1, "{full}");
    }
    assert!(
        find(&w, "Dutch:", AccessibleRole::Text).is_empty(),
        "the grey speaker copy has no role in the tree"
    );
    // the long line is cut to two lines: its row is two lines high (20 px each) plus 16 px of padding
    let hosea = &find(
        &w,
        "Hosea: If we do not get the wagons through the pass before dark, we lose the whole camp to the cold, and the wolves that follow it down from the ridge will be at the horses before morning.",
        AccessibleRole::Text,
    )[0];
    assert!(hosea.size().height <= 40.5, "{:?}", hosea.size());
    assert_eq!(find(&w, "Replay", AccessibleRole::Button).len(), 1);
}

#[test]
fn a_zone_chip_toggles() {
    let w = sheet();
    let chip = |w: &MainWindow| find(w, "Prompts", AccessibleRole::Checkbox).remove(0);
    assert_eq!(chip(&w).accessible_checked(), Some(false));
    chip(&w).invoke_accessible_default_action();
    assert_eq!(chip(&w).accessible_checked(), Some(true));
}

#[test]
fn a_keycap_click_starts_capture() {
    let w = sheet();
    assert!(find(&w, "Press a key", AccessibleRole::Button).is_empty());
    find(&w, "F7", AccessibleRole::Button)[0].invoke_accessible_default_action();
    assert_eq!(find(&w, "Press a key", AccessibleRole::Button).len(), 1);
}

#[test]
fn disabled_controls_take_no_press() {
    let w = sheet();
    let t = &find(&w, "Toggle disabled", AccessibleRole::Checkbox)[0];
    assert_eq!(t.accessible_enabled(), Some(false));
    t.invoke_accessible_default_action();
    assert_eq!(t.accessible_checked(), Some(true));
    let b = &find(&w, "Primary disabled", AccessibleRole::Button)[0];
    assert_eq!(b.accessible_enabled(), Some(false));
}

#[test]
fn the_segmented_control_and_the_slider_answer() {
    let w = sheet();
    let tab = |w: &MainWindow, l: &str| find(w, l, AccessibleRole::Tab).remove(0);
    assert_eq!(tab(&w, "Top right").accessible_item_selected(), Some(true));
    tab(&w, "Bottom").invoke_accessible_default_action();
    assert_eq!(tab(&w, "Bottom").accessible_item_selected(), Some(true));
    assert_eq!(tab(&w, "Top right").accessible_item_selected(), Some(false));
    let s = &find(&w, "Speed", AccessibleRole::Slider)[0];
    assert_eq!(s.accessible_value().as_deref(), Some("40"));
}
