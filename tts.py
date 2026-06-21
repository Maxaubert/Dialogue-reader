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
