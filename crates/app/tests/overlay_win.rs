//! The real overlay windows on a hidden desktop (spec 2.7, app-plan M4): `overlay-probe run` makes a desktop that
//! is never shown, starts a dummy game with a title bar on it, and shows the pill, the quick menu and zone edit
//! over it through `dr_app::overlay` (the app's own code), with the renderer the app ships (skia-opengl). Nothing
//! shows on the user's screen and nothing takes the foreground. See docs/overlay-spike.md for how focus is
//! measured.

#![cfg(windows)]

use std::collections::HashMap;

fn run(renderer: &str) -> HashMap<String, String> {
    let exe = env!("CARGO_BIN_EXE_overlay-probe");
    let out = std::env::temp_dir().join(format!(
        "dr-overlay-probe-{}-{renderer}.txt",
        std::process::id()
    ));
    let status = std::process::Command::new(exe)
        .args(["run", "--out"])
        .arg(&out)
        .args(["--renderer", renderer])
        .status()
        .expect("overlay-probe runs");
    let text = std::fs::read_to_string(&out).unwrap_or_default();
    let _ = std::fs::remove_file(&out);
    assert!(status.success(), "probe exit {status:?}: {text}");
    let map: HashMap<String, String> = text
        .lines()
        .filter_map(|l| l.split_once('='))
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    assert!(!map.contains_key("error"), "probe error: {text}");
    map
}

fn get<'a>(m: &'a HashMap<String, String>, k: &str) -> &'a str {
    m.get(k)
        .map(String::as_str)
        .unwrap_or_else(|| panic!("{k} missing in {m:?}"))
}

fn num(m: &HashMap<String, String>, k: &str) -> i64 {
    get(m, k)
        .parse()
        .unwrap_or_else(|_| panic!("{k} is not a number: {}", get(m, k)))
}

fn rect(m: &HashMap<String, String>, k: &str) -> Vec<i64> {
    get(m, k).split(',').map(|v| v.parse().unwrap()).collect()
}

#[test]
fn overlay_windows_keep_their_styles_and_never_activate() {
    let m = run("skia-opengl");
    assert_eq!(get(&m, "spy"), "true");
    assert_eq!(get(&m, "show_error"), "", "an overlay could not be shown");

    // the positive control: a plain window does activate on this desktop, so the checks below can fail
    assert!(
        num(&m, "control.activations") >= 1,
        "control did not activate: {m:?}"
    );
    assert_eq!(get(&m, "control.active_is_self"), "true");
    assert_eq!(get(&m, "control.hardened"), "false");

    for key in ["pill", "menu", "zone", "pill2", "menu2", "zone2"] {
        let g = |f: &str| get(&m, &format!("{key}.{f}")).to_string();
        assert_eq!(g("has_all"), "true", "{key}: overlay styles missing");
        assert_eq!(g("appwindow"), "false", "{key}: has a taskbar button");
        assert_eq!(g("hardened"), "true", "{key}: not hardened");
        assert_eq!(g("visible"), "true", "{key}: not visible");
        assert_eq!(g("activations"), "0", "{key}: got WM_ACTIVATE");
        assert_eq!(g("setfocus"), "0", "{key}: got WM_SETFOCUS");
        assert_eq!(
            g("active_is_self"),
            "false",
            "{key}: became the active window"
        );
    }
    // hide and show keeps the same window
    for (a, b) in [("pill", "pill2"), ("menu", "menu2"), ("zone", "zone2")] {
        assert_eq!(
            get(&m, &format!("{a}.hwnd")),
            get(&m, &format!("{b}.hwnd")),
            "{b}: new HWND"
        );
    }

    // the pill is click-through; the menu and zone edit take the mouse but never activate on a click
    assert_eq!(get(&m, "pill.center_hits"), "game");
    assert_eq!(get(&m, "pill2.center_hits"), "game");
    assert_eq!(get(&m, "menu.center_hits"), "self");
    assert_eq!(get(&m, "zone.center_hits"), "self");
    assert_eq!(num(&m, "menu.mouseactivate"), 3, "MA_NOACTIVATE");
    assert_eq!(num(&m, "zone.mouseactivate"), 3, "MA_NOACTIVATE");

    // the pill is layered with full alpha, drawn with OpenGL (not a silent software fallback)
    assert_eq!(get(&m, "pill.layered_ok"), "true");
    assert_eq!(num(&m, "pill.layered_alpha"), 255);
    assert!(
        num(&m, "pill.pixel_format") > 0,
        "no GL pixel format: the renderer fell back"
    );

    // the shipped renderer draws see-through margins and the cards
    assert_eq!(
        num(&m, "pill.corner_alpha"),
        0,
        "pill margin not see-through"
    );
    assert!(num(&m, "pill.body_alpha") >= 200, "pill not drawn");
    assert_eq!(
        num(&m, "menu.corner_alpha"),
        0,
        "menu margin not see-through"
    );
    assert_eq!(num(&m, "menu.card_alpha"), 255, "menu card not opaque");

    // placement: the pill at the top right of the client area, the menu centered on it, zone edit over what the
    // capture covers (the game has a title bar, so that is more than the client area)
    let client = rect(&m, "game.client");
    let frame = rect(&m, "game.frame");
    assert_ne!(client, frame, "the probe game should have a title bar");
    assert_eq!(rect(&m, "zone.rect"), frame);
    let scale: f64 = get(&m, "game.scale").parse().unwrap();
    let pill = rect(&m, "pill.rect");
    let inset = |v: f64| (v * scale).round() as i64;
    assert_eq!(
        pill[0] + pill[2] - inset(8.0),
        client[0] + client[2] - inset(24.0),
        "pill right edge"
    );
    assert_eq!(pill[1] + inset(8.0), client[1] + inset(24.0), "pill top");
    assert!(
        num(&m, "menu.center_dx").abs() <= 1,
        "menu not centered across"
    );
    assert!(
        num(&m, "menu.center_dy").abs() <= 1,
        "menu not centered down"
    );
}
