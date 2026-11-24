//! Checks the real overlay windows on a hidden desktop (`tests/overlay_win.rs` runs it; nothing shows on the
//! user's screen and nothing takes the foreground). Three roles:
//!
//! - `run --out <file> [--renderer <name>]`: makes a new desktop (never switched to), starts `game` and then
//!   `probe` on it, waits for the probe, ends the game, closes the desktop.
//! - `game`: a plain window with a title bar (so the client area and what the capture covers differ), which
//!   activates itself.
//! - `probe --out <file> [--renderer <name>]`: selects the winit backend with the overlay hook as `main.rs` does,
//!   makes `dr_app::overlay::Overlays` and shows the pill, the quick menu and zone edit over the game through the
//!   app's own code, then writes what it measured as `key=value` lines. These first checks use instant motion,
//!   so each window is drawn in full at once. A second set of windows with the real motion then fades each one
//!   out: while it fades it must stay visible with its styles, take no focus, and the pill must stay
//!   click-through; after the fade it must be hidden.
//!
//! How focus is measured: on a desktop that is not the input desktop `GetForegroundWindow` is always NULL, so a
//! thread-wide `WH_CALLWNDPROC` hook counts `WM_ACTIVATE` and `WM_SETFOCUS` for every window of the UI thread. A
//! plain window shown last is the positive control: it must activate, so the check can fail
//! (docs/overlay-spike.md).

#![cfg_attr(not(windows), allow(unused))]

#[cfg(not(windows))]
fn main() {}

#[cfg(windows)]
fn main() {
    std::process::exit(imp::main());
}

#[cfg(windows)]
mod imp {
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::fmt::Write as _;
    use std::path::{Path, PathBuf};
    use std::rc::Rc;
    use std::time::{Duration, Instant};

    use dr_app::overlay::place::{GameWin, PillAt};
    use dr_app::overlay::{Overlays, win};
    use slint::ComponentHandle;
    use windows::Win32::Foundation::{
        CloseHandle, HWND, LPARAM, LRESULT, POINT, RECT, WAIT_OBJECT_0, WPARAM,
    };
    use windows::Win32::Graphics::Gdi::{GetDC, ReleaseDC};
    use windows::Win32::Graphics::OpenGL::GetPixelFormat;
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::System::StationsAndDesktops::{
        CloseDesktop, CreateDesktopW, DESKTOP_CONTROL_FLAGS, EnumDesktopWindows, HDESK,
    };
    use windows::Win32::System::Threading::{
        CREATE_NO_WINDOW, CreateProcessW, GetCurrentThreadId, GetExitCodeProcess,
        PROCESS_INFORMATION, STARTUPINFOW, TerminateProcess, WaitForSingleObject,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        ASFW_ANY, AllowSetForegroundWindow, CS_HREDRAW, CS_VREDRAW, CWPSTRUCT, CallNextHookEx,
        CreateWindowExW, DefWindowProcW, DispatchMessageW, FindWindowW, GA_ROOT, GUITHREADINFO,
        GetAncestor, GetClassNameW, GetGUIThreadInfo, GetLayeredWindowAttributes, GetMessageW,
        GetWindowRect, IDC_ARROW, IsWindowVisible, KillTimer, LAYERED_WINDOW_ATTRIBUTES_FLAGS,
        LoadCursorW, MSG, PostQuitMessage, RegisterClassW, SW_SHOW, SendMessageW,
        SetForegroundWindow, SetTimer, SetWindowsHookExW, ShowWindow, TranslateMessage,
        WH_CALLWNDPROC, WINDOW_EX_STYLE, WM_ACTIVATE, WM_DESTROY, WM_MOUSEACTIVATE, WM_SETFOCUS,
        WM_TIMER, WNDCLASSW, WS_EX_APPWINDOW, WS_OVERLAPPEDWINDOW, WS_VISIBLE, WindowFromPoint,
    };
    use windows::core::{PCWSTR, PWSTR, w};

    const GAME_CLASS: PCWSTR = w!("DrM4ProbeGame");
    const GAME_CLASS_NAME: &str = "DrM4ProbeGame";

