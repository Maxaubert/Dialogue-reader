//! The app shell around the main window (spec 2.9, M5): show it from the tray or a second start, hide it to the
//! tray on close, the taskbar button, and quitting.
//!
//! Closing the main window hides it while the tray icon is up (Max's M4 decision: "closing quits until M5"). With
//! no tray icon (it could not be made, or `--no-tray`), closing quits, so the app is never left running with no
//! way back to it.

use std::cell::Cell;

use slint::ComponentHandle;

use crate::MainWindow;

thread_local! {
    static TRAY_UP: Cell<bool> = const { Cell::new(false) };
}

/// The tray icon is up: closing the main window hides it instead of quitting.
pub fn set_tray_up(up: bool) {
    TRAY_UP.with(|t| t.set(up));
}

pub fn tray_up() -> bool {
    TRAY_UP.with(Cell::get)
}

/// Shows the main window (restored if it was minimized) with the taskbar setting applied.
pub fn show_main(w: &MainWindow) {
    let _ = w.show();
    w.window().set_minimized(false);
    set_taskbar(w, w.get_show_in_taskbar());
    #[cfg(windows)]
    crate::overlay::win::round_corners(w.window());
}

/// [`show_main`], then in front of the other windows (tray Open, a second start).
pub fn show_main_in_front(w: &MainWindow) {
    show_main(w);
    #[cfg(windows)]
    crate::overlay::win::bring_to_front(w.window());
}

/// The close button or Alt+F4: hide to the tray, or quit when there is no tray icon.
pub fn close_main(w: &MainWindow) {
    if tray_up() {
        let _ = w.hide();
    } else {
        quit();
    }
}

/// Ends the app (tray Quit, quick menu Quit): the event loop ends and main.rs cleans up.
pub fn quit() {
    let _ = slint::quit_event_loop();
}

/// The main window's taskbar button on or off (Settings > Startup > Show in the taskbar). Only the winit backend
/// has a native window; tests and screens have none, and a window not shown yet gets it in [`show_main`].
pub fn set_taskbar(w: &MainWindow, show: bool) {
    use slint::winit_030::WinitWindowAccessor;
    w.window().with_winit_window(|ww| {
        #[cfg(windows)]
        {
            use slint::winit_030::winit::platform::windows::WindowExtWindows;
            ww.set_skip_taskbar(!show);
        }
        #[cfg(not(windows))]
        let _ = (ww, show);
    });
}
