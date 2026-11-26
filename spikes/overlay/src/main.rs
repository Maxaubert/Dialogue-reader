//! overlay-spike game | probe --out FILE [--renderer femtovg|skia|skia-opengl|software] [--naive]
//!     [--no-layered-alpha] [--layered-all] | run [probe flags] | compose [--renderer NAME] [--menu-like-pill]
//!     | demo [--renderer NAME]
//!
//! `demo` is the visible manual check (see src/demo.rs); it shows windows on the real screen.
//! `compose` runs on the normal desktop with the overlays off screen and checks their alpha with WGC.
//! `run` is the one to use by hand: it makes a hidden desktop, runs `game` and `probe` there and prints the
//! probe's JSON. Nothing appears on the screen.

use std::path::PathBuf;

use overlay_spike::overlay::Opts;
use overlay_spike::{probe, win};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let flag = |name: &str| args.iter().any(|a| a == name);
    let value = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned();
    match args.first().map(String::as_str) {
        Some("game") => {
            let secs = value("--secs").and_then(|s| s.parse().ok()).unwrap_or(90);
            if let Err(e) = win::run_game(secs) {
                eprintln!("game: {e}");
                std::process::exit(2);
            }
        }
        Some("probe") => {
            let Some(out) = value("--out") else {
                eprintln!("probe needs --out FILE");
                std::process::exit(2);
            };
            let opts = Opts {
                subclass: !flag("--naive"),
                layered_alpha: !flag("--no-layered-alpha"),
                layered_all: flag("--layered-all"),
            };
            let renderer = value("--renderer").unwrap_or_else(|| "femtovg".into());
            std::process::exit(probe::run(probe::Args { out: PathBuf::from(out), renderer, opts }));
        }
        Some("compose") => {
            let opts = Opts {
                subclass: !flag("--naive"),
                layered_alpha: !flag("--no-layered-alpha"),
                layered_all: flag("--layered-all"),
            };
            let renderer = value("--renderer").unwrap_or_else(|| "femtovg".into());
            std::process::exit(overlay_spike::compose::run(&renderer, opts, flag("--menu-like-pill")));
        }
        Some("demo") => {
            // Visible on the real screen. Only for the manual check, with Max's yes.
            let renderer = value("--renderer").unwrap_or_else(|| "femtovg".into());
            std::process::exit(overlay_spike::demo::run(&renderer));
        }
        Some("run") => {
            let exe = std::env::current_exe().expect("own path");
            let out = std::env::temp_dir().join(format!("overlay-spike-{}.json", std::process::id()));
            let rest: Vec<&str> = args[1..].iter().map(String::as_str).collect();
            match win::run_hidden(&exe, &rest, &out) {
                Ok(r) => {
                    println!("probe exit: {:?}", r.probe_exit);
                    println!("{}", r.json.unwrap_or_else(|| "(no report)".into()));
                    let _ = std::fs::remove_file(&out);
                }
                Err(e) => {
                    eprintln!("hidden desktop: {e}");
                    std::process::exit(2);
                }
            }
        }
        _ => {
            eprintln!("usage: overlay-spike game | probe --out FILE [--renderer femtovg|skia|skia-opengl|software] [--naive] [--no-layered-alpha] [--layered-all] | run [probe flags] | compose [--renderer NAME] [--menu-like-pill] | demo [--renderer NAME]");
            std::process::exit(2);
        }
    }
}
