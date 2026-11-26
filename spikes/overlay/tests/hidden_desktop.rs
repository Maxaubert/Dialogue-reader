//! Runs the dummy game and the probe on a new hidden desktop (never switched to, nothing on screen) and
//! checks what the probe measured. See docs/overlay-spike.md for what each field means.

use std::path::{Path, PathBuf};

use serde_json::Value;

fn run(args: &[&str], tag: &str) -> Value {
    let exe = Path::new(env!("CARGO_BIN_EXE_overlay-spike"));
    let out: PathBuf = std::env::temp_dir().join(format!("overlay-spike-test-{}-{tag}.json", std::process::id()));
    let r = overlay_spike::win::run_hidden(exe, args, &out).expect("hidden desktop run");
    let _ = std::fs::remove_file(&out);
    assert_eq!(r.probe_exit, Some(0), "probe exit code");
    let json = r.json.expect("probe wrote no report");
    let v: Value = serde_json::from_str(&json).expect("report is JSON");
    assert!(v.get("error").is_none(), "probe error: {json}");
    v
}

fn b(v: &Value, path: &str) -> bool {
    v.pointer(path).and_then(Value::as_bool).unwrap_or_else(|| panic!("{path} missing or not a bool"))
}

fn n(v: &Value, path: &str) -> i64 {
    v.pointer(path).and_then(Value::as_i64).unwrap_or_else(|| panic!("{path} missing or not a number"))
}

fn s<'a>(v: &'a Value, path: &str) -> &'a str {
    v.pointer(path).and_then(Value::as_str).unwrap_or_else(|| panic!("{path} missing or not a string"))
}

