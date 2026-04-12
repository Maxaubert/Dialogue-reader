"""
OCR wrapper around RapidOCR (ONNX runtime, no external installs).

Usage:
    ocr = OCR()
    text = ocr.read(frame_ndarray)   # returns a single cleaned string
"""

from __future__ import annotations

import re

import cv2
import numpy as np
from rapidocr_onnxruntime import RapidOCR


_WHITESPACE_RE = re.compile(r"\s+")

# RapidOCR's text detector model has a min input dimension; small inputs
# silently return None. Upscale anything smaller than this before OCR.
_MIN_OCR_HEIGHT = 80
_MIN_OCR_WIDTH = 400

# Anything wider than this aspect ratio gets padded vertically with white
# space. This makes wide dialogue strips ~9x faster to OCR AND more accurate
# (the detection model is trained on document-shaped images, not 22:1 strips).
_TARGET_ASPECT = 2.0


def _upscale_for_ocr(frame: np.ndarray) -> np.ndarray:
    h, w = frame.shape[:2]
    scale = max(_MIN_OCR_HEIGHT / h, _MIN_OCR_WIDTH / w, 1.0)
    if scale <= 1.0:
        return frame
    new_w, new_h = int(w * scale), int(h * scale)
    return cv2.resize(frame, (new_w, new_h), interpolation=cv2.INTER_CUBIC)


def _pad_for_aspect(frame: np.ndarray) -> np.ndarray:
    """Pad with white above/below if the aspect is wider than target."""
    h, w = frame.shape[:2]
    if w / h <= _TARGET_ASPECT:
        return frame
    target_h = int(w / _TARGET_ASPECT)
    pad_total = target_h - h
    pad_top = pad_total // 2
    pad_bot = pad_total - pad_top
    return cv2.copyMakeBorder(
        frame, pad_top, pad_bot, 0, 0,
        cv2.BORDER_CONSTANT, value=(255, 255, 255),
    )


class OCR:
    def __init__(self, debug: bool = False) -> None:
        # CRITICAL: width_height_ratio=-1 disables RapidOCR's "wide image"
        # bypass. By default, if image w/h > 8, RapidOCR skips text detection
        # entirely and feeds the whole strip into the recognizer as one line,
        # producing garbage like "TheDADLEST..." for normal multi-word text.
        # Dialogue boxes are often very wide and short — exactly that case.
        self._engine = RapidOCR(width_height_ratio=-1)
        self.last_preprocessed: np.ndarray | None = None
        self.debug = debug

    def read(self, frame: np.ndarray) -> str:
        frame = _upscale_for_ocr(frame)
        frame = _pad_for_aspect(frame)
        self.last_preprocessed = frame
        result, _elapsed = self._engine(frame)
        if not result:
            return ""

        # Each item: (center_y, height, left_x, text)
        items = []
        for box, text, _conf in result:
            if not text:
                continue
            ys = [pt[1] for pt in box]
            xs = [pt[0] for pt in box]
            center_y = (min(ys) + max(ys)) / 2
            height = max(ys) - min(ys)
            left_x = min(xs)
            items.append((center_y, height, left_x, text))

        if not items:
            return ""

        if self.debug:
            print("[ocr] raw boxes:", flush=True)
            for cy, h, lx, t in sorted(items, key=lambda it: it[0]):
                try:
                    print(f"  cy={cy:6.1f} h={h:4.1f} lx={lx:6.1f} text={t!r}", flush=True)
                except UnicodeEncodeError:
                    print(f"  cy={cy:6.1f} h={h:4.1f} lx={lx:6.1f} text={t.encode('ascii', 'replace').decode()!r}", flush=True)

        # Cluster boxes into lines using GAP DETECTION instead of fixed buckets.
        # Sort by center_y, then walk through; a new line starts only when the
        # next center_y is more than half the median text height away from the
        # previous one. This is robust to small y wobble within a line and
        # tight inter-line spacing.
        items.sort(key=lambda it: it[0])

        heights = sorted(h for _, h, _, _ in items)
        median_h = heights[len(heights) // 2] or 12
        line_threshold = max(8.0, median_h * 0.6)

        lines: list[list] = []
        for it in items:
            if not lines:
                lines.append([it])
                continue
            prev_cy = lines[-1][-1][0]
            if it[0] - prev_cy > line_threshold:
                lines.append([it])
            else:
                lines[-1].append(it)

        # Sort each line left-to-right by x.
        for line in lines:
            line.sort(key=lambda it: it[2])

        if self.debug:
            print(f"[ocr] grouped into {len(lines)} line(s) "
                  f"(median_h={median_h:.0f}, threshold={line_threshold:.0f})",
                  flush=True)
            for i, line in enumerate(lines):
                joined = " ".join(it[3] for it in line)
                print(f"  line {i}: {joined!r}", flush=True)

        text = " ".join(it[3] for line in lines for it in line)
        return _WHITESPACE_RE.sub(" ", text).strip()
