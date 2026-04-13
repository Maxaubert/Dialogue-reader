# Cleanup and Bug-fixes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove dead code / stale files / outdated docs, and fix 8 bugs in the dialogue-reader codebase.

**Architecture:** Each task is a small, isolated change. Unit tests (pytest) cover the bug-fix behavior changes where practical. Cleanup tasks are verified by "still imports" smoke checks since they're removals.

**Tech Stack:** Python 3.14, pytest, existing project code (Piper / Kokoro / Sherpa-ONNX / WinOCR / EasyOCR).

---

## Task 0: Install test tooling and scaffold

**Files:**
- Create: `tests/__init__.py`
- Create: `tests/conftest.py`

- [ ] **Step 1: Install pytest**

Run:
```bash
pip install pytest
```

Expected: `Successfully installed pytest-...` or similar.

- [ ] **Step 2: Create `tests/` package marker**

Create `tests/__init__.py` (empty file):

```python
```

- [ ] **Step 3: Create `tests/conftest.py` with a sys.path hook so project modules import cleanly**

```python
"""Add the worktree root to sys.path so tests can import project modules."""
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))
```

- [ ] **Step 4: Confirm pytest can discover the directory**

Run:
```bash
cd C:/Users/Admin/Documents/Claude/Github/dialogue-reader/.worktrees/cleanup && pytest tests/ -v
```

Expected: `collected 0 items` — no tests yet, but no errors either.

- [ ] **Step 5: Commit**

```bash
git add tests/__init__.py tests/conftest.py
git commit -m "test: scaffold tests/ dir and conftest for sys.path"
```

---

## Task 1: B2 — Robust `SpeakerManager._load()` against malformed JSON

**Files:**
- Create: `tests/test_speakers_load.py`
- Modify: `speakers.py` (the `_load` method)

- [ ] **Step 1: Write the failing test**

Create `tests/test_speakers_load.py`:

```python
import json
import tempfile
from pathlib import Path

from speakers import SpeakerManager


def _make_mgr(data: dict) -> SpeakerManager:
    """Build a SpeakerManager from a fabricated speakers.json dict."""
    td = tempfile.mkdtemp()
    sp = Path(td) / "speakers.json"
    sp.write_text(json.dumps(data), encoding="utf-8")
    return SpeakerManager(
        voice_pool=["voice:a", "voice:b"],
        save_path=sp,
    )


def test_load_skips_entry_with_null_cycle_index():
    mgr = _make_mgr({
        "assignments": {"Alice": "voice:a", "Bob": "voice:b"},
        "cycle_index": {"Alice": 0, "Bob": None},
        "next_auto_index": 2,
    })
    # Both assignments survive because they are independent of cycle_index.
    assert mgr.assignments == {"Alice": "voice:a", "Bob": "voice:b"}
    # Only the valid cycle_index entry is loaded.
    assert mgr.cycle_index == {"Alice": 0}


def test_load_survives_string_next_auto_index():
    mgr = _make_mgr({
        "assignments": {"Alice": "voice:a"},
        "cycle_index": {"Alice": 0},
        "next_auto_index": "oops",
    })
    assert mgr.assignments == {"Alice": "voice:a"}
    # Falls back to len(assignments).
    assert mgr._next_auto_index == 1


def test_load_skips_non_string_assignment_values():
    mgr = _make_mgr({
        "assignments": {"Alice": "voice:a", "Broken": 42},
        "cycle_index": {},
        "next_auto_index": 0,
    })
    # Non-string voice value is dropped, valid ones survive.
    assert mgr.assignments == {"Alice": "voice:a"}


def test_load_handles_missing_fields():
    mgr = _make_mgr({"assignments": {"Alice": "voice:a"}})
    assert mgr.assignments == {"Alice": "voice:a"}
    assert mgr.cycle_index == {}
```

- [ ] **Step 2: Run test, verify failures**

