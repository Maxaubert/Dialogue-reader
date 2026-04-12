# PaddleOCR Experiment — Design Spec

## Goal

Test PaddleOCR as an alternative OCR engine for both speaker name and dialogue text recognition. Compare accuracy against the current best combination (EasyOCR speakers + WinOCR dialogue).

## Scope

- Add `_paddleocr_read()` method to `ocr.py`
- Wire it up for **both** speaker and dialogue in `read()`
- Existing EasyOCR and WinOCR methods remain untouched in the file
- No changes to any other files (main.py, capture.py, tts.py, etc.)
- Swapping engines is done by editing which method `read()` calls

## Changes to `ocr.py`

### New method: `_paddleocr_read()`

```python
def _paddleocr_read(self, frame: np.ndarray, speaker: bool = False) -> list[str]:
```

**When `speaker=True`** (speaker name mode):
- Run PaddleOCR on the frame
- Apply the same largest-bounding-box selection logic as `_easyocr_read()`: pick the result with the largest bbox area, skip results with confidence < 0.3
- Return single-element list with the best result

**When `speaker=False`** (dialogue mode):
- Run PaddleOCR on the frame
- Return all detected text lines (same behavior as `_winocr_read()`)

### Modified `__init__`

- Import and initialize PaddleOCR reader alongside (or instead of) EasyOCR
- `PaddleOCR(use_angle_cls=True, lang='en', show_log=False)`

### Modified `read()`

- Both `speaker=True` and `speaker=False` call `_paddleocr_read(frame, speaker)`
- The public interface (`read(frame, speaker) -> str`) is unchanged

## Dependencies

- Add `paddlepaddle` and `paddleocr` to requirements
- PaddleOCR will download models on first run (~100-200 MB)

## What success looks like

Manual A/B testing: run the dialogue reader against the same game scenes with PaddleOCR vs the current EasyOCR+WinOCR combo and compare:
- Speaker name accuracy (fewer corrupted entries in speakers.json)
- Dialogue text accuracy (fewer misreads, better handling of game fonts)
- Latency (subjective — does it feel responsive?)