    pub fn main() -> i32 {
        let args: Vec<String> = std::env::args().skip(1).collect();
        let opt = |name: &str| {
            args.iter()
                .position(|a| a == name)
                .and_then(|i| args.get(i + 1))
                .cloned()
        };
        let renderer = opt("--renderer").unwrap_or_else(|| "skia-opengl".into());
        match args.first().map(String::as_str) {
            Some("game") => {
                run_game(60);
                0
            }
            Some("probe") => match opt("--out") {
                Some(out) => probe(Path::new(&out), &renderer),
                None => 2,
            },
            Some("run") => match opt("--out") {
                Some(out) => run_hidden(Path::new(&out), &renderer),
                None => 2,
            },
            _ => {
                eprintln!("usage: overlay-probe run --out <file> [--renderer <name>]");
                2
            }
        }
    }

    // ---- the dummy game ---------------------------------------------------------------------------------------

    fn run_game(secs: u32) {
        unsafe {
            let Ok(inst) = GetModuleHandleW(None) else {
                return;
            };
            let wc = WNDCLASSW {
                style: CS_HREDRAW | CS_VREDRAW,
                lpfnWndProc: Some(game_proc),
                hInstance: inst.into(),
                hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
                lpszClassName: GAME_CLASS,
                ..Default::default()
            };
            RegisterClassW(&wc);
            let Ok(hwnd) = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                GAME_CLASS,
                w!("DR M4 probe game"),
                WS_OVERLAPPEDWINDOW | WS_VISIBLE,
                0,
                0,
                1280,
                720,
                None,
                None,
                Some(inst.into()),
                None,
            ) else {
                return;
            };
            let _ = ShowWindow(hwnd, SW_SHOW);
            let _ = SetForegroundWindow(hwnd);
            // any process may take the foreground: if an overlay could activate, nothing stops it
            let _ = AllowSetForegroundWindow(ASFW_ANY);
            SetTimer(Some(hwnd), 1, secs * 1000, None);
            let mut msg = MSG::default();
            while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
            let _ = KillTimer(Some(hwnd), 1);
        }
    }

    unsafe extern "system" fn game_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
        match msg {
            WM_TIMER | WM_DESTROY => {
                unsafe { PostQuitMessage(0) };
                LRESULT(0)
            }
            _ => unsafe { DefWindowProcW(hwnd, msg, wp, lp) },
        }
    }

    // ---- the hidden desktop runner ----------------------------------------------------------------------------

    fn run_hidden(out: &Path, renderer: &str) -> i32 {
        let exe = match std::env::current_exe() {
            Ok(e) => e,
            Err(_) => return 2,
        };
        let name = format!(
            "dr-m4-overlay-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis())
                .unwrap_or(0)
        );
        let wname: Vec<u16> = name.encode_utf16().chain([0]).collect();
        let _ = std::fs::remove_file(out);
        unsafe {
            let Ok(desk) = CreateDesktopW(
                PCWSTR(wname.as_ptr()),
                PCWSTR::null(),
                None,
                DESKTOP_CONTROL_FLAGS(0),
                0x1000_0000,
                None,
            ) else {
                eprintln!("CreateDesktop failed");
                return 3;
            };
            let code = (|| {
                let game = spawn(&exe, &["game"], &wname).ok()?;
                let until = Instant::now() + Duration::from_secs(10);
                while !desktop_has_game(desk) && Instant::now() < until {
                    std::thread::sleep(Duration::from_millis(50));
                }
                let out_s = out.to_string_lossy().to_string();
                let probe = spawn(
                    &exe,
                    &["probe", "--out", &out_s, "--renderer", renderer],
                    &wname,
                )
                .ok()?;
                let deadline = Instant::now() + Duration::from_secs(90);
                let mut exit = None;
                while Instant::now() < deadline {
                    if WaitForSingleObject(probe.hProcess, 200) == WAIT_OBJECT_0 {
                        let mut c = 0u32;
                        let _ = GetExitCodeProcess(probe.hProcess, &mut c);
                        exit = Some(c);
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
                exit
            })();
            let _ = CloseDesktop(desk);
            match code {
                Some(c) => c as i32,
                None => 4,
            }
        }
    }

    fn desktop_has_game(desk: HDESK) -> bool {
        unsafe extern "system" fn each(hwnd: HWND, found: LPARAM) -> windows::core::BOOL {
            let mut buf = [0u16; 64];
            let n = unsafe { GetClassNameW(hwnd, &mut buf) } as usize;
            if String::from_utf16_lossy(&buf[..n]) == GAME_CLASS_NAME {
                unsafe { *(found.0 as *mut bool) = true };
                return false.into();
            }
            true.into()
        }
        let mut found = false;
        unsafe {
            let _ = EnumDesktopWindows(
                Some(desk),
                Some(each),
                LPARAM(&mut found as *mut bool as isize),
            );
        }
        found
    }

    unsafe fn spawn(
        exe: &Path,
        args: &[&str],
        desktop: &[u16],
    ) -> windows::core::Result<PROCESS_INFORMATION> {
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

    // ---- activation spy ---------------------------------------------------------------------------------------

    #[derive(Clone, Copy, Default)]
    struct Activity {
        activate: u32,
        setfocus: u32,
    }

    thread_local! {
        static ACTIVITY: RefCell<HashMap<usize, Activity>> = RefCell::new(HashMap::new());
    }

    unsafe extern "system" fn spy_proc(code: i32, wp: WPARAM, lp: LPARAM) -> LRESULT {
        if code >= 0 {
            let c = unsafe { &*(lp.0 as *const CWPSTRUCT) };
            let key = c.hwnd.0 as usize;
            if c.message == WM_ACTIVATE && (c.wParam.0 & 0xFFFF) != 0 {
                ACTIVITY.with(|a| a.borrow_mut().entry(key).or_default().activate += 1);
            } else if c.message == WM_SETFOCUS {
                ACTIVITY.with(|a| a.borrow_mut().entry(key).or_default().setfocus += 1);
            }
        }
        unsafe { CallNextHookEx(None, code, wp, lp) }
    }

    fn activity(h: HWND) -> Activity {
        ACTIVITY.with(|a| a.borrow().get(&(h.0 as usize)).copied().unwrap_or_default())
    }

    fn thread_active() -> HWND {
        let mut gi = GUITHREADINFO {
            cbSize: size_of::<GUITHREADINFO>() as u32,
            ..Default::default()
        };
        unsafe {
            let _ = GetGUIThreadInfo(GetCurrentThreadId(), &mut gi);
        }
        gi.hwndActive
    }

    // ---- the probe --------------------------------------------------------------------------------------------

    struct Report(String);

    impl Report {
        fn put(&mut self, k: &str, v: impl std::fmt::Display) {
            let _ = writeln!(self.0, "{k}={v}");
        }
    }

    fn rect(h: HWND) -> RECT {
        let mut r = RECT::default();
        unsafe {
            let _ = GetWindowRect(h, &mut r);
        }
        r
    }

    fn hits(x: i32, y: i32, me: HWND, game: HWND) -> &'static str {
        let h = unsafe { GetAncestor(WindowFromPoint(POINT { x, y }), GA_ROOT) };
        if h == me {
            "self"
        } else if h == game {
            "game"
        } else {
            "other"
        }
    }

    /// Styles, activation and what a click at its center hits, for one overlay window.
    fn check(r: &mut Report, key: &str, w: &slint::Window, kind: win::Kind, game: HWND) {
        let Some(h) = win::hwnd_of(w) else {
            r.put(&format!("{key}.hwnd"), "none");
            return;
        };
        let ex = win::ex_style(h);
        let want = kind.ex_style();
        let a = activity(h);
        let rc = rect(h);
        r.put(&format!("{key}.hwnd"), h.0 as usize);
        r.put(&format!("{key}.has_all"), ex & want == want);
        r.put(&format!("{key}.appwindow"), ex & WS_EX_APPWINDOW.0 != 0);
        r.put(&format!("{key}.hardened"), win::is_hardened(h));
        r.put(&format!("{key}.visible"), unsafe {
            IsWindowVisible(h).as_bool()
        });
        r.put(&format!("{key}.activations"), a.activate);
        r.put(&format!("{key}.setfocus"), a.setfocus);
        r.put(&format!("{key}.active_is_self"), thread_active() == h);
        r.put(
            &format!("{key}.rect"),
            format!(
                "{},{},{},{}",
                rc.left,
                rc.top,
                rc.right - rc.left,
                rc.bottom - rc.top
            ),
        );
        r.put(
            &format!("{key}.center_hits"),
            hits((rc.left + rc.right) / 2, (rc.top + rc.bottom) / 2, h, game),
        );
    }

    fn pixel_format(h: HWND) -> i32 {
        unsafe {
            let dc = GetDC(Some(h));
            let pf = GetPixelFormat(dc);
            ReleaseDC(Some(h), dc);
            pf
        }
    }

    fn alpha_at(w: &slint::Window, fx: f32, fy: f32) -> i64 {
        match w.take_snapshot() {
            Ok(b) => {
                let (bw, bh) = (b.width() as usize, b.height() as usize);
                let x = ((bw as f32 * fx) as usize).min(bw.saturating_sub(1));
                let y = ((bh as f32 * fy) as usize).min(bh.saturating_sub(1));
                b.as_slice().get(y * bw + x).map_or(-1, |p| p.a as i64)
            }
            Err(_) => -1,
        }
    }

    fn probe(out: &Path, renderer: &str) -> i32 {
        std::thread::spawn(|| {
            std::thread::sleep(Duration::from_secs(60));
            std::process::exit(3);
        });
        // as main.rs: the winit backend with the overlay hook and an OpenGL renderer
        if let Err(e) = slint::BackendSelector::new()
            .backend_name("winit".into())
            .renderer_name(renderer.into())
            .with_winit_window_attributes_hook(win::attributes_hook)
            .select()
        {
            let _ = std::fs::write(out, format!("error=backend: {e}\n"));
            return 2;
        }
        let game = unsafe { FindWindowW(GAME_CLASS, PCWSTR::null()) }.unwrap_or_default();
        if game.is_invalid() {
            let _ = std::fs::write(out, "error=no game window\n");
            return 2;
        }
        let spy = unsafe {
            SetWindowsHookExW(WH_CALLWNDPROC, Some(spy_proc), None, GetCurrentThreadId())
        };
        // the first set draws in full at once; the second fades as in the app
        dr_app::set_instant_motion(true);
        let o = match Overlays::new(true) {
            Ok(o) => Rc::new(RefCell::new(o)),
            Err(e) => {
                let _ = std::fs::write(out, format!("error=overlays: {e}\n"));
                return 2;
            }
        };
        dr_app::set_instant_motion(false);
        // the fade timings below are the full ones, whatever the PC's animation setting
        dr_app::set_reduce_motion(Some(false));
        let o2 = match Overlays::new(true) {
            Ok(o) => Rc::new(RefCell::new(o)),
            Err(e) => {
                let _ = std::fs::write(out, format!("error=overlays: {e}\n"));
                return 2;
            }
        };
        let report = Rc::new(RefCell::new(Report(String::new())));
        report.borrow_mut().put("spy", spy.is_ok());
        let out: PathBuf = out.to_path_buf();
        // the HWNDs exist only once the event loop runs: the steps run from timers
        let step = {
            let (o, report) = (o.clone(), report.clone());
            move || {
                let mut r = report.borrow_mut();
                let mut o = o.borrow_mut();
                let Some(mut g) = win::game_win(game.0 as isize) else {
                    r.put("error", "game_win failed");
                    return;
                };
                // nothing is ever in front on a hidden desktop
                g.front = true;
                r.put(
                    "game.client",
                    format!(
                        "{},{},{},{}",
                        g.client.x, g.client.y, g.client.w, g.client.h
                    ),
                );
                r.put(
                    "game.frame",
                    format!("{},{},{},{}", g.frame.x, g.frame.y, g.frame.w, g.frame.h),
                );
                r.put("game.scale", g.scale);
                // one at a time, as the app shows them (zone edit covers the whole game)
                show_all(&mut o, &g, &mut |o, which| match which {
                    0 => {
                        check(&mut r, "pill", o.pill.window(), win::Kind::Pill, game);
                        // renderer output: see-through margin, drawn pill (at the right end, 8 px in)
                        r.put("pill.corner_alpha", alpha_at(o.pill.window(), 0.0, 0.0));
                        r.put("pill.body_alpha", alpha_at(o.pill.window(), 0.9, 0.5));
                        // the shadow fits in the window: nothing is drawn on its bottom edge
                        r.put("pill.edge_alpha", alpha_at(o.pill.window(), 0.9, 1.0));
                    }
                    1 => {
                        check(&mut r, "menu", o.menu.window(), win::Kind::Menu, game);
                        r.put("menu.corner_alpha", alpha_at(o.menu.window(), 0.0, 0.0));
                        r.put("menu.card_alpha", alpha_at(o.menu.window(), 0.5, 0.3));
                        r.put("menu.edge_alpha", alpha_at(o.menu.window(), 0.5, 0.0));
                    }
                    _ => check(&mut r, "zone", o.zones.window(), win::Kind::Zone, game),
                });
                r.put("show_error", o.last_error.take().unwrap_or_default());
                // the menu's card and key hints are centered on the client area
                if let Some(h) = win::hwnd_of(o.menu.window()) {
                    let rc = rect(h);
                    let s = o.menu.window().scale_factor();
                    let cy = rc.top + (o.menu.get_group_cy() * s).round() as i32;
                    r.put(
                        "menu.center_dx",
                        (rc.left + rc.right) / 2 - (g.client.x + g.client.w / 2),
                    );
                    r.put("menu.center_dy", cy - (g.client.y + g.client.h / 2));
                    let ma = unsafe {
                        SendMessageW(
                            h,
                            WM_MOUSEACTIVATE,
                            Some(WPARAM(game.0 as usize)),
                            Some(LPARAM(0)),
                        )
                    };
                    r.put("menu.mouseactivate", ma.0);
                }
                if let Some(h) = win::hwnd_of(o.zones.window()) {
                    let ma = unsafe {
                        SendMessageW(
                            h,
                            WM_MOUSEACTIVATE,
                            Some(WPARAM(game.0 as usize)),
                            Some(LPARAM(0)),
                        )
                    };
                    r.put("zone.mouseactivate", ma.0);
                }
                if let Some(h) = win::hwnd_of(o.pill.window()) {
                    let (mut alpha, mut flags) = (0u8, LAYERED_WINDOW_ATTRIBUTES_FLAGS(0));
                    let ok = unsafe {
                        GetLayeredWindowAttributes(h, None, Some(&mut alpha), Some(&mut flags))
                            .is_ok()
                    };
                    r.put("pill.layered_ok", ok);
                    r.put("pill.layered_alpha", alpha);
                    r.put("pill.pixel_format", pixel_format(h));
                }
            }
        };
        let again = {
            let (o, report) = (o.clone(), report.clone());
            move || {
                let mut r = report.borrow_mut();
                let mut o = o.borrow_mut();
                let Some(mut g) = win::game_win(game.0 as isize) else {
                    return;
                };
                g.front = true;
                // hide every overlay and show it again: the styles stay, the HWND stays
                o.pill_on(false, Some(&g), PillAt::TopRight);
                o.menu_off();
                o.shown = dr_runtime::keys::Overlay::None;
                o.zones_off();
                show_all(&mut o, &g, &mut |o, which| match which {
                    0 => check(&mut r, "pill2", o.pill.window(), win::Kind::Pill, game),
                    1 => check(&mut r, "menu2", o.menu.window(), win::Kind::Menu, game),
                    _ => check(&mut r, "zone2", o.zones.window(), win::Kind::Zone, game),
                });
            }
        };
        // fades: show one window of the second set, then hide it, look at it while it fades and after
        let fade_show = |which: u8| {
            let o2 = o2.clone();
            move || {
                let mut o = o2.borrow_mut();
                let Some(mut g) = win::game_win(game.0 as isize) else {
                    return;
                };
                g.front = true;
                match which {
                    0 => o.pill_on(true, Some(&g), PillAt::TopRight),
                    1 => {
                        if o.menu_on(&g) {
                            o.shown = dr_runtime::keys::Overlay::Menu;
                        }
                    }
                    _ => {
                        o.zones_on(&g);
                    }
                }
            }
        };
        let fade_hide = |which: u8| {
            let (o2, report) = (o2.clone(), report.clone());
            move || {
                let mut r = report.borrow_mut();
                let mut o = o2.borrow_mut();
                let Some(g) = win::game_win(game.0 as isize) else {
                    return;
                };
                let (key, kind) = match which {
                    0 => ("pillfade", win::Kind::Pill),
                    1 => ("menufade", win::Kind::Menu),
                    _ => ("zonefade", win::Kind::Zone),
                };
                // the fade in has ended: the window is drawn in full
                match which {
                    0 => r.put("pillfade.shown_alpha", alpha_at(o.pill.window(), 0.9, 0.5)),
                    1 => r.put("menufade.shown_alpha", alpha_at(o.menu.window(), 0.5, 0.3)),
                    _ => {}
                }
                match which {
                    0 => o.pill_on(false, Some(&g), PillAt::TopRight),
                    1 => {
                        o.menu_off();
                        o.shown = dr_runtime::keys::Overlay::None;
                    }
                    _ => o.zones_off(),
                }
                let w = match which {
                    0 => o.pill.window(),
                    1 => o.menu.window(),
                    _ => o.zones.window(),
                };
                check(&mut r, &format!("{key}.fading"), w, kind, game);
            }
        };
        let fade_mid = |which: u8| {
            let (o2, report) = (o2.clone(), report.clone());
            move || {
                let mut r = report.borrow_mut();
                let o = o2.borrow();
                let (key, w, kind) = match which {
                    0 => ("pillfade", o.pill.window(), win::Kind::Pill),
                    1 => ("menufade", o.menu.window(), win::Kind::Menu),
                    _ => ("zonefade", o.zones.window(), win::Kind::Zone),
                };
                check(&mut r, &format!("{key}.mid"), w, kind, game);
            }
        };
        let fade_after = |which: u8| {
            let (o2, report) = (o2.clone(), report.clone());
            move || {
                let mut r = report.borrow_mut();
                let o = o2.borrow();
                let (key, w) = match which {
                    0 => ("pillfade", o.pill.window()),
                    1 => ("menufade", o.menu.window()),
                    _ => ("zonefade", o.zones.window()),
                };
                let visible =
                    win::hwnd_of(w).is_some_and(|h| unsafe { IsWindowVisible(h).as_bool() });
                r.put(&format!("{key}.after_visible"), visible);
            }
        };
        // the positive control: a plain window of this thread shown last does activate here
        let control: Rc<RefCell<Option<dr_app::MainWindow>>> = Rc::new(RefCell::new(None));
        let control_step = {
            let (control, report) = (control.clone(), report.clone());
            move || {
                let mut r = report.borrow_mut();
                match dr_app::MainWindow::new() {
                    Ok(c) => {
                        if let Err(e) = c.show() {
                            r.put("control.error", e);
                        }
                        *control.borrow_mut() = Some(c);
                    }
                    Err(e) => r.put("control.error", e),
                }
            }
        };
        let control_check = {
            let (control, report) = (control.clone(), report.clone());
            move || {
                let mut r = report.borrow_mut();
                let c = control.borrow();
                let Some(c) = c.as_ref() else { return };
                // the main window's HWND comes a little after its show
                r.put("control.hwnd", win::hwnd_of(c.window()).is_some());
                if let Some(h) = win::hwnd_of(c.window()) {
                    let a = activity(h);
                    r.put("control.activations", a.activate);
                    r.put("control.active_is_self", thread_active() == h);
                    r.put("control.hardened", win::is_hardened(h));
                }
            }
        };
        let done = {
            let (report, out) = (report.clone(), out.clone());
            move || {
                let _ = std::fs::write(&out, &report.borrow().0);
                let _ = slint::quit_event_loop();
            }
        };
        slint::Timer::single_shot(Duration::from_millis(400), step);
        slint::Timer::single_shot(Duration::from_millis(1200), again);
        // each fade: shown, 300 ms later hidden (the fade starts), 40 ms into it and 400 ms after it
        for which in 0..3u8 {
            let t = 2000 + 800 * which as u64;
            slint::Timer::single_shot(Duration::from_millis(t), fade_show(which));
            slint::Timer::single_shot(Duration::from_millis(t + 300), fade_hide(which));
            slint::Timer::single_shot(Duration::from_millis(t + 340), fade_mid(which));
            slint::Timer::single_shot(Duration::from_millis(t + 700), fade_after(which));
        }
        // the control last: once it is active it sits over the game, so it would spoil the hit tests above
        slint::Timer::single_shot(Duration::from_millis(4600), control_step);
        slint::Timer::single_shot(Duration::from_millis(5800), control_check);
        slint::Timer::single_shot(Duration::from_millis(6200), done);
        let _ = slint::run_event_loop_until_quit();
        drop(control);
        drop(o2);
        0
    }

    /// Shows the pill, the menu and zone edit in turn, each alone; `each` checks it while it is the only one.
    fn show_all(o: &mut Overlays, g: &GameWin, each: &mut dyn FnMut(&Overlays, u8)) {
        o.pill_on(true, Some(g), PillAt::TopRight);
        each(o, 0);
        o.pill_on(false, Some(g), PillAt::TopRight);
        if o.menu_on(g) {
            o.shown = dr_runtime::keys::Overlay::Menu;
        }
        each(o, 1);
        o.menu_off();
        o.shown = dr_runtime::keys::Overlay::None;
        o.zones_on(g);
        each(o, 2);
        o.zones_off();
    }
}
