# Kokoro TTS + Better Piper Voices Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add Kokoro-82M as a second local TTS engine with `engine:voice` prefix syntax, and include higher-quality Piper voices in the default pool.

**Architecture:** Introduce a thin `kokoro_tts.py` module mirroring the Piper loader pattern. `TTS.speak()` parses `voice` as `engine:name` and dispatches to the matching engine; both produce float32 audio + sample_rate that play through the existing sounddevice sink. Bare voice names (no colon) default to `piper:` for backward compatibility.

**Tech Stack:** Python, `piper-tts` (existing), `kokoro-onnx` (new), `sounddevice`, `numpy`

---

### Task 1: Add `kokoro-onnx` dependency and verify install

**Files:**
- Modify: `requirements.txt`

- [ ] **Step 1: Install kokoro-onnx in the worktree venv (or system python)**

Run:
```bash
pip install kokoro-onnx
```

Expected: installs `kokoro-onnx` and its deps (`onnxruntime`, `soundfile`, etc.). Typical total install size ~200-300MB for onnxruntime.

- [ ] **Step 2: Verify import works**

Run:
```bash
python -c "from kokoro_onnx import Kokoro; print('kokoro_onnx import OK')"
```

Expected: prints `kokoro_onnx import OK` (no model files yet — just the library).

- [ ] **Step 3: Add to requirements.txt**

Open `requirements.txt`. It currently looks like:

```
mss>=10.0
Pillow>=10.0
numpy>=1.24
PySide6>=6.6
pyttsx3>=2.99
rapidocr-onnxruntime>=1.2
```

Add `kokoro-onnx>=0.3.0` on a new line at the end:

```
mss>=10.0
Pillow>=10.0
numpy>=1.24
PySide6>=6.6
pyttsx3>=2.99
rapidocr-onnxruntime>=1.2
kokoro-onnx>=0.3.0
```

- [ ] **Step 4: Commit**

```bash
git add requirements.txt
git commit -m "deps: add kokoro-onnx for Kokoro-82M TTS engine"
```

---

### Task 2: Create `kokoro_tts.py` module with voice loader

**Files:**
- Create: `kokoro_tts.py`

- [ ] **Step 1: Write the module**

Create `kokoro_tts.py` with exactly this content:

