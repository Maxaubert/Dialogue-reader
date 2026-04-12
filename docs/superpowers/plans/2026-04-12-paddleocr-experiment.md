# PaddleOCR Experiment Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add PaddleOCR as the OCR engine for both speaker and dialogue text, replacing EasyOCR + WinOCR in an experimental branch.

**Architecture:** Add a `_paddleocr_read()` method to the existing `OCR` class in `ocr.py` that handles both speaker (largest-bbox selection) and dialogue (all lines) modes. Wire `read()` to call it for both cases. Existing methods stay untouched for easy rollback.

**Tech Stack:** PaddlePaddle, PaddleOCR, Python, NumPy, OpenCV

---

### Task 1: Install PaddleOCR dependencies

**Files:**
- Modify: `requirements.txt`

- [ ] **Step 1: Install paddlepaddle and paddleocr**

Run:
```bash
pip install paddlepaddle paddleocr
```

Expected: Both packages install successfully. PaddleOCR will download detection/recognition models on first use (~100-200 MB).

- [ ] **Step 2: Verify import works**

Run:
```bash
python -c "from paddleocr import PaddleOCR; print('PaddleOCR import OK')"
```

Expected: Prints `PaddleOCR import OK` (may show model download messages on first run).

- [ ] **Step 3: Add to requirements.txt**

Add `paddlepaddle` and `paddleocr` to `requirements.txt`:

```
mss>=10.0
Pillow>=10.0
numpy>=1.24
PySide6>=6.6
pyttsx3>=2.99
rapidocr-onnxruntime>=1.2
paddlepaddle>=3.0
paddleocr>=2.9
```

- [ ] **Step 4: Commit**

```bash
git add requirements.txt
git commit -m "deps: add paddlepaddle and paddleocr for OCR experiment"
```

---

### Task 2: Add PaddleOCR initialization to OCR class

**Files:**
- Modify: `ocr.py:1-47` (imports and `__init__`)

- [ ] **Step 1: Add PaddleOCR import**

Add to the imports section of `ocr.py` (after the existing imports):

```python
from paddleocr import PaddleOCR as _PaddleOCR
```

- [ ] **Step 2: Initialize PaddleOCR in `__init__`**

Replace the EasyOCR initialization in `__init__` with PaddleOCR. The full new `__init__` should be:

```python
def __init__(self, debug: bool = False) -> None:
    self.last_preprocessed: np.ndarray | None = None
    self.debug = debug
    self._lang = "en"
    self._loop = asyncio.new_event_loop()
    print("[ocr] Loading PaddleOCR...", flush=True)
    self._paddle = _PaddleOCR(use_angle_cls=True, lang="en", show_log=False)
    print("[ocr] PaddleOCR ready", flush=True)
```

Note: EasyOCR import (`import easyocr`) and `self._easy` are no longer initialized. Keep the `import easyocr` line commented out or removed — the `_easyocr_read` method body can stay as-is for future rollback, it just won't be called.

- [ ] **Step 3: Verify initialization**

Run:
```bash
python -c "from ocr import OCR; o = OCR(); print('init OK')"
```

Expected: Prints PaddleOCR loading messages then `init OK`. First run downloads models.

- [ ] **Step 4: Commit**

```bash
git add ocr.py
git commit -m "feat: initialize PaddleOCR engine in OCR class"
```

---

### Task 3: Implement `_paddleocr_read()` and wire up `read()`

**Files:**
- Modify: `ocr.py:49-112` (add method, modify `read()`)

- [ ] **Step 1: Add `_paddleocr_read()` method**

Add this method to the `OCR` class, between `_easyocr_read` and the `read` method:

```python
# ---- PaddleOCR (experimental, both roles) ----

def _paddleocr_read(self, frame: np.ndarray, speaker: bool = False) -> list[str]:
    # PaddleOCR expects BGR numpy array.
    if frame.ndim == 3 and frame.shape[2] == 3:
        bgr = cv2.cvtColor(frame, cv2.COLOR_RGB2BGR)
    else:
        bgr = frame
    result = self._paddle.ocr(bgr, cls=True)
    # result is a list of pages, each page is a list of
    # [bbox, (text, confidence)] or None if nothing detected.
    if not result or not result[0]:
        return []

    detections = result[0]

    if speaker:
        # Speaker mode: pick the largest bounding box by area,
        # same logic as _easyocr_read.
        best = None
        best_area = 0
        for det in detections:
            bbox, (text, conf) = det
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
    else:
        # Dialogue mode: return all detected lines.
        lines = []
        for det in detections:
            bbox, (text, conf) = det
            text = text.strip()
            if text and conf >= 0.3:
                lines.append(text)
        return lines
```

- [ ] **Step 2: Wire `read()` to use PaddleOCR for both modes**

Replace the `read` method body to call `_paddleocr_read` for both cases:

```python
def read(self, frame: np.ndarray, speaker: bool = False) -> str:
    frame = _upscale_for_ocr(frame)
    self.last_preprocessed = frame

    lines_text = self._paddleocr_read(frame, speaker=speaker)

    if self.debug:
        for i, lt in enumerate(lines_text):
            try:
                print(f"[ocr] line {i}: {lt!r}", flush=True)
            except UnicodeEncodeError:
                print(f"[ocr] line {i}: {lt.encode('ascii', 'replace').decode()!r}", flush=True)

    joined = " ".join(lines_text)
    return _WHITESPACE_RE.sub(" ", joined).strip()
```

- [ ] **Step 3: Update module docstring**

Replace the docstring at the top of `ocr.py`:

```python
"""
OCR wrapper — PaddleOCR experiment:
  - PaddleOCR for both dialogue text and speaker names
  - Original WinOCR + EasyOCR methods retained for rollback

Usage:
    ocr = OCR()
    text = ocr.read(frame)                    # dialogue: PaddleOCR
    name = ocr.read(frame, speaker=True)      # speaker: PaddleOCR
"""
```

- [ ] **Step 4: Smoke test — run the full app**

Run:
```bash
python main.py --debug
```

Pick a region and verify PaddleOCR produces output in the debug logs (`[ocr] line 0: ...`). Check both a dialogue region and a speaker region.

- [ ] **Step 5: Commit**

```bash
git add ocr.py
git commit -m "feat: add PaddleOCR engine for both speaker and dialogue OCR"
```
