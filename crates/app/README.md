# dr-app: the DialogReader window

The Slint UI of DialogReader. It builds `DialogReader.exe`.

**Status: M1.** Every approved screen from `design/mockups/c-midnight` is built in Slint and filled with the
fake data of the mockups. Navigation works. There is no capture, OCR, speech or global key yet. The plan is in
[docs/app-plan.md](../../docs/app-plan.md) and the design in [docs/app-spec.md](../../docs/app-spec.md).

## Run, test, render

| What | Command |
|---|---|
| Build | `cargo build -p dr-app` |
| Tests (headless) | `cargo test -p dr-app` |
| Mockup comparison numbers | `cargo test -p dr-app --test screens -- --nocapture screens_match` |
| Render every screen to PNG | `cargo run -p dr-app --bin render-screens -- [out-dir]` |
| Open the app (shows a window) | `cargo run -p dr-app --bin DialogReader` |
| Build with the Skia renderer | `cargo build --release -p dr-app --bin DialogReader --no-default-features --features renderer-skia` |

- `render-screens` draws each screen with Slint's software renderer at 1440 x 900 into a buffer and saves it as
  PNG. It opens no window. The default out-dir is `target/screens`. File names match the mockup shots
  (`01-welcome.png` to `16-zone-edit.png`), plus `12-settings-keys.png` (Settings scrolled to the keys) and
  `17-activity.png` (the list behind "Show all").
- The app options are for checks by hand: `--page <welcome|pick-window|draw-zones|keys|voice|mode|overview|zones|settings|activity>`
  opens a page, and `--preview-overlay <pill|quick-menu|quick-menu-voice|zone-edit>` shows an in-game overlay inside
  the main window. `--version` prints the version.
- Never run the app window from a test. Tests use the Slint testing backend (`tests/nav.rs`) or the software
  renderer (`tests/screens.rs`), so nothing shows on screen.
- `tests/screens.rs` also compares every render with its mockup shot in `design/mockups/c-midnight/shots/`. Both
  pictures are averaged over 10 x 10 px cells; the mean difference and the share of cells that move by more than
  32 must stay under a limit per screen. Screens that differ by design compare only a region (14: the switcher
  panel; 06 and 12: above the changed rows). It catches moved, missing or recolored blocks. It does not catch text
  weight or a 1 to 2 px shift, so the side-by-side review stays.

## Layout

```
build.rs                 compiles ui/app.slint (with debug info outside release, for the testing backend)
src/main.rs              DialogReader.exe: window, command-line options
src/lib.rs               build_main_window(): window + fake data + window buttons
src/fake.rs              fake data and the zone edit callbacks
src/geom.rs              stage mapping, drag to zone, hit tests, dim path (unit-tested)
src/screens.rs           headless renderer and the screen list (sets Render.software)
src/bin/render-screens.rs
ui/theme.slint           tokens: Palette, Space, Radius, Size, Type, Shadow; Render; Kinds, Icons, Art
ui/data.slint            view-model structs, Page and OverlayPreview enums
ui/app.slint             MainWindow: pages, navigation, keys (Ctrl K, Esc, arrows), game switcher, overlay previews
ui/components/           base (Button, Kbd, Toggle, Segmented, Slider, Card, ...), chrome (bars, steps, tabs), stage
ui/pages/                welcome, pick_window, draw_zones, keys, voice, mode, overview, zones, settings, activity
ui/overlays/             pill, game_switcher, quick_menu (menu + voice flyout, what the M4 overlay needs),
                         quick_menu_preview (review only: menu over the game and its two other states), zone_edit
ui/fonts/                Geist 1.800 and Geist Mono 1.701 variable fonts (SIL OFL, see OFL.txt)
ui/icons/, ui/art/       written by tools/export_icons.py from design/mockups/c-midnight/_build.py
tests/                   nav.rs (testing backend), screens.rs (renders every screen, compares with the mockups)
examples/snapshot.rs     one page in a real window with a chosen renderer, saved as PNG (run on a hidden desktop)
tools/hidden-run.ps1     runs a program on a hidden Windows desktop and waits for it
```