```python
"""
Kokoro-82M TTS wrapper.

Mirrors the Piper loader pattern: lazily downloads model files to voices/kokoro/
on first use, loads a single Kokoro instance, and exposes synth(text, voice)
returning (float32 numpy array, sample_rate).

The kokoro-onnx package ships two file paths we have to provide explicitly:
  - an ONNX model file (~310 MB)
  - a voices .bin file (~6 MB, contains every speaker embedding)

Usage:
    k = KokoroTTS(Path("voices"))
    audio, sr = k.synth("hello world", voice="af_heart")
"""

from __future__ import annotations

import urllib.request
from pathlib import Path
from threading import Lock

import numpy as np

_MODEL_URL = (
    "https://github.com/thewh1teagle/kokoro-onnx/releases/download/model-files-v1.0/"
    "kokoro-v1.0.onnx"
)
_VOICES_URL = (
    "https://github.com/thewh1teagle/kokoro-onnx/releases/download/model-files-v1.0/"
    "voices-v1.0.bin"
)

# File size floors used to detect partial/corrupt downloads.
_MIN_MODEL_BYTES = 100_000_000   # ~310 MB real; floor well below that
_MIN_VOICES_BYTES = 1_000_000    # ~6 MB real


def _download_with_progress(url: str, dest: Path) -> None:
    """Download via urlretrieve to a .tmp file, atomic-rename to dest."""
    tmp = dest.with_name(dest.name + ".tmp")
    last_pct = [-1]

    def hook(block_num: int, block_size: int, total_size: int) -> None:
        if total_size <= 0:
            return
        pct = min(100, block_num * block_size * 100 // total_size)
        if pct == last_pct[0]:
            return
        last_pct[0] = pct
        mb = block_num * block_size / (1024 * 1024)
        total_mb = total_size / (1024 * 1024)
        print(
            f"\r[kokoro] downloading {dest.name}: {pct:3d}% "
            f"({mb:.1f} / {total_mb:.1f} MB)",
            end="",
            flush=True,
        )

    try:
        urllib.request.urlretrieve(url, tmp, reporthook=hook)
        print()
        tmp.replace(dest)
    except Exception:
        if tmp.exists():
            tmp.unlink(missing_ok=True)
        raise


def _ensure_file(dest: Path, url: str, min_bytes: int) -> None:
    """Ensure `dest` exists and is at least `min_bytes`. Downloads if not."""
    if dest.exists():
        try:
            size = dest.stat().st_size
        except OSError:
            size = 0
        if size >= min_bytes:
            return
        # Too small — treat as corrupt, redownload.
        print(
            f"[kokoro] '{dest.name}' looks truncated ({size} bytes), "
            f"deleting and redownloading"
        )
        dest.unlink(missing_ok=True)
    dest.parent.mkdir(parents=True, exist_ok=True)
    _download_with_progress(url, dest)


class KokoroTTS:
    def __init__(self, voices_dir: Path) -> None:
        self._dir = voices_dir / "kokoro"
        self._dir.mkdir(parents=True, exist_ok=True)
        self._model_path = self._dir / "kokoro-v1.0.onnx"
        self._voices_path = self._dir / "voices-v1.0.bin"
        self._kokoro = None
        self._load_lock = Lock()

    def _ensure_loaded(self) -> None:
        if self._kokoro is not None:
            return
        with self._load_lock:
            if self._kokoro is not None:
                return
            _ensure_file(self._model_path, _MODEL_URL, _MIN_MODEL_BYTES)
            _ensure_file(self._voices_path, _VOICES_URL, _MIN_VOICES_BYTES)
            print("[kokoro] loading model...", flush=True)
            from kokoro_onnx import Kokoro
            self._kokoro = Kokoro(str(self._model_path), str(self._voices_path))
            print("[kokoro] ready", flush=True)

    def synth(self, text: str, voice: str) -> tuple[np.ndarray, int]:
        """Synthesize text. Returns (audio float32, sample_rate)."""
        self._ensure_loaded()
        assert self._kokoro is not None
        audio, sample_rate = self._kokoro.create(
            text, voice=voice, speed=1.0, lang="en-us"
        )
        audio = np.asarray(audio, dtype=np.float32)
        return audio, int(sample_rate)
```

- [ ] **Step 2: Smoke-test the module**

Run:
```bash
python -c "from kokoro_tts import KokoroTTS; print('module imports')"
```

Expected: prints `module imports` with no error. (No model download yet — that happens on first `synth()`.)

- [ ] **Step 3: Commit**

```bash
git add kokoro_tts.py
git commit -m "feat: add KokoroTTS wrapper module"
```

---

### Task 3: Add engine-prefix parsing to TTS class

**Files:**
- Modify: `tts.py`

- [ ] **Step 1: Add engine prefix parser at module level**

Open `tts.py`. Find this block near the top (around line 42-43):

```python
DEFAULT_VOICE = "en_US-amy-medium"
_VOICE_REPO_BASE = "https://huggingface.co/rhasspy/piper-voices/resolve/main"
```

Replace with:

```python
DEFAULT_VOICE = "piper:en_US-amy-medium"
_VOICE_REPO_BASE = "https://huggingface.co/rhasspy/piper-voices/resolve/main"


def _parse_voice(voice: str) -> tuple[str, str]:
    """Split 'engine:name' into (engine_lower, name). Bare names (no colon)
    default to the 'piper' engine for backward compatibility."""
    if ":" in voice:
        engine, _, name = voice.partition(":")
        engine = engine.strip().lower()
        name = name.strip()
        return engine, name
    return "piper", voice.strip()
```

Note: `DEFAULT_VOICE` now includes the `piper:` prefix explicitly so parsing is uniform.

- [ ] **Step 2: Update TTS.__init__ to initialize optional Kokoro**

Find the current `__init__` method (starts around line 146). Replace the WHOLE method with:

