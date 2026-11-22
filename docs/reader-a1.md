# Reader A1 (2026-11-21, review fixes 2026-11-22)

A1 is the first real reader. It follows direction A1 of the research report
(`C:\Users\Admin\Documents\Claude\research\dialogue-reader\2026-11-21-reimagine-directions.md`): user zones with a type,
cheap change checks on every frame, a short ring of recent crops, and dedup that understands toasts.

- Engine: `crates/core` (package `dr-core`). It is the future product engine. It does not depend on the arena.
- Arena adapter: `crates/arena/src/readers/a1.rs`, reader name `a1`.
- Probe tool: `crates/arena/examples/a1_probe.rs` runs one crop of a PNG through many clean-up settings. It was
  used to find the OCR settings below.
- Whole-pipeline tests for cases the arena does not make: `crates/core/tests/scenarios.rs`.

## Results in short

All numbers: release build, WinRT OCR, simulated clock (real OCR time is charged), one job, 2026-11-22.

**Read this first.** Seed 1 is the tuning set, so its numbers are the lucky end. Seed 59 was made after the last
code change and scored once. Seeds 7, 23 and 41 and hard seed 1 were seen before (by a reviewer, or as a dev
check), but no setting was tuned on them. "In zones" leaves out tooltips. Tooltips sit in no zone, and A1 reads
them only through the no-zone fallback in scenes that have no zone to read.

Main numbers (in zones, and the unwanted share). "Unwanted" is repeats, noise, junk and background reads over all
spoken events, so 0.20 means 1 spoken line in 5 is unwanted.

| Set | Seen? | naive in zones | **a1 in zones** | a1 tooltips | a1 under 1 s | a1 unwanted | naive unwanted |
|---|---|---|---|---|---|---|---|
| standard seed 59 | **unseen, scored once** | 290/411 (0.706) | **373/411 (0.908)** | 39/112 | 56/78 | 0.28 | 0.63 |
| hard seed 59 | **unseen, scored once** | 228/584 (0.390) | **277/584 (0.474)** | 1/128 | 131/407 | 0.35 | 0.57 |
| standard seed 41 | seen once (before f9) | 322/449 (0.717) | 422/449 (0.940) | 30/84 | 43/55 | 0.12 | 0.58 |
| hard seed 41 | seen once (before f9) | 280/576 (0.486) | 347/576 (0.602) | 4/97 | 164/377 | 0.27 | 0.48 |
| standard seed 7 | seen | 285/434 (0.657) | 410/434 (0.945) | 20/69 | 44/55 | 0.21 | 0.63 |
| hard seed 7 | seen | 203/571 (0.356) | 288/571 (0.504) | 7/76 | 150/375 | 0.33 | 0.57 |
| standard seed 23 | seen (review) | 310/418 (0.742) | 396/418 (0.947) | 23/68 | 43/53 | 0.22 | 0.63 |
| hard seed 1 | seen (dev check) | 224/534 (0.419) | 312/534 (0.584) | 24/78 | 167/344 | 0.31 | 0.56 |
| standard seed 1 | **tuning set** | 314/419 (0.749) | 408/419 (0.974) | 44/70 | 41/47 | 0.19 | 0.64 |
| smoke seed 1 | tuning set | 22/26 | 25/26 | 3/3 | 0/1 | 0.11 | 0.62 |
| real world (Satisfactory) | tuning set | 11/15 (all) | 15/15 (all) | - | - | 0.28 | 0.71 |

Seed 59 is the one honest held-out number. On it A1 reads 0.908 of the lines in zones (naive 0.706), and 0.788 of
all lines with tooltips. Both are lower than on every seen seed (in zones 0.940 to 0.974). Seed 59 has many more tooltips (112, against 68 to 84)
and they pull the full recall down.

What it says in short words:
- Inside zones A1 reads about 9 of 10 lines on the unseen standard suite, and about 19 of 20 on the seen seeds it
  was not tuned on. The naive reader reads about 7 of 10.
- On the hard suite A1 reads about half the items in zones, and only a third to a half of the items shown under
  1 second (131/407 on seed 59). Hard under 1 s is the main open target.
- About 1 spoken line in 4 to 8 is unwanted on standard (a repeat, noise or junk), and about 1 in 3 on hard and on
  the real recording. The naive reader is near 2 in 3.
- Tooltips sit in no zone and are weak: A1 reads about a third of them on new seeds, and only in tooltip-only
  scenes.

### Before and after the review fixes

Same suites, same clock. "Before" is the A1 of 2026-11-21.

| Set | Recall before / after | Dup | Noise | Halluc | CER | CPU p95 ms |
|---|---|---|---|---|---|---|
| standard seed 1 | 0.926 / 0.924 | 63 / 59 | 14 / 13 | 67 / 54 | 0.026 / 0.024 | 22.7 / 17.3 |
| smoke seed 1 | 0.966 / 0.966 | 1 / 3 | 0 / 0 | 1 / 1 | 0.017 / 0.008 | 21.0 / 15.0 |
| real world | 1.000 / 1.000 | 3 / 2 | 2 / 2 | 5 / 6 | 0.088 / 0.078 | 19.9 / 17.8 |
| standard seed 7 | 0.859 / 0.855 | 84 / 73 | 2 / 2 | 76 / 74 | 0.024 / 0.023 | 16.6 / 13.9 |
| hard seed 7 | 0.461 / 0.456 | 29 / 26 | 18 / 18 | 178 / 150 | 0.052 / 0.049 | 20.4 / 17.2 |
| standard, `zones=off` | 0.667 / 0.658 | 40 / 39 | 205 / 161 | 506 / 425 | 0.043 / 0.040 | 14.9 / 13.8 |
| standard, `free=on` | 0.957 / 0.955 | 59 / 61 | 142 / 115 | 116 / 91 | 0.027 / 0.026 | 27.5 / 30.0 |

