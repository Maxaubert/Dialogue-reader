//! Single instance (M5, spec 2.9). Every test uses its own instance name, so a DialogReader that runs for real is
//! never reached. The app test runs DialogReader.exe on a hidden desktop with a temp APPDATA: nothing shows on the
//! user's screen, no tray icon is made (`--no-tray`), the real data folder is not touched.

#![cfg(windows)]

use std::cell::Cell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use dr_app::single::{self, Claim};
use dr_runtime::testwin::HiddenDesktop;
use windows::Win32::Foundation::{CloseHandle, HWND, LPARAM, WAIT_OBJECT_0};
use windows::Win32::System::StationsAndDesktops::{EnumDesktopWindows, HDESK};
use windows::Win32::System::Threading::{
    CREATE_UNICODE_ENVIRONMENT, CreateProcessW, GetExitCodeProcess, PROCESS_INFORMATION,
    STARTUPINFOW, TerminateProcess, WaitForSingleObject,
};
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetWindowTextW, GetWindowThreadProcessId, IsWindowVisible, MSG, PM_REMOVE,
    PeekMessageW,
};
use windows::core::{BOOL, PWSTR};

fn unique(tag: &str) -> String {
    let t = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    format!("test-{tag}-{}-{t:x}", std::process::id())
}

#[test]
fn the_name_can_be_changed_for_tests() {
    assert_eq!(single::name_of(None), "DialogReader");
    assert_eq!(single::name_of(Some("  ")), "DialogReader");
    assert_eq!(single::name_of(Some("t1")), "DialogReader.t1");
    assert_eq!(
        single::mutex_name("DialogReader"),
        r"Local\DialogReader.SingleInstance"
    );
}

#[test]
fn the_second_claim_finds_the_first() {
    let name = single::name_of(Some(&unique("claim")));
    let first = match single::claim(&name) {
        Claim::First(g) => g,
        Claim::Second => panic!("nobody holds a new name"),
    };
    assert!(matches!(single::claim(&name), Claim::Second));
    drop(first);
    assert!(matches!(single::claim(&name), Claim::First(_)));
}

#[test]
fn access_denied_means_another_dialogreader_runs_as_administrator() {
    use windows::Win32::Foundation::{ERROR_ACCESS_DENIED, ERROR_INVALID_NAME};
    assert!(matches!(
        single::claim_failed(ERROR_ACCESS_DENIED.to_hresult()),
        Claim::Second
    ));
    assert!(matches!(
        single::claim_failed(ERROR_INVALID_NAME.to_hresult()),
        Claim::First(_)
    ));
}

