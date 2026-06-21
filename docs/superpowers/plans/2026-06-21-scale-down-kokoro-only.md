# Scale-Down: Kokoro-Only TTS + Dead-Code Cleanup Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make Dialogue-reader lighter and leaner by collapsing three TTS engines to Kokoro-only, trimming the voice pool to ~12 good voices, syncing `requirements.txt` with reality, and deleting confirmed dead code.

**Architecture:** The app is already effectively a Kokoro app (the ini `Default` and `Pool` default to Kokoro). Piper and Sherpa exist only to offer extra voices. We remove both engines, delete `sherpa_tts.py`, strip Piper/Sherpa from `tts.py`, remap the 9 `speakers.json` entries that point at Piper/Sherpa voices to Kokoro equivalents, and trim the round-robin pool. OCR stays dual-backend (winocr + EasyOCR) and PySide6 stays — both are explicitly out of scope per the owner's decision.

**Tech Stack:** Python 3.14, kokoro-onnx (TTS), winocr + easyocr (OCR), mss/Pillow/numpy (capture), sounddevice (audio), PySide6 (region picker), pytest.

## Global Constraints

- **No em-dashes** in any code comments, docs, or commit messages. Use en-dashes, commas, or rephrase.
- **OCR stays dual-backend** (winocr + easyocr). Do NOT remove EasyOCR, torch, or any OCR code path.
- **PySide6 stays.** Do NOT rewrite the region picker.
- **Kokoro is the only TTS engine** after this work. Every `speak()` path must go through Kokoro.
- **Existing tests must keep passing:** `tests/test_speakers_load.py`, `tests/test_set_speaker_command.py`.
- Windows-only project. Run pytest from the repo root. Track this work as a GitHub issue + branch + PR (README-only changes excepted, but this is not README-only).
- Each task ends with a commit. Do not chain tasks without the verification step passing.

---

## File Structure

**Deleted:**
- `sherpa_tts.py` (whole file, 239 lines)
- `debug_clean.png`, `debug_normalized.png`, `debug_speaker1.png`, `debug_speaker1_empty.png`, `debug_speaker1_hit.png`, `debug_speaker2_hit.png`, `debug_whitebg.png` (7 OCR debug artifacts in repo root)

**Rewritten:**
- `tts.py` — Kokoro-only (drops Piper + Sherpa, ~460 lines → ~120 lines)
- `requirements.txt` — synced to the true dependency set
- `test_voices.py` — Kokoro-only sample generator

**Modified:**
- `main.py` — trim voice pool to curated Kokoro list, simplify `_expand_voice`, delete Piper pre-download loop, remove dead `pending_frame` buffer, remove `del dialogue_included` no-op
- `dialogue_reader.ini` — `[Voices]` section rewritten for Kokoro-only
- `speakers.json` — remap 9 Piper/Sherpa assignments to Kokoro
- `capture.py` — delete dead `stable_frames()` method
- `magnifier.py` — delete dead `_magnify_process_running()` block

**New tests:**
- `tests/test_tts_kokoro_only.py`
- `tests/test_voice_pool.py`

---

# TIER 1 — Zero-risk cleanup (no behavior change)

These tasks are pure deletions of confirmed-dead code and a requirements sync. They do not change runtime behavior and can land first.

### Task 1: Sync `requirements.txt` with actual imports

**Files:**
- Modify: `requirements.txt`

**Context:** Current file lists `rapidocr-onnxruntime` (imported nowhere) and `sherpa-onnx` (being cut), and omits `sounddevice`, `winocr`, and `easyocr` which are all actually imported. `piper` is imported at `tts.py:40` but will be removed in Tier 2; do not add it.

- [ ] **Step 1: Verify the dead/missing deps**

Run: `grep -rn "import rapidocr\|from rapidocr\|RapidOCR" --include=*.py . | grep -v graphify-out`
Expected: no matches (confirms rapidocr is dead).

Run: `grep -rln "import sounddevice\|import winocr\|import easyocr" --include=*.py . | grep -v graphify-out`
Expected: matches in `main.py`/`tts.py` (sounddevice), `ocr.py` (winocr, easyocr).

- [ ] **Step 2: Replace `requirements.txt` with the true set**

```
mss>=10.0
Pillow>=10.0
numpy>=1.24
sounddevice>=0.4
PySide6>=6.6
kokoro-onnx>=0.3.0
winocr
easyocr>=1.7
```

(Removed: `rapidocr-onnxruntime` dead; `sherpa-onnx` cut in Tier 2. Added: `sounddevice`, `winocr`, `easyocr`.)

- [ ] **Step 3: Commit**

```bash
git add requirements.txt
git commit -m "chore: sync requirements.txt with actual imports (drop rapidocr/sherpa, add sounddevice/winocr/easyocr)"
```

