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
//!
//! Reduce motion: when "Show animations in Windows" is off (`SPI_GETCLIENTAREAANIMATION`), movement and scale
//! jump and fades take 80 ms (`Motion.reduce`). Instant motion wins over it, and tests set it with
//! [`set_reduce_motion`] so they never read the PC's own setting.

use std::cell::Cell;
use std::path::PathBuf;

thread_local! {
    static INSTANT: Cell<bool> = const { Cell::new(false) };
    static REDUCE: Cell<Option<bool>> = const { Cell::new(None) };
}

/// Every window made after this on this thread has instant motion (or real motion again with `false`).
pub fn set_instant_motion(on: bool) {
    INSTANT.with(|i| i.set(on));
}

/// Whether windows made now on this thread get instant motion.
pub fn instant_motion() -> bool {
    INSTANT.with(Cell::get)
}

/// Every window made after this on this thread has reduce motion on or off (`Some`), or follows Windows (`None`).
pub fn set_reduce_motion(on: Option<bool>) {
    REDUCE.with(|r| r.set(on));
}

/// Whether windows made now on this thread get reduce motion: never with instant motion; else the setting of
/// [`set_reduce_motion`], else Windows' own.
pub fn reduce_motion() -> bool {
    if instant_motion() {
        return false;
    }
    REDUCE
        .with(Cell::get)
        .unwrap_or_else(windows_animations_off)
}

/// "Show animations in Windows" is off.
#[cfg(windows)]
pub fn windows_animations_off() -> bool {
    use windows::Win32::UI::WindowsAndMessaging::{
        SPI_GETCLIENTAREAANIMATION, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, SystemParametersInfoW,
    };
    use windows::core::BOOL;
    let mut on = BOOL(1);
    // SAFETY: SPI_GETCLIENTAREAANIMATION writes one BOOL to the pointer, which lives for the call.
    let ok = unsafe {
        SystemParametersInfoW(
            SPI_GETCLIENTAREAANIMATION,
            0,
            Some(std::ptr::from_mut(&mut on).cast()),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )
    };
    ok.is_ok() && !on.as_bool()
}

#[cfg(not(windows))]
pub fn windows_animations_off() -> bool {
    false
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
        let m = $w.global::<$crate::Motion>();
        m.set_instant($crate::fonts::instant_motion());
        m.set_reduce($crate::fonts::reduce_motion());
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
