# dr-app: the DialogReader window

The Slint UI of DialogReader. It builds `DialogReader.exe`.

**Status: M4.** Every approved screen from `design/mockups/c-midnight` is built in Slint. Setup is real: profiles
are saved as JSON in `%APPDATA%\DialogReader\profiles`, page 02 lists the running windows (games first), Continue
takes one screenshot with Windows Graphics Capture, and pages 03 and 11 draw, select, move, resize, rename,
re-kind, switch and delete zones on it (Ctrl+Z undoes the last change). The stages of 03, 10 and 11 show the
game live while it runs (see "Live view"). Onboarding runs only when there is no profile yet. Since M3a, page 05
and Settings > Voices list the Windows voices; "Play sample" and "Test" speak on the speakers (only in the app,
never in a test); voice, speed and volume are saved in the profile. Games that share an exe get one profile each,
told apart by the window title. Since M3b the app reads the current game: it finds its window, captures it,
reads it automatically or on the read key, speaks the lines, and shows the status, the line being spoken and Recent
from the activity log (`log\<id>.jsonl`). Pause, Read now, Replay, Stop and key capture ("Change") work. Since
M4 the in-game overlays are real windows over the game (see "In-game overlays"): the status pill, the quick menu on
the quick menu key, and zone edit over a frozen screenshot. The plan
is in [docs/app-plan.md](../../docs/app-plan.md) and the design in [docs/app-spec.md](../../docs/app-spec.md).

## Run, test, render

| What | Command |
|---|---|
| Build (Skia, the default) | `cargo build -p dr-app` |
| Shipped exe (Skia, fat LTO, `panic = "abort"`) | `cargo build --profile ship -p dr-app --bin DialogReader` (writes `target\ship\DialogReader.exe`) |
| Build with the femtovg fallback | `cargo build --release -p dr-app --bin DialogReader --no-default-features --features renderer-femtovg` |
| Tests (headless) | `cargo test -p dr-app` |
| Mockup comparison numbers | `cargo test -p dr-app --test screens -- --nocapture screens_match` |
| Render every screen to PNG | `cargo run -p dr-app --bin render-screens -- [out-dir]` |
| Open the app (shows a window) | `cargo run -p dr-app --bin DialogReader` (real profiles) or `-- --fake` (mockup data) |

- `render-screens` draws each screen with Slint's software renderer at 1440 x 900 into a buffer and saves it as
  PNG. It opens no window. The default out-dir is `target/screens`. File names match the mockup shots
  (`01-welcome.png` to `16-zone-edit.png`), plus `12-settings-keys.png` (Settings scrolled to the keys) and
  `17-activity.png` (the list behind "Show all").
- The app options are for checks by hand: `--page <welcome|pick-window|draw-zones|keys|voice|mode|overview|zones|settings|activity>`
  opens a page, `--preview-overlay <pill|quick-menu|quick-menu-voice|zone-edit>` shows an in-game overlay inside
  the main window, `--fake` shows the mockup data instead of real profiles. `--version` prints the version.
- Never run the app window from a test. Tests use the Slint testing backend (`tests/nav.rs`, `tests/zones.rs`,
  `tests/setup_flow.rs`) or the software renderer (`tests/screens.rs`), so nothing shows on screen. Tests with real
  data use a temp folder (`Paths::at`) and a fake `Desktop`, never `%APPDATA%` or the real windows.
- A panic writes `%LOCALAPPDATA%\DialogReader\logs\crash-<unix seconds>-<pid>.log` (`src/crash.rs`). In the ship
  profile the app then ends at once (`panic = "abort"`). A window that cannot open (for example OpenGL fails) writes
  `start-error.log` in the same folder; `renderer.log` says when skia-opengl could not be selected.
- `--version` attaches to the console of the terminal that started it, so it prints there although the exe is a
  windows-subsystem app. Without `APPDATA` the app shows the sample data with a notice that nothing can be saved.
- `tests/screens.rs` also compares every render with its mockup shot in `design/mockups/c-midnight/shots/`. Both
  pictures are averaged over 10 x 10 px cells; the mean difference and the share of cells that move by more than
  32 must stay under a limit per screen. Screens that differ by design compare only a region (14: the switcher
  panel; 06 and 12: above the changed rows). It catches moved, missing or recolored blocks. It does not catch text
  weight or a 1 to 2 px shift, so the side-by-side review stays.

## Layout

