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
import time
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
    def __init__(self, voice: str = DEFAULT_VOICE, speed: float = 1.0,
                 media_gate=None):
        self._voices_dir = Path(__file__).parent / "voices"
        self._default_voice = voice
        # Optional media_gate.MediaGate: pauses other media while we speak.
        self._media_gate = media_gate
        # True while a gated (pause_media=True) utterance is in flight.
        self._gate_active = False

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

    # ---- media gate ----

    @property
    def media_gate(self):
        return self._media_gate

    def set_media_gate(self, gate) -> None:
        """Swap the media gate at runtime (RELOAD_CONFIG). The old gate is
        shut down so any media it paused resumes immediately."""
        old, self._media_gate = self._media_gate, gate
        if old is not None:
            try:
                old.shutdown()
            except Exception:
                pass

    def set_default_voice(self, voice: str) -> None:
        """Change the voice used when speak() gets no explicit voice."""
        self._default_voice = voice

    def _gate_started(self) -> None:
        if self._media_gate is not None:
            try:
                self._media_gate.speech_started()
            except Exception:
                pass

    def _gate_ended(self) -> None:
        if self._media_gate is not None:
            try:
                self._media_gate.speech_ended()
            except Exception:
                pass

    # ---- control ----

    def stop(self) -> None:
        """Cancel any in-progress speech immediately."""
        with self._version_lock:
            self._version += 1
        try:
            sd.stop()
        except Exception:
            pass
        self._gate_active = False
        self._gate_ended()

    def speak(self, text: str, voice: str | None = None,
              pause_media: bool = True) -> None:
        """Speak `text`. If `voice` is None, uses the default voice. Any
        previously-playing speech is interrupted. `pause_media=False` marks
        an announcement (startup cues, "voice changed", previews): it skips
        the media gate so the user's music/video keeps playing."""
        if not text:
            return

        if pause_media:
            self._gate_active = True
            self._gate_started()
        elif self._gate_active:
            # This announcement is about to cut off gated dialogue speech;
            # close out the gate so media is not left stuck paused.
            self._gate_active = False
            self._gate_ended()

        try:
            sd.stop()
        except Exception:
            pass

        with self._version_lock:
            self._version += 1
            my_version = self._version

        voice_name = voice or self._default_voice
        _engine, name = _parse_voice(voice_name)

        def _finish():
            """Close out the gate for this utterance, if it owned it."""
            if pause_media and my_version == self._version:
                self._gate_active = False
                self._gate_ended()

        def worker():
            try:
                k = self._get_kokoro()
                if k is None:
                    # No synthesis happened; media must not stay paused.
                    _finish()
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
                    return
                # Time the playback out instead of sd.wait(): the global
                # stream's wait() is not safe against a concurrent stop()/
                # play() from another thread (native crash, issue #20).
                duration = len(audio) / float(sample_rate) if sample_rate else 0.0
                deadline = time.monotonic() + duration + 0.2
                while time.monotonic() < deadline:
                    if my_version != self._version:
                        return
                    time.sleep(0.05)
                _finish()
            except Exception as e:
                print(f"[tts] kokoro worker error: {e}", flush=True)
                _finish()

        threading.Thread(target=worker, daemon=True).start()

    def shutdown(self) -> None:
        with self._version_lock:
            self._version += 1
        try:
            sd.stop()
        except Exception:
            pass
        if self._media_gate is not None:
            try:
                self._media_gate.shutdown()
            except Exception:
                pass
