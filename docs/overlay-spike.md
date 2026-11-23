# Overlay spike: Slint windows as in-game overlays

Written 2026-11-22. The spike that [app-plan.md](app-plan.md) schedules after M1. Code: `spikes/overlay`
([README](../spikes/overlay/README.md)). Slint 1.18.1, winit 0.30.13, windows 0.62.2, Windows 11 build 26300.
The pill and the quick menu are the real M1 components. Zone edit is a stand-in (a veil and a drag box), not
`crates/app/ui/overlays/zone_edit.slint`.

## Decision

**M4 draws all three overlays as Slint windows on winit**, with the three-part recipe below (attributes hook,
`harden` whenever the HWND is new, a window subclass). On a hidden desktop, every check that can run there passed
for the pill, the quick menu and zone edit: the styles stay (also after hide and show, for all three), the windows
never activate, the pill is click-through, and zone edit takes the mouse. Windows Graphics Capture of the pill
(off screen) shows that the window surface DWM holds has see-through margins and the drawn card.

**Renderer: femtovg or `skia-opengl`, not Skia's default.** Skia's default renderer on Windows (a wgpu surface,
with Skia software as its fallback) gives the margins as opaque black, so overlays would show black
boxes. If the app moves to Skia for text (spec section 6, question 8), it must ask for `skia-opengl`. One renderer
serves every window of the process.

The decision has one condition. Headless tests cannot see the final screen (see "Not verified"). Before M4
starts, Max runs one visible check (`demo`, below) over a borderless game, with his yes.
- If the pill corners are not see-through there, the pill moves to the fallback: the software renderer into a
  Win32 layered window (`UpdateLayeredWindow`).
- The quick menu and zone edit have **no composed-pixel evidence at all**. In their real styles, WGC reads every
  pixel as alpha 0, the card too (femtovg, software, and also with `WS_EX_LAYERED`). Their rounded corners,
  shadow margin and the zone edit veil rely on the same blur-behind transparency as the pill, and nobody has
  seen it on a screen. If the check shows black around the menu card or a black zone edit, they move to the
  fallback too. For the menu, a second way out is an opaque rectangular window with no shadow margin.

## Why plain Slint settings are not enough

- Slint gives `no-frame`, `always-on-top` and a transparent background. The winit backend always asks winit for a
  transparent window (`with_transparent(true)`), and winit makes it per-pixel transparent with
  `DwmEnableBlurBehindWindow` and an empty region.
- Slint has no setting for `WS_EX_NOACTIVATE`, `WS_EX_TOOLWINDOW` or click-through.
- **winit rewrites `GWL_EXSTYLE` from its own flags on every flag change**: show, hide, window level and
  others (`winit-0.30.13/src/platform_impl/windows/window_state.rs`, `apply_diff`). Bits it does not know are
  lost. Measured (`--naive`): after one hide and show, the pill went from `0x080801B8` to `0x00040118`. It lost
  `WS_EX_NOACTIVATE`, `WS_EX_TOOLWINDOW`, `WS_EX_LAYERED` and `WS_EX_TRANSPARENT`, gained `WS_EX_APPWINDOW`
  (a taskbar button), and stopped being click-through. The quick menu lost the same bits.
- winit shows a window with `SW_SHOW` (activating) from the second show on, unless the style says otherwise.
- The HWND exists only after the first `show()`, and only once the event loop has started. A `show()` before
  `run_event_loop`, or from a zero-delay timer at start, gives no HWND yet (measured: "no hwnd after show"); from
  a 300 ms timer it was there. So the old spec 2.7 rule "styles before it is shown" cannot be met; spec 2.7 now
  says "right after the first show" (changed 2026-11-22 with this spike). Setting them right after `show()`, in
  the same call, was enough: no activation was measured, and `WS_EX_APPWINDOW` was gone. The taskbar itself was
  not checked (a hidden desktop has none).
- Between the first `show()` and `harden`, the window has `WS_EX_APPWINDOW`: `ex_before_harden` was
  `0x00040118` in every run, also with `with_skip_taskbar(true)` (winit removes the taskbar button through
  `ITaskbarList`, not through the style). A taskbar button may flash for a moment on the first show; the visible
  check watches for it.
