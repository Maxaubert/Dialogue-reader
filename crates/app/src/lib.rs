//! DialogReader app: the Slint UI. M1 shows every approved screen on fake data.
//!
//! `build_main_window` is shared by the app binary, the tests and the `render-screens` tool.

slint::include_modules!();

pub mod fake;
pub mod geom;
pub mod screens;

/// Creates the main window with the fake data of the mockups and wires the callbacks.
pub fn build_main_window() -> Result<MainWindow, slint::PlatformError> {
    let w = MainWindow::new()?;
    fake::fill(&w);
    wire(&w);
    Ok(w)
}

fn wire(w: &MainWindow) {
    let chrome = w.global::<Chrome>();
    let weak = w.as_weak();
    chrome.on_minimize(move || {
        if let Some(w) = weak.upgrade() {
            w.window().set_minimized(true);
        }
    });
    let weak = w.as_weak();
    chrome.on_maximize(move || {
        if let Some(w) = weak.upgrade() {
            let max = w.window().is_maximized();
            w.window().set_maximized(!max);
        }
    });
    chrome.on_close(|| {
        let _ = slint::quit_event_loop();
    });
    let weak = w.as_weak();
    chrome.on_drag(move || {
        if let Some(w) = weak.upgrade() {
            drag_window(&w);
        }
    });
}

fn drag_window(w: &MainWindow) {
    use slint::winit_030::WinitWindowAccessor;
    // Only the winit backend has a native window to move. Tests and screens have none.
    w.window().with_winit_window(|ww| {
        let _ = ww.drag_window();
    });
}

/// Page from its command-line name (`welcome`, `pick-window`, ... `activity`).
pub fn page_from_name(name: &str) -> Option<Page> {
    Some(match name {
        "welcome" => Page::Welcome,
        "pick-window" => Page::PickWindow,
        "draw-zones" => Page::DrawZones,
        "keys" => Page::Keys,
        "voice" => Page::Voice,
        "mode" => Page::Mode,
        "overview" => Page::Overview,
        "zones" => Page::Zones,
        "settings" => Page::Settings,
        "activity" => Page::Activity,
        _ => return None,
    })
}

/// Overlay preview from its command-line name.
pub fn overlay_from_name(name: &str) -> Option<OverlayPreview> {
    Some(match name {
        "none" => OverlayPreview::None,
        "pill" => OverlayPreview::Pill,
        "quick-menu" => OverlayPreview::QuickMenu,
        "quick-menu-voice" => OverlayPreview::QuickMenuVoice,
        "zone-edit" => OverlayPreview::ZoneEdit,
        _ => return None,
    })
}
