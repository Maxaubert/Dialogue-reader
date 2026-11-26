# Test arena: spec and plan (2026-11-21)

Goal: Claude can build and verify the reader without Max playing games. The arena plays fake game
scenes with a known answer key, feeds them to a reader, and scores the result. Approved by Max 2026-11-21
("full arena").

## What it measures (priority order)
1. Catching short-lived text (recall, by on-screen duration).
2. Reading accuracy (character error rate, CER).
3. Staying quiet (no reads of static HUD, menus, noise; no double reads).
4. Speed (appear-to-event latency; CPU time per frame).

## Layout
- `crates/arena` (lib + bin `arena`). Rust, same workspace as `spikes/recorder`.
- Local data in `arena-data/` (git-ignored): background frames, generated suites, reports.
- Fonts: read from `C:\Windows\Fonts` (and `arena-data/fonts/` if present). Never committed.

## Core types (`arena::model`, serde JSON)
- `Scenario { id, seed, width, height, fps, duration_ms, background: Background, layout: Layout, items: Vec<TextItem> }`
- `Background`: `Procedural { kind: gradient|noise|moving_shapes|grass_like, seed }` or `Images { dir, fps }` (image sequence, looped).
- `Layout { zones: Vec<Zone> }`, `Zone { name, kind: dialogue|speaker|notification|prompt|hud, rect }`. Simulates the zones a user would draw. Readers may use them.
- `TextItem { id, text, kind, should_read, rect (final position), start_ms, end_ms, style: Style, anim: Anim }`
  - `kind`: dialogue, speaker, pickup_toast, objective, subtitle, tooltip, prompt, hud_static, damage_number, menu.
  - `should_read`: true for dialogue, speaker (paired with its line), pickup_toast, objective, subtitle, tooltip.
    false for hud_static, damage_number, menu. `prompt` is scored in its own bucket (optional reading).
  - `Style { font, size_px, color, outline {color, px}?, shadow {color, dx, dy, blur}?, panel {color, alpha, padding, radius}?, bold, italic }`
  - `Anim { fade_in_ms, fade_out_ms, slide_px (dx,dy) in/out, typewriter_cps?, replace_of: Option<item id> }`.
    `replace_of` models a toast whose number changes in place ("+ 6 Iron Ore (27)" then "+ 12 Iron Ore (33)").
- Answer key = the scenario itself (items with times and rects).
- Recorded scenario: `RecordedScenario { frames_dir, truth: Vec<TextItem> }`, frames named with ms offsets
  (recorder mark format). Used for the real-world set.

## Reader interface (`arena::reader`)
```rust
pub struct FrameRef<'a> { pub t_ms: f64, pub width: u32, pub height: u32, pub rgba: &'a [u8] }
pub struct ReadEvent { pub t_ms: f64, pub text: String, pub rect: Option<Rect>, pub zone: Option<String> }
pub trait Reader {
    fn name(&self) -> &str;
    fn setup(&mut self, layout: &Layout) {}
    fn on_frame(&mut self, f: &FrameRef) -> Vec<ReadEvent>;
    fn finish(&mut self, t_ms: f64) -> Vec<ReadEvent> { vec![] }
}
```
Built-in readers: `oracle` (emits truth, sanity check: must score 100%), `noop` (must score 0 recall, 0 false reads),
`winocr-naive` (WinRT OCR on each zone every 500 ms, reads when text differs from last; a rough model of the old tool).
The real reader will implement the same trait.

## Clocks
- `ideal`: every frame delivered, processing time ignored. Tests logic only.
- `simulated` (default): the runner measures wall time of each `on_frame` call and advances a virtual clock;
  frames that fall inside the busy time are skipped, as a real capture loop would. Event `t_ms` becomes
  frame time + processing time. Deterministic apart from the measured costs.

