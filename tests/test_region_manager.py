"""Tests for the persistent region manager: multi-add sessions, right-click
delete, F1/Esc toggle-out, and applying the session result in main."""
import os
import sys
from types import SimpleNamespace

os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")

import pytest
from PySide6.QtCore import QPointF, Qt
from PySide6.QtWidgets import QApplication

import region_picker
from region_picker import _Overlay, _next_label

import main as main_mod
from main import WatchedRegion, open_region_manager


@pytest.fixture(scope="module")
def app():
    return QApplication.instance() or QApplication(sys.argv)


def _ev(x, y, button=Qt.LeftButton):
    return SimpleNamespace(
        button=lambda: button,
        position=lambda: QPointF(x, y),
        modifiers=lambda: Qt.KeyboardModifier.NoModifier,
    )


def _key(key):
    return SimpleNamespace(key=lambda: key)


def _outline(x=100, y=200, w=400, h=150, label="dialogue1", mode="dialogue"):
    return {
        "x": x, "y": y, "w": w, "h": h,
        "rotation": 0.0, "label": label, "mode": mode,
    }


# ---- _next_label ----------------------------------------------------------

def test_next_label_first():
    assert _next_label([], "dialogue") == "dialogue1"


def test_next_label_appends():
    assert _next_label(["dialogue1", "dialogue2"], "dialogue") == "dialogue3"


def test_next_label_fills_gap():
    # dialogue1 was deleted earlier — its name must be reused, never
    # colliding with the surviving dialogue2.
    assert _next_label(["dialogue2"], "dialogue") == "dialogue1"


def test_next_label_per_mode():
    assert _next_label(["dialogue1"], "speaker") == "speaker1"


# ---- manager session: staying open, multi-add -----------------------------

def test_completed_drag_adds_region_and_stays_open(app, monkeypatch):
    monkeypatch.setattr(region_picker, "find_window_at", lambda x, y: 42)
    ov = _Overlay(existing=[_outline()], mode="dialogue")
    ov.mousePressEvent(_ev(800, 500))
    ov.mouseMoveEvent(_ev(950, 600))
    ov.mouseReleaseEvent(_ev(950, 600))
    assert ov.done is False                      # session still running
    created = [o for o in ov._existing if o.get("created")]
    assert len(created) == 1
    reg = created[0]
    # QRect(p1, p2) is endpoint-inclusive, so a 150x100 drag reports
    # 151x101 — same behavior as the original single-shot picker.
    assert (reg["x"], reg["y"], reg["w"], reg["h"]) == (800, 500, 151, 101)
    assert reg["label"] == "dialogue2"
    assert reg["mode"] == "dialogue"
    assert reg["hwnd"] == 42
    assert ov._origin is None                    # ready for the next drag
    ov.close()


def test_speaker_session_labels_speaker(app, monkeypatch):
    monkeypatch.setattr(region_picker, "find_window_at", lambda x, y: 0)
    ov = _Overlay(existing=[_outline()], mode="speaker")
    ov.mousePressEvent(_ev(800, 500))
    ov.mouseMoveEvent(_ev(900, 560))
    ov.mouseReleaseEvent(_ev(900, 560))
    created = [o for o in ov._existing if o.get("created")]
    assert created[0]["label"] == "speaker1"
    assert created[0]["mode"] == "speaker"
    ov.close()


# ---- right-click delete ---------------------------------------------------

def test_right_click_deletes_existing_outline(app):
    reg = _outline()
    ov = _Overlay(existing=[reg])
    ov.mousePressEvent(_ev(300, 275, button=Qt.RightButton))  # interior
    assert reg not in ov._existing
    assert ov.deleted == ["dialogue1"]
    assert ov.done is False
    ov.close()


def test_right_click_on_created_region_forgets_it(app, monkeypatch):
    monkeypatch.setattr(region_picker, "find_window_at", lambda x, y: 0)
    ov = _Overlay(existing=[], mode="dialogue")
    ov.mousePressEvent(_ev(800, 500))
    ov.mouseMoveEvent(_ev(950, 600))
    ov.mouseReleaseEvent(_ev(950, 600))
    assert len(ov._existing) == 1
    ov.mousePressEvent(_ev(870, 550, button=Qt.RightButton))
    assert ov._existing == []
    assert ov.deleted == []                      # main never knew about it
    ov.close()


