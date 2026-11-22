//! The windows DialogReader can read (page 02, spec 2.3): title, exe, client size, mode, icon, and whether it
//! looks like a game.
//!
//! A window is in the game group when its exe has a profile, or it covers a whole monitor, or its process has
//! a 3D graphics library loaded (`d3d11.dll`, `d3d12.dll`, `dxgi.dll`, `vulkan-1.dll`). When the process cannot
//! be read (it runs as administrator), only the first two checks apply. Browser and Electron windows
//! (window classes `Chrome_WidgetWin_1` and `MozillaWindowClass`) always load those libraries to draw, so for
//! them the library check is skipped (measured 2026-11-22: Zen and an Electron app were listed as games).
//!
//! `EnumWindows` sees the windows of the calling thread's desktop, so a test can run the list on a hidden
//! desktop with its own window there.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use windows::Win32::Foundation::{CloseHandle, HANDLE, HMODULE, HWND, LPARAM, POINT, RECT};
use windows::Win32::Graphics::Dwm::{DWMWA_CLOAKED, DwmGetWindowAttribute};
use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, ClientToScreen, CreateCompatibleDC, DIB_RGB_COLORS,
    DeleteDC, DeleteObject, GetDIBits, GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO,
    MonitorFromWindow,
};
use windows::Win32::Storage::FileSystem::FILE_FLAGS_AND_ATTRIBUTES;
use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx, CoUninitialize};
use windows::Win32::System::ProcessStatus::{
    EnumProcessModulesEx, GetModuleBaseNameW, LIST_MODULES_ALL,
};
use windows::Win32::System::Threading::{
    GetCurrentProcessId, OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_INFORMATION,
    PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_VM_READ, QueryFullProcessImageNameW,
};
use windows::Win32::UI::HiDpi::{
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetThreadDpiAwarenessContext,
};
use windows::Win32::UI::Shell::{SHFILEINFOW, SHGFI_ICON, SHGFI_LARGEICON, SHGetFileInfoW};
use windows::Win32::UI::WindowsAndMessaging::{
    DestroyIcon, EnumWindows, GW_OWNER, GWL_EXSTYLE, GWL_STYLE, GetClassNameW, GetClientRect,
    GetIconInfo, GetWindow, GetWindowLongPtrW, GetWindowTextLengthW, GetWindowTextW,
    GetWindowThreadProcessId, HICON, ICONINFO, IsIconic, IsWindowVisible, WS_CAPTION, WS_CHILD,
    WS_EX_TOOLWINDOW,
};
use windows::core::{BOOL, PCWSTR};

/// Borderless covers its monitor with no title bar; everything else is windowed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WinMode {
    Borderless,
    Windowed,
}

impl WinMode {
    pub fn label(self) -> &'static str {
        match self {
            WinMode::Borderless => "borderless",
            WinMode::Windowed => "windowed",
        }
    }
}

/// An icon as RGBA (straight alpha).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Icon {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct WinInfo {
    /// The window handle as a number (`HWND.0 as isize`).
    pub hwnd: isize,
    pub pid: u32,
    pub title: String,
    /// Process file name, for example `RDR2.exe`. Empty when the process cannot be read.
    pub exe: String,
    pub exe_path: Option<PathBuf>,
    /// Client area in physical pixels.
    pub width: u32,
    pub height: u32,
    pub mode: WinMode,
    pub minimized: bool,
    pub game: bool,
    pub icon: Option<Icon>,
}

impl WinInfo {
    /// "3840 × 2160 · borderless", as page 02 shows it.
    pub fn meta(&self) -> String {
        if self.minimized {
            return "minimized".into();
        }
        format!("{} × {} · {}", self.width, self.height, self.mode.label())
    }
}

/// What [`list`] looks at.
#[derive(Clone, Debug, Default)]
pub struct ListOpts {
    /// Exe names that have a profile (case does not matter). Their windows count as games.
    pub profile_exes: Vec<String>,
    /// Leave out the windows of this process (the app's own window).
    pub skip_pid: Option<u32>,
    /// Load each exe's icon.
    pub icons: bool,
}

impl ListOpts {
    /// The app's settings: icons on, its own windows left out.
    pub fn for_app(profile_exes: Vec<String>) -> ListOpts {
        ListOpts {
            profile_exes,
            skip_pid: Some(unsafe { GetCurrentProcessId() }),
            icons: true,
        }
    }
}

/// Facts of one window for the game check.
#[derive(Clone, Copy, Debug, Default)]
pub struct GameFacts {
    pub has_profile: bool,
    pub covers_monitor: bool,
    /// `None`: the process could not be read.
    pub gpu_library: Option<bool>,
    /// A browser or Electron window: its 3D libraries say nothing.
    pub browser: bool,
}

