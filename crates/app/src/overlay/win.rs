//! Win32 side of the overlay windows: the recipe of docs/overlay-spike.md (taken over from
//! `spikes/overlay/src/overlay.rs`) and the few window facts placement needs.
//!
//! Three layers, because winit owns `GWL_EXSTYLE`:
//! 1. [`attributes_hook`] (winit window attributes, before the window exists): no activation on the first show, no
//!    taskbar button. Only windows made inside [`new_overlay`] get it; the main window keeps the defaults.
//! 2. [`harden`] right after a `show()` that gives a new HWND ([`show_overlay`] does it on every show): the extra
//!    styles, `SetLayeredWindowAttributes(255)` for the pill, topmost without activation, a window property that
//!    marks the HWND, and the subclass. Every call is checked; a failure comes back as an error to log.
//! 3. The subclass: winit rewrites `GWL_EXSTYLE` on every show, hide and level change; the subclass puts the bits
//!    back in `WM_STYLECHANGING`, answers `WM_MOUSEACTIVATE` with `MA_NOACTIVATE`, and adds `SWP_NOACTIVATE` to
//!    every move or show.
//!
//! Overlays use no popups (`PopupWindow`, `ComboBox`): those would be made without the overlay attributes.

use std::cell::Cell;

use slint::winit_030::WinitWindowAccessor;
use slint::winit_030::winit::platform::windows::WindowAttributesExtWindows;
use slint::winit_030::winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use slint::winit_030::winit::window::WindowAttributes;
use windows::Win32::Foundation::{
    COLORREF, GetLastError, HANDLE, HWND, LPARAM, LRESULT, POINT, RECT, SetLastError, WIN32_ERROR,
    WPARAM,
};
use windows::Win32::Graphics::Dwm::{DWMWA_EXTENDED_FRAME_BOUNDS, DwmGetWindowAttribute};
use windows::Win32::Graphics::Gdi::{ClientToScreen, MONITOR_DEFAULTTONEAREST, MonitorFromWindow};
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
use windows::Win32::UI::Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass};
use windows::Win32::UI::WindowsAndMessaging::{
    GA_ROOTOWNER, GWL_EXSTYLE, GetAncestor, GetClientRect, GetForegroundWindow, GetPropW,
    GetWindowLongPtrW, HWND_TOPMOST, IsIconic, IsWindow, LWA_ALPHA, MA_NOACTIVATE, RemovePropW,
    STYLESTRUCT, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
    SetLayeredWindowAttributes, SetPropW, SetWindowLongPtrW, SetWindowPos, WINDOWPOS,
    WM_MOUSEACTIVATE, WM_NCDESTROY, WM_STYLECHANGING, WM_WINDOWPOSCHANGING, WS_EX_APPWINDOW,
    WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT,
};
use windows::core::w;

use super::place::{GameWin, PxRect};

/// Which overlay a window is. Decides its extra styles.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Status pill: click-through, never active.
    Pill,
    /// Quick menu: takes mouse clicks (no activation); keys come from the low-level hook.
    Menu,
    /// Zone edit: covers the game, takes the mouse (drag to draw), never active.
    Zone,
}

impl Kind {
    /// The extended styles this overlay always has.
    pub fn ex_style(self) -> u32 {
        let base = WS_EX_TOPMOST.0 | WS_EX_NOACTIVATE.0 | WS_EX_TOOLWINDOW.0;
        match self {
            Kind::Pill => base | WS_EX_LAYERED.0 | WS_EX_TRANSPARENT.0,
            Kind::Menu | Kind::Zone => base,
        }
    }
}

thread_local! {
    static NEXT_IS_OVERLAY: Cell<bool> = const { Cell::new(false) };
}

/// winit attributes hook (`BackendSelector::with_winit_window_attributes_hook`). It runs when a Slint component is
/// created, so [`new_overlay`] marks the next window as an overlay.
pub fn attributes_hook(a: WindowAttributes) -> WindowAttributes {
    if NEXT_IS_OVERLAY.with(Cell::get) {
        // with_active(false): winit shows the window with SW_SHOWNOACTIVATE the first time
        a.with_active(false).with_skip_taskbar(true)
    } else {
        a
    }
}

/// Creates a Slint component whose own window gets the overlay attributes.
pub fn new_overlay<T>(create: impl FnOnce() -> T) -> T {
    /// Clears the flag even if `create` panics, so the next normal window is not made an overlay.
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            NEXT_IS_OVERLAY.with(|c| c.set(false));
        }
    }
    NEXT_IS_OVERLAY.with(|c| c.set(true));
    let _reset = Reset;
    create()
}