## Scoring (`arena::score`)
- Match each reader event to the truth item it best fits: text CER <= 0.35 after normalising (case, whitespace,
  punctuation, brackets) and event time within [start_ms, end_ms + 1500]. Truth shorter than 5 normalised chars
  ("Ash", "Kael") must be read exactly. An event that is empty after normalising (only punctuation) matches nothing.
- Metrics: recall on should_read items (overall and by readable-time bucket <1 s, 1-2 s, 2-4 s, >4 s), mean CER of
  matched reads, exact-match rate, duplicates (more than one event per item), noise reads (events matching
  should_read=false items), hallucinations (events matching nothing), prompt recall (separate), latency p50/p95, CPU ms
  per frame p50/p95.
- Readable time (changed 2026-11-21, review fix): from the later of fade-in end and typing end, to the start of the
  fade-out (`TextItem::readable_from_ms` / `readable_ms`). Buckets use it, and latency is event t minus readable-from
  (a read before that counts as 0 ms), so typing time is not charged to the reader.
- Output `report.json` + `report.md`. `--baseline <report.json>` prints deltas and exits non-zero when recall, CER,
  duplicates, noise or hallucinations get worse beyond a tolerance.

## Generator (`arena gen`)
- Seeded. Produces a suite: N scenarios of 10-60 s across genre templates:
  rpg_dialogue (box + speaker name, typewriter), pickup_stack (left toasts, stacking, replace_of counters),
  subtitles (centre bottom), objective_update (top), tooltip (near a moving point), prompt_flicker,
  static_hud (always present), damage_numbers (noise), menu_overlay.
- Durations 0.3 to 6 s, effects drawn from the lists above, fonts from a game-like set available on the PC
  (e.g. Bahnschrift, Segoe UI, Impact, Trebuchet, Georgia, Consolas, plus any in arena-data/fonts).
- Built-in text corpus: item names, dialogue lines, objectives, numbers.
- `arena render --scenario x.json --out dir` writes PNG frames for inspection.

## Commands
- `arena gen --suite smoke|standard|hard --seed N --out arena-data/suites/<name>`
- `arena run --suite <dir> --reader <name> --clock simulated|ideal --report <dir> [--baseline <report.json>]`
- `arena run --recorded <dir>` for the real-world set
- `arena ocr-bench --engine winocr|ppocr --suite <dir>`: single-line crops with truth, CER and ms per crop
- `arena live --scenario x.json --reader <name>`: plays the scene in a real window and captures it with WGC
- `arena realworld-import --recording <recorder session dir>`: builds the recorded scenario from marks + measurements