The fixes did not change recall on the arena. They cut junk and cost. They also fix cases the arena does not make:
back-to-back similar lines, slow typing, still screens and other frame rates. Those cases are tested in
`crates/core/tests/scenarios.rs`.

## Design

### Engine types (`dr-core`)
- `Frame { t_ms, w, h, rgba }` (tight RGBA8; `Frame::new` checks the length), `Rect`.
- `Zone { name, kind, rect, base }`. `base` is the frame size the zone was drawn on; the zone is scaled to other
  frame sizes. `None` means frame pixels.
- `ZoneKind`: `Dialogue` (the whole zone is one line, wrapped rows joined), `Speaker`, `Notification` (each text row is
  its own event), `Prompt` (longer debounce), `Ignore` (never read). The arena maps its `hud` zones to `Ignore`.
- `Ocr` trait: `recognize(w, h, rgba) -> lines with rects`, plus `max_dim`. The arena wraps its WinRT engine (and
  PP-OCR with `--features ppocr`) in `EngineOcr`.
- `Pipeline::observe(frame)`: the cheap step (crops, signatures, ring), no OCR.
- `Pipeline::read(ocr, max_reads)`: OCR of the zones that are due, at most `max_reads` per call (0 = all), the zone
  that waited longest first.
- `Pipeline::on_frame(frame, ocr)`: the no-zone detector, then `observe`, then `read` with `Config::max_reads`.
  It returns `Output { events, errors }`. An OCR error in one zone does not drop the events of the others.
- `Pipeline::finish`, `Pipeline::empty_zones` (zones with no pixels in the current frame).
- `Event { t_ms, text, rect, zone, kind }`.

Modules: `image` (grey crops, resize, stretch, Otsu, padding, sharpness), `signature` (cell change check), `ring`,
`text` (normalise, edit distance, number and word rules), `track` (dedup), `pipeline` (zones), `detect` (no-zone
mode).

### Per frame and zone
1. Cut the zone out as grey (luma). Build a signature: 8 px cells, mean grey and mean edge step per cell.
2. Compare it with the signature of the last OCR read. If no cell moved more than 2 levels, the pixels are the
   same: reuse the last read (no OCR). It is not new evidence, but the lines of that read count as still seen.
3. The zone is due for OCR when it changed, when a line still waits to be confirmed, was missed, or may be changing
   into a new event, or when the refresh time ran out (400 ms with lines, 1000 ms without). Dialogue and toast zones
   are read at most every 50 ms; speaker and prompt zones have no gap.
4. Read the zone:
   - Zones up to 70 px tall (prompt and speaker zones up to 105 px) are one line. Read once at 2x with 16 px
     padding. If that reads nothing useful, read again with stretch, Otsu and dark text.
   - Taller zones: a first pass at 1x with 16 px padding finds the rows. A second pass cuts each row tightly (the
     whole zone width, the row height plus 35% above and below) and reads it with stretch, Otsu black and white and
     dark text. If the second pass reads junk, the row keeps its first-pass text.
   - Toast zones: a line the tracker knows but the zone read missed is read again at its last place (at most 3).
   - Other tall zones: when a line was read a moment ago and the zone now reads nothing, the zone is read once more
     with the black and white clean-up before the line counts as missed.
   - A tall zone that read nothing and then stays still for 150 ms is read once more with the black and white
     clean-up (the "settle read"). Busy backgrounds never stay still that long, so this costs nothing there.
5. Turn rows into units (one per zone, or one per row in toast zones) and feed the zone tracker.
6. Frames that were not read and differ from the last read go to a ring (2.5 s), with a sharpness score.
7. Above 1080 lines, the pixel sizes (`small_h`, paddings, cell) grow with the frame height.

### Dedup (`track.rs`)
- Units are matched to tracks by their letters, digits ignored, with an edit distance up to 0.3. Rows also prefer
  the track at the same height. Words of 4 letters or less must match exactly, or be the last words of the other
  read (OCR lost the start: "Talk" and "[E] Talk").
- A track is spoken once, when one of these holds and it has been seen for the kind's stable time:
  (a) the same text was read 2 times on different pixels; (b) on a still screen, the same text stayed 250 ms;
  (c) on a busy screen, 3 or more reads of different pixels roughly agree: the read closest to the others is spoken.
- Typing: when the letters grew by 2 or more against the newest read at least 100 ms old (or, exact start only, at
  least 250 ms old), the track waits until 400 ms after that text first showed up. A text of 1 or 2 letters always
  waits 400 ms ("Go" may become "Go to the lighthouse"). A line that ends while still growing is never spoken from
  its parts.
- Numbers are voted on. A read that missed a number still supports the full value ("20" fits "20, 23").
- After a line was spoken, a new version is a new event when it holds 60 ms (with 2 fresh reads, or 250 ms on a
  still screen) and either:
  - a number changed ("+ 6 Iron Ore (27)" to "+ 12 Iron Ore (33)"), or
  - it has a new word ("Iron Ore" to "Iron Rod", "Kael" to "Kaelen"). This needs more: the spoken text was read
    exactly; the new text was read exactly on every read since it appeared; it is new against every earlier read
    of the track; and in toast zones it must replace a word, not only add words.
- A miss starts only on fresh pixels. A track ends after 2 misses and the kind's gone time without a read of it.
  The same text after that is a new event, so a toast shown twice is read twice, and a flicker is two events.
- Junk is never spoken: fewer than 2 letters, or mostly symbols.

### Lines that end unconfirmed (short pop-ups)
A line that ends before it was spoken is spoken when:
- its reads already agree (rule (a), or rule (c) within half the jitter), or
- the sharpest unread ring crop from its lifetime reads the same text (within half the jitter), or
- it had one fresh read that held on still pixels for 60 ms: the same crop is read with the other clean-up, and the
  two reads agree (within half the jitter).

It must have 3 or more letters, and it must not be the start of a line still on screen. Prompt zones never do this:
only the debounce may speak a prompt.