```python
    def __init__(
        self,
        voice: str = DEFAULT_VOICE,
        speed: float = 1.0,
    ):
        self._voices_dir = Path(__file__).parent / "voices"
        self._default_voice = voice
        self._voice_cache: dict[str, PiperVoice] = {}
        self._cache_lock = threading.Lock()

        # Lazy Kokoro engine — initialized on first kokoro: voice request.
        self._kokoro = None
        self._kokoro_unavailable = False

        self._speed = max(0.3, min(3.0, float(speed)))
        self._syn_config = SynthesisConfig(
            length_scale=1.0 / self._speed,  # piper: lower = faster
        )

        self._version = 0
        self._version_lock = threading.Lock()

        # Pre-load the default voice so the very first speak() is instant.
        self._ensure_default_loaded()

    def _ensure_default_loaded(self) -> None:
        engine, name = _parse_voice(self._default_voice)
        if engine == "piper":
            try:
                self._get_voice(name)
            except Exception as e:
                print(f"[tts] default voice preload failed: {e}", flush=True)
        elif engine == "kokoro":
            try:
                self._get_kokoro()
            except Exception as e:
                print(f"[tts] Kokoro init failed: {e}", flush=True)

    def _get_kokoro(self):
        """Lazily construct KokoroTTS. Returns None and sets
        _kokoro_unavailable=True if kokoro-onnx isn't installed or fails
        to load."""
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
```

- [ ] **Step 3: Run a quick import test**

Run:
```bash
python -c "from tts import TTS, _parse_voice; print(_parse_voice('kokoro:af_heart')); print(_parse_voice('en_US-amy-medium'))"
```

Expected:
```
('kokoro', 'af_heart')
('piper', 'en_US-amy-medium')
```

- [ ] **Step 4: Commit**

```bash
git add tts.py
git commit -m "feat: add engine-prefix parsing and lazy Kokoro engine in TTS"
```

---

### Task 4: Dispatch speak() on engine prefix

**Files:**
- Modify: `tts.py`

- [ ] **Step 1: Replace speak() to dispatch on engine**

Open `tts.py`. Find the current `speak` method (around line 236). Replace the WHOLE method with:

```python
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
        engine, name = _parse_voice(voice_name)

        # Unknown engine: warn once and fall back to the default voice.
        if engine not in ("piper", "kokoro"):
            print(
                f"[tts] unknown engine '{engine}' in voice "
                f"'{voice_name}', falling back to default",
                flush=True,
            )
            engine, name = _parse_voice(self._default_voice)

        # Kokoro requested but unavailable: fall back to default Piper.
        if engine == "kokoro":
            if self._get_kokoro() is None:
                fb_engine, fb_name = _parse_voice(self._default_voice)
                if fb_engine == "piper":
                    engine, name = fb_engine, fb_name
                else:
                    # Default is also Kokoro but Kokoro is dead — last-
                    # resort fallback to the hardcoded piper default.
                    engine, name = "piper", "en_US-amy-medium"

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

        def worker_kokoro():
            try:
                k = self._get_kokoro()
                if k is None:
                    return
                audio, sample_rate = k.synth(text, name)
                if my_version != self._version:
                    return
                # Kokoro speed control: scale playback sample_rate by speed.
                # Kokoro's internal `speed` param exists but we want consistent
                # behavior with Piper's length_scale, which changes duration.
                # Playing the same audio at sr*speed is the simplest way to
                # match "faster = shorter" semantics.
                effective_sr = int(sample_rate * self._speed)
                sd.play(audio, samplerate=effective_sr, blocking=False)
            except Exception as e:
                print(f"[tts] kokoro worker error: {e}", flush=True)

        worker = worker_kokoro if engine == "kokoro" else worker_piper
        threading.Thread(target=worker, daemon=True).start()
```

- [ ] **Step 2: Update preload() to skip non-piper voices for now**

Find the `preload` method (around line 206). Replace it with:

