# App spec: the DialogReader Windows app

Written 2026-11-22. This spec covers the real Windows app around the `dr-core` engine: windows, overlays,
capture, OCR, speech, keys, profiles and install. The build order is in [app-plan.md](app-plan.md).

Inputs: [PRODUCT.md](../PRODUCT.md), [plan.md](plan.md), [reader-a1.md](reader-a1.md),
[design/README.md](../design/README.md), the approved mockups in `design/mockups/c-midnight` and their tokens in
`design/mockups/c-midnight/tokens.css`.

## 1. Goals and limits

- Setup in minutes in the main window. Then the main window is closed and the game has the screen.
- During play, DialogReader never takes focus from the game. It shows only a small status pill, the quick menu
  (on its key) and the zone editor (from the quick menu).
- Two global keys only: the **read key** (default shown as Mouse 4) and the **quick menu key** (default shown as
  F7). The user picks both. No other global hotkeys (design/README.md, rules of 2026-11-22).
- Works on borderless and windowed games. Exclusive fullscreen is not supported in v1 (see Risks).
- Everything runs on the PC. Nothing is sent online.
- Every test and every check runs headless. No test shows a window or takes the foreground.

## 2. Architecture

### 2.1 Crates

| Crate (folder) | Package | Job | Depends on |
|---|---|---|---|
| `crates/core` | `dr-core` | Engine, as today: zones, change checks, OCR scheduling, dedup. `Pipeline` (auto) and `Manual` (read key). | nothing |
| `crates/ocr` (new) | `dr-ocr` | WinRT OCR engine (`Windows.Media.Ocr`), moved out of `crates/arena/src/ocr/winrt.rs`. Implements `dr_core::Ocr`. | `dr-core`, `windows` |
| `crates/tts` (new) | `dr-tts` | `Tts` trait. First engine: Windows voices (`Windows.Media.SpeechSynthesis.SpeechSynthesizer`). Kokoro later behind the same trait. | `windows` |
| `crates/runtime` (new) | `dr-runtime` | No UI. Capture, reader thread, speech queue, audio out, global keys, profiles on disk. One `Runtime` handle with commands in and events out. | `dr-core`, `dr-ocr`, `dr-tts`, `windows`, `windows-capture` |
| `crates/app` (new) | `dr-app`, binary `DialogReader.exe` | Slint UI: main window, onboarding, overlays (pill, quick menu, zone edit), tray, single instance, start with Windows. | `dr-runtime`, `slint` |
| `crates/arena` | `arena` | Test harness, as today. Uses `dr-ocr` instead of its own WinRT copy. Keeps PP-OCR (`ppocr` feature). | `dr-core`, `dr-ocr` |
| `spikes/recorder` | `recorder` | Spike tool, unchanged. Source of the capture code pattern. | |

Rule: `dr-core` stays free of Windows and UI code. `dr-runtime` has no Slint. `dr-app` holds no reading logic, only
view models and wiring. That keeps the reading path testable without a UI, and the UI testable without a game.

### 2.2 Threads and data flow

```
 game window
     |  Windows Graphics Capture (windows-capture, as in spikes/recorder and arena live.rs)
     v
 [capture thread]  BGRA frame with row padding -> tight RGBA copy -> "latest frame" slot (drops old frames)
     |
     v
 [reader thread]   auto:   dr_core::Pipeline::on_frame(frame, &mut ocr)     -> Output { events, errors }
                   manual: dr_core::Manual::on_frame(frame, &mut ocr)       -> Option<Press>
                   OCR: dr_ocr::WinOcr (runs on this thread, MTA)
     |  Line { text, zone, kind, t } (and errors)
     v
 [speech thread]   queue -> dr_tts engine -> PCM -> audio out (volume, stop, mute)
     |
     +--> events to the UI thread (slint::invoke_from_event_loop): status, spoken line, error

 [keys thread]     low-level keyboard + mouse hooks (WH_KEYBOARD_LL, WH_MOUSE_LL) with its own message loop
                   read key  -> Manual::press() flag, or "read now" in auto mode
                   quick menu key -> UI opens the quick menu; while it is open, arrow/Enter/Esc/Space are
                   taken by the hook and sent to the menu (the game does not see them)

 [UI thread]       Slint event loop (winit backend): main window, overlay windows, tray icon
```

- Capture rate: `MinimumUpdateIntervalSettings` set to 40 ms (25 fps). This matches the `Manual` ring step
  (`step_ms: 40`) and keeps a 4K copy (33 MB) to about 0.8 GB/s at most. The reader takes the newest frame and
  skips the rest, so a slow OCR call never builds a backlog.
- `dr_core::Frame` needs tight RGBA. The capture thread converts BGRA with padding to tight RGBA once per frame
  (as arena `live.rs` does). A later speed-up: copy only the zone rectangles. This needs a change in `dr-core`,
  so it is not in v1.
- Auto mode "Read now" (main page button, read key in auto mode): reads all zones once through `Manual::read_now`
  on the same frame, then auto mode goes on. One read key means the same thing in both modes: "read my zones now".
