//! Test helpers: windows that never show on the user's screen.
//!
//! - [`HiddenDesktop`]: a second Windows desktop that is never switched to. A thread moved there makes windows
//!   only that desktop sees, and `EnumWindows` on such a thread lists them.
//! - [`TestWindow::offscreen`]: a layered, non-activating tool window placed left of every monitor, with its
//!   picture set by `UpdateLayeredWindow`. Windows Graphics Capture cannot capture windows on another desktop
//!   on this PC (docs/arena-spec.md, "Live mode"), but it does capture this window. It is the arena's pattern.
//!
//! Used by the tests of this crate and of dr-app. Not used by the app.

use std::sync::mpsc;
use std::thread::JoinHandle;

use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, SIZE, WPARAM};
use windows::Win32::Graphics::Gdi::{
    AC_SRC_ALPHA, AC_SRC_OVER, BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BLENDFUNCTION,
    CreateCompatibleDC, CreateDIBSection, DIB_RGB_COLORS, DeleteDC, DeleteObject, GdiFlush,
    SelectObject,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::StationsAndDesktops::{
    CloseDesktop, CreateDesktopW, DESKTOP_CONTROL_FLAGS, HDESK, SetThreadDesktop,
};
use windows::Win32::UI::HiDpi::{
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetThreadDpiAwarenessContext,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW,
    GetSystemMetrics, MA_NOACTIVATE, MSG, PostThreadMessageW, RegisterClassW, SM_XVIRTUALSCREEN,
    SM_YVIRTUALSCREEN, SW_SHOWNOACTIVATE, SetTimer, ShowWindow, TranslateMessage, ULW_ALPHA,
    UpdateLayeredWindow, WINDOW_EX_STYLE, WINDOW_STYLE, WM_APP, WM_MOUSEACTIVATE, WM_QUIT,
    WM_TIMER, WNDCLASSW, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_OVERLAPPEDWINDOW,
    WS_POPUP,
};
use windows::core::{HSTRING, PCWSTR, w};

/// A desktop that is never shown. Closed on drop (after its windows are gone).
pub struct HiddenDesktop {
    desk: isize,
    name: String,
}

impl HiddenDesktop {
    pub fn new() -> Result<HiddenDesktop, String> {
        let name = format!("dr-test-{}-{:x}", std::process::id(), unique());
        // GENERIC_ALL, as tools/hidden-run.ps1 asks for.
        let desk = unsafe {
            CreateDesktopW(
                &HSTRING::from(name.as_str()),
                PCWSTR::null(),
                None,
                DESKTOP_CONTROL_FLAGS(0),
                0x1000_0000,
                None,
            )
        }
        .map_err(|e| format!("CreateDesktop: {e}"))?;
        Ok(HiddenDesktop {
            desk: desk.0 as isize,
            name,
        })
    }

    /// The desktop's name: a process started with `STARTUPINFO.lpDesktop` = `WinSta0\<name>` runs on it.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The desktop handle (`HDESK`), for `EnumDesktopWindows`.
    pub fn handle(&self) -> isize {
        self.desk
    }

    /// Runs `f` on a new thread that belongs to this desktop and waits for it.
    pub fn run<T: Send + 'static>(
        &self,
        f: impl FnOnce() -> T + Send + 'static,
    ) -> Result<T, String> {
        let desk = self.desk;
        std::thread::spawn(move || {
            // Must happen before the thread owns any window or hook.
            unsafe { SetThreadDesktop(HDESK(desk as *mut _)) }
                .map_err(|e| format!("SetThreadDesktop: {e}"))?;
            Ok(f())
        })
        .join()
        .map_err(|_| "the test thread panicked".to_string())?
    }
}

impl Drop for HiddenDesktop {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseDesktop(HDESK(self.desk as *mut _));
        }
    }
}