/// The game check of spec 2.3.
pub fn is_game(f: GameFacts) -> bool {
    f.has_profile || f.covers_monitor || (!f.browser && f.gpu_library == Some(true))
}

/// The window is minimized now (it may have been minimized or restored since the list was made). A minimized
/// window gives no picture, so the caller says so at once instead of waiting for the capture timeout.
pub fn is_minimized(hwnd: isize) -> bool {
    unsafe { IsIconic(HWND(hwnd as *mut std::ffi::c_void)).as_bool() }
}

/// Borderless when the window covers its monitor and has no title bar.
pub fn mode_of(covers_monitor: bool, has_caption: bool) -> WinMode {
    if covers_monitor && !has_caption {
        WinMode::Borderless
    } else {
        WinMode::Windowed
    }
}

/// Games first, then the rest; each group keeps the window order (front to back).
pub fn games_first(mut v: Vec<WinInfo>) -> Vec<WinInfo> {
    v.sort_by_key(|w| !w.game);
    v
}

/// Window classes of browser engines (Chromium and Electron, Firefox and its forks).
const BROWSER_CLASSES: [&str; 2] = ["Chrome_WidgetWin_1", "MozillaWindowClass"];

/// The libraries that mark a 3D program.
const GPU_LIBS: [&str; 4] = ["d3d11.dll", "d3d12.dll", "dxgi.dll", "vulkan-1.dll"];

/// Windows of the calling thread's desktop that can be read, games first.
pub fn list(opts: &ListOpts) -> Vec<WinInfo> {
    let mut hwnds: Vec<HWND> = Vec::new();
    unsafe extern "system" fn collect(h: HWND, lp: LPARAM) -> BOOL {
        let v = unsafe { &mut *(lp.0 as *mut Vec<HWND>) };
        v.push(h);
        BOOL(1)
    }
    // Sizes in physical pixels, whatever the caller's DPI awareness.
    let old = unsafe { SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };
    unsafe {
        let _ = EnumWindows(Some(collect), LPARAM(&mut hwnds as *mut Vec<HWND> as isize));
    }
    let com = opts.icons && unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }.is_ok();
    let exes: Vec<String> = opts
        .profile_exes
        .iter()
        .map(|e| e.to_ascii_lowercase())
        .collect();
    let mut procs: HashMap<u32, ProcInfo> = HashMap::new();
    let mut icons: HashMap<PathBuf, Option<Icon>> = HashMap::new();
    let mut out = Vec::new();
    for h in hwnds {
        let Some(base) = readable(h) else { continue };
        let mut pid = 0u32;
        unsafe { GetWindowThreadProcessId(h, Some(&mut pid)) };
        if Some(pid) == opts.skip_pid {
            continue;
        }
        let p = procs
            .entry(pid)
            .or_insert_with(|| ProcInfo::read(pid))
            .clone();
        let exe = p
            .path
            .as_deref()
            .and_then(Path::file_name)
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let facts = GameFacts {
            has_profile: !exe.is_empty() && exes.contains(&exe.to_ascii_lowercase()),
            covers_monitor: base.covers_monitor,
            gpu_library: p.gpu,
            browser: base.browser,
        };
        let icon = match (&p.path, opts.icons) {
            (Some(path), true) => icons
                .entry(path.clone())
                .or_insert_with(|| exe_icon(path))
                .clone(),
            _ => None,
        };
        out.push(WinInfo {
            hwnd: h.0 as isize,
            pid,
            title: base.title,
            exe,
            exe_path: p.path,
            width: base.width,
            height: base.height,
            mode: mode_of(base.covers_monitor, base.has_caption),
            minimized: base.minimized,
            game: is_game(facts),
            icon,
        });
    }
    if com {
        unsafe { CoUninitialize() };
    }
    unsafe { SetThreadDpiAwarenessContext(old) };
    games_first(out)
}

struct Base {
    title: String,
    width: u32,
    height: u32,
    covers_monitor: bool,
    has_caption: bool,
    minimized: bool,
    browser: bool,
}

