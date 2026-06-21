# Startup Audio Cues + Lockfile Singleton Design

**Date:** 2026-06-21
**Status:** Approved (pending spec review)

## Problem

On launch the app takes up to ~5 seconds before hotkeys register, with no feedback, so the user does not know when it is ready and presses hotkeys that do nothing. Two contributing factors:

1. The bulk of the wait is loading the OCR engines (EasyOCR pulls in torch), which is unavoidable, but the wait is silent and opaque.
2. A measurable slice of startup is `_kill_orphans()` shelling out to `powershell.exe Get-CimInstance Win32_Process` (10 s timeout) to enumerate Python processes.

## Goals

1. Give audible startup feedback: announce when loading begins and signal (chime + speech) the moment the app is actually ready (hotkeys live).
2. Remove the powershell process scan from the startup path while preserving the existing "newest launch wins" orphan-handling behavior.

## Non-Goals

- Making torch/EasyOCR load faster (out of scope; the heavy model load stays).
- Making hotkeys functional before models finish loading (commands still require the loaded engines).
- Removing the parent-watchdog (it stays as the other safety net).

## Part A: Startup reorder + audio cues

### Current `main()` order (relevant excerpt)

1. `_kill_orphans()` (powershell scan — replaced in Part B)
2. `_start_parent_watchdog()`
3. read config loaders (voice, polling, capture, ocr, magnifier)
4. `OCR(...)` — loads OCR engines (slow: EasyOCR/torch)
5. `TTS(voice=default_voice, speed=1.1)` — loads Kokoro model in constructor
6. `SpeakerManager(...)`
7. `CommandServer(...).start()` — binds UDP (commands become receivable)
8. `OCRWorker(ocr)`
9. enter main loop (commands are processed here)

### New order

1. Claim singleton (lockfile — Part B). Done first so a stale instance holding the audio device is gone before we play any sound.
2. `_start_parent_watchdog()`
3. read config loaders
4. `TTS(voice=default_voice, speed=1.1)` — load Kokoro FIRST
5. `tts.speak("OCR starting up")` — non-blocking (spawns a daemon thread); plays while step 6 runs
6. `OCR(...)` — load the slow OCR engines
7. `SpeakerManager(...)`
8. `CommandServer(...).start()`
9. `OCRWorker(ocr)`
10. Play the ready cue: `_play_cue(_READY_CUE)` (rising chime, blocking ~160 ms) then `tts.speak("OCR ready")`
11. enter main loop

### Audio cue details

- The "OCR starting up" announcement uses the existing `tts.speak()` (non-blocking, daemon thread). It overlaps the OCR load.
- The ready cue is a rising two-tone chime. Reuse the existing `_make_beep` helper. The existing `_UNPAUSE_CUE = _make_beep([350.0, 700.0])` is already a rising cue; introduce a dedicated `_READY_CUE = _make_beep([523.0, 784.0])` (C5 -> G5) so the ready signal is distinct from the pause/unpause cues. Played via the existing `_play_cue()` (blocking, brief).
- Order at step 10: chime first (blocking), then `tts.speak("OCR ready")`. `tts.speak()` calls `sd.stop()` at entry, so any residual "starting up" audio (long finished by now) would be preempted cleanly.
- Graceful degradation: if Kokoro is unavailable, `tts.speak()` is a no-op, but `_play_cue()` goes straight through `sounddevice`, so the ready chime still plays. The user always gets the ready signal.
- Wording is centralized as two module constants so it is trivial to change:
  - `_STARTUP_PHRASE = "OCR starting up"`
  - `_READY_PHRASE = "OCR ready"`

## Part B: Lockfile singleton

### Behavior (preserves "newest launch wins")

- A lockfile `dialogue_reader.lock` lives next to the script and contains the PID of the currently running instance.
- New function `_claim_singleton(lock_path: Path) -> None`:
  1. If the lockfile exists, read it and parse an int PID.
  2. If that PID is a live process and is not our own PID, terminate it via the existing `_terminate_pid(pid)` (ctypes `OpenProcess`/`TerminateProcess`), then `time.sleep(0.7)` to let Windows release the UDP socket and audio streams.
  3. Write our own PID (`str(os.getpid())`) to the lockfile (create or overwrite).
  4. All file/parse errors are swallowed (a missing, empty, or garbage lockfile just means "no known prior instance" — proceed to write ours).

### Removals

- Delete `_find_orphan_pids()` and `_kill_orphans()` (the powershell-based functions).
- Delete the `import subprocess` and `_NO_WINDOW_FLAGS` module globals (only used by the deleted functions). Confirm no other usage before removing.
- Keep `_terminate_pid()` and `_is_process_alive()` (reused).

### UDP bind-failure path

The existing retry block (calls `_kill_orphans()` on bind failure) changes: replace the `_kill_orphans()` call with a short `time.sleep(0.5)` and a single rebind retry. The known prior instance was already terminated in `_claim_singleton`; a lingering bind failure is most likely a transient socket-release delay. If the retry still fails, keep the existing fatal message and `return 1`.

### Lifecycle / cleanup

- No explicit lockfile deletion on exit is required: the next launch validates the PID is alive before acting on it, so a stale lockfile (from a crash) is harmless. (Optional nicety: remove the lockfile in the `finally` block; not required for correctness.) This design does NOT delete on exit, to keep the shutdown path unchanged.
- `.gitignore`: add `dialogue_reader.lock`.

## Testing

### Unit tests (new `tests/test_singleton.py`)

Mock `_terminate_pid` and `_is_process_alive` so no real process is signalled; use a temp lockfile path.

1. `test_claim_writes_own_pid_when_no_lockfile`: no lockfile exists -> after `_claim_singleton`, the lockfile contains our PID; terminate is never called.
2. `test_claim_terminates_live_prior_pid`: lockfile contains a PID that `_is_process_alive` reports alive (and != our PID) -> `_terminate_pid` called once with that PID; lockfile then contains our PID.
3. `test_claim_skips_dead_prior_pid`: lockfile contains a PID that `_is_process_alive` reports dead -> `_terminate_pid` NOT called; lockfile overwritten with our PID.
4. `test_claim_ignores_garbage_lockfile`: lockfile contains non-integer text -> no terminate, no crash; lockfile overwritten with our PID.
5. `test_claim_does_not_self_terminate`: lockfile contains our own PID -> `_terminate_pid` NOT called.

To make `_claim_singleton` testable, it must accept the lockfile path and the alive/terminate helpers must be module-level functions it calls by name (so tests can monkeypatch `main._is_process_alive` and `main._terminate_pid`).

### Manual verification (in the plan, not automated)

Launch the app and confirm: "OCR starting up" is heard within ~2 s, then after the OCR load a rising chime + "OCR ready" plays, and a hotkey pressed immediately after "OCR ready" works. Relaunch while an instance is running and confirm the old instance dies and the new one binds.

## Files touched

- `main.py`: reorder `main()`; add `_claim_singleton`, `_READY_CUE`, `_STARTUP_PHRASE`, `_READY_PHRASE`; remove `_find_orphan_pids`, `_kill_orphans`, `import subprocess`, `_NO_WINDOW_FLAGS`; adjust UDP bind-failure retry.
- `tests/test_singleton.py`: new unit tests.
- `.gitignore`: add `dialogue_reader.lock`.