```
build.rs                 compiles ui/app.slint (with debug info outside release, for the testing backend)
src/main.rs              DialogReader.exe: renderer (skia-opengl), crash hook, real or fake source, options
src/lib.rs               build_window(source) and build_main_window() (fake): window + view model + window buttons
src/vm/mod.rs            view-model layer: profiles, windows, screenshots, Slint models, callbacks; Desktop trait
src/vm/zones.rs          zone drawing state: draw, select, move, resize, rename, kind, on/off, delete, undo
src/vm/voices.rs         voices for the UI: the installed list, labels ("Windows · David"), the voice test thread
src/vm/live.rs           live view of the game on the stages of 03, 10 and 11: start, stop, frames, fallback
src/vm/reading.rs        reading: the runtime, status, now speaking, activity log, window search, key capture
src/vm/overlays.rs       the in-game overlays in the view model: which one shows, the pill, quick menu actions, zone edit
src/overlay/mod.rs       the three overlay windows: show (through the overlay recipe), place, hide
src/overlay/win.rs       Win32: attributes hook, harden + subclass (docs/overlay-spike.md), game rect, monitor scale
src/overlay/place.rs     placement math: pill, menu centered on the game, zone edit window px to capture px
src/overlay/quick_menu.rs  quick menu state machine (keys and clicks to actions; Quit twice; voice list)
src/overlay/zone_edit.rs zone edit on the Zones tab's editor: view zones, dim layer, pick, add, remove
src/crash.rs             panic hook: crash log in %LOCALAPPDATA%\DialogReader\logs
src/fake.rs              fake data of the mockups (behind vm::Source::Fake) and the zone edit callbacks
src/geom.rs              stage mapping, drag to zone, hit tests, dim path (unit-tested)
src/screens.rs           headless renderer and the screen list (sets Render.software)
src/bin/render-screens.rs
src/bin/overlay-probe.rs the overlay windows on a hidden desktop with a dummy game (tests/overlay_win.rs runs it)
ui/theme.slint           tokens: Palette, Space, Radius, Size, Type, Shadow; Render; Kinds, Icons, Art
ui/data.slint            view-model structs, Page and OverlayPreview enums
ui/app.slint             MainWindow: pages, navigation, keys (Ctrl K, Esc, arrows), game switcher, overlay previews
ui/components/           base (Button, Kbd, Toggle, Segmented, Slider, Card, ...), chrome (bars, steps, tabs), stage
ui/pages/                welcome, pick_window, draw_zones, keys, voice, mode, overview, zones, settings, activity
ui/overlays/             pill, game_switcher, quick_menu (menu + voice flyout), quick_menu_preview (review only:
                         menu over the game and its two other states), zone_edit, windows (the M4 overlay windows:
                         PillWindow, QuickMenuWindow, ZoneEditWindow; no popups)
ui/fonts/                Geist 1.800 and Geist Mono 1.701 variable fonts (SIL OFL, see OFL.txt)
ui/icons/, ui/art/       written by tools/export_icons.py from design/mockups/c-midnight/_build.py
tests/                   nav.rs, zones.rs, zone_cancel.rs, live_view.rs, setup_flow.rs, voices.rs, reading.rs,
                         overlays.rs (testing backend), quick_menu.rs, place.rs (pure), overlay_win.rs (hidden
                         desktop), screens.rs (renders, compares with the mockups), common/ (fake desktop with a
                         fake live view, fake voices, temp data folder, pointer input)
examples/snapshot.rs     one page in a real window with a chosen renderer, saved as PNG (run on a hidden desktop)
tools/hidden-run.ps1     runs a program on a hidden Windows desktop and waits for it
```

## Design rules in code

- All colors, sizes, shadows, letter spacing, line heights and font weights come from `ui/theme.slint`. No page
  writes a color by hand. Colors with alpha are 8-digit hex. In `@markdown` text, write `\{Palette.text-2}`.
- Icons are white SVG files drawn with `colorize`. `<name>.svg` is drawn for 16 px, `-sm` for 12 px, `-lg` for 22 px,
  so the line is as thin as in the mockups. Kind icons are always grey.
- Zone kinds carry no color anywhere (Max, 2026-11-22): every `Stage` draws zones and tags in grey (`text-2`,
  `zone-fill`), and the selected zone's outline and handles in the accent. The Kind picker shows the grey kind
  icons; the 06 summary dots are `text-3`. There are no kind color tokens.
