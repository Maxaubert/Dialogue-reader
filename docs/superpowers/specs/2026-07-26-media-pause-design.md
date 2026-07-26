# Media pause during speech - design

Date: 2026-07-26. Issue: #8.

## Problem

While gaming, the user often listens to other media (YouTube in a browser, Spotify).
When the reader speaks a tooltip or dialogue line, it talks over that media. The reader
should pause playing media when it starts speaking and resume it once it has been quiet
for a moment.

## Decisions (from design dialogue)

- Resume after a **1 second** quiet period (configurable). New speech during the grace
  period cancels the resume, so bursts of dialogue lines do not stutter the media.
- Toggle is **ini only**: `[Media] PauseDuringSpeech`, default `true`. No hotkey.
- Windows **GSMTC** (Global System Media Transport Controls) sessions are the mechanism.
  Verified live: a playing YouTube tab appears as a pausable session; games (VEIN) never
  register a session, so the game itself cannot be paused by this feature.
- Rejected alternatives: media-key emulation (blind toggle, no state, wrong-app risk) and
  audio ducking (user would miss content that keeps playing; new dependency).
- Zero new dependencies: `winrt.windows.media.control` is already installed (winocr deps).

## Architecture

### `media_gate.py` (new)

One class, `MediaGate`, plus a private winrt session-source adapter.

Public surface:

- `MediaGate(resume_delay_ms=1000, session_source=None, verbose=False)`
  - `session_source`: callable returning a list of session objects (injectable for tests).
    Default is the winrt GSMTC adapter, which owns a daemon thread running an asyncio
    loop for the async winrt calls.
- `speech_started()`: cancels any pending resume timer, then (on a worker thread) pauses
  every session that is currently playing and pausable, recording app ids in
  `_paused_ids`. Called on every utterance, so media a user manually resumed mid-speech
  gets re-paused on the next line.
- `speech_ended()`: (re)starts a `threading.Timer` for the resume delay. On fire, resumes
  only sessions whose app id is in `_paused_ids` AND that are still paused (never fights
  manual control), then clears `_paused_ids`.
- `shutdown()`: cancels the timer and resumes synchronously, so quitting the reader never
  leaves media stuck paused.

Session object interface (what fakes implement): `app_id`, `is_playing`, `is_paused`,
`can_pause`, `can_play`, `pause()`, `play()`.

Error handling: every session-source and per-session call is wrapped; failures print
under `verbose` and never propagate into TTS.

### `tts.py` hooks

`TTS(..., media_gate=None)`.

- `speak()`: after the empty-text early return, call `gate.speech_started()` (pause
  happens during Kokoro synthesis, before audio starts).
- worker: after `sd.play`, `sd.wait()`; when playback finishes and this utterance is
  still the current version, call `gate.speech_ended()`. Superseded workers stay silent
  (the newer utterance's lifecycle governs).
- `stop()`: call `gate.speech_ended()` (a stop means silence begins now).
- `shutdown()`: call `gate.shutdown()`.

Spoken confirmations and the startup chime go through `speak()` too, so they briefly
pause media. Accepted for consistency and simplicity.

### `main.py` wiring

- `_load_media_config()` reads `[Media]` from `dialogue_reader.ini`:
  `PauseDuringSpeech` (bool, default true), `ResumeDelayMs` (int, default 1000).
- When enabled, construct `MediaGate` and pass it to `TTS`. When disabled, pass nothing:
  zero overhead. Startup line prints the state in debug mode.

### Config

```ini
[Media]
; Pause other media (YouTube, Spotify, ...) while the reader speaks,
; resume after the reader has been quiet for ResumeDelayMs.
PauseDuringSpeech = true
ResumeDelayMs = 1000
```

## Testing

TDD throughout. Unit tests drive `MediaGate` with fake sessions and short real timers:

- pauses only playing+pausable sessions; records them
- resume fires after the delay, resumes only our sessions that are still paused
- manually-resumed session (playing again at resume time) is skipped
- `speech_started()` during the grace period cancels the resume
- `shutdown()` resumes immediately
- session-source and per-session errors are swallowed

TTS tests use a fake gate to verify hook ordering on speak, interrupt, stop, shutdown.
Config parsing tests cover defaults, disable, and invalid values. Live manual check:
YouTube playing, reader speaks, video pauses, resumes about 1s after silence.

## Implementation plan

1. Issue #8, branch `feat/media-pause-during-speech`. (done)
2. TDD `media_gate.py` bookkeeping and debounce against fakes.
3. TDD TTS hook points.
4. Wire `main.py`, add ini section, README row.
5. Full suite, live verification, PR referencing #8, merge.

## Out of scope

The Electron management UI and per-game profiles are separate future projects with
their own design passes. This feature keeps its settings in the ini so that UI can
manage them later.
