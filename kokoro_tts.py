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