---

### Task 2: Delete OCR debug PNG artifacts from repo root

**Files:**
- Delete: `debug_clean.png`, `debug_normalized.png`, `debug_speaker1.png`, `debug_speaker1_empty.png`, `debug_speaker1_hit.png`, `debug_speaker2_hit.png`, `debug_whitebg.png`

- [ ] **Step 1: Confirm nothing references them**

Run: `grep -rn "debug_clean\|debug_normalized\|debug_speaker\|debug_whitebg" --include=*.py . | grep -v graphify-out`
Expected: no matches.

- [ ] **Step 2: Delete**

```bash
git rm debug_clean.png debug_normalized.png debug_speaker1.png debug_speaker1_empty.png debug_speaker1_hit.png debug_speaker2_hit.png debug_whitebg.png
```

- [ ] **Step 3: Commit**

```bash
git commit -m "chore: remove stale OCR debug PNGs from repo root"
```

---

### Task 3: Delete dead `stable_frames()` from `capture.py`

**Files:**
- Modify: `capture.py` (remove the `stable_frames()` method, currently ~lines 338-376, plus its docstring mention at line 12 and the comment at line 214)

**Context:** `stable_frames()` is never called anywhere; the live loop uses `poll_once()` (`main.py:1099`). Grep confirmed only self-references.

- [ ] **Step 1: Re-confirm it is dead**

Run: `grep -rn "stable_frames" --include=*.py . | grep -v graphify-out`
Expected: only matches inside `capture.py` (def + its own docstring/comment), none in `main.py` or elsewhere.

- [ ] **Step 2: Delete the method**

Remove the entire `def stable_frames(self) -> Iterator[np.ndarray]:` method body (from its `def` line through its final `return`/`yield` block, ~lines 338-376). Also remove the now-stale module-docstring line at `capture.py:12` ("`stable_frames()` blocks until pixels...") and the comment at `capture.py:214` referencing it. If `Iterator` from `typing`/`collections.abc` becomes unused after removal, delete that import too.

- [ ] **Step 3: Verify imports still resolve**

Run: `python -c "import capture; print('capture OK')"`
Expected: `capture OK` (no NameError for `Iterator`).

- [ ] **Step 4: Run the test suite**

Run: `python -m pytest -q`
Expected: all tests pass (capture has no direct tests; this confirms no import breakage).

- [ ] **Step 5: Commit**

```bash
git add capture.py
git commit -m "refactor: remove dead stable_frames() iterator from capture.py"
```

---

### Task 4: Delete dead `_magnify_process_running()` from `magnifier.py`

**Files:**
- Modify: `magnifier.py` (remove `_magnify_process_running()` and its PROCESSENTRY32W toolhelp-snapshot helper, ~lines 94-149)

**Context:** `is_zoomed()` (magnifier.py ~157-168) explicitly stopped calling the process check; its docstring says "Level alone is the most reliable." The block is unreachable.

- [ ] **Step 1: Confirm no caller**

Run: `grep -rn "_magnify_process_running\|magnify_process_running" --include=*.py . | grep -v graphify-out`
Expected: only the definition line in `magnifier.py`, no callers.

- [ ] **Step 2: Delete the block**

Remove `_magnify_process_running()` and any helper/struct it alone uses (the toolhelp `PROCESSENTRY32W` ctypes plumbing). Leave `get_magnification_level()`, `_MagState`, and `is_zoomed()` intact.

- [ ] **Step 3: Verify**

Run: `python -c "import magnifier; print(magnifier.is_zoomed())"`
Expected: prints `True` or `False` with no error.

- [ ] **Step 4: Commit**

```bash
git add magnifier.py
git commit -m "refactor: remove dead magnifier process-snapshot check"
```

---

### Task 5: Remove dead `pending_frame` buffer and `del dialogue_included` no-op in `main.py`

**Files:**
- Modify: `main.py:515` (field), `main.py:774`, `main.py:832`, `main.py:1102` (assignments); `main.py:898,904-905,912` (`dialogue_included`)

**Context:** `WatchedRegion.pending_frame` is written but never read — the OCR worker re-snapshots via `r.capture.snapshot()`. At 12 Hz this allocates a full RGB numpy frame per changed region that nothing consumes. `dialogue_included` is computed then `del`-eted with a comment admitting the guarded case "Can't happen currently."

- [ ] **Step 1: Remove the `pending_frame` field**

In the `WatchedRegion` dataclass (`main.py:514-515`), delete the line:
```python
    pending_frame: np.ndarray | None = None
```
Keep `has_pending_frame: bool = False`.

- [ ] **Step 2: Remove the three `pending_frame` writes**