- The HWND is not forever. Slint's winit backend destroys the window on `suspend()`, and on `hide()` when
  `SLINT_DESTROY_WINDOW_ON_HIDE` is set (`i-slint-backend-winit-1.18.1/winitwindowadapter.rs`, lines 1724 to
  1731). The next `show()` makes a new HWND with winit's styles only. Today a plain hide and show keeps the HWND
  (measured for all three overlays, and the test asserts it).

## The recipe for M4

1. **Attributes hook.** It runs when a component is created (`Component::new`), not at show. A thread-local
   flag marks overlay windows, so the main window keeps the default attributes.

   ```rust
   slint::BackendSelector::new()
       .backend_name("winit".into())
       .renderer_name("femtovg".into())
       .with_winit_window_attributes_hook(overlay::attributes_hook)
       .select()?;

   pub fn attributes_hook(a: WindowAttributes) -> WindowAttributes {
       if NEXT_IS_OVERLAY.with(Cell::get) {
           // first show uses SW_SHOWNOACTIVATE; ITaskbarList removes the button at creation
           a.with_active(false).with_skip_taskbar(true)
       } else {
           a
       }
   }
   let pill = overlay::new_overlay(PillWindow::new)?; // sets the flag around new(), cleared by a drop guard
   ```

   Only the component's own window gets the overlay attributes. Windows Slint makes later for it
   (`PopupWindow`, `ComboBox` or context menu popups) go through the hook without the flag and get default
   attributes. **Overlays must not use popups.** The M1 overlay components use none today.

2. **`harden` after every `show()` that gives a new HWND** (`overlay::show_overlay` does it: show, read the
   HWND, harden when it is not marked yet). Keying "harden once" on a bool would leave a new HWND naive. HWND
   from winit's raw window handle:

   ```rust
   w.with_winit_window(|ww| match ww.window_handle().ok()?.as_raw() {
       RawWindowHandle::Win32(h) => Some(HWND(h.hwnd.get() as *mut _)),
       _ => None,
   }).flatten()
   ```

   Then `SetWindowSubclass`, OR the wanted bits into `GWL_EXSTYLE` (and remove `WS_EX_APPWINDOW`),
   `SetLayeredWindowAttributes(hwnd, 0, 255, LWA_ALPHA)` for the pill (Microsoft's docs: a layered window is
   not shown until it gets `SetLayeredWindowAttributes` or `UpdateLayeredWindow`; WGC saw the pill's content
   with and without it, so this was not measured), `SetWindowPos(HWND_TOPMOST, ... SWP_NOACTIVATE | SWP_FRAMECHANGED)`,
   and a window property that marks the HWND as hardened. `harden` returns a `Result`: every call is checked,
   because a failed `SetWindowSubclass` would lose the styles on the next show with no other sign. The caller
   logs it (or stops). Do not use `GetWindowSubclass` to test for the mark: with it imported, the exe failed at
   start with `STATUS_ENTRYPOINT_NOT_FOUND` (0xC0000139, measured; it started fine without it). The likely cause
   is that the comctl32 this exe loads (no v6 manifest) does not export it by name; that part is not checked.

   | Overlay | Extra styles |
   |---|---|
   | Pill | `TOPMOST`, `NOACTIVATE`, `TOOLWINDOW`, `LAYERED`, `TRANSPARENT` |
   | Quick menu, zone edit | `TOPMOST`, `NOACTIVATE`, `TOOLWINDOW` |

3. **The subclass** keeps it that way:

   ```rust
   WM_STYLECHANGING if wparam.0 as i32 == GWL_EXSTYLE.0 => {
       let ss = &mut *(lparam.0 as *mut STYLESTRUCT);
       ss.styleNew = (ss.styleNew | want) & !WS_EX_APPWINDOW.0;
   }
   WM_MOUSEACTIVATE => return LRESULT(MA_NOACTIVATE as isize),
   WM_WINDOWPOSCHANGING => (*(lparam.0 as *mut WINDOWPOS)).flags |= SWP_NOACTIVATE,
   WM_NCDESTROY => { RemovePropW(...); RemoveWindowSubclass(...) }
   ```

   Without the subclass, `WM_MOUSEACTIVATE` answers `MA_ACTIVATE` (1). With it, it answers `MA_NOACTIVATE` (3).