/// The window facts, or `None` when it is not a window a user would pick: hidden, cloaked (other virtual
/// desktop or a suspended store app), a tool window, owned by another window, untitled, or the desktop shell.
fn readable(h: HWND) -> Option<Base> {
    unsafe {
        if !IsWindowVisible(h).as_bool() {
            return None;
        }
        let style = GetWindowLongPtrW(h, GWL_STYLE) as u32;
        let ex = GetWindowLongPtrW(h, GWL_EXSTYLE) as u32;
        if style & WS_CHILD.0 != 0 || ex & WS_EX_TOOLWINDOW.0 != 0 {
            return None;
        }
        if GetWindow(h, GW_OWNER).is_ok_and(|o| !o.is_invalid()) {
            return None;
        }
        let mut cloaked = 0u32;
        if DwmGetWindowAttribute(h, DWMWA_CLOAKED, &mut cloaked as *mut u32 as *mut _, 4).is_ok()
            && cloaked != 0
        {
            return None;
        }
        let mut class = [0u16; 64];
        let n = GetClassNameW(h, &mut class) as usize;
        let class = String::from_utf16_lossy(&class[..n]);
        if matches!(class.as_str(), "Progman" | "WorkerW" | "Shell_TrayWnd") {
            return None;
        }
        let len = GetWindowTextLengthW(h);
        if len <= 0 {
            return None;
        }
        let mut buf = vec![0u16; len as usize + 1];
        let n = GetWindowTextW(h, &mut buf) as usize;
        let title = String::from_utf16_lossy(&buf[..n]).trim().to_string();
        if title.is_empty() {
            return None;
        }
        let minimized = IsIconic(h).as_bool();
        let mut cr = RECT::default();
        GetClientRect(h, &mut cr).ok()?;
        let (width, height) = (
            (cr.right - cr.left).max(0) as u32,
            (cr.bottom - cr.top).max(0) as u32,
        );
        if !minimized && (width == 0 || height == 0) {
            return None;
        }
        // The client area on screen, so a maximized window with a title bar (its frame hangs over the edges)
        // does not count, and a borderless game does.
        let mut origin = POINT::default();
        let covers_monitor = !minimized && ClientToScreen(h, &mut origin).as_bool() && {
            let client = RECT {
                left: origin.x,
                top: origin.y,
                right: origin.x + width as i32,
                bottom: origin.y + height as i32,
            };
            let mon = MonitorFromWindow(h, MONITOR_DEFAULTTONEAREST);
            let mut mi = MONITORINFO {
                cbSize: size_of::<MONITORINFO>() as u32,
                ..Default::default()
            };
            GetMonitorInfoW(mon, &mut mi).as_bool() && rect_covers(client, mi.rcMonitor)
        };
        Some(Base {
            title,
            width,
            height,
            covers_monitor,
            has_caption: style & WS_CAPTION.0 == WS_CAPTION.0,
            minimized,
            browser: BROWSER_CLASSES.contains(&class.as_str()),
        })
    }
}

/// The window rect holds the whole monitor rect.
fn rect_covers(w: RECT, m: RECT) -> bool {
    w.left <= m.left && w.top <= m.top && w.right >= m.right && w.bottom >= m.bottom
}

#[derive(Clone)]
struct ProcInfo {
    path: Option<PathBuf>,
    gpu: Option<bool>,
}

impl ProcInfo {
    fn read(pid: u32) -> ProcInfo {
        ProcInfo {
            path: exe_path(pid),
            gpu: gpu_library(pid),
        }
    }
}

struct Handle(HANDLE);
impl Drop for Handle {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

fn exe_path(pid: u32) -> Option<PathBuf> {
    unsafe {
        let h = Handle(OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?);
        let mut buf = vec![0u16; 1024];
        let mut len = buf.len() as u32;
        QueryFullProcessImageNameW(
            h.0,
            PROCESS_NAME_WIN32,
            windows::core::PWSTR(buf.as_mut_ptr()),
            &mut len,
        )
        .ok()?;
        Some(PathBuf::from(String::from_utf16_lossy(
            &buf[..len as usize],
        )))
    }
}

/// `Some(true)` when the process has a 3D library loaded, `None` when it cannot be read.
fn gpu_library(pid: u32) -> Option<bool> {
    unsafe {
        let h = Handle(OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, false, pid).ok()?);
        let mut mods = vec![HMODULE::default(); 1024];
        let mut need = 0u32;
        let bytes = (mods.len() * size_of::<HMODULE>()) as u32;
        EnumProcessModulesEx(h.0, mods.as_mut_ptr(), bytes, &mut need, LIST_MODULES_ALL).ok()?;
        let n = (need as usize / size_of::<HMODULE>()).min(mods.len());
        let mut name = [0u16; 260];
        for m in &mods[..n] {
            let len = GetModuleBaseNameW(h.0, Some(*m), &mut name) as usize;
            let s = String::from_utf16_lossy(&name[..len]).to_ascii_lowercase();
            if GPU_LIBS.contains(&s.as_str()) {
                return Some(true);
            }
        }
        Some(false)
    }
}

/// The large (32 px) icon of an exe as RGBA.
pub fn exe_icon(path: &Path) -> Option<Icon> {
    let wide: Vec<u16> = path.as_os_str().encode_wide_z();
    let mut info = SHFILEINFOW::default();
    let ok = unsafe {
        SHGetFileInfoW(
            PCWSTR(wide.as_ptr()),
            FILE_FLAGS_AND_ATTRIBUTES(0),
            Some(&mut info),
            size_of::<SHFILEINFOW>() as u32,
            SHGFI_ICON | SHGFI_LARGEICON,
        )
    };
    if ok == 0 || info.hIcon.is_invalid() {
        return None;
    }
    let icon = icon_rgba(info.hIcon);
    unsafe {
        let _ = DestroyIcon(info.hIcon);
    }
    icon
}