/// The Win32 handle of a shown Slint window (None before the first `show()`, and on the testing backend).
pub fn hwnd_of(w: &slint::Window) -> Option<HWND> {
    w.with_winit_window(|ww| match ww.window_handle().ok()?.as_raw() {
        RawWindowHandle::Win32(h) => Some(HWND(h.hwnd.get() as *mut _)),
        _ => None,
    })
    .flatten()
}

const SUBCLASS_ID: usize = 0x4452_4f56; // "DROV"

/// Window property that marks a hardened HWND. Not `GetWindowSubclass`: with it imported, the spike's exe failed
/// at start with STATUS_ENTRYPOINT_NOT_FOUND (docs/overlay-spike.md).
const HARDENED_PROP: windows::core::PCWSTR = w!("DialogReader.OverlayHardened");

/// True when [`harden`] has run on this HWND. A new HWND (Slint made a new window) is not marked.
pub fn is_hardened(hwnd: HWND) -> bool {
    unsafe { !GetPropW(hwnd, HARDENED_PROP).is_invalid() }
}

/// The extended style of a window.
pub fn ex_style(hwnd: HWND) -> u32 {
    unsafe { GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32 }
}

/// Adds the overlay styles to a shown window and subclasses it.
pub fn harden(hwnd: HWND, kind: Kind) -> windows::core::Result<()> {
    let want = kind.ex_style();
    unsafe {
        if !SetWindowSubclass(hwnd, Some(subclass_proc), SUBCLASS_ID, want as usize).as_bool() {
            return Err(windows::core::Error::from_thread());
        }
        let ex = (GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32 | want) & !WS_EX_APPWINDOW.0;
        // SetWindowLongPtrW returns the old value, which can be 0: only GetLastError tells a failure
        SetLastError(WIN32_ERROR(0));
        if SetWindowLongPtrW(hwnd, GWL_EXSTYLE, ex as isize) == 0
            && GetLastError() != WIN32_ERROR(0)
        {
            return Err(windows::core::Error::from_thread());
        }
        if want & WS_EX_LAYERED.0 != 0 {
            // a layered window is not drawn until it gets SetLayeredWindowAttributes or UpdateLayeredWindow
            SetLayeredWindowAttributes(hwnd, COLORREF(0), 255, LWA_ALPHA)?;
        }
        SetWindowPos(
            hwnd,
            Some(HWND_TOPMOST),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_FRAMECHANGED,
        )?;
        // any non-null value
        SetPropW(hwnd, HARDENED_PROP, Some(HANDLE(std::ptr::dangling_mut())))?;
    }
    Ok(())
}

/// What [`show_overlay`] did.
#[derive(Clone, Copy, Debug)]
pub struct Shown {
    pub hwnd: HWND,
    /// This show hardened the window (a new HWND: the first show, or Slint made a new window).
    pub hardened_now: bool,
}

/// Shows an overlay and hardens it when its HWND is not hardened yet. Use it for every show of an overlay.
pub fn show_overlay(w: &slint::Window, kind: Kind) -> Result<Shown, String> {
    w.show().map_err(|e| format!("show failed: {e}"))?;
    let hwnd = hwnd_of(w).ok_or("no window handle after show")?;
    if is_hardened(hwnd) {
        return Ok(Shown {
            hwnd,
            hardened_now: false,
        });
    }
    harden(hwnd, kind).map_err(|e| format!("overlay styles not set: {e}"))?;
    Ok(Shown {
        hwnd,
        hardened_now: true,
    })
}

unsafe extern "system" fn subclass_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _id: usize,
    want: usize,
) -> LRESULT {
    match msg {
        WM_STYLECHANGING if wparam.0 as i32 == GWL_EXSTYLE.0 => {
            // winit writes its own idea of the ex-style; keep ours in it
            let ss = unsafe { &mut *(lparam.0 as *mut STYLESTRUCT) };
            ss.styleNew = (ss.styleNew | want as u32) & !WS_EX_APPWINDOW.0;
        }
        WM_MOUSEACTIVATE => return LRESULT(MA_NOACTIVATE as isize),
        WM_WINDOWPOSCHANGING => {
            let wp = unsafe { &mut *(lparam.0 as *mut WINDOWPOS) };
            wp.flags |= SWP_NOACTIVATE;
        }
        WM_NCDESTROY => unsafe {
            let _ = RemovePropW(hwnd, HARDENED_PROP);
            let _ = RemoveWindowSubclass(hwnd, Some(subclass_proc), SUBCLASS_ID);
        },
        _ => {}
    }
    unsafe { DefSubclassProc(hwnd, msg, wparam, lparam) }
}

