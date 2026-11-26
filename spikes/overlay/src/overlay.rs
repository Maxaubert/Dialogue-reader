//! The part of the spike that M4 can take over: turn a Slint window on winit into an overlay.
//!
//! Three layers, because winit owns `GWL_EXSTYLE`:
//! 1. `attributes_hook` (winit window attributes, before the window exists): no activation on the first
//!    show, no taskbar button.
//! 2. `harden` (right after `show()`, when the HWND exists): adds the extra styles and subclasses the window.
//!    `show_overlay` shows a window and hardens it whenever its HWND is not marked as hardened yet. Use it for every
//!    show, not only the first: Slint's winit backend can destroy the window and make a new HWND (on
//!    `suspend()`, or with `SLINT_DESTROY_WINDOW_ON_HIDE`), and a new HWND starts with winit's styles.
//! 3. The subclass: winit rewrites `GWL_EXSTYLE` from its own flags on every show, hide and level change
//!    (`winit-0.30.13/src/platform_impl/windows/window_state.rs`, `apply_diff`). The subclass puts our bits
//!    back in `WM_STYLECHANGING`, answers `WM_MOUSEACTIVATE` with `MA_NOACTIVATE`, and adds
//!    `SWP_NOACTIVATE` to every move or show.

use std::cell::Cell;

use slint::winit_030::winit::platform::windows::WindowAttributesExtWindows;
use slint::winit_030::winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use slint::winit_030::winit::window::WindowAttributes;
use slint::winit_030::WinitWindowAccessor;
use windows::Win32::Foundation::{COLORREF, GetLastError, HWND, LPARAM, LRESULT, SetLastError, WIN32_ERROR, WPARAM};
use windows::Win32::Foundation::HANDLE;
use windows::Win32::UI::Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass};
use windows::Win32::UI::WindowsAndMessaging::{GetPropW, RemovePropW, SetPropW};
use windows::core::w;
use windows::Win32::UI::WindowsAndMessaging::{
    GWL_EXSTYLE, GetWindowLongPtrW, HWND_TOPMOST, LWA_ALPHA, MA_NOACTIVATE, STYLESTRUCT, SWP_FRAMECHANGED,
    SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SetLayeredWindowAttributes, SetWindowLongPtrW, SetWindowPos,
    WINDOWPOS, WM_MOUSEACTIVATE, WM_NCDESTROY, WM_STYLECHANGING, WM_WINDOWPOSCHANGING, WS_EX_APPWINDOW,
    WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT,
};

/// Which overlay a window is. Decides its extra styles.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Status pill: click-through, never active.
    Pill,
    /// Quick menu: takes mouse clicks (no activation), keys come from the low-level hook.
    Menu,
    /// Zone edit: covers the game, takes the mouse (drag to draw), never active.
    Zone,
}

impl Kind {
    /// The extended styles this overlay must always have.
    pub fn ex_style(self) -> u32 {
        let base = WS_EX_TOPMOST.0 | WS_EX_NOACTIVATE.0 | WS_EX_TOOLWINDOW.0;
        match self {
            Kind::Pill => base | WS_EX_LAYERED.0 | WS_EX_TRANSPARENT.0,
            Kind::Menu | Kind::Zone => base,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Kind::Pill => "pill",
            Kind::Menu => "menu",
            Kind::Zone => "zone",
        }
    }
}

/// How `harden` treats a window. The defaults are the M4 recipe; the switches exist to show what breaks
/// without each part.
#[derive(Clone, Copy, Debug)]
pub struct Opts {
    /// Subclass the window so the styles survive winit's style rewrites. Off = "naive": set the styles once.
    pub subclass: bool,
    /// Call `SetLayeredWindowAttributes(255, LWA_ALPHA)` on layered windows (the pill).
    pub layered_alpha: bool,
    /// Make the quick menu and zone edit layered too (`WS_EX_LAYERED` without `WS_EX_TRANSPARENT`, so they
    /// still take the mouse).
    pub layered_all: bool,
}

impl Default for Opts {
    fn default() -> Self {
        Opts { subclass: true, layered_alpha: true, layered_all: false }
    }
}

thread_local! {
    static NEXT_IS_OVERLAY: Cell<bool> = const { Cell::new(false) };
}

