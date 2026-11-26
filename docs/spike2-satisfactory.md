# Spike 2 result: Satisfactory (2026-11-21)

Recording: `recordings/satisfactory-20261121-183704` (local only, 20 GB, 11 marks, 4K, 43 fps average).
Measured by 4 Sonnet agents frame by frame (about 100 ms resolution). Raw per-mark output: workflow wf_688bcdd7-c0c.

## Capture
- Windows Graphics Capture works on Satisfactory (Unreal, borderless 4K). 7 black frames (loading), no size changes.
- The recorder's 10 fps lossless buffer dropped frames at 4K (72 dropped, gaps up to 1.2 s inside marks).
  QOI-encoding full 4K frames is too slow for one thread. The real reader must work on cropped zones, not full frames.

## Pop-up types

| Type | Position (4K px) | On screen | Fully readable | Animation |
|---|---|---|---|---|
| Pickup toast ("+ 9 Leaves (9)", "+ 1 Iron Ore (21)") | bottom left, about [40,1270,800,1480], always the same spot | 2.7 to 3.0 s | about 2.6 to 2.8 s | fade in 100 to 200 ms, fade out and slide down 100 to 200 ms |
| Repeated pickup while mining ("+ 6 Iron Ore (27)" then "+ 12 Iron Ore (33)") | same spot | 1.0 s each, replaced in place | 1.0 s | number changes, panel stays |
| Interaction prompt ("Press E to pick up Leaves", "Press E to start mining Iron Ore (Impure)") | centre, about [1470,1190,2370,1290] | 0.1 to 5 s, follows the crosshair | same | hard cut on and off, flickers for 1 to 3 frames |
| Build hint bar ("Dismantle / Hold LMB ...") | bottom centre [1180,1745,2660,1870] | about 0.4 s | 0.1 to 0.3 s | boxes fade in left to right |
| Menus and search (pause menu, emote wheel, Quick Search) | various | while open | | UI the player is using |

Always visible (ignore): version string top left, "Objective: Build the HUB" top right, compass letters.

## What this means for the design
- Pickup toasts sit in one fixed spot, so a "notification" zone (direction A1) is enough here. Whole-window detection is not needed for this game.
- The old tool fails on all of them: it needs about 0.6 to 0.9 s of identical pixels plus OCR time. Toasts fade and slide, and mining toasts change every 1.0 s.
- The old dedup would treat "+ 6 Iron Ore (27)" to "+ 12 Iron Ore (33)" as jitter. New dedup must treat a changed number as a new event.
- Interaction prompts flicker as the crosshair moves. They need a debounce (for example 300 ms stable) or a per-zone "read prompts: off" choice.
- Text sits on semi-transparent panels over moving grass. Orange numbers have low contrast. This is the OCR benchmark's hard case.
- Menus the player opens on purpose (pause, wheel, search) should not be read by default.