## Design rules in code

- All colors, sizes, shadows, letter spacing, line heights and font weights come from `ui/theme.slint`. No page
  writes a color by hand. Colors with alpha are 8-digit hex. In `@markdown` text, write `\{Palette.text-2}`.
- Icons are white SVG files drawn with `colorize`. `<name>.svg` is drawn for 16 px, `-sm` for 12 px, `-lg` for 22 px,
  so the line is as thin as in the mockups. Kind icons are always grey.
- Zone kinds have no color on 03 and 11 (Max, 2026-11-22): `Stage { grey-kinds: true; }` draws every zone and tag
  in grey (`text-2`, `zone-fill`), and the selected zone's outline and handles in the accent. The Kind picker dots
  and the 06 summary dots are `text-3`. The 01 and 10 stages still use the kind colors, as their mockups do.
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
- Both renderers apply the weight axis: the software renderer passes the variation to swash, and femtovg (the
  real window; Slint builds it with only its `swash` feature) builds glyphs with swash using the same values and
  keys its glyph cache by them.
- The M1 review said bold text draws as regular. That did not reproduce. With the earlier static TTFs the brand
  word "DialogReader" had 415.6 / 434.5 / 447.7 units of ink at 400 / 500 / 600, so each weight draws differently.
  The mockup shot has 517.4 at 600.
- The gap is the rasterizer, not the weight. Chromium draws with ClearType and extra contrast; Slint draws
  grayscale coverage. Regular text has the same gap: the "now speaking" paragraph has 1762.6 units of ink in the
  shot and 1362.7 in Slint (+29%). Raising the weight does not close it: at 700 the brand word reaches only 474.6.
  The Skia renderer closes this gap; see "Renderer" below.

## Renderer (Skia spike, measured 2026-11-22)

The real window draws with femtovg (OpenGL), the default feature `renderer-femtovg`. The software renderer is
always built in: render-screens and the tests use it, and `SLINT_BACKEND=winit-software` makes the window use it
when OpenGL fails. Skia is an opt-in feature, `renderer-skia` (same Slint 1.18.1).

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

`cargo build --profile ship -p dr-app --bin DialogReader --no-default-features --features renderer-skia` writes to
`target\ship` (checked 2026-11-22 with the same settings passed as `--config profile.ship.*`: 22,085,632 bytes,
2 min 49 s). Tests, the arena and normal release builds keep unwinding. The cost is in the shipped exe only: a
panic in any thread ends the app at once, with no `Drop` cleanup. With unwinding, a panic in a worker thread ends
only that thread, and a panic on the UI thread still ends the app, but runs `Drop` first. The app has no
`catch_unwind` or panic hook today. Skia stays off by default until Max decides (docs/app-spec.md, section 6,
item 8).

**Build.** `skia-bindings` downloads a prebuilt Skia (about 10 MB,
`skia-binaries-...-x86_64-pc-windows-msvc-d3d-ganesh-gl-jpegd-jpege-pdf.tar.gz`) from GitHub at build time, so the
first build needs the internet (or `SKIA_BINARIES_URL` pointing at a local copy). It unpacks to about 82 MB in
`target`. With the other crates already built, the release build that added Skia took 1 min 19 s (download
included); a release rebuild of dr-app alone takes about 51 s. The fat LTO build took 4 min 20 s. No build errors.
Skia's window surface on Windows is wgpu (GPU), with a CPU surface (softbuffer) as its fallback.

## Known gaps (M1)

- The game pictures are the fake SVG scenes from the mockups. M2 uses real screenshots.
- Key capture ("Change"), voice test, Refresh, Retake screenshot, Import settings and the Settings buttons do nothing yet.
- Overlays are previews inside the main window. Real overlay windows come in M4, after the overlay spike.
- No view-model layer yet: `src/fake.rs` fills the window directly. `src/vm/` starts in M2.