/// winit attributes hook (pass to `BackendSelector::with_winit_window_attributes_hook`). It runs when a
/// Slint component is created (`Component::new`), so `new_overlay` marks the next window as an overlay.
pub fn attributes_hook(a: WindowAttributes) -> WindowAttributes {
    if NEXT_IS_OVERLAY.with(Cell::get) {
        // with_active(false): winit shows the window with SW_SHOWNOACTIVATE the first time.
        a.with_active(false).with_skip_taskbar(true)
    } else {
        a
    }
}

/// Creates a Slint component whose window gets the overlay attributes.
///
/// Only the component's own window gets them. Windows that Slint makes later for it (`PopupWindow`,
/// `ComboBox` or context menu popups) go through the hook without the flag and get default attributes, so
/// overlays must not use popups.
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

/// The Win32 handle of a shown Slint window (None before the first `show()`).
pub fn hwnd_of(w: &slint::Window) -> Option<HWND> {
    w.with_winit_window(|ww| match ww.window_handle().ok()?.as_raw() {
        RawWindowHandle::Win32(h) => Some(HWND(h.hwnd.get() as *mut _)),
        _ => None,
    })
    .flatten()
}

const SUBCLASS_ID: usize = 0x4452_4f56; // "DROV"

/// Window property that marks a hardened HWND. Not `GetWindowSubclass`: with it imported, the exe failed at
/// start with STATUS_ENTRYPOINT_NOT_FOUND (measured; likely the comctl32 it loads, without a v6 manifest, does
/// not export it by name).
const HARDENED_PROP: windows::core::PCWSTR = w!("DialogReader.OverlayHardened");

/// True when `harden` has run on this HWND. A new HWND (Slint made a new window) is not marked.
pub fn is_hardened(hwnd: HWND) -> bool {
    unsafe { !GetPropW(hwnd, HARDENED_PROP).is_invalid() }
}

/// Adds the overlay styles to a shown window. Call it right after `show()` when the HWND is new (see
/// `show_overlay`). Every step is checked: if one fails, the overlay would lose its styles later with no
/// other sign, so the caller must log it or stop.
pub fn harden(hwnd: HWND, kind: Kind, opts: Opts) -> windows::core::Result<()> {
    let want = kind.ex_style() | if opts.layered_all { WS_EX_LAYERED.0 } else { 0 };
    unsafe {
        if opts.subclass && !SetWindowSubclass(hwnd, Some(subclass_proc), SUBCLASS_ID, want as usize).as_bool() {
            return Err(windows::core::Error::from_thread());
        }
        let ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
        let ex = (ex | want) & !WS_EX_APPWINDOW.0;
        // SetWindowLongPtrW returns the old value, which can be 0: only GetLastError tells a failure.
        SetLastError(WIN32_ERROR(0));
        if SetWindowLongPtrW(hwnd, GWL_EXSTYLE, ex as isize) == 0 && GetLastError() != WIN32_ERROR(0) {
            return Err(windows::core::Error::from_thread());
        }
        if want & WS_EX_LAYERED.0 != 0 && opts.layered_alpha {
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
        SetPropW(hwnd, HARDENED_PROP, Some(HANDLE(std::ptr::dangling_mut())) /* any non-null value */)?;
    }
    Ok(())
}

/// What `show_overlay` did.
#[derive(Clone, Copy, Debug)]
pub struct Shown {
    pub hwnd: HWND,
    /// True when this show hardened the window (a new HWND: the first show, or Slint made a new window).
    pub hardened_now: bool,
}

/// Shows an overlay and hardens it when its HWND is not hardened yet. Use it for every show of an overlay.
pub fn show_overlay(w: &slint::Window, kind: Kind, opts: Opts) -> Result<Shown, String> {
    w.show().map_err(|e| format!("show failed: {e}"))?;
    let hwnd = hwnd_of(w).ok_or("no HWND after show (has the event loop started?)")?;
    if is_hardened(hwnd) {
        return Ok(Shown { hwnd, hardened_now: false });
    }
    harden(hwnd, kind, opts).map_err(|e| format!("harden failed: {e}"))?;
    Ok(Shown { hwnd, hardened_now: true })
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
            // winit writes its own idea of the ex-style; keep ours in it.
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
