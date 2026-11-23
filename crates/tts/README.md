# dr-tts: text to speech for DialogReader

The `Tts` trait and its engines. An engine lists voices and turns text into 16-bit PCM in memory. It never plays
sound: the speech queue, the speech thread and audio out (rodio, or a null sink for tests) are in
`crates/runtime/src/speech.rs`.

| Type | What |
|---|---|
| `Tts` | `voices()` and `synth(text, voice id, speed)` |
| `WinTts` | the voices built into Windows (`Windows.Media.SpeechSynthesis`), engine name `windows`. `WinTts::probe()` gives the list or why it is empty |
| `FakeTts` | tests: silent PCM, 60 ms per character at speed 1, and a log of every call |
| `wav::parse`, `wav::write` | 16-bit PCM WAV |

Kokoro comes later as a second `impl Tts`.

- Tests: `cargo test -p dr-tts`. The Windows voice tests synthesize to memory and skip with a message when no
  Windows voice is installed. Nothing is played.
- `cargo run -p dr-tts --example probe_voices` prints what WinRT says about the voices (no sound).
- On Max's PC (2026-11-22) WinRT finds no voice: `AllVoices` fails with 0x80070002 because no OneCore voice is
  registered (only the SAPI 5 voice Zira is). Add a voice in Windows Settings > Time & language > Speech.