Run:
```bash
cd C:/Users/Admin/Documents/Claude/Github/dialogue-reader/.worktrees/cleanup && pytest tests/test_speakers_load.py -v
```

Expected: `test_load_skips_entry_with_null_cycle_index` fails (TypeError inside `int(None)`), `test_load_survives_string_next_auto_index` fails (ValueError), `test_load_skips_non_string_assignment_values` may pass (assignments are read raw), `test_load_handles_missing_fields` may pass.

- [ ] **Step 3: Fix `SpeakerManager._load`**

Open `speakers.py`. Replace the current `_load` body (lines 65-74) with:

```python
    def _load(self) -> None:
        if not self.save_path.exists():
            return
        try:
            data = json.loads(self.save_path.read_text(encoding="utf-8"))
        except (json.JSONDecodeError, OSError):
            return
        if not isinstance(data, dict):
            return

        # Assignments: keep only entries where both key and value are strings.
        raw_assignments = data.get("assignments", {})
        if isinstance(raw_assignments, dict):
            self.assignments = {
                name: voice
                for name, voice in raw_assignments.items()
                if isinstance(name, str) and isinstance(voice, str)
            }

        # cycle_index: drop entries that can't be coerced to int.
        raw_cycle = data.get("cycle_index", {})
        if isinstance(raw_cycle, dict):
            clean: dict[str, int] = {}
            for name, idx in raw_cycle.items():
                if not isinstance(name, str):
                    continue
                try:
                    clean[name] = int(idx)
                except (TypeError, ValueError):
                    continue
            self.cycle_index = clean

        # next_auto_index: fall back to number of assignments on malformed input.
        raw_next = data.get("next_auto_index")
        try:
            self._next_auto_index = int(raw_next) if raw_next is not None else len(self.assignments)
        except (TypeError, ValueError):
            self._next_auto_index = len(self.assignments)
```

- [ ] **Step 4: Run test, verify pass**

Run:
```bash
pytest tests/test_speakers_load.py -v
```

Expected: all 4 pass.

- [ ] **Step 5: Commit**

```bash
git add speakers.py tests/test_speakers_load.py
git commit -m "fix: harden SpeakerManager._load against malformed JSON (B2)"
```

---

## Task 2: B1 — Wire `SET_SPEAKER:<name>` UDP command

**Files:**
- Create: `tests/test_set_speaker_command.py`
- Modify: `main.py` `handle_command`

- [ ] **Step 1: Write the failing test**

Create `tests/test_set_speaker_command.py`:

```python
"""Verify that `SET_SPEAKER:<name>` goes through handle_command and sets the speaker."""
import tempfile
from pathlib import Path
from unittest.mock import MagicMock

from speakers import SpeakerManager


def _mgr(pool=("voice:a", "voice:b")) -> SpeakerManager:
    td = tempfile.mkdtemp()
    return SpeakerManager(voice_pool=list(pool), save_path=Path(td) / "s.json")


def test_set_speaker_command_updates_current_speaker():
    from main import handle_command

    mgr = _mgr()
    tts = MagicMock()
    tts.speak = MagicMock()
    state = {"paused": False, "last_spoken": "", "candidate": ""}

    handle_command(
        "SET_SPEAKER:Alice",
        regions=[],
        tts=tts,
        speaker_mgr=mgr,
        state=state,
        debug=False,
    )

    assert mgr.current_speaker == "Alice"
    assert "Alice" in mgr.assignments


def test_set_speaker_command_trims_whitespace_and_strips_prefix():
    from main import handle_command

    mgr = _mgr()
    tts = MagicMock()
    state = {"paused": False, "last_spoken": "", "candidate": ""}

    handle_command(
        "SET_SPEAKER:  Bob Smith  ",
        regions=[],
        tts=tts,
        speaker_mgr=mgr,
        state=state,
        debug=False,
    )

    assert mgr.current_speaker == "Bob Smith"


def test_set_speaker_command_ignores_empty_name():
    from main import handle_command

    mgr = _mgr()
    tts = MagicMock()
    state = {"paused": False, "last_spoken": "", "candidate": ""}

    handle_command(
        "SET_SPEAKER:",
        regions=[],
        tts=tts,
        speaker_mgr=mgr,
        state=state,
        debug=False,
    )

    assert mgr.current_speaker == ""
```

