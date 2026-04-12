"""
OCR wrapper around Windows native OCR (via winocr / WinRT).

Drop-in replacement for the RapidOCR version. Same interface:
    ocr = OCR()
    text = ocr.read(frame_ndarray)   # returns a single cleaned string

Uses the OCR engine built into Windows 10/11 — zero model downloads,
very fast, good accuracy on UI text. Requires the 'winocr' pip package.
"""

from __future__ import annotations

import asyncio
import re

import cv2
import numpy as np
import winocr
from PIL import Image


_WHITESPACE_RE = re.compile(r"\s+")

# Upscale small inputs for better recognition.
_MIN_OCR_HEIGHT = 80
_MIN_OCR_WIDTH = 400


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
        # Create a persistent event loop for the async winocr calls.
        self._loop = asyncio.new_event_loop()
        print("[ocr] Windows native OCR engine (WinRT)", flush=True)

    def read(self, frame: np.ndarray) -> str:
        frame = _upscale_for_ocr(frame)
        self.last_preprocessed = frame

        # winocr expects a PIL Image.
        if frame.ndim == 3 and frame.shape[2] == 3:
            pil_img = Image.fromarray(frame)
        elif frame.ndim == 2:
            pil_img = Image.fromarray(frame).convert("RGB")
        else:
            pil_img = Image.fromarray(frame[..., :3])

        result = self._loop.run_until_complete(
            winocr.recognize_pil(pil_img, self._lang)
        )

        lines_text: list[str] = []
        for line in result.lines:
            text = line.text.strip()
            if text:
                lines_text.append(text)

        if self.debug:
            for i, lt in enumerate(lines_text):
                try:
                    print(f"[ocr] line {i}: {lt!r}", flush=True)
                except UnicodeEncodeError:
                    print(f"[ocr] line {i}: {lt.encode('ascii', 'replace').decode()!r}", flush=True)

        joined = " ".join(lines_text)
        return _WHITESPACE_RE.sub(" ", joined).strip()
