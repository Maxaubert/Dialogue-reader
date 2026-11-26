//! Composition check: does DWM really see through the transparent pixels?
//!
//! On the hidden desktop nothing is composed (`PrintWindow` gives black for GL windows, the screen DC is
//! empty, and WGC refuses windows there; see docs/arena-spec.md). So this mode runs on the normal desktop
//! but places each overlay left of the whole virtual screen, where it can never be seen, and asks Windows
//! Graphics Capture for one frame of it. WGC gives the window as DWM holds it, alpha included.
//! No game window is made and nothing asks for the foreground; the foreground is recorded before and after.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{Map, Value, json};
use slint::ComponentHandle;
use windows::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN};
use windows_capture::capture::{Context, GraphicsCaptureApiHandler};
use windows_capture::frame::Frame;
use windows_capture::graphics_capture_api::InternalCaptureControl;
use windows_capture::settings::{
    ColorFormat, CursorCaptureSettings, DirtyRegionSettings, DrawBorderSettings, MinimumUpdateIntervalSettings,
    SecondaryWindowSettings, Settings,
};

use crate::overlay::{self, Kind, Opts};
use crate::win;
use crate::{MenuWindow, PillWindow};

type Slot = Arc<Mutex<Option<(u32, u32, Vec<u8>)>>>;

struct Grab {
    slot: Slot,
    buf: Vec<u8>,
}

impl GraphicsCaptureApiHandler for Grab {
    type Flags = Slot;
    type Error = Box<dyn std::error::Error + Send + Sync>;

    fn new(ctx: Context<Self::Flags>) -> Result<Self, Self::Error> {
        Ok(Grab { slot: ctx.flags, buf: Vec::new() })
    }

    fn on_frame_arrived(&mut self, frame: &mut Frame, control: InternalCaptureControl) -> Result<(), Self::Error> {
        let (w, h) = (frame.width(), frame.height());
        let fb = frame.buffer()?;
        let rgba = fb.as_nopadding_buffer(&mut self.buf).to_vec();
        if let Ok(mut s) = self.slot.lock() {
            *s = Some((w, h, rgba));
        }
        control.stop();
        Ok(())
    }
}

