//! Visible demo for the manual check (only with Max's yes; it shows windows on the real screen).
//!
//! Start it, then click into a borderless game within 5 seconds. The window in front then counts as the
//! game. The pill shows at its top right. F8 opens or closes the quick menu (arrows move, Enter chooses,
//! Esc closes; these keys are swallowed only while the menu is open). F9 opens or closes zone edit (drag to
//! draw a box, Esc closes). Only one of the two is open at a time. F10 quits. Every second the console says
//! if the game is still in front.
//!
//! The key hook runs on its own thread (see keys.rs). Its handler sets the wanted state in atomics at once,
//! so the next key is already judged by it, and posts one `sync` to the UI thread, which makes the windows
//! match that state.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use slint::ComponentHandle;
use windows::Win32::Foundation::RECT;

use crate::keys::{self, Key, KeyEvent, MenuKey};
use crate::overlay::{self, Kind, Opts};
use crate::win;
use crate::{MenuWindow, PillWindow, ZoneWindow};

const VK_F8: u32 = 0x77;
const VK_F9: u32 = 0x78;
const VK_F10: u32 = 0x79;

/// What the user asked for. Written by the hook thread, read by both threads.
#[derive(Default)]
struct Wanted {
    menu: AtomicBool,
    zone: AtomicBool,
}

/// Shows an overlay through `show_overlay` (hardens it whenever its HWND is new) and prints the result.
fn show(w: &slint::Window, kind: Kind) {
    match overlay::show_overlay(w, kind, Opts::default()) {
        Ok(s) if s.hardened_now => {
            let ex = win::ex_style(s.hwnd);
            let ok = ex & kind.ex_style() == kind.ex_style();
            println!(
                "{}: new window 0x{:X}, ex-style 0x{ex:08X} ({})",
                kind.name(),
                s.hwnd.0 as usize,
                if ok { "all overlay styles set" } else { "STYLES MISSING" }
            );
        }
        Ok(_) => {}
        Err(e) => println!("{}: {e}", kind.name()),
    }
}

struct Demo {
    game_rect: RECT,
    pill: PillWindow,
    menu: MenuWindow,
    zone: ZoneWindow,
    wanted: Arc<Wanted>,
    menu_shown: Cell<bool>,
    zone_shown: Cell<bool>,
    pill_shown: Cell<bool>,
}

thread_local! {
    // The demo lives on the UI thread; work posted from the hook thread finds it here.
    static DEMO: RefCell<Option<Rc<Demo>>> = const { RefCell::new(None) };
}

fn with_demo(f: impl FnOnce(&Demo)) {
    let d = DEMO.with(|d| d.borrow().clone());
    if let Some(d) = d {
        f(&d);
    }
}

/// Posts work to the UI thread (from the hook thread).
fn post(f: impl FnOnce(&Demo) + Send + 'static) {
    let _ = slint::invoke_from_event_loop(move || with_demo(f));
}

impl Demo {
    /// Makes the windows match `wanted`: menu or zone edit or neither, and the pill only when neither.
    fn sync(&self) {
        let menu = self.wanted.menu.load(Ordering::SeqCst);
        let zone = self.wanted.zone.load(Ordering::SeqCst) && !menu;
        if self.menu_shown.get() && !menu {
            let _ = self.menu.hide();
            self.menu_shown.set(false);
        }
        if self.zone_shown.get() && !zone {
            let _ = self.zone.hide();
            self.zone_shown.set(false);
        }
        let pill = !menu && !zone;
        if self.pill_shown.get() && !pill {
            let _ = self.pill.hide();
            self.pill_shown.set(false);
        }
        let g = self.game_rect;
        if menu && !self.menu_shown.get() {
            self.menu.set_highlight(0);
            self.menu.set_activated(-1);
            show(self.menu.window(), Kind::Menu);
            // Its size is known after show.
            let s = self.menu.window().size();
            let x = (g.left + g.right - s.width as i32) / 2;
            let y = (g.top + g.bottom - s.height as i32) / 2;
            self.menu.window().set_position(slint::PhysicalPosition::new(x, y));
            self.menu_shown.set(true);
        }
        if zone && !self.zone_shown.get() {
            let w = self.zone.window();
            w.set_position(slint::PhysicalPosition::new(g.left, g.top));
            w.set_size(slint::PhysicalSize::new((g.right - g.left) as u32, (g.bottom - g.top) as u32));
            show(w, Kind::Zone);
            self.zone_shown.set(true);
        }
        if pill && !self.pill_shown.get() {
            // A first guess, so the window does not show at a random place for a frame.
            self.pill.window().set_position(slint::PhysicalPosition::new(g.right - 480, g.top + 20));
            show(self.pill.window(), Kind::Pill);
            // Top right of the game, 20 px in. Its size (which depends on the scale) is known after show.
            let s = self.pill.window().size();
            self.pill.window().set_position(slint::PhysicalPosition::new(g.right - s.width as i32 - 20, g.top + 20));
            self.pill_shown.set(true);
        }
    }
}