- `main.py:774` (inside the `result.error` loop): delete `r.pending_frame = None`.
- `main.py:832` (inside `_apply_ocr_result`): delete `r.pending_frame = None`.
- `main.py:1101-1102` (in the poll loop): change
```python
                    if frame is not None:
                        r.has_pending_frame = True
                        r.pending_frame = frame
                        any_changed = True
```
to
```python
                    if frame is not None:
                        r.has_pending_frame = True
                        any_changed = True
```

- [ ] **Step 3: Remove the `dialogue_included` no-op in `_build_batch_job`**

In `main.py:897-912`, delete the `dialogue_included = False` line (898), the `if r.mode == "dialogue": dialogue_included = True` lines (904-905), and the `del dialogue_included` line (912). The function keeps building `specs`, the `if not any(r.has_pending_frame ...)` guard, and the `return OCRBatchJob(...)`.

- [ ] **Step 4: Verify import and tests**

Run: `python -c "import main; print('main OK')"`
Expected: `main OK`.

Run: `python -m pytest -q`
Expected: all pass (test_set_speaker_command imports `main.handle_command`).

- [ ] **Step 5: Commit**

```bash
git add main.py
git commit -m "perf: drop unused per-poll pending_frame buffer and dead dialogue_included tracking"
```

---

# TIER 2 — Kokoro-only TTS + voice trim (the headline)

### Task 6: Add regression tests for Kokoro-only voice parsing and pool (write FIRST)

**Files:**
- Create: `tests/test_tts_kokoro_only.py`
- Create: `tests/test_voice_pool.py`

**Interfaces:**
- Consumes: `tts._parse_voice`, `tts.DEFAULT_VOICE` (exist today); `main._DEFAULT_VOICE_POOL`, `main._expand_voice` (exist today, will be edited in Task 8/9).
- Produces: the invariants later tasks must preserve — bare names parse to `kokoro`, the pool is 100% Kokoro, `kokoro:all` expands to the pool.

**Context:** These tests encode the end-state. They will FAIL now (bare name parses to `piper` today; pool/`_expand_voice` still have Piper/Sherpa) and PASS after Tasks 7-9. This is the TDD anchor for the engine cut.

- [ ] **Step 1: Write `tests/test_tts_kokoro_only.py`**

```python
"""Kokoro-only TTS invariants."""
from tts import _parse_voice, DEFAULT_VOICE


def test_bare_name_defaults_to_kokoro():
    # After the engine cut, a colon-less voice name is a Kokoro voice,
    # not a Piper voice.
    assert _parse_voice("af_heart") == ("kokoro", "af_heart")


def test_explicit_kokoro_voice_parses():
    assert _parse_voice("kokoro:am_michael") == ("kokoro", "am_michael")


def test_default_voice_is_kokoro():
    assert DEFAULT_VOICE.startswith("kokoro:")


def test_tts_module_has_no_piper_or_sherpa_symbols():
    import tts
    # Piper/Sherpa helpers must be gone from the module surface.
    for gone in ("PiperVoice", "_ensure_voice", "_voice_url_base", "_get_sherpa"):
        assert not hasattr(tts, gone), f"{gone} should be removed from tts.py"
```

- [ ] **Step 2: Write `tests/test_voice_pool.py`**

```python
"""Voice pool is Kokoro-only after the scale-down."""
from main import _DEFAULT_VOICE_POOL, _expand_voice


def test_pool_is_all_kokoro():
    assert _DEFAULT_VOICE_POOL, "pool must be non-empty"
    assert all(v.startswith("kokoro:") for v in _DEFAULT_VOICE_POOL)


def test_pool_is_trimmed():
    # Trimmed to a curated set, not all 28.
    assert len(_DEFAULT_VOICE_POOL) <= 14


def test_kokoro_all_expands_to_pool():
    assert _expand_voice("kokoro:all") == [
        v for v in _DEFAULT_VOICE_POOL if v.startswith("kokoro:")
    ]


def test_unknown_voice_passes_through():
    assert _expand_voice("kokoro:af_heart") == ["kokoro:af_heart"]
```

- [ ] **Step 3: Run to confirm they FAIL**

Run: `python -m pytest tests/test_tts_kokoro_only.py tests/test_voice_pool.py -q`
Expected: failures — `test_bare_name_defaults_to_kokoro` (parses to `piper` today), `test_tts_module_has_no_piper_or_sherpa_symbols`, `test_pool_is_all_kokoro` (pool has Piper in `_PIPER_ALL` referenced via `_expand_voice`, and pool currently has 28 Kokoro entries so the trim test fails), etc.

- [ ] **Step 4: Commit the failing tests**