fn icon_rgba(hicon: HICON) -> Option<Icon> {
    unsafe {
        let mut ii = ICONINFO::default();
        GetIconInfo(hicon, &mut ii).ok()?;
        let dc = CreateCompatibleDC(None);
        let read = |bmp: windows::Win32::Graphics::Gdi::HBITMAP| -> Option<(u32, u32, Vec<u8>)> {
            if bmp.is_invalid() {
                return None;
            }
            let mut bmi = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: size_of::<BITMAPINFOHEADER>() as u32,
                    ..Default::default()
                },
                ..Default::default()
            };
            if GetDIBits(dc, bmp, 0, 0, None, &mut bmi, DIB_RGB_COLORS) == 0 {
                return None;
            }
            let (w, h) = (
                bmi.bmiHeader.biWidth.unsigned_abs(),
                bmi.bmiHeader.biHeight.unsigned_abs(),
            );
            bmi.bmiHeader.biBitCount = 32;
            bmi.bmiHeader.biCompression = BI_RGB.0;
            bmi.bmiHeader.biHeight = -(h as i32);
            let mut px = vec![0u8; (w * h * 4) as usize];
            if GetDIBits(
                dc,
                bmp,
                0,
                h,
                Some(px.as_mut_ptr() as *mut _),
                &mut bmi,
                DIB_RGB_COLORS,
            ) == 0
            {
                return None;
            }
            Some((w, h, px))
        };
        let color = read(ii.hbmColor);
        let mask = read(ii.hbmMask);
        let _ = DeleteDC(dc);
        if !ii.hbmColor.is_invalid() {
            let _ = DeleteObject(ii.hbmColor.into());
        }
        if !ii.hbmMask.is_invalid() {
            let _ = DeleteObject(ii.hbmMask.into());
        }
        let (w, h, mut px) = color?;
        let has_alpha = px.as_chunks::<4>().0.iter().any(|p| p[3] != 0);
        for (i, p) in px.as_chunks_mut::<4>().0.iter_mut().enumerate() {
            p.swap(0, 2); // BGRA -> RGBA
            if !has_alpha {
                // old icons: the AND mask says which pixels are see-through (white = transparent)
                let see_through = mask
                    .as_ref()
                    .is_some_and(|(_, _, m)| m.get(i * 4).is_some_and(|&v| v != 0));
                p[3] = if see_through { 0 } else { 255 };
            }
        }
        Some(Icon {
            width: w,
            height: h,
            rgba: px,
        })
    }
}

trait WideZ {
    fn encode_wide_z(&self) -> Vec<u16>;
}
impl WideZ for std::ffi::OsStr {
    fn encode_wide_z(&self) -> Vec<u16> {
        use std::os::windows::ffi::OsStrExt;
        self.encode_wide().chain(std::iter::once(0)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn game_check() {
        let f = GameFacts::default();
        assert!(!is_game(f));
        assert!(is_game(GameFacts {
            has_profile: true,
            ..f
        }));
        assert!(is_game(GameFacts {
            covers_monitor: true,
            ..f
        }));
        assert!(is_game(GameFacts {
            gpu_library: Some(true),
            ..f
        }));
        // an elevated process cannot be read: only the first two checks count
        assert!(!is_game(GameFacts {
            gpu_library: None,
            ..f
        }));
        // a browser always has them loaded
        assert!(!is_game(GameFacts {
            gpu_library: Some(true),
            browser: true,
            ..f
        }));
        assert!(is_game(GameFacts {
            covers_monitor: true,
            browser: true,
            ..f
        }));
    }

    #[test]
    fn mode_needs_a_full_monitor_and_no_title_bar() {
        assert_eq!(mode_of(true, false), WinMode::Borderless);
        assert_eq!(mode_of(true, true), WinMode::Windowed);
        assert_eq!(mode_of(false, false), WinMode::Windowed);
    }

    #[test]
    fn covers_is_inclusive() {
        let m = RECT {
            left: 0,
            top: 0,
            right: 1920,
            bottom: 1080,
        };
        assert!(rect_covers(m, m));
        assert!(rect_covers(
            RECT {
                left: -8,
                top: -8,
                right: 1928,
                bottom: 1088
            },
            m
        ));
        assert!(!rect_covers(
            RECT {
                left: 0,
                top: 0,
                right: 1920,
                bottom: 1040
            },
            m
        ));
    }
}
