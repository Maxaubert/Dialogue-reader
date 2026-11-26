//! Measures Kokoro on this PC: first load, one dialogue line, memory. Plays nothing.
//! `cargo run --release -p dr-tts --example kokoro_bench`
#[cfg(windows)]
fn main() {
    use dr_tts::{KokoroFiles, KokoroTts, Tts};
    use std::time::Instant;
    let ws = || {
        std::process::Command::new("powershell")
            .args([
                "-NoProfile",
                "-Command",
                &format!("(Get-Process -Id {}).WorkingSet64", std::process::id()),
            ])
            .output()
            .ok()
            .and_then(|o| {
                String::from_utf8_lossy(&o.stdout)
                    .trim()
                    .parse::<u64>()
                    .ok()
            })
            .map_or("?".into(), |b| format!("{} MB", b / 1_000_000))
    };
    let files = match KokoroFiles::locate() {
        Ok(f) => f,
        Err(e) => return println!("{e}"),
    };
    println!("files: {files:?}");
    println!("memory before load: {}", ws());
    let mut k = KokoroTts::new(files);
    let t = Instant::now();
    if let Err(e) = k.load() {
        return println!("load failed: {e}");
    }
    println!("first load: {} ms", t.elapsed().as_millis());
    println!("memory after load: {}", ws());
    let line = "Arthur: We need to get these horses moving before the snow buries us.";
    for i in 0..3 {
        let t = Instant::now();
        match k.synth(line, "kokoro:af_heart", 1.0) {
            Ok(p) => println!(
                "synth {}: {} ms for {:.0} ms of sound",
                i + 1,
                t.elapsed().as_millis(),
                p.duration_ms()
            ),
            Err(e) => println!("synth failed: {e}"),
        }
    }
    println!("memory after 3 lines: {}", ws());
}
#[cfg(not(windows))]
fn main() {}
