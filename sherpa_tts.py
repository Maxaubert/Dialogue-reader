"""
Sherpa-ONNX TTS wrapper — ONNX-based VITS models with multi-speaker support.

Current supported model:
    vctk  — VCTK VITS, 109 British English speakers, speaker ID is an integer.

Voice string format (passed via TTS engine): `<model>:<speaker_id>` e.g. `vctk:0`.

Mirrors the Piper/Kokoro loader pattern: lazily downloads model files under
voices/sherpa_<model>/ on first use, caches a single OfflineTts instance per
model in RAM.

Usage:
    s = SherpaTTS(Path("voices"))
    audio, sr = s.synth("hello", voice="vctk:0")
"""

from __future__ import annotations

import tarfile
import urllib.request
from pathlib import Path
from threading import Lock

import numpy as np


# Model descriptors: name -> {url, archive_subdir, onnx, lexicon, tokens}.
# archive_subdir is the top-level dir inside the .tar.bz2 that extracts next
# to the archive (sherpa releases all nest under a subdir named after the
# model).
_MODELS = {
    "vctk": {
        "url": (
            "https://github.com/k2-fsa/sherpa-onnx/releases/download/"
            "tts-models/vits-vctk.tar.bz2"
        ),
        "archive_subdir": "vits-vctk",
        "onnx": "vits-vctk.onnx",
        "lexicon": "lexicon.txt",
        "tokens": "tokens.txt",
    },
}

# File-size floor to detect partial/corrupt downloads.
_MIN_ARCHIVE_BYTES = 50_000_000


def _download_with_progress(url: str, dest: Path) -> None:
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
            f"\r[sherpa] downloading {dest.name}: {pct:3d}% "
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


def _ensure_model(voices_dir: Path, model_name: str) -> Path:
    """Download + extract the model archive if missing. Returns path to the
    extracted model subdirectory."""
    spec = _MODELS[model_name]
    model_root = voices_dir / f"sherpa_{model_name}"
    model_root.mkdir(parents=True, exist_ok=True)
    extracted_dir = model_root / spec["archive_subdir"]
    onnx_path = extracted_dir / spec["onnx"]

    if onnx_path.exists() and onnx_path.stat().st_size > _MIN_ARCHIVE_BYTES:
        return extracted_dir

    archive_path = model_root / f"{spec['archive_subdir']}.tar.bz2"
    print(f"[sherpa] preparing '{model_name}' model...", flush=True)
    if not archive_path.exists():
        _download_with_progress(spec["url"], archive_path)
    print(f"[sherpa] extracting {archive_path.name}...", flush=True)
    with tarfile.open(archive_path, "r:bz2") as tar:
        tar.extractall(model_root)
    # Delete archive to reclaim disk.
    try:
        archive_path.unlink()
    except OSError:
        pass
    return extracted_dir


class SherpaTTS:
    def __init__(self, voices_dir: Path) -> None:
        self._voices_dir = voices_dir
        # model_name -> (OfflineTts, num_speakers)
        self._cache: dict[str, tuple] = {}
        self._load_lock = Lock()

    def _get_model(self, model_name: str):
        if model_name not in _MODELS:
            raise ValueError(
                f"Unknown sherpa model '{model_name}'. "
                f"Supported: {list(_MODELS)}"
            )
        if model_name in self._cache:
            return self._cache[model_name]
        with self._load_lock:
            if model_name in self._cache:
                return self._cache[model_name]
            model_dir = _ensure_model(self._voices_dir, model_name)
            spec = _MODELS[model_name]
            # Import lazily so the module loads even if sherpa_onnx isn't
            # installed until first use.
            import sherpa_onnx
            cfg = sherpa_onnx.OfflineTtsConfig(
                model=sherpa_onnx.OfflineTtsModelConfig(
                    vits=sherpa_onnx.OfflineTtsVitsModelConfig(
                        model=str(model_dir / spec["onnx"]),
                        lexicon=str(model_dir / spec["lexicon"]),
                        tokens=str(model_dir / spec["tokens"]),
                    ),
                    num_threads=2,
                ),
            )
            print(f"[sherpa] loading {model_name}...", flush=True)
            tts = sherpa_onnx.OfflineTts(cfg)
            print(f"[sherpa] {model_name} ready ({tts.num_speakers} speakers)", flush=True)
            self._cache[model_name] = (tts, tts.num_speakers)
            return self._cache[model_name]

    def synth(
        self, text: str, voice: str, speed: float = 1.0
    ) -> tuple[np.ndarray, int]:
        """Synthesize text with a Sherpa-ONNX voice.

        `voice` format: `<model>:<speaker_id>` e.g. `vctk:0`.
        `speed` is a time-stretch factor (1.0 = normal, >1 faster)."""
        if ":" not in voice:
            raise ValueError(
                f"sherpa voice must be '<model>:<speaker_id>', got '{voice}'"
            )
        model_name, speaker_str = voice.split(":", 1)
        try:
            speaker_id = int(speaker_str)
        except ValueError:
            raise ValueError(
                f"sherpa speaker id must be an integer, got '{speaker_str}'"
            )
        tts, num_speakers = self._get_model(model_name)
        if speaker_id < 0 or speaker_id >= num_speakers:
            raise ValueError(
                f"sherpa '{model_name}' has {num_speakers} speakers; "
                f"id {speaker_id} out of range"
            )
        audio_obj = tts.generate(text, sid=speaker_id, speed=float(speed))
        audio = np.asarray(audio_obj.samples, dtype=np.float32)
        return audio, int(audio_obj.sample_rate)