fn grab(w: &slint::Window) -> Value {
    let Some(hwnd) = overlay::hwnd_of(w) else {
        return json!({ "error": "no hwnd" });
    };
    let slot: Slot = Arc::new(Mutex::new(None));
    let item = windows_capture::window::Window::from_raw_hwnd(hwnd.0);
    let settings = Settings::new(
        item,
        CursorCaptureSettings::WithoutCursor,
        DrawBorderSettings::WithoutBorder,
        SecondaryWindowSettings::Default,
        MinimumUpdateIntervalSettings::Default,
        DirtyRegionSettings::Default,
        ColorFormat::Rgba8,
        slot.clone(),
    );
    let control = match Grab::start_free_threaded(settings) {
        Ok(c) => c,
        Err(e) => return json!({ "error": format!("wgc: {e}") }),
    };
    for _ in 0..40 {
        if slot.lock().map(|s| s.is_some()).unwrap_or(false) {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let _ = control.stop();
    let got = slot.lock().ok().and_then(|mut s| s.take());
    let Some((fw, fh, px)) = got else {
        return json!({ "error": "no frame in 2 s" });
    };
    let at = |x: u32, y: u32| {
        let i = ((y * fw + x) * 4) as usize;
        [px[i], px[i + 1], px[i + 2], px[i + 3]]
    };
    let transparent = px.as_chunks::<4>().0.iter().filter(|p| p[3] == 0).count() as f64 / (fw * fh).max(1) as f64;
    json!({
        "w": fw, "h": fh,
        "corner_rgba": at(4, 4),
        "center_rgba": at(fw / 2, fh / 2),
        "share_alpha_0": transparent,
    })
}

/// Shows the pill and the quick menu off screen, captures each with WGC and prints a JSON report.
pub fn run(renderer: &str, opts: Opts, menu_like_pill: bool) -> i32 {
    std::thread::spawn(|| {
        std::thread::sleep(Duration::from_secs(30));
        std::process::exit(3);
    });
    let mut sel = slint::BackendSelector::new()
        .backend_name("winit".into())
        .with_winit_window_attributes_hook(overlay::attributes_hook);
    if renderer != "default" {
        sel = sel.renderer_name(renderer.into());
    }
    if let Err(e) = sel.select() {
        println!("{}", json!({ "error": format!("backend: {e}") }));
        return 2;
    }
    let (Ok(pill), Ok(menu)) = (overlay::new_overlay(PillWindow::new), overlay::new_overlay(MenuWindow::new)) else {
        println!("{}", json!({ "error": "create failed" }));
        return 2;
    };
    let fg_before = win::foreground();
    let (vx, vy) = unsafe { (GetSystemMetrics(SM_XVIRTUALSCREEN), GetSystemMetrics(SM_YVIRTUALSCREEN)) };
    // Far left of every monitor. The windows are smaller than 3000 px.
    pill.window().set_position(slint::PhysicalPosition::new(vx - 3200, vy + 50));
    menu.window().set_position(slint::PhysicalPosition::new(vx - 3200, vy + 400));
    let mut report = Map::new();
    report.insert("renderer_asked".into(), json!(renderer));
    let report = std::rc::Rc::new(std::cell::RefCell::new(report));
    let (pill_w, menu_w) = (pill.as_weak(), menu.as_weak());
    let r1 = report.clone();
    // The window (and its HWND) only exists once the event loop has started (winit "resumed"). A zero-delay
    // timer is too early (no HWND after show), so wait a moment.
    slint::Timer::single_shot(Duration::from_millis(300), move || {
        let (Some(p), Some(m)) = (pill_w.upgrade(), menu_w.upgrade()) else { return };
        let mut rep = r1.borrow_mut();
        for (name, w, kind) in [("pill", p.window(), Kind::Pill), ("menu", m.window(), Kind::Menu)] {
            if let Err(e) = w.show() {
                rep.insert(format!("{name}_show_error"), json!(e.to_string()));
                continue;
            }
            let Some(h) = overlay::hwnd_of(w) else {
                rep.insert(format!("{name}_window"), json!({ "error": "no hwnd after show" }));
                continue;
            };
            // Off screen, WGC only sees GL windows that have WS_EX_TRANSPARENT. `--menu-like-pill` gives the
            // menu the pill's styles so its pixels can be read too (measurement only).
            let kind = if menu_like_pill { Kind::Pill } else { kind };
            if let Err(e) = overlay::harden(h, kind, opts) {
                rep.insert(format!("{name}_harden_error"), json!(e.to_string()));
            }
            let r = win::window_rect(h);
            rep.insert(
                format!("{name}_window"),
                json!({
                    "rect": [r.left, r.top, r.right, r.bottom],
                    "pixel_format": win::pixel_format(h),
                    "ex_style": format!("0x{:08X}", win::ex_style(h)),
                }),
            );
        }
        let (pill_w, menu_w) = (p.as_weak(), m.as_weak());
        let r2 = r1.clone();
        slint::Timer::single_shot(Duration::from_millis(2000), move || {
            let mut rep = r2.borrow_mut();
            if let Some(m) = menu_w.upgrade() {
                rep.insert("menu_wgc".into(), grab(m.window()));
                let _ = m.hide();
            }
            if let Some(p) = pill_w.upgrade() {
                rep.insert("pill_wgc".into(), grab(p.window()));
                let _ = p.hide();
            }
            let fg_after = win::foreground();
            rep.insert("foreground_unchanged".into(), json!(fg_before == fg_after));
            if fg_before != fg_after {
                // Max may simply have switched windows; record whether one of ours is in front.
                let ours = [pill_w.upgrade().and_then(|p| overlay::hwnd_of(p.window())),
                            menu_w.upgrade().and_then(|m| overlay::hwnd_of(m.window()))];
                rep.insert("foreground_is_ours".into(), json!(ours.contains(&Some(fg_after))));
                rep.insert("foreground_pid_is_ours".into(), json!(win::pid_of(fg_after) == std::process::id()));
            }
            let _ = slint::quit_event_loop();
        });
    });
    let _ = slint::run_event_loop_until_quit();
    println!("{}", serde_json::to_string_pretty(&Value::Object(report.borrow().clone())).unwrap_or_default());
    0
}
