//! The in-game overlays (M4, spec 2.7): the status pill, the quick menu and zone edit as real windows over the
//! game. The view model drives them (`vm/overlays.rs`); this module owns the windows.
//!
//! - [`Overlays::new`] makes the three windows. In the app ([`Overlays::new`] with `win32`) they are overlay windows
//!   on winit: `win::new_overlay` gives them no activation and no taskbar button, and every show goes through
//!   `win::show_overlay` (extra styles, subclass; docs/overlay-spike.md). On the Slint testing backend (`win32`
//!   off) they are plain test windows, placed nowhere, and the game is [`GameWin::fake`].
//! - Placement: `place.rs` (client rect, monitor scale). Quick menu: `quick_menu.rs`. Zone edit: `zone_edit.rs`.
//! - Only `main.rs` makes them with `win32` (after the winit backend is selected with `win::attributes_hook`), so
//!   no test or headless render shows a window on the desktop. `render-screens` keeps the previews in the main
//!   window (13, 15, 16), which use the same components.
//! - Show and hide fade (README motion table, 9 to 11). Each window has an `open` property: true fades it in, false
//!   fades it out. The state here (`shown`, `pill_shown`) changes at once; only the Win32 hide waits until the fade
//!   ends ([`Fade`]). While it fades out the window keeps its overlay styles (the pill stays click-through, none
//!   takes focus) and the menu and zone edit take no clicks. With instant motion (tests, renders) the hide is at
//!   once, as the testing backend runs no timers.

pub mod place;
pub mod quick_menu;
#[cfg(windows)]
pub mod win;
pub mod zone_edit;

use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

use dr_runtime::keys::Overlay;
use slint::{ComponentHandle, ModelRc, PhysicalPosition, PhysicalSize, VecModel};

use crate::{EditZone, PillWindow, QuickMenuWindow, VoiceData, ZoneEditWindow};
use place::{CaptureMap, GameWin, PillAt};

pub struct Overlays {
    pub pill: PillWindow,
    pub menu: QuickMenuWindow,
    pub zones: ZoneEditWindow,
    /// Real overlay windows on winit: styles and placement through Win32.
    win32: bool,
    /// The game as the tests see it (the testing backend has no game window).
    pub fake_game: Option<GameWin>,
    /// The menu or zone edit on screen now.
    pub shown: Overlay,
    pub pill_shown: bool,
    /// Reading is held for an open overlay.
    pub held: bool,
    pub state: quick_menu::Menu,
    /// Zone edit: its window and screenshot, in capture px.
    pub map: Option<CaptureMap>,
    pub voices: Rc<VecModel<VoiceData>>,
    pub edit_zones: Rc<VecModel<EditZone>>,
    /// Where each window was put last (physical px), to move it only when the game moved.
    pill_at: Option<(i32, i32)>,
    menu_at: Option<(i32, i32)>,
    /// What went wrong last when showing an overlay (written to the log once).
    pub last_error: Option<String>,
    /// The app is quitting: nothing shows any more.
    pub closed: bool,
    /// The delayed hides of the pill, the menu and zone edit.
    fade: [Fade; 3],
    /// Instant motion (tests, renders): hide at once.
    instant: bool,
}

/// How long each window takes to fade out (README motion table): the pill 200 ms, the menu 120 ms, zone edit
/// 140 ms. The hide waits a little longer, so the last frame is drawn.
pub const PILL_OUT: Duration = Duration::from_millis(200);
pub const MENU_OUT: Duration = Duration::from_millis(120);
pub const ZONE_OUT: Duration = Duration::from_millis(140);
const HIDE_SLACK: Duration = Duration::from_millis(30);

/// The delayed hide of one window. Each show or hide counts up `turn`; a timer hides the window only when no
/// show or other hide came after it, so a window shown again while it fades stays.
#[derive(Default, Clone)]
struct Fade {
    turn: Rc<Cell<u64>>,
}

impl Fade {
    /// A show: a pending hide is void.
    fn cancel(&self) {
        self.turn.set(self.turn.get() + 1);
    }