### No-zone mode (`detect.rs`)
Used when the layout has no zone to read (none drawn, or only "ignore" zones), or with `zones=off`. With `free=on`
it also runs outside the zones. It never looks inside any zone, ignore zones included.
- The frame is read at half size in 16 px cells (frame pixels). A cell is a text candidate when its edge level is at
  least 14 and it changed within the last 600 ms. Changes are measured against a frame at least 30 ms old, so a fade
  counts the same at 30 and at 60 fps.
- Candidate cells join into boxes (one empty cell may be bridged along a row; between two lines of a block the
  bridge may step one cell to the side). Boxes become regions with their own tracker. Lines in a region are joined,
  so a tooltip is one event.
- Regions are read like zones (same two passes), at most 4 per frame, at most every 60 ms each. A region whose
  pixels did not change since its last read reuses that read, and the reuse is no new evidence. Quiet regions
  expire after 400 ms.

## Options (`--reader-opt key=value`)

Defaults are the tuned settings. Unknown keys are an error.

| Key | Default | What |
|---|---|---|
| `engine`, `lang`, `models` | winocr, en-US, - | OCR engine (as for winocr-naive) |
| `zones` | on | off: ignore the layout and use the no-zone mode |
| `free` | off | also run the no-zone mode outside the zones |
| `prompts` | on | read prompt zones |
| `scale`, `stretch`, `binarize`, `dark`, `pad`, `chan` | 1, off, off, off, 16, luma | first-pass clean-up of tall zones |
| `refine`, `r_scale`, `r_stretch`, `r_binarize`, `r_dark`, `r_pad` | on, 1, on, on, on, 8 | second pass per row |
| `r_margin`, `r_fullw` | 0.35, on | row margin (share of row height); row crop takes the zone width |
| `recheck` | on | read a missed line again (toast row, or the whole zone with the other clean-up) |
| `small_h` | 70 | zones up to this tall are read in one pass |
| `loose` | 1.5 | prompt and speaker zones up to this times `small_h` are read in one pass |
| `s_scale`, `s_stretch`, `s_binarize`, `s_dark`, `s_pad` | 2, off, off, off, 16 | one-pass clean-up |
| `small_alt` | on | second try (stretch, Otsu, dark) when the one pass reads nothing |
| `cell`, `cell_thresh`, `change_share`, `same_px` | 8, 10, 0, 2 | change check |
| `refresh_ms`, `idle_ms`, `ring_ms` | 400, 1000, 2500 | read timing and ring length |
| `stable_dialogue`, `stable_speaker`, `stable_notif`, `stable_prompt` | 150, 100, 60, 300 | ms seen before speaking |
| `gone_dialogue`, `gone_speaker`, `gone_notif`, `gone_prompt` | 100, 100, 300, 150 | ms missing before a line is gone |
| `gap_dialogue`, `gap_speaker`, `gap_notif`, `gap_prompt` | 50, 0, 50, 0 | shortest ms between two reads of a zone |
| `max_reads` | 0 | zone OCR reads per frame (0 = no limit) |
| `ref_h` | 1080 | frame height the pixel sizes are tuned for (taller frames scale them up) |
| `confirm`, `consensus`, `jitter`, `min_letters`, `gone_reads` | 2, 3, 0.3, 2, 2 | dedup rules |
| `static_ms`, `typing_ms`, `grow_ms`, `change_ms` | 250, 400, 100, 60 | still-screen trust, typing wait, typing look-back, hold time of a changed line |
| `rescue`, `emit_lost`, `lost_letters`, `second_ms` | on, on, 3, 60 | ring rescue; speak ended lines whose reads agree; letters an ended line needs; still time before a second opinion (negative: off) |
| `free_cell`, `free_edge`, `free_change`, `free_recent`, `free_gap`, `free_max`, `free_min_cells`, `free_expire`, `free_stable`, `free_move`, `free_diff` | 8, 14, 12, 600, 60, 4, 3, 400, 120, 1000, 30 | no-zone mode |
| `trace` | - | folder: write one text trace per scene (every read with row heights, every speak) |

`regrow` is gone: a spoken dialogue line that gains words is now always a new event.

## Speed

The simulated clock charges `on_frame`. The median frame cost hides a lot, because still synthetic backgrounds cost
almost nothing. Cost per background kind, standard seed 1 (final run):

| Background | Frames | p50 ms | p95 ms | mean ms | frames over 2 ms |
|---|---|---|---|---|---|
| game image | 11064 | 5.13 | 25.8 | 7.8 | 57% |
| gradient | 6049 | 0.16 | 3.2 | 0.4 | 7% |
| grass_like | 4443 | 0.53 | 6.7 | 1.6 | 26% |
| moving_shapes | 5614 | 0.52 | 5.5 | 1.6 | 32% |
| noise | 4225 | 2.43 | 7.7 | 2.8 | 55% |
| all | 31395 | 0.52 | 17.3 | 3.7 | 38% |

- Game-image backgrounds are the honest case: median 5 ms, 95th percentile 26 ms. They also play at 10, 15 or
  30 fps in the generator, so some frames repeat and the "same pixels" skip fires more often than in a real game.
- The real recording (1920x1080, two zones) costs 9.5 ms at the median and 17.8 ms at the 95th percentile.
- 99.2% of standard frames were delivered (31395 of 31645; it was 98.4% before the review fixes).
- The 50 ms default gap for dialogue and toast zones cut OCR by about a fifth on the dev runs, at no recall cost.
- `max_reads=1` caps OCR at one zone per frame (dev run: CPU p95 17 ms against 22 ms, prompts 0.746 against
  0.714). It is not the default.
- Measured with the probe: a one-line crop costs about 1 to 3 ms, a 384x259 toast zone 7 to 16 ms at 1x and 20 to
  40 ms at 2x. That is why big zones use 1x and only small zones use 2x.
- 4K and real 60 fps footage were not measured. At 30 fps and 1080p a real game costs about a third of a core
  while text is on screen (extrapolated from the real recording, not run).

## The real-world set and its limits

