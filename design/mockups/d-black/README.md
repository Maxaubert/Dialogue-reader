# d-black mockups

The redesign mockups for DialogReader, made on 2026-11-24.
The brief is `research/dialogue-reader/2026-11-24-redesign-brief.md`.
The layout comes from `../c-midnight`. The look, the text and the motion are new.
Round 2 applied the design review. Round 3 applied Max's decisions below and added every remaining screen. Round 4 (this round) applies the design director's review: see "Round 4 changes".

Open the files in Zen. Every page is drawn at the app window size, 1440 x 900.
Mockup controls are below the window frame. Scroll down to find them.

## Decisions (Max, 2026-11-24)

- **Direction approved.** "Yes, continue."
- **Live accent is lime `#C5FA09`, not teal.** It is used only for the status dot, the waveform and the active zone outline (and the pill dot, the in-game form of the status dot). `tokens.css` now has lime as `--live`. The teal option is gone (round 4).
- **Long lines in the read list: two lines, then cut.** Hover a cut line for 200 ms and the full row lies over the rows below (fade 120 ms). This is in the Overview list, the Activity list and the token sheet.
- **Page push: slight overlap.** The old page fades out while the new page slides in, 80 ms apart. Forward: out 100 ms `ease-in`, in 260 ms `ease-out-strong` after 80 ms. Back: out 120 ms, in 200 ms after 80 ms. This replaces the round 2 sequential push (where the delay equalled the exit time). The setup step slide and the Activity push use the same timing.
- **Round 4 answers (2026-11-24).** Mockup set approved for the Slint port. Switch game is 240 ms in total: drop the 80 ms pause after a pick (the mockup still plays 320 ms; the port uses 240). Welcome stays top-aligned with the steps. The not-running stage keeps one button, "Switch game". The lime dot stays on an empty Activity page. The keycap font is decided after the first Slint render. Old > new text: `TEXT.md`.

## Files

| File | What it is | What to look at |
|---|---|---|
| `tokens.css` | The shared tokens from brief section 1.2 | Colours, type, shape and motion values |
| `00-tokens.html` | The token sheet | Greys, white for "on", lime, the zone chip, toggle, keycap, rows (with a wrapped row), panel, motion |
| `01-setup.html` | Welcome and the 5 setup steps in one file | Get started, then Continue and Back. Watch the step slide, the footer dots and Finish (the setup turns into the Overview). |
| `10-overview.html` | The Overview, the home screen | "Game not running" is a toggle below. Hover the cut Hosea line. |
| `11-zones.html` | Zones, the stage-grow target | The merged toolbar. Drag on the picture to add a zone, click to select, drag to move, handles to resize, Delete to remove. "Play enter" replays the stage grow. `?state=top` shows a box at the top edge. |
| `12-settings.html` | Settings, one column, no sidebar | Unchanged this round, apart from lime |
| `13-pill.html` | The in-game status pill | "Play a line" shows the show, hold and hide motion. Top right or bottom, with or without the speaker name. |
| `14-game-switcher.html` | The game switcher (Ctrl K) | Type to search, arrows glide the highlight, Enter switches and the game name rolls. States: open, search, no match, closed. |
| `15-quick-menu.html` | The in-game quick menu | F7 opens and closes it. Arrow keys, Enter, Esc. Voice opens the voice list beside it. Stopped and "Quit pressed once" states. |
| `16-zone-edit.html` | Zone edit over the live game | The shade, the outlines and the top bar fade in. Drag a new box: the kind picker pops under it. Drag a box up under the bar: the bar moves to a place clear of every zone, and back home when home is clear. Done or Esc fades it all out. `?s=top` shows that state. |
| `17-activity.html` | Activity, pushed from "Show all" | The Overview list style at 720 px. All, Read, Skipped. Time on the right, replay on hover. Empty state. |
| `20-motion.html` | The Overview with every push wired | Settings, Zones, Activity, the switcher and the new-line motion |
| `_build.py` | Builds the HTML files (`python _build.py`) | Edit this file, not the HTML |
| `shots/` | Headless Chromium renders, 1440 x 900 | See the list below |