fn unique() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static N: AtomicU64 = AtomicU64::new(0);
    let t = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    t ^ N.fetch_add(1, Ordering::Relaxed).rotate_left(48)
}

/// How a test window looks.
#[derive(Clone, Debug)]
pub enum Kind {
    /// A normal window with a title bar, client area `w` x `h` (physical px), shown without activation.
    Framed { title: String, w: i32, h: i32 },
    /// A window with no frame at (x, y), `w` x `h` (a borderless game when it covers a monitor).
    Popup {
        title: String,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
    },
    /// A layered tool window left of every monitor showing `rgba` (`w` x `h`, straight alpha 255).
    Offscreen { w: u32, h: u32, rgba: Vec<u8> },
    /// As `Offscreen`, but it repaints every `every_ms` (a bright band moves down a dark picture), so Windows
    /// Graphics Capture keeps sending frames (it sends one only when the window changed). Live view tests.
    OffscreenMoving { w: u32, h: u32, every_ms: u32 },
}

/// A window on its own thread, which pumps its messages until the window is dropped.
pub struct TestWindow {
    pub hwnd: isize,
    /// The thread that owns the window and paints it.
    pub thread_id: u32,
    join: Option<JoinHandle<()>>,
}

impl TestWindow {
    /// Makes the window on a new thread, on `desktop` when given (else the normal desktop).
    pub fn spawn(kind: Kind, desktop: Option<&HiddenDesktop>) -> Result<TestWindow, String> {
        if desktop.is_none()
            && !matches!(kind, Kind::Offscreen { .. } | Kind::OffscreenMoving { .. })
        {
            return Err("only off-screen windows may be made on the normal desktop".into());
        }
        let desk = desktop.map(|d| d.desk);
        let (tx, rx) = mpsc::channel::<Result<(isize, u32), String>>();
        let join = std::thread::spawn(move || {
            if let Some(d) = desk
                && let Err(e) = unsafe { SetThreadDesktop(HDESK(d as *mut _)) }
            {
                let _ = tx.send(Err(format!("SetThreadDesktop: {e}")));
                return;
            }
            unsafe {
                let _ = SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
            }
            let hwnd = match make(&kind) {
                Ok(h) => h,
                Err(e) => {
                    let _ = tx.send(Err(e));
                    return;
                }
            };
            let tid = unsafe { windows::Win32::System::Threading::GetCurrentThreadId() };
            let _ = tx.send(Ok((hwnd.0 as isize, tid)));
            let mut msg = MSG::default();
            unsafe {
                while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                    if msg.hwnd.is_invalid() && msg.message == WM_RESIZE_TO {
                        let (w, h) = (msg.wParam.0 as u32, msg.lParam.0 as u32);
                        MOVING.with(|m| {
                            if let Some(m) = m.borrow_mut().as_mut() {
                                *m = Moving::new(m.hwnd, w, h);
                                m.step();
                            }
                        });
                        continue;
                    }
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
                let _ = DestroyWindow(hwnd);
            }
        });
        match rx.recv() {
            Ok(Ok((hwnd, thread_id))) => Ok(TestWindow {
                hwnd,
                thread_id,
                join: Some(join),
            }),
            Ok(Err(e)) => {
                let _ = join.join();
                Err(e)
            }
            Err(_) => Err("the window thread ended early".into()),
        }
    }

    /// An off-screen window whose left half is `left` and right half is `right` (RGBA).
    pub fn offscreen_halves(
        w: u32,
        h: u32,
        left: [u8; 4],
        right: [u8; 4],
    ) -> Result<TestWindow, String> {
        let mut rgba = Vec::with_capacity((w * h * 4) as usize);
        for _y in 0..h {
            for x in 0..w {
                rgba.extend_from_slice(if x < w / 2 { &left } else { &right });
            }
        }
        TestWindow::spawn(Kind::Offscreen { w, h, rgba }, None)
    }
}