The real-world set is one recording of Satisfactory. Its "1.000" is not proof that A1 works on real games:
- It has 15 items from one game, all toasts. 11 are 2 to 4 s long, the other 4 have an unsure time.
- It plays at about 10 fps (median frame gap 104 ms, gaps up to 1188 ms). It is a 2x downscaled JPEG copy.
- It was used while tuning (`small_h`, `loose`, `stable_prompt` were tried on it), so it is a tuning set.
- At 10 fps no frame is ever dropped, so the simulated clock tests nothing there.

Short pop-ups in a real game at 60 fps are untested. The next step is a second game, recorded at full frame rate,
kept unseen until it is scored.

Prompt recall on the real set is 0.44 (naive 0.84). The 300 ms debounce is a small part of it:
`stable_prompt=100` only lifts it to 0.64, with more repeats. The larger cause is OCR quality on the 120 px
prompt zone. The trace shows many frames where the prompt is on screen and the zone read is empty or garbled
("Press ato pick up Leaves", "pick up").

## Final numbers

All runs on 2026-11-22: release build, WinRT, simulated clock, one job, nothing else running. Reports are in
`docs/baselines/` (see Files). The naive runs of seeds 23 and 41, loose zones and hard seed 1 come from the same
night (the naive reader did not change). "Unwanted" is (dup + noise + halluc + background) over all events. CPU is
per frame in ms. Run-to-run noise is about 1 item of recall and a few duplicates.

