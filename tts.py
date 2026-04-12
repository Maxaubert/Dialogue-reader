"""
Natural-voice TTS via Piper (local neural model, ONNX runtime).

Multi-voice with on-demand caching: TTS holds a {voice_name -> PiperVoice}
dict. The first time a voice is used it's loaded from disk (or downloaded
from HuggingFace if not already cached locally), then it stays in RAM for
instant switching. Each loaded voice uses ~60-100 MB of memory.

Usage:
    tts = TTS()                                  # default: en_US-amy-medium
    tts = TTS(voice="en_US-lessac-medium")       # different default
    tts.speak("hello")                           # use the default voice
    tts.speak("hi", voice="en_US-ryan-medium")   # use a specific voice
    tts.shutdown()

Voices download once and stay on disk in voices/ — subsequent runs are
instant. The caller doesn't need to pre-download anything; speak() will
fetch on first use.

Recommended voices (all ~60 MB unless noted):
    en_US-amy-medium         (default; female, US, neutral)
    en_US-lessac-medium      (female, US, very clean)
    en_US-ryan-medium        (male, US)
    en_US-ryan-high          (male, US, ~110 MB)
    en_US-joe-medium         (male, US, deeper)
    en_GB-alan-medium        (male, British)
    en_GB-jenny_dioco-medium (female, British)
"""

from __future__ import annotations

import threading
import urllib.request
from pathlib import Path

import numpy as np
import sounddevice as sd
from piper import PiperVoice
from piper.config import SynthesisConfig


DEFAULT_VOICE = "en_US-amy-medium"
_VOICE_REPO_BASE = "https://huggingface.co/rhasspy/piper-voices/resolve/main"


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

        self._speed = max(0.3, min(3.0, float(speed)))
        self._syn_config = SynthesisConfig(
            length_scale=1.0 / self._speed,  # piper: lower = faster
        )

        # Version counter for cancellation. Each speak() bumps this; in-flight
        # synth/playback workers compare against the current value and exit
        # early if a newer call has superseded them.
        self._version = 0
        self._version_lock = threading.Lock()

        # Pre-load the default voice so the very first speak() is instant.
        self._get_voice(self._default_voice)

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
        so the first cycle/speak with each one is instant."""
        for name in voice_names:
            try:
                self._get_voice(name)
            except Exception as e:
                print(f"[tts] preload of '{name}' failed: {e}", flush=True)

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

        # Stop any audio that's currently playing.
        try:
            sd.stop()
        except Exception:
            pass

        with self._version_lock:
            self._version += 1
            my_version = self._version

        voice_name = voice or self._default_voice

        def worker():
            try:
                piper_voice = self._get_voice(voice_name)
                chunks: list[np.ndarray] = []
                sample_rate: int | None = None
                for chunk in piper_voice.synthesize(text, self._syn_config):
                    if my_version != self._version:
                        return  # superseded by a newer speak() call
                    chunks.append(chunk.audio_float_array)
                    sample_rate = chunk.sample_rate

                if not chunks or sample_rate is None:
                    return
                if my_version != self._version:
                    return

                audio = np.concatenate(chunks)
                sd.play(audio, samplerate=sample_rate, blocking=False)
            except Exception as e:
                print(f"[tts] worker error: {e}", flush=True)

        threading.Thread(target=worker, daemon=True).start()

    def shutdown(self) -> None:
        with self._version_lock:
            self._version += 1
        try:
            sd.stop()
        except Exception:
            pass
