//! DialogReader.exe. M1: the UI on fake data.
//!
//! `--preview-overlay <pill|quick-menu|quick-menu-voice|zone-edit>` shows an in-game overlay inside the main
//! window (debug: the real overlay windows come in M4). `--page <name>` opens a page directly.

#![windows_subsystem = "windows"]

use dr_app::Page;
use slint::ComponentHandle;

fn main() -> Result<(), slint::PlatformError> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--version") {
        println!("DialogReader {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    let w = dr_app::build_main_window()?;
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
