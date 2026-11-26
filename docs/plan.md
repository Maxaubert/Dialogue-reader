# Plan

Source report (2026-11-21): `C:\Users\Admin\Documents\Claude\research\dialogue-reader\2026-11-21-reimagine-directions.md`

## Decisions (2026-11-21, Max)
- Direction A: OCR-first, one process, continuous window capture, frame ring buffer,
  typed zones (dialogue, speaker, notification, ignore). Engine plugins later, if ever.
- Language: Rust, if the ort + Kokoro spike passes. C#/.NET 10 is the fallback.
- Target games: mostly custom and Unreal engines (RDR2, Yakuza/Judgment, Persona, Death Stranding,
  KCD2, Clair Obscur, MGS Master Collection, emulated Animal Crossing). Toasts and tooltips matter.

## Phase 0 spikes
1. OCR benchmark on labelled screenshots from Max's games (WinRT OCR, PP-OCRv5 ONNX, others).
2. Capture + toast replay: does Windows Graphics Capture work on each game, how long do toasts stay,
   where do they land. Tool: `spikes/recorder`.
3. Rust runtime: ort + DirectML + Kokoro time-to-first-audio.
4. UI shell: overlay over borderless games, hotkeys vs elevated games. It also binds the manual-mode read key
   (`dr_core::Manual`, see [reader-a1.md](reader-a1.md#manual-mode)). The user picks the key during onboarding
   (Max, 2026-11-22); there is no fixed default.
5. UI redesign: the whole UI is redesigned from scratch (Max, 2026-11-22). Starts with mockups. Stack: Slint (Rust), chosen 2026-11-22
   over Tauri, egui and Qt. Look: concept design/gpt-image/v06.webp, chosen by Max
   2026-11-22 (compact dark, big game covers in the sidebar, three columns). Full screen set in design/mockups/v06.

## Results
- Spike 2, Satisfactory: [spike2-satisfactory.md](spike2-satisfactory.md)
- Test arena baseline: [arena-baseline.md](arena-baseline.md)
- First real reader, A1 (`crates/core`, arena reader `a1`): [reader-a1.md](reader-a1.md)
- Manual mode (2026-11-22, Max: "click a key and it checks and reads all zones, and never does anything outside"):
  [reader-a1.md#manual-mode](reader-a1.md#manual-mode)

## Idea: in-game quick menu (Max, 2026-11-22)
- A second hotkey (or holding the read key) opens a small menu overlay over the game, like RivaTuner or WeMod.
- It controls things without leaving the game: mute, stop speaking, pause reading, mode, voice and speed, zones on/off, open the main window or settings, quit.
- Must work with keyboard only (games capture the mouse): arrow or number keys, Esc closes.
- An overlay window works over borderless and windowed games. Exclusive fullscreen would need injection; check in spike 4.
- Mockups: design/mockups/c-midnight/15-quick-menu-*.html.

## The app (2026-11-22)
- Spec: [app-spec.md](app-spec.md) (architecture, Slint tokens and page map, risks).
- Milestones M1 to M5: [app-plan.md](app-plan.md).