impl Drop for TestWindow {
    fn drop(&mut self) {
        unsafe {
            let _ = PostThreadMessageW(self.thread_id, WM_QUIT, WPARAM(0), LPARAM(0));
        }
        if let Some(j) = self.join.take() {
            let _ = j.join();
        }
    }
}

impl TestWindow {
    /// A moving off-screen window (`Kind::OffscreenMoving`) changes to `w` x `h` (a game that changed size).
    pub fn resize(&self, w: u32, h: u32) {
        unsafe {
            let _ = PostThreadMessageW(
                self.thread_id,
                WM_RESIZE_TO,
                WPARAM(w as usize),
                LPARAM(h as isize),
            );
        }
    }
}

/// Thread message of [`TestWindow::resize`].
const WM_RESIZE_TO: u32 = WM_APP + 1;

/// The picture of a moving window, on its thread.
struct Moving {
    hwnd: HWND,
    w: u32,
    h: u32,
    tick: u32,
    rgba: Vec<u8>,
}

thread_local! {
    static MOVING: std::cell::RefCell<Option<Moving>> = const { std::cell::RefCell::new(None) };
}

impl Moving {
    fn new(hwnd: HWND, w: u32, h: u32) -> Moving {
        Moving {
            hwnd,
            w,
            h,
            tick: 0,
            rgba: [24u8, 28, 36, 255].repeat((w * h) as usize),
        }
    }

    /// Moves the band one step and shows the picture.
    fn step(&mut self) {
        let band = (self.h / 16).max(1);
        let rows = |n: u32| (n * band * 7) % self.h.max(1);
        let dark = [24u8, 28, 36, 255];
        let light = [230u8, 220, 120, 255];
        let row_bytes = (self.w * 4) as usize;
        for (y0, px) in [(rows(self.tick), dark), (rows(self.tick + 1), light)] {
            for y in y0..(y0 + band).min(self.h) {
                let r = &mut self.rgba[y as usize * row_bytes..(y as usize + 1) * row_bytes];
                for c in r.as_chunks_mut::<4>().0 {
                    *c = px;
                }
            }
        }
        self.tick += 1;
        let _ = paint_layered(self.hwnd, self.w, self.h, &self.rgba);
    }
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_MOUSEACTIVATE => LRESULT(MA_NOACTIVATE as isize),
        WM_TIMER => {
            MOVING.with(|m| {
                if let Some(m) = m.borrow_mut().as_mut() {
                    m.step();
                }
            });
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wp, lp) },
    }
}

