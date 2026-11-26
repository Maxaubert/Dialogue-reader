//! Overlay spike: can Slint 1.18 windows on winit be DialogReader's in-game overlays?
//! See README.md and docs/overlay-spike.md.

slint::include_modules!();

pub mod compose;
pub mod demo;
pub mod keys;
pub mod overlay;
pub mod probe;
pub mod win;

/// Quick menu rows (Manage zones ... Quit).
pub const MENU_ROWS: i32 = 9;

/// What a menu key does to the menu (move, choose). Esc is handled by the caller (it closes the window).
pub fn menu_key(m: &MenuWindow, k: keys::MenuKey) {
    use keys::MenuKey::*;
    let n = MENU_ROWS;
    match k {
        Down => m.set_highlight((m.get_highlight() + 1) % n),
        Up => m.set_highlight((m.get_highlight() + n - 1) % n),
        Enter | Space => m.set_activated(m.get_highlight()),
        _ => {}
    }
}
