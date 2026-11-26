//! Win32 test helpers: the dummy game, the hidden desktop runner and the checks.

use std::path::Path;
use std::time::{Duration, Instant};

use windows::Win32::Foundation::{CloseHandle, HWND, LPARAM, LRESULT, POINT, RECT, WAIT_OBJECT_0, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BitBlt, ClientToScreen, CreateCompatibleDC, CreateDIBSection,
    CreateSolidBrush, DIB_RGB_COLORS, DeleteDC, DeleteObject, GetDC, HBRUSH, ReleaseDC, SRCCOPY, SelectObject,
};
use windows::Win32::Graphics::OpenGL::GetPixelFormat;
use windows::Win32::Storage::Xps::{PRINT_WINDOW_FLAGS, PrintWindow};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::StationsAndDesktops::{
    CloseDesktop, CreateDesktopW, DESKTOP_CONTROL_FLAGS, EnumDesktopWindows, HDESK,
};
use windows::Win32::System::Threading::{
    CREATE_NO_WINDOW, CreateProcessW, PROCESS_INFORMATION, STARTUPINFOW, TerminateProcess, WaitForSingleObject,
};
use windows::Win32::UI::WindowsAndMessaging::{
    ASFW_ANY, AllowSetForegroundWindow, CS_HREDRAW, CS_VREDRAW, CreateWindowExW, DefWindowProcW, DispatchMessageW,
    FindWindowW, GA_ROOT, GWL_EXSTYLE, GetAncestor, GetClassNameW, GetClientRect, GetForegroundWindow, GetMessageW,
    GetWindowLongPtrW, GetWindowRect, IDC_ARROW, KillTimer, LAYERED_WINDOW_ATTRIBUTES_FLAGS, LoadCursorW, MSG,
    PostQuitMessage, RegisterClassW, SW_SHOW, SetForegroundWindow, SetTimer, ShowWindow, TranslateMessage,
    WINDOW_EX_STYLE, WM_DESTROY, WM_TIMER, WNDCLASSW, WS_OVERLAPPEDWINDOW, WS_VISIBLE, WindowFromPoint,
};
use windows::core::{PCWSTR, PWSTR, w};

pub const GAME_CLASS: PCWSTR = w!("DrSpikeGame");
/// The dummy game's color (BGR in COLORREF): RGB(32, 128, 64).
pub const GAME_RGB: (u8, u8, u8) = (32, 128, 64);

fn colorref(r: u8, g: u8, b: u8) -> windows::Win32::Foundation::COLORREF {
    windows::Win32::Foundation::COLORREF(r as u32 | (g as u32) << 8 | (b as u32) << 16)
}

/// Runs the dummy game: a plain green window at (0, 0) 1280 x 720 that activates itself (on a hidden
/// desktop nothing is ever the foreground; see the activation spy below). It lets any
/// process take the foreground (`AllowSetForegroundWindow(ASFW_ANY)`), so the overlay test is as strict
/// as it can be: if an overlay could activate, nothing stops it. Exits after `secs` seconds.
pub fn run_game(secs: u32) -> windows::core::Result<()> {
    unsafe {
        let inst = GetModuleHandleW(None)?;
        let wc = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(game_proc),
            hInstance: inst.into(),
            hCursor: LoadCursorW(None, IDC_ARROW)?,
            hbrBackground: HBRUSH(CreateSolidBrush(colorref(GAME_RGB.0, GAME_RGB.1, GAME_RGB.2)).0),
            lpszClassName: GAME_CLASS,
            ..Default::default()
        };
        RegisterClassW(&wc);
        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            GAME_CLASS,
            w!("DR spike game"),
            WS_OVERLAPPEDWINDOW | WS_VISIBLE,
            0,
            0,
            1280,
            720,
            None,
            None,
            Some(inst.into()),
            None,
        )?;
        let _ = ShowWindow(hwnd, SW_SHOW);
        let _ = SetForegroundWindow(hwnd);
        let _ = AllowSetForegroundWindow(ASFW_ANY);
        SetTimer(Some(hwnd), 1, secs * 1000, None);
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        let _ = KillTimer(Some(hwnd), 1);
    }
    Ok(())
}