fn make(kind: &Kind) -> Result<HWND, String> {
    unsafe {
        let inst = GetModuleHandleW(None).map_err(|e| e.to_string())?;
        let class = w!("DrRuntimeTestWindow");
        let wc = WNDCLASSW {
            lpfnWndProc: Some(wndproc),
            hInstance: inst.into(),
            lpszClassName: class,
            ..Default::default()
        };
        RegisterClassW(&wc); // 0 when already registered: fine
        let create = |ex: WINDOW_EX_STYLE,
                      title: &str,
                      style: WINDOW_STYLE,
                      x: i32,
                      y: i32,
                      w: i32,
                      h: i32| {
            CreateWindowExW(
                ex,
                class,
                &HSTRING::from(title),
                style,
                x,
                y,
                w,
                h,
                None,
                None,
                Some(inst.into()),
                None,
            )
            .map_err(|e| format!("CreateWindowEx: {e}"))
        };
        match kind {
            Kind::Framed { title, w, h } => {
                // grow the window so the client area is w x h
                let mut r = windows::Win32::Foundation::RECT {
                    left: 0,
                    top: 0,
                    right: *w,
                    bottom: *h,
                };
                let style = WS_OVERLAPPEDWINDOW;
                let _ = windows::Win32::UI::WindowsAndMessaging::AdjustWindowRectEx(
                    &mut r,
                    style,
                    false,
                    WINDOW_EX_STYLE(0),
                );
                let hwnd = create(
                    WINDOW_EX_STYLE(0),
                    title,
                    style,
                    40,
                    40,
                    r.right - r.left,
                    r.bottom - r.top,
                )?;
                let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
                Ok(hwnd)
            }
            Kind::Popup { title, x, y, w, h } => {
                let hwnd = create(WINDOW_EX_STYLE(0), title, WS_POPUP, *x, *y, *w, *h)?;
                let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
                Ok(hwnd)
            }
            Kind::Offscreen { w, h, rgba } => {
                let x = GetSystemMetrics(SM_XVIRTUALSCREEN) - *w as i32 - 200;
                let y = GetSystemMetrics(SM_YVIRTUALSCREEN);
                let hwnd = create(
                    WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW | WS_EX_LAYERED,
                    "dr-runtime offscreen",
                    WS_POPUP,
                    x,
                    y,
                    *w as i32,
                    *h as i32,
                )?;
                if let Err(e) = paint_layered(hwnd, *w, *h, rgba) {
                    let _ = DestroyWindow(hwnd);
                    return Err(e);
                }
                let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
                Ok(hwnd)
            }
            Kind::OffscreenMoving { w, h, every_ms } => {
                let x = GetSystemMetrics(SM_XVIRTUALSCREEN) - *w as i32 - 200;
                let y = GetSystemMetrics(SM_YVIRTUALSCREEN);
                let hwnd = create(
                    WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW | WS_EX_LAYERED,
                    "dr-runtime offscreen moving",
                    WS_POPUP,
                    x,
                    y,
                    *w as i32,
                    *h as i32,
                )?;
                let mut m = Moving::new(hwnd, *w, *h);
                m.step();
                MOVING.with(|s| *s.borrow_mut() = Some(m));
                SetTimer(Some(hwnd), 1, *every_ms, None);
                let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
                Ok(hwnd)
            }
        }
    }
}

/// Puts an RGBA picture into a layered window with `UpdateLayeredWindow` (a DIB section, BGRA).
fn paint_layered(hwnd: HWND, w: u32, h: u32, rgba: &[u8]) -> Result<(), String> {
    let bmi = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: w as i32,
            biHeight: -(h as i32),
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };
    unsafe {
        let dc = CreateCompatibleDC(None);
        let mut bits: *mut std::ffi::c_void = std::ptr::null_mut();
        let bmp =
            CreateDIBSection(Some(dc), &bmi, DIB_RGB_COLORS, &mut bits, None, 0).map_err(|e| {
                let _ = DeleteDC(dc);
                format!("CreateDIBSection: {e}")
            })?;
        let old = SelectObject(dc, bmp.into());
        let dst = std::slice::from_raw_parts_mut(bits as *mut u8, (w * h * 4) as usize);
        for (d, s) in dst
            .as_chunks_mut::<4>()
            .0
            .iter_mut()
            .zip(rgba.as_chunks::<4>().0)
        {
            d.copy_from_slice(&[s[2], s[1], s[0], 255]);
        }
        let _ = GdiFlush();
        let size = SIZE {
            cx: w as i32,
            cy: h as i32,
        };
        let src = POINT { x: 0, y: 0 };
        let blend = BLENDFUNCTION {
            BlendOp: AC_SRC_OVER as u8,
            BlendFlags: 0,
            SourceConstantAlpha: 255,
            AlphaFormat: AC_SRC_ALPHA as u8,
        };
        let r = UpdateLayeredWindow(
            hwnd,
            None,
            None,
            Some(&size),
            Some(dc),
            Some(&src),
            COLORREF(0),
            Some(&blend),
            ULW_ALPHA,
        )
        .map_err(|e| format!("UpdateLayeredWindow: {e}"));
        SelectObject(dc, old);
        let _ = DeleteObject(bmp.into());
        let _ = DeleteDC(dc);
        r
    }
}
