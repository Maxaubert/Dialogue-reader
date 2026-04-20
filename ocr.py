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
import hashlib
import queue
import re
import threading
import time
from dataclasses import dataclass, field

import cv2
import numpy as np
from PIL import Image


_WHITESPACE_RE = re.compile(r"\s+")

_MIN_OCR_HEIGHT = 200
_MIN_OCR_WIDTH = 600

VALID_ENGINES = ("winocr", "easyocr")


def _hash_frame_fast(arr: np.ndarray) -> str:
    """Cheap perceptual-ish frame hash (downsample 8x, quantize, md5).
    Fast enough to run at poll rate on full-screen regions: ~1-2ms on a
    1248x1422 frame vs. ~800ms for an EasyOCR read of the same frame."""
    small = arr[::8, ::8]
    if small.ndim == 3 and small.shape[-1] == 4:
        small = small[..., :3]
    quantized = (small >> 3).astype(np.uint8)
    return hashlib.md5(quantized.tobytes()).hexdigest()


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
        # Dialogue role: preserve reading order. EasyOCR returns boxes in
        # a non-deterministic order and a single visual line can be split
        # across several boxes (e.g. wide word-spacing). Group boxes into
        # rows by vertical overlap, then sort each row left-to-right.
        boxes: list[tuple[float, float, float, str]] = []
        for bbox, text, conf in results:
            text = text.strip()
            if not text or conf < 0.3:
                continue
            ys = [p[1] for p in bbox]
            xs = [p[0] for p in bbox]
            boxes.append((min(ys), max(ys), min(xs), text))

        if not boxes:
            return []

        # Walk boxes in y_top order, placing each into the first existing
        # row whose vertical span overlaps >50% of the box's own height.
        # Otherwise start a new row. Handles ragged baselines (italics,
        # subscripts) and multiple columns of text within one region.
        boxes.sort(key=lambda b: b[0])
        rows: list[list[tuple[float, float, float, str]]] = []
        for box in boxes:
            y_top, y_bot, _, _ = box
            height = max(1.0, y_bot - y_top)
            placed = False
            for row in rows:
                row_top = min(b[0] for b in row)
                row_bot = max(b[1] for b in row)
                overlap = min(y_bot, row_bot) - max(y_top, row_top)
                if overlap > height * 0.5:
                    row.append(box)
                    placed = True
                    break
            if not placed:
                rows.append([box])

        rows.sort(key=lambda row: min(b[0] for b in row))
        out: list[str] = []
        for row in rows:
            row.sort(key=lambda b: b[2])  # x_left within row
            out.extend(b[3] for b in row)
        return out

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


# ---------------------------------------------------------------------------
# OCR worker thread
#
# Problem: OCR on large dialogue regions (especially EasyOCR) takes 500ms-2s.
# When OCR ran on the main loop, UDP commands (PICK_REGION, CYCLE_VOICE,
# PAUSE…) sat in the queue for the full OCR duration. Typing a hotkey mid-OCR
# felt unresponsive.
#
# Fix: run OCR + pre-snapshot sleeps + text-confirm loops on a worker thread.
# The main loop stays tight — it drains commands every ~83ms regardless of
# OCR state, polls regions for pixel changes, enqueues batch jobs, and
# applies results non-blockingly when they come back.
# ---------------------------------------------------------------------------


@dataclass
class OCRRegionSpec:
    """One region in an OCR batch job. `capture` is used by the worker to
    re-snapshot for the pre-OCR settle pause and for text-confirm polls."""
    name: str
    mode: str  # "dialogue" or "speaker"
    capture: object  # RegionCapture — worker calls .snapshot() on it


@dataclass
class OCRBatchJob:
    generation: int
    regions: list[OCRRegionSpec]
    confirm_polls: int
    debug: bool = False
    # Tunables — kept in the job so main.py can pass its constants without
    # the worker having to import them.
    pre_snapshot_delay: float = 0.15
    confirm_interval: float = 0.10
    confirm_max_multiplier: int = 4
    confirm_hard_cap: int = 30


@dataclass
class OCRBatchResult:
    generation: int
    texts: dict[str, str] = field(default_factory=dict)
    error: str | None = None


