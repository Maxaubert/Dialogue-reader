# dr-tts: text to speech for DialogReader

The `Tts` trait and its engines. An engine lists voices and turns text into 16-bit PCM in memory. It never plays
sound: the speech queue, the speech thread and audio out (rodio, or a null sink for tests) are in
`crates/runtime/src/speech.rs`.

| Type | What |
|---|---|
| `Tts` | `voices()` and `synth(text, voice id, speed)` |
| `KokoroTts` | Kokoro voices, the main engine when its files are on the PC (ships with the app: docs/app-spec.md section 6). Engine name `kokoro`, 28 English voices, 24 kHz mono. `src/kokoro` |
| `KokoroFiles` | where the model, the voice styles and `onnxruntime.dll` are (`locate()`), never downloads |
| `WinTts` | the voices built into Windows (`Windows.Media.SpeechSynthesis`), engine name `windows`, the backup when Kokoro's files are missing. `WinTts::probe()` gives the list or why it is empty |
| `Engines` | one engine over both. Profile ids carry the engine: `kokoro:af_heart`, `win:<id>`; an id without a prefix is a Windows id |
| `FakeTts` | tests: silent PCM, 60 ms per character at speed 1, and a log of every call |
| `wav::parse`, `wav::write` | 16-bit PCM WAV |

Kokoro's code comes from Sonara (MIT, `LICENSE-Sonara`): `misaki` (the G2P) is a git dependency pinned to a commit,
and the Kokoro modules are copied into `src/kokoro` and moved to `ort` 2.0.0-rc.15. Why not the whole
`sonara-engine` crate: docs/app-spec.md, section 6, "Kokoro engine (built 2026-11-23)".

Where Kokoro's files are looked for, in order:

| File | Places |
|---|---|
| `kokoro-v1.0.onnx`, `voices-v1.0.bin` | `<exe dir>\kokoro\` (the app folder), `%LOCALAPPDATA%\DialogReader\models\kokoro\v1.0\`, then Sonara's `%LOCALAPPDATA%\Sonara\models\kokoro\v1.0\` and `%USERPROFILE%\.sonara\kokoro\` (developer fallback: debug builds, or `DIALOGREADER_SONARA_KOKORO=1`) |
| `onnxruntime.dll` | `<exe dir>`, then `%LOCALAPPDATA%\Sonara\runtime\<newest version>\` (developer fallback, as above) |

- Tests: `cargo test -p dr-tts`. Unit tests use a fake model. Tests that need real files are `#[ignore]`d: the
  Windows voice tests (`--test voices`) and the Kokoro test (`--test kokoro`). Run them with `-- --ignored` on a
  PC that has the voices or the files; there they fail when they are missing. Nothing is played.
- `cargo run --release -p dr-tts --example kokoro_bench` measures Kokoro's first load, one dialogue line and
  memory (no sound). A release build looks in Sonara's folders only with `DIALOGREADER_SONARA_KOKORO=1` set.
- `cargo run -p dr-tts --example kokoro_text` prints what game lines become before the model (no model needed).
- `cargo run -p dr-tts --example probe_voices` prints what WinRT says about the voices (no sound).
- On Max's PC (2026-11-22) WinRT finds no voice: `AllVoices` fails with 0x80070002 because no OneCore voice is
  registered (only the SAPI 5 voice Zira is). Kokoro works there.