unsafe extern "system" fn game_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_TIMER | WM_DESTROY => {
            unsafe { PostQuitMessage(0) };
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

pub fn find_game() -> Option<HWND> {
    unsafe { FindWindowW(GAME_CLASS, PCWSTR::null()).ok() }
}

pub fn foreground() -> HWND {
    unsafe { GetForegroundWindow() }
}

pub fn ex_style(hwnd: HWND) -> u32 {
    unsafe { GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32 }
}

/// Top-level window under a screen point (what a mouse click there would hit).
pub fn window_at(x: i32, y: i32) -> HWND {
    unsafe { GetAncestor(WindowFromPoint(POINT { x, y }), GA_ROOT) }
}

pub fn window_rect(hwnd: HWND) -> RECT {
    let mut r = RECT::default();
    unsafe {
        let _ = GetWindowRect(hwnd, &mut r);
    }
    r
}

/// The client rect of a window in screen pixels.
pub fn client_rect_screen(hwnd: HWND) -> RECT {
    let mut r = RECT::default();
    unsafe {
        let _ = GetClientRect(hwnd, &mut r);
        let mut p = POINT { x: 0, y: 0 };
        let _ = ClientToScreen(hwnd, &mut p);
        RECT { left: p.x, top: p.y, right: p.x + r.right, bottom: p.y + r.bottom }
    }
}

/// `GetLayeredWindowAttributes`: (ok, alpha, flags).
pub fn layered_attrs(hwnd: HWND) -> (bool, u8, u32) {
    let mut alpha = 0u8;
    let mut flags = LAYERED_WINDOW_ATTRIBUTES_FLAGS(0);
    let ok = unsafe {
        windows::Win32::UI::WindowsAndMessaging::GetLayeredWindowAttributes(
            hwnd,
            None,
            Some(&mut alpha),
            Some(&mut flags),
        )
        .is_ok()
    };
    (ok, alpha, flags.0)
}

/// Pixel format index of the window's DC. Not 0 means an OpenGL context was made on it (femtovg / Skia GL).
pub fn pixel_format(hwnd: HWND) -> i32 {
    unsafe {
        let dc = GetDC(Some(hwnd));
        let pf = GetPixelFormat(dc);
        ReleaseDC(Some(hwnd), dc);
        pf
    }
}

/// A BGRA capture. `pixels` is top-down, 4 bytes per pixel.
pub struct Capture {
    pub w: i32,
    pub h: i32,
    pub pixels: Vec<u8>,
}

pub const SENTINEL: [u8; 4] = [0x11, 0x22, 0x33, 0x44];

impl Capture {
    pub fn at(&self, x: i32, y: i32) -> [u8; 4] {
        let i = ((y * self.w + x) * 4) as usize;
        [self.pixels[i], self.pixels[i + 1], self.pixels[i + 2], self.pixels[i + 3]]
    }
    /// Share of pixels still equal to the sentinel (nothing was drawn there).
    pub fn untouched(&self) -> f64 {
        let n = self.pixels.as_chunks::<4>().0.iter().filter(|p| **p == SENTINEL).count();
        n as f64 / (self.w * self.h).max(1) as f64
    }
}

/// Captures a window with `PrintWindow(PW_RENDERFULLCONTENT)` into a 32 bpp DIB pre-filled with a sentinel,
/// or (`screen` = true) copies its rect from the desktop's screen DC.
pub fn capture(hwnd: HWND, screen: bool) -> Option<Capture> {
    let r = window_rect(hwnd);
    let (w, h) = (r.right - r.left, r.bottom - r.top);
    if w <= 0 || h <= 0 {
        return None;
    }
    unsafe {
        let screen_dc = GetDC(None);
        let mem = CreateCompatibleDC(Some(screen_dc));
        let bmi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w,
                biHeight: -h,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bits: *mut core::ffi::c_void = core::ptr::null_mut();
        let dib = CreateDIBSection(Some(mem), &bmi, DIB_RGB_COLORS, &mut bits, None, 0).ok()?;
        let old = SelectObject(mem, dib.into());
        let len = (w * h * 4) as usize;
        let buf = std::slice::from_raw_parts_mut(bits as *mut u8, len);
        for px in buf.as_chunks_mut::<4>().0 {
            *px = SENTINEL;
        }
        let ok = if screen {
            BitBlt(mem, 0, 0, w, h, Some(screen_dc), r.left, r.top, SRCCOPY).is_ok()
        } else {
            PrintWindow(hwnd, mem, PRINT_WINDOW_FLAGS(2)).as_bool() // PW_RENDERFULLCONTENT
        };
        let pixels = buf.to_vec();
        SelectObject(mem, old);
        let _ = DeleteObject(dib.into());
        let _ = DeleteDC(mem);
        ReleaseDC(None, screen_dc);
        ok.then_some(Capture { w, h, pixels })
    }
}

/// Result of one run on a hidden desktop.
pub struct HiddenRun {
    pub probe_exit: Option<u32>,
    pub json: Option<String>,
}

/// Makes a new desktop (never switched to, so nothing shows on the user's screen), starts the dummy game
/// and then the probe on it, waits for the probe, ends the game and closes the desktop.
/// Runs are serialized: one hidden desktop at a time per process.
pub fn run_hidden(exe: &Path, probe_args: &[&str], out: &Path) -> windows::core::Result<HiddenRun> {
    static ONE_AT_A_TIME: std::sync::Mutex<()> = std::sync::Mutex::new(());
    static COUNT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let _guard = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let name = format!(
        "dr-overlay-spike-{}-{}-{}",
        std::process::id(),
        COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0)
    );
    let wname: Vec<u16> = name.encode_utf16().chain([0]).collect();
    let _ = std::fs::remove_file(out);
    unsafe {
        let desk = CreateDesktopW(PCWSTR(wname.as_ptr()), PCWSTR::null(), None, DESKTOP_CONTROL_FLAGS(0), 0x1000_0000, None)?;
        let result = (|| {
            let game = spawn(exe, &["game", "--secs", "90"], &wname)?;
            // Wait until the game window exists on that desktop (up to 10 s), not a fixed time. The probe
            // looks for it again itself.
            let until = Instant::now() + Duration::from_secs(10);
            while !desktop_has_game(desk) && Instant::now() < until {
                std::thread::sleep(Duration::from_millis(50));
            }
            let mut args: Vec<&str> = vec!["probe", "--out"];
            let out_s = out.to_string_lossy().to_string();
            args.push(&out_s);
            args.extend_from_slice(probe_args);
            let probe = spawn(exe, &args, &wname)?;
            let deadline = Instant::now() + Duration::from_secs(90);
            let mut exit = None;
            while Instant::now() < deadline {
                if WaitForSingleObject(probe.hProcess, 200) == WAIT_OBJECT_0 {
                    let mut code = 0u32;
                    let _ = windows::Win32::System::Threading::GetExitCodeProcess(probe.hProcess, &mut code);
                    exit = Some(code);
                    break;
                }
            }
            if exit.is_none() {
                let _ = TerminateProcess(probe.hProcess, 9);
            }
            let _ = TerminateProcess(game.hProcess, 0);
            let _ = WaitForSingleObject(game.hProcess, 5000);
            for pi in [probe, game] {
                let _ = CloseHandle(pi.hProcess);
                let _ = CloseHandle(pi.hThread);
            }
            Ok(HiddenRun { probe_exit: exit, json: std::fs::read_to_string(out).ok() })
        })();
        let _ = CloseDesktop(desk);
        result
    }
}