- The stage shows the fake 16:9 art with "cover" and a real screenshot whole with "contain" (`real-shots`), so a
  window that is not 16:9 is never cropped. Zones are in screenshot pixels; the stage maps them.
- After a change to the mockup icons or art, run `python -I crates/app/tools/export_icons.py`.
- Text heights follow the CSS line boxes (`Type.lh1` to `lh3`), so blocks stack as in the mockups. Multi-line
  text uses `line-height-factor` (`Type.lhf-115` to `lhf-150`; Geist's natural line height is 1.3 em).
- The software renderer does not clip to rounded corners and draws no shadows. The real window clips pictures
  with `clip: true` and a border radius. `CornerMask` paints the corners with the color behind them, only when
  `Render.software` is true (set by `src/screens.rs`), so it never shows a wrong color in the real window.
  Shadows show only in the real window.
- Software renders of the full-window overlay previews (13, 15, 16) use `ui/art/scene-1440.png`: the scene drawn
  by headless Edge at 1440 x 900, as in the mockups, so no scaling. The real window keeps the SVG.

## Fonts and text weight (measured 2026-11-22)

- The fonts are the variable Geist and Geist Mono from Google Fonts (github.com/google/fonts, `ofl/geist`,
  `ofl/geistmono`), the same builds the mockup pages load. Weights are the tokens `Type.w-regular` (400),
  `w-medium` (500) and `w-semibold` (600).
- The renderers apply the weight axis: the software renderer passes the variation to swash, and femtovg (the
  fallback; Slint builds it with only its `swash` feature) builds glyphs with swash using the same values and
  keys its glyph cache by them. The real window draws with Skia since M2 (see "Renderer").
- The M1 review said bold text draws as regular. That did not reproduce. With the earlier static TTFs the brand
  word "DialogReader" had 415.6 / 434.5 / 447.7 units of ink at 400 / 500 / 600, so each weight draws differently.
  The mockup shot has 517.4 at 600.
- The gap is the rasterizer, not the weight. Chromium draws with ClearType and extra contrast; Slint draws
  grayscale coverage. Regular text has the same gap: the "now speaking" paragraph has 1762.6 units of ink in the
  shot and 1362.7 in Slint (+29%). Raising the weight does not close it: at 700 the brand word reaches only 474.6.
  The Skia renderer closes this gap; see "Renderer" below.

## Renderer (Skia, decided 2026-11-22)

Max chose Skia with `panic = "abort"` in a ship-only profile (docs/app-spec.md, section 6, item 8). The default
feature `renderer-skia` turns on Slint's `renderer-skia-opengl`, and `main.rs` selects the `skia-opengl` renderer
by name: the M4 overlays need OpenGL Skia, because Skia's default surface draws their see-through margins black
(docs/overlay-spike.md). femtovg is the opt-in fallback `renderer-femtovg`. The software renderer is always built
in: render-screens and the tests use it, and `SLINT_BACKEND=winit-software` makes the window use it when OpenGL
fails (debug only: the in-game overlays are off then, as it draws their see-through margins black). The measurements below are from the spike, when femtovg was still the default.

**Text ink.** `examples/snapshot.rs` opens the window on a hidden desktop (`tools/hidden-run.ps1`) on the overview
page (10-main-b) and saves `Window::take_snapshot()`. For Skia that is a CPU re-render of the scene, not a
read-back of the window. Its last argument is the scale factor (it sets `SLINT_SCALE_FACTOR`); the default is 1,
which gives 1440 x 900. Ink is the sum of (luminance minus the most common luminance) / 255 over a box around each
label. At 2.25 the boxes are 2.25 times larger, and the mockup shot is a headless Edge shot of 10-main-b.html with
`--force-device-scale-factor=2.25` (not in `design/`). The 1x shot taken the same way gives exactly the numbers of
`shots/10-main-b.png`. The crops differ from the section above (the brand word is 431.3 in the shot here, 517.4
there), so compare numbers only inside this table.

| Label (weight), scale 1 (100%) | Mockup shot | Software (render-screens) | femtovg | Skia |
|---|---|---|---|---|
| brand "DialogReader" (600) | 431.3 | 353.5 | 355.5 | 432.9 |
| tab "Overview" (600) | 240.7 | 186.2 | 187.5 | 242.1 |
| button "Read now" (500) | 236.2 | 188.1 | 189.2 | 237.7 |
| "Recent" title (600) | 208.4 | 169.1 | 170.2 | 207.3 |
| zone "Speaker name" (500) | 361.3 | 283.2 | 285.2 | 363.2 |
| now-speaking paragraph (400 / 500) | 1763.4 | 1383.1 | 1393.1 | 1795.0 |
| recent row "Arthur: We need..." (400 / 500) | 455.0 | 354.0 | 356.9 | 458.6 |
| **total at scale 1, against the shot** | **100%** | **79%** | **79%** | **101%** |
| **total at scale 2.25 (this PC, 225%)** | **16654.4 (100%)** | **14574.5 (88%)** | **14867.2 (89%)** | **16652.1 (100%)** |

Skia matches the Chromium shots at both scales, at every weight. The gap it closes depends on the scale: at 100%
femtovg has 21% less ink than the shot, at 225% only 11% less. So on Max's 225% screen Skia has 12% more ink than
femtovg, against 27% at 100%.

**Size of `DialogReader.exe`** (release; MB = 10^6 bytes, so 29.2 MB shows as 27.9 MB in Explorer; zip = deflate
level 9, xz = level 9, as a guide to the download size):

| Build | femtovg + software | Skia + software |
|---|---|---|
| repo release profile | 19.1 MB (zip 8.0, xz 5.9) | 29.2 MB (zip 12.2, xz 9.0) |
| + `lto = "fat"`, `codegen-units = 1` | 17.8 MB | 27.2 MB |
| + also `panic = "abort"` | not measured | 22.1 MB |

The goal was under about 25 MB. Skia reaches it only with `panic = "abort"`. That does not have to change the
release profile: a named profile in the workspace `Cargo.toml` applies only to builds that ask for it.

```toml
[profile.ship]
inherits = "release"
lto = "fat"
codegen-units = 1
panic = "abort"
```

The workspace `Cargo.toml` now has this profile. `cargo build --profile ship -p dr-app --bin DialogReader` writes
`target\ship\DialogReader.exe` (spike, Skia default surface: 22,085,632 bytes, 2 min 49 s; with `skia-opengl` and
M2, measured 2026-11-22: 22,675,456 bytes, 2 min 36 s; it ran 8 s on a hidden desktop with no crash; with M3a,
measured 2026-11-23: 23,644,160 bytes (22.5 MiB, 0.9 MiB more than M2), 3 min 35 s, built only, not run. M3a
adds speech (dr-tts, rodio, cpal); cpal brings a second `windows` crate, 0.54.0, next to 0.62.2). Tests, the arena and normal release builds keep unwinding. The cost is in the
shipped exe only: a panic in any thread ends the app at once, with no `Drop` cleanup. The panic hook in
`src/crash.rs` writes the crash log first.

**Build.** `skia-bindings` downloads a prebuilt Skia (about 10 MB,
`skia-binaries-...-x86_64-pc-windows-msvc-d3d-ganesh-gl-jpegd-jpege-pdf.tar.gz`) from GitHub at build time, so the
first build needs the internet (or `SKIA_BINARIES_URL` pointing at a local copy). It unpacks to about 82 MB in
`target`. With the other crates already built, the release build that added Skia took 1 min 19 s (download
included); a release rebuild of dr-app alone takes about 51 s. The fat LTO build took 4 min 20 s. No build errors.
Skia's window surface on Windows is wgpu (GPU), with a CPU surface (softbuffer) as its fallback.

## Zone drags and the other mouse buttons (fixed 2026-11-22)

Max found that a right click while drawing a zone froze the box, and the left release then put the zone where
the cursor was. Cause: a Slint `TouchArea` ends its mouse grab on the release of ANY button. After that its
`moved` callback no longer fires, but `pressed` stays true (only a left release clears it), so the stage stopped
following the mouse. The left release still reached the stage, and `Editor::pointer_up` moved the zone to the
release point first. Now:

- `ui/components/stage.slint` tracks the left drag itself and takes moves from `pointer-event` (it fires with or
  without the grab). A right press during a drag sends a cancel (kind 3); so does a lost pointer (`cancel`, or
  the mouse leaving the stage after the grab is gone).
- Cancel (`Editor::cancel_drag`) puts a moved or resized zone back and drops a zone being drawn. It is no change:
  no save, no undo step. Esc does the same while a drag is on (`zone-cancel`); without a drag Esc is not used.
- In the real window, `keep_left_drags` (src/lib.rs) keeps the middle, back and forward buttons away from Slint
  while the left button is down, so they never end or freeze a drag. The right button still goes through.
- When the app window loses the focus (Alt+Tab, a UAC prompt), the left release may never come: `focus_lost`
  (src/lib.rs) sends the stage a pointer exit and cancels the drag. A press that finds a drag still open (a
  release lost some other way) cancels that drag first, so a zone is never left at a stray place.
- Tests: `tests/zone_cancel.rs` (right click during draw, move and resize, Esc, middle click, lost grab, focus
  loss) and `Editor` unit tests.

## Live view (added 2026-11-22)

Max asked for a live view of the game in the zone editor. The stages of 03, 10 (Overview) and 11 (Zones) now
show the game live while it runs; "Retake screenshot" still updates the saved still.

- **Capture** (`dr_runtime::live`): one Windows Graphics Capture session on the game window (the window picked
  on 02, or a running window of the current profile), frames at most every 66 ms (about 15 fps:
  `MinimumUpdateIntervalSettings` plus a time check before the readback). On the capture thread the GPU makes
  the frame exactly the stage size in physical pixels (never larger than the capture): a mip chain, then one
  draw with a trilinear sampler into a texture of that size, which is the only thing read back. So the cost
  does not depend on how big the stage is next to the game. If any D3D call fails (or the shader compiler,
  `d3dcompiler_47.dll`, is missing), it reads the full frame and box-filters it on the CPU instead.
- **Start and stop never wait on the UI thread.** Windows took 39 to 271 ms to build a capture session (the
  review of 2026-11-22 measured the old start on the calling thread), so
  `Live::start` does that on a helper thread and returns at once; a start Windows refuses comes back as the end
  of the session. The game's window is searched on another thread too (`Desktop::windows_quick_then`: the window
  list takes about 9 ms). `stop` waits at most for a frame being handed over (about 1 to 3 ms); the capture
  thread is stopped on a helper thread.
