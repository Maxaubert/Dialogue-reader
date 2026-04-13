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
  Piper (fast, ~60 MB each; -high variants ~110 MB):
    en_US-amy-medium, en_US-ryan-high, en_US-hfc_female-medium,
    en_US-hfc_male-medium, en_GB-alan-medium, en_GB-alba-medium,
    en_GB-jenny_dioco-medium, en_GB-northern_english_male-medium
  Kokoro (natural, share one ~310 MB model):
    af_heart, af_bella, af_sarah (US female)
    am_michael, am_adam (US male)
    bf_emma (British female)
    bm_george (British male)
"""

from __future__ import annotations

import threading
import urllib.request
from pathlib import Path

import numpy as np
import sounddevice as sd
from piper import PiperVoice
from piper.config import SynthesisConfig


DEFAULT_VOICE = "kokoro:af_heart"
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


def _voice_url_base(voice_name: str) -> str:
    """e.g. en_US-amy-medium -> .../en/en_US/amy/medium"""
    lang_country, speaker, quality = voice_name.split("-", 2)
    lang = lang_country.split("_")[0]
    return f"{_VOICE_REPO_BASE}/{lang}/{lang_country}/{speaker}/{quality}"


def _download_with_progress(url: str, dest: Path) -> None:
    """Download to a .tmp file then atomic-rename to dest. If anything
    fails midway, only the .tmp file is left behind and dest is never
    in a partially-written state."""
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
            f"\r[tts] downloading {dest.name}: {pct:3d}% ({mb:.1f} / {total_mb:.1f} MB)",
            end="",
            flush=True,
        )

    try:
        urllib.request.urlretrieve(url, tmp, reporthook=hook)
        print()  # newline after progress
        tmp.replace(dest)  # atomic rename
    except Exception:
        if tmp.exists():
            tmp.unlink(missing_ok=True)
        raise


# Anything below this size for a .onnx file is treated as a partial /
# corrupt download. Real Piper voices are at least ~25 MB; the smallest
# medium voices land around 60 MB. 1 MB is a safe floor.
_MIN_ONNX_BYTES = 1_000_000
_MIN_JSON_BYTES = 100


def _validate_or_remove(path: Path, min_bytes: int) -> bool:
    """Return True if the file is present and looks intact. If it exists
    but is too small (truncated/corrupt), delete it and return False."""
    if not path.exists():
        return False
    try:
        size = path.stat().st_size
    except OSError:
        return False
    if size < min_bytes:
        print(
            f"[tts] '{path.name}' looks truncated ({size} bytes), "
            f"deleting and redownloading"
        )
        try:
            path.unlink(missing_ok=True)
        except OSError:
            pass
        return False
    return True


def _ensure_voice(voice_dir: Path, voice_name: str) -> Path:
    voice_dir.mkdir(parents=True, exist_ok=True)
    onnx_path = voice_dir / f"{voice_name}.onnx"
    json_path = voice_dir / f"{voice_name}.onnx.json"

    onnx_ok = _validate_or_remove(onnx_path, _MIN_ONNX_BYTES)
    json_ok = _validate_or_remove(json_path, _MIN_JSON_BYTES)
    if onnx_ok and json_ok:
        return onnx_path

    base = _voice_url_base(voice_name)
    print(f"[tts] downloading voice model '{voice_name}' (~60 MB)")
    try:
        if not onnx_path.exists():
            _download_with_progress(f"{base}/{voice_name}.onnx", onnx_path)
        if not json_path.exists():
            _download_with_progress(
                f"{base}/{voice_name}.onnx.json", json_path
            )
    except Exception as e:
        # Clean up partial files so a retry works.
        for p in (onnx_path, json_path):
            if p.exists() and p.stat().st_size < _MIN_ONNX_BYTES:
                p.unlink(missing_ok=True)
        raise RuntimeError(
            f"Failed to download Piper voice '{voice_name}': {e}\n"
            f"Check internet connection or try a different voice name."
        ) from e

    return onnx_path


class TTS:
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

        # Lazy Sherpa-ONNX engine — initialized on first sherpa: voice request.
        self._sherpa = None
        self._sherpa_unavailable = False

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
        elif engine == "sherpa":
            try:
                self._get_sherpa()
            except Exception as e:
                print(f"[tts] Sherpa init failed: {e}", flush=True)

    def _get_sherpa(self):
        """Lazily construct SherpaTTS. Returns None and sets
        _sherpa_unavailable=True if sherpa-onnx isn't installed or fails
        to load."""
        if self._sherpa_unavailable:
            return None
        if self._sherpa is not None:
            return self._sherpa
        try:
            from sherpa_tts import SherpaTTS
        except Exception as e:
            print(f"[tts] sherpa_tts module unavailable: {e}", flush=True)
            self._sherpa_unavailable = True
            return None
        try:
            self._sherpa = SherpaTTS(self._voices_dir)
        except Exception as e:
            print(f"[tts] SherpaTTS init failed: {e}", flush=True)
            self._sherpa_unavailable = True
            return None
        return self._sherpa

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

    # ---- voice cache ----

    def _get_voice(self, voice_name: str) -> PiperVoice:
        """Return a loaded PiperVoice, fetching+loading on first request.
        If the cached file fails to load (truncated download, corrupt
        protobuf, etc.) we delete it and try downloading once more."""
        with self._cache_lock:
            cached = self._voice_cache.get(voice_name)
            if cached is not None:
                return cached
        # Load outside the lock so concurrent requests for *different*
        # voices don't serialize.
        onnx_path = _ensure_voice(self._voices_dir, voice_name)
        print(f"[tts] Loading Piper voice '{voice_name}'...")
        try:
            loaded = PiperVoice.load(str(onnx_path))
        except Exception as e:
            # File exists but Piper can't parse it — corrupt download.
            # Wipe and retry exactly once with a fresh download.
            print(
                f"[tts] '{voice_name}' failed to load ({e.__class__.__name__}: "
                f"{e}); the file is corrupt. Deleting and redownloading."
            )
            for suffix in (".onnx", ".onnx.json"):
                p = self._voices_dir / f"{voice_name}{suffix}"
                try:
                    p.unlink(missing_ok=True)
                except OSError:
                    pass
            onnx_path = _ensure_voice(self._voices_dir, voice_name)
            loaded = PiperVoice.load(str(onnx_path))
        with self._cache_lock:
            self._voice_cache[voice_name] = loaded
            return loaded

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
            elif engine == "sherpa":
                # Each sherpa model is loaded on demand inside SherpaTTS.
                # Touch _get_sherpa once so the instance exists.
                try:
                    self._get_sherpa()
                except Exception as e:
                    print(f"[tts] Sherpa preload failed: {e}", flush=True)

    # ---- speed ----

    def set_speed(self, speed: float) -> None:
        """Update playback speed. Takes effect on the next speak() call."""
        self._speed = max(0.3, min(3.0, float(speed)))
        self._syn_config = SynthesisConfig(length_scale=1.0 / self._speed)

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
        engine, name = _parse_voice(voice_name)

        # Unknown engine: warn once and fall back to the default voice.
        if engine not in ("piper", "kokoro", "sherpa"):
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

        # Sherpa requested but unavailable: fall back to default Piper.
        if engine == "sherpa":
            if self._get_sherpa() is None:
                fb_engine, fb_name = _parse_voice(self._default_voice)
                if fb_engine == "piper":
                    engine, name = fb_engine, fb_name
                else:
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

        def worker_kokoro():
            try:
                k = self._get_kokoro()
                if k is None:
                    return
                # Pass speed to Kokoro's own time-stretch (pitch-preserving),
                # then play at the native sample rate so pitch is unchanged.
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

        if engine == "kokoro":
            worker = worker_kokoro
        elif engine == "sherpa":
            worker = worker_sherpa
        else:
            worker = worker_piper
        threading.Thread(target=worker, daemon=True).start()

    def shutdown(self) -> None:
        with self._version_lock:
            self._version += 1
        try:
            sd.stop()
        except Exception:
            pass
