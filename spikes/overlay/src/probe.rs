//! The probe: runs on the hidden desktop next to the dummy game, opens each overlay and writes what it
//! measured to a JSON file. It asserts nothing itself; `tests/hidden_desktop.rs` reads the JSON.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use serde_json::{Map, Value, json};
use slint::ComponentHandle;
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    HTCLIENT, IsWindowVisible, PostMessageW, SendMessageW, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEACTIVATE,
    WM_MOUSEMOVE,
};

use crate::keys::{self, Key, MenuKey};
use crate::overlay::{self, Kind, Opts};
use crate::win;
use crate::{MenuWindow, PillWindow, PlainWindow, ZoneWindow};

pub struct Args {
    pub out: PathBuf,
    pub renderer: String,
    pub opts: Opts,
}

struct Ctx {
    game: HWND,
    opts: Opts,
    pill: PillWindow,
    menu: MenuWindow,
    zone: ZoneWindow,
    plain: PlainWindow,
    report: Map<String, Value>,
    hook: Option<keys::Hook>,
}

fn hex(v: u32) -> String {
    format!("0x{v:08X}")
}

fn h(hwnd: HWND) -> String {
    format!("0x{:X}", hwnd.0 as usize)
}

fn px(p: [u8; 4]) -> Value {
    json!(p)
}

impl Ctx {
    fn set(&mut self, key: &str, v: Value) {
        self.report.insert(key.to_string(), v);
    }

    /// Styles, focus and hit test of one overlay window.
    fn check(&self, kind: Kind, w: &slint::Window) -> Value {
        let Some(hwnd) = overlay::hwnd_of(w) else {
            return json!({ "hwnd": null });
        };
        let ex = win::ex_style(hwnd);
        let want = kind.ex_style();
        let r = win::window_rect(hwnd);
        let (cx, cy) = ((r.left + r.right) / 2, (r.top + r.bottom) / 2);
        let hit = win::window_at(cx, cy);
        let fg = win::foreground();
        let act = win::activity(hwnd);
        let (ui_active, ui_focus) = win::thread_active(win::current_thread());
        json!({
            "hwnd": h(hwnd),
            "activations": act.activate,
            "setfocus": act.setfocus,
            "ui_thread_active_is_self": ui_active == hwnd,
            "ui_thread_focus_is_self": ui_focus == hwnd,
            "is_hardened": overlay::is_hardened(hwnd),
            "ex_style": hex(ex),
            "want": hex(want),
            "has_all": ex & want == want,
            "missing": hex(want & !ex),
            "appwindow": ex & 0x0004_0000 != 0,
            "visible": unsafe { IsWindowVisible(hwnd).as_bool() },
            "rect": [r.left, r.top, r.right, r.bottom],
            "foreground": h(fg),
            "foreground_is_game": fg == self.game,
            "center_hits": if hit == self.game { "game" } else if hit == hwnd { "self" } else { "other" },
        })
    }

    /// Everything about the pixels: renderer buffer, PrintWindow, screen copy.
    fn pixels(&self, w: &slint::Window, margin_px: i32) -> Value {
        let Some(hwnd) = overlay::hwnd_of(w) else {
            return Value::Null;
        };
        let mut out = Map::new();
        out.insert("pixel_format".into(), json!(win::pixel_format(hwnd)));
        let (lok, la, lf) = win::layered_attrs(hwnd);
        out.insert("layered_attrs".into(), json!({ "ok": lok, "alpha": la, "flags": lf }));
        match w.take_snapshot() {
            Ok(buf) => {
                let (bw, bh) = (buf.width() as usize, buf.height() as usize);
                let get = |x: usize, y: usize| {
                    let p = buf.as_slice()[y * bw + x];
                    [p.r, p.g, p.b, p.a]
                };
                let m = margin_px.max(1) as usize / 2;
                out.insert(
                    "snapshot".into(),
                    json!({ "w": bw, "h": bh, "corner": get(m, m), "center": get(bw / 2, bh / 2) }),
                );
            }
            Err(e) => {
                out.insert("snapshot".into(), json!({ "error": e.to_string() }));
            }
        }
        for (key, screen) in [("printwindow", false), ("screen_blt", true)] {
            let v = match win::capture(hwnd, screen) {
                Some(c) => {
                    let m = margin_px.max(1) / 2;
                    json!({
                        "w": c.w, "h": c.h,
                        "corner_bgra": px(c.at(m, m)),
                        "center_bgra": px(c.at(c.w / 2, c.h / 2)),
                        "untouched": c.untouched(),
                    })
                }
                None => json!({ "error": "capture failed" }),
            };
            out.insert(key.into(), v);
        }
        Value::Object(out)
    }
}

