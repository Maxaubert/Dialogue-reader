# Startup Audio Cues + Lockfile Singleton Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give audible startup feedback ("OCR starting up" -> chime + "OCR ready") and replace the powershell orphan scan with a fast lockfile singleton.

**Architecture:** Two changes to `main.py`. (1) Replace the powershell-based `_kill_orphans()` with `_claim_singleton()`, which reads a PID from a lockfile, terminates that prior instance if alive (reusing the existing ctypes `_terminate_pid`/`_is_process_alive`), and writes its own PID. (2) Reorder `main()` to load Kokoro before the slow OCR engines so it can announce "OCR starting up", then signal readiness with a rising chime + "OCR ready" right before the main loop (when hotkeys go live).

**Tech Stack:** Python 3.14, ctypes (Win32), sounddevice, kokoro-onnx (TTS), pytest.

## Global Constraints

- Kokoro is the only TTS engine. Spoken cues go through `tts.speak(...)`.
- Preserve "newest launch wins": a fresh launch terminates the prior running instance.
- The ready chime must play even if Kokoro is unavailable (it goes through `sounddevice` via `_play_cue`, not Kokoro).
- Reuse existing helpers: `_terminate_pid`, `_is_process_alive`, `_make_beep`, `_play_cue`, `tts.speak`.
- Startup wording lives in module constants: `_STARTUP_PHRASE = "OCR starting up"`, `_READY_PHRASE = "OCR ready"`.
- No em-dashes anywhere (code, comments, commit messages).
- Windows-only. Run pytest from the repo root: `python -m pytest -q`. Python interpreter is `python` (3.14).
- This work builds on the Kokoro-only `tts.py` already on the `scale-down-kokoro-only` branch.

---

## File Structure

**Modified:**
- `main.py` — add `_LOCK_PATH`, `_claim_singleton`, `_READY_CUE`, `_STARTUP_PHRASE`, `_READY_PHRASE`; remove `_find_orphan_pids`, `_kill_orphans`, `import subprocess`, `_NO_WINDOW_FLAGS`; rewrite the singleton comment; reorder `main()`; adjust the UDP bind-failure retry.
- `.gitignore` — add `dialogue_reader.lock`.

**New tests:**
- `tests/test_singleton.py` — unit tests for `_claim_singleton`.
- `tests/test_startup_cues.py` — guards the cue constants and wording.

---

### Task 1: Lockfile singleton replaces the powershell orphan scan

**Files:**
- Modify: `main.py` (add `_LOCK_PATH` + `_claim_singleton`; remove `_find_orphan_pids`, `_kill_orphans`, `import subprocess`, `_NO_WINDOW_FLAGS`; rewrite singleton comment block; update the `_kill_orphans()` call sites in `main()`)
- Modify: `.gitignore`
- Test: `tests/test_singleton.py`

**Interfaces:**
- Consumes: existing `_terminate_pid(pid: int) -> None` and `_is_process_alive(pid: int) -> bool` (unchanged).
- Produces: `_LOCK_PATH: Path` (module constant) and `_claim_singleton(lock_path: Path = _LOCK_PATH) -> None`. `_claim_singleton` looks up `_is_process_alive` and `_terminate_pid` as module globals so tests can monkeypatch them.

- [ ] **Step 1: Write the failing tests**

Create `tests/test_singleton.py`:

