//! The real Kokoro engine: load the model and synthesize to PCM in memory. Nothing is played.
//!
//! The tests need Kokoro's files (`kokoro-v1.0.onnx`, `voices-v1.0.bin`, `onnxruntime.dll`) where
//! `KokoroFiles::locate` looks, so they are `#[ignore]`d and show as "ignored" on a PC without them. Run them with
//! `cargo test -p dr-tts --test kokoro -- --ignored`; there they fail when the files are not found.

#![cfg(windows)]

use std::time::Instant;

use dr_tts::{KokoroFiles, KokoroTts, Tts};

const LINE: &str = "Arthur: We need to get these horses moving before the snow buries us.";

fn engine() -> KokoroTts {
    let files = KokoroFiles::locate().unwrap_or_else(|e| panic!("{e}"));
    eprintln!("Kokoro files: {files:?}");
    KokoroTts::new(files)
}

#[test]
#[ignore = "needs Kokoro's files: run with --ignored on a PC that has them"]
fn speaks_a_dialogue_line_into_pcm() {
    let mut k = engine();
    let t = Instant::now();
    k.load().unwrap();
    eprintln!("load: {} ms", t.elapsed().as_millis());
    assert!(k.is_loaded());

    let t = Instant::now();
    let pcm = k.synth(LINE, "kokoro:af_heart", 1.0).unwrap();
    let ms = pcm.duration_ms();
    eprintln!(
        "synth: {} ms for {ms:.0} ms of sound",
        t.elapsed().as_millis()
    );
    assert_eq!((pcm.rate, pcm.channels), (24_000, 1));
    // 13 words: about 3 to 7 s at speed 1
    assert!((2500.0..8000.0).contains(&ms), "{ms} ms");
    let loud = pcm
        .samples
        .iter()
        .filter(|s| s.unsigned_abs() > 1000)
        .count();
    assert!(
        loud > pcm.samples.len() / 10,
        "mostly silent: {loud} loud samples"
    );

    // faster speed, shorter sound; a British male voice works too
    let fast = k.synth(LINE, "bm_george", 2.0).unwrap();
    assert!(
        fast.duration_ms() < ms * 0.75,
        "{} vs {ms}",
        fast.duration_ms()
    );
    // a short question (lead-in cut off again)
    let q = k.synth("Should I?", "am_adam", 1.0).unwrap();
    assert!(
        (200.0..2500.0).contains(&q.duration_ms()),
        "{} ms",
        q.duration_ms()
    );
}
