"""Integration tests for outline editing in the picker overlay: synthetic
mouse events against a real (offscreen) _Overlay widget."""
import os
import sys
from types import SimpleNamespace

os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")

import pytest
from PySide6.QtCore import QPointF, Qt
from PySide6.QtWidgets import QApplication

from region_picker import _Overlay


@pytest.fixture(scope="module")
def app():
    return QApplication.instance() or QApplication(sys.argv)


def _ev(x, y, button=Qt.LeftButton):
    return SimpleNamespace(
        button=lambda: button,
        position=lambda: QPointF(x, y),
        modifiers=lambda: Qt.KeyboardModifier.NoModifier,
    )


def _outline(x=100, y=200, w=400, h=150, label="dialogue1", mode="dialogue"):
    return {
        "x": x, "y": y, "w": w, "h": h,
        "rotation": 0.0, "label": label, "mode": mode,
    }


def test_drag_edge_moves_outline(app):
    reg = _outline()
    ov = _Overlay(existing=[reg])
    ov.mousePressEvent(_ev(200, 200))        # on top edge line
    ov.mouseMoveEvent(_ev(230, 260))         # drag +30, +60
    ov.mouseReleaseEvent(_ev(230, 260))
    assert (reg["x"], reg["y"]) == (130, 260)
    assert (reg["w"], reg["h"]) == (400, 150)
    assert reg["edited"] is True
    # Editing must not have started a new-region drag or ended the session.
    assert not [o for o in ov._existing if o.get("created")]
    assert ov.done is False
    ov.close()


def test_drag_corner_resizes_outline(app):
    reg = _outline()
    ov = _Overlay(existing=[reg])
    ov.mousePressEvent(_ev(500, 350))        # se corner handle
    ov.mouseMoveEvent(_ev(560, 380))         # +60, +30
    ov.mouseReleaseEvent(_ev(560, 380))
    assert (reg["x"], reg["y"]) == (100, 200)
    assert (reg["w"], reg["h"]) == (460, 180)
    assert reg["edited"] is True
    ov.close()


def test_drag_side_handle_resizes_one_axis(app):
    reg = _outline()
    ov = _Overlay(existing=[reg])
    ov.mousePressEvent(_ev(500, 275))        # e handle
    ov.mouseMoveEvent(_ev(540, 320))         # +40 x, +45 y (y ignored)
    ov.mouseReleaseEvent(_ev(540, 320))
    assert (reg["w"], reg["h"]) == (440, 150)
    assert (reg["x"], reg["y"]) == (100, 200)
    ov.close()


def test_press_away_from_outline_still_starts_pick(app):
    reg = _outline()
    ov = _Overlay(existing=[reg])
    ov.mousePressEvent(_ev(800, 500))
    assert ov._origin is not None            # normal pick drag started
    assert "edited" not in reg
    ov.close()


def test_hover_sets_resize_cursor(app):
    reg = _outline()
    ov = _Overlay(existing=[reg])
    ov.mouseMoveEvent(_ev(500, 350))         # hover se handle, no button
    assert ov.cursor().shape() == Qt.SizeFDiagCursor
    ov.mouseMoveEvent(_ev(200, 200))         # hover edge line
    assert ov.cursor().shape() == Qt.SizeAllCursor
    ov.mouseMoveEvent(_ev(800, 500))         # hover empty space
    assert ov.cursor().shape() == Qt.CrossCursor
    ov.close()