```python
"""Unit tests for the lockfile singleton (_claim_singleton)."""
import os

import main


def test_claim_writes_own_pid_when_no_lockfile(tmp_path, monkeypatch):
    called = []
    monkeypatch.setattr(main, "_terminate_pid", lambda pid: called.append(pid))
    monkeypatch.setattr(main, "_is_process_alive", lambda pid: False)
    lock = tmp_path / "dialogue_reader.lock"
    main._claim_singleton(lock)
    assert lock.read_text(encoding="utf-8").strip() == str(os.getpid())
    assert called == []


def test_claim_terminates_live_prior_pid(tmp_path, monkeypatch):
    called = []
    monkeypatch.setattr(main, "_terminate_pid", lambda pid: called.append(pid))
    monkeypatch.setattr(main, "_is_process_alive", lambda pid: True)
    monkeypatch.setattr(main.time, "sleep", lambda s: None)
    lock = tmp_path / "dialogue_reader.lock"
    lock.write_text("424242", encoding="utf-8")
    main._claim_singleton(lock)
    assert called == [424242]
    assert lock.read_text(encoding="utf-8").strip() == str(os.getpid())


def test_claim_skips_dead_prior_pid(tmp_path, monkeypatch):
    called = []
    monkeypatch.setattr(main, "_terminate_pid", lambda pid: called.append(pid))
    monkeypatch.setattr(main, "_is_process_alive", lambda pid: False)
    lock = tmp_path / "dialogue_reader.lock"
    lock.write_text("424242", encoding="utf-8")
    main._claim_singleton(lock)
    assert called == []
    assert lock.read_text(encoding="utf-8").strip() == str(os.getpid())


def test_claim_ignores_garbage_lockfile(tmp_path, monkeypatch):
    called = []
    monkeypatch.setattr(main, "_terminate_pid", lambda pid: called.append(pid))
    monkeypatch.setattr(main, "_is_process_alive", lambda pid: True)
    lock = tmp_path / "dialogue_reader.lock"
    lock.write_text("not-a-pid", encoding="utf-8")
    main._claim_singleton(lock)
    assert called == []
    assert lock.read_text(encoding="utf-8").strip() == str(os.getpid())


def test_claim_does_not_self_terminate(tmp_path, monkeypatch):
    called = []
    monkeypatch.setattr(main, "_terminate_pid", lambda pid: called.append(pid))
    monkeypatch.setattr(main, "_is_process_alive", lambda pid: True)
    lock = tmp_path / "dialogue_reader.lock"
    lock.write_text(str(os.getpid()), encoding="utf-8")
    main._claim_singleton(lock)
    assert called == []
    assert lock.read_text(encoding="utf-8").strip() == str(os.getpid())
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `python -m pytest tests/test_singleton.py -q`
Expected: FAIL with `AttributeError: module 'main' has no attribute '_claim_singleton'`.

- [ ] **Step 3: Add `_LOCK_PATH` and `_claim_singleton`, remove the powershell functions**

In `main.py`, replace the entire singleton comment block and the `_NO_WINDOW_FLAGS`/`_find_orphan_pids`/`_terminate_pid`/`_kill_orphans` region. Find the comment that starts `# ---- singleton enforcement` and the functions through `_kill_orphans`. Replace everything from that comment header down to (but NOT including) the `# ---- parent watchdog` header with:

```python
# ---- singleton enforcement ------------------------------------------------
#
# py.exe (the Python launcher) spawns python.exe as a child process. Killing
# py.exe via AHK's ProcessClose() does NOT cascade to python.exe, so it can
# survive as an orphan holding the UDP port and continuing to OCR/speak.
#
# Each instance records its PID in a lock file next to this script. On startup
# we read that file and, if it names a different live process, terminate it
# (newest launch wins) before binding our own UDP socket and opening audio.
# The parent watchdog below is the other safety net: it kills this process
# when the launching AHK dies.

_LOCK_PATH = Path(__file__).parent / "dialogue_reader.lock"


def _terminate_pid(pid: int) -> None:
    PROCESS_TERMINATE = 0x0001
    handle = ctypes.windll.kernel32.OpenProcess(PROCESS_TERMINATE, False, pid)
    if not handle:
        return
    try:
        ctypes.windll.kernel32.TerminateProcess(handle, 0)
    finally:
        ctypes.windll.kernel32.CloseHandle(handle)


def _claim_singleton(lock_path: Path = _LOCK_PATH) -> None:
    """Ensure this is the only running instance. If lock_path records a prior
    instance's PID that is still alive (and not our own), terminate it, wait
    briefly for Windows to release its UDP socket and audio streams, then
    record our own PID. A missing, empty, or unparseable lock file just means
    'no known prior instance' and we proceed to write ours."""
    my_pid = os.getpid()
    try:
        prior_pid = int(lock_path.read_text(encoding="utf-8").strip())
    except (OSError, ValueError):
        prior_pid = None
    if prior_pid is not None and prior_pid != my_pid and _is_process_alive(prior_pid):
        print(f"[singleton] killing prior instance pid {prior_pid}")
        try:
            _terminate_pid(prior_pid)
        except Exception as e:
            print(f"[singleton] failed to kill {prior_pid}: {e}")
        # Let Windows release the UDP socket and sounddevice streams.
        time.sleep(0.7)
    try:
        lock_path.write_text(str(my_pid), encoding="utf-8")
    except OSError as e:
        print(f"[singleton] could not write lock file: {e}")
```