- **To the UI** (`src/vm/live.rs`): the sink copies the frame into a `SharedPixelBuffer`, keeps only the newest
  one in a slot, and wakes the UI thread once with `invoke_from_event_loop` until the UI has taken it. The UI
  thread only wraps it in an `Image`. A session number drops frames of a stopped session.
- **When**: only while 03 (with a picked window), 10 or 11 shows and the app window is not minimized or hidden. A
  page change checks at once (`page-shown`); a 500 ms UI timer checks the rest. Other pages stop it.
- **Overview needs the focus** (Max, 2026-11-23): on 10 the live view runs only while the DialogReader window is
  in front. When the app loses the focus (Max is in the game), the session stops and the last frame stays on the
  stage, with its note. When the app gets the focus back, it starts again from that frame. 03 and 11 keep running
  without the focus. The winit `Focused` event calls `focus_lost` / `focus_gained` (`src/lib.rs`).
- **Zones** stay in capture pixels. Stages map them with the live capture size (`LiveView.w` and `h`). When the
  game changes size, the editor rescales the zones (after a drag ends); edits then save with the new `base`. When
  the live view stops, the zones go back onto the saved screenshot's size. A size change alone is not saved.
- **Fallback**: a minimized game (no frames for 1.5 s and `IsIconic`), a closed one, or a window Windows will not
  capture shows the saved screenshot with a small note on the stage ("Saved screenshot · the game is
  minimized", "... is closed", "... no live picture from this window"). An end of the session says "closed"
  only when the window is really gone; otherwise it is "no live picture". A live stage shows "Live" with a green
  dot. The last frame is let go when the saved screenshot shows again.
- **Where the note shows** (Max, 2026-11-23): never on the game picture (it covered the top right corner, where a
  zone may be). On 11 it is part of the info line under the stage ("Live · W × H · Fit to window" with a green dot,
  or "Saved screenshot · the game is minimized · W × H ..."). On 03 and 10 it is a small line under the stage (on 10
  under the zone strip: a long note in the strip pushed the zone toggles out). `LiveNote` in
  `ui/components/stage.slint`.
- **Size change during a drag**: the frame waits until the drag ends (release, right click, Esc), then shows
  without a new frame (a paused game sends none).
  A game that does not change sends no frames; that is not an error, the last frame stays. The line under the
  stage on page 11 says "Live · W × H" or "Screenshot · W × H".
- **Tests**: `crates/runtime/tests/live.rs` captures a test window left of every monitor that repaints every
  20 ms: frames arrive about 66 ms apart at the asked size, a new target takes effect, nothing arrives after
  `stop`, a resized window gives its new size, a closed window ends the session, the start returns at once and
  a stop during the start ends it. It also compares the GPU scale with the CPU box filter on a static picture
  of four colored quarters at a ratio of 0.65 (same size, same corners, small mean difference).
  `tests/live_view.rs` (fake desktop) covers start and stop on page changes, the hidden app, the window search
  off the UI thread, zones on a game that changed size, the size change during a drag, and each fallback note.

**Cost** (measured 2026-11-22 on this PC: Ryzen 9 9950X3D, 32 threads, RTX 5090; release build;
`cargo test --release -p dr-runtime --test live -- --ignored --nocapture live_cost_4k`, about 2 minutes). Source:
an off-screen window that repaints every 30 ms, so a frame comes every 66 ms (14.4 fps in every row). CPU is
this process minus the test window's own painting thread, in percent of ONE core (one core is about 3 % of the
whole CPU here). Each row ran 5 s, and Windows counts CPU time in 15.6 ms steps, so a row is good to about
0.3 points (the three rows at exactly 3.1 % are the same step). Another build may have run on the PC at the same
time, so single numbers can be off by a point or two.

