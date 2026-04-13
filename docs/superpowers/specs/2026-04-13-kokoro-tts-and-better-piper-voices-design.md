# Kokoro TTS + Better Piper Voices — Design Spec

## Goal

Add Kokoro-82M as a second local TTS engine alongside Piper, and include higher-quality Piper voices in the default pool. Both engines selectable per-voice via an `engine:voice` prefix syntax in the ini config.

## Motivation

- Current Piper voices are OK but not the most natural available.
- Kokoro-82M (Apache 2.0, 82M params, ONNX) delivers noticeably more natural voices at Piper-comparable CPU speed.
- Research rated Kokoro ~40-60% more natural than current Piper voices.
- Some Piper voices we already have access to (`lessac-high`, `hfc_female-medium`, `libritts_r-medium`) are higher quality than the defaults in our current pool.

## Scope

**In scope:**
- Add `kokoro-onnx` dependency
- New `kokoro.py` module wrapping `kokoro-onnx` with voice load + synth
- Modify `tts.py` to dispatch on an `engine:voice_name` syntax
- Add curated Kokoro voices to the default pool
- Add higher-quality Piper voices to the default pool

**Out of scope:**
- No new hotkey for engine switching
- No UI indicator of current engine
- No per-speaker engine locking
- No streaming synthesis (both engines synth the full utterance then play)

## Voice Naming Format

Config strings use `engine:voice_name` syntax:

```
Default=kokoro:af_heart
Pool=kokoro:af_heart,piper:en_US-lessac-high,kokoro:am_michael,piper:en_GB-alba-medium
```

- Bare names (no prefix) default to `piper:` for backward compatibility with existing inis / `speakers.json` entries.
- Prefix is case-insensitive but stored lowercase internally.
- Supported engines: `piper`, `kokoro`.
- Unknown engine prefixes fall back to the default voice with a one-line warning.

## Components

### `kokoro.py` (new)

Lightweight wrapper around `kokoro-onnx`. Loads voices on demand, caches loaded `Kokoro` instance(s), synthesizes audio.

Interface:

```python
class KokoroTTS:
    def __init__(self, voices_dir: Path): ...
    def synth(self, text: str, voice: str) -> tuple[np.ndarray, int]:
        """Return (float32 audio, sample_rate)."""
```

The `kokoro-onnx` package wants one ONNX file + one voices `.bin` file loaded once; voice selection per-synth is via the `voice` parameter on the model's `create()` call. So we load a single `Kokoro` instance and hand it the requested voice name per synth.

First use downloads the model files (~310MB combined) to `voices/kokoro/` just like Piper does with its voice files.

### `tts.py` (modified)

Current design: flat `{voice_name -> PiperVoice}` cache, `speak(text, voice)` loads and plays.

New design:
- Parse `voice` input: split on `:` → `(engine, name)` with `engine` defaulting to `piper` when no colon.
- Dispatch:
  - `piper` → existing `_get_piper_voice(name)` path (unchanged)
  - `kokoro` → new `_kokoro.synth(text, name)` path
- Both branches produce `(audio_np, sample_rate)`; play via existing sounddevice sink (which already handles per-utterance sample rate).
- Interrupt behavior unchanged — `stop()` halts current playback; switching engines mid-sentence works.

The `speak()` public signature stays the same: `speak(text: str, voice: str | None = None)`.

### `dialogue_reader.ini` (modified)

Default `[Voices]` section becomes:

```
Default=piper:en_US-amy-medium

; Mixed-engine round-robin pool. Order matters — new speakers get
; pool[N % len] in arrival order.
;
; Kokoro voices (Apache 2.0, more natural, ~350ms/sent CPU):
;   af_heart, af_bella, af_sarah, am_michael, am_adam, bf_emma, bm_george
; Piper voices (fast, decent quality):
;   en_US-amy-medium, en_US-ryan-medium, en_US-lessac-high,
;   en_US-hfc_female-medium, en_US-hfc_male-medium, en_GB-alba-medium,
;   en_GB-northern_english_male-medium
Pool=kokoro:af_heart,piper:en_US-lessac-high,kokoro:am_michael,piper:en_US-hfc_female-medium,kokoro:bf_emma,piper:en_GB-alan-medium,kokoro:am_adam,piper:en_US-ryan-medium
```

User can edit the pool to go pure-Piper, pure-Kokoro, or any mix.

### `speakers.json` compatibility

Existing entries are bare Piper voice names like `en_US-amy-medium`. These continue to work (bare names default to `piper:`). No migration needed. New assignments made after the change will use the prefix format automatically since voices come from the pool as prefixed strings.

## Data Flow

```
main.py  ── speaker_mgr.voice_for_current() ──►  "kokoro:af_heart"
             │
             ▼
tts.speak(text, voice="kokoro:af_heart")
             │
             ▼
       split on ":"
             │
       ┌─────┴─────┐
       │           │
       ▼           ▼
   piper path   kokoro path
       │           │
       ▼           ▼
   audio_np + sample_rate
             │
             ▼
   sounddevice.play(...)
```

## Error Handling

- **Unknown engine prefix** (e.g. `elevenlabs:xyz`): log a warning, fall back to the default voice.
- **Unknown voice name within an engine**: let the engine's own error propagate (Piper already handles missing voice; Kokoro raises).
- **Model file download failure on first use**: same behavior as current Piper — error message, voice unusable until download succeeds. No silent fallback.
- **Kokoro fails to initialize at startup** (e.g. onnx runtime missing): log a warning, mark kokoro engine as unavailable; any `kokoro:` voice resolves to the default Piper voice instead.

## Dependencies

- New: `kokoro-onnx` (Apache 2.0, pip-installable, Windows-friendly)

## What "done" looks like

- App starts with both engines available (or at least Piper — Kokoro degrades gracefully if unavailable)
- Editing `Pool=` in the ini with a mix of `piper:` and `kokoro:` voices just works after restart
- Speaker cycle hotkeys walk through the mixed pool treating each entry as opaque
- Subjective listening test: Kokoro voices sound noticeably more natural than Piper on game dialogue