Shared parts are one Python function or one script each, used by every page that shows them: the switcher (`switcher()`, `SW_JS`), the Zones toolbar and editor (`zn_toolbar()`, `ZONE_JS`), the Activity list (`activity_body()`, `AC_JS`), list rows (`lrow()`, `LIST_JS`), keycaps (`KEYCAP_JS`) and the motion core (`MCORE`). The Zones rectangle is defined once (`R_ZN`) and used by `11-zones` and `20-motion`.

## What to try in 20-motion.html

- Click the gear. The Overview fades out and Settings slides in from the right, 80 ms later. Esc or the back chevron returns.
- Click the stage or the pencil. The stage grows into Zones, which is now the real Zones page: the same toolbar and editor as `11-zones.html`. Done, Esc or back shrinks it again.
- Click "Show all". The Overview fades, Activity rises from 24 px below and its first 6 rows stagger 30 ms. Esc or back returns.
- Click the game name, or press Ctrl K. The switcher is the real one from `14-game-switcher.html`: type to search, arrow keys, Enter. Pick "Satisfactory" to see the switch-game motion. The Zones page follows the game.
- Click "Read now" to make a new line arrive. Rows are one or two lines tall, so the rows below move down by the new row's own height.
- Click a zone chip under the stage. It turns reading of that kind on or off, and the outlines dim with it.
- Click Pause, "Adam" or "Mouse 4" as before. The mode option now switches between "Automatic" and "On key press".
- Add `?test=1` to the URL. A self-test clicks through all of the above (38 checks) and prints PASS or FAIL under the frame.
- Add `?s=settings`, `?s=zones`, `?s=activity` or `?s=switcher` to jump to a state. Add `-mid&ms=N` to freeze a transition at N ms. `?from=zones` or `?from=activity` plays the way back to the Overview (Done and back on `11` and `17` link here).

## Motion in the new files (from brief section 3, with the decided page-push timing)

| File | Transition | What moves |
|---|---|---|
| `01-setup` | 1 and 2, step slide | Old step out 100 ms `ease-in`, 16 px away. New step 80 ms later, from 24 px, 260 ms `ease-out-strong`, its parts 40 ms apart (at most 6). Back reverses the direction and uses the back push: out 120 ms, in 200 ms after 80. The active dot widens 8 to 20 px, 260 ms. The footer fades in on step 1 and out on Welcome. |
| `01-setup` | 3, Finish | Setup and footer out 140 ms `ease-in`. 80 ms later: stage opacity and scale 0.98 to 1, then the top controls, the Now panel and the zone strip, 40 ms apart, 320 ms `ease-out-strong`. The brand swaps for the game name, and the dot and gear fade in. |
| `11-zones` | 4, Zones end | Stage rect from the Overview rect to the Zones rect, 300 ms `ease-out-strong`. Toolbar and tags fade in 120 ms later, 180 ms. |
| `14-game-switcher` | 7 and 8 | Scrim 0 to 0.6, panel opacity, y -8 to 0, scale 0.98 to 1, 180 ms `ease-out`; close 140 ms `ease-in`. Highlight glide 360 ms (mouse) or 120 ms (keys). Name roll: out 100, in 180. |
| `17-activity`, `20-motion` | 6 | Activity from y +24 with a fade, 260 ms `ease-out-strong`, 80 ms after the Overview starts to fade. Rows stagger 30 ms; rows after the 6th share the 6th row's delay. The last row ends 490 ms after the click. |
| `13-pill` | 9 | Show 180 ms `ease-out`, opacity and y 6 to 0. Hold 600 ms after the line, hide 200 ms `ease-in`. A new line during the hold only crossfades the name. |
| `15-quick-menu` | 10 | Open 160 ms `ease-out`, opacity and scale 0.97 to 1 from the top centre. Close 120 ms `ease-in`. The voice list: opacity and x -8 to 0. |
| `16-zone-edit` | 11 | Shade 180 ms, outlines and tags 180 ms after 60 ms, top bar 180 ms, all `ease-out`. Kind picker opacity and scale 0.96 to 1, 140 ms. Bar moving out of the way: out 140 ms `ease-in`, in at the new place 180 ms `ease-out`, y 8 px. Out: 140 ms `ease-in`. |

