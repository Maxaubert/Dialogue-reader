# UI design

Mockups for the new DialogReader UI (Slint). Static HTML at 1440x900, no JS, only what Slint can draw.

## Direction: Midnight (`mockups/c-midnight`)
Chosen by Max on 2026-11-22. The pages are built by `_build.py` from `tokens.css`. Screenshots are in `shots/`.

| Page | Status |
|---|---|
| 01 welcome, 02 pick window, 05 voice, 06 mode, 12 settings, 13 overlay pill, 14 game switcher | approved as first made |
| 03 draw zones, 11 zone editor | approved after changes: grey kind icons, no colored tags, no "Text found now" panel |
| 04-read-key-d "Your keys" | chosen for setup and Settings: 2 keys only, Read zones now (Mouse 4) and Quick menu (F7) |
| 04-read-key-c (mouse drawing) | not used |
| 15-quick-menu-a, 15-quick-menu-voice | approved: centered in-game quick menu |
| 16-zone-edit | approved: in-game zone editing from the quick menu |
| 10-main-b | chosen main page (overview), 2026-11-22. 10-main-a and 10-main-c not used |

## Rules Max set (2026-11-22)
- Clean, uniform, sleek, modern, easy to navigate. No themes or gimmicks.
- Zone kinds are shown with one grey line icon: Dialogue (speech bubble), Speaker (person), Pickups (box), Prompts (keycap), Ignore (eye-off). No colored dots, pills or labels.
- Zone kinds carry no color anywhere (2026-11-22): the zone boxes on 01, 03, 10-main-b and 11 are grey, and the Kind picker on 03 and 11 shows the kind icons. Pages marked `.kgrey` in `_build.py`; notes at the end of `tokens.css`.
- Keys: one read key and one quick menu key. No other hotkeys.
- Quick menu: centered. Order: Manage zones, Stop (play/pause toggle), Mute, Voice, Speed, Volume, Mode, Settings, Quit (press twice).
- Zone edit: drag on empty space adds a zone, click selects it, Delete/Backspace or the small x removes it, Esc finishes.
- Feedback on named screens changes only those screens.

## Other material
- `gpt-image/`: gpt-image-2 concepts (v01 to v10 main window, s* screens in the v06 style, qm-* quick menu). Prompts in the `.txt` files.
- `mockups/a-graphite`, `mockups/b-porcelain`, `mockups/v06`: alternative sets, not chosen. Kept for ideas.
