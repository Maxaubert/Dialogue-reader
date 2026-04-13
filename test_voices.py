"""
Sample-generator test for new voices — writes WAV files to the project root
so you can play them back and compare. Renders each voice through the full
TTS dispatch, so this also exercises engine selection & fallback paths.

Run:
    python test_voices.py            # render all NEW_VOICES
    python test_voices.py piper:en_US-ryan-high sherpa:vctk:0
"""

from __future__ import annotations

import sys
import wave
from pathlib import Path

import numpy as np

# Voices added in the latest round — edit this list as we add more.
NEW_VOICES = [
    "sherpa:melo_en:0",
    "sherpa:libritts_r:0",
    "sherpa:libritts_r:100",
    "sherpa:libritts_r:500",
]

TEST_TEXT = (
    "Hello there. This is a sample of my voice. I like long walks on the "
    "beach and a good cup of tea. How do I sound to you?"
)


def render(voice: str, out_dir: Path) -> Path:
    # Import inside the function so a missing engine doesn't crash the whole
    # script — _get_<engine> will just log and fallback to piper.
    from tts import TTS, _parse_voice

    engine, _name = _parse_voice(voice)
    tts = TTS(voice=f"piper:en_US-amy-medium", speed=1.0)

    # We want the raw audio+rate rather than playback. Call into the engine
    # directly via the same helpers tts.speak() uses, skipping sounddevice.
    if engine == "piper":
        piper_voice = tts._get_voice(_name)
        chunks: list[np.ndarray] = []
        sr: int | None = None
        from piper.config import SynthesisConfig
        for ch in piper_voice.synthesize(TEST_TEXT, SynthesisConfig(length_scale=1.0)):
            chunks.append(ch.audio_float_array)
            sr = ch.sample_rate
        assert sr is not None and chunks
        audio = np.concatenate(chunks)
    elif engine == "kokoro":
        k = tts._get_kokoro()
        assert k is not None, "Kokoro engine unavailable"
        audio, sr = k.synth(TEST_TEXT, _name, speed=1.0)
    elif engine == "sherpa":
        s = tts._get_sherpa()
        assert s is not None, "Sherpa engine unavailable"
        audio, sr = s.synth(TEST_TEXT, _name, speed=1.0)
    else:
        raise ValueError(f"unknown engine '{engine}'")

    safe = voice.replace(":", "_").replace("/", "_")
    out = out_dir / f"sample_{safe}.wav"
    pcm = (np.clip(audio, -1.0, 1.0) * 32767).astype(np.int16)
    with wave.open(str(out), "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(sr)
        w.writeframes(pcm.tobytes())
    return out


def main() -> int:
    voices = sys.argv[1:] if len(sys.argv) > 1 else NEW_VOICES
    out_dir = Path(__file__).parent
    for voice in voices:
        try:
            path = render(voice, out_dir)
            print(f"[ok] {voice}  ->  {path.name}")
        except Exception as e:
            print(f"[err] {voice}: {e}", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