```bash
git add tests/test_tts_kokoro_only.py tests/test_voice_pool.py
git commit -m "test: add Kokoro-only invariants for TTS parsing and voice pool"
```

---

### Task 7: Rewrite `tts.py` as Kokoro-only

**Files:**
- Rewrite: `tts.py`

**Interfaces:**
- Consumes: `kokoro_tts.KokoroTTS` (unchanged).
- Produces: `TTS` with the same public surface `main.py` uses — `TTS(voice, speed)`, `.speak(text, voice=None)`, `.stop()`, `.set_speed(f)`, `.get_speed()`, `.preload(list)`, `.shutdown()`, `._voices_dir`, `._get_kokoro()`; module-level `DEFAULT_VOICE` and `_parse_voice`.

**Context:** Today `tts.py` hard-imports `from piper import PiperVoice` at module level, so importing `tts` requires Piper installed. The rewrite removes that, plus all Sherpa code, the Piper voice cache/download machinery, and `SynthesisConfig`. Kokoro handles speed as a pitch-preserving time-stretch passed straight to `synth()`.

- [ ] **Step 1: Replace the entire file contents**

```python
"""
Natural-voice TTS using the local Kokoro-82M engine.

Voices are referenced as `kokoro:<name>` strings (e.g. `kokoro:af_heart`).
A bare name with no engine prefix is treated as a Kokoro voice name.

Kokoro uses a single shared model (~325 MB on disk, loaded once into RAM);
voice selection is a per-synth parameter, so every voice is effectively free
once the model is loaded.

Usage:
    tts = TTS()                                 # kokoro:af_heart
    tts = TTS(voice="kokoro:am_michael")        # explicit voice
    tts.speak("hello")                          # default voice
    tts.speak("hi", voice="kokoro:bf_emma")     # explicit voice
    tts.shutdown()
"""

from __future__ import annotations

import threading
from pathlib import Path

import numpy as np
import sounddevice as sd


DEFAULT_VOICE = "kokoro:af_heart"


def _parse_voice(voice: str) -> tuple[str, str]:
    """Split 'kokoro:name' into (engine_lower, name). A bare name with no
    colon is treated as a Kokoro voice."""
    if ":" in voice:
        engine, _, name = voice.partition(":")
        return engine.strip().lower(), name.strip()
    return "kokoro", voice.strip()


class TTS:
    def __init__(self, voice: str = DEFAULT_VOICE, speed: float = 1.0):
        self._voices_dir = Path(__file__).parent / "voices"
        self._default_voice = voice

        # Lazy Kokoro engine, initialized on first use.
        self._kokoro = None
        self._kokoro_unavailable = False

        self._speed = max(0.3, min(3.0, float(speed)))

        self._version = 0
        self._version_lock = threading.Lock()

        # Warm the model so the very first speak() is instant.
        self._ensure_default_loaded()

    def _ensure_default_loaded(self) -> None:
        try:
            self._get_kokoro()
        except Exception as e:
            print(f"[tts] Kokoro init failed: {e}", flush=True)

    def _get_kokoro(self):
        """Lazily construct KokoroTTS. Returns None and latches
        _kokoro_unavailable if kokoro-onnx isn't installed or fails to load."""
        if self._kokoro_unavailable:
            return None
        if self._kokoro is not None:
            return self._kokoro
        try:
            from kokoro_tts import KokoroTTS
        except Exception as e:
            print(f"[tts] kokoro_tts module unavailable: {e}", flush=True)
            self._kokoro_unavailable = True
            return None
        try:
            self._kokoro = KokoroTTS(self._voices_dir)
        except Exception as e:
            print(f"[tts] KokoroTTS init failed: {e}", flush=True)
            self._kokoro_unavailable = True
            return None
        return self._kokoro

    def preload(self, voice_names: list[str]) -> None:
        """Trigger one-time Kokoro init so the first speak() is fast. Kokoro
        shares one model across all voices, so individual voice names need no
        separate preloading. `voice_names` is accepted for call-site
        compatibility but only the engine warm-up matters."""
        try:
            self._get_kokoro()
        except Exception as e:
            print(f"[tts] Kokoro preload failed: {e}", flush=True)

    # ---- speed ----

    def set_speed(self, speed: float) -> None:
        """Update playback speed. Takes effect on the next speak() call."""
        self._speed = max(0.3, min(3.0, float(speed)))

    def get_speed(self) -> float:
        return self._speed

    # ---- control ----

    def stop(self) -> None:
        """Cancel any in-progress speech immediately."""
        with self._version_lock:
            self._version += 1
        try:
            sd.stop()
        except Exception:
            pass

    def speak(self, text: str, voice: str | None = None) -> None:
        """Speak `text`. If `voice` is None, uses the default voice. Any
        previously-playing speech is interrupted."""
        if not text:
            return

        try:
            sd.stop()
        except Exception:
            pass

        with self._version_lock:
            self._version += 1
            my_version = self._version

        voice_name = voice or self._default_voice
        _engine, name = _parse_voice(voice_name)

        def worker():
            try:
                k = self._get_kokoro()
                if k is None:
                    return
                # speed is a pitch-preserving time-stretch handled by Kokoro,
                # then played at the native sample rate so pitch is unchanged.
                audio, sample_rate = k.synth(text, name, speed=self._speed)
                if my_version != self._version:
                    return
                sd.play(audio, samplerate=sample_rate, blocking=False)
                if my_version != self._version:
                    try:
                        sd.stop()
                    except Exception:
                        pass
            except Exception as e:
                print(f"[tts] kokoro worker error: {e}", flush=True)

        threading.Thread(target=worker, daemon=True).start()

    def shutdown(self) -> None:
        with self._version_lock:
            self._version += 1
        try:
            sd.stop()
        except Exception:
            pass
```

