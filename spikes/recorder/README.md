# Spike 2: recorder

Records a game window with Windows Graphics Capture (WGC). It answers:
- Does WGC capture this game (no black frames, real frame rate)?
- How long does pop-up text (pickups, tooltips) stay on screen, and where does it appear?

It also gives lossless screenshots for the OCR benchmark (spike 1).

## Build

```
cargo build --release
```

## Use

1. Start the game, borderless or windowed if possible.
2. Run from the repo root:
   ```
   target\release\recorder.exe --name rdr2
   ```
   Switch to the game within 5 s. Or pick the window directly: `--process RDR2.exe`
   (`--list` shows the names).
3. Play. Each time a pop-up or new dialogue line appears, press the **|** key (left of 1 on a Nordic keyboard).
   The tool saves lossless PNGs from 8 s before to 3 s after the press.
4. Press **F10** to stop (or Ctrl+C in the console). Closing the console window also saves
   everything; the mark frames then stay as `.qoi` until you run `recorder --convert "<session folder>"`.

Keys can be changed with `--mark-key` and `--stop-key`: pipe, scrolllock, pause, insert, home, f8 to f12,
mouse4, mouse5 (side buttons), or any virtual-key code such as `0xDC`.
If the game runs as admin, the keys may not reach the recorder. Then run the recorder as admin too.

## Output

`recordings/<name>-<date>-<time>/` (git-ignored):

| File | What |
|---|---|
| `video_001.mp4`, ... | H.264 of every captured frame. A new file starts when the frame size changes |
| `frames.csv` | Every frame: time, raw clock ticks, gap since last frame, size, brightness of sampled frames, time spent in the capture callback (`cb_ms`) |
| `marks.csv` | Time of each key press |
| `marks/mark_NNN/*.png` | Lossless frames around each mark, named by offset from the press |
| `session.json`, `stats.json` | Window, settings, frame rate, longest gap, black frames, dropped frames |

Use `frames.csv` and the mark PNGs for timing, not the video: the encoder can lag behind.
If `cb_ms` is often longer than the frame gap, the recorder itself is slowing capture (lower `--fps`).
The first frame's `cb_ms` is high because the video encoder starts then.

Memory: the lossless buffer is capped by `--ring-mb` (default 2000). At 4K, lower `--pre` or `--fps`
if `ring_frames_pruned_for_memory` in stats.json is above 0.

WGC only sends a frame when the window changes, so gaps in a static menu are normal.