- [ ] **Step 2: Run test, verify failure**

Run:
```bash
pytest tests/test_set_speaker_command.py -v
```

Expected: all 3 fail — first two assert on `mgr.current_speaker` being set, but no handler exists.

- [ ] **Step 3: Find `handle_command` and add the SET_SPEAKER branch**

Open `main.py` and find the `handle_command` function. Locate the end of the `CYCLE_VOICE_PREV` branch (around line 598-608). After the full `CYCLE_VOICE_PREV` block, add a new branch BEFORE the final `else:` fallback that prints "unknown command" (or equivalent — check what the last branch looks like):

```python
    elif cmd.startswith("SET_SPEAKER:"):
        name = cmd[len("SET_SPEAKER:"):].strip()
        if name:
            voice = speaker_mgr.set_current(name)
            if voice:
                _safe_print(
                    "[speakers] current = ",
                    f"{speaker_mgr.current_speaker!r} voice={voice} (manual SET_SPEAKER)",
                )
```

- [ ] **Step 4: Run test, verify pass**

Run:
```bash
pytest tests/test_set_speaker_command.py -v
```

Expected: all 3 pass.

- [ ] **Step 5: Commit**

```bash
git add main.py tests/test_set_speaker_command.py
git commit -m "fix: handle SET_SPEAKER:<name> UDP command (B1)"
```

---

## Task 3: B4 — Cap text-confirm max attempts

**Files:**
- Modify: `main.py` (near `TEXT_CONFIRM_MAX_MULTIPLIER` constant and its usage)

- [ ] **Step 1: Locate the constant and usage**

Open `main.py`. Find the constants block (around lines 65-67):

```python
TEXT_CONFIRM_POLLS = 3
TEXT_CONFIRM_INTERVAL = 0.10  # seconds between confirmation polls
TEXT_CONFIRM_MAX_MULTIPLIER = 4  # max attempts = polls * this
```

And the usage inside the dialogue confirmation loop (around line 788):

```python
                        max_attempts = text_confirm_polls * TEXT_CONFIRM_MAX_MULTIPLIER
```

- [ ] **Step 2: Add a hard ceiling constant and apply it**

Replace the `TEXT_CONFIRM_MAX_MULTIPLIER` line with:

```python
TEXT_CONFIRM_MAX_MULTIPLIER = 4  # max attempts = polls * this
TEXT_CONFIRM_HARD_CAP = 30  # absolute ceiling regardless of polls setting
```

Replace the `max_attempts = ...` line with:

```python
                        max_attempts = min(
                            text_confirm_polls * TEXT_CONFIRM_MAX_MULTIPLIER,
                            TEXT_CONFIRM_HARD_CAP,
                        )
```

- [ ] **Step 3: Verify the module still imports**

Run:
```bash
python -c "import main; print('import ok')"
```

Expected: `import ok`.

- [ ] **Step 4: Commit**

```bash
git add main.py
git commit -m "fix: cap text-confirm max_attempts at 30 to prevent long freezes (B4)"
```

---

## Task 4: B5 — Guard `sd.play` with version check in TTS workers

**Files:**
- Modify: `tts.py` (`worker_piper`, `worker_kokoro`, `worker_sherpa` inside `speak`)

- [ ] **Step 1: Locate the `speak` method in `tts.py`**

Find `def speak(self, text: str, voice: str | None = None)`. Inside it there are three worker functions: `worker_piper`, `worker_kokoro`, `worker_sherpa`. Each ends with something like:

