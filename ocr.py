"""
OCR wrapper — pluggable engines per role.

Two engines are available:
  - winocr  : Windows native OCR (WinRT). Fast, great on clean UI text.
  - easyocr : EasyOCR (CPU). Slower, more accurate on stylized game fonts.

Either engine can be assigned to either role (dialogue vs. speaker) via
dialogue_reader.ini's [OCR] section. Only the engines actually used are
loaded — if you set both roles to winocr, EasyOCR never initializes.

Usage:
    ocr = OCR(dialogue_engine="winocr", speaker_engine="easyocr")
    text = ocr.read(frame)                    # dialogue role
    name = ocr.read(frame, speaker=True)      # speaker role
"""

from __future__ import annotations

import asyncio
import re

import cv2
import numpy as np
from PIL import Image


_WHITESPACE_RE = re.compile(r"\s+")

_MIN_OCR_HEIGHT = 200
_MIN_OCR_WIDTH = 600

VALID_ENGINES = ("winocr", "easyocr")


def _upscale_for_ocr(frame: np.ndarray) -> np.ndarray:
    h, w = frame.shape[:2]
    scale = max(_MIN_OCR_HEIGHT / h, _MIN_OCR_WIDTH / w, 1.0)
    if scale <= 1.0:
        return frame
    new_w, new_h = int(w * scale), int(h * scale)
    return cv2.resize(frame, (new_w, new_h), interpolation=cv2.INTER_CUBIC)


class OCR:
    def __init__(
        self,
        debug: bool = False,
        dialogue_engine: str = "winocr",
        speaker_engine: str = "easyocr",
    ) -> None:
        dialogue_engine = dialogue_engine.strip().lower()
        speaker_engine = speaker_engine.strip().lower()
        for role, eng in (("dialogue", dialogue_engine), ("speaker", speaker_engine)):
            if eng not in VALID_ENGINES:
                raise ValueError(
                    f"Unknown OCR engine for {role}: {eng!r}. "
                    f"Valid: {VALID_ENGINES}"
                )
        self._dialogue_engine = dialogue_engine
        self._speaker_engine = speaker_engine

        self.last_preprocessed: np.ndarray | None = None
        self.debug = debug
        self._lang = "en"
        self._loop: asyncio.AbstractEventLoop | None = None
        self._easy = None  # lazy

        engines_needed = {dialogue_engine, speaker_engine}
        if "winocr" in engines_needed:
            self._init_winocr()
        if "easyocr" in engines_needed:
            self._init_easyocr()

        print(
            f"[ocr] ready — dialogue={dialogue_engine} speaker={speaker_engine}",
            flush=True,
        )

    # ---- engine init (only called if needed) ----

    def _init_winocr(self) -> None:
        import winocr  # noqa: F401 (imported for side effect / availability check)
        self._winocr_module = winocr
        self._loop = asyncio.new_event_loop()

    def _init_easyocr(self) -> None:
        import easyocr
        print("[ocr] Loading EasyOCR...", flush=True)
        self._easy = easyocr.Reader(["en"], gpu=False, verbose=False)

    # ---- WinOCR ----

    def _winocr_read(self, frame: np.ndarray) -> list[str]:
        if frame.ndim == 3 and frame.shape[2] == 3:
            pil_img = Image.fromarray(frame)
        elif frame.ndim == 2:
            pil_img = Image.fromarray(frame).convert("RGB")
        else:
            pil_img = Image.fromarray(frame[..., :3])
        result = self._loop.run_until_complete(
            self._winocr_module.recognize_pil(pil_img, self._lang)
        )
        return [line.text.strip() for line in result.lines if line.text.strip()]

    # ---- EasyOCR ----

    def _easyocr_read(self, frame: np.ndarray, speaker: bool) -> list[str]:
        # EasyOCR takes BGR numpy arrays directly.
        if frame.ndim == 3 and frame.shape[2] == 3:
            bgr = cv2.cvtColor(frame, cv2.COLOR_RGB2BGR)
        else:
            bgr = frame
        results = self._easy.readtext(bgr)
        if not results:
            return []
        if speaker:
            # Speaker role: pick only the result with the largest bounding
            # box. The name banner text is bigger than any dialogue text
            # bleeding in from the edges of a large region.
            best = None
            best_area = 0
            for bbox, text, conf in results:
                text = text.strip()
                if not text or conf < 0.3:
                    continue
                xs = [p[0] for p in bbox]
                ys = [p[1] for p in bbox]
                area = (max(xs) - min(xs)) * (max(ys) - min(ys))
                if area > best_area:
                    best_area = area
                    best = text
            return [best] if best else []
        # Dialogue role: keep every confident line, ordered top-to-bottom.
        kept: list[tuple[float, str]] = []
        for bbox, text, conf in results:
            text = text.strip()
            if not text or conf < 0.3:
                continue
            y_top = min(p[1] for p in bbox)
            kept.append((y_top, text))
        kept.sort(key=lambda t: t[0])
        return [t for _, t in kept]

    # ---- dispatch ----

    def _read_with(self, engine: str, frame: np.ndarray, speaker: bool) -> list[str]:
        if engine == "winocr":
            return self._winocr_read(frame)
        if engine == "easyocr":
            return self._easyocr_read(frame, speaker=speaker)
        raise ValueError(f"Unknown OCR engine: {engine!r}")

    # ---- public API ----

    def read(self, frame: np.ndarray, speaker: bool = False) -> str:
        frame = _upscale_for_ocr(frame)
        self.last_preprocessed = frame

        engine = self._speaker_engine if speaker else self._dialogue_engine
        lines_text = self._read_with(engine, frame, speaker=speaker)

        if self.debug:
            for i, lt in enumerate(lines_text):
                print(f"[ocr] line {i}: {lt!r}", flush=True)

        joined = " ".join(lines_text)
        return _WHITESPACE_RE.sub(" ", joined).strip()
