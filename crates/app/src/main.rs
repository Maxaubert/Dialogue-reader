//! DialogReader.exe. M2: profiles in `%APPDATA%\DialogReader`, the real window list, a screenshot, and zones.
//! M3a: voices, with a voice test that plays on the speakers: Kokoro first when its files are on the PC, the Windows
//! voices when they are missing or its model does not load. M3b: reading (capture, OCR, speech) and the two
//! global keys, through `dr_runtime::runtime`.
//!
//! M4: the in-game overlays (pill, quick menu, zone edit) as overlay windows over the game (`dr_app::overlay`).
//!
//! `--fake` shows the mockup data instead of real profiles. `--preview-overlay <pill|quick-menu|quick-menu-voice|
//! zone-edit>` shows an in-game overlay inside the main window (the mockup review; the real ones are windows).
//! `--page <name>` opens a page directly. `--version` prints the version.
//!
//! M5: one DialogReader at a time (`dr_app::single`: a second start shows the first one's window and exits), the
//! tray icon (`dr_app::tray`), close hides to the tray, and `--minimized` (Start with Windows) starts in the tray when
//! "Open minimized" is on (with a tray icon: without one the window shows). `--no-tray` leaves the tray icon out (the
//! single-instance test uses it). The listener window (`dr_app::single`) also ends the app for the installer and at
//! the end of the Windows session, and makes the tray icon again after Explorer restarts.

#![windows_subsystem = "windows"]

use std::cell::RefCell;
use std::rc::Rc;

use dr_app::Page;
use slint::ComponentHandle;

fn main() -> Result<(), slint::PlatformError> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--version") {
        attach_console();
        println!("DialogReader {}", dr_app::VERSION);
        return Ok(());
    }
    // before anything loads (voices, key hooks): a second start only asks the first to show its window
    let instance = dr_app::single::name();
    let _guard = match dr_app::single::claim(&instance) {
        dr_app::single::Claim::First(g) => g,
        dr_app::single::Claim::Second => {
            dr_app::single::show_first(&instance, std::time::Duration::from_secs(20));
            return Ok(());
        }
    };
    // the listener window right away, so a second start finds it while voices and the window still load; its
    // handlers are set in `run` (a show request that comes earlier waits for them)
    let listener = dr_app::single::listen(&instance)
        .map_err(|e| log("instance.log", &format!("{e}\n")))
        .ok();
    // A panic in the shipped exe (panic = "abort") ends the app at once; this writes a crash log first.
    dr_app::crash::install(None);
    let overlays = select_renderer();
    let r = run(&args, overlays, listener.as_ref());
    // A window that cannot open (for example OpenGL fails) would end the app without a word: say why in the logs.
    if let Err(e) = &r {
        log(
            "start-error.log",
            &format!("DialogReader could not open its window: {e}\n"),
        );
    }
    r
}

fn run(
    args: &[String],
    overlays: bool,
    listener: Option<&dr_app::single::Listener>,
) -> Result<(), slint::PlatformError> {
    let fake = args.iter().any(|a| a == "--fake");
    let paths = dr_runtime::Paths::from_env();
    let no_folder = paths.is_none() && !fake;
    // the speech thread reads the lines the reader finds (M3b); the runtime sends them there
    let mut speech = None;
    let source = match paths {
        Some(paths) if !fake => {
            // Start with Windows follows this exe (a moved folder, a portable copy replaced by the installed one).
            // Only here: tests and tools never write the app's Run value.
            if let Ok(exe) = std::env::current_exe()
                && let Err(e) = dr_runtime::autostart::Autostart::app().follow(&exe)
            {
                log("autostart.log", &format!("Start with Windows: {e}\n"));
            }
            // the only place that can reach the speakers (the voice test and reading)
            let voices = Box::new(dr_app::vm::voices::Voices::real());
            speech = voices.speech.as_ref().map(|s| s.sender());
            dr_app::vm::Source::Real {
                paths,
                desktop: Box::new(dr_app::vm::RealDesktop),
                voices,
            }
        }
        _ => dr_app::vm::Source::Fake,
    };
    let (w, vm) = dr_app::build_window(source)?;
    // the only place that captures the game, reads it and installs the global key hooks
    if let Some(tx) = speech {
        let rt = dr_runtime::runtime::Runtime::new(dr_runtime::runtime::Parts::real(tx));
        let reading_log = dr_runtime::paths::crash_logs().map(|d| d.join("reading.log"));
        dr_app::vm::attach_runtime(&vm, rt, reading_log);
        // the in-game overlays need the winit backend with the overlay hook and an OpenGL renderer
        if overlays {
            match dr_app::overlay::Overlays::new(true) {
                Ok(o) => dr_app::vm::attach_overlays(&vm, o),
                Err(e) => log(
                    "renderer.log",
                    &format!("the in-game overlays could not be made: {e}\n"),
                ),
            }
        }
    }
    if no_folder {
        w.set_notice(
            "Windows did not say where app data goes (APPDATA is not set), so nothing can be saved. \
             This is sample data."
                .into(),
        );
    }
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--preview-overlay" => {
                if let Some(o) = it.next().and_then(|v| dr_app::overlay_from_name(v)) {
                    w.set_page(Page::Overview);
                    w.set_overlay(o);
                }
            }
            "--page" => {
                if let Some(p) = it.next().and_then(|v| dr_app::page_from_name(v)) {
                    w.set_page(p);
                }
            }
            _ => {}
        }
    }
    // the tray icon keeps the app running while the window is hidden; without one, closing quits (shell.rs)
    let no_tray = args.iter().any(|a| a == "--no-tray");
    let tray: Rc<RefCell<Option<dr_app::tray::TrayIcon>>> = Rc::default();
    if !no_tray {
        make_tray(&w, &vm, &tray);
    }
    if let Some(l) = listener {
        // a second start asks for the window (single.rs)
        let weak = w.as_weak();
        l.on_show(move || {
            if let Some(w) = weak.upgrade() {
                dr_app::shell::show_main_in_front(&w);
            }
        });
        // the installer, the uninstaller or the end of the Windows session: end as tray Quit does
        let v = Rc::downgrade(&vm);
        l.on_quit(move || dr_app::tray::quit(&v));
        // Explorer restarted: the tray icon is made again, after this message is handled
        if !no_tray {
            let (weak, v, t) = (w.as_weak(), Rc::downgrade(&vm), Rc::downgrade(&tray));
            l.on_taskbar_created(move || {
                let (weak, v, t) = (weak.clone(), v.clone(), t.clone());
                slint::Timer::single_shot(std::time::Duration::ZERO, move || {
                    if let (Some(w), Some(vm), Some(t)) = (weak.upgrade(), v.upgrade(), t.upgrade())
                    {
                        let existing = t.borrow().clone();
                        match existing {
                            Some(icon) => {
                                if let Err(e) = icon.remake() {
                                    log(
                                        "tray.log",
                                        &format!("the tray icon could not be made again: {e}\n"),
                                    );
                                }
                            }
                            None => make_tray(&w, &vm, &t),
                        }
                    }
                });
            });
        }
    }
    // Start with Windows passes --minimized: only the tray icon shows when "Open minimized" is on. With no tray icon
    // the window shows, so the app is never running with nothing on screen.
    let minimized = args.iter().any(|a| a == "--minimized")
        && vm.borrow().prefs().open_minimized
        && dr_app::shell::tray_up();
    if !minimized {
        dr_app::shell::show_main(&w);
    }
    let r = slint::run_event_loop_until_quit();
    let _ = w.hide();
    r
}

