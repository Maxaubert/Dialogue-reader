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
  Injected keys (`LLKHF_INJECTED`, pad mappers) count as real keys in the spike; Max decides before M4.
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
crates/app/src/single.rs           named mutex + message-only window, "show" from a second start
crates/runtime/src/autostart.rs    HKCU Run key, or a scheduled task when run as admin
crates/runtime/src/settings.rs     settings.json load/save
crates/app/app.manifest            per-monitor DPI v2, common controls, asInvoker
installer/                         installer script (per-user install, no admin), version from Cargo
.github/workflows/ci.yml, release.yml   per the shipping skill
CLAUDE.md                          build, test and release block (shipping skill)
```

**Tests.** Settings round trip. Autostart writes and removes the Run value (on a test value name, cleaned up).
Single instance: a second process started by the test exits and the first gets the "show" message (no visible
window: the first instance runs with `--minimized` on a hidden desktop). Version string equals the workspace
version. Installer builds in CI.

**Done when.** Install from the installer on this PC, the version shows in Settings and in `--version`, start with
Windows works after a sign-out, and all headless tests pass. Merge only after Max's yes (global rules).

## After v1

- Kokoro voices behind the `Tts` trait (needs the ort + Kokoro spike, plan.md spike 3).
- Zone-only frame copies in `dr-core` to cut CPU on 4K.
- Per-speaker voices, OCR languages beyond en-US.
- Media pause while speaking. The old app had it. Max to confirm that v1 may ship without it (spec section 6).
