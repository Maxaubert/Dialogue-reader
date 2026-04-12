# Dialogue Reader — Execution Plan

A tool that watches a user-selected region of the screen for new text
(game dialogue boxes, subtitles) and reads it aloud via TTS.

## Pipeline

```
[ region picker ]  ->  [ capture loop ]  ->  [ change detect ]
                                                    |
                                                    v
                                              [ OCR engine ]
                                                    |
                                                    v
                                          [ text diff vs prev ]
                                                    |
                                                    v
                                            [ TTS (interruptible) ]
```

## Stages — simple to complex

### Stage 1 — MVP (this pass)
- One Python script you run from a terminal.
- User picks a screen region with a click-and-drag overlay.
- Captures that region ~10x/sec.
- Detects when pixels change AND then stay still for ~250ms (text fully revealed).
- Runs OCR on the stable frame.
- If text differs from last spoken text, send to TTS.
- TTS uses Windows SAPI (`pyttsx3`) — instant, robotic but functional.
- New text interrupts currently-playing speech.
- Ctrl+C to quit.

### Stage 2 — Quality of life
- Save/load region profiles per game (JSON).
- Min/max text length filters (skip "Press A").
- Regex ignore list ("^Press", "Loading...", etc.).
- Voice/speed settings.
- Pause/resume hotkey.

### Stage 3 — Robustness
- Bind region to a window (by title) instead of absolute screen coords.
- DPI-awareness fix.
- Multi-region support per profile (subtitles + speaker name).
- Typewriter-text handling: wait for the text to stop *growing* before speaking.

### Stage 4 — Polish
- System tray app (pystray).
- Live OCR preview / calibration mode.
- Better TTS engine: bundle Piper for natural neural voices.
- Settings GUI.

### Stage 5 — Distribution
- Package with PyInstaller `--onedir`.
- Antivirus false-positive submission if needed.

## File layout (MVP)

```
dialogue-reader/
  PLAN.md
  requirements.txt
  main.py             # entrypoint, main loop
  region_picker.py    # PySide6 transparent overlay
  capture.py          # mss capture + change detection
  ocr.py              # OCR wrapper
  tts.py              # pyttsx3 wrapper, interruptible
```

## Dependency choices and why

- **mss** — fastest cross-platform screen capture, pure-pip.
- **Pillow / numpy** — image handling, hashing.
- **PySide6** — region picker overlay. Has Python 3.14 wheels.
- **pyttsx3** — Windows SAPI wrapper. Zero-download, instant. Good enough for MVP.
- **rapidocr-onnxruntime** — pure-pip OCR, bundles small ONNX models, no Tesseract install needed. Falls back to **pytesseract** if 3.14 wheels aren't ready.