```python
    def preload(self, voice_names: list[str]) -> None:
        """Make sure each voice in the list is downloaded + loaded into RAM
        so the first cycle/speak with each one is instant. Only preloads
        Piper voices; Kokoro shares a single model across all voices so
        preloading individual Kokoro voice names is a no-op."""
        kokoro_seen = False
        for name in voice_names:
            engine, voice = _parse_voice(name)
            if engine == "piper":
                try:
                    self._get_voice(voice)
                except Exception as e:
                    print(f"[tts] preload of '{name}' failed: {e}", flush=True)
            elif engine == "kokoro" and not kokoro_seen:
                kokoro_seen = True
                # Trigger one-time Kokoro init so first Kokoro speak() is fast.
                try:
                    self._get_kokoro()
                except Exception as e:
                    print(f"[tts] Kokoro preload failed: {e}", flush=True)
```

- [ ] **Step 3: Manual smoke test — Piper path still works**

With the worktree venv or system python, run:

```bash
python -c "from tts import TTS; t = TTS(voice='piper:en_US-amy-medium', speed=1.0); t.speak('Hello from Piper.'); import time; time.sleep(3)"
```

Expected: "Hello from Piper." is spoken aloud.

- [ ] **Step 4: Manual smoke test — Kokoro path (downloads ~310 MB on first run)**

Run:
```bash
python -c "from tts import TTS; t = TTS(voice='piper:en_US-amy-medium', speed=1.0); t.speak('Hello from Kokoro.', voice='kokoro:af_heart'); import time; time.sleep(5)"
```

Expected: downloads Kokoro model files on first run (progress bar), then speaks "Hello from Kokoro." in the `af_heart` voice.

- [ ] **Step 5: Commit**

```bash
git add tts.py
git commit -m "feat: dispatch TTS.speak() on engine prefix (piper|kokoro)"
```

---

### Task 5: Update dialogue_reader.ini with mixed-engine default pool

**Files:**
- Modify: `dialogue_reader.ini`
- Modify: `main.py` (update `_DEFAULT_VOICE_POOL` to match)

- [ ] **Step 1: Find the default pool constant in main.py**

Open `main.py`. Find `_DEFAULT_VOICE_POOL` (use `grep -n _DEFAULT_VOICE_POOL main.py` if needed). It's a list of Piper voice names.

Replace that list with:

```python
_DEFAULT_VOICE_POOL = [
    "kokoro:af_heart",
    "piper:en_US-lessac-high",
    "kokoro:am_michael",
    "piper:en_US-hfc_female-medium",
    "kokoro:bf_emma",
    "piper:en_GB-alan-medium",
    "kokoro:am_adam",
    "piper:en_US-ryan-medium",
]
```

Also find the `DEFAULT_VOICE` usage — if `main.py` references it, the import still resolves to the new `"piper:en_US-amy-medium"` string.

- [ ] **Step 2: Update the [Voices] section of dialogue_reader.ini**

Open `dialogue_reader.ini`. Find the `[Voices]` section (at the end of the file). Replace the whole section with:

```ini
[Voices]
; Default voice used when no speaker is currently set.
; Format: engine:voice_name.  engine ∈ {piper, kokoro}.
; Bare names (no colon) are treated as piper: for backward compatibility.
Default=piper:en_US-amy-medium

; Round-robin pool for speaker auto-assignment AND cycle hotkeys.
; Mix engines freely. Order matters.
;
; Kokoro voices (Apache 2.0, natural, ~350ms/sent CPU, shares one ~310MB model):
;   af_heart, af_bella, af_sarah       (US female)
;   am_michael, am_adam                 (US male)
;   bf_emma                             (British female)
;   bm_george                           (British male)
;
; Piper voices (fast, ~60 MB per voice):
;   en_US-amy-medium, en_US-ryan-medium, en_US-lessac-medium,
;   en_US-lessac-high, en_US-hfc_female-medium, en_US-hfc_male-medium,
;   en_US-joe-medium,
;   en_GB-alan-medium, en_GB-alba-medium, en_GB-jenny_dioco-medium,
;   en_GB-northern_english_male-medium
Pool=kokoro:af_heart,piper:en_US-lessac-high,kokoro:am_michael,piper:en_US-hfc_female-medium,kokoro:bf_emma,piper:en_GB-alan-medium,kokoro:am_adam,piper:en_US-ryan-medium
```

- [ ] **Step 3: Manual end-to-end test**

```bash
python main.py --debug
```