/// True when the dummy game's window exists on `desk` (`FindWindowW` here only sees this process's desktop).
fn desktop_has_game(desk: HDESK) -> bool {
    unsafe extern "system" fn each(hwnd: HWND, found: LPARAM) -> windows::core::BOOL {
        let mut buf = [0u16; 64];
        let n = unsafe { GetClassNameW(hwnd, &mut buf) } as usize;
        if String::from_utf16_lossy(&buf[..n]) == "DrSpikeGame" {
            unsafe { *(found.0 as *mut bool) = true };
            return false.into();
        }
        true.into()
    }
    let mut found = false;
    unsafe {
        let _ = EnumDesktopWindows(Some(desk), Some(each), LPARAM(&mut found as *mut bool as isize));
    }
    found
}

unsafe fn spawn(exe: &Path, args: &[&str], desktop: &[u16]) -> windows::core::Result<PROCESS_INFORMATION> {
    let mut cmd = format!("\"{}\"", exe.display());
    for a in args {
        cmd.push_str(&format!(" \"{a}\""));
    }
    let mut cmd: Vec<u16> = cmd.encode_utf16().chain([0]).collect();
    let mut desk: Vec<u16> = desktop.to_vec();
    let si = STARTUPINFOW {
        cb: size_of::<STARTUPINFOW>() as u32,
        lpDesktop: PWSTR(desk.as_mut_ptr()),
        ..Default::default()
    };
    let mut pi = PROCESS_INFORMATION::default();
    unsafe {
        CreateProcessW(
            PCWSTR::null(),
            Some(PWSTR(cmd.as_mut_ptr())),
            None,
            None,
            false,
            CREATE_NO_WINDOW,
            None,
            PCWSTR::null(),
            &si,
            &mut pi,
        )?;
    }
    Ok(pi)
}

