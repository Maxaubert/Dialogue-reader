"""
OCR wrapper — dual engine:
  - Windows native OCR (WinRT/winocr) for dialogue text (fast)
  - EasyOCR for speaker names (better on stylized game fonts)

Usage:
    ocr = OCR()
    text = ocr.read(frame)                    # dialogue: WinOCR
    name = ocr.read(frame, speaker=True)      # speaker: EasyOCR
"""

from __future__ import annotations

import asyncio
import re

import cv2
import numpy as np
import easyocr
import winocr
from PIL import Image


_WHITESPACE_RE = re.compile(r"\s+")

_MIN_OCR_HEIGHT = 200
_MIN_OCR_WIDTH = 600


def _upscale_for_ocr(frame: np.ndarray) -> np.ndarray:
    h, w = frame.shape[:2]
    scale = max(_MIN_OCR_HEIGHT / h, _MIN_OCR_WIDTH / w, 1.0)
    if scale <= 1.0:
        return frame
    new_w, new_h = int(w * scale), int(h * scale)
    return cv2.resize(frame, (new_w, new_h), interpolation=cv2.INTER_CUBIC)


class OCR:
    def __init__(self, debug: bool = False) -> None:
        self.last_preprocessed: np.ndarray | None = None
        self.debug = debug
        self._lang = "en"
        self._loop = asyncio.new_event_loop()
        print("[ocr] Loading EasyOCR for speaker names...", flush=True)
        self._easy = easyocr.Reader(["en"], gpu=False, verbose=False)
        print("[ocr] Windows OCR (dialogue) + EasyOCR (speakers) ready", flush=True)

    # ---- WinOCR (fast, for dialogue) ----

    def _winocr_read(self, frame: np.ndarray) -> list[str]:
        if frame.ndim == 3 and frame.shape[2] == 3:
            pil_img = Image.fromarray(frame)
        elif frame.ndim == 2:
            pil_img = Image.fromarray(frame).convert("RGB")
        else:
            pil_img = Image.fromarray(frame[..., :3])
        result = self._loop.run_until_complete(
            winocr.recognize_pil(pil_img, self._lang)
        )
        return [line.text.strip() for line in result.lines if line.text.strip()]

    # ---- EasyOCR (accurate, for speaker names) ----

    def _easyocr_read(self, frame: np.ndarray) -> list[str]:
        # EasyOCR takes BGR numpy arrays directly.
        if frame.ndim == 3 and frame.shape[2] == 3:
            bgr = cv2.cvtColor(frame, cv2.COLOR_RGB2BGR)
        else:
            bgr = frame
        results = self._easy.readtext(bgr)
        if not results:
            return []
        # For speaker names: pick only the result with the largest bounding
        # box (by area). The name banner text is bigger than any dialogue
        # text bleeding in from the edges of a large region.
        best = None
        best_area = 0
        for bbox, text, conf in results:
            text = text.strip()
            if not text or conf < 0.3:
                continue
            # bbox is [[x0,y0],[x1,y1],[x2,y2],[x3,y3]]
            xs = [p[0] for p in bbox]
            ys = [p[1] for p in bbox]
            area = (max(xs) - min(xs)) * (max(ys) - min(ys))
            if area > best_area:
                best_area = area
                best = text
        return [best] if best else []

    # ---- public API ----

    def read(self, frame: np.ndarray, speaker: bool = False) -> str:
        frame = _upscale_for_ocr(frame)
        self.last_preprocessed = frame

        if speaker:
            lines_text = self._easyocr_read(frame)
        else:
            lines_text = self._winocr_read(frame)

        if self.debug:
            for i, lt in enumerate(lines_text):
                try:
                    print(f"[ocr] line {i}: {lt!r}", flush=True)
                except UnicodeEncodeError:
                    print(f"[ocr] line {i}: {lt.encode('ascii', 'replace').decode()!r}", flush=True)

        joined = " ".join(lines_text)
        return _WHITESPACE_RE.sub(" ", joined).strip()