Note: `numpy as np` is retained only if still referenced; in the body above it is not used directly (Kokoro returns the array). Remove the `import numpy as np` line if `python -W error -c "import tts"` plus a linter flags it as unused — but keep it if any downstream type hint needs it. Safer: delete the `import numpy as np` line since nothing in this file uses `np`.

- [ ] **Step 2: Remove the unused numpy import**

Delete `import numpy as np` from the new `tts.py` (the rewritten body does not reference `np`).

- [ ] **Step 3: Verify it imports without piper/sherpa**

Run: `python -c "import tts; print(tts.DEFAULT_VOICE)"`
Expected: `kokoro:af_heart` with no ImportError about `piper`.

- [ ] **Step 4: Run the new TTS tests**

Run: `python -m pytest tests/test_tts_kokoro_only.py -q`
Expected: all 4 pass.

- [ ] **Step 5: Commit**

```bash
git add tts.py
git commit -m "feat: collapse TTS to Kokoro-only, drop Piper and Sherpa engines"
```

---

### Task 8: Delete `sherpa_tts.py` and strip its references from `main.py`

**Files:**
- Delete: `sherpa_tts.py`
- Modify: `main.py:129-170` (`_expand_voice`), `main.py:109-126` (`_PIPER_ALL`/`_KOKORO_ALL`)

**Context:** `sherpa_tts` is imported only by `tts.py` (already gone after Task 7) and `main._expand_voice` via `get_known_models`. The Piper/Sherpa shorthand expansion is now dead.

- [ ] **Step 1: Delete the file**

```bash
git rm sherpa_tts.py
```

- [ ] **Step 2: Simplify `_expand_voice` in `main.py`**

Replace the whole `_expand_voice` function (`main.py:129-170`) with:

```python
def _expand_voice(voice: str) -> list[str]:
    """Expand shorthand voice entries into a concrete list.

    Supported forms:
      - `kokoro:all`  -> every curated Kokoro voice
      - any other single voice passes through unchanged.
    """
    v = voice.strip()
    if not v:
        return []
    if v == "kokoro:all":
        return list(_KOKORO_ALL)
    return [v]
```

- [ ] **Step 3: Remove the `_PIPER_ALL` table**

Delete the `_PIPER_ALL = [ ... ]` list (`main.py:112-125`). Keep `_KOKORO_ALL = [v for v in _DEFAULT_VOICE_POOL if v.startswith("kokoro:")]` (it will be redefined trivially once the pool is all Kokoro, but leaving the comprehension is harmless and robust).

- [ ] **Step 4: Verify no lingering sherpa references**

Run: `grep -rn "sherpa\|SherpaTTS\|get_known_models\|_PIPER_ALL" --include=*.py . | grep -v graphify-out | grep -v "tests/"`
Expected: no matches in `main.py`, `tts.py` (only possibly in `dialogue_reader.ini` comments, handled in Task 11).

Run: `python -c "import main; print('main OK')"`
Expected: `main OK`.

- [ ] **Step 5: Commit**

```bash
git add main.py
git rm sherpa_tts.py
git commit -m "refactor: delete sherpa_tts.py and Piper/Sherpa pool expansion"
```

---

### Task 9: Trim the voice pool to a curated Kokoro set

**Files:**
- Modify: `main.py:78-107` (`_DEFAULT_VOICE_POOL`)

**Context:** Today the pool is all 28 Kokoro voices including D/F-grade entries. Trim to 12 good voices, ordered F/M-ish for a natural round-robin spread. Assignments in `speakers.json` to voices NOT in this pool still work (Kokoro can synth any of its 28 voices); the pool only governs round-robin auto-assignment and cycling.

