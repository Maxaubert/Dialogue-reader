//! DialogReader.exe. M2: profiles in `%APPDATA%\DialogReader`, the real window list, a screenshot, and zones.
//! M3a: voices, with a voice test that plays on the speakers: Kokoro first when its files are on the PC, the Windows
//! voices when they are missing or its model does not load. M3b: reading (capture, OCR, speech) and the two
//! global keys, through `dr_runtime::runtime`.
//!
//! `--fake` shows the mockup data instead of real profiles. `--preview-overlay <pill|quick-menu|quick-menu-voice|
//! zone-edit>` shows an in-game overlay inside the main window (debug: the real overlay windows come in M4).
//! `--page <name>` opens a page directly. `--version` prints the version.

#![windows_subsystem = "windows"]

use dr_app::Page;
use slint::ComponentHandle;

fn main() -> Result<(), slint::PlatformError> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--version") {
        attach_console();
        println!("DialogReader {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    // A panic in the shipped exe (panic = "abort") ends the app at once; this writes a crash log first.
    dr_app::crash::install(None);
    select_renderer();
    let r = run(&args);
    // A window that cannot open (for example OpenGL fails) would end the app without a word: say why in the logs.
    if let Err(e) = &r {
        log(
            "start-error.log",
            &format!("DialogReader could not open its window: {e}\n"),
        );
    }
    r
}

fn run(args: &[String]) -> Result<(), slint::PlatformError> {
    let fake = args.iter().any(|a| a == "--fake");
    let paths = dr_runtime::Paths::from_env();
    let no_folder = paths.is_none() && !fake;
    // the speech thread reads the lines the reader finds (M3b); the runtime sends them there
    let mut speech = None;
    let source = match paths {
        Some(paths) if !fake => {
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
        let log = dr_runtime::paths::crash_logs().map(|d| d.join("reading.log"));
        dr_app::vm::attach_runtime(&vm, rt, log);
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
    w.run()
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

/// Skia through OpenGL when it is built in (the default): overlays need it (docs/overlay-spike.md), and one
/// renderer serves every window of the process. `SLINT_BACKEND` overrides it (for example `winit-software`).
fn select_renderer() {
    if std::env::var_os("SLINT_BACKEND").is_some() {
        return;
    }
    #[cfg(feature = "renderer-skia")]
    {
        let r = slint::BackendSelector::new()
            .backend_name("winit".into())
            .renderer_name("skia-opengl".into())
            .select();
        // Without it Slint picks its default renderer; say so in the logs folder, since overlays (M4) need it.
        if let Err(e) = r {
            log(
                "renderer.log",
                &format!("skia-opengl could not be selected: {e}\n"),
            );
        }
    }
}