fn show_hardened(w: &slint::Window, kind: Kind, opts: Opts) -> Value {
    if let Err(e) = w.show() {
        return json!({ "show_error": e.to_string() });
    }
    match overlay::hwnd_of(w) {
        Some(hwnd) => {
            let before = win::ex_style(hwnd);
            let r = overlay::harden(hwnd, kind, opts);
            json!({
                "ex_before_harden": hex(before),
                "ex_after_harden": hex(win::ex_style(hwnd)),
                "harden_ok": r.is_ok(),
                "harden_error": r.err().map(|e| e.to_string()),
            })
        }
        None => json!({ "hwnd": null }),
    }
}

/// Logical px -> physical px of this window.
fn phys(w: &slint::Window, logical: f32) -> i32 {
    (logical * w.scale_factor()).round() as i32
}

fn lparam_xy(x: i32, y: i32) -> LPARAM {
    LPARAM(((y as u16 as u32) << 16 | x as u16 as u32) as isize)
}

type Step = Box<dyn Fn(&mut Ctx)>;

fn steps() -> Vec<Step> {
    vec![
        // 0: pill shows
        Box::new(|c| {
            let fg = win::foreground();
            c.set("start", json!({ "game": h(c.game), "foreground": h(fg), "game_is_foreground": fg == c.game }));
            let g = win::client_rect_screen(c.game);
            c.pill.window().set_position(slint::PhysicalPosition::new(g.right - 400, g.top + 20));
            let v = show_hardened(c.pill.window(), Kind::Pill, c.opts);
            c.set("pill_show", v);
        }),
        // 1: pill checks
        Box::new(|c| {
            let v = c.check(Kind::Pill, c.pill.window());
            c.set("pill", v);
            let m = phys(c.pill.window(), 24.0);
            let v = c.pixels(c.pill.window(), m);
            c.set("pill_pixels", v);
        }),
        // 2: hide
        Box::new(|c| {
            let _ = c.pill.hide();
        }),
        // 3: show again (winit's second show uses SW_SHOW and rewrites the ex-style)
        Box::new(|c| {
            let _ = c.pill.show();
        }),
        // 4: check, then change a property (Slint re-applies window properties)
        Box::new(|c| {
            let v = c.check(Kind::Pill, c.pill.window());
            c.set("pill_reshow", v);
            c.pill.set_speaker("Sadie".into());
        }),
        // 5: check, hide the pill, open the quick menu centered on the game, install the key hook
        Box::new(|c| {
            let v = c.check(Kind::Pill, c.pill.window());
            c.set("pill_prop_change", v);
            let _ = c.pill.hide();
            let v = show_hardened(c.menu.window(), Kind::Menu, c.opts);
            c.set("menu_show", v);
            let g = win::client_rect_screen(c.game);
            let s = c.menu.window().size();
            let x = (g.left + g.right - s.width as i32) / 2;
            let y = (g.top + g.bottom - s.height as i32) / 2;
            c.menu.window().set_position(slint::PhysicalPosition::new(x, y));
            let weak = c.menu.as_weak();
            // Runs on the hook thread (or on this thread through `keys::feed`): posts the key to the UI
            // thread. Never swallows here: on the hidden desktop the hook should see no real keys at all.
            let hook = keys::install(move |ev| {
                if let Key::Menu(k) = ev.key {
                    let _ = weak.upgrade_in_event_loop(move |m| crate::menu_key(&m, k));
                }
                false
            });
            c.set("hook_installed", json!(hook.is_ok()));
            c.hook = hook.ok();
        }),
        // 6: menu checks, mouse activation answer, keys through the hook handler
        Box::new(|c| {
            let v = c.check(Kind::Menu, c.menu.window());
            c.set("menu", v);
            let m = phys(c.menu.window(), 24.0);
            let v = c.pixels(c.menu.window(), m);
            c.set("menu_pixels", v);
            if let Some(hwnd) = overlay::hwnd_of(c.menu.window()) {
                let lp = LPARAM(((WM_LBUTTONDOWN << 16) | HTCLIENT) as isize);
                let r = unsafe { SendMessageW(hwnd, WM_MOUSEACTIVATE, Some(WPARAM(hwnd.0 as usize)), Some(lp)) };
                c.set("menu_mouseactivate", json!(r.0));
            }
            keys::feed(Key::Menu(MenuKey::Down));
            keys::feed(Key::Menu(MenuKey::Down));
            keys::feed(Key::Menu(MenuKey::Enter));
        }),
        // 7: key results, hide
        Box::new(|c| {
            let fg = win::foreground();
            c.set(
                "menu_keys",
                json!({
                    "highlight": c.menu.get_highlight(),
                    "activated": c.menu.get_activated(),
                    "foreground_is_game": fg == c.game,
                    "menu_activations": overlay::hwnd_of(c.menu.window()).map(|w| win::activity(w).activate),
                }),
            );
            let _ = c.menu.hide();
        }),
        // 8: show again
        Box::new(|c| {
            let _ = c.menu.show();
        }),
        // 9: check, open zone edit over the game's client area
        Box::new(|c| {
            let v = c.check(Kind::Menu, c.menu.window());
            c.set("menu_reshow", v);
            let _ = c.menu.hide();
            let g = win::client_rect_screen(c.game);
            let zw = c.zone.window();
            zw.set_position(slint::PhysicalPosition::new(g.left, g.top));
            zw.set_size(slint::PhysicalSize::new((g.right - g.left) as u32, (g.bottom - g.top) as u32));
            let v = show_hardened(zw, Kind::Zone, c.opts);
            c.set("zone_show", v);
        }),
        // 10: zone checks, then a drag with posted mouse messages (no SendInput: it could reach the real desktop)
        Box::new(|c| {
            let v = c.check(Kind::Zone, c.zone.window());
            c.set("zone", v);
            if let Some(hwnd) = overlay::hwnd_of(c.zone.window()) {
                let lp = LPARAM(((WM_LBUTTONDOWN << 16) | HTCLIENT) as isize);
                let r = unsafe { SendMessageW(hwnd, WM_MOUSEACTIVATE, Some(WPARAM(hwnd.0 as usize)), Some(lp)) };
                c.set("zone_mouseactivate", json!(r.0));
                unsafe {
                    let _ = PostMessageW(Some(hwnd), WM_MOUSEMOVE, WPARAM(0), lparam_xy(100, 100));
                    let _ = PostMessageW(Some(hwnd), WM_LBUTTONDOWN, WPARAM(1), lparam_xy(100, 100));
                    let _ = PostMessageW(Some(hwnd), WM_MOUSEMOVE, WPARAM(1), lparam_xy(200, 160));
                    let _ = PostMessageW(Some(hwnd), WM_MOUSEMOVE, WPARAM(1), lparam_xy(300, 250));
                    let _ = PostMessageW(Some(hwnd), WM_LBUTTONUP, WPARAM(0), lparam_xy(300, 250));
                }
            }
        }),
        // 11: drag result
        Box::new(|c| {
            let fg = win::foreground();
            let sf = c.zone.window().scale_factor();
            c.set(
                "zone_drag",
                json!({
                    "zones_drawn": c.zone.get_zones_drawn(),
                    "rect_phys": [
                        (c.zone.get_last_x() * sf).round(), (c.zone.get_last_y() * sf).round(),
                        (c.zone.get_last_w() * sf).round(), (c.zone.get_last_h() * sf).round(),
                    ],
                    "foreground_is_game": fg == c.game,
                    "zone_activations": overlay::hwnd_of(c.zone.window()).map(|w| win::activity(w).activate),
                }),
            );
            let _ = c.zone.hide();
        }),
        // 12: show zone edit again (a bare show, like the other re-shows: the subclass must keep the styles)
        Box::new(|c| {
            let _ = c.zone.show();
        }),
        // 13: check, hide
        Box::new(|c| {
            let v = c.check(Kind::Zone, c.zone.window());
            c.set("zone_reshow", v);
            let _ = c.zone.hide();
        }),
        // 14: positive control, a plain window with default settings
        Box::new(|c| {
            let _ = c.plain.show();
        }),
        // 15: did the plain window activate?
        Box::new(|c| {
            let fg = win::foreground();
            let plain = overlay::hwnd_of(c.plain.window());
            let (ui_active, _) = win::thread_active(win::current_thread());
            // Recorded to show it is no evidence: it stays true even while the plain window activates, because
            // a window of another process never changes the game thread's active window on this desktop.
            let (game_active, _) = win::thread_active(win::thread_of(c.game));
            c.set(
                "control",
                json!({
                    "plain": plain.map(h),
                    "foreground": h(fg),
                    "took_foreground": plain == Some(fg),
                    "activations": plain.map(|w| win::activity(w).activate),
                    "setfocus": plain.map(|w| win::activity(w).setfocus),
                    "ui_thread_active_is_plain": plain == Some(ui_active),
                    "game_thread_active_is_game": game_active == c.game,
                    "plain_is_hardened": plain.map(overlay::is_hardened),
                }),
            );
            // show_overlay on an already hardened window must not harden it again.
            let again = overlay::show_overlay(c.pill.window(), Kind::Pill, c.opts);
            c.set(
                "show_overlay_again",
                json!({
                    "ok": again.is_ok(),
                    "hardened_now": again.as_ref().map(|s| s.hardened_now).ok(),
                    "same_hwnd": again.as_ref().ok().map(|s| Some(s.hwnd) == overlay::hwnd_of(c.pill.window())),
                    "ex_style": again.as_ref().ok().map(|s| hex(win::ex_style(s.hwnd))),
                }),
            );
            let _ = c.pill.hide();
            let _ = c.plain.hide();
            c.set("hook_seen_real_keys", json!(keys::seen()));
            c.set("hook_seen_injected_keys", json!(keys::seen_injected()));
            c.hook = None;
        }),
    ]
}