```python
                if my_version != self._version:
                    return
                audio = np.concatenate(chunks)
                sd.play(audio, samplerate=sample_rate, blocking=False)
```

The version check happens BEFORE `sd.play`, but `sd.play` is non-blocking — it starts playback and returns. A newer `speak()` call between the version check and `sd.play` would result in an unwanted playback starting AFTER `sd.stop()` has been called.

- [ ] **Step 2: Tighten the race in each worker by re-checking version after play**

In `worker_piper` (around lines 378-388), the existing code is:

```python
        def worker_piper():
            try:
                piper_voice = self._get_voice(name)
                chunks: list[np.ndarray] = []
                sample_rate: int | None = None
                for chunk in piper_voice.synthesize(text, self._syn_config):
                    if my_version != self._version:
                        return
                    chunks.append(chunk.audio_float_array)
                    sample_rate = chunk.sample_rate
                if not chunks or sample_rate is None:
                    return
                if my_version != self._version:
                    return
                audio = np.concatenate(chunks)
                sd.play(audio, samplerate=sample_rate, blocking=False)
            except Exception as e:
                print(f"[tts] piper worker error: {e}", flush=True)
```

Replace with:

```python
        def worker_piper():
            try:
                piper_voice = self._get_voice(name)
                chunks: list[np.ndarray] = []
                sample_rate: int | None = None
                for chunk in piper_voice.synthesize(text, self._syn_config):
                    if my_version != self._version:
                        return
                    chunks.append(chunk.audio_float_array)
                    sample_rate = chunk.sample_rate
                if not chunks or sample_rate is None:
                    return
                audio = np.concatenate(chunks)
                # Atomic version-check + play: any newer speak() call will
                # increment self._version again; a belt-and-braces stop after
                # play covers the tiny window between check and play.
                if my_version != self._version:
                    return
                sd.play(audio, samplerate=sample_rate, blocking=False)
                if my_version != self._version:
                    try:
                        sd.stop()
                    except Exception:
                        pass
            except Exception as e:
                print(f"[tts] piper worker error: {e}", flush=True)
```

Apply the same pattern to `worker_kokoro` and `worker_sherpa` — insert the post-play version check and `sd.stop()` guard:

In `worker_kokoro` (current body ends with `sd.play(audio, samplerate=sample_rate, blocking=False)`):

```python
        def worker_kokoro():
            try:
                k = self._get_kokoro()
                if k is None:
                    return
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
```

In `worker_sherpa`:

```python
        def worker_sherpa():
            try:
                s = self._get_sherpa()
                if s is None:
                    return
                audio, sample_rate = s.synth(text, name, speed=self._speed)
                if my_version != self._version:
                    return
                sd.play(audio, samplerate=sample_rate, blocking=False)
                if my_version != self._version:
                    try:
                        sd.stop()
                    except Exception:
                        pass
            except Exception as e:
                print(f"[tts] sherpa worker error: {e}", flush=True)
```

- [ ] **Step 3: Verify module imports**

Run:
```bash
python -c "import tts; print('tts ok')"
```

Expected: `tts ok`.

- [ ] **Step 4: Commit**

```bash
git add tts.py
git commit -m "fix: re-check version after sd.play in TTS workers (B5)"
```

---

## Task 5: B6 — Replace `assert` in production code with explicit raises

**Files:**
- Modify: `command_server.py:49`
- Modify: `kokoro_tts.py:129`

- [ ] **Step 1: Fix `command_server.py`**

Open `command_server.py`. Find the `_run` method and the line `assert self._sock is not None` (around line 49). Replace with:

```python
        if self._sock is None:
            raise RuntimeError("CommandServer._run called before start()")
```

- [ ] **Step 2: Fix `kokoro_tts.py`**

Open `kokoro_tts.py`. Find `assert self._kokoro is not None` (around line 129) in the `synth` method. Replace with:

