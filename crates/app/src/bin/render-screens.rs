//! Renders every DialogReader screen to PNG at 1440 x 900 with the software renderer. No window opens.
//!
//! Usage: cargo run -p dr-app --bin render-screens -- [out-dir] [WxH]
//! The default out-dir is `target/screens` in the workspace; WxH is the window size (default 1440x900).

use std::path::PathBuf;

fn main() {
    let out = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("..")
                .join("..")
                .join("target")
                .join("screens")
        });
    let size = std::env::args().nth(2).map(|s| {
        s.split_once('x')
            .and_then(|(w, h)| Some((w.parse::<u32>().ok()?, h.parse::<u32>().ok()?)))
            .unwrap_or_else(|| {
                eprintln!("render-screens: bad size {s}");
                std::process::exit(2);
            })
    });
    let (w, h) = size.unwrap_or((dr_app::screens::WIDTH, dr_app::screens::HEIGHT));
    match dr_app::screens::render_all_at(&out, w, h) {
        Ok(files) => {
            for f in &files {
                println!("{}", f.display());
            }
            println!("{} screens written to {}", files.len(), out.display());
        }
        Err(e) => {
            eprintln!("render-screens: {e}");
            std::process::exit(1);
        }
    }
}
