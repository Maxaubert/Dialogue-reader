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
4. UI shell: overlay over borderless games, hotkeys vs elevated games.

## Results
- Spike 2, Satisfactory: [spike2-satisfactory.md](spike2-satisfactory.md)