def test_right_click_empty_space_does_nothing(app):
    reg = _outline()
    ov = _Overlay(existing=[reg])
    ov.mousePressEvent(_ev(900, 600, button=Qt.RightButton))
    assert reg in ov._existing
    assert ov.deleted == []
    ov.close()


# ---- Esc / F1 exit semantics ----------------------------------------------

def test_esc_cancels_drag_first_then_closes(app):
    ov = _Overlay(existing=[])
    ov.mousePressEvent(_ev(800, 500))
    ov.keyPressEvent(_key(Qt.Key_Escape))
    assert ov._origin is None
    assert ov.done is False                      # drag cancelled, still open
    ov.keyPressEvent(_key(Qt.Key_Escape))
    assert ov.done is True


def test_pick_command_closes_session_others_kept(app):
    pending = [["SPEED_UP"], ["PICK_REGION", "PAUSE"]]
    ov = _Overlay(existing=[], poll_commands=lambda: pending.pop(0))
    ov._process_commands()
    assert ov.done is False
    assert ov.unhandled == ["SPEED_UP"]
    ov._process_commands()
    assert ov.done is True
    assert ov.unhandled == ["SPEED_UP", "PAUSE"]


# ---- main.open_region_manager ---------------------------------------------

class _FakeCapture:
    def __init__(self, region, hwnd=0, poll_hz=12.0, stable_ms=350,
                 verbose=False, rotation=0.0, capture_mode="auto"):
        self.region = region
        self.hwnd = hwnd
        self.rotation = rotation
        self.capture_mode = capture_mode
        self.bbox = {"left": region[0], "top": region[1],
                     "width": region[2], "height": region[3]}
        self.target_w = region[2]
        self.target_h = region[3]
        self.rel_x = self.rel_y = 0
        self.use_window_mode = False
        self.game_mode = False


def _watched(name="dialogue1", mode="dialogue"):
    cap = _FakeCapture((100, 200, 400, 150))
    return WatchedRegion(name=name, capture=cap, mode=mode)


def test_manager_result_applied(monkeypatch):
    monkeypatch.setattr(main_mod, "RegionCapture", _FakeCapture)
    monkeypatch.setattr(main_mod, "find_window_at", lambda x, y: 7)
    r1 = _watched("dialogue1")
    r2 = _watched("dialogue2")
    regions = [r1, r2]

    def fake_manage(outlines, mode="dialogue", poll_commands=None):
        # dialogue1 deleted; dialogue2 moved; one new region created.
        survivors = [o for o in outlines if o["label"] == "dialogue2"]
        survivors[0]["x"] = 111
        survivors[0]["edited"] = True
        survivors.append({
            "x": 5, "y": 6, "w": 300, "h": 90, "rotation": 0.0,
            "label": "dialogue1", "mode": "dialogue",
            "created": True, "hwnd": 99,
        })
        return SimpleNamespace(
            outlines=survivors, deleted=["dialogue1"], unhandled=["PAUSE"],
        )

    monkeypatch.setattr(main_mod, "manage_regions", fake_manage)
    state = {"generation": 0, "capture_mode": "screen", "last_spoken": "x",
             "candidate": "", "speaker_candidate": ""}
    unhandled = open_region_manager(
        regions, debug=False, mode="dialogue", state=state
    )
    names = [r.name for r in regions]
    assert names == ["dialogue2", "dialogue1"]   # deleted, then re-created
    assert regions[0].capture.region[0] == 111   # edit applied (rebuilt)
    assert regions[1].capture.region == (5, 6, 300, 90)
    assert regions[1].capture.hwnd == 99
    assert regions[1].capture.capture_mode == "screen"
    assert unhandled == ["PAUSE"]
    assert state["generation"] >= 1