| Run | Recall | In-zone | Tooltips | <1 s | 1-2 s | 2-4 s | >4 s | CER | Exact | Prompts | Dup | Noise | Halluc | Unwanted | Lat p50/p95 | CPU p50/p95 |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| smoke s1 naive | 0.759 (22/29) | 22/26 (0.846) | 0/3 | 1/1 | 3/5 | 15/19 | 3/4 | 0.045 | 0.636 | 0.800 (4/5) | 24 | 0 | 19 | 0.62 (43/69) | 257/664 | 0.00/1.6 |
| smoke s1 a1 | 0.966 (28/29) | 25/26 (0.962) | 3/3 | 0/1 | 5/5 | 19/19 | 4/4 | 0.008 | 0.821 | 1.000 (5/5) | 3 | 0 | 1 | 0.11 (4/37) | 154/439 | 0.18/15.0 |
| standard s1 naive | 0.644 (315/489) | 314/419 (0.749) | 1/70 | 19/47 | 90/141 | 127/209 | 79/92 | 0.053 | 0.673 | 0.643 (81/126) | 317 | 84 | 310 | 0.64 (711/1107) | 259/1248 | 0.00/1.3 |
| standard s1 a1 | 0.924 (452/489) | 408/419 (0.974) | 44/70 | 41/47 | 134/141 | 189/209 | 88/92 | 0.024 | 0.823 | 0.786 (99/126) | 59 | 13 | 54 | 0.19 (126/677) | 151/431 | 0.52/17.3 |
| real world naive | 0.733 (11/15) | - | - | 0/0 | 0/0 | 7/11 | 0/0 | 0.143 | 0.273 | 0.840 (21/25) | 43 | 0 | 34 | 0.71 (77/109) | 403/604 | 0.00/9.6 |
| real world a1 | 1.000 (15/15) | - | - | 0/0 | 0/0 | 11/11 | 0/0 | 0.078 | 0.533 | 0.440 (11/25) | 2 | 2 | 6 | 0.28 (10/36) | 205/333 | 9.48/17.8 |
| standard s59 naive | 0.558 (292/523) | 290/411 (0.706) | 2/112 | 31/78 | 71/131 | 114/220 | 76/94 | 0.055 | 0.651 | 0.756 (136/180) | 309 | 60 | 348 | 0.63 (717/1145) | 195/1218 | 0.00/1.2 |
| standard s59 a1 | 0.788 (412/523) | 373/411 (0.908) | 39/112 | 56/78 | 100/131 | 168/220 | 88/94 | 0.022 | 0.801 | 0.867 (156/180) | 83 | 20 | 117 | 0.28 (221/789) | 138/441 | 1.23/15.8 |
| hard s59 naive | 0.320 (228/712) | 228/584 (0.390) | 0/128 | 100/407 | 101/265 | 27/40 | 0/0 | 0.057 | 0.654 | 0.351 (59/168) | 134 | 2 | 244 | 0.57 (380/667) | 197/751 | 0.00/1.2 |
| hard s59 a1 | 0.390 (278/712) | 277/584 (0.474) | 1/128 | 131/407 | 117/265 | 30/40 | 0/0 | 0.037 | 0.723 | 0.363 (61/168) | 29 | 19 | 130 | 0.35 (179/518) | 169/796 | 2.19/20.9 |
| standard s41 naive | 0.606 (323/533) | 322/449 (0.717) | 1/84 | 25/55 | 85/149 | 147/254 | 66/75 | 0.038 | 0.786 | 0.698 (120/172) | 200 | 97 | 312 | 0.58 (609/1052) | 245/1486 | 0.00/1.8 |
| standard s41 a1 | 0.848 (452/533) | 422/449 (0.940) | 30/84 | 43/55 | 131/149 | 205/254 | 73/75 | 0.012 | 0.894 | 0.779 (134/172) | 21 | 5 | 57 | 0.12 (83/669) | 128/414 | 0.37/14.2 |
| hard s41 naive | 0.418 (281/673) | 280/576 (0.486) | 1/97 | 126/377 | 131/261 | 24/35 | 0/0 | 0.056 | 0.641 | 0.543 (102/188) | 158 | 5 | 193 | 0.48 (356/739) | 171/575 | 0.00/1.3 |
| hard s41 a1 | 0.522 (351/673) | 347/576 (0.602) | 4/97 | 164/377 | 153/261 | 34/35 | 0/0 | 0.035 | 0.758 | 0.622 (117/188) | 33 | 2 | 135 | 0.27 (170/638) | 158/626 | 0.53/15.3 |
| standard s7 naive | 0.567 (285/503) | 285/434 (0.657) | 0/69 | 27/55 | 77/138 | 129/236 | 52/74 | 0.023 | 0.832 | 0.752 (115/153) | 152 | 244 | 296 | 0.63 (692/1092) | 224/1542 | 0.00/1.8 |
| standard s7 a1 | 0.855 (430/503) | 410/434 (0.945) | 20/69 | 44/55 | 116/138 | 199/236 | 71/74 | 0.023 | 0.809 | 0.837 (128/153) | 73 | 2 | 74 | 0.21 (149/707) | 130/429 | 0.34/13.9 |
| hard s7 naive | 0.314 (203/647) | 203/571 (0.356) | 0/76 | 108/375 | 75/240 | 20/32 | 0/0 | 0.091 | 0.478 | 0.450 (72/160) | 114 | 11 | 245 | 0.57 (370/645) | 186/740 | 0.00/1.6 |
| hard s7 a1 | 0.456 (295/647) | 288/571 (0.504) | 7/76 | 150/375 | 118/240 | 27/32 | 0/0 | 0.049 | 0.668 | 0.600 (96/160) | 26 | 18 | 150 | 0.33 (194/585) | 171/655 | 1.65/17.2 |
| standard s23 naive | 0.640 (311/486) | 310/418 (0.742) | 1/68 | 34/53 | 84/147 | 127/208 | 66/78 | 0.033 | 0.781 | 0.717 (91/127) | 241 | 94 | 347 | 0.63 (682/1084) | 245/1625 | 0.00/2.1 |
| standard s23 a1 | 0.862 (419/486) | 396/418 (0.947) | 23/68 | 43/53 | 126/147 | 176/208 | 74/78 | 0.014 | 0.874 | 0.866 (110/127) | 68 | 12 | 71 | 0.22 (151/680) | 130/445 | 0.52/19.5 |
| hard s1 naive | 0.368 (225/612) | 224/534 (0.419) | 1/78 | 96/344 | 108/233 | 21/35 | 0/0 | 0.082 | 0.511 | 0.517 (89/172) | 127 | 2 | 273 | 0.56 (402/716) | 244/909 | 0.00/1.0 |
| hard s1 a1 | 0.549 (336/612) | 312/534 (0.584) | 24/78 | 167/344 | 140/233 | 29/35 | 0/0 | 0.036 | 0.738 | 0.645 (111/172) | 32 | 18 | 152 | 0.31 (202/649) | 197/728 | 1.55/28.1 |
| standard s1 at 60 fps a1 | 0.920 (450/489) | 406/419 (0.969) | 44/70 | 41/47 | 132/141 | 189/209 | 88/92 | 0.023 | 0.824 | 0.786 (99/126) | 65 | 11 | 58 | 0.20 (134/683) | 134/414 | 0.27/13.4 |
| standard s1, loose zones naive | 0.669 (327/489) | 325/419 (0.776) | 2/70 | 20/47 | 95/141 | 129/209 | 83/92 | 0.054 | 0.670 | 0.619 (78/126) | 349 | 84 | 302 | 0.64 (735/1140) | 260/1250 | 0.00/1.7 |
| standard s1, loose zones a1 | 0.914 (447/489) | 402/419 (0.959) | 45/70 | 41/47 | 131/141 | 186/209 | 89/92 | 0.028 | 0.796 | 0.778 (98/126) | 67 | 13 | 56 | 0.20 (136/681) | 155/475 | 0.55/27.7 |
| smoke, `zones=off` a1 | 0.724 (21/29) | 18/26 (0.692) | 3/3 | 1/1 | 2/5 | 15/19 | 3/4 | 0.052 | 0.476 | 0.800 (4/5) | 3 | 11 | 29 | 0.63 (43/68) | 159/1347 | 0.75/10.9 |
| standard, `zones=off` a1 | 0.658 (322/489) | 261/419 (0.623) | 61/70 | 31/47 | 98/141 | 140/209 | 53/92 | 0.040 | 0.696 | 0.476 (60/126) | 39 | 161 | 425 | 0.62 (632/1014) | 168/851 | 0.87/13.8 |
| real world, `zones=off` a1 | 0.200 (3/15) | - | - | 0/0 | 0/0 | 3/11 | 0/0 | 0.273 | 0.000 | 0.000 (0/25) | 0 | 19 | 53 | 0.96 (72/75) | 1334/1334 | 7.05/21.3 |
| standard, `free=on` a1 | 0.955 (467/489) | 408/419 (0.974) | 59/70 | 44/47 | 137/141 | 198/209 | 88/92 | 0.026 | 0.818 | 0.778 (98/126) | 61 | 115 | 91 | 0.32 (271/836) | 161/441 | 2.29/30.0 |

Notes:
- The "60 fps" and "loose zones" suites are standard seed 1 made with `--fps 60` and `--loose-px 24`. At 60 fps
  recall holds (0.920 against 0.924; it was 0.896 before the fixes). With loose zones it is 0.914 (0.904 before),
  but CPU p95 rises to 27.7 ms.
- The no-zone fallback (`zones=off`) still reads far more noise and junk than zones: 425 hallucinations on
  standard. On the real recording it reads only 3 of 15, because grass and the HUD change all the time. Zones are
  the way to use A1.
- `free=on` (zones plus the no-zone mode outside them) reads 59 of 70 tooltips against 44. It also adds noise 115
  and hallucinations 91 (damage numbers, menus and texture). It is off by default.

## Known weaknesses

- **Tooltips and other text outside zones.** Tooltips in a scene that also has zones are not read. `free=on` reads
  them, but also reads damage numbers ("CRIT 740"), menus and junk. Moving-text suppression (`free_move`) did not
  help: damage numbers move only 30 px. On new seeds about a third of tooltips are read.
- **Hard suite, short items.** Under 1 s on hard: 131 of 407 on seed 59, 167 of 344 on seed 1. Hallucinations stay
  high (130 to 152): partial reads of fading or sliding toasts that still look like text.