/// Makes the tray icon into `slot`. A failure is written to `tray.log` (the app then runs without one).
fn make_tray(
    w: &dr_app::MainWindow,
    vm: &dr_app::vm::Shared,
    slot: &Rc<RefCell<Option<dr_app::tray::TrayIcon>>>,
) {
    match dr_app::tray::make(w, vm) {
        Ok(t) => *slot.borrow_mut() = Some(t),
        Err(e) => log(
            "tray.log",
            &format!("the tray icon could not be made: {e}\n"),
        ),
    }
}

/// Writes `text` to `name` in the logs folder (`%LOCALAPPDATA%\DialogReader\logs`).
fn log(name: &str, text: &str) {
    if let Some(dir) = dr_runtime::paths::crash_logs() {
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::write(dir.join(name), text);
    }
}

/// A windows-subsystem exe has no console, so `--version` would print nothing in a terminal. This attaches to
/// the console of the terminal that started it (nothing happens when there is none, or output is redirected).
fn attach_console() {
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn AttachConsole(process_id: u32) -> i32;
    }
    const ATTACH_PARENT_PROCESS: u32 = u32::MAX;
    unsafe {
        AttachConsole(ATTACH_PARENT_PROCESS);
    }
}

/// The winit backend with the overlay attributes hook (docs/overlay-spike.md) and Skia through OpenGL when it is
/// built in (the default; femtovg with the fallback feature). One renderer serves every window of the process, and
/// the overlays need an OpenGL one: Skia's default surface draws their see-through margins black. `SLINT_BACKEND`
/// picks another renderer (for example `winit-software`, for debugging: the overlays are off with it). Returns true
/// when the overlays can be shown: the winit backend with the hook was selected, with a renderer that is not the
/// software one. A failure is written to `renderer.log` in the logs folder.
fn select_renderer() -> bool {
    let renderer = match std::env::var("SLINT_BACKEND") {
        Ok(b) => match b.strip_prefix("winit-") {
            Some(r) => Some(r.to_string()),
            None if b == "winit" => None,
            // another backend: no overlay windows
            None => return false,
        },
        Err(_) => default_renderer().map(str::to_string),
    };
    let mut sel = slint::BackendSelector::new().backend_name("winit".into());
    if let Some(r) = &renderer {
        sel = sel.renderer_name(r.clone());
    }
    #[cfg(windows)]
    {
        sel = sel.with_winit_window_attributes_hook(dr_app::overlay::win::attributes_hook);
    }
    match sel.select() {
        // a software renderer draws the see-through margins black: no overlays with it (debug use only)
        Ok(()) => !renderer
            .as_deref()
            .is_some_and(|r| r.starts_with("software")),
        Err(e) => {
            log(
                "renderer.log",
                &format!(
                    "{} could not be selected: {e}. The in-game overlays are off.\n",
                    renderer.as_deref().unwrap_or("winit")
                ),
            );
            false
        }
    }
}

/// The renderer the build has: skia-opengl (default feature) or femtovg (fallback feature).
fn default_renderer() -> Option<&'static str> {
    if cfg!(feature = "renderer-skia") {
        Some("skia-opengl")
    } else if cfg!(feature = "renderer-femtovg") {
        Some("femtovg")
    } else {
        None
    }
}