// ---- Activation spy -------------------------------------------------------------------------------------
//
// On a desktop that is not the input desktop `GetForegroundWindow` is always NULL, so it cannot show a stolen
// focus there. Activation still happens per thread: a window that activates gets `WM_ACTIVATE` and becomes
// its thread's active window (`GetGUIThreadInfo`). A thread-wide `WH_CALLWNDPROC` hook counts those messages
// for every window of the UI thread, from before the first show.

use std::cell::RefCell;
use std::collections::HashMap;

use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::WindowsAndMessaging::{
    CWPSTRUCT, CallNextHookEx, GUITHREADINFO, GetGUIThreadInfo, GetWindowThreadProcessId, HHOOK,
    SetWindowsHookExW, WH_CALLWNDPROC, WM_ACTIVATE, WM_SETFOCUS,
};

#[derive(Clone, Copy, Default, Debug)]
pub struct Activity {
    pub activate: u32,
    pub setfocus: u32,
}

thread_local! {
    static ACTIVITY: RefCell<HashMap<usize, Activity>> = RefCell::new(HashMap::new());
    static SPY: RefCell<Option<HHOOK>> = const { RefCell::new(None) };
}

/// Starts counting `WM_ACTIVATE` (active) and `WM_SETFOCUS` for every window of this thread.
pub fn spy_start() -> bool {
    let hook = unsafe { SetWindowsHookExW(WH_CALLWNDPROC, Some(spy_proc), None, GetCurrentThreadId()) };
    let ok = hook.is_ok();
    SPY.with(|s| *s.borrow_mut() = hook.ok());
    ok
}

pub fn activity(hwnd: HWND) -> Activity {
    ACTIVITY.with(|a| a.borrow().get(&(hwnd.0 as usize)).copied().unwrap_or_default())
}

unsafe extern "system" fn spy_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 {
        let cwp = unsafe { &*(lparam.0 as *const CWPSTRUCT) };
        let key = cwp.hwnd.0 as usize;
        if cwp.message == WM_ACTIVATE && (cwp.wParam.0 & 0xFFFF) != 0 {
            ACTIVITY.with(|a| a.borrow_mut().entry(key).or_default().activate += 1);
        } else if cwp.message == WM_SETFOCUS {
            ACTIVITY.with(|a| a.borrow_mut().entry(key).or_default().setfocus += 1);
        }
    }
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

/// (active window, focus window) of a thread's input queue.
pub fn thread_active(tid: u32) -> (HWND, HWND) {
    let mut gi = GUITHREADINFO { cbSize: size_of::<GUITHREADINFO>() as u32, ..Default::default() };
    unsafe {
        let _ = GetGUIThreadInfo(tid, &mut gi);
    }
    (gi.hwndActive, gi.hwndFocus)
}

pub fn current_thread() -> u32 {
    unsafe { GetCurrentThreadId() }
}

pub fn thread_of(hwnd: HWND) -> u32 {
    unsafe { GetWindowThreadProcessId(hwnd, None) }
}

/// Process id of a window's owner.
pub fn pid_of(hwnd: HWND) -> u32 {
    let mut pid = 0u32;
    unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
    pid
}