    /// Hides `c` after `after`, unless a show or hide comes first. `instant`: at once.
    fn hide_later<C: ComponentHandle + 'static>(&self, c: &C, after: Duration, instant: bool) {
        self.cancel();
        if instant {
            let _ = c.hide();
            return;
        }
        let (turn, mine, weak) = (self.turn.clone(), self.turn.get(), c.as_weak());
        slint::Timer::single_shot(after + HIDE_SLACK, move || {
            if turn.get() == mine
                && let Some(c) = weak.upgrade()
            {
                let _ = c.hide();
            }
        });
    }
}

impl Overlays {
    /// Makes the three windows (not shown). `win32`: real overlay windows (only `main.rs`, after selecting the
    /// winit backend with the attributes hook).
    pub fn new(win32: bool) -> Result<Overlays, slint::PlatformError> {
        #[cfg(windows)]
        let make = |f: &dyn Fn() -> Result<(), slint::PlatformError>| {
            if win32 { win::new_overlay(f) } else { f() }
        };
        #[cfg(not(windows))]
        let make = |f: &dyn Fn() -> Result<(), slint::PlatformError>| f();
        let pill = std::cell::RefCell::new(None);
        let menu = std::cell::RefCell::new(None);
        let zones = std::cell::RefCell::new(None);
        make(&|| {
            *pill.borrow_mut() = Some(PillWindow::new()?);
            Ok(())
        })?;
        make(&|| {
            *menu.borrow_mut() = Some(QuickMenuWindow::new()?);
            Ok(())
        })?;
        make(&|| {
            *zones.borrow_mut() = Some(ZoneEditWindow::new()?);
            Ok(())
        })?;
        let (Some(pill), Some(menu), Some(zones)) =
            (pill.into_inner(), menu.into_inner(), zones.into_inner())
        else {
            return Err(slint::PlatformError::Other(
                "overlay windows not made".into(),
            ));
        };
        crate::fonts::style_window!(pill);
        crate::fonts::style_window!(menu);
        crate::fonts::style_window!(zones);
        let voices = Rc::new(VecModel::default());
        let edit_zones = Rc::new(VecModel::default());
        menu.set_voices(ModelRc::from(voices.clone()));
        zones.set_zones(ModelRc::from(edit_zones.clone()));
        Ok(Overlays {
            pill,
            menu,
            zones,
            win32,
            fake_game: (!win32).then(|| GameWin::fake(1920, 1080)),
            shown: Overlay::None,
            pill_shown: false,
            held: false,
            state: quick_menu::Menu::default(),
            map: None,
            voices,
            edit_zones,
            pill_at: None,
            menu_at: None,
            last_error: None,
            closed: false,
            fade: Default::default(),
            instant: crate::fonts::instant_motion(),
        })
    }

    /// Real overlay windows (the app), not the testing backend.
    pub fn is_win32(&self) -> bool {
        self.win32
    }

    /// The game window now: its place, monitor scale, and whether it is in front. None when it is gone.
    pub fn game(&self, hwnd: Option<isize>) -> Option<GameWin> {
        if !self.win32 {
            return hwnd.and(self.fake_game);
        }
        #[cfg(windows)]
        {
            hwnd.and_then(win::game_win)
        }
        #[cfg(not(windows))]
        {
            None
        }
    }

    /// Shows a window; a failure is kept in `last_error` (the view model logs it).
    fn show(&mut self, which: Kind) -> bool {
        let w = match which {
            Kind::Pill => self.pill.window(),
            Kind::Menu => self.menu.window(),
            Kind::Zone => self.zones.window(),
        };
        match show_window(self.win32, w, which) {
            Ok(()) => true,
            Err(e) => {
                self.last_error = Some(e);
                false
            }
        }
    }

    /// Shows or hides the pill at its place over the game.
    pub fn pill_on(&mut self, on: bool, g: Option<&GameWin>, at: PillAt) {
        let on = on && at != PillAt::Off && g.is_some();
        if !on {
            if self.pill_shown {
                self.pill.set_open(false);
                self.fade[0].hide_later(&self.pill, PILL_OUT, self.instant);
                self.pill_shown = false;
            }
            return;
        }
        let Some(g) = g else { return };
        self.pill
            .set_align(if at == PillAt::BottomCenter { 1 } else { 0 });
        let pos = place::pill(g.client, g.scale, at);
        if !self.pill_shown {
            // its place before the first frame, so it never shows somewhere else
            self.pill
                .window()
                .set_position(PhysicalPosition::new(pos.0, pos.1));
            self.fade[0].cancel();
            if !self.show(Kind::Pill) {
                return;
            }
            self.pill.set_open(true);
            self.pill_shown = true;
            self.pill_at = None;
        }
        if self.pill_at != Some(pos) {
            self.pill
                .window()
                .set_position(PhysicalPosition::new(pos.0, pos.1));
            self.pill_at = Some(pos);
        }
    }