- [ ] **Step 1: Replace `_DEFAULT_VOICE_POOL`**

```python
_DEFAULT_VOICE_POOL = [
    "kokoro:af_heart",      # F  grade A   (US)
    "kokoro:am_fenrir",     # M  grade C+  (US)
    "kokoro:af_bella",      # F  grade A-  (US)
    "kokoro:am_michael",    # M  grade C+  (US)
    "kokoro:bf_emma",       # F  grade B-  (British)
    "kokoro:am_puck",       # M  grade C+  (US)
    "kokoro:af_nicole",     # F  grade B-  (US, ASMR/whisper)
    "kokoro:bm_fable",      # M  grade C   (British)
    "kokoro:af_aoede",      # F  grade C+  (US)
    "kokoro:bm_george",     # M  grade C   (British)
    "kokoro:af_kore",       # F  grade C+  (US)
    "kokoro:af_sarah",      # F  grade C+  (US)
]
```

(The owner can adjust which 12 by ear later; this is the curated default. It must stay 100% `kokoro:` and <= 14 entries to satisfy `tests/test_voice_pool.py`.)

- [ ] **Step 2: Run the pool tests**

Run: `python -m pytest tests/test_voice_pool.py -q`
Expected: all 4 pass.

- [ ] **Step 3: Run the full suite**

Run: `python -m pytest -q`
Expected: all pass.

- [ ] **Step 4: Commit**

```bash
git add main.py
git commit -m "feat: trim round-robin voice pool to 12 curated Kokoro voices"
```

---

### Task 10: Remove the Piper pre-download loop from `main()`

**Files:**
- Modify: `main.py:964-978`

**Context:** This loop imports `_ensure_voice` from `tts` (removed in Task 7) and pre-downloads every Piper voice in the pool. With Kokoro-only, Kokoro's single model is fetched lazily by `KokoroTTS` on first synth, so the loop is both broken (no `_ensure_voice`) and unnecessary.

- [ ] **Step 1: Delete the loop**

Remove `main.py:964-978` in full — the comment block plus:
```python
    from tts import _ensure_voice, _parse_voice
    for voice_name in voice_pool:
        if voice_name == default_voice:
            continue
        engine, inner = _parse_voice(voice_name)
        if engine != "piper":
            continue
        try:
            _ensure_voice(tts._voices_dir, inner)
        except Exception as e:
            print(f"[tts] could not pre-download '{voice_name}': {e}")
```

- [ ] **Step 2: Verify import (catches the now-removed `_ensure_voice` reference)**

Run: `python -c "import main; print('main OK')"`
Expected: `main OK`.

- [ ] **Step 3: Run the suite**

Run: `python -m pytest -q`
Expected: all pass.

- [ ] **Step 4: Commit**

```bash
git add main.py
git commit -m "refactor: drop Piper pre-download loop (Kokoro self-caches one model)"
```

---

### Task 11: Rewrite the `[Voices]` section of `dialogue_reader.ini`

**Files:**
- Modify: `dialogue_reader.ini:56-95`

**Context:** The section documents Piper/Sherpa engines, sherpa sub-models, and range syntax that no longer exist. Replace with Kokoro-only docs. `Default` stays `kokoro:af_heart`; `Pool` becomes `kokoro:all` (which now expands to the 12 curated voices).

- [ ] **Step 1: Replace lines 56-95 with**

```
[Voices]
; --- Voice string format ---------------------------------------------------
;   kokoro:<id>        a specific Kokoro voice by name (e.g. kokoro:af_heart)
;   <id>               a bare name (no colon) is treated as a Kokoro voice.
;
; --- Shorthand (auto-expanded in Pool) ------------------------------------
;   kokoro:all         every voice in the curated pool (see main.py
;                      _DEFAULT_VOICE_POOL). Edit that list to change the set.
;
; Browse the full Kokoro voice list in: docs/voices/ (kokoro grades).
; Any of Kokoro's 28 voices can be assigned to a specific speaker by name even
; if it is not in the round-robin Pool.

Default=kokoro:af_heart

Pool=kokoro:all
```

- [ ] **Step 2: Verify config still loads**

Run: `python -c "from main import _load_voice_config; pool, default = _load_voice_config(); print(len(pool), default); assert all(v.startswith('kokoro:') for v in pool)"`
Expected: prints `12 kokoro:af_heart` (or pool length matching the curated list) with no assertion error.

- [ ] **Step 3: Commit**

```bash
git add dialogue_reader.ini
git commit -m "docs: rewrite [Voices] ini section for Kokoro-only"
```

---

### Task 12: Remap Piper/Sherpa assignments in `speakers.json` to Kokoro

**Files:**
- Modify: `speakers.json`