Where the brief differs: the setup step slide and the Activity push use the decided page-push timing (out 100 or 120 ms, not 140 or 200, with the 80 ms overlap), and the Activity stagger is capped at the 6th row. Every interactive page (01, 11, 13 to 17, 20) has "Slow motion x4" below the frame. Reduce motion works on every page: movement jumps, fades take 80 ms.

## Shots

All from headless Chromium (Playwright), 1440 x 900. Nothing was checked in Zen.

- Tokens: `00-tokens.png` (tall)
- Setup: `01-setup-0-welcome.png`, `01-setup-1-game.png`, `01-setup-2-zones.png`, `01-setup-3-keys.png`, `01-setup-4-voice.png`, `01-setup-4-voice-none.png`, `01-setup-5-mode.png`, `01-setup-done.png`; frozen: `01-setup-1-mid.png` and `01-setup-2-mid.png` at 140 ms, `01-setup-done-mid.png` at 160 ms
- Overview: `10-overview.png`, `10-overview-off.png`, `10-overview-hover-full.png` (full text of a cut line), `10-overview-bright.png` (outlines on a bright stand-in picture)
- Zones: `11-zones.png`, `11-zones-empty.png`, `11-zones-top.png` (box at the top edge, tag under it), `11-zones-bright.png` (bright picture, the off Prompts zone selected), `11-zones-enter-mid.png` at 120 ms
- Settings: `12-settings.png`, `12-settings-full.png`
- Pill: `13-pill.png`, `13-pill-bottom.png`, `13-pill-show-mid.png` at 60 ms
- Switcher: `14-game-switcher.png`, `14-game-switcher-filter.png`, `14-game-switcher-empty.png`, `14-game-switcher-open-mid.png` at 60 ms
- Quick menu: `15-quick-menu.png`, `15-quick-menu-voice.png`, `15-quick-menu-stopped.png`, `15-quick-menu-quit.png`, `15-quick-menu-open-mid.png` at 50 ms
- Zone edit: `16-zone-edit.png`, `16-zone-edit-new.png` (new box with the kind picker), `16-zone-edit-top.png` (box under the bar: the bar moved to the top right), `16-zone-edit-bright.png`, `16-zone-edit-show-mid.png` at 90 ms
- Activity: `17-activity.png`, `17-activity-hover-full.png`, `17-activity-empty.png`, `17-activity-enter-mid.png` at 120 ms
- Motion: `20-motion.png`, `20-motion-settings.png`, `20-motion-zones.png`, `20-motion-switcher.png`, `20-motion-activity.png`; frozen: `20-motion-settings-mid.png` at 120 ms, `20-motion-settings-back-mid.png` at 150 ms, `20-motion-zones-mid.png` at 120 ms, `20-motion-activity-mid.png` at 160 ms, `20-motion-newline-mid.png` at 90 ms

If the three weights in the font-check row of `00-tokens.html` look the same in Zen, the Segoe UI Variable font did not load there.

## Choices made in round 3 (check these)

- **One word for the mode: "On key press"**, everywhere: the setup card, the Overview option, the quick menu. The brief (4.2) shortens it to "Key" in the quick menu only, but rule 4.1.3 asks for the same word everywhere, and "On key press" fits in the menu.
- **Quick menu keeps a small status** in its header ("Reading" with the lime dot, "Stopped" with a grey dot). The game has no title strip, so this is the only place the status shows in the game.
- **Quick menu has no key-hint footer** (brief 4.1.5: one place for key hints, the switcher search field).
- **Zones: dragging on empty picture always draws.** "Draw zone" only matters for drawing on top of another zone. The kind buttons show the selected zone's kind, or the kind for the next box when nothing is selected. Delete is disabled when nothing is selected.
- **Zone edit (16) has a small delete button** at the end of the selected box's tag (round 4: it used to sit on the corner, over a resize handle), because the hint no longer says "Delete to remove".
- **Setup step 1** has no resolution or exe name, and selection is shown only by the fill. Running and Other are two panels with the label above each (round 4), the fill glides inside a panel and crossfades between them. **Step 4** puts Speed and Volume beside the voice list, with a play button on each voice row. **Step 5** cards show selection with the white border and the surface-2 fill only. The summary line is "5 zones · Mouse 4 · Adam 1.1x" and follows what you picked.
- **Activity** shows the time at the row end (replay covers it on hover). Skipped rows are dimmed with an "off" or "repeat" tag before the time, so the time column stays straight. The filter is All, Read, Skipped.
- **Welcome** keeps the three step titles as a numbered list and a small picture of the game with the pill. No other text (round 4 dropped "About 2 minutes.").