- **Generalisation gap.** On the unseen seed 59, in-zone recall is 0.908, against 0.940 to 0.974 on the seen seeds.
  Expect about 0.9 in zones on new scenes, not 0.97.
- **Duplicates on busy backgrounds.** When OCR misses a line for longer than the gone time and then reads it again,
  it is spoken again. Most left-over duplicates are prompts that flicker and toasts over grass.
- **Toast lines that only gain words** ("Glass" to "Glass Vial") stay one event. See code finding 1 in "Review fixes".
- **Prompt recall on the real set** is 0.44 (naive 0.84). See "The real-world set and its limits".
- **Speaker misreads are new names.** Short names must match exactly, so "Keel" for "Kael" is spoken as a new line.
- **Menus inside a zone** (pause menu rows in real marks 6 and 7 fall inside the notification zone) are read as
  toasts. The arena cannot tell the reader a menu is open.
- **OCR blocks the caller.** `read` runs OCR inline. The product loop needs OCR off the capture thread.
- **CPU.** On game-image backgrounds the 95th percentile is about 26 ms per frame, close to one frame at 30 fps.
- **PP-OCR** was not tuned or run through A1. The adapter accepts `engine=ppocr` (with `--features ppocr`), but all
  numbers are WinRT.

## Review fixes (2026-11-22)

