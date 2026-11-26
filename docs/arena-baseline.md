# Arena baseline (2026-11-21)

These are the first trusted numbers of the arena. They were made after the review fixes (scorer, generator and
real-world truth). Earlier numbers are not comparable: the scorer and the suites changed.

- Reader under test: `winocr-naive`. It runs WinRT OCR on each zone every 500 ms and speaks when the text
  changes. It is a rough model of the old tool, not the new reader.
- Suites: seed 1, made with `arena gen`. smoke 29, standard 489, hard 612 items to read; real world 15. Clock: simulated (the real OCR time is charged). Release build.
- PC: the dev PC (Windows 11 build 26300). CPU and latency numbers depend on the PC.
- Saved reports: [`docs/baselines/`](baselines/) (`report.json` files only).

## Sanity checks

| Run | Recall | Duplicates | Noise | Hallucinations |
|---|---|---|---|---|
| smoke, oracle, simulated | 1.000 (29/29) | 0 | 0 | 0 |
| smoke, oracle, ideal | 1.000 (29/29) | 0 | 0 | 0 |
| smoke, noop, simulated and ideal | 0.000 (0/29), 0 events | 0 | 0 | 0 |
| standard, oracle, ideal | 1.000 (489/489) | 0 | 0 | 0 |
| hard, oracle, ideal | 1.000 (612/612) | 0 | 0 | 0 |
| real world, oracle, ideal and simulated | 1.000 (15/15) | 0 | 0 | 0 |
| real world, noop | 0.000 (0/15), 0 events | 0 | 0 | 0 |

The oracle scores 100% and the noop scores 0 on every set. So the scorer and the runner agree with the answer key.

## winocr-naive

| Set | Recall | Mean CER | Exact | Prompt recall | Duplicates | Noise | Hallucinations | Latency p50 / p95 |
|---|---|---|---|---|---|---|---|---|
| smoke (8 scenes) | 0.759 (22/29) | 0.045 | 0.636 | 0.800 (4/5) | 24 | 0 | 19 | 257 / 664 ms |
| standard (40 scenes) | 0.644 (315/489) | 0.053 | 0.673 | 0.643 (81/126) | 317 | 84 | 310 | 259 / 1248 ms |
| real world (11 marks) | 0.733 (11/15) | 0.143 | 0.273 | 0.840 (21/25) | 43 | 0 | 34 | 403 / 604 ms |

Background reads are 0 on all three sets. CPU per frame p50 is 0.00 ms, because the reader only does OCR every
500 ms. The p95 is 1.61 ms (smoke), 1.27 ms (standard) and 9.62 ms (real world).

### Recall by readable time

Readable time is the time the whole text is shown at full opacity: fades and typing are left out.

| Readable time | smoke | standard | real world |
|---|---|---|---|
| under 1 s | 1/1 | 19/47 (0.404) | 0/0 |
| 1 to 2 s | 3/5 | 90/141 (0.638) | 0/0 |
| 2 to 4 s | 15/19 | 127/209 (0.608) | 7/11 (0.636) |
| over 4 s | 3/4 | 79/92 (0.859) | 0/0 |
| time unsure | - | - | 4/4 |

"Time unsure" holds real-world items that start on the first frame, end on the last frame, or sit next to a frame
gap over 250 ms. They count for recall, but not for buckets or latency.

What it says in short words:
- The naive reader misses about 1 in 3 items on standard. Short items are worst: it reads only 40% of the items under 1 s.
- It repeats itself a lot (317 duplicates on standard) and reads text that is not there (310 hallucinations).
- When it does read an item, the text is mostly right (CER 0.05 on standard).

## a1 (2026-11-22)

`a1` is the first real reader (the `dr-core` engine). Its design, options, tuning log and the review fixes are in
[`reader-a1.md`](reader-a1.md). Same rules as above: release build, WinRT, simulated clock, one job.

Seed 1 and the real-world set were used for tuning. Seed 59 was made after the last code change and scored once,
so it is the honest number. "In zones" leaves out tooltips, which sit in no zone.