Note: `_terminate_pid` is moved up into this block (it was previously defined here too). `_claim_singleton` references `_is_process_alive`, which is defined just below in the parent-watchdog section; since `_claim_singleton` only calls it at runtime (not at import), the forward reference is fine.

- [ ] **Step 4: Remove the now-unused `import subprocess`**

In `main.py`, delete the line `import subprocess` (top-of-file imports). `_NO_WINDOW_FLAGS` was removed with the block in Step 3; confirm no other line references `subprocess` or `_NO_WINDOW_FLAGS`.

Run: `grep -n "subprocess\|_NO_WINDOW_FLAGS\|_find_orphan_pids\|_kill_orphans" main.py`
Expected: no matches.

- [ ] **Step 5: Wire `_claim_singleton` into `main()` (startup + bind-retry)**

In `main()`, replace the startup orphan-kill. Find:

```python
    # Kill any orphaned instances of this script BEFORE we touch heavy stuff
    # like the OCR/TTS models or the UDP socket. Without this, leftover
    # python.exe processes from previous runs (e.g. ones AHK couldn't
    # cascade-kill via py.exe) keep watching the screen and speaking, and
    # the new instance silently crashes when it can't bind UDP.
    _kill_orphans()
```

Replace with:

```python
    # Claim the singleton BEFORE touching heavy stuff (OCR/TTS models, UDP
    # socket, audio device). This terminates any prior instance recorded in
    # the lock file so it is not still holding the port or the sound output.
    _claim_singleton()
```

Then find the UDP bind-failure retry:

```python
    except OSError as e:
        # Port still held — try one more aggressive sweep, then bail.
        print(f"[singleton] UDP bind failed ({e}); retrying after another orphan sweep")
        _kill_orphans()
        time.sleep(0.5)
        try:
```

Replace with:

```python
    except OSError as e:
        # Port still held by a just-terminated prior instance; give Windows a
        # moment to release it, then retry once before giving up.
        print(f"[singleton] UDP bind failed ({e}); retrying after a short wait")
        time.sleep(0.5)
        try:
```

- [ ] **Step 6: Gitignore the lock file**

Append `dialogue_reader.lock` to `.gitignore` (if not already present).

- [ ] **Step 7: Run the tests to verify they pass**

Run: `python -m pytest tests/test_singleton.py -q`
Expected: 5 passed.

- [ ] **Step 8: Verify import and full suite**

Run: `python -c "import main; print('main OK')"`
Expected: `main OK`.

Run: `python -m pytest -q`
Expected: all tests pass (15 prior + 5 new = 20).

- [ ] **Step 9: Commit**

