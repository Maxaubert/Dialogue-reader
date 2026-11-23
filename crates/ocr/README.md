# dr-ocr: OCR for DialogReader

The OCR built into Windows (`Windows.Media.Ocr`) behind `dr_core::Ocr`. The reader (`dr_core::Pipeline`,
`dr_core::Manual`) and the arena use the same engine. It moved here from `crates/arena/src/ocr/winrt.rs` in M3a;
the arena keeps a thin adapter there, and its scores did not change (docs/app-plan.md, M3a result).

```rust
let mut ocr = dr_ocr::WinOcr::new("en-US")?;      // falls back to the user's languages, with a warning
let lines = dr_core::Ocr::recognize(&mut ocr, w, h, &rgba)?;
```

- `WinOcr::has_language("en-US")` says whether that OCR language pack is installed.
- Tests: `cargo test -p dr-ocr` reads the PNGs in `tests/fixtures/` (Segoe UI 30 px). They skip with a message
  when the en-US OCR pack is missing.
