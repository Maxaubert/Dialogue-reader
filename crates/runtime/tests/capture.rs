//! The window list and the screenshot, against windows these tests make. Nothing shows on the user's screen:
//! the list test runs on a hidden desktop, and the screenshot test captures a window left of every monitor
//! (Windows Graphics Capture refuses windows on another desktop on this PC, docs/arena-spec.md).

#![cfg(windows)]

use dr_runtime::screenshot;
use dr_runtime::testwin::{HiddenDesktop, Kind, TestWindow};
use dr_runtime::windows_list::{self, ListOpts, WinMode};

/// The primary monitor rect in physical pixels.
fn primary_monitor() -> (i32, i32, i32, i32) {
    use windows::Win32::Foundation::POINT;
    use windows::Win32::Graphics::Gdi::{
        GetMonitorInfoW, MONITOR_DEFAULTTOPRIMARY, MONITORINFO, MonitorFromPoint,
    };
    use windows::Win32::UI::HiDpi::{
        DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetThreadDpiAwarenessContext,
    };
    unsafe {
        let old = SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        let mon = MonitorFromPoint(POINT { x: 0, y: 0 }, MONITOR_DEFAULTTOPRIMARY);
        let mut mi = MONITORINFO {
            cbSize: size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        assert!(GetMonitorInfoW(mon, &mut mi).as_bool());
        SetThreadDpiAwarenessContext(old);
        let r = mi.rcMonitor;
        (r.left, r.top, r.right - r.left, r.bottom - r.top)
    }
}

#[test]
fn window_list_on_a_hidden_desktop() {
    let desk = HiddenDesktop::new().expect("hidden desktop");
    let framed = TestWindow::spawn(
        Kind::Framed {
            title: "DR test notes".into(),
            w: 640,
            h: 360,
        },
        Some(&desk),
    )
    .unwrap();
    let (mx, my, mw, mh) = primary_monitor();
    let game = TestWindow::spawn(
        Kind::Popup {
            title: "DR test game".into(),
            x: mx,
            y: my,
            w: mw,
            h: mh,
        },
        Some(&desk),
    )
    .unwrap();
    let (framed_hwnd, game_hwnd) = (framed.hwnd, game.hwnd);

    let opts = ListOpts {
        profile_exes: vec![],
        skip_pid: None,
        icons: true,
    };
    let list = desk.run(move || windows_list::list(&opts)).unwrap();
    let names: Vec<_> = list.iter().map(|w| w.title.as_str()).collect();
    let g = list
        .iter()
        .find(|w| w.hwnd == game_hwnd)
        .unwrap_or_else(|| panic!("game window missing from {names:?}"));
    let f = list
        .iter()
        .find(|w| w.hwnd == framed_hwnd)
        .unwrap_or_else(|| panic!("framed window missing from {names:?}"));

    // the full-monitor popup is a borderless game, listed first
    assert_eq!(g.title, "DR test game");
    assert_eq!((g.width, g.height), (mw as u32, mh as u32));
    assert_eq!(g.mode, WinMode::Borderless);
    assert!(g.game);
    assert!(
        list.iter().position(|w| w.hwnd == game_hwnd)
            < list.iter().position(|w| w.hwnd == framed_hwnd)
    );

    // the framed window: exact client size, windowed, the exe of this test
    assert_eq!((f.width, f.height), (640, 360));
    assert_eq!(f.mode, WinMode::Windowed);
    assert_eq!(f.meta(), "640 × 360 · windowed");
    assert_eq!(f.pid, std::process::id());
    let me = std::env::current_exe().unwrap();
    assert_eq!(f.exe, me.file_name().unwrap().to_string_lossy());
    assert!(
        f.icon
            .as_ref()
            .is_some_and(|i| i.width > 0 && i.rgba.len() == (i.width * i.height * 4) as usize)
    );
    // a profile for its exe makes it a game (the 3D library check may already say yes: the screenshot test in
    // this process loads dxgi.dll)
    let opts = ListOpts {
        profile_exes: vec![f.exe.to_uppercase()],
        skip_pid: None,
        icons: false,
    };
    let again = desk.run(move || windows_list::list(&opts)).unwrap();
    assert!(again.iter().find(|w| w.hwnd == framed_hwnd).unwrap().game);

    // skip_pid leaves out this process
    let opts = ListOpts {
        profile_exes: vec![],
        skip_pid: Some(std::process::id()),
        icons: false,
    };
    let none = desk.run(move || windows_list::list(&opts)).unwrap();
    assert!(none.iter().all(|w| w.pid != std::process::id()));

    drop(framed);
    drop(game);
}

#[test]
fn screenshot_of_an_offscreen_window() {
    let (w, h) = (320u32, 200u32);
    let (red, blue) = ([220u8, 30, 40, 255], [20u8, 60, 230, 255]);
    let win = TestWindow::offscreen_halves(w, h, red, blue).expect("offscreen window");
    let shot = screenshot::capture(win.hwnd).expect("capture");
    assert_eq!((shot.width, shot.height), (w, h));
    assert_eq!(shot.rgba.len(), (w * h * 4) as usize);
    // red left, blue right: the channels are in RGBA order
    let close = |a: [u8; 4], b: [u8; 4]| (0..3).all(|c| a[c].abs_diff(b[c]) <= 2);
    assert!(
        close(shot.pixel(10, 100), red),
        "left {:?}",
        shot.pixel(10, 100)
    );
    assert!(
        close(shot.pixel(w - 10, 100), blue),
        "right {:?}",
        shot.pixel(w - 10, 100)
    );
}

#[test]
fn screenshot_of_a_gone_window_fails_plainly() {
    let win = TestWindow::offscreen_halves(64, 64, [0, 0, 0, 255], [255, 255, 255, 255]).unwrap();
    let hwnd = win.hwnd;
    drop(win);
    assert!(screenshot::capture(hwnd).is_err());
}