```bash
git add main.py .gitignore tests/test_singleton.py
git commit -m "perf: replace powershell orphan scan with lockfile singleton

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

### Task 2: Startup reorder + audio cues

**Files:**
- Modify: `main.py` (add `_READY_CUE`, `_STARTUP_PHRASE`, `_READY_PHRASE`; reorder `main()` so Kokoro loads before OCR; announce on startup; play ready chime + speech before the loop)
- Test: `tests/test_startup_cues.py`

**Interfaces:**
- Consumes: `_make_beep(frequencies: list[float], tone_ms: int = 80) -> np.ndarray`, `_play_cue(audio) -> None`, `tts.speak(text, voice=None)` (all existing).
- Produces: module constants `_READY_CUE: np.ndarray`, `_STARTUP_PHRASE = "OCR starting up"`, `_READY_PHRASE = "OCR ready"`.

- [ ] **Step 1: Write the failing test**

Create `tests/test_startup_cues.py`:

```python
"""Guards the startup cue constants and their wording."""
import numpy as np

import main


def test_startup_phrase_wording():
    assert main._STARTUP_PHRASE == "OCR starting up"


def test_ready_phrase_wording():
    assert main._READY_PHRASE == "OCR ready"


def test_ready_cue_is_audio():
    assert isinstance(main._READY_CUE, np.ndarray)
    assert main._READY_CUE.size > 0


def test_ready_cue_distinct_from_unpause_cue():
    # The ready chime must not be the same samples as the unpause cue, so the
    # two signals are audibly different.
    if main._READY_CUE.shape == main._UNPAUSE_CUE.shape:
        assert not np.array_equal(main._READY_CUE, main._UNPAUSE_CUE)
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `python -m pytest tests/test_startup_cues.py -q`
Expected: FAIL with `AttributeError: module 'main' has no attribute '_STARTUP_PHRASE'` (or `_READY_CUE`).

- [ ] **Step 3: Add the cue + phrase constants**

In `main.py`, find:

```python
# Pause descends (stop) — unpause ascends (go).
_PAUSE_CUE = _make_beep([700.0, 350.0])
_UNPAUSE_CUE = _make_beep([350.0, 700.0])
```

Add immediately after it (also fix the em-dash in the existing comment while editing this block, changing "Pause descends (stop) — unpause ascends (go)." to use a comma):

```python
# Pause descends (stop), unpause ascends (go).
_PAUSE_CUE = _make_beep([700.0, 350.0])
_UNPAUSE_CUE = _make_beep([350.0, 700.0])

# Distinct rising chime for "app is ready" (C5 -> G5), separate from unpause.
_READY_CUE = _make_beep([523.0, 784.0])
_STARTUP_PHRASE = "OCR starting up"
_READY_PHRASE = "OCR ready"
```

- [ ] **Step 4: Reorder `main()` to load Kokoro first and announce**

In `main()`, find this block (the config reads, OCR construction, then TTS construction):

```python
    dialogue_engine, speaker_engine = _load_ocr_config()
    print(
        f"[dialogue-reader] Loading OCR engines "
        f"(dialogue={dialogue_engine}, speaker={speaker_engine})..."
    )
    ocr = OCR(
        debug=debug,
        dialogue_engine=dialogue_engine,
        speaker_engine=speaker_engine,
    )
    print("[dialogue-reader] Loading TTS engine...")
    tts = TTS(voice=default_voice, speed=1.1)
```

Replace it with (TTS first, announce, then the slow OCR load):

```python
    dialogue_engine, speaker_engine = _load_ocr_config()

    # Load Kokoro FIRST so it can announce startup while the slower OCR
    # engines (EasyOCR/torch) load. tts.speak() is non-blocking, so the
    # announcement plays during the OCR load below.
    print("[dialogue-reader] Loading TTS engine...")
    tts = TTS(voice=default_voice, speed=1.1)
    tts.speak(_STARTUP_PHRASE)

    print(
        f"[dialogue-reader] Loading OCR engines "
        f"(dialogue={dialogue_engine}, speaker={speaker_engine})..."
    )
    ocr = OCR(
        debug=debug,
        dialogue_engine=dialogue_engine,
        speaker_engine=speaker_engine,
    )
```

