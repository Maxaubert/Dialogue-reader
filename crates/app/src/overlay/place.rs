//! Where the overlay windows go (spec 2.7, Risks "DPI and mixed monitors"). Plain math, unit-tested
//! (`tests/place.rs`).
//!
//! - The game's client rect and capture rect come in physical screen px (`GetClientRect` + `ClientToScreen`, the
//!   DWM frame bounds); the app is per-monitor DPI aware (winit makes it so), so these are real pixels on every
//!   monitor, also left of or above the main one (negative x, y).
//! - Overlay windows are laid out by Slint in logical px. On the game's monitor one logical px is `scale`
//!   physical px (1.0 at 100 %, 1.5 at 150 %). Window positions are set in physical px.
//! - Zone edit covers what the capture covers (for a game with a title bar that is more than the client area), so
//!   its frozen screenshot lines up with the game. [`CaptureMap`] turns its logical px into capture px (the pixels
//!   zones are saved in) and back.

/// A rectangle in physical screen px.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PxRect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl PxRect {
    pub const fn new(x: i32, y: i32, w: i32, h: i32) -> PxRect {
        PxRect { x, y, w, h }
    }

    pub fn right(&self) -> i32 {
        self.x + self.w
    }

    pub fn bottom(&self) -> i32 {
        self.y + self.h
    }

    pub fn is_empty(&self) -> bool {
        self.w <= 0 || self.h <= 0
    }
}

/// What placement needs to know about the game window now.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GameWin {
    /// Client area in screen px.
    pub client: PxRect,
    /// What Windows Graphics Capture covers: the visible window bounds (the frame and title bar too, for a game
    /// with one), in screen px.
    pub frame: PxRect,
    /// Its monitor's scale (1.0 at 100 %).
    pub scale: f32,
    pub minimized: bool,
    /// It (or a window it owns) is the foreground window.
    pub front: bool,
}

impl GameWin {
    /// A borderless game of `w` x `h` at the top left of a 100 % monitor, in front (tests, the testing backend).
    pub fn fake(w: i32, h: i32) -> GameWin {
        let r = PxRect::new(0, 0, w, h);
        GameWin {
            client: r,
            frame: r,
            scale: 1.0,
            minimized: false,
            front: true,
        }
    }
}

/// Where the pill sits (Settings > In-game overlay > Position).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PillAt {
    TopRight,
    BottomCenter,
    Off,
}

impl PillAt {
    /// From the Settings choice (0 top right, 1 bottom center, 2 off).
    pub fn of(i: i32) -> PillAt {
        match i {
            1 => PillAt::BottomCenter,
            2 => PillAt::Off,
            _ => PillAt::TopRight,
        }
    }
}

/// The pill window (logical px, as `PillWindow` in ui/overlays/windows.slint).
pub const PILL_W: f32 = 320.0;
pub const PILL_H: f32 = 48.0;
/// Margin around the pill inside its window (holds the shadow).
pub const PILL_PAD: f32 = 8.0;
/// How far the pill sits from the game's edge (mockup 13: 24 px on a 1440 px wide game).
pub const PILL_INSET: f32 = 24.0;

fn px(v: f32) -> i32 {
    v.round() as i32
}

/// The pill window's position (physical px) over the game's client rect, at the monitor `scale`.
pub fn pill(client: PxRect, scale: f32, at: PillAt) -> (i32, i32) {
    let s = scale.max(0.1);
    match at {
        PillAt::TopRight | PillAt::Off => (
            client.right() - px((PILL_INSET + PILL_W - PILL_PAD) * s),
            client.y + px((PILL_INSET - PILL_PAD) * s),
        ),
        PillAt::BottomCenter => (
            client.x + (client.w - px(PILL_W * s)) / 2,
            client.bottom() - px((PILL_INSET + PILL_H - PILL_PAD) * s),
        ),
    }
}

/// The window position (physical px) that puts the point (`ax`, `ay`) of the window (logical px) on the middle of
/// the game's client rect. The quick menu passes the middle of its card and key hints.
pub fn center_on(client: PxRect, scale: f32, ax: f32, ay: f32) -> (i32, i32) {
    let s = scale.max(0.1);
    let (cx, cy) = (client.x + client.w / 2, client.y + client.h / 2);
    (cx - px(ax * s), cy - px(ay * s))
}

/// Zone edit: its window covers `rect` (physical px, what the capture covers) at the window's `scale`, and shows a
/// screenshot of `shot_w` x `shot_h` capture px stretched over it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CaptureMap {
    pub rect: PxRect,
    pub scale: f32,
    pub shot_w: f32,
    pub shot_h: f32,
}

impl CaptureMap {
    pub fn new(rect: PxRect, scale: f32, shot_w: u32, shot_h: u32) -> CaptureMap {
        CaptureMap {
            rect,
            scale: scale.max(0.1),
            shot_w: shot_w.max(1) as f32,
            shot_h: shot_h.max(1) as f32,
        }
    }

    /// Capture px per logical px, across and down.
    fn per_logical(&self) -> (f32, f32) {
        (
            self.scale * self.shot_w / self.rect.w.max(1) as f32,
            self.scale * self.shot_h / self.rect.h.max(1) as f32,
        )
    }

    /// The window's size in logical px.
    pub fn view_size(&self) -> (f32, f32) {
        (
            self.rect.w as f32 / self.scale,
            self.rect.h as f32 / self.scale,
        )
    }

    /// A point of the window (logical px) in capture px.
    pub fn to_capture(&self, x: f32, y: f32) -> (f32, f32) {
        let (kx, ky) = self.per_logical();
        (x * kx, y * ky)
    }

    /// A point in capture px in the window (logical px).
    pub fn to_view(&self, x: f32, y: f32) -> (f32, f32) {
        let (kx, ky) = self.per_logical();
        (x / kx, y / ky)
    }

    /// Logical px per capture px across (the zone editor's view scale).
    pub fn view_per_capture(&self) -> f32 {
        1.0 / self.per_logical().0
    }
}