**Context:** 9 of the 26 assignments point at `piper:` or `sherpa:` voices that no longer have an engine. They must be remapped to Kokoro voices or those speakers will fail to synth. The other 17 already use valid Kokoro voices and are left untouched. `cycle_index` values reference positions in the old 150-voice pool; reset them so cycling rederives cleanly against the new 12-voice pool.

- [ ] **Step 1: Apply this remap to the `assignments` block**

| Speaker | Old | New |
|---|---|---|
| `__default__` | `piper:en_US-ryan-high` | `kokoro:am_michael` |
| `B-King Leader` | `piper:en_US-norman-medium` | `kokoro:am_fenrir` |
| `B-King Thug` | `sherpa:libritts_r:879` | `kokoro:am_puck` |
| `Yakuza` | `sherpa:vctk:30` | `kokoro:bm_george` |
| `Antonio` | `sherpa:vctk:0` | `kokoro:bm_fable` |
| `KK:` | `sherpa:vctk:37` | `kokoro:am_eric` |
| `off` | `sherpa:vctk:38` | `kokoro:am_onyx` |
| `Enjoy` | `sherpa:vctk:39` | `kokoro:bf_emma` |
| `sure` | `sherpa:vctk:40` | `kokoro:af_bella` |

Leave all other assignments exactly as they are (they are already `kokoro:`).

- [ ] **Step 2: Reset `cycle_index` and `next_auto_index`**

Replace the `cycle_index` object with `{}` and set `next_auto_index` to `0`. The loader (`speakers.py`, covered by `tests/test_speakers_load.py`) tolerates a missing/empty `cycle_index` and rederives indices on demand.

- [ ] **Step 3: Verify no non-Kokoro voices remain**

Run: `grep -n "piper:\|sherpa:" speakers.json`
Expected: no matches.

Run: `python -c "import json; d=json.load(open('speakers.json')); assert all(v.startswith('kokoro:') for v in d['assignments'].values()); print('all kokoro')"`
Expected: `all kokoro`.

- [ ] **Step 4: Smoke-test SpeakerManager loads it**

Run: `python -c "from pathlib import Path; from speakers import SpeakerManager; m=SpeakerManager(voice_pool=['kokoro:af_heart','kokoro:am_michael'], save_path=Path('speakers.json')); print(len(m.assignments), 'speakers loaded')"`
Expected: prints `26 speakers loaded` with no error.

- [ ] **Step 5: Commit**

```bash
git add speakers.json
git commit -m "fix: remap Piper/Sherpa speaker assignments to Kokoro voices"
```

---

### Task 13: Trim `test_voices.py` to Kokoro-only

**Files:**
- Rewrite: `test_voices.py`

**Context:** The sample generator currently imports `piper.config` and targets `sherpa:` voices. Both break post-cut. Trim to a Kokoro-only sample generator (keeps a handy "render these voices to WAV" dev tool without the dead engines).

- [ ] **Step 1: Replace the file contents**

```python
"""
Sample-generator for Kokoro voices: renders each voice to a WAV in the
project root so you can listen and compare.

Run:
    python test_voices.py                       # render all SAMPLE_VOICES
    python test_voices.py kokoro:af_heart kokoro:am_michael
"""

from __future__ import annotations

import sys
import wave
from pathlib import Path

import numpy as np

SAMPLE_VOICES = [
    "kokoro:af_heart",
    "kokoro:am_michael",
    "kokoro:bf_emma",
    "kokoro:bm_george",
]

TEST_TEXT = (
    "Hello there. This is a sample of my voice. I like long walks on the "
    "beach and a good cup of tea. How do I sound to you?"
)


def render(voice: str, out_dir: Path) -> Path:
    from tts import TTS, _parse_voice

    _engine, name = _parse_voice(voice)
    tts = TTS(voice=voice, speed=1.0)
    k = tts._get_kokoro()
    assert k is not None, "Kokoro engine unavailable"
    audio, sr = k.synth(TEST_TEXT, name, speed=1.0)

    safe = voice.replace(":", "_").replace("/", "_")
    out = out_dir / f"sample_{safe}.wav"
    pcm = (np.clip(audio, -1.0, 1.0) * 32767).astype(np.int16)
    with wave.open(str(out), "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(sr)
        w.writeframes(pcm.tobytes())
    return out


def main() -> int:
    voices = sys.argv[1:] if len(sys.argv) > 1 else SAMPLE_VOICES
    out_dir = Path(__file__).parent
    for voice in voices:
        try:
            path = render(voice, out_dir)
            print(f"[ok] {voice}  ->  {path.name}")
        except Exception as e:
            print(f"[err] {voice}: {e}", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
```