| Set | Reader | Recall | In zones | Under 1 s | CER | Dup | Noise | Halluc | Unwanted | Latency p50 / p95 |
|---|---|---|---|---|---|---|---|---|---|---|
| smoke | winocr-naive | 0.759 | 22/26 | 1/1 | 0.045 | 24 | 0 | 19 | 0.62 | 257 / 664 ms |
| smoke | a1 | 0.966 | 25/26 | 0/1 | 0.008 | 3 | 0 | 1 | 0.11 | 154 / 439 ms |
| standard | winocr-naive | 0.644 | 314/419 | 19/47 | 0.053 | 317 | 84 | 310 | 0.64 | 259 / 1248 ms |
| standard | a1 | 0.924 | 408/419 | 41/47 | 0.024 | 59 | 13 | 54 | 0.19 | 151 / 431 ms |
| real world | winocr-naive | 0.733 (11/15) | - | - | 0.143 | 43 | 0 | 34 | 0.71 | 403 / 604 ms |
| real world | a1 | 1.000 (15/15) | - | - | 0.078 | 2 | 2 | 6 | 0.28 | 205 / 333 ms |
| standard seed 59 (unseen) | winocr-naive | 0.558 | 290/411 | 31/78 | 0.055 | 309 | 60 | 348 | 0.63 | 195 / 1218 ms |
| standard seed 59 (unseen) | a1 | 0.788 | 373/411 | 56/78 | 0.022 | 83 | 20 | 117 | 0.28 | 138 / 441 ms |
| hard seed 59 (unseen) | winocr-naive | 0.320 | 228/584 | 100/407 | 0.057 | 134 | 2 | 244 | 0.57 | 197 / 751 ms |
| hard seed 59 (unseen) | a1 | 0.390 | 277/584 | 131/407 | 0.037 | 29 | 19 | 130 | 0.35 | 169 / 796 ms |
| hard seed 1 | winocr-naive | 0.368 | 224/534 | 96/344 | 0.082 | 127 | 2 | 273 | 0.56 | 244 / 909 ms |
| hard seed 1 | a1 | 0.549 | 312/534 | 167/344 | 0.036 | 32 | 18 | 152 | 0.31 | 197 / 728 ms |

"Unwanted" is duplicates, noise, hallucinations and background reads over all spoken events.

What it says in short words:
- In zones, A1 reads about 9 of 10 lines on an unseen standard suite. The naive reader reads about 7 of 10.
- About 1 spoken line in 4 to 5 is still unwanted. For the naive reader it is about 2 in 3.
- On the hard suite A1 gains less: recall 0.390 against 0.320 on seed 59. It reads about 1 in 3 items under 1 s.
- Tooltips outside zones are mostly missed (39 of 112 on seed 59).

## OCR bench (WinRT OCR, standard suite, scale 1)

1402 single-line crops. Mean CER 0.231, micro CER 0.178, exact 0.614, empty 0.185, 0.6 ms p50 / 1.7 ms p95 per crop.

| Kind | Crops | Mean CER | Exact | Empty |
|---|---|---|---|---|
| dialogue | 63 | 0.007 | 0.841 | 0.000 |
| speaker | 62 | 0.042 | 0.887 | 0.016 |
| subtitle | 78 | 0.047 | 0.705 | 0.000 |
| objective | 39 | 0.158 | 0.564 | 0.103 |
| pickup_toast | 178 | 0.183 | 0.455 | 0.051 |
| tooltip | 201 | 0.190 | 0.781 | 0.179 |
| prompt | 126 | 0.429 | 0.302 | 0.373 |
| hud_static | 300 | 0.343 | 0.450 | 0.273 |
| damage_number | 280 | 0.292 | 0.689 | 0.279 |
| menu | 75 | 0.028 | 0.960 | 0.027 |

| Background | Crops | Mean CER | Exact |
|---|---|---|---|
| gradient | 226 | 0.110 | 0.819 |
| grass_like | 220 | 0.155 | 0.682 |
| noise | 234 | 0.151 | 0.637 |
| moving_shapes | 268 | 0.272 | 0.664 |
| images:satisfactory | 454 | 0.347 | 0.438 |

The full tables (by font, outline, panel, size) are in `baselines/ocr-bench-standard-winocr.json`.

## How to reproduce

Run from the repo root. Nothing else should run at the same time, because the simulated clock charges real time.

```sh
cargo build -p arena --release
A=target/release/arena.exe

$A gen --suite smoke --seed 1
$A gen --suite standard --seed 1
$A gen --suite hard --seed 1
$A realworld-import --recording recordings/satisfactory-20261121-183704

$A run --suite arena-data/suites/smoke --reader oracle
$A run --suite arena-data/suites/smoke --reader noop
$A run --suite arena-data/suites/smoke --reader winocr-naive
$A run --suite arena-data/suites/standard --reader winocr-naive      # about 2.5 minutes
$A run --recorded arena-data/realworld/satisfactory --reader winocr-naive
$A ocr-bench --engine winocr --suite arena-data/suites/standard
```

To check a reader against the baseline, add `--baseline docs/baselines/<name>.json` to the same `run` command.
The exit code is 2 when recall, CER, duplicates, noise or hallucinations got worse beyond the tolerance. It is 1
when the baseline was made on other scenes (suite, seed, scenes, items or answer key differ). The OCR timing makes
small run-to-run changes. A second standard run with `--baseline` gave the same counts (only latency p50
moved, by 0.1 ms) and exited 0.

## Files

| File | What |
|---|---|
| `baselines/smoke-oracle-simulated.json` | oracle on smoke |
| `baselines/smoke-noop-simulated.json` | noop on smoke |
| `baselines/smoke-winocr-naive-simulated.json` | winocr-naive on smoke |
| `baselines/standard-winocr-naive-simulated.json` | winocr-naive on standard |
| `baselines/recorded-winocr-naive-simulated.json` | winocr-naive on the real-world set |
| `baselines/ocr-bench-standard-winocr.json` | WinRT OCR bench on standard |
| `baselines/*-a1-*.json` and the seed 7, 23, 41, 59 and hard naive runs | a1 section; full list in [`reader-a1.md`](reader-a1.md#files) |
