//! Single instance (spec 2.9, M5): a named mutex `Local\DialogReader.SingleInstance` and a hidden listener window.
//!
//! The first DialogReader holds the mutex and makes a hidden top-level window (class [`CLASS`], title = the instance
//! name). It is top-level, not message-only, because only top-level windows get broadcasts: Explorer's
//! `TaskbarCreated` after it restarts, and the end-of-session messages. The window:
//! - [`WM_SHOW`]: a second start asks for the main window. The second finds the mutex taken, finds this window,
//!   lets the first take the foreground (`AllowSetForegroundWindow`) and posts it. The second then exits.
//! - [`WM_QUIT_APP`]: the installer and uninstaller ask the app to end (installer/DialogReader.iss), so its files
//!   can be replaced or removed.
//! - `WM_ENDSESSION`: Windows signs out or shuts down, or Restart Manager closes the app: the app ends itself.
//! - `TaskbarCreated`: Explorer restarted and the tray is empty: the app makes its tray icon again.
//!
//! [`WM_SHOW`] and [`WM_QUIT_APP`] pass the UIPI filter, so a DialogReader that runs as administrator still gets
//! them from a normal second start or the installer.
//!
//! The window lives on the UI thread, so its messages come through the winit message loop and the handlers run
//! on the UI thread. It is made right after the mutex is taken, before voices and the main window load: a second
//! start then finds it at once. A [`WM_SHOW`] that comes before the show handler is set is kept and runs when the
//! handler is set.
//!
//! `DIALOGREADER_INSTANCE` changes the name (`DialogReader.<value>`): the tests start their own DialogReader under
//! another name, so they never reach a DialogReader that is running for real.

use std::cell::{Cell, RefCell};
use std::time::{Duration, Instant};

use windows::Win32::Foundation::{
    CloseHandle, ERROR_ACCESS_DENIED, ERROR_ALREADY_EXISTS, GetLastError, HANDLE, HWND, LPARAM,
    LRESULT, WPARAM,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::CreateMutexW;
use windows::Win32::UI::WindowsAndMessaging::{
    AllowSetForegroundWindow, ChangeWindowMessageFilterEx, CreateWindowExW, DefWindowProcW,
    DestroyWindow, FindWindowW, GWLP_USERDATA, GetWindowLongPtrW, GetWindowThreadProcessId,
    MSGFLT_ALLOW, PostMessageW, RegisterClassW, RegisterWindowMessageW, SetWindowLongPtrW, WM_APP,
    WM_ENDSESSION, WM_QUERYENDSESSION, WNDCLASSW, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_POPUP,
};
use windows::core::{HSTRING, w};

/// The window class of the listener window.
pub const CLASS: &str = "DialogReader.Instance";
/// The message a second start posts: show the main window.
pub const WM_SHOW: u32 = WM_APP + 0x51;
/// The message the installer and uninstaller post: end the app (as tray Quit does). installer/DialogReader.iss
/// uses the number (32850).
pub const WM_QUIT_APP: u32 = WM_APP + 0x52;
/// The environment variable that renames the instance (tests).
pub const ENV: &str = "DIALOGREADER_INSTANCE";

/// The instance name: `DialogReader`, or `DialogReader.<DIALOGREADER_INSTANCE>`.
pub fn name() -> String {
    name_of(std::env::var(ENV).ok().as_deref())
}

/// [`name`] for a given value of `DIALOGREADER_INSTANCE`.
pub fn name_of(env: Option<&str>) -> String {
    match env.map(str::trim).filter(|v| !v.is_empty()) {
        Some(v) => format!("DialogReader.{v}"),
        None => "DialogReader".into(),
    }
}

/// The mutex name of an instance name.
pub fn mutex_name(name: &str) -> String {
    format!(r"Local\{name}.SingleInstance")
}

/// The message Explorer broadcasts when the taskbar is made again (after Explorer restarts).
pub fn taskbar_created() -> u32 {
    unsafe { RegisterWindowMessageW(w!("TaskbarCreated")) }
}

/// Holds the mutex while the first instance runs.
pub struct Guard(HANDLE);

impl Drop for Guard {
    fn drop(&mut self) {
        if !self.0.is_invalid() {
            unsafe {
                let _ = CloseHandle(self.0);
            }
        }
    }
}

pub enum Claim {
    /// This is the first instance. Keep the guard alive until the app ends.
    First(Guard),
    /// Another instance holds the mutex.
    Second,
}

/// Takes the mutex of `name`. Access denied means another DialogReader made it with other rights (it runs as
/// administrator): that is a second start. Any other failure counts as the first (better two DialogReaders than
/// none).
pub fn claim(name: &str) -> Claim {
    let h = unsafe { CreateMutexW(None, false, &HSTRING::from(mutex_name(name))) };
    match h {
        Ok(h) => {
            if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
                unsafe {
                    let _ = CloseHandle(h);
                }
                Claim::Second
            } else {
                Claim::First(Guard(h))
            }
        }
        Err(e) => claim_failed(e.code()),
    }
}

/// What a `CreateMutexW` failure with `code` means.
pub fn claim_failed(code: windows::core::HRESULT) -> Claim {
    if code == ERROR_ACCESS_DENIED.to_hresult() {
        Claim::Second
    } else {
        Claim::First(Guard(HANDLE::default()))
    }
}

