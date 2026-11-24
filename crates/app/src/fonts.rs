//! The UI font and the motion switch of every window (ui/theme.slint, `Type` and `Motion`).
//!
//! The app font is "Segoe UI Variable Text" (Windows 11), keycaps use "Cascadia Mono". Slint takes one family
//! name, and a missing family falls back to an Arial-like font, not to the next name of a list. So Rust looks for
//! the font files and sets the bundled Geist and Geist Mono when they are missing (Windows 10, or no Windows
//! Terminal). Slint spike, 2026-11-24.
//!
//! Motion: headless tests and screen renders have no running clock, so animations there would stop at their
//! first frame. They call [`set_instant_motion`] first; every window made after that on the thread animates
//! nothing.

use std::cell::Cell;
use std::path::PathBuf;

thread_local! {
    static INSTANT: Cell<bool> = const { Cell::new(false) };
}

/// Every window made after this on this thread has instant motion (or real motion again with `false`).
pub fn set_instant_motion(on: bool) {
    INSTANT.with(|i| i.set(on));
}

/// Whether windows made now on this thread get instant motion.
pub fn instant_motion() -> bool {
    INSTANT.with(Cell::get)
}

/// The font families for `Type`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fonts {
    pub font: &'static str,
    pub mono: &'static str,
    /// The font's own line height in em (the `lhf-*` factors use it).
    pub natural: f32,
}

pub const SEGOE: &str = "Segoe UI Variable Text";
pub const GEIST: &str = "Geist";
pub const CASCADIA: &str = "Cascadia Mono";
pub const GEIST_MONO: &str = "Geist Mono";

/// Picks the fonts from what is installed: `has` answers whether a font file is there.
pub fn pick_with(has: impl Fn(&str) -> bool) -> Fonts {
    let segoe = has("SegUIVar.ttf");
    Fonts {
        font: if segoe { SEGOE } else { GEIST },
        mono: if has("CascadiaMono.ttf") {
            CASCADIA
        } else {
            GEIST_MONO
        },
        natural: if segoe { 1.35 } else { 1.3 },
    }
}

/// The fonts of this PC: a font file in the Windows fonts folder or in the user's own fonts folder.
pub fn pick() -> Fonts {
    let dirs: Vec<PathBuf> = [
        std::env::var_os("WINDIR").map(|w| PathBuf::from(w).join("Fonts")),
        std::env::var_os("LOCALAPPDATA").map(|l| {
            PathBuf::from(l)
                .join("Microsoft")
                .join("Windows")
                .join("Fonts")
        }),
    ]
    .into_iter()
    .flatten()
    .collect();
    pick_with(|file| dirs.iter().any(|d| d.join(file).is_file()))
}

thread_local! {
    static PICKED: Fonts = pick();
}

/// The fonts of this PC, looked up once per thread.
pub fn current() -> Fonts {
    PICKED.with(|f| *f)
}

/// Sets the fonts and the motion switch on a window (`MainWindow` and the overlay windows).
macro_rules! style_window {
    ($w:expr) => {{
        use slint::ComponentHandle as _;
        let f = $crate::fonts::current();
        let t = $w.global::<$crate::Type>();
        t.set_font(f.font.into());
        t.set_mono(f.mono.into());
        t.set_natural(f.natural);
        $w.global::<$crate::Motion>()
            .set_instant($crate::fonts::instant_motion());
    }};
}
pub(crate) use style_window;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn segoe_and_cascadia_when_installed() {
        let f = pick_with(|_| true);
        assert_eq!((f.font, f.mono), (SEGOE, CASCADIA));
    }

    #[test]
    fn geist_when_missing() {
        let f = pick_with(|_| false);
        assert_eq!((f.font, f.mono), (GEIST, GEIST_MONO));
        assert_eq!(f.natural, 1.3);
    }
}
