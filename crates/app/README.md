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
```

## Design rules in code

- All colors, sizes, shadows, letter spacing, line heights and font weights come from `ui/theme.slint`. No page
  writes a color by hand. Colors with alpha are 8-digit hex. In `@markdown` text, write `\{Palette.text-2}`.
- Icons are white SVG files drawn with `colorize`. `<name>.svg` is drawn for 16 px, `-sm` for 12 px, `-lg` for 22 px,
  so the line is as thin as in the mockups. Kind icons are always grey.
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
  Open question for Max: docs/app-spec.md, section 6.

## Known gaps (M1)

- The game pictures are the fake SVG scenes from the mockups. M2 uses real screenshots.
- Key capture ("Change"), voice test, Refresh, Retake screenshot, Import settings and the Settings buttons do nothing yet.
- Overlays are previews inside the main window. Real overlay windows come in M4, after the overlay spike.
- No view-model layer yet: `src/fake.rs` fills the window directly. `src/vm/` starts in M2.