- [ ] **Step 2: Verify it imports cleanly**

Run: `python -c "import test_voices; print('test_voices OK')"`
Expected: `test_voices OK` (no `piper` import error).

- [ ] **Step 3: Commit**

```bash
git add test_voices.py
git commit -m "refactor: trim test_voices.py to a Kokoro-only sample generator"
```

---

### Task 14: Full verification pass

**Files:** none (verification only).

- [ ] **Step 1: Full test suite**

Run: `python -m pytest -q`
Expected: all tests pass, including the new Kokoro-only and pool tests.

- [ ] **Step 2: Import smoke across all runtime modules**

Run: `python -c "import main, tts, kokoro_tts, ocr, capture, window_capture, magnifier, region_picker, speakers, command_server; print('all imports OK')"`
Expected: `all imports OK`.

- [ ] **Step 3: Confirm no dangling Piper/Sherpa references in runtime code**

Run: `grep -rn "piper\|sherpa\|PiperVoice\|SynthesisConfig" --include=*.py . | grep -v graphify-out | grep -v tests/`
Expected: no matches (or only inside `docs/`). If any remain in `main.py`/`tts.py`/`ocr.py`, fix before finishing.

- [ ] **Step 4: Live Kokoro synth smoke test (manual, optional but recommended)**

Run: `python test_voices.py kokoro:af_heart`
Expected: writes `sample_kokoro_af_heart.wav`; play it to confirm audio is intelligible. (First run downloads the ~325 MB Kokoro model if not cached.)

- [ ] **Step 5: Final commit if any fixes were needed, then open the PR**

```bash
git add -A
git commit -m "chore: final verification fixes for Kokoro-only scale-down"
```

Open a PR referencing the tracking issue. In the PR body, note the footprint reduction (Piper + Sherpa engines and models removed, requirements synced) and that OCR/Qt were intentionally left in scope.

---

# TIER 3 — Optional efficiency follow-ups (NOT yet committed)

These came out of the audit as "more efficient" wins but are behavior-preserving refactors the owner has not signed off on. They are listed here as candidate future work, not fully planned tasks. Decide after Tier 1-2 lands.

- **Consolidate the 6 ini loaders** (`_load_voice_config`, `_load_speaker_assignment_strategy`, `_load_capture_mode`, `_load_ocr_config`, `_load_skip_when_zoomed`, `_load_text_confirm_polls`) into one `_load_config()` that reads `dialogue_reader.ini` once. Removes ~80-90 lines and 5 redundant disk reads/parses at startup. Medium effort, low risk.
- **Reduce `POLL_HZ`** from 12 to ~6 (capture.py / main.py). Halves idle capture+hash CPU; dialogue changes well under 2 Hz. Needs a quick latency check on a real game first.
- **Replace md5 frame hashing** with a cheap non-crypto equality check (`_hash_frame` in capture.py, `_hash_frame_fast` in ocr.py — consolidate to one). Low impact, low risk.
- **Simplify the orphan-killer** (`_find_orphan_pids`/`_kill_orphans`, ~150 lines shelling out to `powershell.exe Get-CimInstance` at every startup) to a lockfile/named-mutex singleton check. The parent watchdog already covers orphan death. Medium risk — the owner relies on robust orphan cleanup on Windows, so this needs care.
- **Trim the speaker fuzzy-matching** in `speakers.py` (`_prefix_extension_match`, `_prefix_shrink_match`, `_has_garbled_extension`, ~95 lines) to exact + single-char fuzzy. Medium risk — changes name-matching behavior the owner tuned for OCR variants.

---

## Self-Review

**Spec coverage (against the owner's decisions):**
- Kokoro-only TTS, remap Piper/Sherpa assignments first → Tasks 7, 8, 12. ✓
- Keep EasyOCR / torch → no OCR task touches it; Global Constraints forbid it. ✓
- Keep PySide6 → no region-picker task; Global Constraints forbid it. ✓
- Trim voices to good ones → Task 9 (pool) + Task 12 (assignments). ✓
- Lighter / dead-code → Tasks 1-5, 10, 13. ✓
- Plan-first, checkpoint per task → every task ends in commit + verify. ✓

**Placeholder scan:** no TBD/TODO; every code step shows full content; deletions cite exact line ranges. ✓

**Type/symbol consistency:** new `tts.py` preserves every symbol `main.py` consumes (`TTS`, `DEFAULT_VOICE`, `_parse_voice`, `.speak/.stop/.set_speed/.get_speed/.preload/.shutdown`, `._voices_dir`, `._get_kokoro`). `_KOKORO_ALL`/`_DEFAULT_VOICE_POOL`/`_expand_voice` names match between Tasks 8 and 9 and the tests in Task 6. ✓
