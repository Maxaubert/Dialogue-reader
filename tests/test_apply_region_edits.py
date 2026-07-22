"""Tests for _apply_region_edits — pushing picker outline edits back into
the running WatchedRegion captures."""
from types import SimpleNamespace

import main
from main import WatchedRegion, _apply_region_edits


class _FakeCapture:
    """Stands in for RegionCapture; records constructor args."""
    def __init__(self, region, hwnd=0, poll_hz=12.0, stable_ms=350,
                 verbose=False, rotation=0.0, capture_mode="auto"):
        self.region = region
        self.hwnd = hwnd
        self.rotation = rotation
        self.capture_mode = capture_mode


def _watched(name="dialogue1", mode="dialogue"):
    cap = SimpleNamespace(bbox={"left": 0, "top": 0, "width": 1, "height": 1})
    r = WatchedRegion(name=name, capture=cap, mode=mode)
    r.last_text = "old text"
    r.last_spoken_text = "old text"
    r.has_pending_frame = True
    return r


def test_edited_outline_rebuilds_capture(monkeypatch):
    monkeypatch.setattr(main, "RegionCapture", _FakeCapture)
    monkeypatch.setattr(main, "find_window_at", lambda x, y: 77)
    r = _watched()
    old_cap = r.capture
    outlines = [{
        "x": 130, "y": 260, "w": 400, "h": 150, "rotation": 0.0,
        "label": "dialogue1", "mode": "dialogue", "edited": True,
    }]
    state = {"generation": 3, "capture_mode": "auto"}
    _apply_region_edits([r], outlines, state)
    assert r.capture is not old_cap
    assert r.capture.region == (130, 260, 400, 150)
    assert r.capture.hwnd == 77
    assert r.capture.capture_mode == "auto"
    # Text state resets: the region now shows different content.
    assert r.last_text == ""
    assert r.last_spoken_text == ""
    assert r.has_pending_frame is False
    # In-flight OCR against the old capture is invalidated.
    assert state["generation"] == 4


def test_unedited_outline_left_alone(monkeypatch):
    monkeypatch.setattr(main, "RegionCapture", _FakeCapture)
    monkeypatch.setattr(main, "find_window_at", lambda x, y: 77)
    r = _watched()
    old_cap = r.capture
    outlines = [{
        "x": 100, "y": 200, "w": 400, "h": 150, "rotation": 0.0,
        "label": "dialogue1", "mode": "dialogue",
    }]
    state = {"generation": 3, "capture_mode": "auto"}
    _apply_region_edits([r], outlines, state)
    assert r.capture is old_cap
    assert r.last_text == "old text"
    assert state["generation"] == 3


def test_edit_matches_region_by_label(monkeypatch):
    monkeypatch.setattr(main, "RegionCapture", _FakeCapture)
    monkeypatch.setattr(main, "find_window_at", lambda x, y: 0)
    r1 = _watched("dialogue1")
    r2 = _watched("speaker1", mode="speaker")
    old1 = r1.capture
    outlines = [
        {"x": 100, "y": 200, "w": 400, "h": 150, "rotation": 0.0,
         "label": "dialogue1", "mode": "dialogue"},
        {"x": 5, "y": 6, "w": 200, "h": 40, "rotation": 0.0,
         "label": "speaker1", "mode": "speaker", "edited": True},
    ]
    state = {"generation": 0, "capture_mode": "screen"}
    _apply_region_edits([r1, r2], outlines, state)
    assert r1.capture is old1
    assert r2.capture.region == (5, 6, 200, 40)
    assert r2.capture.capture_mode == "screen"