## Live mode rules
- Must never take the foreground or show on Max's screen. First try a window on a hidden desktop
  (CreateDesktop, like Prism's tools/e2e/HiddenDesktop.cs). If WGC cannot capture there, use an off-screen,
  non-activating tool window (WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW, shown with SW_SHOWNOACTIVATE). Verify the
  foreground window is unchanged before and after; abort if it changed.

### Live mode as built (2026-11-21, `src/live.rs`)
- Command: `arena live [--scenario <json>] [--reader oracle] [--mode auto|hidden-desktop-thread|hidden-desktop|offscreen]
  [--report <dir>] [--lenient-foreground] [--reader-delay-ms N]`. Default scenario `arena-data/suites/smoke/smoke-000.json`;
  default report `arena-data/reports/live-<id>-<reader>/` (`report.json`, `report.md` with a "Live run" section,
  `live.json`, `events/<id>.json`). Use a release build.
- **Hidden desktop does not work with WGC on this PC (Windows 11 build 26300).** Both variants were tried: (a) a child
  `arena` process started on a CreateDesktop desktop (STARTUPINFO.lpDesktop, as in Prism's HiddenDesktop.cs), and
  (b) in process, only the player thread moved there with SetThreadDesktop before it made the window, while capture
  stayed on the default desktop. In both, the window is created, but `IGraphicsCaptureItemInterop::CreateForWindow`
  fails with E_INVALIDARG (0x80070057), so no frame arrives. `--mode auto` still tries both first (well under a second)
  and then falls back; the reasons are kept in `live.json` (`hidden_desktop_note`).
- **What works: an off-screen layered tool window.** `WS_POPUP` with `WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW |
  WS_EX_LAYERED`, placed left of the whole virtual screen (`SM_XVIRTUALSCREEN - width - 200`), shown with
  `SW_SHOWNOACTIVATE`; `WM_MOUSEACTIVATE` answers `MA_NOACTIVATE`. No taskbar button, not in Alt+Tab.
  Pixels go in with `UpdateLayeredWindow` from a DIB section. Plain GDI drawing (`GetDC` + `SetDIBitsToDevice`)
  does **not** work off screen: the drawing is clipped away and WGC sends one black frame and nothing more.
  The player thread is per-monitor DPI aware so the window is exactly the scene size.
- **Frame identity.** The window is 8 rows taller than the scene. Those rows hold the frame index as 19 black/white
  blocks of 16 px (sync, 16 bits, parity). The capture side decodes it, so it knows which scene frame it got and when
  that frame was presented; the strip is cut off before the reader sees the frame. After the run, 6 captured frames
  are compared with a fresh render (`pixel_checks`): the oracle never reads pixels, so this proves capture is correct.
- **Threads.** Player: renders ahead, presents each frame at its due time on the QPC clock, jumps to the due frame when
  late. WGC callback: copies the frame into a one-slot mailbox (an unread frame is overwritten and counted). Reader:
  the main thread builds the reader (no `Send` needed) and feeds it the newest frame. `FrameRef.t_ms` is the scene time
  of the decoded frame; event `t_ms` is wall time since scene t=0 when `on_frame` returned, so the scorer's item latency
  is end to end. `ScenarioRun.clock` is stored as `live` (`Clock::Live`, which `arena run` does not accept).
- **Validity gate (2026-11-21).** After the run the capture is checked, so a broken capture never looks like a bad
  reader. The run fails (after writing the report, with `invalid_reasons` in `live.json`) when: no pixel check, worst
  pixel diff above 2.0, under 90% of scene frames presented, decodable captured frames under 80% of presented, over 5%
  undecodable, or no frame read. `frames_read` is not gated, since a slow reader may skip frames. A failed
  `UpdateLayeredWindow` stops the player with an error. `--child-out` refuses to run on the "Default" desktop, since its
  window sits at (0, 0) and would show on the user's screen if started by hand.
- **Foreground guard (default as the rule says).** `GetForegroundWindow` is polled every 50 ms from before the window
  exists until after it is gone; any change aborts the run. The window procedure also flags `WM_ACTIVATE`,
  `WM_NCACTIVATE` and `WM_SETFOCUS`, which a poll could miss; that flag always aborts. In practice the strict rule
  aborted 6 of 8 early runs because the user was switching between other apps (a browser and Explorer) on this PC.
  `--lenient-foreground` aborts only when the foreground becomes a window of this run (or its hidden-desktop child)
  and logs other switches in `live.json`. Proposal: make lenient the default, since it still catches arena stealing focus.
- **Measured** (smoke-000, 394 frames at 30 fps, 1280x720, release, oracle, auto mode, strict guard): 394/394 frames
  presented, captured and read, 0 late, 0 undecodable, 0 overwritten; pixel diff 0.00; recall 1.0, CER 0.
  Present to WGC callback p50 13.9 / p95 22.6 ms, copy in callback p50 0.7 ms, queue p50 0.2 ms, end to end (present
  to reader done) p50 14.9 / p95 23.8 / max 24.8 ms; scorer item latency p50 41 ms (includes waiting for the next 33 ms
  frame). CPU over 13.6 s: reader thread 2.0% of a core, capture thread 5.9%, whole process 21.8% (mostly the scene
  renderer). With `--reader-delay-ms 100`: 132 of 394 frames read, 262 overwritten, recall still 1.0, end to end p50 127 ms.
- Limits: thread CPU comes from GetThreadTimes, which counts in scheduler ticks (about 15.6 ms), so per-thread CPU is
  coarse (the same oracle run gave 62 to 344 ms reader and 16 to 1062 ms capture); WGC timestamps are the planned
  composition time (often later than the callback), so latency uses QPC at the callback; readers that work on other
  threads (WinRT OCR is async) show their CPU in the process figure only; the player's late-frame skip was never
  triggered (0 late frames in every run); one scene per call.

## Background text
Image backgrounds are real game frames, so they contain real text (Satisfactory: version string, compass
letters, "Objective: Build the HUB", hotbar keys, and in some frames pickup toasts, "Press E" prompts, menus
or the emote wheel). A reader that reads that text is not hallucinating, but it is not reading our items
either. Approach (chosen 2026-11-21):
- **Static sidecar per background folder.** `background_text.json` lists the text regions:
  `{ source_width, source_height, clean_frames?: [first, last], regions: [{ name, rect, frames?: [first, last], texts: [...] }] }`.
  Frame numbers are 0-based indices into the sorted file list. It is read from the image folder first, else
  from the committed copy `crates/arena/bgtext/<folder name>.json` (arena-data is git-ignored). Regions were
  drawn by hand from contact sheets of the frames, not from memory.
- **Scoring.** Truth is matched first. An event that matches no truth item is a `background_read` when, at
  its time or up to 1.5 s before it, a region is shown and either (a) the event has a rect that lies at least
  50% inside the region, or (b) the event has no rect, has 3 or more characters after normalising, and its text is within
  CER 0.35 of one of the region's known strings. Everything else stays a hallucination, so a texture misread as text is never hidden.
  `background_reads` is reported next to hallucinations and shown in the baseline delta (it is still
  unwanted speech), but it does not fail `--baseline`.
- **Generation.** `clean_frames` limits suites to plain gameplay (Satisfactory: frames 286-499; the menus,
  splash, toasts, prompts and emote wheel are outside it). Text zones are placed to avoid every region shown in
  that slice. The corpus has no strings from the background games.
- Why not a second "background only" reader pass: it doubles run time, depends on timing (so baselines flap),
  cannot be re-scored offline, and does not exist for the recorded set.
- Recorded scenarios have no sidecar; real-game text there is part of the truth or a hallucination.

## Implementation notes (stage 1, 2026-11-21)
- Modules: `model`, `rng` (SplitMix64, fixed forever), `fonts`, `render`, `background`, `corpus`, `generator`,
  `text` (normalise, CER), `reader`, `readers/{oracle,noop,winocr}`, `runner`, `score`, `cli`; stubs owned by
  other agents: `live.rs`, `ocr.rs`, `readers/winocr.rs`, `realworld.rs` (each has its own clap `Args` + `run`).
- `TextItem.rect` is the drawn box: the panel when there is one, else the text block. Text starts `padding`
  inside a panel. Extra fields: `Style.align`, `Style.line_spacing`, `Anim.slide_in/slide_out/slide_ms`,
  `Scenario.genres`, `Background::Images { first_frame, count, offset_ms }`.
- Readers are built per scenario by `readers::create(name, &ReaderContext)`; the context carries the layout,
  options (`--reader-opt k=v`) and the truth (only `oracle` may use it). Events from `on_frame` get their
  `t_ms` set by the runner.
- Matching (changed 2026-11-21 after the review: the old greedy pass let a repeated read claim a different, missed
  item). Each event only competes for its best items: those with its lowest CER, and of those only the ones on
  screen at the event time when any is. Events and items are then matched one-to-one inside those sets with a
  maximum matching (events in time order), so two identical toasts on screen together can both be read. A
  leftover event is a further read of its best item: a duplicate, or noise when that item is not to be read.
  CER is on normalised text; empty truth never divides by zero. Exact match means CER 0 after normalising.
- `--baseline` tolerance: recall -0.01, CER +0.01, duplicates, noise and hallucinations + max(2, 10%). Exit code 2.
  The baseline must be made on the same scenes: suite name, seed, scene ids, items to read and the answer key
  fingerprint (`model::truth_fingerprint`, FNV-1a of the truth JSON) must match, else the command fails (exit 1).
  A different reader or reader options is allowed and printed as a note.
- Runs save `events/<id>.json` (with the truth fingerprint and reader options), so `arena score --events <report dir>`
  re-scores without the reader. It refuses events made for another answer key (for example a suite generated again).
- Default report folder: `<suite>-<reader>[-<opt>-<value>...]-<clock>`, so runs with different `--reader-opt` do
  not overwrite each other. `report.json` stores `seed`, `truth_fingerprint` and `reader_options`.
- Readers must work synchronously inside `on_frame` (see the `Reader` trait doc): the simulated clock charges only
  that call. It also hands over the next frame after a busy period, not the newest one already waiting, so it can be
  up to one frame (33 ms) later than a real one-slot mailbox. `winocr-naive` blocks on WinRT OCR in `on_frame`.
- Recorded set: `arena run --recorded <dir>` loads every `*.recorded.json` (`RecordedScenario { id, frames_dir,
  truth, layout, start_uncertain, end_uncertain }`); frames are `f{i:03}_{offset:+06}ms.png` or `.jpg` in
  `frames_dir` (relative to the JSON file). Truth times use the frame time base. The recorder writes offsets from
  the mark key press; `realworld-import` shifts them so t = 0 is the first frame of the mark (`import.json` keeps
  `mark_key_ms`).
- Recorded items whose start or end is not exact (on the first or last frame, or next to a frame gap over 250 ms)
  are listed in `start_uncertain` / `end_uncertain`. They count for recall but sit in their own "time unsure" bucket,
  and unsure starts give no latency.

## Generator rules (review fixes, 2026-11-21)
- Text of one genre never covers text of another genre on screen at the same time (4 px margin). Zones of one genre
  stay out of the zones of other genres and off background text. Placement order: dialogue and subtitles, pickups,
  objectives, tooltips, prompts, HUD, menus, damage numbers. So text to read gets the free places, and a menu or
  damage number that would cover it is dropped. When no candidate place is free, the item (or genre) is skipped.
  On seed 1 no read genre is lost; only menus are (3 standard and 8 hard scenes list `menu_overlay` with no menu).
- Objective and prompt zones are sized for the widest possible text, so every item lies inside its zone.
- Fades take at most a quarter of the item time each. A toast replaced in place does not fade out.
- A typewriter line that the scene end would cut before it is typed out is not started.
- The same speaker name twice in a row must still be read twice. That is a policy choice, kept on purpose: the
  reader cannot know it is the same line of dialogue.

## Real-world set
- The 11 Satisfactory marks (`recordings/satisfactory-20261121-183704`, measurements in `marks_measured.json`)
  become a recorded scenario. Local only (20 GB source); the import writes a small downscaled copy plus truth.
- The control-hint bar of mark 3 ("Dismantle Hold LMB" and so on) is static UI, so its hints are `hud_static`
  (not to be read). Before the review fix they were 5 of the 20 items to read; now 15 items are to read.

## Done when
- `cargo test` covers scorer matching, CER, clocks, generator determinism (same seed, same suite).
- `oracle` scores recall 1.0 / CER 0 / no noise; `noop` scores recall 0 / no false reads.
- `winocr-naive` has a baseline report on smoke and standard suites and on the real-world set.
- `ocr-bench` reports WinRT OCR (and PP-OCR if it builds) per style bucket.
- Live mode runs one scene without touching the foreground.
- Fable review done and fixes applied.

## Build plan
1. Core (one agent): model, renderer, generator, scorer, runner with both clocks, oracle and noop readers, CLI
   with stub subcommands for the parts below, tests.
2. In parallel (separate files): (a) live mode, (b) winocr-naive reader + ocr-bench (+ PP-OCR via ort if feasible),
   (c) real-world import.
3. Fable review, then fixes, then baseline reports committed as `docs/arena-baseline.md`.
