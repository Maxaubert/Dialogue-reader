# App plan: milestones

Written 2026-11-22. Build order for the app in [app-spec.md](app-spec.md). Each milestone is one PR (or a short
stack), ships something that works, and is tested headless. Nothing in a test shows a window, plays sound or takes
the foreground.

Shared rules for every milestone:
- Build with the session's `CARGO_TARGET_DIR` (the repo target). Never pass `--target-dir`.
- `cargo test --workspace` and `cargo clippy --workspace --all-targets -- -D warnings` pass.
- Arena numbers do not change unless the milestone says so (`docs/arena-baseline.md`, `docs/reader-a1.md`).
- New dependencies are pinned to an exact version.
- Plain-English docs, no em-dashes.

## M1: UI shell on fake data

**Scope.** Every approved screen in Slint, filled with the fake data of the mockups (Red Dead Redemption 2, five
zones, Kokoro · Adam, Mouse 4 and F7). Navigation works: onboarding 01 > 02 > 03 > 04 > 05 > 06 > main, Back on
every step, tabs Overview / Zones / Settings, Ctrl K and the game button open the game switcher, Esc closes it.
Overlays (pill, quick menu, quick menu voice list, zone edit) render as components on fake data; they are not yet
real overlay windows. No capture, OCR, speech or global keys.

**Files.**
```
Cargo.toml                         add "crates/app" to members
crates/app/Cargo.toml              dr-app; slint (exact), slint-build; dev: i-slint-backend-testing (same version), image
crates/app/build.rs                slint_build::compile("ui/app.slint")
crates/app/src/main.rs             windows_subsystem = "windows", starts MainWindow with fake data
crates/app/src/lib.rs              app setup shared by main, the tests and the render-screens bin
crates/app/src/fake.rs             fake profiles, zones, voices, recent lines, windows list (fills the window
                                   directly; the view-model layer src/vm/ starts in M2, see there)
crates/app/ui/app.slint            MainWindow, Page enum, navigation
crates/app/ui/theme.slint          Palette, Space, Radius, Size, Type globals (spec section 3)
crates/app/ui/components/*.slint   shared components (spec section 3.1)
crates/app/ui/pages/*.slint        welcome, pick_window, draw_zones, keys, voice, mode, overview, zones, settings
crates/app/ui/overlays/*.slint     pill, quick_menu, zone_edit, game_switcher; quick_menu_preview (review only)
crates/app/ui/fonts/*.ttf          Geist and Geist Mono variable fonts (OFL, Google Fonts builds), with the licence
crates/app/ui/icons/*.svg          exported from design/mockups/c-midnight/_build.py
crates/app/ui/art/*                fake game scenes from the mockups (SVG; scene-1440.png for software renders)
crates/app/tools/export_icons.py   writes icons/ and art/ from _build.py
crates/app/src/bin/render-screens.rs  renders every page and overlay to PNG (software renderer, no window)
crates/app/tests/screens.rs        each page renders at 1440x900, is not blank, and matches its mockup shot
                                   within a tolerance (10 px cells); golden images once approved
crates/app/tests/nav.rs            testing backend: click through onboarding, tabs, switcher, Esc
crates/app/README.md               how to run, test and render screens
```

**Tests.**
- `tests/screens.rs`: every page and overlay renders headless to PNG with no panic. Each PNG is not one flat color.
  Each screen is compared with its mockup shot in `design/mockups/c-midnight/shots/`: both are averaged over
  10 x 10 px cells, and the mean difference and the share of changed cells must stay under a limit per screen.
  It catches moved, missing or recolored blocks, not font weight or 1 to 2 px shifts.
- Golden images (`crates/app/tests/golden/`) come only after Max approves the screens and the open questions in
  spec section 6 (kind colors, Settings keys) are settled, so they start from a fixed reference.
- `tests/nav.rs` (testing backend): Get started goes to page 02; Continue and Back walk all five steps; Finish
  lands on Overview; tabs switch; Ctrl K opens the switcher and Esc closes it; zone toggles change the view model.
- Unit tests for the stage mapping (image px to view px and back) and for zone drawing math (normalize a drag,
  minimum size, clamp to the image, hit test of handles).

**Review aid.** `cargo run -p dr-app --bin render-screens -- <scratch dir>` writes all PNGs. A side-by-side page
(mockup shot and Slint render) is built in the scratchpad for Max. All 14 approved mockup shots are in `shots/`
(01, 02, 05, 06, 12, 13 and 14 were added on 2026-11-22, drawn from the HTML with headless Edge).

**Done when.** All tests pass headless; the screens PNGs match the mockups closely (same layout, colors, type,
icons; small text-height differences allowed); Max has seen the side-by-side page. The app window is opened only if
Max says yes in that session.

## Spike after M1: overlay windows

The biggest risk of the app is the overlay window, so it is checked right after M1, not in M4. One small test
program (in `spikes/`) makes a Slint window on winit that is transparent, frameless, topmost and never takes focus
(`WS_EX_NOACTIVATE`, `WS_EX_TOOLWINDOW`, `WS_EX_TOPMOST`, and `WS_EX_LAYERED` + `WS_EX_TRANSPARENT` for the
click-through pill). It runs on a hidden second desktop and checks: the styles are set, `GetForegroundWindow` does
not change when it shows, and the see-through parts are really see-through (per-pixel alpha).

**Done when.** A short note in `docs/` says which way M4 draws the overlays: Slint windows on winit, or the
software renderer into a Win32 layered window (`UpdateLayeredWindow`).

**Result (2026-11-22, [overlay-spike.md](overlay-spike.md)).** Slint windows on winit, with an attributes hook, styles
set right after a show that gives a new HWND, and a window subclass that keeps them (winit rewrites `GWL_EXSTYLE` on
every show). All hidden-desktop checks passed. Composed transparency cannot be seen headless (for the quick menu and
zone edit not even off screen), so Max runs one visible check (`overlay-spike demo`) before M4. If the pill corners
are not see-through, the pill moves to the fallback; if the menu or zone edit shows black, they move too.

## M2: profiles, window picking, screenshot, zone drawing

**Scope.** Real data for setup. Profiles saved as JSON in `%APPDATA%\DialogReader\` (spec 2.8). Page 02 lists real
windows (games first) with icons, size and mode. Continue takes one screenshot with WGC. Pages 03 and 11 draw,
select, resize, rename, re-kind and delete zones on that screenshot and save them in capture pixels with `base`.
Keys and voice pages save their choices into the profile (key capture itself is M3b). The game switcher lists real
profiles. Onboarding runs on first start only (no profile yet).

**Files.**
```
crates/runtime/Cargo.toml          dr-runtime; windows, windows-capture, serde, serde_json
crates/runtime/src/profile.rs      Profile, ZoneCfg, load/save (atomic), migration by "version", kind names
crates/runtime/src/paths.rs        %APPDATA% folders (overridable root for tests)
crates/runtime/src/windows_list.rs enumerate windows, exe, size, mode, icon, game check
crates/runtime/src/screenshot.rs   one WGC frame -> PNG
crates/app/src/vm/mod.rs           view-model layer: plain Rust types for zones, voices, lines, windows,
                                   settings, and their Slint models; fake.rs moves behind it
crates/app/src/vm/zones.rs         drawing state, undo of the last change
crates/runtime/tests/profile.rs    round trip, bad file kept as .bad, migration, base scaling to dr_core::Zone
crates/app/tests/zones.rs          testing backend: drag to add, click to select, drag a corner, Delete, kind change
```

**Tests.** Profile round trip and `Zone::rect_in` scaling at 1080p, 1440p and 4K. Zone drawing through simulated
pointer events. The window list test runs against a window the test creates on a hidden desktop (no window on
Max's screen). Screenshot test captures that hidden test window, or is marked `#[ignore]` with a reason if WGC
refuses windows on another desktop (arena-spec.md notes that it does on this PC); then a manual check with a real
game is listed in the PR.

**Done when.** A profile made through onboarding is saved, survives a restart, and its zones load as
`dr_core::Zone`s with the right size on a frame of another size. All headless tests pass.

**Result (2026-11-22, branch `feat/app-m2`).** Built as planned, with these notes:
- `crates/runtime` also has `shot.rs` (a screenshot in memory, PNG load and save) and `testwin.rs` (test windows:
  a hidden desktop, and an off-screen layered window for capture tests). `examples/list_windows.rs` prints what
  page 02 would list.