#[test]
fn slint_overlays_keep_styles_and_never_activate() {
    let v = run(&["--renderer", "femtovg"], "hardened");

    // The positive control: a plain Slint window does activate on this desktop, so the checks below can fail.
    assert!(n(&v, "/control/activations") >= 1, "control window did not activate: the focus checks prove nothing");
    assert!(b(&v, "/control/ui_thread_active_is_plain"));
    // The mark that show_overlay uses: the plain window has none, and showing a hardened overlay again
    // through show_overlay does not harden it a second time.
    assert!(!b(&v, "/control/plain_is_hardened"));
    assert!(b(&v, "/show_overlay_again/ok"));
    assert!(!b(&v, "/show_overlay_again/hardened_now"));
    assert!(b(&v, "/show_overlay_again/same_hwnd"));

    // Not checked: the game thread's active window. It stays the game even while the control activates
    // (`/control/game_thread_active_is_game` is true), so it can not fail. Only the per-window WM_ACTIVATE /
    // WM_SETFOCUS counts and the UI thread's active window are evidence.

    // harden reported no failed Win32 call.
    for key in ["pill_show", "menu_show", "zone_show"] {
        assert!(b(&v, &format!("/{key}/harden_ok")), "{key}: harden failed: {}", v[key]["harden_error"]);
    }

    // Slint keeps the same HWND across hide and show. If an update changes this, a new HWND would start
    // without the styles: M4 must harden on every show where the HWND is new (overlay::show_overlay).
    for (first, again) in [("pill", "pill_reshow"), ("pill", "pill_prop_change"), ("menu", "menu_reshow"), ("zone", "zone_reshow")] {
        assert_eq!(s(&v, &format!("/{first}/hwnd")), s(&v, &format!("/{again}/hwnd")), "{again}: new HWND");
    }

    for key in ["pill", "pill_reshow", "pill_prop_change", "menu", "menu_reshow", "zone", "zone_reshow"] {
        let p = format!("/{key}");
        assert!(b(&v, &format!("{p}/has_all")), "{key}: styles missing {}", s(&v, &format!("{p}/missing")));
        assert!(!b(&v, &format!("{p}/appwindow")), "{key}: has WS_EX_APPWINDOW (taskbar button)");
        assert!(b(&v, &format!("{p}/is_hardened")), "{key}: HWND not marked as hardened");
        assert!(b(&v, &format!("{p}/visible")), "{key}: not visible");
        assert_eq!(n(&v, &format!("{p}/activations")), 0, "{key}: got WM_ACTIVATE");
        assert_eq!(n(&v, &format!("{p}/setfocus")), 0, "{key}: got WM_SETFOCUS");
        assert!(!b(&v, &format!("{p}/ui_thread_active_is_self")), "{key}: became the active window");
    }

    // Click-through: a click on the pill hits the game. The menu and zone edit take the click themselves.
    for key in ["pill", "pill_reshow", "pill_prop_change"] {
        assert_eq!(s(&v, &format!("/{key}/center_hits")), "game", "{key} is not click-through");
    }
    for key in ["menu", "menu_reshow", "zone", "zone_reshow"] {
        assert_eq!(s(&v, &format!("/{key}/center_hits")), "self", "{key} does not take the mouse");
    }

    // A click would not activate the menu or zone edit (MA_NOACTIVATE = 3).
    assert_eq!(n(&v, "/menu_mouseactivate"), 3);
    assert_eq!(n(&v, "/zone_mouseactivate"), 3);

    // The pill is layered with full constant alpha (otherwise a layered window is never drawn).
    assert!(b(&v, "/pill_pixels/layered_attrs/ok"));
    assert_eq!(n(&v, "/pill_pixels/layered_attrs/alpha"), 255);

    // The renderer was really OpenGL (femtovg), not a silent fallback to software.
    assert!(n(&v, "/pill_pixels/pixel_format") > 0, "no GL pixel format: renderer fell back");

    // Renderer output: see-through margin, card drawn.
    assert_eq!(n(&v, "/pill_pixels/snapshot/corner/3"), 0, "pill corner not transparent");
    assert!(n(&v, "/pill_pixels/snapshot/center/3") >= 200, "pill body not drawn");
    assert_eq!(n(&v, "/menu_pixels/snapshot/corner/3"), 0, "menu corner not transparent");
    assert_eq!(n(&v, "/menu_pixels/snapshot/center/3"), 255, "menu card not opaque");

    // Keys through the hook handler move and choose; the hook saw no real key on this desktop.
    assert_eq!(n(&v, "/menu_keys/highlight"), 2);
    assert_eq!(n(&v, "/menu_keys/activated"), 2);
    assert_eq!(n(&v, "/menu_keys/menu_activations"), 0);
    assert!(b(&v, "/hook_installed"));
    assert_eq!(n(&v, "/hook_seen_real_keys"), 0);

    // Zone edit: a drag draws a box and nothing activates.
    assert_eq!(n(&v, "/zone_drag/zones_drawn"), 1);
    let rect: Vec<i64> = v["zone_drag"]["rect_phys"].as_array().unwrap().iter().map(|x| x.as_f64().unwrap() as i64).collect();
    assert_eq!(rect, vec![100, 100, 200, 150]);
    assert_eq!(n(&v, "/zone_drag/zone_activations"), 0);
}

/// Shows why the subclass is needed: without it, winit rewrites GWL_EXSTYLE on the second show and the
/// pill loses click-through, no-activate and tool-window, and gets a taskbar button. If this test starts to
/// fail, winit changed and the subclass may no longer be needed.
#[test]
fn without_subclass_winit_wipes_the_styles() {
    let v = run(&["--renderer", "femtovg", "--naive"], "naive");
    assert!(b(&v, "/pill/has_all"), "first show should still have the styles");
    assert!(!b(&v, "/pill_reshow/has_all"), "styles survived a re-show without the subclass");
    assert!(b(&v, "/pill_reshow/appwindow"));
    assert_eq!(s(&v, "/pill_reshow/center_hits"), "self", "pill still click-through without the subclass");
    assert!(!b(&v, "/menu_reshow/has_all"));
}
