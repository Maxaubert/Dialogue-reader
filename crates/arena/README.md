# arena

Plays fake game scenes with a known answer key to a text reader and scores it. With it, the reader can be
built and checked without playing a game. Spec: [`docs/arena-spec.md`](../../docs/arena-spec.md).
Baseline numbers: [`docs/arena-baseline.md`](../../docs/arena-baseline.md).

## Build

Use a release build for timed runs: the simulated clock charges real reader time.

```sh
cargo build -p arena --release
```

Local data goes in `arena-data/` (git-ignored): background frames, suites, reports, the real-world set.

## Use

```sh
# Make the suites (seed 1 is the baseline seed)
arena gen --suite smoke --seed 1          # 8 scenes
arena gen --suite standard --seed 1       # 40 scenes
arena gen --suite hard --seed 1           # 40 scenes
# Variants (the defaults give the normal suite, byte for byte)
arena gen --suite standard --seed 1 --fps 60 --out arena-data/suites/standard-60fps        # play at 60 fps
arena gen --suite standard --seed 1 --loose-px 24 --out arena-data/suites/standard-loose24 # zones drawn 24 px looser

# Play a suite to a reader and score it (report in arena-data/reports/<suite>-<reader>-<clock>/)
arena run --suite arena-data/suites/smoke --reader oracle          # must score recall 1.0
arena run --suite arena-data/suites/smoke --reader noop            # must score recall 0
arena run --suite arena-data/suites/standard --reader winocr-naive

# Compare with a saved baseline: exit code 2 on a regression, 1 when it was made on other scenes
arena run --suite arena-data/suites/standard --reader winocr-naive \
  --baseline docs/baselines/standard-winocr-naive-simulated.json

# Score saved events again, without running the reader
arena score --suite arena-data/suites/standard --events arena-data/reports/standard-winocr-naive-simulated

# Real-world set (Satisfactory recording)
arena realworld-import --recording recordings/satisfactory-20261121-183704
arena run --recorded arena-data/realworld/satisfactory --reader winocr-naive

# OCR engine bench on single-line crops
arena ocr-bench --engine winocr --suite arena-data/suites/standard

# Look at a scene
arena render --scenario arena-data/suites/smoke/smoke-000.json --out arena-data/render-check/s000 --every 15

# One scene in a real off-screen window, captured with WGC (never takes the foreground)
arena live --scenario arena-data/suites/smoke/smoke-000.json --reader oracle --lenient-foreground
```

Readers: `oracle` (emits the answer key), `noop` (emits nothing), `winocr-naive` (WinRT OCR on each zone every
500 ms), `a1` (the real reader: the `dr-core` engine in `crates/core`, see [`docs/reader-a1.md`](../../docs/reader-a1.md)). Options go in with `--reader-opt key=value`. A new reader implements `arena::reader::Reader` and is
listed in `src/readers/mod.rs`. It must do its work inside `on_frame`.

Clocks: `simulated` (default; frames that come while the reader is busy are skipped) and `ideal` (every frame,
time ignored).

## Tests

```sh
cargo test -p arena
```

Tests use procedural backgrounds only, so they pass without `arena-data`.
