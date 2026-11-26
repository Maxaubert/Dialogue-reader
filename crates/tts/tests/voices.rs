//! The Windows voices: list them and synthesize to PCM in memory. Nothing is played.
//!
//! The tests that need an installed Windows voice are `#[ignore]`d, so a PC with no voice shows them as "ignored"
//! instead of passing them without a check. Run them on a PC with a voice:
//! `cargo test -p dr-tts --test voices -- --ignored`. There they fail when no voice is found.

#![cfg(windows)]

use dr_tts::{Tts, WinTts};

/// The engine and its first voice. Panics when no voice is installed: these tests run only when asked for.
fn engine() -> (WinTts, String) {
    let tts = WinTts::new();
    let voices = tts.voices();
    eprintln!("Windows voices: {voices:?}");
    let v = voices.first().unwrap_or_else(|| {
        panic!("no Windows voice is installed (Windows Settings > Time & language > Speech)")
    });
    let id = v.id.clone();
    (tts, id)
}

#[test]
#[ignore = "needs an installed Windows voice: run with --ignored on a PC that has one"]
fn lists_voices_with_names_and_languages() {
    let (tts, _) = engine();
    for v in tts.voices() {
        assert!(!v.id.is_empty());
        assert!(
            !v.name.is_empty() && !v.name.starts_with("Microsoft "),
            "{v:?}"
        );
        assert!(v.lang.contains('-'), "{v:?}");
        assert_eq!(v.engine, "windows");
    }
}

#[test]
#[ignore = "needs an installed Windows voice: run with --ignored on a PC that has one"]
fn synth_gives_pcm_that_grows_with_text_and_shrinks_with_speed() {
    let (mut tts, voice) = engine();
    let short = tts.synth("test", &voice, 1.0).unwrap();
    assert!(
        short.rate >= 8000 && short.channels >= 1,
        "{} Hz, {} ch",
        short.rate,
        short.channels
    );
    assert!(short.duration_ms() > 100.0, "{} ms", short.duration_ms());
    assert!(short.samples.iter().any(|&s| s != 0), "not silent");

    let long = tts
        .synth(
            "test. We need to get these horses moving before the snow buries us.",
            &voice,
            1.0,
        )
        .unwrap();
    assert!(
        long.duration_ms() > short.duration_ms() * 2.0,
        "{} vs {}",
        long.duration_ms(),
        short.duration_ms()
    );

    let fast = tts
        .synth(
            "We need to get these horses moving before the snow buries us.",
            &voice,
            2.0,
        )
        .unwrap();
    let slow = tts
        .synth(
            "We need to get these horses moving before the snow buries us.",
            &voice,
            1.0,
        )
        .unwrap();
    assert!(
        fast.duration_ms() < slow.duration_ms() * 0.8,
        "fast {} slow {}",
        fast.duration_ms(),
        slow.duration_ms()
    );
}

#[test]
#[ignore = "needs an installed Windows voice: run with --ignored on a PC that has one"]
fn an_unknown_voice_uses_the_default() {
    let (mut tts, _) = engine();
    let pcm = tts.synth("test", "no such voice", 1.0).unwrap();
    assert!(pcm.duration_ms() > 100.0);
}

/// Runs on every PC: with no voice, the list is empty and `synth` says why it cannot speak.
#[test]
fn probe_agrees_with_the_list_and_synth_says_why_it_cannot_speak() {
    let mut tts = WinTts::new();
    let list = tts.voices();
    match WinTts::probe() {
        Ok(v) => assert_eq!(v, list),
        Err(why) => {
            assert!(list.is_empty());
            eprintln!("Windows could not list its voices: {why}");
        }
    }
    if list.is_empty() {
        let e = tts.synth("test", "", 1.0).unwrap_err();
        assert!(e.starts_with("Windows voices: "), "{e}");
        eprintln!("no Windows voice here (synth: {e})");
    }
}