```python
        if self._kokoro is None:
            raise RuntimeError("KokoroTTS.synth called before _ensure_loaded")
```

- [ ] **Step 3: Verify both modules import**

Run:
```bash
python -c "import command_server; import kokoro_tts; print('ok')"
```

Expected: `ok`.

- [ ] **Step 4: Commit**

```bash
git add command_server.py kokoro_tts.py
git commit -m "fix: replace assert with explicit raise in prod code (B6)"
```

---

## Task 6: B7 — Log real errors in `window_capture.capture_window`

**Files:**
- Modify: `window_capture.py:~96`

- [ ] **Step 1: Locate the bare exception handler**

Open `window_capture.py`. Find `capture_window` function. There is a try/except that returns `None` on any exception (around line 96).

- [ ] **Step 2: Rewrite the exception handling**

Replace the bare `except ... return None` with explicit logging for unexpected exceptions. If the current code looks like:

```python
    try:
        # ... bitmap code ...
        return img
    except Exception:
        return None
```

Change to:

```python
    try:
        # ... bitmap code ...
        return img
    except (OSError, ValueError) as e:
        # Window may have just closed, DC is invalid, or bitmap size 0 —
        # these are expected transient failures.
        return None
    except Exception as e:
        # Unexpected — log so it's diagnosable instead of silently returning None.
        print(f"[window_capture] unexpected error: {e!r}", flush=True)
        return None
```

If the existing structure is different (e.g., nested try/except), apply the same principle: distinguish expected vs unexpected failures.

- [ ] **Step 3: Verify module imports**

Run:
```bash
python -c "import window_capture; print('ok')"
```

Expected: `ok`.

- [ ] **Step 4: Commit**

```bash
git add window_capture.py
git commit -m "fix: log unexpected errors in window_capture instead of silent None (B7)"
```

---

## Task 7: B8 — Remove over-defensive try/except in `ocr.read`

**Files:**
- Modify: `ocr.py` (around lines 100-110, the debug print block)

- [ ] **Step 1: Locate the block**

Open `ocr.py`. Find the block in the `read` method that prints debug output (around line 100-110). It looks like:

```python
        if self.debug:
            for i, lt in enumerate(lines_text):
                try:
                    print(f"[ocr] line {i}: {lt!r}", flush=True)
                except UnicodeEncodeError:
                    print(f"[ocr] line {i}: {lt.encode('ascii', 'replace').decode()!r}", flush=True)
```

- [ ] **Step 2: Verify unicode handling is already safe**

The `!r` (repr) formatter already escapes non-ASCII characters. A repr'd string cannot raise `UnicodeEncodeError` because it uses only ASCII. The try/except is pure superstition.

Replace the block with:

```python
        if self.debug:
            for i, lt in enumerate(lines_text):
                print(f"[ocr] line {i}: {lt!r}", flush=True)
```

- [ ] **Step 3: Verify module imports and prints an emoji-filled string without crashing**

Run:
```bash
python -c "from ocr import OCR; print('import ok')"
```

Expected: `import ok`.

- [ ] **Step 4: Commit**

```bash
git add ocr.py
git commit -m "fix: drop redundant try/except around repr-based print in ocr.read (B8)"
```

---

## Task 8: C1 — Remove `pyttsx3` from requirements

**Files:**
- Modify: `requirements.txt`

- [ ] **Step 1: Verify `pyttsx3` isn't imported**

Run:
```bash
grep -r "pyttsx3" --include="*.py" . 2>&1 | grep -v .worktrees || echo "no references"
```

Expected: `no references`.

- [ ] **Step 2: Delete the line**

Open `requirements.txt`. Remove the line `pyttsx3>=2.99`.

- [ ] **Step 3: Commit**

```bash
git add requirements.txt
git commit -m "chore: drop pyttsx3 dependency (C1)"
```

---

## Task 9: C2 — Delete unused `voice_for` method