Two reviews looked at A1: one at the code, one at how honest the numbers are. This table says what was done with
each finding. "Fixed" means a test or a run shows it. "Partly" means the main case is fixed and a stated case is
not. The new dr-core tests are in `crates/core/tests/scenarios.rs` (ported from the reviewer's fake-OCR checks).

### Code review

| # | Finding | Status | What was done |
|---|---|---|---|
| 1 | A spoken line swallows the next similar line | Partly | A spoken line that changes to a text with a new word is a new event ("Iron Ore" to "Iron Rod", "Kael" to "Kaelen"). The new text must be read exactly on 2 fresh frames (or stay still for 250 ms), be new against every earlier read of the track, and the spoken text must have been read exactly (not picked from rough reads). Not fixed: in toast zones a line that only gains words ("Glass" to "Glass Vial") stays one event, because OCR often reads a part of a row first. In dialogue zones it is a new event. |
| 2 | Typing counts reads, not time | Partly | Typing compares with the newest read at least 100 ms old, and with one at least 250 ms old (exact start only). Short texts of 1 or 2 letters wait the typing time. The number rule now needs 60 ms, not 3 reads. Tests pass for 10 to 60 letters per second at 30, 60, 120 and 144 fps. Still counted in reads: `gone_reads` (2, always with `gone_ms`) and `consensus` (3 fresh reads, always with the stable time). |
| 3 | A short pop-up on a still screen is never spoken | Fixed | A line that ended after one fresh read held on still pixels for 60 ms or more is read again from the same crop with the other clean-up (a second opinion). It is spoken when the two reads agree closely. Pop-ups of 3 frames or more are now spoken. |
| 4 | One bad read on a still screen repeats a line | Partly | On frames with the same pixels the lines of the last read are kept current (`touch`), and a miss only starts on fresh pixels. A spoiled read that holds 2 frames (66 ms) no longer repeats the line. A tall zone that suddenly reads nothing is read once more with the other clean-up. Not fixed: a spoiled read that holds longer than the gone time (dialogue 100 ms) still ends the line, by design: a prompt that flickers off for that long is a new event too. |
| 5 | `emit_lost` speaks junk; ring rescue does little | Fixed | A line that ended unconfirmed needs 3 or more letters, must not be the start of a line still on screen, and a rough agreement must be within half the usual jitter. A second read (ring or second opinion) must also agree within half the jitter. Junk from these paths fell from about 20 to a few per suite. |
| 6 | A number change on a still screen is 800 ms late | Fixed | A spoken track whose current read may be a new event keeps the zone "pending", so it is read every frame. The 250 ms still-screen rule counts from the first read of the new value. The test shows 300 ms or less. |
| 7 | No-zone mode treats every read as fresh | Fixed | Each region keeps the signature of its last read. Same pixels reuse that read, cost no OCR and are no new evidence. |
| 8 | An OCR error drops events already decided | Fixed | `on_frame` returns `Output { events, errors }`. An error in one zone keeps the events of the others; the failed zone is read again next frame. The arena adapter still stops on any error, because its scores would be wrong. |
| 9 | The API blocks and has no OCR budget | Partly | The work is split in `observe` (crops, signatures, ring; no OCR) and `read` (OCR with a budget, `max_reads` per call, the zone that waited longest first). `on_frame` does both. A default 50 ms gap for dialogue and toast zones cuts OCR by about a fifth. Not fixed: the default budget is unlimited, and `read` still blocks. A host that wants OCR off the capture thread needs OCR jobs and results passed back. That is a design step for the product loop, not something the arena can measure (it is single-threaded). |
| 10 | Zones and pixel sizes are absolute | Fixed | A zone can carry the frame size it was drawn on (`Zone::base`); it is scaled to the frame. Above 1080 lines the pixel sizes (`small_h`, paddings, cell) scale with frame height (no change at 720p and 1080p). Zones with no pixels are reported by `Pipeline::empty_zones` (the arena prints them once). |
| 11 | The ring stores read frames too | Fixed | The ring keeps only frames that were not read and differ from the last read. `Ring::best_in` picks the crop. |
| 12 | `best()` picks the earliest version on a tie | Fixed | It picks the latest. |
| 13 | Speaker clean-up cuts "Player 1", "Henry I" | Fixed | Digits are never trimmed; a letter (I, l) only at the start. |
| 14 | Small per-read costs | Fixed | The padding median uses a histogram. The tracker works out each track's match anchors and the distances between versions once per read. |
| 15 | Small items | Mostly | `nd` is 0 for a first read on old pixels. The trace stops at 200 000 lines. `Frame::new` checks the buffer length in release builds too. Events carry the zone kind. Not done: see the disagreements below. |

### Honesty review

| # | Finding | Status | What was done |
|---|---|---|---|
| 1 | The headline is the best seed | Fixed | The summary leads with in-zone recall on a new seed (59) that was made after the last code change and scored once, and gives tooltips their own column. |
| 2 | The real-world set is thin | Partly | Its four limits are in "The real-world set and its limits". Not fixed: a second game, recorded at full frame rate and kept unseen, needs a live recording session. |
| 3 | The cost claim comes from still backgrounds | Fixed | Cost per background kind is in "Speed". A 50 ms default gap was measured and kept. |
| 4 | Settings fit the generator's zone shapes | Fixed | `arena gen --loose-px 24` makes the loose-zone suite. Prompt and speaker zones up to 1.5 times `small_h` now take the one-line 2x path. Loose-zone prompt recall went from 0.651 to about 0.8 on the dev runs. |
| 5 | The naive baseline flatters A1 | Fixed | The tables give the unwanted share (repeats, noise and junk over all events) for A1 itself. |
| 6 | Only 30 fps is tested | Fixed | `arena gen --fps 60` makes a 60 fps suite. The tracker counts time, and the no-zone detector compares cells with a frame at least 30 ms old. |
| 7 | The fallback ignores the "different pixels" rule | Fixed | Same as code finding 7. |
| 8 | Hard results are poor and only seed 7 is given | Fixed | Hard seeds 1, 7, 41 and 59 are all in the tables. Hard under 1 s is named as the main open target. |
| 9 | The reason for low real-world prompt recall is partial | Fixed | Stated in "The real-world set and its limits": OCR quality on the 120 px prompt zone is the larger cause. |
| 10 | Doc sentences that do not match | Fixed | Rewritten. The dev subset is named as an overfit risk. |
| 11 | Speaker clean-up drops real characters | Fixed | Same as code finding 13. |
| 12 | `finish()` runs outside the clock; seeds are no longer unseen | Noted | `finish` now also reads frames still waiting, still outside the clock. Seeds 7 and 23 and hard seed 1 are labelled "seen". Seed 41 was made after the first freeze, but steps f9 to f11 came after its first run, so it is labelled "seen once". Seed 59 was made after the last code change and is the only unseen seed. |

### Disagreements

- **Box bridge in the no-zone mode (code 15).** The reviewer read the one-cell side step between two lines of a
  block (dy 2, dx 1) as a bug against its comment. Removing it split tooltips into 3 regions and added junk
  ("Field Kit", "stores ackable") on smoke. It is kept, and the comment now says why: the lines of a tooltip do not
  start and end at the same cell.
- **Number-only text is junk (code 15).** "+50" or "x3" alone is not spoken. For this reader that is wanted:
  numbers alone are damage numbers, counters and HUD values, which the arena counts as noise. A pickup toast always
  has a word.

## Tuning log

All runs: release build, simulated clock, one job, nothing else running. "dev" is a subset of 12 standard seed-1
scenes with the most problems (000, 001, 002, 004, 009, 010, 016, 021, 025, 026, 029, 030). Picking the
worst scenes of the tuning seed is a risk of overfitting; the unseen seed 59 is the check on it. Small scene subsets
("p016", "s001") were used to find causes with `trace=`. Raw lines: `arena-data/a1-tuning/tuning-log.tsv`
(local only). The run labels in this table are by order and do not match the labels in that file.

| Run | Set | Change | Recall | Dup | Halluc | Note |
|---|---|---|---|---|---|---|
| - | standard | winocr-naive baseline | 0.644 | 317 | 310 | |
| r00 | smoke | first version: 2x, no padding | 0.759 | 24 | 13 | speaker "Juno" never read (text on the zone edge) |
| r00b | smoke | `pad=16` (plain) | 0.759 | 18 | 17 | padding fixes the edge case |
| r00c | smoke | `pad=16`, plus consensus and speaker edge trim | 0.793 | 7 | 15 | |
| r01 | smoke | `pad=16 scale=1` | 0.931 | 1 | 3 | big zones read better at 1x, and cost 3x less |
| r01 | standard | same | 0.808 | 323 | 262 | toasts over grass: agreement came from reading the same frozen frame twice |
| r02 | p016 | only reads of new pixels count as agreement; letters match, numbers voted | 0.944 | 19 | 21 | was dup 113, halluc 82 |
| r03 | p016 | second pass per row: tight crop, stretch, Otsu, dark text | 0.944 | 3 | 4 | CER 0.068 to 0.005 |
| r03 | standard | same | 0.806 | 110 | 101 | |
| r04 | s001 | numbers: partial reads fit ("20" fits "20, 23") | 1.000 | 14 | 10 | was 25 / 12 |
| r05 | s001 | row crop takes the whole zone width | 1.000 | 11 | 0 | |
| r06 | dev | zones up to 70 px tall: one pass at 2x | 0.804 | 51 | 63 | `small_h=0`: 0.804 / 57 / 86; binarized one pass: 0.780 |
| r07 | dev | typing wait 400 ms; free mode when only ignore zones; lines bridge | 0.916 | 51 | 42 | tooltip-only scenes now read |
| r08 | dev | fuzzy typing prefix; lost lines that were growing are not spoken; small pad 16 | 0.921 | 51 | 39 | |
| r09 | dev | short words match as last words; prompts never rescued; `gone_notif` 300 vs 600 | 0.925 / 0.911 | 41 / 45 | 35 / 34 | kept 300 |
| r10 | dev | recheck of missed toast lines (on / off) | 0.935 / 0.916 | 41 / 48 | 39 / 36 | kept on |
| r11 | dev | second try on small zones when the first reads nothing | 0.935 | 22 | 41 | speaker repeats mostly gone |
| r08 | standard | all of the above | 0.916 | 58 | 108 | |
| r08 | smoke | | 0.862 | 1 | 1 | subtitles never spoken: junk at the end looked like typing |
| r09 | smoke | typing needs 2 more letters against the read 100 ms before | 0.966 | 1 | 1 | |
| r12 | dev | first pass: `pad=16` / `pad=0 stretch=1` / `pad=16 stretch=1` / `pad=0` | 0.939 / 0.911 / 0.893 / 0.949 | 30 / 38 / 67 / 36 | 45 / 56 / 57 / 40 | kept pad 16 (smoke: pad 16 and pad 16 + stretch best) |
| r13 | free5 | box into a masked zone is skipped | 1.000 | 4 | 15 | was 53 halluc in standard-024 (HUD edges) |
| r13b | free5 | `free_move` 24 / 48 / off (moving text not read) | 0.930 / 0.977 / 1.000 | 2 / 3 / 4 | 7 / 13 / 15 | off: damage numbers move too little to tell apart |
| r10 | standard | prompt `gone_prompt` 400 tried on 4 prompt scenes: prompt recall 0.83 to 0.56, same dups | 0.926 | 63 | 67 | kept 150 |
| final | smoke / standard / real | frozen defaults | 0.966 / 0.926 / 1.000 | 1 / 63 / 3 | 1 / 67 / 5 | |

Run-to-run noise: the clock charges real OCR time, so repeat runs move by about 1 item of recall and a few
duplicates on the dev subset.

### Review fixes log (2026-11-22)

Dev runs on the ideal clock (8 jobs; recall agrees with the simulated clock, CPU does not), standard seed 1 unless
named. "loose24" and "60fps" are standard seed 1 made with `--loose-px 24` and `--fps 60`. The first row is the
A1 of 2026-11-21 on the same clock.

| Step | Change | Recall | Prompts | Dup | Halluc | Note |
|---|---|---|---|---|---|---|
| f0 | A1 before the fixes | 0.924 | 0.786 | 63 | 68 | latency p50 113 ms |
| f1 | all tracker and pipeline fixes, first draft | 0.928 | 0.714 | 85 | 72 | garbled subtitles became "new lines"; a prompt flicker no longer ended the line |
| f2 | a new word needs an exact spoken text, exact new reads, a text new against all earlier reads; toast zones need a replaced word | 0.924 | 0.746 | 66 | 65 | |
| f3 | a miss counts again after the first fresh miss (gone time as before) | 0.924 | 0.770 | 66 | 65 | prompt flicker is two events again |
| f4 | typing: second look-back exact start only; rough agreement never picks the start of the current read | 0.924 | 0.770 | 66 | 65 | smoke 0.931 to 0.966 |
| f5 | no-zone box bridge kept as it was | | | | | smoke junk 5 to 1 (tooltip no longer split in 3) |
| f6 | 50 ms gap for every kind | 0.924 | 0.714 | 57 | 62 | latency p50 104 to 150 ms, CPU p95 27 to 22 ms |
| f6b | `max_reads=1` instead | 0.924 | 0.746 | 62 | 68 | CPU p95 17 ms; not the default |
| f7 | loose24: `loose=2` (one-line kinds only) | 0.912 | 0.802 | 87 | 78 | without: 0.906, 0.651, 88, 89 |
| f7b | real world, `loose=2` | 1.000 | 0.560 | 6 | 6 | CPU p50/p95 22/55 ms (2x); `loose=1.5` keeps the 120 px real prompt zone on the cheap path |
| f8 | no-zone mask holds ignore zones again (a bug from f1) | | | | | |
| f9 | second reads (ring, second opinion) must agree within half the jitter | 0.924 | 0.778 | 57 | 53 | hard seed 1 (dev check): halluc 153 to 151; standard halluc 62 to 53 |
| f10 | no gap for speaker zones | 0.924 | 0.778 | 59 | 56 | hard seed 1: recall 0.541 to 0.549 (short names need every read) |
| f11 | settle read after 150 ms still | 0.924 | 0.778 | 59 | 56 | smoke "Hold the line!" read again (0.931 to 0.966); hard halluc unchanged (165 with no still wait) |

Hard seed 1 was used as a dev check in f9 to f11, so it is no longer unseen.

## Reproduce

```sh
cargo build -p arena --release
A=target/release/arena.exe
$A run --suite arena-data/suites/smoke --reader a1
$A run --suite arena-data/suites/standard --reader a1 --baseline docs/baselines/standard-a1-simulated.json
$A run --recorded arena-data/realworld/satisfactory --reader a1
$A run --suite arena-data/suites/standard --reader a1 --reader-opt zones=off
$A gen --suite standard --seed 59 --out arena-data/suites/standard-s59
$A gen --suite hard --seed 59 --out arena-data/suites/hard-s59
$A run --suite arena-data/suites/standard-s59 --reader a1 --report arena-data/reports/standard-s59-a1
$A gen --suite standard --seed 1 --fps 60 --out arena-data/suites/standard-60fps
$A gen --suite standard --seed 1 --loose-px 24 --out arena-data/suites/standard-loose24
cargo test -p dr-core          # unit tests and the whole-pipeline scenarios
# Look at one crop with many clean-up settings
cargo run -p arena --release --example a1_probe -- frame.png <x> <y> <w> <h>
```

## Files

All in `docs/baselines/`, simulated clock.

| File | What |
|---|---|
| `smoke-a1-simulated.json`, `standard-a1-simulated.json`, `recorded-a1-simulated.json` | a1, seed 1 and real world |
| `standard-s59-*`, `hard-s59-*` (a1 and winocr-naive) | unseen seed 59 |
| `standard-s41-*`, `hard-s41-*`, `standard-s7-*`, `hard-s7-*`, `standard-s23-*` (a1 and winocr-naive) | seen seeds |
| `hard-a1-simulated.json`, `hard-winocr-naive-simulated.json` | hard seed 1 |
| `standard-60fps-a1-simulated.json`, `standard-loose24-*` (a1 and winocr-naive) | variants of standard seed 1 |
| `*-a1-nozone-simulated.json`, `standard-a1-free-simulated.json` | a1 with `zones=off` and `free=on` |