The GPU path is the normal one. The stage sizes are those of page 03 (`DrawZones`), page 10 (Overview, the start
page) and page 11 (Zones) at 225 % and 150 %.

| Game | Stage (physical px) | GPU path: per frame readback / copy / hand-over | GPU path CPU | CPU path: box filter per frame | CPU path CPU |
|---|---|---|---|---|---|
| 3840 x 2160 | 1962 x 1102 (page 03, 225 %) | 0.4 / 0.7 / 1.2 ms | 3.1 % | 16.8 ms | 25.9 % |
| 3840 x 2160 | 2516 x 1415 (Overview, 225 %) | 0.5 / 1.0 / 2.0 ms | 5.6 % | 22.4 ms | 36.6 % |
| 3840 x 2160 | 2286 x 1286 (Zones, 1440 x 900 window, 225 %) | 0.5 / 0.9 / 1.6 ms | 5.0 % | 20.1 ms | 33.4 % |
| 3840 x 2160 | 2880 x 1620 (Zones, maximized, 225 %) | 0.6 / 1.3 / 2.6 ms | 7.8 % | 21.7 ms | 36.6 % |
| 3840 x 2160 | 1677 x 944 (Overview, 150 %) | 0.4 / 0.6 / 0.9 ms | 3.1 % | 16.1 ms | 26.2 % |
| 3840 x 2160 | 872 x 490 (page 03, 100 %) | 0.3 / 0.2 / 0.2 ms | 3.1 % | 5.0 ms | 10.0 % |
| 2560 x 1440 | 1962 x 1102 | 0.4 / 0.6 / 1.2 ms | 4.1 % | 10.2 ms | 17.5 % |
| 2560 x 1440 | 2516 x 1415 | 0.5 / 1.0 / 2.0 ms | 7.2 % | 14.7 ms | 24.7 % |