- The window list test runs on a hidden desktop. The screenshot test captures an off-screen layered window it
  makes (WGC refuses windows on another desktop here), so it is not `#[ignore]`d. A manual check with a real game
  is still listed for the PR: pick a running game on page 02, Continue, and see its screenshot on page 03.
- `crates/app/tests/setup_flow.rs` is the done-when test: onboarding on a temp data folder with a fake desktop,
  Finish, then a second window on the same folder opens the profile on Overview, and the saved zones scale to 4K.
- Undo is one step, Ctrl+Z on pages 03 and 11. Rename is a text field in the selected zone row (no mockup for it).
- The game switcher lists real profiles with their screenshot as cover; "Add game" starts at page 02.
- Not real yet: voices (M3a), the activity log and status (M3b), zone edit and key capture (M3b, M4).
- Review fixes (2026-11-22): a new id never takes a file that was not loaded, and `id` comes from the file name;
  Delete during a drag ends the drag; page 02 and the zone lists scroll; only the current game's screenshot is
  decoded at full size, other covers are made small when the switcher first opens; an empty exe never matches
  (title only); Back on page 02 of "Add game" returns to Overview; a minimized window is refused at once; an
  unsupported "no border" or "no cursor" setting falls back to the default; speed, volume and the Overview mode
  are saved; a rename is saved on Enter or focus loss as one undo step; a tab or game change clears the notice.
- Still open: capture and PNG encoding run on the UI thread (up to 3 s for a window that gives no picture).
  Proposal, not yet planned: move capture to its own thread in M3b.

## M3a: OCR, speech and the voice screens (headless)

**Scope.** Everything of the reading path that needs no game and no keys: the WinRT OCR moves into `crates/ocr`,
`crates/tts` with Windows voices, audio out with volume, speed, mute and stop, the speech queue (spec 2.5), and the
small `dr-core` change for skipped lines. M3a also owns the voice screens, because v1 ships Windows voices and the
mockups say Kokoro (spec section 6, question 1):
- page 05: the list header ("Kokoro voices") and the "Get more voices" button,
- Settings > Voices (Kokoro and Piper rows),
- the fake data behind them,
- Max approves the new wording before it is built.

**Files.**
```
crates/ocr/                        dr-ocr: WinOcr moved from crates/arena/src/ocr/winrt.rs, impl dr_core::Ocr
crates/arena/src/ocr/winrt.rs      becomes a thin adapter over dr_ocr::WinOcr
crates/tts/                        dr-tts: Tts trait, Voice, Pcm, WinTts (SpeechSynthesizer), WAV parse
crates/runtime/src/speech.rs       queue rules (spec 2.5), audio sink (rodio), null sink for tests
crates/core/src/pipeline.rs        skipped list in Output (already read, prompts off), small change
crates/core/src/manual.rs          same for Press
crates/app/ui/pages/voice.slint    voice page header and button (new wording)
crates/app/ui/pages/settings.slint Voices section (new wording)
crates/ocr/tests/fixtures.rs       WinRT OCR reads fixture PNGs (skips if no en-US pack)
crates/tts/tests/voices.rs         lists voices, synth "test" to PCM (no playback)
crates/runtime/tests/speech.rs     queue order, Stop clears, old lines dropped, speaker + line joined
```

**Tests.** OCR move: the arena gives the same numbers on seed 59 (standard and hard) before and after. TTS: voices
list not empty, PCM length grows with text, speed changes length. Speech queue with a fake TTS and a null sink. No
test plays sound.

**Done when.** The arena numbers are unchanged, the Windows voices synthesize headless, the voice screens show
the wording Max approved, and all headless tests pass.

**Result (2026-11-22, branch `feat/app-m3a`).**
- `crates/ocr` (dr-ocr): `WinOcr` implements `dr_core::Ocr`; the reading code is the arena's, unchanged. The arena
  keeps `ocr::winrt::WinOcr` as an adapter (same name `winocr(<lang>)`, same `max_dim`). Fixture PNGs in
  `crates/ocr/tests/fixtures/` (Segoe UI 30 px, made with Pillow).
- Arena, before and after the move and the `dr-core` change, ideal clock (deterministic): every event of smoke,
  standard seed 59 and hard seed 59 (a1 and winocr-naive) and every crop of the OCR bench on standard are
  identical, timing fields left out. Same recall, CER, dup, noise, halluc and latency.
- Simulated clock, seed 59, against `docs/baselines/` (before and after run in turn, nothing else running). Every
  run exits 0 with "no regressions". Recall is the same; a1 moves by a few dup, noise or halluc, the run-to-run
  noise of a clock that charges real OCR time. That noise is about 4 duplicates on standard a1 (runs of the same
  code gave dup 79, 80, 81 and 83), not 1.

  | Run | Before | After |
  |---|---|---|
  | standard a1 | 0.788 (412/523), dup 80, noise 20, halluc 118 | 0.788 (412/523), dup 81, noise 20, halluc 118 |
  | standard winocr-naive | 0.558 (292/523), dup 309, noise 60, halluc 348 | same |
  | hard a1 | 0.392 (279/712), dup 28, noise 19, halluc 129 | 0.392 (279/712), dup 29, noise 20, halluc 128 |
  | hard winocr-naive | 0.320 (228/712), dup 134, noise 2, halluc 244 | same |

  Prompts off (`--reader-opt prompts=false`, the profile default), seed 59, from the M3a review (2026-11-23).
  Ideal clock, every scene's events identical before and after: standard a1 0.826 (432/523), dup 83, noise 37,
  halluc 117; hard a1 0.397 (283/712), dup 26, noise 23, halluc 132. Simulated clock (soft, another session was
  running): the same score before and after, 0.826 (432/523) on standard, but CPU per frame rose from p50 0.65 ms
  to 2.35 ms and p95 14.6 ms to 17.9 ms, because prompt zones are now read for the log (see M3b checklist).
- `crates/tts` (dr-tts): `Tts`, `Voice`, `Pcm`, `WinTts`, `wav::parse`, `FakeTts`, and `examples/probe_voices.rs`
  (prints what WinRT says about its voices). The queue, the speech thread, `RodioOut` and `NullSink` are in
  `crates/runtime/src/speech.rs` (rodio =0.21.1, playback only).
- `dr-core`: `Output::skipped` and `Press::skipped` (`Skipped { line, reason }`). Prompt zones with `read_prompts`
  off are read and go to `skipped` (prompts off). "Already read" is reported where the pipeline drops a line it
  already speaks (a line rescued after it ended). Nothing changes with `read_prompts` on (the arena default).
- Not met on this PC: Windows lists no WinRT voice here (`AllVoices` returns an empty list; `DefaultVoice` and
  `new` fail with 0x80070002; no OneCore voice is registered, only SAPI 5 Zira), so `WinTts::synth` has never run
  on a real voice. The three `dr-tts` tests that need a voice are `#[ignore]`d, so the gate shows them as
  "3 ignored" instead of passing them. Max adds an English voice in Windows Settings > Time & language > Speech
  once, then runs `cargo test -p dr-tts --test voices -- --ignored` (they fail when no voice is found). If that
  does not work, v1 needs a SAPI 5 engine before M3b (spec section 6, question 11).
- Games that share an exe: several profiles per exe (spec 2.8), `vm::best_profile`, `vm::running`, `vm::same_game`.

**Text changes for Max to approve** (old > new):