Other rules for M4:
- **Zone edit is not a fullscreen window.** winit drops `SWP_NOACTIVATE` for fullscreen windows. Size a
  frameless window to the game's client rect (`set_position` and `set_size` in physical px) instead.
- Place the quick menu after `show()`: its size is known then. `set_position` goes through winit's
  `SetWindowPos(... SWP_NOACTIVATE)`.
- Hide and show overlays with Slint's `hide()` and `show()` (through `show_overlay`). With the subclass that is
  safe (tested for all three).
- **Low-level keyboard hook** (`WH_KEYBOARD_LL`) on **its own thread with its own message loop**, not on the
  Slint UI thread. Windows waits for a low-level hook on every key on the PC: on the UI thread, every render,
  GL window creation or snapshot would delay all keyboard input, and Windows silently removes a hook that
  answers too slowly. The callback only reads atomic flags ("menu open", "zone edit open"), sets them for
  F8 / F9 / Esc at once (so the next key is already judged right), and posts the work to the UI thread with
  `slint::invoke_from_event_loop`. `spikes/overlay/src/keys.rs` and `demo.rs` do this.
- **Key-up rule:** a key belongs to whoever got its first key-down, until it is released. Its auto-repeats and
  its key-up go the same way (`keys::Swallow`, unit tested). So after Esc closes the menu on its key-down, the
  game does not get the Esc key-up, and a key held before the menu opened still reaches the game.
- **Injected keys** (`LLKHF_INJECTED`, from `SendInput`: Steam Input, AutoHotkey, other pad mappers, game
  streaming) are treated like real keys in the spike, by choice: a player who maps a pad button to the menu key expects it to work. The event
  carries the flag, so M4 can change this in one place. Open question for Max. If DialogReader ever sends keys
  itself (for example a media pause key), its hook must not act on those.
- Harden an overlay when its HWND exists, not on a fixed first call: `show()` before the loop has started
  creates nothing yet.
- The overlay margin around a card must be at least its shadow blur. The quick menu's shadow blur is 40 px and
  the spike used 24 px: Skia drew alpha 3 in the corner pixel. Use 40 px.
- One renderer serves every window of the process: femtovg (today) or `skia-opengl`. Not Skia's default.
  `skia-opengl` can be picked by name only when the winit backend has OpenGL (the `renderer-femtovg` or
  `renderer-skia-opengl` feature); if M4 drops femtovg, it must turn on `renderer-skia-opengl`.

## Evidence

All runs on a hidden desktop (`CreateDesktop`, never switched to), with a dummy game window in a separate
process that holds activation. `cargo test -p overlay-spike` runs it (`tests/hidden_desktop.rs`, about 15 s;
passed 5 times in a row after the review fixes). The runner waits until the game window exists on the
hidden desktop; the probe's steps still run on a 400 ms timer, so a very loaded PC could make them flaky.

**How focus is measured.** On a desktop that is not the input desktop, `GetForegroundWindow` is always NULL,
so "foreground unchanged" proves nothing there. The probe uses activation per thread instead: a thread-wide
`WH_CALLWNDPROC` hook counts `WM_ACTIVATE` (active) and `WM_SETFOCUS` for every window of the UI thread from
before the first show, and `GetGUIThreadInfo` gives the active window of the UI thread and of the game thread.
**Positive control:** a plain Slint window shown last gets 1 `WM_ACTIVATE`, 1 `WM_SETFOCUS` and becomes the
active window. So the check can fail, and for the overlays it does not. The game thread's active window is
**no** evidence: it stays the game even while the control window activates (a window of another process does
not change it on this desktop), so it is recorded for the control only and not checked.

Results with the subclass (femtovg; skia and skia-opengl gave the same focus and style results):