/// The hook handler. Runs on the hook thread: reads and sets atomics, posts work, answers at once.
fn on_key(wanted: &Wanted, ev: KeyEvent) -> bool {
    let menu = wanted.menu.load(Ordering::SeqCst);
    let zone = wanted.zone.load(Ordering::SeqCst);
    match ev.key {
        Key::Other(VK_F10) => {
            if !ev.repeat {
                let _ = slint::invoke_from_event_loop(|| {
                    let _ = slint::quit_event_loop();
                });
            }
            true
        }
        Key::Other(VK_F8) => {
            if !ev.repeat {
                wanted.menu.store(!menu, Ordering::SeqCst);
                wanted.zone.store(false, Ordering::SeqCst);
                post(Demo::sync);
            }
            true
        }
        Key::Other(VK_F9) => {
            if !ev.repeat {
                wanted.zone.store(!zone, Ordering::SeqCst);
                wanted.menu.store(false, Ordering::SeqCst);
                post(Demo::sync);
            }
            true
        }
        Key::Menu(MenuKey::Esc) if menu || zone => {
            wanted.menu.store(false, Ordering::SeqCst);
            wanted.zone.store(false, Ordering::SeqCst);
            post(Demo::sync);
            true
        }
        Key::Menu(k) if menu => {
            post(move |d| crate::menu_key(&d.menu, k));
            true
        }
        // Zone edit uses Delete and Backspace (remove a zone); arrows and the rest go to the game.
        Key::Menu(MenuKey::Delete) if zone => true,
        _ => false,
    }
}

pub fn run(renderer: &str) -> i32 {
    std::thread::spawn(|| {
        std::thread::sleep(Duration::from_secs(600));
        std::process::exit(3);
    });
    let mut sel = slint::BackendSelector::new()
        .backend_name("winit".into())
        .with_winit_window_attributes_hook(overlay::attributes_hook);
    if renderer != "default" {
        sel = sel.renderer_name(renderer.into());
    }
    if let Err(e) = sel.select() {
        eprintln!("backend: {e}");
        return 2;
    }
    println!("Click into the game now. Taking the window in front in 5 seconds.");
    std::thread::sleep(Duration::from_secs(5));
    let game = win::foreground();
    let g = win::client_rect_screen(game);
    if game.is_invalid() || g.right <= g.left || g.bottom <= g.top {
        eprintln!(
            "No usable game window in front (window 0x{:X}, client {:?}). Start again and click into the game.",
            game.0 as usize,
            (g.left, g.top, g.right, g.bottom)
        );
        return 2;
    }
    if win::pid_of(game) == std::process::id() {
        eprintln!("The window in front is this program, not a game. Start again and click into the game.");
        return 2;
    }
    println!("Game window 0x{:X}, client {:?}", game.0 as usize, (g.left, g.top, g.right, g.bottom));

    let (Ok(pill), Ok(menu), Ok(zone)) = (
        overlay::new_overlay(PillWindow::new),
        overlay::new_overlay(MenuWindow::new),
        overlay::new_overlay(ZoneWindow::new),
    ) else {
        eprintln!("could not create the windows");
        return 2;
    };
    let wanted = Arc::new(Wanted::default());
    let demo = Rc::new(Demo {
        game_rect: g,
        pill,
        menu,
        zone,
        wanted: wanted.clone(),
        menu_shown: Cell::new(false),
        zone_shown: Cell::new(false),
        pill_shown: Cell::new(false),
    });
    DEMO.with(|d| *d.borrow_mut() = Some(demo.clone()));
    let hook: Rc<RefCell<Option<keys::Hook>>> = Rc::new(RefCell::new(None));

    // The HWND exists only once the event loop has started (a zero-delay timer is too early): show the
    // pill and install the hook from a timer.
    {
        let (demo, hook) = (demo.clone(), hook.clone());
        slint::Timer::single_shot(Duration::from_millis(300), move || {
            demo.sync();
            let w = wanted.clone();
            match keys::install(move |ev| on_key(&w, ev)) {
                Ok(h) => *hook.borrow_mut() = Some(h),
                Err(e) => println!("key hook: {e}"),
            }
        });
    }

    let lost = Rc::new(Cell::new(0u32));
    let timer = slint::Timer::default();
    {
        let lost = lost.clone();
        timer.start(slint::TimerMode::Repeated, Duration::from_secs(1), move || {
            let fg = win::foreground();
            if fg != game {
                lost.set(lost.get() + 1);
                println!("Game NOT in front (front window 0x{:X})", fg.0 as usize);
            } else {
                println!("Game in front");
            }
        });
    }
    let _ = slint::run_event_loop_until_quit();
    // Removes the hook, ends its thread and clears its handler.
    hook.borrow_mut().take();
    DEMO.with(|d| d.borrow_mut().take());
    println!("Done. Seconds the game was not in front: {}", lost.get());
    0
}