| Where | Old | New |
|---|---|---|
| 05 list header | Kokoro voices | Windows voices |
| 05 header button | Get more voices | Add voices (opens Windows Settings > Speech) |
| 05 test card label | Kokoro · Adam | Windows · David (the picked voice) |
| 05 button | Stop (always) | Play sample, and Stop while it plays |
| 05 empty list (new, review) | none | "No Windows voice is installed. Press Add voices to get one." |
| 05 list when Windows cannot list its voices (new, review) | none | "Windows could not list its voices. Press Add voices to check the speech settings." |
| 05 Play sample and 12 Test with no voice (review) | none | dimmed, cannot be pressed |
| 06 summary voice with no voice (new, review) | none | No voice · "Nothing is spoken until you add a voice." |
| 05 voice rows (sample data) | Adam, Bella, Michael ... | David, Zira, Mark, Hazel, George, Susan, ... |
| 12 Voices row 1 | Kokoro · Installed · 11 voices · 312 MB · Manage | Windows voices · Built into Windows · N voices · Add voices |
| 12 Voices row 2 | Piper · Not installed · 48 voices · about 60 MB each · Download | Kokoro · Coming later · more natural voices that run on your PC (no button) |
| 12 Voices row 3 | Default voice for new games: Kokoro · Adam | Voice for this game (game name below): Windows · David, with a Test button |
| 12 Voices rows 4 and 5 (new) | none | Speed (slider, 1.1×) and Volume (slider, 80%) |
| 06 summary voice | Kokoro · Adam, 1.1× speed | Windows · David, and the speed from page 05 |
| 10 voice button | Kokoro · Adam | Windows · David (the game's voice) |
| 01 preview card | Read just now · Kokoro · Adam | Read just now · Windows · David |
| quick menu voice (sample data) | Adam | David |
| 05 card label, 06, 10, 12 picker with no voice (new) | none | No voice |
| 05 list with no voice and no reason (new) | none | No voice found. |
| 12 row 1 with 0 or 1 voice (new) | none | Built into Windows · none installed / · 1 voice |
| 12 row 3 with no game (new) | none | Set up a game first. |
| 12 row 3 button (new) | none | Test, and Stop while it plays |
| 12 picker list rows (new) | none | David · US · Male |
| notice, test without speech, only with `--fake` (new, review) | none | Sample data: voices do not play here. |
| notice, test failed: engine (new, review) | none | The Windows voice could not speak the sample. |
| notice, test failed: no device (new, review) | none | No sound device was found. |
| notice, test failed: device lost (new, review) | none | The sound device stopped working. Try again. |
| notice, the first game of a shared exe could not be saved (new, review) | none | The other game of this program could not be saved: (error) |
| new game default speed | 1.1× (mockup slider) | 1× (the profile default) |

Layout change, not only wording (also for Max): Settings > Voices gained a Test button in the voice row and two
rows (Speed, Volume), and the Kokoro row has no button. Without them the game's voice could not be changed after
setup until the quick menu (M4). Page 05 keeps its layout; with no voice the list shows the message instead.
`render-screens` writes `18-voice-none.png`, `18-mode-voice-none.png` and `18-settings-voices-none.png` for that
state, and `18-settings-voice-list.png` with the Settings voice picker open (six rows show, the rest scroll).

**After the M3a review (2026-11-23).** Fixed: the voice tests that need a voice are ignored, not passed; the speech
thread recovers from a lost or stuck device and moves to a new default device; it joins the multithreaded COM
apartment; events carry the item and the test id, and status is written while a line is made, so a quick
Play, Stop, Play keeps the Stop button; a Stop during synth drops the sound; `WinTts` keeps the picked voice; the
first profile of a shared exe gets its title, titles are compared without counters, and the duplicate check uses
the same rule; the Settings sliders save when let go (and on a page change), not on every move; leaving page 05 or
Settings stops the sample; the page 05 Volume slider changes a sample that plays; with no voice the test buttons
are off and page 06 says nothing will be spoken; raw Windows and audio errors go to `speech.log`, the notice shows
a plain line; the Settings voice picker scrolls. Rows marked "(review)" in the table above are new or changed.
Arena after these fixes (none touch `dr-core` or `dr-ocr`): ideal clock, seed 59, standard a1, hard a1, standard
winocr-naive, and standard and hard a1 with prompts off, every scene's events identical to the review's runs (0 of
40 differ in each). Simulated clock against `docs/baselines/` (soft): all three exit 0 with "no regressions";
standard a1 0.788, dup 83, halluc 118; hard a1 0.390 (278/712), halluc 131; standard winocr-naive unchanged.

### Kokoro engine (built 2026-11-23)

Kokoro is the main engine when its files are on the PC; the Windows voices are the backup when Kokoro's files are
missing or its model does not load. DialogReader does not depend on an installed Sonara. Kokoro ships with the app
(Max, 2026-11-23: "kokoro voices should be bundled"). Why Sonara's code is copied and not used as a whole crate: spec
section 6, "Kokoro engine (built 2026-11-23)". Built on `feat/app-m3a`:

- `crates/tts/src/kokoro/`: `KokoroTts` (voices without a model, one load shared by clones, `preload` on its own
  thread, synth to PCM with 250 ms between sentences, `game_text` so capital lines and "O'" names are read as
  words, not spelled), `KokoroFiles` (looks in the app folder, then
  `%LOCALAPPDATA%\DialogReader\models\kokoro\v1.0`, then Sonara's folders in a debug build or with
  `DIALOGREADER_SONARA_KOKORO=1`; files count only at their known size; nothing is downloaded), and the modules
  copied from Sonara with attribution (`LICENSE-Sonara`).
- `crates/tts/src/engines.rs`: `Engines`, one `Tts` over Kokoro and Windows. Profile ids `kokoro:af_heart` and
  `win:<id>`; an id without a prefix is an old Windows id. A new game gets Heart (`af_heart`).
- `crates/app/src/vm/voices.rs`: `Voices::real()` (was `windows()`) builds both engines; the list is Kokoro's 28
  voices, or the Windows voices when Kokoro is missing. `VoiceData.engine` gives each row its engine name.
- Tests: unit tests on a fake model and fake engines; `cargo test -p dr-tts --test kokoro -- --ignored` runs the
  real model to PCM in memory (ignored by default, fails when the files are missing). Run on this PC: passed (load
  1681 ms and synth 427 ms in a debug build, 3840 ms of sound for the sample line). render-screens adds
  `18-voice-windows.png` and `18-settings-voices-windows.png` (Kokoro missing).
  `cargo run -p dr-tts --example kokoro_text` prints what game lines become (no model).
- Not tested yet: the app speaking through Kokoro end to end (`Engines`, the speech thread, the speakers): Max is
  the first to hear it. The files were found in Sonara's folders; the app-folder paths ran only in unit tests on
  temp folders.
- Measured on this PC (release build, `cargo run --release -p dr-tts --example kokoro_bench`, files from Sonara's
  folders, ONNX Runtime 1.28.2): first load 835 to 889 ms; one dialogue line (the 05 sample, 3.8 s of sound)
  389 to 430 ms; memory 7 MB before load, 507 MB after load, 605 MB after three lines.
- `DialogReader.exe` (`cargo build --profile ship`): 29,021,184 bytes, was 23,644,160 (+5.4 MB: the misaki
  lexicons and tagger are compiled in). Build 3 min 26 s. After the Kokoro review fixes: 29,032,960 bytes (build 3 min 7 s). The model (about 354 MB) and `onnxruntime.dll` are
  separate files.

**After the Kokoro review (2026-11-23).** A model that does not load (for example an `onnxruntime.dll` older than
API 24) no longer leaves Kokoro listed with every line failing: `Engines` speaks with the Windows default voice and
notes it once in `speech.log`, and the app reads `KokoroTts::watch` at each page change and voice test and then
lists the Windows voices. When Windows has no voice either, the error is Kokoro's. `Engines::synth` no longer lists
the Windows voices for every line. Sonara's folders are searched only in debug builds or with
`DIALOGREADER_SONARA_KOKORO=1`. A capital speaker name before a colon ("ZOE: Go.") is read as a name, not spelled.
`THIRD_PARTY_NOTICES.md` added (the misaki lexicons are in the exe). `vm::Source` holds `Box<Voices>` (no clippy
allow). The Settings Kokoro row says "Installed" (the approved mockup's word), not "Included".

**Text changes for Max to approve** (wait for question 13; old > new; old is the M3a text):

| Where | Old | New |
|---|---|---|
| 05 list header | Windows voices | Kokoro voices (as in the mockup); Windows voices when Kokoro is missing |
| 05 list count | number of Windows voices | 28 (Kokoro's English voices) |
| 05 voice rows | David, Zira, ... (Windows names) | Heart, Bella, Emma, Nicole, ... (Kokoro names, Kokoro's own order); sample data back to the mockup's Adam, Bella, Michael ... |
| 05 test card label | Windows · David | Kokoro · Heart (engine of the picked voice) |
| 05 header button | Add voices (opens Windows Settings > Speech) | unchanged (still adds Windows voices only; mockup said "Get more voices", open question) |
| 05 empty list | No Windows voice is installed. Press Add voices to get one. | The Kokoro voices are missing and no Windows voice is installed. Press Add voices to get a Windows voice. |
| 06 summary voice | Windows · David | Kokoro · Heart (engine of the picked voice) |
| 10 voice button, 01 preview card, quick menu (sample data) | Windows · David / Read just now · Windows · David / David | Kokoro · Adam / Read just now · Kokoro · Adam / Adam (back to the mockups) |
| 12 Voices row 1 | Windows voices · Built into Windows · N voices · Add voices | Kokoro · Installed · 28 voices · 354 MB (no button; mockup: "Installed · 11 voices · 312 MB" with "Manage") |
| 12 Voices row 2 | Kokoro · Coming later · more natural voices that run on your PC | Windows voices · Built into Windows · N voices · Add voices (moved down, unchanged) |
| 12 Kokoro row when Kokoro's files are missing (new) | none | Kokoro · Not found · Windows voices speak instead |
| 12 voice picker | Windows · David | Kokoro · Heart (engine of the voice) |
| notice, test failed: engine | The Windows voice could not speak the sample. | The voice could not speak the sample. |

Layout change (also for Max): Settings > Voices shows the Kokoro row first and the Windows row second, as mockup 12
(which had Kokoro, then Piper). Page 05 keeps its layout; with Kokoro the Windows voices are not listed (they
are the backup only).

## M3b: capture, keys and live reading

**Scope.** The real loop (spec 2.2): capture thread, reader thread with `Pipeline` (auto) or `Manual` (read key),
and global keys through low-level hooks with key capture on the keys page. Main page shows real status, the line
being spoken, and Recent from the log. Pause, Read now, mode, voice and key controls work. Elevated and exclusive
fullscreen games are detected and explained.

**Files.**
```
crates/runtime/src/capture.rs      FrameSource trait, WGC source (newest-frame slot, 40 ms), file source for tests
crates/runtime/src/reader.rs       reader thread: Pipeline / Manual, ReadNow, zone on/off, prompts
crates/runtime/src/keys.rs         Key type and text form, LL hooks thread, pure decode function, capture mode
crates/runtime/src/elevation.rs    elevated game check, restart as admin
crates/runtime/src/runtime.rs      Runtime handle: Cmd in, Ev out
crates/runtime/tests/live.rs       arena scene frames -> runtime -> fake TTS: lines spoken in order, Stop, Pause
```

**Tests.** Runtime test with arena frames: auto mode speaks the expected lines; manual mode speaks nothing until a
press; Stop clears the queue; Pause stops OCR (OCR call count stays flat). Keys: decode table for keyboard keys,
mouse 3/4/5, modifiers, refused left/right click.

**Manual check (listed in the PR, done by Max or with his yes).** Satisfactory borderless 4K: read key reads the
zones, auto mode reads dialogue, CPU use of DialogReader noted. One elevated program to see the warning.

**Done when.** A profile reads a live game in both modes with Windows voices, keys work in the game, CPU use is
noted in `docs/`, and all headless tests pass.

**Checklist from the M3a review (2026-11-23).**
- A Windows voice speaks on Max's PC, or a SAPI 5 engine is in (spec section 6, question 11). Before M3b starts.
- Prompts off costs OCR time: prompt zones are read for the log. Standard a1 (simulated clock, soft): CPU per frame
  p50 0.65 to 2.35 ms, p95 14.6 to 17.9 ms, same score. In manual mode a press waits for the OCR of every prompt
  zone before any line is spoken. Measure in the CPU check; if it shows, leave quiet zones out of a press (manual),
  or drop prompt zones and their log rows (spec Risks, "Log shows skipped lines").
- Drain the speech events on a timer (`Speech::events`): nothing reads them outside a voice test, so the channel
  grows once `Say` is used. The events carry the `Item` for the activity log. `Vm::test_voice` still throws away
  waiting events; with a reader running it must hand them to the log instead.
- The reader thread joins the multithreaded COM apartment first, as the speech thread does (WinRT OCR waits).
- `Drop for Speech` waits for a sound being made, so closing the app waits for one synth call (about a second).
- A new default device is checked by name at each line (`RodioOut::play`); a lost device is let go on its error
  or when a sound runs well past its length. Read from the rodio 0.21.1 and cpal 0.16 source; not tried with a
  real device (no test may play sound). Try once by hand with headphones.

**Result (2026-11-23, branch `feat/app-m3b`, not merged).** Built as planned, with these notes:
- `crates/runtime`: `capture.rs` (`FrameSource`; `WgcSource`, 40 ms, newest-frame slot with a spare buffer, so a
  4K frame is not allocated or copied twice; `ScriptSource` with a `Gate` for tests), `reader.rs` (reader thread),
  `keys.rs` (`Key` and its text form, `decode_key` / `decode_mouse`, the pure `State::feed`, `Hooks` with
  `WH_KEYBOARD_LL` and `WH_MOUSE_LL` on their own thread), `elevation.rs`, `runtime.rs` (`Runtime`, `Cmd`, `Ev`,
  `Parts`), and `activity.rs` (the activity log `log\<id>.jsonl`, capped at 5 MB, spec 2.8). `speech.rs` gained
  `SpeechTx` (the reader sends `Say` and `Replace` from its thread).
- The headless runtime test is `crates/runtime/tests/reading.rs` (the plan said `tests/live.rs`, which the live view
  already uses): an arena scene (speaker, two dialogue lines, a prompt) drawn frame by frame, real WinRT OCR, the
  speech thread with `FakeTts` and `NullSink`. Auto speaks the lines in order and logs the prompt as "prompts off";
  a line that shows on one frame of a still screen is spoken; manual reads nothing (and runs no OCR) until a press;
  Stop clears the queue; Pause keeps the OCR count flat. `crates/app/tests/reading.rs` covers the app side on fake
  parts. `tests/live.rs` gained `one_session_serves_the_reader_and_an_attached_live_view`.
- **One capture session per window.** The reader's session (`Live::start_full`) hands full-size frames to the
  reader; the live view of the same window attaches to it (`Attacher::attach`) and gets small GPU-scaled frames of
  the same captured frames, so there are never two WGC sessions on one window while reading runs. When the reader
  does not capture that window (no profile, game not found yet, setup page 03), the live view starts its own
  session as before. Cost: an attached live view gets about every second frame of the 40 ms session (12.5 frames a
  second instead of 15). When the reader starts or stops, the live view moves on its next check; the last frame
  stays on the stage meanwhile.
- **Pause** stops OCR only. Frames are still taken and the newest kept (a still window sends no new frame), so a
  press while paused, and Resume, read the screen at once. Cost while paused: the frame copy (25 a second).
- **Still screens.** WGC sends a frame only when the window changes. The reader keeps the newest frame and feeds it
  again every 40 ms with the time now, so a line on a screen that stays still is confirmed, and a press reads the
  screen as it is without waiting for a new frame.
- **Prompts off and the press.** A press reads the spoken zones first, hands their lines to speech (`Replace`), and
  only then reads the prompt zones for the log: two `dr_core::Manual`s on disjoint zones give the same lines as
  one. `dr-core` is not changed.
- **Speech events have one reader.** `Vm::drain_speech` (on a 100 ms UI timer, and in the voice test poll) gives the
  voice test its own events and turns the others into log lines and the "now speaking" card. A line is logged when
  it starts to be spoken, or as skipped: prompts off, already read, waited too long (Dropped), voice error, stopped
  before it was heard (Cut, or Cleared for a line that still waited in the queue). A replay (the card's Replay, a
  Recent row's replay) is not logged again; a row replay is spoken next and keeps the waiting lines.
- **Game switch.** Opening another game stops the old reader (joined, so its last skipped lines go to its log),
  then stops speech; the events of that Stop go to the old game's log until a speech mark comes back. Pause is not
  reset: the next game opens "Paused" (decided by Max, 2026-11-23, spec section 6).
- **Window.** The app finds the current game's window with its profile rules (`best_profile`, once a second on
  another thread while it is not read) and gives it to the runtime (`Cmd::Window`). A closed window ends the reader;
  a capture that fails on an open window is tried again after 3 s. The reader follows the current game; switching
  games by itself when another profile's game starts (spec 2.8) is not built.
- **Keys.** The read key goes from the hook thread straight to the reader (`ReadNow`), in both modes. The quick menu
  key comes out as an event (nothing uses it before M4). Hooks exist only while a profile is active or a key capture
  is open. Mouse moves and the wheel leave the mouse hook before any lock. Key capture swallows the key it takes
  (down and up); left and right click pass through and are refused with a plain line; a modifier alone is not a
  key; Alt+Tab, Alt+Esc, Ctrl+Esc and Win combos are not taken (Windows gets them); Esc, "Cancel", a page change
  or the main window losing the focus ends it. A key saved without Ctrl, Shift or Alt fires while they are held;
  one saved with modifiers needs exactly those; the more specific key wins (decided by Max, 2026-11-23). A key
  that is already the other key is refused. Injected input counts as real input (as in the spike).
- **Elevated and exclusive fullscreen.** Checked by the runtime when it starts on a window (token elevation; a
  process that cannot be opened counts as elevated) and on its tick (the game window is in front and
  `SHQueryUserNotificationState` says D3D fullscreen). Black frames alone are not used: loading screens are black
  too. The plain lines show in the notice strip. `elevation::restart_as_admin` exists, but no button calls it
  (open question).
- `main.rs` is the only place that builds the real runtime (`Parts::real`: WinRT OCR, WGC, hooks, window checks).
  Raw runtime errors go to `%LOCALAPPDATA%\DialogReader\logs\reading.log`. No test installs a hook, starts the real
  runtime parts in the app, or sends input.
- Arena: `crates/arena/src/readers/a1.rs` gained the option `prompt_zones=off` (prompt zones are not given to
  dr-core), only for the CPU check below. Ideal clock, standard seed 59, a1 default, `prompts=false`, and
  `mode=manual prompts=false`: every scene's events identical before and after (0 of 40 differ in each).

**CPU check: prompts off (2026-11-23, simulated clock, standard seed 59, release; no build of this session ran
meanwhile, other programs may have, so the numbers are soft).** CPU per
frame is what the reader spends on a frame (crops, signatures, OCR). A press is a frame of manual mode that ran OCR
(at least 2 ms; the arena presses every 3 s).

| Run | Score | CPU per frame: mean / p50 / p95 / p99 | Per press: p50 / p95 / max |
|---|---|---|---|
| auto, prompts on (the arena default) | 0.788 (412/523) | 4.40 / 1.22 / 17.1 / 29.6 ms | |
| auto, prompts off (read for the log) | 0.826 (432/523) | 5.03 / 2.34 / 18.9 / 32.4 ms | |
| auto, prompt zones left out | 0.834 (436/523) | 4.05 / 0.72 / 14.4 / 27.5 ms | |
| manual, prompts on | 0.522 | 0.12 / 0.00 / 0.07 ms | 9.3 / 27.4 / 52.6 ms |
| manual, prompts off | 0.518 | 0.13 / 0.00 / 0.07 ms | 10.1 / 32.3 / 57.5 ms |
| manual, prompt zones left out | 0.518 | 0.11 / 0.00 / 0.07 ms | 9.9 / 32.1 / 57.6 ms |

- Auto: reading prompt zones for the log costs about 1 ms per frame on average (5.03 against 4.05 ms), about 2.5 %
  of one core at 25 frames a second. Kept: the log shows the "Prompts off" rows of the mockups.
- Manual: the prompt zones add about 1 ms to a press on average and almost nothing at p95 on this suite. The M3b
  reader reads them after the spoken lines are handed to speech, so they never delay a press.
- These are arena frames (1280 x 720 and similar). A 4K game adds the copy of each frame (about 33 MB at 25 a second);
  Max's manual check measures that.

**Manual check for Max (not run; needs his yes for windows and sound).**
1. Satisfactory, borderless, 4K: start DialogReader, pick Satisfactory (or open its profile). The status chip says
   "Reading".
2. Automatic: talk to a character or trigger a dialogue; the line is spoken with Kokoro, shows in "now speaking" and
   in Recent. A prompt shows in the log as "Prompts off".
3. On key press: switch the mode on Overview, press Mouse 4 in the game: the zones are read, the game keeps focus
   and still gets Mouse 4.
4. Pause: nothing is read; Resume reads again. Read now on Overview reads once.
5. Settings > Keys > Change: press F8; the key shows and is saved. Left click is refused with a line. Esc cancels.
6. CPU: note DialogReader's CPU in Task Manager while reading (auto) and idle in the game, and with Overview open
   (live view attached).
7. An elevated program (any app started with "Run as administrator", picked on page 02): the notice says it runs as
   administrator. Also check whether the read key works in it (spec Risks: `RegisterHotKey` as a fallback).
8. Headphones: plug them in while a line plays; the next line moves to them (M3a checklist).
9. Replay: press replay on a Recent row and Replay in the "now speaking" card. The line is spoken again, and no new
   row shows in Recent or in the activity list.
10. Game switch while a line plays (two profiles): speech stops at once; the new game's Recent is empty, and the old
    game's activity list shows the line once (not again as "Stopped").

**Text changes for Max to approve (M3b)** (old > new):

| Where | Old | New |
|---|---|---|
| title bar status chip, 10 control bar status | Reading (always green) | Reading (green), or Paused, Not running, Stopped, Cannot read (grey) |
| 10 "now speaking" card, nothing spoken yet (new) | none | Nothing read yet. Lines show here as they are spoken. (grey) |
| 04 and Settings key row while waiting for a key (new) | keycap with the key; Change | keycap "Press a key"; Cancel |
| notice, key capture got left or right click (new) | none | Left and right click are for your game. Press another key or mouse button. |
| notice, key capture got the other key (new) | none | That key is already the quick menu key. Press another one. / That key is already the read key. Press another one. |
| notice, Read now while the game is not running (new) | none | (game) is not running. Start it, then press Read now. |
| notice, game runs as administrator (new, spec Risks) | none | This game runs as administrator. DialogReader needs to run as administrator too. |
| notice, exclusive fullscreen (new, spec Risks) | none | This game runs in exclusive fullscreen. Switch it to borderless or windowed in the game's settings. |
| notice, no OCR language (new) | none | Windows has no English text recognition. Add the English language in Windows Settings > Time & language > Language. |
| notice, the key hooks failed (new) | none | The keys could not be set up, so the read key does nothing. Restart DialogReader to try again. |
| activity list skip reasons (new, next to Prompts off and Already read) | none | Waited too long, Voice error, Stopped |
| 10 voice button and read key button (behaviour) | open nothing | open Settings at Voices, and at Keys |

`render-screens` adds `19-keys-capture.png` and `19-overview-not-running.png` for the new states.

**Open questions for Max (M3b).**
- No zone to speak from (every zone switched off, none drawn, or only prompt zones with prompts off): dr-core
  would then read the whole screen aloud (its no-zone mode). The reader does not read automatically in that case
  (`reader::speaks`, tested). Should the app say so on Overview, or should whole-screen reading be an option?
- Elevated games: a "Restart as administrator" button (`elevation::restart_as_admin` exists; needs a mockup).
- The Overview voice and read key buttons open Settings (Voices, Keys); the mockup draws them as dropdowns.
- Switch the current game by itself when another profile's game starts (spec 2.8)? Not built.
- Not built from spec Risks: a hook watchdog (re-install hooks that stop firing), the `RegisterHotKey` fallback
  for elevated games (not checked), and the black-frames check for exclusive fullscreen (left out: loading screens
  are black too).
- After the M3b review fixes (2026-11-23):
  - A replay from a Recent row is spoken next. It no longer throws out the waiting lines, and it does not cut the
    line that plays. Is that right, or should it cut in at once, like Replay in the card?
  - A game switch now stops speech. This also cuts a voice test that plays at that moment.
  - A voice test still clears the game lines that wait in the queue; they now show in the log as "Stopped". Keep
    that, or let the test wait or play over them?
  - "Cannot read" (no OCR language) stays until the game is reopened or the app restarts; there is no retry.
  - The chip says "Reading" also when nothing can be spoken (no spoken zone, or On key press mode).
  - The exclusive fullscreen notice may be a false alarm on DX12 flip-model games where capture still works
    (not checked; step 1 of the manual check).
- An attached live view gets about 12 frames a second (15 before). `Drop for Speech` still waits for one synth call
  at exit, and the screenshot (Continue, Retake) still runs on the UI thread.

## M4: overlays

**Scope.** The pill, the quick menu (with voice list) and zone edit become real overlay windows (spec 2.7):
topmost, non-activating, click-through for the pill. The quick menu key opens the menu centered on the game; the
hook takes arrows, Enter, Space, Esc while it is open. Manage zones opens zone edit over a frozen screenshot. Quit
needs two presses. Overlays follow the game window, its monitor and its DPI.

**Files.**
```
crates/app/src/overlay/mod.rs      create overlay windows, apply Win32 ex-styles through the HWND, place them
crates/app/src/overlay/place.rs    client rect -> screen px -> Slint logical px per monitor scale
crates/app/src/overlay/quick_menu.rs  menu state machine (move, change, select, back, quit twice)
crates/app/src/overlay/zone_edit.rs   add, select, delete, kind, done; saves the profile
crates/app/tests/quick_menu.rs     state machine: order, wrap, stepper limits, quit twice, Esc
crates/app/tests/place.rs          placement at 100%, 150%, 200%, two monitors
crates/app/tests/overlay_win.rs    on a hidden desktop: ex-styles set and kept after hide/show, no activation
                                   (counted per thread, with a plain-window positive control)
```

**Drawing path.** Decided by the overlay spike ([overlay-spike.md](overlay-spike.md)): Slint windows on winit for the
pill, the quick menu and zone edit, one renderer for all windows: femtovg or `skia-opengl`. Skia's default renderer
(not OpenGL) draws the see-through margins opaque black, so it cannot serve the overlays. M4 takes over
`spikes/overlay/src/overlay.rs`:
- `with_winit_window_attributes_hook`: overlays get `with_active(false)` and `with_skip_taskbar(true)` (a thread-local
  flag set around the component's `new()`).
- After every `show()` that gives a new HWND (`overlay::show_overlay`): `SetWindowSubclass`, the extra ex-styles,
  `SetLayeredWindowAttributes(255)` for the pill, `SetWindowPos(HWND_TOPMOST, SWP_NOACTIVATE)`, a window property
  that marks the HWND. Every call is checked and a failure is logged. Overlays use no popups (they would get default
  window attributes).
- The subclass puts the styles back in `WM_STYLECHANGING`, answers `WM_MOUSEACTIVATE` with `MA_NOACTIVATE` and adds
  `SWP_NOACTIVATE` in `WM_WINDOWPOSCHANGING`.
- Zone edit is a frameless window sized to the client rect, never a fullscreen window (winit activates those).
- The keyboard hook runs on the keys thread with its own message loop (section 2 of the spec), never on the UI
  thread. Its callback reads atomic "menu open" / "zone edit open" flags, decides, and posts the work with
  `slint::invoke_from_event_loop` (a slow hook is removed by Windows). A key belongs to whoever got its first
  key-down until it is released: auto-repeats and the key-up go the same way (`keys::Swallow` in the spike).
  Injected keys (`LLKHF_INJECTED`, pad mappers) count as real keys (M4 default, as in the spike; Max may flip it).
- Card margins at least the shadow blur (40 px for the quick menu).
Condition: Max's visible check (`cargo run -p overlay-spike --release -- demo`) shows see-through pill corners,
click-through, the game around the menu card and through the zone edit veil. If the pill fails, it is drawn with
the software renderer into a layered window (`UpdateLayeredWindow`); if the menu or zone edit shows black, they
move to that fallback too (or the menu becomes an opaque rectangle window).

**Tests.** Quick menu and zone edit state machines as pure Rust. Placement math. Overlay window styles and focus on
the hidden desktop (`GetForegroundWindow` is always NULL there, so focus is checked per thread as in the spike).
Screens of every overlay state rendered headless and compared with golden images.

**Done when.** On a borderless game the pill shows, the quick menu opens and works with the keyboard only, zone
edit changes zones that the reader uses at once, the game never loses focus, and all headless tests pass.

### Status (built 2026-11-23, branch feat/app-m4)

The pill, the quick menu (with voice list) and zone edit are real overlay windows on the running game.
`crates/app/README.md`, "In-game overlays (M4)", says how it works.

**Files.**
```
crates/runtime/src/keys.rs        Overlay (none, menu, zones), Nav keys, which keys an open overlay takes, repeats,
                                  the menu only over the game in front; Bindings::after (changes on the hook thread)
crates/runtime/src/runtime.rs     Cmd::Overlay, Cmd::Hold (reading waits, apart from Pause), waker, overlay(), zones()
crates/app/src/overlay/mod.rs     the three windows: show through the recipe, place, hide
crates/app/src/overlay/win.rs     the spike's recipe (attributes hook, harden, subclass), game rect, monitor scale,
                                  bring the main window to the front
crates/app/src/overlay/place.rs   pill, menu centered on the game, zone edit window px <> capture px
crates/app/src/overlay/quick_menu.rs  menu state machine
crates/app/src/overlay/zone_edit.rs   zone edit on the Zones tab's editor (add, pick, remove, dim layer)
crates/app/src/vm/overlays.rs     the view model side: sync with the hook, pill content, menu actions, zone edit
crates/app/src/bin/overlay-probe.rs  hidden-desktop probe (game with a title bar, app code, positive control)
crates/app/ui/overlays/windows.slint PillWindow, QuickMenuWindow, ZoneEditWindow (no popups)
crates/app/tests/quick_menu.rs, place.rs, overlays.rs, overlay_win.rs
```

**Decisions taken in M4 (Max may change them).**
- Injected keys (`LLKHF_INJECTED`: Steam Input, pad mappers) count as real keys, as in the spike.
- The quick menu key opens the menu only while the game window (or a window it owns) is in front, and still goes
  through to the game then (spec 2.6: keys go through). While an overlay is open, the quick menu key is taken: it
  closes the menu, or goes from zone edit back to the menu.
- The menu and zone edit close when the game is no longer in front (Alt+Tab, minimized, closed). The hook stops
  taking their keys at once when the game leaves the front (it checks the foreground window on each key-down), not
  only after the UI's next tick.
- An open overlay never takes a key pressed with Win, Ctrl or Alt held: Ctrl+Esc, Ctrl+Shift+Esc, Alt+Esc,
  Win+arrows, Alt+Enter and Alt+Space go to Windows and the game. Shift alone does not stop a key.
- The pill shows only while the game is in front and being read (Reading or Paused): a topmost pill over another
  app would be in the way. It hides while the menu or zone edit is open (spec 2.7). Its dot is green while
  reading, grey when paused; the speaker of the line being spoken shows when Settings > Show the speaker is on, and
  the wave is lit while a line plays. With no line playing it shows the dot and a grey wave (no text). The demo's
  "pill at top center" while the menu was open was a demo detail: M4 follows mockup 13 (top right, 24 px in) and
  spec 2.7 (hidden while the menu or zone edit is open).
- Zone edit covers the DWM frame bounds, not only the client rect: Windows Graphics Capture of a window is
  assumed to cover the same rect (true for the borderless check; for a game with a title bar only the window
  placement was tested, on the hidden desktop), so a game with a title bar should line up too. Its frozen screenshot also becomes the profile's picture (as Retake). Each change is saved and sent
  to the reader at once.
- Reading is held while an overlay is open (`Cmd::Hold`), apart from the user's Pause.
- The quick menu window is wide enough for the voice list on the right and as wide on the left, so the card stays
  in the middle; a voice list longer than 8 rows scrolls with the highlight. A click outside the card closes the menu.
- Speed steps 0.1 (0.5 to 2.0), Volume steps 5 %. Each step is saved to the profile at once. Mute is for the
  session only.
- Settings in the quick menu closes it and brings the main window to the front on Settings.
- Closing the main window now quits the app (before, it was the last window; the overlays would keep the app
  running with no window, and the tray is M5).
- The renderer: the app ships skia-opengl; `main.rs` selects it in the same `BackendSelector` as the overlay hook.
  If that fails, `renderer.log` says so and the overlays are off. `SLINT_BACKEND=winit-<renderer>` still works;
  with `winit-software` (debug only) the overlays are off, as it draws their see-through margins black.

**Text changes for Max to approve (M4)** (old > new):

| Where | Old | New |
|---|---|---|
| 13 pill, no line playing | dot, "Arthur", lit wave | dot, grey wave (no text) |
| 13 pill, paused | green dot | grey dot |
| 15 quick menu, game name and voice | sample data (Red Dead Redemption 2, Adam) | the current game's name and voice |
| overlay window titles (not shown on screen; Task Manager only) (new) | none | DialogReader status, DialogReader quick menu, DialogReader zone edit |

No other new UI text.

**Tests (all headless).** `tests/overlay_win.rs` (hidden desktop, skia-opengl, a dummy game with a title bar at the
monitor's 225 %): styles kept after hide and show, same HWND, no `WM_ACTIVATE` or `WM_SETFOCUS` (a plain window
activates as the positive control), pill click-through, menu and zone edit take the mouse with `MA_NOACTIVATE`,
GL pixel format, see-through margins (alpha 0) with drawn cards, pill top right 24 px in, menu centered, zone edit
over the frame bounds. `tests/overlays.rs` (testing backend, keys fed as the hook hands them): pill on and off,
menu open, move, Esc, voice list Esc goes back, Speed and Mode saved, Stop is the user's Pause, Quit twice,
Settings, menu only over the game in front, zone edit saves a zone and the runtime reads with it at once, Delete,
Esc drops a box being drawn first, a mouse click on the Volume slider or a Mode button followed by keys (the
menu still shows the new values), and a menu action never reopens a menu the hook closed. `tests/quick_menu.rs`, `tests/place.rs` (100, 150, 200, 225 %, a monitor left of
the main one, a title bar) and the hook rules in `keys.rs` are pure (also: an open overlay takes no key with Win,
Ctrl or Alt held, and none while the game is away).

**Visible check (2026-11-23, run by Claude; Max gave the PC).** Ship build (`target\ship\DialogReader.exe`,
skia-opengl) over a stand-in game: a fullscreen borderless Tk window (3840 x 2160, monitor at 225 %) with a checker
pattern and a dialogue box, which logs keys, clicks and focus. A profile for it in a temp `APPDATA` and
`LOCALAPPDATA`; Max's profiles untouched. Input from `keybd_event` / `mouse_event`. Quick menu key F7. Final clean
run below (earlier runs failed only where the test's click landed on the main window); screenshots in the session
scratchpad `m4-visible`.

| Step | Result |
|---|---|
| 1 Pill on the game | Pass. Top right, 24 px in at 225 %; its corners show the game; ex-style 0x080801B8. |
| 2 Click-through | Pass. A click on the pill reached the game; the game stayed in front. |
| 3 Quick menu key | Pass. Menu centered (1 px off), its margin shows the game, the pill hides. |
| 4 Arrows, Right, Esc | Pass. The menu moved and closed; the game got none of those keys, and got F7 down and up (the quick menu key goes through, spec 2.6). Positive control: after closing, Down reached the game. |
| F1 Shortcuts (review fix) | Pass. With the menu open, Ctrl+Down reached the game and the menu did not move; plain Down was still the menu's. |
| F2 Mouse then keys (review fix) | Pass. A click on the Volume slider (50 %), then Right: the menu showed 55 %; a click on "On key press", then Left: "Automatic" showed as chosen; reopened, it showed both. The game got no click and kept the focus. |
| 4c Voice list | Pass. Right on Voice opens the list (as mockup 15-quick-menu-voice); the first Esc goes back to the menu, the second closes it; the game got no Esc or Right. |
| 5 Manage zones | Pass. Zone edit over the frozen screenshot, the game dimmed (not black); a drag drew a box; picking Dialogue saved the zone (capture px, base 3840 x 2160); the drag did not reach the game. |
| 6 Reader uses the new zone | Pass. The activity log got the line from the new zone at once ("ld nuggets were found near the rive": the text ran past the drawn box, so its ends were outside the zone). The pill's wave lit while it was spoken. |
| 7 Taskbar, Alt+Tab | Pass. All three overlays: WS_EX_TOOLWINDOW, no WS_EX_APPWINDOW, no owner. |
| 8 Focus | Pass. The game had 0 focus losses from step 1 to 7. |
| 9 Settings | Pass. The main window came to the front on Settings; the pill hid (the game is not in front). |
| 10 Quit twice | Pass. The first Enter armed Quit, the second ended the app (exit 0). |
| 11 Logs | Pass. No `renderer.log`, no crash log, no overlay errors in `reading.log`. |

Found on the way: on this PC another program eats the key-down of F8 (its key-up still arrives) also when
DialogReader is not running (checked with the Tk game alone; `sonarad.exe` was running, not confirmed as the cause).
So F7 was used. F8 as a quick menu key would open the menu but the game never sees F8 go down.

Not covered: a real DirectX game (flip model, raw input, pause on focus loss), mixed-DPI monitors, a game with a
title bar on screen (covered on the hidden desktop only), an elevated game. Part of Max's game check.

**Open questions for Max (M4).**
- The pill shows the speaker, not the line (mockup 13 and "Show the speaker"). Should it show the line being spoken
  too (a new design)?
- Injected keys count as real keys. Keep, or ignore them?
- The quick menu key goes through to the game when it opens the menu (spec 2.6). Swallow it instead, so the game
  never sees it?
- Pill and menu only over the game in front; menu and zone edit close on Alt+Tab. Right?
- Closing the main window now quits the app until the tray comes (M5). Right for now?
- Manage zones when the screenshot fails: the menu stays open and `reading.log` says why; nothing shows in the game.
  Add an in-game message (needs text)?
- Speed shows "1x" for 1.0 (the M1 component); the mockup only shows "1.1x". Show "1.0x"?

## M5: settings, tray, start with Windows, installer, version

**Scope.** Settings tab works: Startup (start with Windows, open minimized, show in taskbar), In-game overlay
(position, show speaker), Keys (the read key and the quick menu key with Change, as the two 04-read-key-d
cards; Max: "D everywhere"), Voices, Updates (version, check now). Tray icon with Open,
Pause/Resume, Quit. Single instance. Closing the window keeps the app in the tray. Profile export and import
("Import settings" on the welcome page). Installer with the version in its name. Release follows the `shipping`
skill.

**Files.**
```
crates/app/src/tray.rs             tray-icon on the UI thread
crates/app/src/single.rs           named mutex + hidden listener window: show, quit, end of session, TaskbarCreated
crates/runtime/src/autostart.rs    HKCU Run key, or a scheduled task when run as admin
crates/runtime/src/settings.rs     settings.json load/save
crates/app/app.manifest            per-monitor DPI v2, common controls, asInvoker
installer/                         installer script (per-user install, no admin), version from Cargo
.github/workflows/ci.yml, release.yml   per the shipping skill
CLAUDE.md                          build, test and release block (shipping skill)
```

**Kokoro in the installer (added 2026-11-23, Max: Kokoro is bundled).** The installer ships next to
`DialogReader.exe`: `onnxruntime.dll` (Microsoft's official CPU build, 1.28.2 or newer that has API 24, from the
`onnxruntime-win-x64` zip; with `onnxruntime-LICENSE.txt` and `onnxruntime-ThirdPartyNotices.txt`), the four
Visual C++ DLLs it imports (`msvcp140.dll`, `msvcp140_1.dll`, `vcruntime140.dll`, `vcruntime140_1.dll`, 14.40 or
newer, app-local; Sonara's `packaging/runtime_dlls.py` shows how), and `kokoro\kokoro-v1.0.onnx` plus
`kokoro\voices-v1.0.bin` (from the kokoro-onnx release `model-files-v1.0`, SHA-256 checked at build time, about
354 MB). Plus `THIRD_PARTY_NOTICES` with: Sonara (MIT, the copied Kokoro code), misaki-rs (MIT) and its lexicons
(Apache-2.0), Kokoro-82M (Apache-2.0), kokoro-onnx (MIT), ONNX Runtime (MIT), the VC++ runtime terms (Sonara's
`packaging/notices/models-and-data.md` has the texts). Test: a clean install on a PC without Sonara speaks with
Kokoro, and DialogReader finds the files in its own folder (not the Sonara fallback, which a ship build does not
search).

**Licence gates (added 2026-11-23, apply whether Kokoro is bundled or not).**
- `THIRD_PARTY_NOTICES.md` ships with every `DialogReader.exe` given to anyone, from the first one (the misaki
  lexicons are compiled in). Before that first copy: add each Rust crate's own copyright line (a tool such as
  `cargo about` can list them; Sonara's own notices lack these lines, so its texts cannot simply be copied).
- Before any paid release: the provenance check of the misaki lexicons (some entries came from espeak-ng output,
  GPL-3.0; spec section 6). Without a clean result, a paid release does not ship them.

**Tests.** Settings round trip. Autostart writes and removes the Run value (on a test value name, cleaned up).
Single instance: a second process started by the test exits and the first gets the "show" message (no visible
window: the first instance runs with `--minimized` on a hidden desktop). Version string equals the workspace
version. Installer builds in CI.

**Done when.** Install from the installer on this PC, the version shows in Settings and in `--version`, start with
Windows works after a sign-out, and all headless tests pass. Merge only after Max's yes (global rules).

### Status (built 2026-11-23, branch feat/app-m5)

Built, not merged. Version 0.2.0 (`[workspace.package]` in the root `Cargo.toml`; every crate takes it).

- `crates/runtime/src/settings.rs`: `settings.json` (Startup, In-game overlay). Defaults from spec 2.9: Start with
  Windows off, Open minimized on, Show in the taskbar on (the mockup shows it off), pill top right, speaker on. A
  broken file is kept as `settings.json.bad`, a newer file is not touched.
- `crates/runtime/src/autostart.rs`: the HKCU Run value `DialogReader` = `"<exe>" --minimized`; an elevated
  DialogReader makes a Task Scheduler task with the highest privileges instead. The registry is the truth: the
  toggle shows what Windows will do.
- `crates/app/src/single.rs`: mutex `Local\DialogReader.SingleInstance` and a hidden top-level listener window,
  made right after the mutex. A second start posts "show" and exits (140 ms on this PC). Review fixes (2026-11-23):
  the window is top-level, not message-only, because only top-level windows get Explorer's `TaskbarCreated` (the
  tray icon is made again after Explorer restarts) and the end-of-session messages (`WM_ENDSESSION` quits, so
  sign-out, shutdown and Restart Manager do not have to kill the app); the installer posts its quit message
  (`WM_QUIT_APP`, 32850); a mutex that cannot be opened (access denied: the first runs as administrator) counts
  as a second start, and the window lets show and quit through the UIPI filter. `DIALOGREADER_INSTANCE` renames it
  for tests.
- `crates/app/src/tray.rs`, `ui/tray.slint`: Slint's own `SystemTrayIcon` (the `system-tray` feature was already
  on), not the `tray-icon` crate: no new dependency. Open, Pause/Resume (follows the app), Quit; left click opens.
- `crates/app/src/shell.rs`: close (button or Alt+F4) hides the window while the tray icon is up, and quits when
  there is none. `--minimized` (Start with Windows) starts in the tray when Open minimized is on; a start by hand
  always shows the window. The app runs `run_event_loop_until_quit`.
- `crates/app/src/vm/prefs.rs`: Settings rows saved; Check now opens the releases page; Export and Import of a game
  (one profile JSON, the file dialogs run on their own thread, `crates/app/src/dialogs.rs`). "Import settings" on
  the welcome page imports a game. An imported game gets a new id, so nothing on disk is overwritten.
- `crates/app/app.manifest`, `app.rc`, `app.ico`, `build.rs`: asInvoker, per-monitor DPI v2, common controls v6,
  the icon and a version block (`embed-resource`, needs rc.exe from the Windows SDK).
- `installer/DialogReader.iss` (Inno Setup 6), `installer/build.ps1`: per-user install to
  `%LOCALAPPDATA%\Programs\DialogReader`, no admin; `DialogReader-Setup-x64-<version>.exe` (336 MB). Ships the exe,
  ONNX Runtime 1.28.2 with its licence and notices, the four VC++ DLLs (14.40 or newer), the Kokoro model and voices
  (SHA-256 pinned, checked at build time) and `THIRD_PARTY_NOTICES.md`. `-Fetch` downloads ONNX Runtime and the
  Kokoro files from their official releases (CI); without it the files next to `target\ship\DialogReader.exe`
  are used. The uninstaller removes the Run value and keeps `%APPDATA%\DialogReader`.
- `installer/notices.py`: the Rust crate table of `THIRD_PARTY_NOTICES.md` (each crate linked into the exe, its
  licence and the copyright lines of its licence files). CI runs it with `--check`.
- `.github/workflows/ci.yml`, `release.yml`, `CLAUDE.md` (shipping skill). Neither workflow has run yet.
- Tests: `crates/runtime/tests/{settings,autostart}.rs`, `crates/app/tests/{settings,single,version}.rs`. The
  single-instance test runs DialogReader.exe twice on a hidden desktop with a temp APPDATA and `--no-tray`.
- `speech.log` names the Kokoro files a start uses (the installed app logs its own folder, not Sonara's).

### Text changes for Max to approve (M5)

Old (mockup) > new (app):

1. Settings section list: Startup, In-game overlay, Keys, Voices, Updates > the same plus **Games** (no mockup).
   Its rows: "Export this game" (the game's name under it, button "Export") and "Import a game" ("Adds a game from
   an exported file.", button "Import").
2. Updates: "You have the latest version." > "Check now opens the releases page in your browser." The app cannot
   know whether it is the latest version without an update check.
3. Updates: the row "Install updates automatically / Only when no game is running." > not shown (nothing installs
   updates yet, spec 6 item 2).
4. "Version 0.4.0" > the real version (0.2.0).
5. Tray icon (no mockup): tooltip "DialogReader"; menu "Open", "Pause" / "Resume", "Quit".
6. Welcome "Import settings" keeps its text; it imports one exported game file (spec 6 item 3).
7. New notices: "<game> was exported to <path>.", "This file is not a DialogReader game (<reason>), so nothing was
   imported.", "The file could not be read: <reason>", "The game could not be saved: <reason>", "Start with Windows
   could not be changed: <reason>", "The settings could not be saved: <reason>", and the three settings file
   notices in `settings.rs` (`Problem::text`). The export notice uses the red notice strip, which is meant for
   problems; a quieter style for a success needs a design.
8. File dialogs: titles "Export this game" and "Import a game", file type "DialogReader game (*.json)".
9. Installer (Inno Setup's English texts) plus "Create a desktop shortcut", its group title "Additional
   shortcuts:", and "Start DialogReader".
10. Settings > Games with no game set up: "Set up a game first." (`ui/pages/settings.slint`, no mockup).
11. More new notices (`src/vm/prefs.rs`): "The game could not be exported: <reason>", "Nothing can be saved here."
    (Import with no data folder), and "This file is too big to be a DialogReader game, so nothing was imported."
12. A Startup or In-game overlay change while settings.json cannot be written (from a newer DialogReader, or it
    could not be read or moved aside): before, nothing was said and the change was lost at the next start; now
    "This change is not saved: DialogReader does not write over a settings file it could not use. It lasts until
    DialogReader closes."
13. Start with Windows off, when a DialogReader running as administrator turned it on (a sign-in task): "Start
    with Windows could not be changed: it was turned on by DialogReader running as administrator, so it can only
    be turned off that way (<schtasks error>)".

### Open questions (M5)

1. Installer tool: Inno Setup 6 was used (on this PC already, RCMM uses it). The shipping skill says the tool is
   Max's choice: keep Inno, or NSIS (Glint, Thrasher), WiX, Velopack?
2. Where releases live: the repo is private, so "Check now" opens a page only Max can see, and an in-app update
   check (GitHub API without a token) cannot work. A public releases repo, or another host?
3. Code signing (SignPath or other): releases are unsigned until then.
4. Slint licence: GPL-3.0, Royalty-free 2.0 (needs the AboutSlint widget in an About screen, or the "Made with
   Slint" badge on the download page) or a paid licence. Not chosen.
5. Skia's bundled libraries (FreeType, HarfBuzz, libpng, zlib, ICU and so on) are not listed one by one in the
   notices yet.
6. Show in the taskbar: on by default here (the mockup shows it off).
7. CI is unproven: the capture and hidden-desktop tests may not run on a GitHub runner, and `release.yml` publishes
   a release on every push to main once merged.
8. ONNX Runtime download: on 2026-11-23 `github.com/microsoft/onnxruntime` (repo, API and the 1.28.2 release zip)
   answered 404 from this PC, and NuGet has no 1.28.2 (1.28.0, then 1.29.0). So `build.ps1 -Fetch` (CI) stops at
   that download; the local build uses the copy next to `target\ship` (SHA-256 pinned). Retry later, or pick another
   ONNX Runtime version and source (the DLL pin changes with it). The Kokoro URLs answer. The review (2026-11-23)
   confirmed it: the `installer` job of `ci.yml` and all of `release.yml` fail until this is chosen.
9. A silent update over a running DialogReader now starts it again (with `--minimized`) after the install. A
   normal install still asks with its "Start DialogReader" box. Keep this?

## After v1

- Zone-only frame copies in `dr-core` to cut CPU on 4K.
- Per-speaker voices, OCR languages beyond en-US.
- Media pause while speaking. The old app had it. Max to confirm that v1 may ship without it (spec section 6).