## Round 4 changes (design director review, 2026-11-24)

- **Zones on bright games.** Every outline (Overview, Zones, zone edit, the box being drawn) has a black keyline; the live one is 2 px. The selected tag (white) has one too.
- **Boxes at the top edge.** The tag goes under the box (inside it if there is no room under it either), and the kind picker goes under the tag. In zone edit, home for the bar is top centre. When the selected box, its tag or the picker would sit under it, the bar moves to the first place clear of every zone (bottom centre, top right, top left), and goes home when home is clear of every zone again.
- **Zone edit does not dim the text you box.** Zones over the shade are lifted (white 8%), not darkened.
- **One word for off.** A zone tag shows the same small "off" pill as the lists, not "(off)".
- **One column in setup.** Every step, Welcome included, uses the 960 px column at x 240, and every title sits at the same height. Steps 1 and 3 fill the column.
- **Row and control sizes.** Quick menu rows 44 (settings rows), voice rows 36 in the menu and in setup (list rows), the mode switch at the normal 32 px and 14 px, the setup search field 32, the pill 12 px each side.
- **Quick menu.** Speed is a slider like in Settings. Volume is 80% everywhere. Every speed slider now uses the app's range, 0.5x to 2.0x (`SPEED_MIN`, `SPEED_MAX` in `crates/app/src/overlay/quick_menu.rs`), so 1.1x sits at 40%, not 55%.
- **Activity.** The "off" and "repeat" tag sits before the time. The rows stagger to the 6th row; the last row ends 490 ms after the click. The empty state's Read now is the white pill, as on the Overview.
- **Game not running.** The stage message now has one button, "Switch game" (it opens the switcher). The pencil is disabled with the chips. The dot keeps its hover label "Game not running", no longer pinned in the mock, so the words still exist in two places (partly fixed, see open questions).
- **Setup Back** uses the decided back push (out 120, in 200 after 80).
- **Teal is gone** from `10-overview.html` and `tokens.css`.
- **Group labels** sit above the panel everywhere (setup step 1 now matches step 4 and Settings).
- **Not changed:** the switch-game total (320 ms, see the proposed brief changes), the keycap font (check it in the first Slint render), the lime dot on an empty Activity page (it shows that reading is on; nothing has been read yet).

## Changes after review (round 2, still valid)

