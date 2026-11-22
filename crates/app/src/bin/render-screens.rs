//! Renders every DialogReader screen to PNG at 1440 x 900 with the software renderer. No window opens.
//!
//! Usage: cargo run -p dr-app --bin render-screens -- [out-dir]
//! The default out-dir is `target/screens` in the workspace.

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
    match dr_app::screens::render_all(&out) {
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
