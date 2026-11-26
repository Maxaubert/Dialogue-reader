# DialogReader (Rust rewrite)

Reads game dialogue aloud: capture, OCR, speech (Kokoro, Windows voices as backup). Spec: `docs/app-spec.md`;
milestones: `docs/app-plan.md`; crates: `crates/{core,ocr,tts,arena,runtime,app}`.

## Rules for this repo
- Every cargo command uses `CARGO_TARGET_DIR` = this repo's `target` (the session hook sets it). Never `--target-dir`.
- No workspace `cargo fmt`: rustfmt only the files you changed.
- Tests stay headless: no visible window (hidden desktop, see `crates/runtime/src/testwin.rs`), no sound, never the
  real `%APPDATA%\DialogReader`, registry tests on a test value name only.
- Plain-English docs and UI text, no em-dashes. UI follows `design/mockups/c-midnight`; text that differs is listed
  in `docs/app-plan.md` for Max.
- `THIRD_PARTY_NOTICES.md`: rerun `python installer/notices.py` after a dependency change (CI checks it).

## Build, test, release
- Typecheck/lint: `cargo clippy --workspace --all-targets -- -D warnings`
- Unit: `cargo test --workspace`
- E2E (headless): `cargo test -p dr-app` (screens, overlay windows and single instance on a hidden desktop)    Run when: crates/app/ui/**, crates/app/src/**
- Build / package: `pwsh installer/build.ps1`   Artifact: `DialogReader-Setup-x64-<version>.exe` (in `target\installer`)
- Known failures to tolerate: none locally; CI (GitHub runners) is not proven yet for the capture and hidden-desktop tests
- Version source: `Cargo.toml` `[workspace.package] version`   Release: release.yml on push to main
- Install locally after merge: `target\installer\DialogReader-Setup-x64-<version>.exe /VERYSILENT /SUPPRESSMSGBOXES`   Confirm version: Settings > Updates, or `DialogReader.exe --version`
- Deploy: installer via GitHub Releases
- Signing: unsigned