Expected:
- Starts up normally.
- Default voice preload succeeds (`[tts] Loading Piper voice 'en_US-amy-medium'...`).
- Pool preload runs; first time through, Kokoro downloads models and loads, other Piper voices download.
- Pick a dialogue region and trigger some text. Voices cycle through the mixed pool.
- Ctrl+C cleanly exits.

- [ ] **Step 4: Commit**

```bash
git add dialogue_reader.ini main.py
git commit -m "feat: default voice pool now mixes Kokoro + higher-quality Piper voices"
```

---

### Task 6: Backward-compat test for bare voice names in speakers.json

**Files:**
- Modify/verify: no code changes expected — we're confirming existing entries still work.

- [ ] **Step 1: Create a test speakers.json with bare-name entries**

Make a backup, then temporarily write a simple test file. Run this in a Python shell from the worktree root:

```python
import json
from pathlib import Path

p = Path("speakers.json")
backup = Path("speakers.json.bak")
if p.exists() and not backup.exists():
    backup.write_bytes(p.read_bytes())

# Write a minimal test file with a bare-name assignment.
p.write_text(json.dumps({
    "assignments": {"TestCharacter": "en_US-lessac-medium"},
    "cycle_idx": {}
}))
print("wrote test speakers.json")
```

- [ ] **Step 2: Run app and verify the bare name resolves**

```bash
python main.py --debug
```

In the debug output, look for a line like:
```
[speakers] 1 speaker(s) loaded from speakers.json
```

- [ ] **Step 3: Quick in-process check**

In a separate Python shell:

```python
from tts import TTS, _parse_voice
print(_parse_voice("en_US-lessac-medium"))
```

Expected: `('piper', 'en_US-lessac-medium')`

- [ ] **Step 4: Restore speakers.json**

```python
from pathlib import Path
p = Path("speakers.json")
backup = Path("speakers.json.bak")
if backup.exists():
    p.write_bytes(backup.read_bytes())
    backup.unlink()
    print("restored")
```

- [ ] **Step 5: Commit (no code changes; commit only if any file was modified)**

```bash
git status
# If speakers.json changed unexpectedly, restore it from git:
# git checkout -- speakers.json
```

No commit required for this verification task.

---

### Task 7: Update tts.py docstring to reflect dual-engine support

**Files:**
- Modify: `tts.py`

- [ ] **Step 1: Replace the module docstring**

Open `tts.py`. Replace the top docstring (lines 1-28, the triple-quoted block at the very top of the file) with:

```python
"""
Natural-voice TTS with two local engines: Piper (fast) and Kokoro-82M
(more natural, ~350ms/sentence on CPU).

Voices are referenced as `engine:name` strings. Bare names (no colon)
default to `piper:` for backward compatibility.

Multi-voice with on-demand caching:
  - Piper: {voice_name -> PiperVoice} dict; each voice ~60-100 MB RAM.
  - Kokoro: single shared model (~310 MB on disk, loaded once into RAM),
    voice selection is a per-synth parameter.

Usage:
    tts = TTS()                                       # piper:en_US-amy-medium
    tts = TTS(voice="kokoro:af_heart")                 # default Kokoro voice
    tts.speak("hello")                                 # use default voice
    tts.speak("hi", voice="piper:en_US-ryan-medium")   # explicit Piper
    tts.speak("hi", voice="kokoro:am_michael")         # explicit Kokoro
    tts.shutdown()

Voice files download once on first use:
  - Piper voices → voices/<voice>.onnx + .onnx.json
  - Kokoro model + voices → voices/kokoro/{kokoro-v1.0.onnx, voices-v1.0.bin}

Recommended voices:
  Piper (fast, ~60 MB each):
    en_US-amy-medium, en_US-ryan-medium, en_US-lessac-medium,
    en_US-lessac-high, en_US-hfc_female-medium, en_US-hfc_male-medium,
    en_GB-alan-medium, en_GB-jenny_dioco-medium
  Kokoro (natural, share one ~310 MB model):
    af_heart, af_bella, af_sarah (US female)
    am_michael, am_adam (US male)
    bf_emma (British female)
    bm_george (British male)
"""
```

- [ ] **Step 2: Commit**

```bash
git add tts.py
git commit -m "docs: update tts.py module docstring for dual-engine support"
```
