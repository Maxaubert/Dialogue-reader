//! The tray icon (spec 2.9, M5): Slint's `SystemTrayIcon` (ui/tray.slint) on the UI thread. Left click and Open
//! show the main window, Pause/Resume pauses reading (the title follows the main window's `paused`), Quit ends the
//! app. Only main.rs makes it: tests and render-screens never put an icon in the user's tray.
//!
//! When Explorer restarts, the tray is empty. Slint listens for `TaskbarCreated` on a message-only window, which
//! never gets that broadcast, so the app's listener window (src/single.rs) hears it and calls
//! [`TrayIcon::remake`]: the old icon is dropped and a new one is made.

use std::cell::RefCell;
use std::rc::{Rc, Weak};

use slint::ComponentHandle;

use crate::{MainWindow, Tray, shell, vm};

/// The tray icon. Keep it alive while the app runs; clones share the one icon.
#[derive(Clone)]
pub struct TrayIcon {
    slot: Rc<RefCell<Option<Tray>>>,
    w: slint::Weak<MainWindow>,
    vm: Weak<RefCell<vm::Vm>>,
}

/// Makes the tray icon for `w`. An error leaves the app without a tray (closing the window then quits).
pub fn make(w: &MainWindow, vm: &vm::Shared) -> Result<TrayIcon, slint::PlatformError> {
    let t = TrayIcon {
        slot: Rc::default(),
        w: w.as_weak(),
        vm: Rc::downgrade(vm),
    };
    *t.slot.borrow_mut() = Some(new_tray(w, &t.vm)?);
    let slot = Rc::downgrade(&t.slot);
    w.on_paused_changed(move |p| {
        if let Some(s) = slot.upgrade()
            && let Some(tray) = s.borrow().as_ref()
        {
            tray.set_paused(p);
        }
    });
    shell::set_tray_up(true);
    Ok(t)
}

impl TrayIcon {
    /// Drops the icon and makes it again (Explorer restarted). On an error the app has no tray icon, and closing
    /// the window quits.
    pub fn remake(&self) -> Result<(), slint::PlatformError> {
        let Some(w) = self.w.upgrade() else {
            return Ok(());
        };
        // the old icon goes first (Slint removes it from the tray on drop)
        drop(self.slot.borrow_mut().take());
        match new_tray(&w, &self.vm) {
            Ok(tray) => {
                *self.slot.borrow_mut() = Some(tray);
                shell::set_tray_up(true);
                Ok(())
            }
            Err(e) => {
                shell::set_tray_up(false);
                Err(e)
            }
        }
    }

    /// The tray icon is there (tests).
    pub fn is_up(&self) -> bool {
        self.slot.borrow().is_some()
    }
}

fn new_tray(w: &MainWindow, vm: &Weak<RefCell<vm::Vm>>) -> Result<Tray, slint::PlatformError> {
    let tray = Tray::new()?;
    let weak = w.as_weak();
    tray.on_open(move || {
        if let Some(w) = weak.upgrade() {
            shell::show_main_in_front(&w);
        }
    });
    let v = vm.clone();
    tray.on_pause(move || {
        if let Some(vm) = v.upgrade()
            && let Ok(mut vm) = vm.try_borrow_mut()
        {
            vm.toggle_pause();
        }
    });
    let v = vm.clone();
    tray.on_quit(move || quit(&v));
    tray.set_paused(w.get_paused());
    Ok(tray)
}

/// Ends the app as tray Quit does: a zone name still being typed is saved, then the event loop ends. Also for the
/// installer's quit request and the end of the Windows session (src/single.rs).
pub fn quit(vm: &Weak<RefCell<vm::Vm>>) {
    if let Some(vm) = vm.upgrade()
        && let Ok(mut vm) = vm.try_borrow_mut()
    {
        vm.commit_rename();
    }
    shell::quit();
}