#[test]
fn show_reaches_the_listener() {
    let name = single::name_of(Some(&unique("listen")));
    let shown = Rc::new(Cell::new(0));
    let s = shown.clone();
    let l = single::listen(&name).expect("listen");
    l.on_show(move || s.set(s.get() + 1));
    assert!(single::find(&name).is_some());
    let n = name.clone();
    let poster = std::thread::spawn(move || single::show_first(&n, Duration::from_secs(5)));
    let end = Instant::now() + Duration::from_secs(10);
    while shown.get() == 0 && Instant::now() < end {
        pump();
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(poster.join().unwrap(), "the message was posted");
    assert_eq!(shown.get(), 1);
    drop(l);
    assert!(single::find(&name).is_none());
    // nobody listens: the second start gives up after the wait
    assert!(!single::show_first(&name, Duration::from_millis(100)));
}

#[test]
fn a_show_request_before_the_window_is_ready_is_kept() {
    let name = single::name_of(Some(&unique("early")));
    let l = single::listen(&name).expect("listen");
    // a second start while the first still loads its voices and window
    post(l.hwnd(), single::WM_SHOW, 0);
    pump();
    let shown = Rc::new(Cell::new(0));
    let s = shown.clone();
    l.on_show(move || s.set(s.get() + 1));
    assert_eq!(
        shown.get(),
        1,
        "the early request runs when the handler is set"
    );
    post(l.hwnd(), single::WM_SHOW, 0);
    pump();
    assert_eq!(shown.get(), 2);
}

/// Broadcasts (Explorer's TaskbarCreated, the end of the session) reach only top-level windows, not message-only
/// ones; the window must also never show.
#[test]
fn the_listener_is_a_hidden_top_level_window() {
    use windows::Win32::UI::WindowsAndMessaging::{GA_PARENT, GetAncestor, GetDesktopWindow};
    let name = single::name_of(Some(&unique("top")));
    let l = single::listen(&name).expect("listen");
    unsafe {
        assert_eq!(GetAncestor(l.hwnd(), GA_PARENT), GetDesktopWindow());
        assert!(!IsWindowVisible(l.hwnd()).as_bool());
    }
}

#[test]
fn taskbar_created_asks_for_a_new_tray_icon() {
    let name = single::name_of(Some(&unique("taskbar")));
    let l = single::listen(&name).expect("listen");
    let made = Rc::new(Cell::new(0));
    let m = made.clone();
    l.on_taskbar_created(move || m.set(m.get() + 1));
    // what Explorer broadcasts after it restarts (sent to this window only: no other app is reached)
    let msg = single::taskbar_created();
    assert_ne!(msg, 0);
    post(l.hwnd(), msg, 0);
    pump();
    assert_eq!(made.get(), 1);
}

#[test]
fn the_installer_and_the_end_of_the_session_end_the_app() {
    use windows::Win32::UI::WindowsAndMessaging::{
        SendMessageW, WM_ENDSESSION, WM_QUERYENDSESSION,
    };
    let name = single::name_of(Some(&unique("quit")));
    let l = single::listen(&name).expect("listen");
    let quits = Rc::new(Cell::new(0));
    let q = quits.clone();
    l.on_quit(move || q.set(q.get() + 1));
    // the installer and uninstaller (installer/DialogReader.iss posts 32850)
    assert_eq!(single::WM_QUIT_APP, 32850);
    post(l.hwnd(), single::WM_QUIT_APP, 0);
    pump();
    assert_eq!(quits.get(), 1);
    // sign-out, shutdown or Restart Manager: the app agrees, then ends when the session really ends
    let ok = unsafe { SendMessageW(l.hwnd(), WM_QUERYENDSESSION, None, None) };
    assert_eq!(ok.0, 1);
    assert_eq!(quits.get(), 1);
    post(l.hwnd(), WM_ENDSESSION, 0);
    pump();
    assert_eq!(quits.get(), 1, "the session goes on: nothing ends");
    post(l.hwnd(), WM_ENDSESSION, 1);
    pump();
    assert_eq!(quits.get(), 2);
}

fn post(h: HWND, msg: u32, wp: usize) {
    use windows::Win32::Foundation::WPARAM;
    use windows::Win32::UI::WindowsAndMessaging::PostMessageW;
    unsafe { PostMessageW(Some(h), msg, WPARAM(wp), LPARAM(0)) }.expect("PostMessage");
}

fn pump() {
    unsafe {
        let mut m = MSG::default();
        while PeekMessageW(&mut m, None, 0, 0, PM_REMOVE).as_bool() {
            DispatchMessageW(&m);
        }
    }
}

/// A process on the hidden desktop with these extra environment variables.
struct Proc(PROCESS_INFORMATION);

impl Proc {
    fn start(exe: &str, args: &str, desktop: &str, env: &[(&str, String)]) -> Proc {
        let mut vars: Vec<(String, String)> = std::env::vars()
            .filter(|(k, _)| !env.iter().any(|(e, _)| e.eq_ignore_ascii_case(k)))
            .collect();
        vars.extend(env.iter().map(|(k, v)| (k.to_string(), v.clone())));
        vars.sort_by_key(|(k, _)| k.to_uppercase());
        let mut block: Vec<u16> = Vec::new();
        for (k, v) in &vars {
            block.extend(format!("{k}={v}").encode_utf16());
            block.push(0);
        }
        block.push(0);
        let mut cmd: Vec<u16> = format!("\"{exe}\" {args}").encode_utf16().collect();
        cmd.push(0);
        let mut desk: Vec<u16> = format!("WinSta0\\{desktop}").encode_utf16().collect();
        desk.push(0);
        let si = STARTUPINFOW {
            cb: std::mem::size_of::<STARTUPINFOW>() as u32,
            lpDesktop: PWSTR(desk.as_mut_ptr()),
            ..Default::default()
        };
        let mut pi = PROCESS_INFORMATION::default();
        unsafe {
            CreateProcessW(
                None,
                Some(PWSTR(cmd.as_mut_ptr())),
                None,
                None,
                false,
                CREATE_UNICODE_ENVIRONMENT,
                Some(block.as_ptr().cast()),
                None,
                &si,
                &mut pi,
            )
        }
        .expect("CreateProcess");
        Proc(pi)
    }

    /// The exit code once the process ended within `wait`.
    fn wait(&self, wait: Duration) -> Option<u32> {
        unsafe {
            if WaitForSingleObject(self.0.hProcess, wait.as_millis() as u32) != WAIT_OBJECT_0 {
                return None;
            }
            let mut code = 0u32;
            GetExitCodeProcess(self.0.hProcess, &mut code).ok()?;
            Some(code)
        }
    }
}

impl Drop for Proc {
    fn drop(&mut self) {
        unsafe {
            let _ = TerminateProcess(self.0.hProcess, 1);
            let _ = WaitForSingleObject(self.0.hProcess, 5000);
            let _ = CloseHandle(self.0.hProcess);
            let _ = CloseHandle(self.0.hThread);
        }
    }
}

/// Visible main windows (title "DialogReader") of process `pid` on the desktop.
fn visible_windows(desk: isize, pid: u32) -> Vec<HWND> {
    struct Find {
        pid: u32,
        found: Vec<HWND>,
    }
    unsafe extern "system" fn each(h: HWND, lp: LPARAM) -> BOOL {
        let f = unsafe { &mut *(lp.0 as *mut Find) };
        let mut p = 0u32;
        unsafe { GetWindowThreadProcessId(h, Some(&mut p)) };
        if p == f.pid && unsafe { IsWindowVisible(h) }.as_bool() {
            // winit's own message window counts as visible; the main window has the app's title
            let mut t = [0u16; 64];
            let n = unsafe { GetWindowTextW(h, &mut t) } as usize;
            if String::from_utf16_lossy(&t[..n]) == "DialogReader" {
                f.found.push(h);
            }
        }
        true.into()
    }
    let mut f = Find {
        pid,
        found: Vec::new(),
    };
    unsafe {
        let _ = EnumDesktopWindows(
            Some(HDESK(desk as *mut _)),
            Some(each),
            LPARAM(&mut f as *mut Find as isize),
        );
    }
    f.found
}

fn wait_until(wait: Duration, mut f: impl FnMut() -> bool) -> bool {
    let end = Instant::now() + wait;
    while Instant::now() < end {
        if f() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    f()
}

#[test]
fn a_second_start_shows_the_first_window_and_exits() {
    use windows::Win32::UI::WindowsAndMessaging::{SW_HIDE, ShowWindow};
    let exe = env!("CARGO_BIN_EXE_DialogReader");
    let desk = HiddenDesktop::new().expect("hidden desktop");
    let data = tempfile::tempdir().unwrap();
    let id = unique("app");
    let name = single::name_of(Some(&id));
    let env = [
        ("DIALOGREADER_INSTANCE", id.clone()),
        (
            "APPDATA",
            data.path().join("Roaming").to_string_lossy().into_owned(),
        ),
        (
            "LOCALAPPDATA",
            data.path().join("Local").to_string_lossy().into_owned(),
        ),
    ];
    // Start with Windows, but no tray icon (`--no-tray`): the window shows, so the app never runs with nothing on
    // screen
    let first = Proc::start(exe, "--fake --minimized --no-tray", desk.name(), &env);
    let pid = first.0.dwProcessId;
    // the listener window belongs to a desktop: look from a thread on the hidden one
    let find = |name: &str| {
        let n = name.to_string();
        desk.run(move || single::find(&n).map(|h| h.0 as isize))
            .ok()
            .flatten()
    };
    let started = wait_until(Duration::from_secs(60), || find(&name).is_some());
    assert!(
        started,
        "the first DialogReader did not start (exit {:?})",
        first.wait(Duration::ZERO)
    );
    assert_eq!(first.wait(Duration::ZERO), None, "the first one runs");
    assert!(
        wait_until(Duration::from_secs(30), || !visible_windows(
            desk.handle(),
            pid
        )
        .is_empty()),
        "--minimized without a tray icon must show the window"
    );
    // hidden, as in the tray
    for h in visible_windows(desk.handle(), pid) {
        unsafe {
            let _ = ShowWindow(h, SW_HIDE);
        }
    }
    assert!(wait_until(Duration::from_secs(10), || visible_windows(
        desk.handle(),
        pid
    )
    .is_empty()));

    // the second one asks it to show the window and ends
    let second = Proc::start(exe, "--fake", desk.name(), &env);
    assert_eq!(second.wait(Duration::from_secs(30)), Some(0));
    assert!(
        wait_until(Duration::from_secs(30), || !visible_windows(
            desk.handle(),
            pid
        )
        .is_empty()),
        "the first DialogReader did not show its window"
    );
    assert_eq!(first.wait(Duration::ZERO), None, "the first one still runs");

    // the installer and uninstaller ask it to end (installer/DialogReader.iss): it ends by itself
    let h = find(&name).expect("the listener window");
    unsafe {
        windows::Win32::UI::WindowsAndMessaging::PostMessageW(
            Some(HWND(h as *mut _)),
            single::WM_QUIT_APP,
            windows::Win32::Foundation::WPARAM(0),
            LPARAM(0),
        )
    }
    .expect("PostMessage");
    assert_eq!(
        first.wait(Duration::from_secs(30)),
        Some(0),
        "the first DialogReader did not end when asked"
    );
}