- **The zone strip has chips, not switches.** One chip per kind (Speaker, Dialogue, Pickups, Prompts). On is `text`, off is `text-2` with a struck icon. Ignore zones are not listed. The 40 x 22 toggle is for Settings only.
- **No dead band.** The stage sits in a `surface-1` letterbox frame (100 to 824), the zone strip sits under it (832 to 876), and the Now panel runs 100 to 876. All margins are 24 px. The panel is now 320 px wide, so the stage is 1056 x 594.
- **One toolbar style.** Automatic, Adam and Mouse 4 are icon plus label, no border, no chevron. Pause is a ghost button.
- **Settings.** Headings are 12.5 px `text-2`. "Voice" row is now "Narrator". One "Voices" row holds "Kokoro 11 · Windows 11" and Add voices. "Updates" and "Games" became one "App" group. Buttons have no icons.
- **New token `text-3` #808080** for off but readable rows ("Hold E to talk"). `text-off` is for disabled controls only.
- **List rows.** Replay floats over the row end on hover. All sample history rows fit at 320 px. A long line (the 60-character Now line, once Read now pushes it into the list) still ends in "...". (Round 3: two lines, then cut. See the decisions.)
- **Fast-click bugs.** Switching game fades only content, never the page containers, so no ghost zone strip shows over Settings. Read now changes the Now text at once, so fast clicks never duplicate a row.
- **Motion details.** Reduce motion: movement jumps, opacity fades 80 ms. Press scale 0.97 on all pressable items. Switch game starts 80 ms after the switcher starts to close. The new game picture fades in over the old one (no dark dip). Settings opens at the section instead of scrolling while it slides.
- **Tokens.** Radii are 6, 8, 10 or full only. Spacing snaps to 4, 8, 12, 16, 24. New `h-media` 48 px for rows with a picture (switcher). Floating panels are opaque `#0A0A0A`. The stand-in game art is now neutral grey, not navy. The live outline has a black 60% keyline so it holds on bright games (round 4: 2 px, and every other outline has one too).

## Proposed brief changes (not made; the brief is outside this folder)

- 1.2: `live` is lime `#C5FA09` (Max's decision). Add `text-3` #808080 and `h-media` 48 px. Floating panels are opaque, not 92 to 96%.
- 3.1 rows 1, 2, 5 and 6: the page push and the step slide overlap 80 ms (old out 100 `ease-in`, new in 260 `ease-out-strong` after 80; back out 120, in 200 after 80). Row 1's "starts after 60 ms" becomes 80 ms.
- 3.1: the title strip swap is sequential, not a crossfade (80 out, 160 in after 80).
- 3.1 row 6: rows after the 6th share the 6th row's delay.
- 3.1 row 8: the switch starts 80 ms after the switcher starts to close. The old picture stays at full strength while the new one fades in. With that 80 ms the switch ends 320 ms after Enter, not 240 (the switch itself is 100 out plus 140 in).
- 3.3 item 3 (`PageSlot`): the incoming page needs its own delay. Page opacity and content opacity must be separate properties.
- 4.2: "About 2 minutes." on Welcome is dropped (round 4). The "Game not running." stage line gets a "Switch game" button (rule 4.1.7).
- 4.2: "Narrator" for the voice row, "Voices" with "Kokoro 11 · Windows 11", the "App" group, "On key press" everywhere.

## Notes for the Slint port

- Text is drawn one weight lighter than the Slint target. Body is 400 here and 500 in Slint. The Slint values are on the token sheet and in `tokens.css`.
- Every animation goes from A to B on opacity, x/y, scale or a rectangle. The stage grow animates real x, y, width and height, as brief 3.3 item 5 says.
- There is no blur, backdrop filter, mask or blend. Floating panels are opaque `#0A0A0A` with a 1 px white 10% border and one soft drop shadow. The zone-edit shade is a flat black 45% Rectangle; zones do not cut holes in it.
- **Spike: two-line cut in Slint 1.18.** The list rows need `wrap: word-wrap` plus `overflow: elide` with a two-line maximum height. Check early that Slint elides the last visible line. If not, cut the string in Rust. The hover card is a second Rectangle with a higher `z`, shown after a 200 ms `Timer`.
- Every zone outline keyline is a second Rectangle 1 px larger with a black 60% border (30% on dimmed zones); the live outline's is 2 px. Slint drop shadows have no spread, so no stacked or inset shadows.
- The zone tag position is a test in Rust: no room above the box (28 px), the tag goes under it; no room under it either, inside at the top left.
- The quick menu, pill and zone edit are overlay windows. As brief 3.3 item 8 says, the window stays alive and a root Rectangle animates; Rust hides the window after the exit time.
- `font-variant-numeric: tabular-nums` was removed. Slint 1.18 has no such switch.
- Only Red Dead Redemption 2 and Satisfactory have game art. In `20-motion`, any other pick shows Satisfactory. After Finish in `01-setup`, the Overview always shows the Red Dead picture, whichever game was picked (only the name follows the pick).
