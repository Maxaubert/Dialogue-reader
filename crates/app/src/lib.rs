//! DialogReader app: the Slint UI.
//!
//! `build_window(source)` is shared by the app binary, the tests and the `render-screens` tool. The app passes
//! real profiles and the real desktop; render-screens and most tests pass the fake data of the mockups.

slint::include_modules!();

pub mod crash;
pub mod fake;
pub mod geom;
pub mod screens;
pub mod vm;

/// The main window with the fake data of the mockups (render-screens and the screen and nav tests).
pub fn build_main_window() -> Result<MainWindow, slint::PlatformError> {
    build_window(vm::Source::Fake).map(|(w, _)| w)
}

/// The main window on `source`, with every callback wired. Keep the returned view model alive as long as the
/// window (the callbacks hold it too).
pub fn build_window(source: vm::Source) -> Result<(MainWindow, vm::Shared), slint::PlatformError> {
    let w = MainWindow::new()?;
    let vm = vm::install(&w, source);
    wire(&w, &vm);
    Ok((w, vm))
}

fn wire(w: &MainWindow, vm: &vm::Shared) {
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
    let v = vm.clone();
    w.window().on_close_requested(move || {
        v.borrow_mut().commit_rename();
        slint::CloseRequestResponse::HideWindow
    });
    let v = vm.clone();
    chrome.on_close(move || {
        // a zone name still being typed is saved first
        v.borrow_mut().commit_rename();
        let _ = slint::quit_event_loop();
    });
    // the left button is down while a mouse button is held on the app (see keep_left_drags)
    let left = std::rc::Rc::new(std::cell::Cell::new(false));
    let weak = w.as_weak();
    let l = left.clone();
    chrome.on_drag(move || {
        if let Some(w) = weak.upgrade() {
            drag_window(&w);
        }
        // the native move loop of a title bar drag eats the left release
        l.set(false);
    });
    keep_left_drags(w, vm, left);
}

/// The app window lost the focus (Alt+Tab, a UAC prompt): the left release of a zone drag may never come. The
/// stage lets go of the mouse (a pointer exit) and the drag is cancelled, as a right click does: the zone goes
/// back, a zone being drawn is dropped. Overview pauses its live view (the game is in front, src/vm/live.rs).
/// Key capture ends as Cancel does (Max, 2026-11-23): a key pressed in the game is not taken.
/// Public for the tests (the testing backend has no winit events).
pub fn focus_lost(w: &MainWindow, vm: &vm::Shared) {
    use slint::platform::WindowEvent;
    w.window().dispatch_event(WindowEvent::PointerExited);
    if let Ok(mut vm) = vm.try_borrow_mut() {
        vm.cancel_zone_drag();
        vm.cancel_capture();
        vm.live_focus(false);
    }
}

/// The app window got the focus back: Overview starts its live view again. Public for the tests.
pub fn focus_gained(vm: &vm::Shared) {
    if let Ok(mut vm) = vm.try_borrow_mut() {
        vm.live_focus(true);
    }
}

/// While the left button is down, the middle, back and forward buttons never reach Slint. Their release would
/// end the mouse grab of a zone drag on the stage, which then could not follow the mouse outside the stage. The
/// right button still goes through: on the stage it cancels the drag (ui/components/stage.slint).
/// Focus loss cancels a drag (`focus_lost`). Focus changes also go to the live view (`focus_gained`).
fn keep_left_drags(w: &MainWindow, vm: &vm::Shared, left: std::rc::Rc<std::cell::Cell<bool>>) {
    use slint::winit_030::winit::event::{ElementState, MouseButton, WindowEvent};
    use slint::winit_030::{EventResult, WinitWindowAccessor};
    // Only the winit backend has winit events. Tests and screens have none.
    let weak = w.as_weak();
    let v = std::rc::Rc::downgrade(vm);
    w.window().on_winit_window_event(move |_, e| match e {
        WindowEvent::MouseInput {
            state,
            button: MouseButton::Left,
            ..
        } => {
            left.set(*state == ElementState::Pressed);
            EventResult::Propagate
        }
        WindowEvent::MouseInput {
            button: MouseButton::Right,
            ..
        } => EventResult::Propagate,
        WindowEvent::MouseInput { .. } if left.get() => EventResult::PreventDefault,
        WindowEvent::Focused(false) => {
            left.set(false);
            // after this event is handled, not inside the winit hook
            let (weak, v) = (weak.clone(), v.clone());
            slint::Timer::single_shot(std::time::Duration::ZERO, move || {
                if let (Some(w), Some(vm)) = (weak.upgrade(), v.upgrade()) {
                    focus_lost(&w, &vm);
                }
            });
            EventResult::Propagate
        }
        WindowEvent::Focused(true) => {
            let v = v.clone();
            slint::Timer::single_shot(std::time::Duration::ZERO, move || {
                if let Some(vm) = v.upgrade() {
                    focus_gained(&vm);
                }
            });
            EventResult::Propagate
        }
        _ => EventResult::Propagate,
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