- The readback on the GPU path is the time to queue the scale and map the small texture; the GPU work itself is
  not CPU time. "Copy" is the copy out of the mapped texture. The hand-over in the test checks every pixel; in
  the app it is one copy into the `SharedPixelBuffer`.
- The CPU path runs only when D3D fails. The same command measures it (`Live::start_with(.., cpu_only: true,
  ..)`).
- On the calling thread: `Live::start` took 0.02 to 0.15 ms and `stop` 0.04 to 0.10 ms in every row.
- The window search the app runs every 3 s while a stage page shows and the game is closed (window list without
  icons): median 9.1 ms, now on its own thread.
- Not measured: the UI thread's texture upload of each frame (about 14 MB at 2516 x 1415) in the real Skia
  window.
- Overview is the start page, and the live view runs whenever a stage page shows and the app is not minimized
  or hidden, also when the app is behind the game. Whether it should run only while the app is in front is
  open (Max's call).

## Voices (M3a)

- `main.rs` is the only place that builds `Voices::windows()`: the Windows voices (`dr_tts::WinTts`) and a speech
  thread with the speakers (`dr_runtime::speech::RodioOut`, opened at the first sound). Tests pass fake voices
  (`dr_tts::FakeTts`) and a `NullSink`; render-screens uses the fake source. No test plays sound.
- A saved voice that is not installed (or an empty id) uses the first installed voice, and the screens show it.
- With no Windows voice installed, page 05 says so, "Play sample" and the Settings "Test" are off, page 06 says
  nothing is spoken, and Finish saves an empty voice id. "Add voices" opens `ms-settings:speech` through the
  `Desktop` trait (a test counts the call).
- The Settings Speed and Volume sliders reach the speech thread on every move and save the current profile when
  let go (or on a page change). Leaving page 05 or Settings stops a sample that plays.
- A failed voice test shows a plain line; the raw error goes to `%LOCALAPPDATA%\DialogReader\logs\speech.log`
  (only the app binary has a log path, so tests write nothing there).
- `cargo test -p dr-tts` shows "3 ignored" on a PC with no Windows voice. On a PC with one, run
  `cargo test -p dr-tts --test voices -- --ignored`.

## Reading (M3b)

- `main.rs` builds the runtime (`dr_runtime::runtime::Parts::real`) and hands it to the view model
  (`vm::attach_runtime`). Tests pass a runtime on fake parts (`tests/reading.rs`) or none; no test installs a key
  hook or captures a game.
- A 100 ms UI timer (`Vm::reading_tick`) runs the runtime's tick, its events and the speech events, and looks for
  the current game's window once a second while it is not read.
- The live view attaches to the reader's capture session when both want the same window (one WGC session).
- Raw errors (capture, OCR, hooks) go to `%LOCALAPPDATA%\DialogReader\logs\reading.log`.

## In-game overlays (M4)

- `main.rs` selects the winit backend with the overlay attributes hook (`overlay::win::attributes_hook`) and the
  OpenGL renderer the build has (skia-opengl; femtovg with the fallback feature), then makes the three windows
  (`overlay::Overlays::new(true)`) and hands them to the view model (`vm::attach_overlays`). If the backend cannot
  be selected, `renderer.log` says so and the overlays are off (the quick menu key then does nothing).
- Every show goes through the spike's recipe (`overlay::win::show_overlay`): extra styles, `SetLayeredWindowAttributes`
  for the pill, topmost without activation, a property that marks the HWND, and a subclass that keeps the styles.
- The key hook keeps which overlay is open (`dr_runtime::keys::Overlay`) and changes it at once for the quick menu
  key and Esc; `Vm::overlay_sync` makes the windows match on every key event (the runtime's waker posts to the UI
  thread) and on the 100 ms reading timer. While the menu is open the hook takes the arrows, Enter, Space, Esc and
  the quick menu key; in zone edit it takes Delete, Backspace, Esc and the quick menu key (back to the menu).
- The menu and zone edit open only over the game in front, and close when it is not in front any more. Reading is
  held while one is open (not the user's Pause). The pill shows while the game is read (Reading or Paused) and is
  in front, at Settings > In-game overlay > Position (kept for this session; saving it is M5).
- Zone edit covers what the capture covers (the DWM frame bounds; the title bar too, for a game with one), shows a
  frozen screenshot (also saved as the profile's picture, as Retake does) and edits the Zones tab's zones: every
  change is saved and sent to the reader at once.
- Closing the main window quits the app (the overlays would otherwise keep it running with no window; the tray is
  M5).
- Tests: `tests/overlay_win.rs` runs `overlay-probe` on a hidden desktop (styles kept after hide and show, no
  activation with a positive control, click-through, `MA_NOACTIVATE`, GL pixel format, see-through margins with
  skia-opengl, placement over a game with a title bar). `tests/overlays.rs` runs the view model on the testing
  backend with keys fed as the hook hands them. `tests/quick_menu.rs` and `tests/place.rs` are pure.

## Known gaps (M4)

- Import settings and the other Settings buttons (Startup, overlay, updates) do nothing yet (M5). The overlay
  Position and Show the speaker work for the running session only.
- The reader follows the game picked in the app; it does not switch by itself when another profile's game starts.
- No button restarts DialogReader as administrator for an elevated game (the notice says why; open question).
- The overlays were checked on a stand-in game (a Tk window), not a DirectX game (flip model, raw input, games that
  pause on focus loss): that is part of Max's game check.
- The zone strip on Overview shows every zone in one row; many zones run past the edge (no "+N" chip with real
  data yet).
- Page 02 shows the exe icon for real windows, not a live picture of the window.
- A game in exclusive fullscreen gives a black or missing live view, as it does for the screenshot (spec
  section 5).