| Check | Pill | Quick menu | Zone edit |
|---|---|---|---|
| All wanted ex-styles after first show | yes (`0x080801B8`) | yes (`0x08000198`) | yes (`0x08000198`) |
| Still there after hide + show | yes | yes | yes |
| Same HWND after hide + show | yes | yes | yes |
| HWND marked as hardened (`is_hardened`) | yes | yes | yes |
| Still there after a property change | yes | - | - |
| `WS_EX_APPWINDOW` (taskbar button) | no | no | no |
| `WM_ACTIVATE` / `WM_SETFOCUS` received | 0 / 0 | 0 / 0 | 0 / 0 |
| Became the UI thread's active window | no | no | no |
| `WindowFromPoint` at its center hits | the game (click-through) | the menu | zone edit |
| `WM_MOUSEACTIVATE` answer | - | 3 (`MA_NOACTIVATE`) | 3 |
| Layered attributes | alpha 255, `LWA_ALPHA` | - | - |

- Quick menu keys: Down, Down, Enter through the hook's handler (`keys::feed`, posted to the UI thread) gave
  highlight 2 and chose row 2, with 0 activations. The hook thread installed the hook and saw 0 real keys on
  the hidden desktop. `feed` calls the handler directly: `hook_proc` itself (decode, `Swallow`, `CallNextHookEx`)
  never ran in a test, because no real key reaches a hidden desktop. `Swallow` and `decode` are unit tested.
- `show_overlay` on the already hardened pill showed it again without a second `harden` (same HWND); the
  plain control window has no mark.
- Zone edit: a drag with posted mouse messages from (100, 100) to (300, 250) drew one zone, rect
  (100, 100, 200 x 150) in physical px, with 0 activations.
- Renderer: femtovg gave an OpenGL pixel format on the window (12), so it did not fall back to software.
  Skia's default has no GL pixel format; skia-opengl has one. In `i-slint-renderer-skia` 1.18.1 (`lib.rs`,
  `create_default_surface`) the default tries a wgpu surface first (wgpu picks the graphics API), then software.
  Pixel format 0 fits both, so which one drew the black margins is not known.
- Renderer output (`Window::take_snapshot`): the pill margin has alpha 0 and the pill body alpha 233 (the
  0.86 pill background). The quick menu margin has alpha 0 and the card alpha 255.

**Window surface as DWM holds it** (`overlay-spike compose`, normal desktop, windows left of every monitor,
one WGC frame each). Off screen, WGC only gets pixels from windows with `WS_EX_TRANSPARENT` (without it every
pixel came back alpha 0, the card too), so `--menu-like-pill` gives the menu the pill's styles for this
measurement only.

| Renderer | Pill margin | Pill body | Menu margin | Menu card | Pixels with alpha 0 (pill) |
|---|---|---|---|---|---|
| femtovg | alpha 0 | (6, 8, 12) alpha 233 | alpha 0 | alpha 255 | 67% |
| skia-opengl | alpha 0 | alpha 233 | alpha 0 | alpha 255 | 67% |
| software | alpha 0 | alpha 219 | alpha 0 | alpha 255 | 72% |
| Skia default (wgpu surface or software) | **opaque black** | alpha 255 | **opaque black** | alpha 255 | 0% |

The foreground window was the same before and after in all `compose` runs but one (femtovg, the first of a
batch). Four reruns that also record the new front window never repeated it, and the windows have
`WS_EX_NOACTIVATE`. A reviewer saw the same once in 6 runs (software, `--menu-like-pill`): the new front window
was not ours and not of our process. Most likely Max switched windows during those runs, but that is not proven.

## Not verified (needs the visible check)

- **The final screen.** WGC shows the window surface with the right alpha, but not what ends up on the screen
  over a game. On the hidden desktop there is nothing to read the screen from:
  - `PrintWindow(PW_RENDERFULLCONTENT)` gives opaque black for the GL windows (femtovg, skia-opengl), even where
    the card is drawn. For the software renderer and Skia (not GL) it gives the card, but the margin comes back opaque black
    (alpha 255 everywhere), so it says nothing about see-through pixels either.
  - The screen DC of a hidden desktop cannot be copied (`BitBlt` fails).
  - WGC refuses windows on another desktop (E_INVALIDARG, docs/arena-spec.md).
  - Off screen on the normal desktop, WGC reads only windows with `WS_EX_TRANSPARENT` (table above). The quick
    menu and zone edit in their real styles (no `WS_EX_TRANSPARENT`) could not be read at all.
  The surface has the right alpha, so the pill is very likely shown see-through. But the mix of a layered window
  (`LWA_ALPHA`) with OpenGL and blur-behind transparency has not been seen on a screen yet.