- Commands in (`Runtime::send(Cmd)`): `Start(profile)`, `Stop`, `Pause`, `Resume`, `ReadNow`, `StopSpeaking`,
  `Mute(bool)`, `SetVoice`, `SetSpeed`, `SetVolume`, `SetMode`, `SetZones`, `SetKeys`, `CaptureKey`,
  `Screenshot`, `Quit`.
- Events out (`Runtime::events() -> Receiver<Ev>`): `Status(Reading | Paused | Stopped | NoWindow | Error)`,
  `Spoken(Line)`, `Skipped(Line, reason)`, `Speaking { line, progress }`, `KeyCaptured(Key)`, `Screenshot(png)`,
  `Error(String)`, `WindowGone`, `ElevatedGame`.

### 2.3 Capture

- Window list: `EnumWindows` (the windows of the caller's desktop), plus for each window: title, process exe
  name, client size, mode (borderless when the window covers its monitor and has no caption, else windowed), and an
  icon from the exe (`SHGetFileInfoW`) for the list.
- "Games running now" vs "Other windows" (page 02): a window is in the game group when its exe has a profile, or its
  client area covers a whole monitor, or the process has `d3d11.dll`, `d3d12.dll`, `dxgi.dll` or `vulkan-1.dll`
  loaded. When the process cannot be read (elevated), only the first two checks apply. Browser and Electron windows
  (classes `Chrome_WidgetWin_1`, `MozillaWindowClass`) always load those libraries, so the library check is skipped
  for them (M2, 2026-11-22: without that, Zen and an Electron app showed as games). Other apps that draw with
  Direct3D still show as games (on this PC: Flare); see section 6, item 10.
- Settings: `CursorCaptureSettings::WithoutCursor`, `DrawBorderSettings::WithoutBorder`, `ColorFormat::Rgba8`
  (tight RGBA rows, as `dr-core` takes them). On systems that refuse "no border" or "no cursor" (older Windows 10),
  the capture starts again with the Windows default for that setting, so the yellow border shows. That is
  cosmetic, not an error.
- WGC captures the whole window. For a windowed game that includes the title bar and frame, so the screenshot can
  be larger than the client size page 02 shows. Zones are stored in these capture pixels. M4 places overlays by
  the client rect, so it must add the offset of the client area inside the captured window.
- A minimized window gives no picture. Page 02 and Retake refuse it at once with a plain message instead of
  waiting for the 3 s capture timeout. In M2 the capture still runs on the UI thread (up to 3 s, plus PNG
  encoding at Finish). Proposal, not yet planned: move capture to its own thread in M3b.
- The window is found again by exe name when the game restarts. `Status(NoWindow)` while it is gone. When the
  game's process cannot be read (it runs as administrator, or anti-cheat protects it), the exe is empty. Such a
  profile is saved with `title_contains` set to the window title and matches by title only. An empty exe never
  matches a profile.
- Black frames (loading screens) are passed on; `dr-core` reads nothing from them.
- Screenshot for drawing zones: one captured frame saved as PNG next to the profile.
- Live view (added 2026-11-22, Max asked for it): the stages of pages 03, 10 and 11 show the game live (about
  15 fps, made the stage size on the GPU) while the game runs and the app is not minimized or hidden. Zones stay
  in capture pixels. A minimized, closed or uncapturable game shows the saved screenshot with a note. Details
  and cost: `crates/app/README.md`, "Live view".

### 2.4 OCR (`crates/ocr`)

- `WinOcr::new(lang)` and `impl dr_core::Ocr for WinOcr` (`max_dim`, `recognize`). The code is the current
  `crates/arena/src/ocr/winrt.rs`, with `dr_core::OcrLine` and `dr_core::Rect` instead of the arena types.
- The arena keeps its `Engine` trait (it adds `name()`) with a thin adapter over `dr_ocr::WinOcr`. Arena numbers
  must not change after the move: same seeds, same scores.
- Language: the profile's OCR language (default en-US). When the pack is missing, the app says so in plain words
  and links to Windows language settings.

### 2.5 Speech (`crates/tts`) and audio

```rust
pub struct Voice { pub id: String, pub name: String, pub lang: String, pub gender: Option<Gender>, pub engine: &'static str }
pub struct Pcm { pub rate: u32, pub channels: u16, pub samples: Vec<i16> }
pub trait Tts: Send {
    fn voices(&self) -> Vec<Voice>;
    fn synth(&mut self, text: &str, voice: &str, speed: f32) -> Result<Pcm, String>;
}
```

- First engine `WinTts`: `SpeechSynthesizer`, `AllVoices()` for the list, `Options().SetSpeakingRate(speed)`,
  `SynthesizeTextToStreamAsync` gives a WAV stream that is parsed to PCM.
- Kokoro later: a second `impl Tts` (ONNX through `ort`), chosen per profile. The UI shows the engine name before
  the voice ("Windows · David", later "Kokoro · Adam").
- Audio out: one output stream through `rodio` (pinned exact version). Volume, stop and mute act on the sink.
  Speed is passed to the engine (better quality than resampling).
- Queue: lines are spoken in order. A new manual press replaces the queue. Stop clears it. Lines older than 10 s
  in the queue are dropped (they no longer match the screen). A speaker line and the dialogue line after it are
  spoken as one ("Arthur: We need to ...") when they come in the same read.
- Built in M3a: `dr-tts` holds the trait, `WinTts`, WAV parsing and `FakeTts`. The queue, the speech thread and
  the sinks (`RodioOut`, `NullSink`) are in `dr-runtime` (`speech.rs`), as section 2.1 says. Commands: Say,
  Replace (manual press), Test (voice test, with an id), SetTestVolume, Stop, Skip, Replay, Mute, SetVoice,
  SetSpeed, SetVolume. Events (Started, Finished, Cut, Dropped, Error) carry the `Item` (its lines and zones) and
  the test id, so a poll ends only on its own sample. Error has a kind (engine, no device, device lost); the UI
  shows a plain line for the kind and writes the raw text to `speech.log`.
- Speech thread rules added after the M3a review (2026-11-23): commands sent while a line is being made are read
  before it plays, and a Stop, Skip, Replay or Test among them drops the sound (no short blip). A device that
  reports an error, or a sound that runs past 1.5 times its length plus 1.5 s, ends the line with an error, lets
  go of the device and opens the default device again at the next line; the next line also moves to a new default
  device. The thread joins the multithreaded COM apartment first (cpal would make it a single-threaded one with no
  message loop). `WinTts` keeps the picked voice until the id changes.
- An empty or unknown voice id means the Windows default voice in `WinTts`; the app shows and uses the first
  installed voice in that case. On Max's PC (2026-11-22, checked again 2026-11-23 with `probe_voices`) WinRT
  lists no voice: `AllVoices` returns an empty list, and `DefaultVoice` and `SpeechSynthesizer::new` fail with
  0x80070002. The registry key `HKLM\SOFTWARE\Microsoft\Speech_OneCore\Voices\Tokens` is missing, although
  `C:\Windows\Speech_OneCore\Engines\TTS\en-US` exists; only the old SAPI 5 voice Zira is registered. The app
  shows the "no voice" state there. Adding a voice in Windows Settings > Time & language > Speech may not fix it
  (the files are there but not registered); if it does not, v1 needs a SAPI 5 engine (open question 11).

### 2.6 Global keys

- One `Key` type, saved as text: `"Mouse 4"`, `"Mouse 5"`, `"Mouse 3"`, `"F7"`, `"Ctrl+Shift+R"`.
- Low-level hooks on the keys thread catch both keyboard keys and mouse side buttons (`WH_MOUSE_LL`,
  `XBUTTON1`/`XBUTTON2`). One code path for both. The hook callback only posts a message and returns at once
  (Windows drops hooks that take longer than `LowLevelHooksTimeout`).
- The read key and quick menu key are passed through to the game (not swallowed), except keys that are only used
  by DialogReader while the quick menu or zone edit is open.
- Key capture ("Change" on the keys page): the next key or mouse button press is taken as the new key. Esc cancels.
  Left and right mouse buttons are refused (a game needs them).
- Elevated games: see Risks.

### 2.7 Windows and overlays

All windows are Slint windows on the winit backend, on the UI thread. Each overlay gets extra Win32 styles through
its HWND (winit accessor) right after its first show, because the HWND exists only then, and again whenever Slint
gives it a new HWND. A window subclass keeps the styles, because winit rewrites them on every show. The recipe is
in [overlay-spike.md](overlay-spike.md).

| Window | Mockup | Styles | Input |
|---|---|---|---|
| Main window | 01 to 06, 10-main-b, 11, 12, 14 | frameless (own title bar as in the mockups), normal taskbar window | normal |
| Status pill | 13 | `WS_EX_LAYERED`, `WS_EX_TRANSPARENT`, `WS_EX_TOPMOST`, `WS_EX_NOACTIVATE`, `WS_EX_TOOLWINDOW` | none, click-through |
| Quick menu | 15-quick-menu-a, 15-quick-menu-voice | `WS_EX_TOPMOST`, `WS_EX_NOACTIVATE`, `WS_EX_TOOLWINDOW`, transparent background | keyboard through the hook; mouse clicks work but never activate it |
| Zone edit | 16-zone-edit | as the quick menu, covers the game's client area | mouse on the window (no activation); Delete, Backspace, Esc through the hook |

- Placement: overlays follow the game's client rect (`GetClientRect` + `ClientToScreen`, physical pixels) and its
  monitor's scale. The pill sits at the position set in Settings (top right, bottom center, off).
- While the quick menu or zone edit is open, reading pauses and the pill hides.
- The quick menu is centered on the game (design rule). Order: Manage zones, Stop (play/pause), Mute, Voice,
  Speed, Volume, Mode, Settings, Quit (press twice). Arrows move and change, Enter selects, Esc closes.
- Zone edit: drag on empty space adds a zone, click selects, Delete/Backspace or the small x removes, Esc or Done
  finishes. It draws over a frozen screenshot of the game, so the zones line up even when the game moves.

### 2.8 Profiles and settings on disk

Folder: `%APPDATA%\DialogReader\`.

```
settings.json                    app settings (start with Windows, overlay position, default voice, keys default)
profiles\<id>.json               one per game
profiles\<id>.png                last screenshot of that game (for the zone editor and the cover)
log\<id>.jsonl                   read lines (time, zone, kind, text, spoken or skipped + reason), capped at 5 MB
```

Profile JSON (version field so later changes can migrate):

```json
{
  "version": 1,
  "id": "rdr2",
  "name": "Red Dead Redemption 2",
  "match": { "exe": "RDR2.exe", "title_contains": null },
  "mode": "auto",
  "keys": { "read": "Mouse 4", "quick_menu": "F7" },
  "voice": { "engine": "windows", "id": "...", "speed": 1.1, "volume": 0.8 },
  "read_prompts": false,
  "ocr_lang": "en-US",
  "zones": [
    { "name": "Subtitles", "kind": "dialogue", "on": true,
      "rect": [700, 1880, 2440, 160], "base": [3840, 2160] }
  ]
}
```

- `mode` is `auto` (reads new text as it appears) or `manual` (on the read key).
- Zone rects are in capture pixels of the screenshot they were drawn on (`base`), exactly what `dr_core::Zone`
  takes. They scale when the game runs at another size. DPI does not touch them.
- Kind names on disk and in the UI: `dialogue` = Dialogue, `speaker` = Speaker, `notification` = Pickups,
  `prompt` = Prompts, `ignore` = Ignore.
- Zones that are switched off (main page toggles) are not passed to `dr-core`.
- The file name is the id: on load, `id` is set from the file name, so a copied or hand-edited file always saves
  back to itself. A new profile takes an id that no file in `profiles\` uses yet (`.json`, `.json.bad`, `.png`;
  case does not matter), so a file that was not loaded (newer format, damaged) is never overwritten.
- Several profiles can share an exe (emulators, `javaw.exe`, `Game.exe`; Max, 2026-11-22), told apart by the
  window title. The first profile of an exe matches by exe only. A new profile for an exe that already has one
  saves the window title as `title_contains`, and the profiles of that exe with no title get their name (the window
  title at their setup) as `title_contains` too, so the first game no longer takes every window of the exe
  (M3a review, 2026-11-23). Titles are compared without counters (`vm::stable_title`): numbers with a point,
  comma or colon, numbers with a unit (fps, ms, Hz, %) and numbers after "FPS" or "VPS" are left out, other
  numbers stay ("Persona 4" and "Persona 5" stay apart; "HD 1.5" and "HD 2.5" do not). A title that changes its
  shape (for example when paused) still loses its match. A window belongs to the profile that fits it best: a
  profile with a title beats one with the exe only, a longer title beats a shorter one, and on a tie the newest
  file wins. So one window makes one profile "Running", and Retake uses that same window. Page 02 refuses a window
  when its best profile matched it by title (the same rule), or, for a profile with the exe only, when its name is
  this window's title apart from counters, and points to the game switcher.
- `voice.id` is the Windows voice picked on page 05 or in Settings > Voices (empty when none is installed);
  `speed` (0.5 to 2.0) and `volume` come from page 05 and Settings > Voices.
- The active profile follows the game: when a window that matches a profile appears, the profile it fits best
  starts.
- Writes are atomic (write to `.tmp`, then rename). A broken file is kept as `.bad` and the app starts with
  defaults and says so.

### 2.9 App shell

- Single instance: a named mutex `Local\DialogReader.SingleInstance`. A second start finds the first instance's
  message-only window and asks it to show the main window, then exits.
- Tray icon (`tray-icon` crate, on the UI thread): left click opens the main window; menu: Open, Pause/Resume,
  Quit.
- Start with Windows: `HKCU\Software\Microsoft\Windows\CurrentVersion\Run\DialogReader` =
  `"<path>\DialogReader.exe" --minimized`. Off by default, set in Settings.
- Closing the main window keeps the app in the tray while a profile is active. Quit is in the tray, the quick
  menu, and the main window menu.
- `#![windows_subsystem = "windows"]`, per-monitor DPI aware v2 (app manifest), `--version` prints the version.
- Version: the workspace version (`CARGO_PKG_VERSION`), shown in Settings > Updates and in the installer name.

## 3. Design tokens in Slint

The tokens live in `crates/app/ui/theme.slint` as globals. Values come from
`design/mockups/c-midnight/tokens.css`. Slint colors with alpha use 8-digit hex (`#RRGGBBAA`).

```slint
export global Palette {
    out property <color> bg: #07090E;
    out property <color> surface: #0C1018;
    out property <color> surface-2: #121825;
    out property <color> surface-3: #1A2232;
    out property <color> line: #1C2331;
    out property <color> line-2: #283143;
    out property <color> scrim: #030509A8;        // rgba(3,5,9,0.66)
    out property <color> text: #EDF0F6;
    out property <color> text-2: #A2ABBC;
    out property <color> text-3: #7F899D;
    out property <color> accent: #5B7CFF;
    out property <color> accent-soft: #5B7CFF2E;  // 0.18
    out property <color> accent-ring: #5B7CFF73;  // 0.45, selection ring
    out property <color> ok: #4ADE80;
    out property <color> ok-soft: #4ADE801F;      // 0.12
    out property <color> danger: #F87171;
    out property <color> pill-bg: #07090EDB;      // 0.86
    out property <color> pill-line: #FFFFFF1A;    // 0.10
    // zones on every stage are grey; zone kinds carry no color (section 6, item 4)
    out property <color> zone-fill: #A2ABBC24;    // grey zone fill (0.14)
}
export global Space { out property <length> s1: 4px; out property <length> s2: 8px; out property <length> s3: 12px;
    out property <length> s4: 16px; out property <length> s5: 24px; out property <length> s6: 32px; out property <length> s7: 48px; }
export global Radius { out property <length> xs: 4px; out property <length> sm: 6px; out property <length> md: 8px;
    out property <length> lg: 12px; out property <length> pill: 999px; }
export global Size { out property <length> ctl: 32px; out property <length> ctl-lg: 40px; out property <length> bar: 52px;
    out property <length> row: 44px; out property <length> icon: 16px; out property <length> icon-sm: 12px;
    out property <length> kbd: 24px; out property <length> kbd-sm: 20px; out property <length> col: 960px;
    out property <length> page-x: 32px; out property <length> page-max: 1376px; }
export global Type { out property <length> fs1: 12px; out property <length> fs2: 14px; out property <length> fs3: 16px;
    out property <length> fs4: 20px; out property <length> fs5: 32px;
    out property <string> font: "Geist"; out property <string> mono: "Geist Mono"; }
```

- **Fonts:** Geist and Geist Mono (SIL OFL) are bundled as variable TTFs in `crates/app/ui/fonts/` (the Google
  Fonts builds, as the mockups load them) and loaded with `import "fonts/Geist-Variable.ttf";`. Weights used: 400,
  500, 600, as tokens `Type.w-regular`, `w-medium`, `w-semibold`. The Slint renderers in use (software for
  tests and renders, Skia for the window, femtovg as fallback) apply the weight axis. Headings: `letter-spacing` -0.025em on fs5 (about -0.8px, `Type.ls-display`).
- **Text weight on screen:** Chromium draws the mockup text with ClearType and extra contrast; Slint draws
  grayscale coverage. With the software and femtovg renderers, all text has about 15 to 30% less
  ink than in the shots; the weights themselves are right (measured, see `crates/app/README.md`). The real window
  uses Skia, which matches the shots (section 6, item 8).
- **Line height:** Slint 1.18 `Text` has `line-height-factor`, relative to the font's natural line height
  (Geist: 1.3 em). Tokens `Type.lhf-115` to `lhf-150` give the CSS line heights 1.15 to 1.5. `StyledText` has no
  line height, so mixed-color multi-line text is one `Text` with the colored prefix drawn over its start.
- **Shadows:** `drop-shadow-offset-y`, `drop-shadow-blur`, `drop-shadow-color` on `Rectangle`, from the `Shadow`
  global: low = (0, 1px, 2px, #00000066), pop = (0, 16px, 40px, #0000008C). The software renderer draws none.
- **Rounded pictures:** the real window clips to the border radius (`clip: true`). The software renderer cannot,
  so `CornerMask` paints the corners there only (`Render.software`, set by the headless renderer).
- **No hand-written colors in pages:** every color is a `Palette` token, including the one-off ones (white on
  the accent, dark text on zone tags, stage background, quit reds, zone edit selection).
- **Selection ring:** an outer `Rectangle` 3px larger on each side with a 1px `accent-ring` border (as the
  tokens.css note says; Slint has no outline-offset).
- **Icons:** one style: 24x24 viewBox, 1.5px stroke, round caps. The path data in `_build.py` (`P = {...}`) is
  exported once to `crates/app/ui/icons/<name>.svg` by a small script (`crates/app/tools/export_icons.py`).
  Slint shows them with `Image { source: @image-url("icons/x.svg"); colorize: <color>; width: 16px; }`.
  Kind icons: `k-dialogue` (speech bubble), `k-speaker` (person), `k-pickups` (box), `k-prompts` (keycap),
  `k-ignore` (eye-off). Always grey (`text-2` or `text-3`). There are no kind colors (section 6, item 4).
- **Game art** in M1 (fake data): the SVG scenes the mockups draw are exported to `crates/app/ui/art/*.svg`.
  From M2 on, the real screenshot of the game takes their place.
- **No CSS-only effects:** no blur, no backdrop filter. All mockups were made inside that rule.

### 3.1 Shared components (`crates/app/ui/components/`)

| Slint component | From tokens.css class | Notes |
|---|---|---|
| `Button` (kind: default, primary, ghost, danger; size: md, lg; icon-only) | `.btn*` | optional icon and keycap |
| `Kbd` (size: md, sm) | `.kbd` | mono text |
| `TextInput`, `Select` | `.input`, `.select` | built on `TextInput` and a `PopupWindow` list |
| `Toggle` | `.toggle` | 32x20, animated knob |
| `Segmented` | `.seg` | model of labels, selected index |
| `Radio` | `.radio` | |
| `Slider` | `.slider` | 4px track, 16px knob, drag with `TouchArea` |
| `Card`, `CardHead`, `SetRow`, `Count` | `.card`, `.card-head`, `.set-row`, `.count` | |
| `StatusChip`, `Dot` | `.status`, `.dot` | |
| `KindIcon`, `KindTag` | `.kind`, `.kicon`, `.zic` | grey icons only |
| `ZoneRow`, `ZoneList` | `.zrow` (round 4) | icon, name, toggle |
| `ZoneBox`, `Handle`, `ZoneBadge` | `.zone`, `.handle` | on the stage and in zone edit |
| `Stage` | `.stage` | 16:9 image with zones on top, maps image px to view px |
| `ZoneTools`, `KindPicker` | `.ztools` | Select / Draw zone, kind segmented (grey kind icon + name), Retake screenshot |
| `Waveform`, `PillWave` | `.wave`, `.pill-wave` | bars from a model of heights |
| `TitleBar`, `Tabs`, `GameButton`, `WinControls` | `.bar`, `.tabs`, `.switcher`, `.winctl` | drag and resize via winit |
| `StepsBar`, `ObHead`, `ObFoot` | `.steps`, `.ob-*`, `.foot` | onboarding chrome |
| `RecentRow`, `LogRow` | `.log-row` | two-line recent row (10-main-b), full log row (14) |
| `ITile` | `.itile` | icon tile in lists |
| `KeysCard`, `KeyRow` | 04-read-key-d | onboarding step 3. Settings shows the same KeysCard (Max: "D everywhere") |

### 3.2 Pages and windows

All main-window pages are 1440x900 logical px in the mockups. The window opens at that size (or smaller on small
screens) and lays out to the size it gets.

| Mockup | Slint file | Top component and parts |
|---|---|---|
| 01-welcome | `pages/welcome.slint` | `WelcomePage`: brand bar, headline, three step items, preview card (fake dialogue + pill), Get started, Import settings |
| 02-pick-window | `pages/pick_window.slint` | `PickWindowPage`: steps bar (1), search input, Refresh, card "Games running now" (`WindowCard`: thumb, name, exe · size · mode), card "Other windows" (`WindowRow`), Back / Continue |
| 03-draw-zones | `pages/draw_zones.slint` | `DrawZonesPage`: steps bar (2), `ZoneTools`, `Stage` with `ZoneBox`es, `ZoneList`, card "What each kind does" |
| 04-read-key-d | `pages/keys.slint` | `KeysPage`: steps bar (3, "Keys"), `KeysCard` with two `KeyRow`s (Read zones now, Quick menu), Change opens key capture |
| 05-voice | `pages/voice.slint` | `VoicePage`: steps bar (4), voice list (`VoiceRow`), test card (sample line, `Waveform`, Speed and Volume sliders, Stop) |
| 06-mode | `pages/mode.slint` | `ModePage`: steps bar (5), two `ModeCard`s (Automatic, On key press), setup summary card, Finish |
| 10-main-b | `pages/overview.slint` | `MainShell` (`TitleBar` with game button + Ctrl K, tabs Overview/Zones/Settings, status chip) and `OverviewTab`: control bar (status, Read now, Pause, mode, voice, key), live `Stage`, zone strip (toggles, +N overflow, edit), now-speaking card (line, wave, replay, stop), Recent card (5 `RecentRow`s, Show all) |
| 11-zones | `pages/zones.slint` | `ZonesTab`: `ZoneTools`, `Stage` with handles, screenshot info line, `ZoneList` with Add zone and Delete zone |
| 12-settings | `pages/settings.slint` | `SettingsTab`: section list and cards: Startup, In-game overlay, Keys (the two 04-read-key-d cards, in place of the old Shortcuts section; Max: "D everywhere"), Voices, Updates |
| 13-overlay | `overlays/pill.slint` | `PillWindow`: dot, speaker, `PillWave` |
| 14-game-switcher | `overlays/game_switcher.slint` | `GameSwitcher`: scrim + panel over the main window (search, Running now, Your games, Enter / Esc hints). Opened with Ctrl K or the game button. The page behind it is 10-main-b. "Show all" in Recent opens the activity list shown in 14's background (All / Read / Skipped) |
| 15-quick-menu-a | `overlays/quick_menu.slint` | `QuickMenuWindow`: header (game, status), rows (`QmNavRow`, `QmToggleRow`, `QmStepperRow`, `QmSliderRow`, `QmSegRow`, `QmDangerRow` with press-twice state), key hints bar; states "stopped" and "press again to quit" |
| 15-quick-menu-voice | same file | `QmVoiceList`: voice sub-list (Space preview, Enter choose, Left back). The review previews (menu over the game, the two other states) are in `overlays/quick_menu_preview.slint`, so the overlay window imports only the menu |
| 16-zone-edit | `overlays/zone_edit.slint` | `ZoneEditWindow`: hint bar with Done, zone boxes with grey kind badges, selected zone with handles and x, kind picker popover, size tag while drawing |

View state lives in Rust and is pushed to Slint through `in` properties and `ModelRc<VecModel<...>>`. Since M2 the
view-model layer (`crates/app/src/vm/`) owns it: `vm::Source::Real` (profiles in a data folder and a `Desktop` for
windows and screenshots) or `vm::Source::Fake` (the mockup data of `src/fake.rs`, for render-screens and tests).
`vm/zones.rs` holds the zone drawing state with one step of undo. Slint callbacks go back to Rust (`clicked`, `zone-drawn(rect)`, `key-change(which)`).
Navigation is one Slint enum property on the main window:
`Page { welcome, pick-window, draw-zones, keys, voice, mode, overview, zones, settings }` plus `switcher-open`.

## 4. Testing headless

- **Screenshots:** a test platform (`slint::platform::Platform` with a `MinimalSoftwareWindow` and the software
  renderer) renders each page at 1440x900 to an RGBA buffer and saves a PNG. No window is created, nothing
  touches the desktop. `cargo run -p dr-app --bin render-screens -- <out-dir>` writes all pages and overlays.
- **Mockup check:** `tests/screens.rs` compares each render with its mockup shot over 10 x 10 px cells (mean
  difference and share of changed cells, limits per screen). It catches layout and color changes, not text
  weight.
- **Golden images:** after Max approves the M1 screens and the open questions in section 6 are settled, they are
  stored in `crates/app/tests/golden/`. Tests compare new renders with a small tolerance (per-pixel delta and a
  cap on changed pixels).
- **Interaction:** the Slint testing backend (`i-slint-backend-testing`, same version as `slint`) runs the UI with
  no window. Tests find elements by `accessible-label` or element id, click, type and drag, and check the
  `page` property and view models.
- **Runtime:** `FrameSource` and `Tts` are traits. Tests feed arena scenes or PNG sequences as frames and use a
  fake TTS and a null audio sink, so no sound plays and no game is needed. OCR tests use real WinRT OCR on fixture
  PNGs (skipped with a clear message when no en-US OCR pack is installed).
- **Win32 overlay checks:** created on a hidden second desktop (the pattern in Prism's `tools/e2e/hidden.mjs` and
  `HiddenDesktop.cs`). The test checks the extended styles and that `GetForegroundWindow` did not change.
- **Keys:** the hook callback is a pure function from the hook struct to an action, unit-tested without real
  input. A real input check (SendInput) runs only on the hidden desktop.

## 5. Risks and plans

| Risk | What happens | Plan |
|---|---|---|
| Exclusive fullscreen | WGC gives black frames or none. Overlays do not show over it. Many DX12 and "fullscreen optimised" games are not true exclusive, so the gap may be small (unverified). | Detect it: `SHQueryUserNotificationState` returns `QUNS_RUNNING_D3D_FULL_SCREEN`, or only black frames for 3 s. Tell the user in plain words: "This game runs in exclusive fullscreen. Switch it to borderless or windowed in the game's settings." Keys still work; speech still works if capture works. No injection in v1 (hudhook licence and anti-cheat). Test list in M3b/M4: the target games from plan.md. |
| Elevated games | A game that runs as admin: low-level hooks of a normal-level app do not see its input (UIPI), the overlay may not stay on top, capture may fail (unverified). | Detect it: `OpenProcess` + `GetTokenInformation(TokenElevation)`; access denied also counts as elevated. Show "This game runs as administrator. DialogReader needs to run as administrator too." with a button that restarts DialogReader elevated (`ShellExecuteW` "runas"). Start with Windows "as administrator" uses a Task Scheduler task with highest privileges instead of the Run key. Check in M3b whether `RegisterHotKey` still fires for keyboard keys in that case and use it as a fallback for keyboard read keys. |
| Hook timeouts and anti-cheat | Slow hook callbacks get removed by Windows. Some anti-cheat dislikes low-level hooks (unverified). | Callback only posts a message. Re-install the hooks when they stop firing (watchdog). Hooks are installed only while a profile is active. Document games where anti-cheat complains. |
| Games that lock the mouse | In zone edit, a game with a captured or clipped cursor may keep the mouse. | Zone edit releases the clip (`ClipCursor(None)`) while open. If the game takes it back, the hint bar says "Use Zones in the main window" and Esc closes. The main window Zones tab always works. |
| Focus | An overlay that activates would minimise or pause some games. | `WS_EX_NOACTIVATE` on every overlay, keyboard only through the hook, `ShowWindow(SW_SHOWNOACTIVATE)`. A test on a hidden desktop counts activation per thread (`GetForegroundWindow` is always NULL there; see overlay-spike.md). |
| DPI and mixed monitors | Zones, overlays and screenshots could be off by the scale factor. | Per-monitor DPI aware v2. Zones in capture pixels (never in logical px). Overlays placed in physical px from the game's client rect, then sized with that monitor's scale. Re-place on `WM_DPICHANGED` and when the game window moves or changes size. Tests for the mapping at 100%, 150% and 200%. |
| Frameless main window | Own title bar loses some native behaviour (snap layouts, resize borders). | winit `drag_window` and `drag_resize_window` from the title bar and edges. If snap layouts matter, keep the native frame and theme it dark later. |
| Speed of 4K frames | Copying 33 MB per frame costs CPU next to a game. | 25 fps cap, newest-frame slot, later zone-only copies. Measure CPU in M3b with Satisfactory. |
| Slint limits | No line height on `StyledText`, no outline-offset, no blur, no rounded clip and no shadows in the software renderer. Overlay transparency depends on the winit backend. | `line-height-factor` on `Text`. Rings as extra rectangles. Corner masks in software renders only. Check transparent, frameless, topmost windows in a spike right after M1 (app-plan.md); fallback: draw overlays with the software renderer into a Win32 layered window (`UpdateLayeredWindow`). |
| Windows voices quality | Built-in voices sound flat next to Kokoro. | Tts trait from day one. Kokoro comes after v1 behind it. |
| Log shows skipped lines | 10-main-b and 14 show "Prompts off" and "Already read" rows. `dr-core` does not report skipped lines today. | M1 shows them from fake data. M3a adds a small `dr-core` change: a `skipped` list in `Output` and `Press` (line, zone, reason: already read, prompts off). With prompts off, prompt zones are still read but not spoken, so the log can show them; if that OCR cost shows up in the M3b CPU check, prompt zones are dropped and the log shows nothing for them. Cost seen in the M3a review (simulated clock, soft): standard a1 with prompts off, CPU per frame p50 0.65 to 2.35 ms, p95 14.6 to 17.9 ms, same score. In manual mode a press also waits for the OCR of every prompt zone before any line is spoken. On the M3b checklist. |

## 6. Open questions for Max

1. Voice labels in the approved mockups say "Kokoro · Adam". v1 ships Windows voices first. From M3a the labels
   show the real engine ("Windows · David"). Built in M3a and waiting for Max's yes: the text changes are listed
   in app-plan.md, M3a result.
2. Settings > Updates: v1 shows the version and a "Check now" link to GitHub Releases. Automatic install of updates
   is later.
3. "Import settings" on the welcome page: v1 imports a profile JSON exported from DialogReader (Export is in the
   profile menu). It does not read the old C# app's settings.
4. Zone kind colors: settled. Max (2026-11-22): zone kinds carry no color anywhere, one grey line icon per kind.
   Every stage (01, 03, 10-main-b, 11) draws zones with grey `text-2` lines and tags and the `zone-fill` fill; the
   selected zone uses the accent for selection only. The 06 summary dots are `text-3`. The `z-*` color tokens and
   `Kinds.color` / `Kinds.soft` are gone from the app. The "Dialogue" chip under the 01 picture is a grey icon and
   name in the app (its mockup chip was left as it was, see item 9).
5. Settings > Keys: settled. Max picked "D everywhere" (2026-11-22), so Settings shows the same two 04-read-key-d
   cards as setup.
6. Media pause while speaking. The old app had it; this plan puts it after v1. Is v1 without it acceptable?
7. Kokoro wording. M3a changed the voice page header, "Get more voices" and Settings > Voices to what v1 ships
   (Windows voices), as a proposal. Max approves it or names other words (list in app-plan.md, M3a result).
8. Renderer: settled. Max (2026-11-22): Skia with `panic = "abort"` in a ship-only profile. Measured before
   (`crates/app/README.md`, "Renderer"): Skia text has 100 to 101% of the mockup shots' ink at 100% and 225%
   scale, femtovg 79 to 89%. The workspace `Cargo.toml` has `[profile.ship]` (inherits release, fat LTO, one
   codegen unit, `panic = "abort"`); only `cargo build --profile ship` uses it, so tests, the arena and normal
   builds keep unwinding. `renderer-skia` is the default feature of dr-app and turns on Slint's
   `renderer-skia-opengl`; `main.rs` asks for the `skia-opengl` renderer by name, because overlays need OpenGL
   Skia (Skia's default surface draws their see-through margins black, docs/overlay-spike.md). femtovg stays as an
   opt-in fallback (`--no-default-features --features renderer-femtovg`). render-screens and the tests keep the
   software renderer. A panic hook (`crates/app/src/crash.rs`) writes a crash log to
   `%LOCALAPPDATA%\DialogReader\logs` before the abort.
9. Kind picker and the 01 and 10 stages: settled. Max (2026-11-22): the stages on 01 and 10-main-b turn grey (app
   and those two mockups, nothing else on them changed), and the Kind picker on 03 and 11 shows the grey kind line
   icons instead of five identical grey dots (app and those two mockups). Done 2026-11-22. Open: the 01 mockup
   still shows a colored "Dialogue" chip under the picture (the app shows it grey), and the 06 summary still shows
   five identical grey dots; both were left alone because Max named only the stages and the picker.
10. Game check on page 02 (M2): any app that draws with Direct3D counts as a game by the library check (on this
   PC, Flare). Browsers and Electron are left out by window class. Is that good enough, or should the library
   check go (games would then need a full-monitor window or a profile)?
11. No Windows voice on Max's PC (M3a review, 2026-11-23). Max adds an English voice in Windows Settings > Time &
    language > Speech once, then runs `cargo test -p dr-tts --test voices -- --ignored`. If WinRT still lists no
    voice, v1 needs a SAPI 5 engine (Zira is registered there) before M3b, or Max cannot hear his own app.
12. Games that share an exe (M3a review): the first profile of an exe gets its title when a second game of that exe
    is set up, and titles are compared without counters (FPS, frame times, percent, versions). Open: let Max edit
    the saved title on page 02 (new UI, needs a mockup). The reviewer advised doing it.

### Decisions after M2 (Max, 2026-11-22)
- Games that share one exe (emulators, javaw.exe, Game.exe): allow several profiles per exe and tell them apart by
  exe plus window title. Replaces the "one profile per exe" refusal from M2. Built in M3a (section 2.8).
- UI bits added in M2 without a mockup (rename field, notice strip, exe icons on page 02, scrolling lists, the blue
  box while drawing) are accepted as built. Max flags anything he dislikes when he sees it.