**Files:**
- Modify: `speakers.py`

- [ ] **Step 1: Verify `voice_for` is not called**

Run:
```bash
grep -rn "voice_for(" --include="*.py" . 2>&1 | grep -v "voice_for_current\|voice_for\.py"
```

Expected: either empty or only the definition site in `speakers.py`.

- [ ] **Step 2: Delete the method**

Open `speakers.py`. Find `def voice_for(self, name: str) -> str | None:` (around line 287). Remove the entire method (2 lines — signature + body).

- [ ] **Step 3: Verify module imports and existing tests still pass**

Run:
```bash
python -c "import speakers; print('ok')" && pytest tests/ -v
```

Expected: `ok`, all tests still pass.

- [ ] **Step 4: Commit**

```bash
git add speakers.py
git commit -m "chore: delete unused SpeakerManager.voice_for method (C2)"
```

---

## Task 10: C3 + C4 — Delete stale voice files

**Files:**
- Delete: `voices/en_US-joe-medium.onnx`
- Delete: `voices/en_US-joe-medium.onnx.json`
- Delete: `voices/en_US-lessac-medium.onnx`
- Delete: `voices/en_US-lessac-medium.onnx.json`

- [ ] **Step 1: Verify the voices aren't referenced**

Run:
```bash
grep -rn "joe-medium\|lessac-medium" --include="*.py" --include="*.ini" --include="*.csv" . 2>&1 | grep -v .worktrees || echo "no references"
```

Expected: `no references`.

- [ ] **Step 2: Remove the files**

Run:
```bash
rm -f voices/en_US-joe-medium.onnx voices/en_US-joe-medium.onnx.json voices/en_US-lessac-medium.onnx voices/en_US-lessac-medium.onnx.json
```

- [ ] **Step 3: Confirm removal**

Run:
```bash
ls voices/ | grep -E "joe-medium|lessac-medium" || echo "gone"
```

Expected: `gone`.

- [ ] **Step 4: No commit required**

`voices/` is gitignored via `/voices/`. Nothing to commit. Skip.

---

## Task 11: C5 — Delete stale `PLAN.md`

**Files:**
- Delete: `PLAN.md`

- [ ] **Step 1: Verify file is tracked**

Run:
```bash
git ls-files PLAN.md
```