class OCRWorker:
    """Single-consumer worker thread that runs OCR batch jobs off the main
    loop. Thread model:

      - main thread  : submits OCRBatchJob, polls for OCRBatchResult
                       (non-blocking), handles commands every tick
      - worker thread: dequeues job, sleeps pre-snapshot delay, takes fresh
                       snapshots, runs OCR (and text-confirm loop for
                       dialogue regions), publishes result

    Stale results (generation mismatch) are simply ignored by the caller;
    the worker doesn't know about pause/clear. Main bumps its generation
    when state is invalidated.
    """

    def __init__(self, ocr: OCR) -> None:
        self._ocr = ocr
        self._jobs: queue.Queue[OCRBatchJob] = queue.Queue()
        self._results: queue.Queue[OCRBatchResult] = queue.Queue()
        # `_busy` is set by submit() and cleared by the worker after a
        # result has been published. Main checks it to decide whether to
        # enqueue another batch. We use a plain bool guarded by a lock
        # instead of Event because we need atomic "set if not already set"
        # — Event.set() is idempotent but gives no read-modify-write.
        self._busy = False
        self._busy_lock = threading.Lock()
        self._thread = threading.Thread(
            target=self._run, daemon=True, name="ocr-worker"
        )
        self._thread.start()

    def submit(self, job: OCRBatchJob) -> None:
        with self._busy_lock:
            self._busy = True
        self._jobs.put(job)

    def poll_result(self) -> OCRBatchResult | None:
        try:
            return self._results.get_nowait()
        except queue.Empty:
            return None

    @property
    def busy(self) -> bool:
        with self._busy_lock:
            return self._busy

    def _run(self) -> None:
        while True:
            job = self._jobs.get()
            try:
                result = self._process(job)
            except Exception as e:
                print(f"[ocr-worker] error: {e}", flush=True)
                result = OCRBatchResult(generation=job.generation, error=str(e))
            self._results.put(result)
            with self._busy_lock:
                self._busy = False

    def _process(self, job: OCRBatchJob) -> OCRBatchResult:
        # Pre-snapshot settle: catches typewriter mid-animation and lazy
        # renders. Matches the pre-existing 150ms delay from main.py.
        time.sleep(job.pre_snapshot_delay)

        texts: dict[str, str] = {}
        for r in job.regions:
            fresh = r.capture.snapshot()
            new_text = self._ocr.read(fresh, speaker=(r.mode == "speaker"))

            if r.mode == "dialogue" and job.confirm_polls > 1:
                new_text = self._confirm_dialogue_text(
                    capture=r.capture,
                    initial=new_text.strip(),
                    initial_hash=_hash_frame_fast(fresh),
                    polls=job.confirm_polls,
                    interval=job.confirm_interval,
                    max_multiplier=job.confirm_max_multiplier,
                    hard_cap=job.confirm_hard_cap,
                    region_name=r.name,
                    debug=job.debug,
                )

            texts[r.name] = new_text

        return OCRBatchResult(generation=job.generation, texts=texts)

    def _confirm_dialogue_text(
        self,
        capture: object,
        initial: str,
        initial_hash: str,
        polls: int,
        interval: float,
        max_multiplier: int,
        hard_cap: int,
        region_name: str,
        debug: bool,
    ) -> str:
        """Re-snapshot + re-OCR until we see `polls` consecutive identical
        results, or we hit the attempt cap. Prevents reading partial
        typewriter text.

        Optimization: hash the raw snapshot. If the hash matches the last
        frame we actually OCR'd, the pixels are identical and the OCR
        result is guaranteed identical too — skip the expensive OCR call
        and count it as a match. Turns the stable-text case (the common
        one) from N * OCR_cost into 1 * OCR_cost + (N-1) * hash_cost.
        """
        confirmed = initial
        matches = 1
        max_attempts = min(polls * max_multiplier, hard_cap)
        attempts = 0
        last_ocr_hash: str | None = initial_hash
        ocr_calls = 1  # the initial read in _process() counts
        skipped = 0
        while matches < polls and attempts < max_attempts:
            attempts += 1
            time.sleep(interval)
            fresh = capture.snapshot()
            fresh_hash = _hash_frame_fast(fresh)
            if last_ocr_hash is not None and fresh_hash == last_ocr_hash:
                # Pixels identical to the frame we last OCR'd — OCR would
                # return the same text. Count as a match without the call.
                matches += 1
                skipped += 1
                continue
            fresh_text = self._ocr.read(fresh, speaker=False).strip()
            ocr_calls += 1
            last_ocr_hash = fresh_hash
            if fresh_text == confirmed:
                matches += 1
            else:
                confirmed = fresh_text
                matches = 1
        if debug:
            print(
                f"[ocr-worker {region_name}] text-confirm: "
                f"{matches}/{polls} matches in {attempts} attempts "
                f"(OCR calls: {ocr_calls}, hash-skipped: {skipped})",
                flush=True,
            )
        return confirmed
