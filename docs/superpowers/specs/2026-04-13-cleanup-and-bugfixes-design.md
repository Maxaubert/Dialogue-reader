# Cleanup and Bug-fixes — Design Spec

## Goal

Remove dead code, stale files, and documentation drift; fix 8 bugs ranging from a missing command handler to minor defensive-code smells. No new features; no behavior changes beyond the bug fixes themselves.

## Scope

17 items total, grouped by type.

### Cleanup (C1 – C9)

| Id | File | What | How |
|---|---|---|---|
| **C1** | `requirements.txt` | Remove `pyttsx3>=2.99` | Delete the line — package not imported anywhere in the code. |
| **C2** | `speakers.py` | Delete `voice_for()` method | Only `voice_for_current()` is called. Remove the unused method. |
| **C3** | `voices/en_US-joe-medium.onnx` + `.onnx.json` | Remove stale voice files | Delete from disk. Not referenced by any ini or csv. |
| **C4** | `voices/en_US-lessac-medium.onnx` + `.onnx.json` | Remove stale voice files | Delete from disk. Not referenced anywhere. |
| **C5** | `PLAN.md` | Delete outdated MVP plan | File claims SAPI/pyttsx3 flow that was superseded. No longer useful as planning doc. |
| **C6** | `tts.py` (module docstring, ~lines 1-35) | Update "two engines" → mention three | Rewrite the docstring to reflect Piper + Kokoro + Sherpa-ONNX. |
| **C7** | `test_voices.py` | Fix ignored `voice` parameter | Replace the hardcoded `piper:en_US-amy-medium` default with the requested voice; dispatch on the parsed engine so kokoro/sherpa/piper all work. |
| **C8** | `main.py:49` | Replace `print = functools.partial(...)` | Define a local `_p` helper that wraps print with flush=True; use it where needed; stop shadowing the builtin. Remove `import functools` if no other use. |
| **C9** | `capture.py:27` | Remove redundant `import ctypes` | Only `ctypes.windll` is used via the pre-resolved `_user32`. Verify no `ctypes.` references remain; if truly redundant, delete the import. |

### Bug-fixes (B1 – B8)

| Id | File | Bug | Fix |
|---|---|---|---|
| **B1** | `main.py` `handle_command` | `SET_SPEAKER:<name>` commands are silently ignored. | Add an `elif cmd.startswith("SET_SPEAKER:")` branch that parses the name (UTF-8, trim) and calls `speaker_mgr.set_current(name)` with a console log. |
| **B2** | `speakers.py:65-74` `_load` | `int()` calls on `null` or malformed values in `speakers.json` crash the whole load (silently swallowed), state is lost. | Wrap each field in try/except so one bad entry doesn't wipe the dict. Use `.get()` with defaults and guard each int conversion. |
| **B3** | `test_voices.py` | `voice` parameter passed to `render()` is ignored; every sample uses the default Piper voice. | Use `_parse_voice(voice)` correctly and dispatch through the TTS engines explicitly. Same fix as C7 — treating as one change. |
| **B4** | `main.py:788` | `TextConfirmPolls=100` would cap `max_attempts` at 400 × 100ms = 40s freeze. | Cap `max_attempts` at a hard ceiling (e.g. 30) regardless of multiplier. Keep `TEXT_CONFIRM_MAX_MULTIPLIER` as the soft target. |
| **B5** | `tts.py` `stop()` / `speak()` | Race: `sd.stop()` runs unconditionally; mid-synthesis worker then also bails via version check. Two teardown paths, unpredictable interrupt ordering. | Only `sd.stop()` is fine (calling twice is safe). The visible risk is a worker starting playback AFTER `sd.stop()` was called. Guard worker's `sd.play()` with a version check immediately before the call. |
| **B6** | `command_server.py:49`, `kokoro_tts.py:129` | `assert` in production (disabled by `-O`). | Replace with explicit `if x is None: raise RuntimeError(...)` so the check runs regardless of optimisations. |
| **B7** | `window_capture.py:96` | Bare `except` returns None silently — real errors are invisible. | Catch specifically-expected errors (OSError, win32-related) and log via `print("[window_capture] ...")`; let unexpected exceptions propagate. |
| **B8** | `ocr.py:105-108` | Redundant try/except around a normal print that can't fail. | Remove the try/except; the underlying `_safe_print` already handles this pattern elsewhere. |

## Non-Goals

- No refactoring beyond what each bullet specifies.
- No new features, no UX changes, no config additions.
- No reformatting / stylistic rewrites.
- Existing behavior preserved everywhere except where the bug fix explicitly alters it.

## Risk and Testing

Each item is a local change in one file. Risks:

- **C3 / C4 (voice file deletion)**: user might still reference these voices in a manually-edited ini outside of the pool. Impact: the file would re-download on first use — acceptable. No code breakage.
- **B1 (SET_SPEAKER handler)**: new code path. Validate by sending `SET_SPEAKER:Alice` via UDP and confirming `[speakers] current = 'Alice'` appears.
- **B2 (_load robustness)**: unit test — feed malformed JSON variants and confirm the manager still loads what it can and recovers on next save.
- **B5 (play/stop race)**: hard to test deterministically; the fix is a small safety check that doesn't change the happy path.

For everything else, import-and-run smoke test is sufficient (`python -c "import main; import tts; import speakers; import ocr; print('ok')"`).

## What "done" looks like

- All 9 cleanup items applied
- All 8 bug fixes applied
- `python -c "import main"` loads without error
- `python main.py --debug` starts, preloads voices, and handles SET_SPEAKER UDP commands
- `speakers.json` with a deliberately-malformed `cycle_index: {"foo": null}` still loads the valid parts
- One git commit per logical group (~4-6 commits total): dead-code-removal, docs updates, SET_SPEAKER handler, speakers.py robustness, assertion replacements, misc.