- **A real mouse click** on the menu or zone edit. `WS_EX_NOACTIVATE` and `MA_NOACTIVATE` are what Windows uses
  to refuse activation on a click, but no real click was sent (`SendInput` from a hidden desktop could reach
  Max's desktop).
- **Real key delivery** to the hook while a game has focus (same reason).
- **A real game**: exclusive fullscreen, games that pause on focus loss, mixed DPI.

## Visible check for Max (one time, with his yes)

```
cargo run -p overlay-spike --release -- demo
```

Click into a borderless game within 5 seconds. The demo stops with an error if the window in front has no
client area or is its own. Then:
1. The console prints "all overlay styles set" for the pill. The pill shows at the top right of the game.
   Its corners show the game, not black. No taskbar button flashes when it first shows.
2. A click on the pill reaches the game.
3. F8 opens the quick menu in the middle. The area around the menu card (its corners and shadow margin) shows
   the game, not black. Arrows move, Enter chooses, Esc closes. The game does not pause or minimise, and the
   console keeps saying "Game in front".
4. After closing the menu with Esc, the game does not react to Esc (no pause menu opens).
5. F9 opens zone edit over the game. The game shows through the veil (dimmed, not black). Dragging draws a box.
   A click does not take focus from the game. F8 while zone edit is open swaps to the menu (only one is open).
6. The taskbar has no extra button besides the console that runs `demo` (the exe is `overlay-spike`, not
   DialogReader), and Alt+Tab does not list "DR pill", "DR quick menu" or "DR zone edit".
7. F10 quits. The last line gives the seconds the game was not in front (should be 0).

If 1 or 2 fails, the pill moves to the fallback. If 3 or 5 shows black, the menu or zone edit moves to the
fallback too (or, for the menu, an opaque rectangle window). If 4 fails, the key-up rule is wrong. If the game
loses focus or 6 fails, the decision changes and this file is updated. The `demo` mode compiles but was never
run (it shows windows on the real screen); its hook thread and key-up rule have not run with real keys either.

## Visible check result (2026-11-23, run by Claude with Max's yes)

Max handed the PC over ("pc is yours"), so Claude ran `demo` itself against a stand-in game: a borderless 3840 x 2160
Tk window with a bright checker pattern that logs every key, click and focus change. Input came from
`keybd_event` / `mouse_event`. Script: session scratchpad `overlay/drive.py` and `fakegame.py` (not kept).

| Step | Result |
|---|---|
| 1 Pill | Pass. "all overlay styles set" (ex-style 0x080801B8). Corners show the game, no taskbar flash. |
| 2 Click through | Pass. A click on the pill reached the game (game log: click at 3614,110), game stayed in front. |
| 3 Quick menu (F8) | Pass. Menu centered, corners and shadow margin show the game. Arrows moved, game kept focus. |
| 4 Esc after menu | Pass. The game got no F8, arrow or Esc key at all. |
| 5 Zone edit (F9) | Pass. Game dimmed, not black; a drag drew a box; the drag did not reach the game; F8 swapped to the menu. |
| 6 Taskbar, Alt+Tab | Pass. Every demo window has WS_EX_TOOLWINDOW, no WS_EX_APPWINDOW. |
| 7 F10 | Pass. "Seconds the game was not in front: 0", exit code 0. |

Seen: while the menu or zone edit is open, the pill shows at the top center instead of the top right (the demo
moves it; check the wanted place in M4). Not covered: a real DirectX game (flip model, raw input, games that pause
on focus loss). Try one in the M4 manual check.

## For the M4 overlay test

`crates/app/tests/overlay_win.rs` (M4) should reuse the probe's way of checking: the hidden desktop runner
(`win::run_hidden`), the dummy game in a separate process, activation counted per thread (not
`GetForegroundWindow`), and the plain-window positive control.