/// Runs the probe. Returns the process exit code.
pub fn run(args: Args) -> i32 {
    // Watchdog: never outlive the test.
    std::thread::spawn(|| {
        std::thread::sleep(Duration::from_secs(60));
        std::process::exit(3);
    });
    let mut sel = slint::BackendSelector::new()
        .backend_name("winit".into())
        .with_winit_window_attributes_hook(overlay::attributes_hook);
    if args.renderer != "default" {
        sel = sel.renderer_name(args.renderer.clone());
    }
    if let Err(e) = sel.select() {
        write(&args, json!({ "error": format!("backend: {e}") }));
        return 2;
    }
    let spy = win::spy_start();
    let mut game = None;
    for _ in 0..50 {
        game = win::find_game();
        if game.is_some() {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    let Some(game) = game else {
        write(&args, json!({ "error": "dummy game window not found" }));
        return 2;
    };
    let made = (|| -> Result<_, slint::PlatformError> {
        let pill = overlay::new_overlay(PillWindow::new)?;
        let menu = overlay::new_overlay(MenuWindow::new)?;
        let zone = overlay::new_overlay(ZoneWindow::new)?;
        let plain = PlainWindow::new()?;
        Ok((pill, menu, zone, plain))
    })();
    let (pill, menu, zone, plain) = match made {
        Ok(v) => v,
        Err(e) => {
            write(&args, json!({ "error": format!("create: {e}") }));
            return 2;
        }
    };
    let mut report = Map::new();
    report.insert("renderer_asked".into(), json!(args.renderer));
    report.insert("spy_installed".into(), json!(spy));
    report.insert("opts".into(), json!({
            "subclass": args.opts.subclass,
            "layered_alpha": args.opts.layered_alpha,
            "layered_all": args.opts.layered_all,
        }));
    let ctx = Rc::new(RefCell::new(Ctx {
        game,
        opts: args.opts,
        pill,
        menu,
        zone,
        plain,
        report,
        hook: None,
    }));
    let steps = Rc::new(steps());
    let next = Rc::new(std::cell::Cell::new(0usize));
    let timer = slint::Timer::default();
    let out = args.out.clone();
    {
        let ctx = ctx.clone();
        timer.start(slint::TimerMode::Repeated, Duration::from_millis(400), move || {
            let i = next.get();
            next.set(i + 1);
            if let Some(step) = steps.get(i) {
                step(&mut ctx.borrow_mut());
            } else {
                let report = Value::Object(ctx.borrow().report.clone());
                let _ = std::fs::write(&out, serde_json::to_string_pretty(&report).unwrap_or_default());
                let _ = slint::quit_event_loop();
            }
        });
    }
    if let Err(e) = slint::run_event_loop_until_quit() {
        write(&args, json!({ "error": format!("event loop: {e}") }));
        return 2;
    }
    0
}

fn write(args: &Args, v: Value) {
    let _ = std::fs::write(&args.out, v.to_string());
}