- [ ] **Step 5: Play the ready cue when hotkeys go live**

In `main()`, find:

```python
    ocr_worker = OCRWorker(ocr)
    print("[dialogue-reader] OCR worker thread started.")

    print("[dialogue-reader] Ready. Use the AHK script (or send UDP commands) to control.")
```

Insert the ready cue between the worker-started print and the "Ready." print:

```python
    ocr_worker = OCRWorker(ocr)
    print("[dialogue-reader] OCR worker thread started.")

    # Everything is loaded and the command server is bound, so hotkeys are
    # now live. Signal it: rising chime (plays even if Kokoro is down) then a
    # spoken confirmation.
    _play_cue(_READY_CUE)
    tts.speak(_READY_PHRASE)

    print("[dialogue-reader] Ready. Use the AHK script (or send UDP commands) to control.")
```

- [ ] **Step 6: Run the cue test and full suite**

Run: `python -m pytest tests/test_startup_cues.py -q`
Expected: 4 passed.

Run: `python -m pytest -q`
Expected: all pass (20 prior + 4 new = 24).

- [ ] **Step 7: Import smoke**

Run: `python -c "import main; print('main OK')"`
Expected: `main OK`.

- [ ] **Step 8: Manual launch verification**

This step is manual (audio is hardware). Launch the app via the normal AHK flow (or `python main.py --debug --parent-pid <pid>`). Confirm:
1. "OCR starting up" is heard within ~2 seconds of launch.
2. After the OCR load, a rising chime followed by "OCR ready" plays.
3. A hotkey pressed immediately after "OCR ready" works (e.g. PICK_REGION opens the picker).
4. Relaunch while an instance is running: the old instance stops and the new one binds the port without error.

Record the outcome in the implementer report. If a subagent cannot run the GUI app, note that Step 8 must be performed by the human and leave it unchecked.

- [ ] **Step 9: Commit**

```bash
git add main.py tests/test_startup_cues.py
git commit -m "feat: announce startup and signal readiness with audio cues

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Self-Review

**Spec coverage:**
- Reorder so Kokoro announces before OCR load -> Task 2 Step 4. ✓
- Ready chime + speech when hotkeys live -> Task 2 Step 5. ✓
- Graceful (chime without Kokoro) -> `_play_cue(_READY_CUE)` is independent of `tts.speak`; covered by Global Constraints and Task 2 Step 5 comment. ✓
- Distinct ready cue (C5->G5) -> Task 2 Step 3 + `test_ready_cue_distinct_from_unpause_cue`. ✓
- Centralized wording -> `_STARTUP_PHRASE`/`_READY_PHRASE` + tests. ✓
- Lockfile singleton, newest-wins, reuse `_terminate_pid`/`_is_process_alive` -> Task 1 Step 3. ✓
- Remove `_find_orphan_pids`, `_kill_orphans`, `subprocess`, `_NO_WINDOW_FLAGS` -> Task 1 Steps 3-4. ✓
- UDP bind-retry without powershell -> Task 1 Step 5. ✓
- No exit-time lockfile delete (shutdown path unchanged) -> not modified; correct by omission. ✓
- `.gitignore` the lock -> Task 1 Step 6. ✓
- Singleton unit tests (5 cases) -> Task 1 Step 1. ✓
- Manual audio verification -> Task 2 Step 8. ✓

**Placeholder scan:** No TBD/TODO; all code shown in full; deletions identified by exact surrounding text.

**Type/symbol consistency:** `_claim_singleton(lock_path=_LOCK_PATH)`, `_LOCK_PATH`, `_terminate_pid`, `_is_process_alive`, `_READY_CUE`, `_STARTUP_PHRASE`, `_READY_PHRASE` are named identically in their defining task, the tests, and the `main()` call sites. Test counts assume the branch currently has 15 passing tests (verified before this plan).