    /// Shows the quick menu centered on the game (or only moves it with the game).
    pub fn menu_on(&mut self, g: &GameWin) -> bool {
        let pos = |m: &QuickMenuWindow| {
            place::center_on(
                g.client,
                g.scale,
                m.window().size().width as f32 / m.window().scale_factor() / 2.0,
                m.get_group_cy(),
            )
        };
        if self.shown != Overlay::Menu {
            let p = pos(&self.menu);
            self.menu
                .window()
                .set_position(PhysicalPosition::new(p.0, p.1));
            self.fade[1].cancel();
            if !self.show(Kind::Menu) {
                return false;
            }
            self.menu.set_open(true);
            self.menu_at = None;
        }
        // its size at this monitor's scale is known after show
        let p = pos(&self.menu);
        if self.menu_at != Some(p) {
            self.menu
                .window()
                .set_position(PhysicalPosition::new(p.0, p.1));
            self.menu_at = Some(p);
        }
        true
    }

    /// Fades the menu out; its window hides when the fade ends.
    pub fn menu_off(&mut self) {
        if self.menu.get_open() {
            self.menu.set_open(false);
            self.fade[1].hide_later(&self.menu, MENU_OUT, self.instant);
        }
    }

    /// Shows zone edit over what the capture covers.
    pub fn zones_on(&mut self, g: &GameWin) -> bool {
        let r = g.frame;
        let w = self.zones.window();
        w.set_position(PhysicalPosition::new(r.x, r.y));
        w.set_size(PhysicalSize::new(r.w.max(1) as u32, r.h.max(1) as u32));
        self.fade[2].cancel();
        if !self.show(Kind::Zone) {
            return false;
        }
        self.zones.set_open(true);
        // a move to another monitor may have changed the size: set it again
        let w = self.zones.window();
        w.set_position(PhysicalPosition::new(r.x, r.y));
        w.set_size(PhysicalSize::new(r.w.max(1) as u32, r.h.max(1) as u32));
        true
    }

    /// Fades zone edit out; its window hides when the fade ends.
    pub fn zones_off(&mut self) {
        self.zones.set_draft(false);
        if self.zones.get_open() {
            self.zones.set_open(false);
            self.fade[2].hide_later(&self.zones, ZONE_OUT, self.instant);
        }
    }

    /// The scale of the zone edit window (its monitor's), once shown.
    pub fn zones_scale(&self) -> f32 {
        self.zones.window().scale_factor()
    }

    /// Hides every overlay for good (the app quits).
    pub fn hide_all(&mut self) {
        self.closed = true;
        for f in &self.fade {
            f.cancel();
        }
        self.pill.set_open(false);
        self.menu.set_open(false);
        self.zones.set_open(false);
        let _ = self.pill.hide();
        let _ = self.menu.hide();
        let _ = self.zones.hide();
        self.pill_shown = false;
        self.shown = Overlay::None;
    }
}

/// Which window [`Overlays::show`] shows (its Win32 styles).
#[derive(Clone, Copy)]
enum Kind {
    Pill,
    Menu,
    Zone,
}

#[cfg(windows)]
impl Kind {
    fn win(self) -> win::Kind {
        match self {
            Kind::Pill => win::Kind::Pill,
            Kind::Menu => win::Kind::Menu,
            Kind::Zone => win::Kind::Zone,
        }
    }
}

/// Shows an overlay window: through the overlay recipe on winit (`win32`), plainly on the testing backend.
fn show_window(win32: bool, w: &slint::Window, which: Kind) -> Result<(), String> {
    #[cfg(windows)]
    if win32 {
        return win::show_overlay(w, which.win()).map(|_| ());
    }
    let _ = (win32, which);
    w.show().map_err(|e| e.to_string())
}
