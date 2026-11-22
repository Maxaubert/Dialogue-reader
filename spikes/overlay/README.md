# Overlay spike

Checks if Slint 1.18.1 windows on winit can be DialogReader's in-game overlays: the status pill, the quick menu
and zone edit. Findings and the decision: [docs/overlay-spike.md](../../docs/overlay-spike.md).

The pill and the quick menu use the real M1 components (`crates/app/ui/overlays/pill.slint`,
`quick_menu.slint`). Zone edit is a stand-in (a veil and a drag box), not `zone_edit.slint`.
`src/overlay.rs` (and the hook thread and key-up rule in `src/keys.rs`) is the part M4 can take over.

## Modes

One binary, `overlay-spike`:

| Mode | What it does | On screen? |
|---|---|---|
| `run [--renderer femtovg\|skia\|skia-opengl\|software] [--naive] [--no-layered-alpha] [--layered-all]` | Makes a hidden desktop, starts `game` and `probe` there, prints the probe's JSON report | no |
| `game --secs N` | Dummy game: a plain green window that activates itself | (used by `run`) |
| `probe --out FILE ...` | Opens each overlay next to the game and measures styles, activation, hit tests, pixels | (used by `run`) |
| `compose [--renderer NAME] [--menu-like-pill]` | Normal desktop, overlays left of every monitor, one WGC frame each | no (off screen) |
| `demo [--renderer NAME]` | Visible manual check over a real game (F8 menu, F9 zone edit, F10 quit) | **yes**, only with Max's yes |

`--naive` sets the styles once without the subclass, to show that winit wipes them. `--layered-all` makes the
menu and zone edit layered too. `--menu-like-pill` gives the menu the pill's styles in `compose`, because off
screen WGC only reads windows with `WS_EX_TRANSPARENT`. `skia` and `skia-opengl`
need `--features skia` (off by default, so the workspace build of dr-app keeps its features).

## Run

From the repo root:

```
cargo run -p overlay-spike -- run
cargo test -p overlay-spike
```

`tests/hidden_desktop.rs` runs the probe twice on hidden desktops (about 15 s). Nothing shows on screen and
the hidden desktop is never switched to.
