"""
Sample-generator for Kokoro voices: renders each voice to a WAV in the
project root so you can listen and compare.

Run:
    python test_voices.py                       # render all SAMPLE_VOICES
    python test_voices.py kokoro:af_heart kokoro:am_michael
"""

from __future__ import annotations

import sys
import wave
from pathlib import Path

import numpy as np

SAMPLE_VOICES = [
    "kokoro:af_heart",
    "kokoro:am_michael",
    "kokoro:bf_emma",
    "kokoro:bm_george",
]

TEST_TEXT = (
    "Hello there. This is a sample of my voice. I like long walks on the "
    "beach and a good cup of tea. How do I sound to you?"
)


def render(voice: str, out_dir: Path) -> Path:
    from tts import TTS, _parse_voice

    _engine, name = _parse_voice(voice)
    tts = TTS(voice=voice, speed=1.0)
    k = tts._get_kokoro()
    assert k is not None, "Kokoro engine unavailable"
    audio, sr = k.synth(TEST_TEXT, name, speed=1.0)

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
    voices = sys.argv[1:] if len(sys.argv) > 1 else SAMPLE_VOICES
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