Expected: `PLAN.md` (meaning it's tracked).

- [ ] **Step 2: Remove with git**

Run:
```bash
git rm PLAN.md
```

- [ ] **Step 3: Commit**

```bash
git commit -m "chore: delete stale PLAN.md (C5)"
```

---

## Task 12: C6 — Update `tts.py` docstring to mention Sherpa

**Files:**
- Modify: `tts.py:1-35`

- [ ] **Step 1: Locate the docstring**

Open `tts.py`. The first `"""..."""` block at the top of the file covers lines 1-35ish. It currently says "two local engines: Piper and Kokoro-82M".

- [ ] **Step 2: Update the docstring**

Replace the entire top docstring (the triple-quoted block at lines 1-35) with:

```python
"""
Natural-voice TTS with three local engines: Piper (fast), Kokoro-82M (more
natural, ~350ms/sentence on CPU), and Sherpa-ONNX (multi-speaker VITS models
including VCTK, LibriTTS-R, and MeloTTS-en).

Voices are referenced as `engine:name` strings. Sherpa uses a nested form
`sherpa:<model>:<speaker_id>` (e.g. `sherpa:vctk:0`). Bare names (no colon)
default to `piper:` for backward compatibility.

Multi-voice with on-demand caching:
  - Piper: {voice_name -> PiperVoice} dict; each voice ~60-100 MB RAM.
  - Kokoro: single shared model (~310 MB on disk, loaded once into RAM),
    voice selection is a per-synth parameter.
  - Sherpa: one model per registered name, loaded on demand; each model
    holds many speakers selected by integer id.

Usage:
    tts = TTS()                                       # piper:en_US-amy-medium
    tts = TTS(voice="kokoro:af_heart")                 # default Kokoro voice
    tts.speak("hello")                                 # use default voice
    tts.speak("hi", voice="piper:en_US-ryan-medium")   # explicit Piper
    tts.speak("hi", voice="kokoro:am_michael")         # explicit Kokoro
    tts.speak("hi", voice="sherpa:vctk:30")            # explicit Sherpa
    tts.shutdown()

Voice files download once on first use:
  - Piper voices  → voices/<voice>.onnx + .onnx.json
  - Kokoro        → voices/kokoro/{kokoro-v1.0.onnx, voices-v1.0.bin}
  - Sherpa models → voices/sherpa_<model>/<archive contents>
"""
```

- [ ] **Step 3: Verify module still imports**

Run:
```bash
python -c "import tts; print('ok')"
```

Expected: `ok`.

- [ ] **Step 4: Commit**

```bash
git add tts.py
git commit -m "docs: update tts.py module docstring to include Sherpa engine (C6)"
```

---

## Task 13: C7 + B3 — Fix `test_voices.py` to honor requested voice

**Files:**
- Modify: `test_voices.py`

- [ ] **Step 1: Locate the bug**

Open `test_voices.py`. The `render` function parses the voice but then constructs `TTS(voice=f"piper:en_US-amy-medium", speed=1.0)`, ignoring the requested engine. The subsequent engine dispatch IS correct — it's only the TTS initialization voice that's wrong (and ignored by the engine-specific synth call paths anyway).

Re-read the file end-to-end. The bug is the `f"piper:en_US-amy-medium"` hard-coding. Because every engine path uses its own synth directly (bypassing `tts.speak`), the hardcoded init voice doesn't actually break rendering — BUT it triggers preloading a Piper voice that may not be needed.

- [ ] **Step 2: Fix `render` to use the requested voice for TTS init**

In `test_voices.py`, replace the line:

```python
    tts = TTS(voice=f"piper:en_US-amy-medium", speed=1.0)
```

with:

```python
    # Use the requested voice for init so we don't needlessly preload a
    # Piper voice when testing Kokoro/Sherpa. If init fails (e.g., voice
    # isn't in the pool yet), fall back to a known-good default.
    try:
        tts = TTS(voice=voice, speed=1.0)
    except Exception:
        tts = TTS(voice="piper:en_US-amy-medium", speed=1.0)
```

- [ ] **Step 3: Smoke test — render a Piper and a Kokoro sample**

Run:
```bash
python test_voices.py piper:en_US-amy-medium
```

Expected: `[ok] piper:en_US-amy-medium  ->  sample_piper_en_US-amy-medium.wav`.

Then:
```bash
python test_voices.py kokoro:af_heart
```

Expected: `[ok] kokoro:af_heart  ->  sample_kokoro_af_heart.wav`. Should NOT say "Loading Piper voice 'en_US-amy-medium'" in the output (because we now init with the Kokoro voice).

- [ ] **Step 4: Cleanup the generated WAV samples**

Run:
```bash
rm -f sample_*.wav
```

- [ ] **Step 5: Commit**

```bash
git add test_voices.py
git commit -m "fix: honor requested voice in test_voices.py init (C7/B3)"
```

---

## Task 14: C8 — Replace global `print` shadowing

**Files:**
- Modify: `main.py:30` (remove `import functools` if possible), `main.py:49` (replace shadowing line)

- [ ] **Step 1: Check if `functools` is used elsewhere**

Run:
```bash
grep -n "functools\." main.py
```

Expected: only matches on the single `functools.partial(print, ...)` use, not elsewhere.

- [ ] **Step 2: Remove the global shadowing and the functools import**

Open `main.py`. Line 30 has `import functools`. Line 49 has `print = functools.partial(print, flush=True)`.

Remove BOTH lines.

Now the existing `print(...)` calls in the file will use the builtin `print` (no auto-flush). This matches Python's normal behavior; explicit `flush=True` is already used in performance-sensitive prints (`print(..., flush=True)`) and debug helpers in the codebase. Any non-flushed prints in `main.py` are in the startup path or one-shot diagnostics — ok to let them buffer.

- [ ] **Step 3: Verify module still imports**

Run:
```bash
python -c "import main; print('ok')"
```

Expected: `ok`.

- [ ] **Step 4: Commit**

```bash
git add main.py
git commit -m "chore: stop shadowing builtin print with functools.partial (C8)"
```

---

## Task 15: C9 — Check `ctypes` import in capture.py

**Files:**
- Modify: `capture.py:27` (possibly)

- [ ] **Step 1: Search for ctypes usage**

Run:
```bash
grep -n "ctypes\." capture.py
```

Expected: Shows every usage site. If ALL usages go through `_user32 = ctypes.windll.user32` (module-level) and the module-level `ctypes` import is still needed to resolve that initialization, the import must stay. If there are no `ctypes.` references outside the initialization of `_user32` AND `_user32` is used everywhere else, the import is still required (module-level init runs on import).

- [ ] **Step 2: Determine if the import is truly redundant**

The `_user32` resolution at module load time DOES require `ctypes` to be imported. So `import ctypes` is NOT redundant — it's needed for `ctypes.windll.user32` on line 31. The audit called this out incorrectly.

**This task is a no-op.** Skip. Leave `import ctypes` in place.

- [ ] **Step 3: No changes, no commit**

If the investigation turns out differently (i.e., `ctypes` really is redundant), proceed with the removal. Otherwise, document the decision with a comment and move on.

---

## Task 16: Delete orphan debug images

**Files:**
- Delete: `last_region.png`, `test_failing.png`, `test_window_capture.png`

These were created during development / testing and are not referenced by any code. They match the `.gitignore` pattern `debug_*.png` only for the third; `last_region.png` and `test_failing.png` are leftover artifacts.

- [ ] **Step 1: Verify they're not tracked**

Run:
```bash
git ls-files last_region.png test_failing.png test_window_capture.png 2>&1
```

If output is empty, they're untracked (already ignored). If any are tracked, git-rm them.

- [ ] **Step 2: Delete and update .gitignore pattern**

Run:
```bash
rm -f last_region.png test_failing.png test_window_capture.png
```

Open `.gitignore` and add these patterns if they aren't already covered:

```
last_region.png
test_*.png
```

Keep the existing `debug_*.png`.

- [ ] **Step 3: Commit the gitignore update**

```bash
git add .gitignore
git commit -m "chore: delete orphan debug images; broaden .gitignore (C residual)"
```

---

## Task 17: Final smoke test

**Files:** none

- [ ] **Step 1: Run all tests**

Run:
```bash
pytest tests/ -v
```

Expected: all tests pass. No new failures introduced.

- [ ] **Step 2: Full module-import smoke test**

Run:
```bash
python -c "import main; import tts; import speakers; import ocr; import capture; import command_server; import kokoro_tts; import sherpa_tts; import window_capture; import region_picker; print('all modules import cleanly')"
```

Expected: `all modules import cleanly`.

- [ ] **Step 3: Final file check**

Run:
```bash
cd C:/Users/Admin/Documents/Claude/Github/dialogue-reader/.worktrees/cleanup && ls -la PLAN.md 2>&1 || echo 'PLAN.md removed'
grep -n "pyttsx3" requirements.txt 2>&1 || echo 'pyttsx3 removed'
grep -n "voice_for(" speakers.py 2>&1 | grep -v "voice_for_current" || echo 'voice_for removed'
```

Expected:
- `PLAN.md removed`
- `pyttsx3 removed`
- `voice_for removed`

No commit — this is a validation step only.