/// The listener window of the instance `name`, if it is there (on this thread's desktop).
pub fn find(name: &str) -> Option<HWND> {
    unsafe { FindWindowW(&HSTRING::from(CLASS), &HSTRING::from(name)) }
        .ok()
        .filter(|h| !h.is_invalid())
}

/// A second start: asks the first instance to show its window. Waits up to `wait` for its listener window (the
/// first may still be starting). True when the message was posted.
pub fn show_first(name: &str, wait: Duration) -> bool {
    let end = Instant::now() + wait;
    loop {
        if let Some(h) = find(name) {
            let mut pid = 0u32;
            unsafe {
                GetWindowThreadProcessId(h, Some(&mut pid));
                if pid != 0 {
                    // the first instance may then bring its window to the front
                    let _ = AllowSetForegroundWindow(pid);
                }
                return PostMessageW(Some(h), WM_SHOW, WPARAM(0), LPARAM(0)).is_ok();
            }
        }
        if Instant::now() >= end {
            return false;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

type Handler = RefCell<Option<Box<dyn Fn()>>>;

/// What the listener window does with its messages. Set through [`Listener`].
#[derive(Default)]
struct Handlers {
    show: Handler,
    /// A show request came before the show handler was set.
    show_pending: Cell<bool>,
    quit: Handler,
    taskbar: Handler,
    taskbar_msg: u32,
}

impl Handlers {
    /// Runs `h`. False when it is not set.
    fn run(h: &Handler) -> bool {
        match h.try_borrow() {
            Ok(f) => match f.as_ref() {
                Some(f) => {
                    f();
                    true
                }
                None => false,
            },
            Err(_) => false,
        }
    }
}

/// The listener window of the first instance. Destroyed on drop.
pub struct Listener {
    hwnd: HWND,
    handlers: Box<Handlers>,
}

impl Listener {
    pub fn hwnd(&self) -> HWND {
        self.hwnd
    }

    /// A second start asks for the main window. A request that came before this runs now.
    pub fn on_show(&self, f: impl Fn() + 'static) {
        *self.handlers.show.borrow_mut() = Some(Box::new(f));
        if self.handlers.show_pending.replace(false) {
            Handlers::run(&self.handlers.show);
        }
    }

    /// The app is asked to end: the installer or uninstaller ([`WM_QUIT_APP`]), or the end of the Windows session
    /// (`WM_ENDSESSION`). Windows may end the process soon after the handler returns, so it saves what must be
    /// saved before it returns.
    pub fn on_quit(&self, f: impl Fn() + 'static) {
        *self.handlers.quit.borrow_mut() = Some(Box::new(f));
    }

    /// Explorer restarted (`TaskbarCreated`): the tray icon must be made again.
    pub fn on_taskbar_created(&self, f: impl Fn() + 'static) {
        *self.handlers.taskbar.borrow_mut() = Some(Box::new(f));
    }
}

impl Drop for Listener {
    fn drop(&mut self) {
        unsafe {
            SetWindowLongPtrW(self.hwnd, GWLP_USERDATA, 0);
            let _ = DestroyWindow(self.hwnd);
        }
    }
}

/// Makes the hidden listener window of `name` on this thread. The handlers run on this thread; it must run a
/// message loop (the Slint event loop does).
pub fn listen(name: &str) -> Result<Listener, String> {
    unsafe {
        let module = GetModuleHandleW(None).map_err(|e| format!("GetModuleHandle: {e}"))?;
        let class = HSTRING::from(CLASS);
        let wc = WNDCLASSW {
            lpfnWndProc: Some(wndproc),
            hInstance: module.into(),
            lpszClassName: windows::core::PCWSTR(class.as_ptr()),
            ..Default::default()
        };
        // a second Listener in one process (tests) finds the class registered already
        RegisterClassW(&wc);
        let handlers = Box::new(Handlers {
            taskbar_msg: taskbar_created(),
            ..Default::default()
        });
        // hidden (no WS_VISIBLE), no taskbar button, never activated, no parent: a top-level window
        let hwnd = CreateWindowExW(
            WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
            &class,
            &HSTRING::from(name),
            WS_POPUP,
            0,
            0,
            0,
            0,
            None,
            None,
            Some(module.into()),
            None,
        )
        .map_err(|e| format!("CreateWindowEx: {e}"))?;
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, &*handlers as *const Handlers as isize);
        // a DialogReader that runs as administrator still hears a normal second start, the installer, and Explorer
        for m in [WM_SHOW, WM_QUIT_APP, handlers.taskbar_msg] {
            if m != 0 {
                let _ = ChangeWindowMessageFilterEx(hwnd, m, MSGFLT_ALLOW, None);
            }
        }
        Ok(Listener { hwnd, handlers })
    }
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    let ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *const Handlers;
    if !ptr.is_null() {
        let h = unsafe { &*ptr };
        match msg {
            WM_SHOW => {
                if !Handlers::run(&h.show) {
                    h.show_pending.set(true);
                }
                return LRESULT(0);
            }
            WM_QUIT_APP => {
                Handlers::run(&h.quit);
                return LRESULT(0);
            }
            // the app may end: yes
            WM_QUERYENDSESSION => return LRESULT(1),
            WM_ENDSESSION => {
                if wp.0 != 0 {
                    Handlers::run(&h.quit);
                }
                return LRESULT(0);
            }
            m if m != 0 && m == h.taskbar_msg => {
                Handlers::run(&h.taskbar);
                return LRESULT(0);
            }
            _ => {}
        }
    }
    unsafe { DefWindowProcW(hwnd, msg, wp, lp) }
}