fn hwnd(h: isize) -> HWND {
    HWND(h as *mut _)
}

/// Reads the game window's place, monitor scale and state.
pub fn game_win(h: isize) -> Option<GameWin> {
    let g = hwnd(h);
    unsafe {
        if !IsWindow(Some(g)).as_bool() {
            return None;
        }
        let mut c = RECT::default();
        GetClientRect(g, &mut c).ok()?;
        let mut p = POINT::default();
        let _ = ClientToScreen(g, &mut p);
        let client = PxRect::new(p.x, p.y, c.right - c.left, c.bottom - c.top);
        let mut f = RECT::default();
        let frame = if DwmGetWindowAttribute(
            g,
            DWMWA_EXTENDED_FRAME_BOUNDS,
            &mut f as *mut RECT as *mut _,
            size_of::<RECT>() as u32,
        )
        .is_ok()
        {
            PxRect::new(f.left, f.top, f.right - f.left, f.bottom - f.top)
        } else {
            client
        };
        let mon = MonitorFromWindow(g, MONITOR_DEFAULTTONEAREST);
        let (mut dx, mut dy) = (96u32, 96u32);
        let scale = if GetDpiForMonitor(mon, MDT_EFFECTIVE_DPI, &mut dx, &mut dy).is_ok() {
            dx as f32 / 96.0
        } else {
            1.0
        };
        Some(GameWin {
            client,
            frame,
            scale,
            minimized: IsIconic(g).as_bool(),
            front: is_front(h),
        })
    }
}

/// `game` is the foreground window, or owns it.
pub fn is_front(game: isize) -> bool {
    unsafe {
        let fg = GetForegroundWindow();
        !fg.is_invalid() && (fg == hwnd(game) || GetAncestor(fg, GA_ROOTOWNER) == hwnd(game))
    }
}

/// Brings a window of this app to the front (Settings in the quick menu). Windows lets a process take the
/// foreground only in some cases; joining the input of the window in front first is the usual way.
pub fn bring_to_front(w: &slint::Window) {
    use windows::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
    use windows::Win32::UI::Input::KeyboardAndMouse::SetFocus;
    use windows::Win32::UI::WindowsAndMessaging::{
        BringWindowToTop, GetWindowThreadProcessId, SW_RESTORE, SetForegroundWindow, ShowWindow,
    };
    let Some(me) = hwnd_of(w) else { return };
    unsafe {
        let fg = GetForegroundWindow();
        let fg_thread = GetWindowThreadProcessId(fg, None);
        let mine = GetCurrentThreadId();
        let join = fg_thread != 0 && fg_thread != mine;
        if join {
            let _ = AttachThreadInput(fg_thread, mine, true);
        }
        let _ = ShowWindow(me, SW_RESTORE);
        let _ = BringWindowToTop(me);
        let _ = SetForegroundWindow(me);
        let _ = SetFocus(Some(me));
        if join {
            let _ = AttachThreadInput(fg_thread, mine, false);
        }
    }
}

/// Windows 11 round corners on the main window (Max, 2026-11-26), with DWM's own 1 px border in the app's line
/// color (`Palette.line-2` on black, #242424), so the border follows the curve. DWM drops both while the window
/// is maximized. Windows 10 has neither attribute: the calls fail there and nothing changes.
pub fn round_corners(w: &slint::Window) {
    use windows::Win32::Graphics::Dwm::{
        DWMWA_BORDER_COLOR, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND, DwmSetWindowAttribute,
    };
    let Some(h) = hwnd_of(w) else { return };
    let pref = DWMWCP_ROUND;
    let color = COLORREF(0x0024_2424);
    unsafe {
        let _ = DwmSetWindowAttribute(
            h,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            std::ptr::from_ref(&pref).cast(),
            std::mem::size_of_val(&pref) as u32,
        );
        let _ = DwmSetWindowAttribute(
            h,
            DWMWA_BORDER_COLOR,
            std::ptr::from_ref(&color).cast(),
            std::mem::size_of_val(&color) as u32,
        );
    }
}
