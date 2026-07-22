"""
Fullscreen transparent overlay for picking a screen region.

Usage:
    from region_picker import pick_region
    region, hwnd, rotation = pick_region()
    # region   == (x, y, w, h) in physical screen pixels, or None if cancelled.
    # hwnd     == the underlying top-level window's HWND at the time of
    #             release, or 0 if no window was detected. Captured INSIDE
    #             the picker after the overlay is hidden so WindowFromPoint
    #             sees the real window underneath, not the overlay itself.
    # rotation == clockwise rotation in degrees applied during the drag
    #             via the mouse wheel. 0 if the user didn't scroll.

While the user is dragging:
    Mouse wheel       rotate ±2° per notch
    Shift + wheel     rotate ±10° per notch
    0 (zero key)      reset rotation to 0
    Esc               cancel the pick entirely
"""

import ctypes
import math
import sys
import time

from PySide6.QtCore import Qt, QRect, QPoint
from PySide6.QtGui import QPainter, QColor, QPen, QGuiApplication, QFont
from PySide6.QtWidgets import QApplication, QWidget

from window_capture import find_window_at


_user32 = ctypes.windll.user32
_SW_HIDE = 0
_SWP_NOSIZE = 0x0001
_SWP_NOMOVE = 0x0002
_SWP_NOACTIVATE = 0x0010
_SWP_HIDEWINDOW = 0x0080


def _force_hide(hwnd: int) -> None:
    """Hide a window via raw Win32, in addition to whatever Qt has done.
    Qt's hide() is sometimes deferred and the window remains in the
    z-order long enough that WindowFromPoint still returns it."""
    try:
        _user32.ShowWindow(hwnd, _SW_HIDE)
        _user32.SetWindowPos(
            hwnd, 0, 0, 0, 0, 0,
            _SWP_NOSIZE | _SWP_NOMOVE | _SWP_NOACTIVATE | _SWP_HIDEWINDOW,
        )
    except Exception:
        pass


# Outline colors for already-watched regions shown during a pick.
_DIALOGUE_OUTLINE = QColor(120, 220, 140)
_SPEAKER_OUTLINE = QColor(255, 170, 60)

# ---- outline editing (Word/Docs-style image handles) ----------------------
#
# Each existing-region outline carries 8 handles: corners resize both axes,
# side midpoints resize one axis. Grabbing the dashed line itself (away from
# any handle) moves the whole region. All math runs in the rect's LOCAL
# (unrotated) space so rotated outlines edit correctly too.

# Handle name -> local direction sign (sx, sy). (-1,-1) = top-left corner.
_HANDLES = {
    "nw": (-1, -1), "n": (0, -1), "ne": (1, -1),
    "e": (1, 0), "se": (1, 1), "s": (0, 1),
    "sw": (-1, 1), "w": (-1, 0),
}

_HANDLE_CURSORS = {
    "move": Qt.SizeAllCursor,
    "n": Qt.SizeVerCursor, "s": Qt.SizeVerCursor,
    "e": Qt.SizeHorCursor, "w": Qt.SizeHorCursor,
    "nw": Qt.SizeFDiagCursor, "se": Qt.SizeFDiagCursor,
    "ne": Qt.SizeBDiagCursor, "sw": Qt.SizeBDiagCursor,
}

_HANDLE_HIT_R = 8.0   # px: click-radius of a handle dot
_HANDLE_DRAW_R = 4    # px: drawn dot radius
_EDGE_HIT_R = 6.0     # px: half-thickness of the draggable edge band
_MIN_EDIT_SIZE = 20.0  # px: smallest w/h a resize can reach


def _to_local(rect, rotation_deg, pos):
    """Map a screen point into the rect's local space: origin at the rect
    center, axes unrotated. Returns (lx, ly)."""
    x, y, w, h = rect
    a = math.radians(rotation_deg)
    dx = pos[0] - (x + w / 2.0)
    dy = pos[1] - (y + h / 2.0)
    return (
        dx * math.cos(a) + dy * math.sin(a),
        -dx * math.sin(a) + dy * math.cos(a),
    )


def _hit_test(rect, rotation_deg, pos):
    """What does `pos` grab on this outline? Returns a handle name from
    _HANDLES, "move" for the edge line between handles, or None."""
    lx, ly = _to_local(rect, rotation_deg, pos)
    hw, hh = rect[2] / 2.0, rect[3] / 2.0
    for name, (sx, sy) in _HANDLES.items():
        px, py = sx * hw, sy * hh
        if (lx - px) ** 2 + (ly - py) ** 2 <= _HANDLE_HIT_R ** 2:
            return name
    on_vertical = (
        abs(abs(lx) - hw) <= _EDGE_HIT_R and abs(ly) <= hh + _EDGE_HIT_R
    )
    on_horizontal = (
        abs(abs(ly) - hh) <= _EDGE_HIT_R and abs(lx) <= hw + _EDGE_HIT_R
    )
    if on_vertical or on_horizontal:
        return "move"
    return None


def _apply_edit(rect, rotation_deg, kind, dx, dy):
    """Apply a drag of (dx, dy) screen pixels to `rect` (x, y, w, h).

    kind "move" translates; a handle name resizes along its local axes with
    the opposite edge/corner anchored — the standard image-handle behavior.
    """
    x, y, w, h = rect
    if kind == "move":
        return (x + dx, y + dy, w, h)

    a = math.radians(rotation_deg)
    cos_a, sin_a = math.cos(a), math.sin(a)
    # Drag delta in local space.
    ldx = dx * cos_a + dy * sin_a
    ldy = -dx * sin_a + dy * cos_a
    sx, sy = _HANDLES[kind]
    new_w = max(_MIN_EDIT_SIZE, w + sx * ldx) if sx else w
    new_h = max(_MIN_EDIT_SIZE, h + sy * ldy) if sy else h
    # The center shifts by half the (clamped) growth along the handle's
    # local direction — that's what anchors the opposite edge.
    lcx = sx * (new_w - w) / 2.0
    lcy = sy * (new_h - h) / 2.0
    cx = x + w / 2.0 + (lcx * cos_a - lcy * sin_a)
    cy = y + h / 2.0 + (lcx * sin_a + lcy * cos_a)
    return (cx - new_w / 2.0, cy - new_h / 2.0, new_w, new_h)


class _Overlay(QWidget):
    def __init__(self, existing: list[dict] | None = None):
        super().__init__()
        # Already-watched regions, drawn as labeled outlines so the user
        # can see what's being watched while picking a new region. Each is
        # {x, y, w, h, rotation, label, mode} in physical screen pixels.
        self._existing = existing or []
        # No Qt.Tool flag — Tool windows do not trigger lastWindowClosed,
        # which causes app.exec() to hang forever after the user makes a pick.
        self.setWindowFlags(
            Qt.FramelessWindowHint
            | Qt.WindowStaysOnTopHint
        )
        self.setAttribute(Qt.WA_TranslucentBackground)
        self.setAttribute(Qt.WA_DeleteOnClose)
        self.setCursor(Qt.CrossCursor)
        # Hover feedback (move/resize cursors over outlines) needs move
        # events even when no button is down.
        self.setMouseTracking(True)

        # Outline-edit drag state. _edit_kind is "move" or a handle name
        # while an existing outline is being dragged; None otherwise.
        self._edit_index: int = -1
        self._edit_kind: str | None = None
        self._edit_orig: tuple[float, float, float, float] | None = None
        self._edit_press: tuple[float, float] | None = None

        # Cover the full virtual desktop (all monitors).
        screen_geo = QGuiApplication.primaryScreen().virtualGeometry()
        self.setGeometry(screen_geo)

        self._origin: QPoint | None = None
        self._current: QPoint | None = None
        self._rotation: float = 0.0  # CW degrees, applied during drag

        self.result: tuple[int, int, int, int] | None = None
        self.result_hwnd: int = 0
        self.result_rotation: float = 0.0

    def _finish(self):
        """Close window and exit the QApplication event loop."""
        self.close()
        app = QApplication.instance()
        if app is not None:
            app.quit()

    # ---- outline editing helpers ----
    def _logical_rect(self, reg: dict) -> tuple[float, float, float, float]:
        """An outline's rect in widget-local logical pixels."""
        dpr = self.screen().devicePixelRatio()
        geo = self.geometry()
        return (
            reg["x"] / dpr - geo.x(),
            reg["y"] / dpr - geo.y(),
            reg["w"] / dpr,
            reg["h"] / dpr,
        )

    def _sync_outline(self, reg: dict, rect: tuple[float, float, float, float]) -> None:
        """Write a widget-local logical rect back into the outline dict
        (physical screen pixels)."""
        dpr = self.screen().devicePixelRatio()
        geo = self.geometry()
        reg["x"] = round((rect[0] + geo.x()) * dpr)
        reg["y"] = round((rect[1] + geo.y()) * dpr)
        reg["w"] = round(rect[2] * dpr)
        reg["h"] = round(rect[3] * dpr)

    def _hit_existing(self, pos) -> tuple[int, str] | None:
        """Topmost outline (and what part of it) under `pos`, or None."""
        for i in range(len(self._existing) - 1, -1, -1):
            reg = self._existing[i]
            kind = _hit_test(
                self._logical_rect(reg), float(reg.get("rotation", 0.0)),
                (pos.x(), pos.y()),
            )
            if kind:
                return i, kind
        return None

    # ---- input ----
    def mousePressEvent(self, ev):
        if ev.button() == Qt.LeftButton:
            pos = ev.position().toPoint()
            hit = self._hit_existing(pos)
            if hit is not None:
                # Grab an existing outline: start move/resize, not a pick.
                self._edit_index, self._edit_kind = hit
                self._edit_orig = self._logical_rect(
                    self._existing[self._edit_index]
                )
                self._edit_press = (pos.x(), pos.y())
                self.setCursor(_HANDLE_CURSORS[self._edit_kind])
                return
            self._origin = pos
            self._current = self._origin
            self.update()

    def mouseMoveEvent(self, ev):
        pos = ev.position().toPoint()
        if self._edit_kind is not None:
            reg = self._existing[self._edit_index]
            new_rect = _apply_edit(
                self._edit_orig,
                float(reg.get("rotation", 0.0)),
                self._edit_kind,
                pos.x() - self._edit_press[0],
                pos.y() - self._edit_press[1],
            )
            self._sync_outline(reg, new_rect)
            self.update()
            return
        if self._origin is not None:
            self._current = pos
            self.update()
            return
        # Idle hover: show what a click would grab.
        hit = self._hit_existing(pos)
        self.setCursor(
            _HANDLE_CURSORS[hit[1]] if hit is not None else Qt.CrossCursor
        )

    def wheelEvent(self, ev):
        # Rotate the in-progress selection. Only meaningful while dragging
        # (otherwise there's no rectangle to rotate yet).
        if self._origin is None:
            ev.ignore()
            return
        delta = ev.angleDelta().y()
        if delta == 0:
            ev.ignore()
            return
        step = 10.0 if (ev.modifiers() & Qt.ShiftModifier) else 2.0
        self._rotation += step if delta > 0 else -step
        # Wrap into (-180, 180]
        while self._rotation > 180:
            self._rotation -= 360
        while self._rotation <= -180:
            self._rotation += 360
        self.update()
        ev.accept()

    def mouseReleaseEvent(self, ev):
        if ev.button() == Qt.LeftButton and self._edit_kind is not None:
            # Commit the outline edit. The dict already holds the final
            # geometry (synced on every move); flag it for the caller.
            self._existing[self._edit_index]["edited"] = True
            self._edit_index = -1
            self._edit_kind = None
            self._edit_orig = None
            self._edit_press = None
            self.setCursor(Qt.CrossCursor)
            self.update()
            return
        if ev.button() == Qt.LeftButton and self._origin is not None:
            rect = QRect(self._origin, ev.position().toPoint()).normalized()
            if rect.width() > 5 and rect.height() > 5:
                # Qt 6 reports logical (DPI-scaled) pixels by default.
                # mss reads in physical pixels, so we must convert via the
                # screen's devicePixelRatio. dpr is 1.0 at 100% scaling,
                # 1.25 at 125%, 1.5 at 150%, etc.
                top_left = self.mapToGlobal(rect.topLeft())
                dpr = self.screen().devicePixelRatio()
                x = int(top_left.x() * dpr)
                y = int(top_left.y() * dpr)
                w = int(rect.width() * dpr)
                h = int(rect.height() * dpr)
                self.result = (x, y, w, h)

                # CRITICAL: hide ourselves before WindowFromPoint, otherwise
                # it returns OUR HWND (the overlay) and the resulting
                # RegionCapture either silently falls back to screen mode
                # (PrintWindow on a dying handle returns blank) or worse,
                # binds to the wrong app on subsequent picks.
                #
                # We hide three ways for redundancy:
                #   1. Qt's hide()  — graceful but sometimes deferred
                #   2. Win32 ShowWindow(SW_HIDE) — forces it now
                #   3. Win32 SetWindowPos(HIDEWINDOW) — also forces it now
                # ...then verify we don't get our own HWND back, and retry
                # for up to ~400 ms if we do.
                my_hwnd = int(self.winId())
                self.hide()
                _force_hide(my_hwnd)
                QApplication.processEvents()

                cx = x + w // 2
                cy = y + h // 2

                detected = 0
                for _ in range(10):
                    try:
                        detected = find_window_at(cx, cy)
                    except Exception:
                        detected = 0
                    if detected and detected != my_hwnd:
                        break
                    # Overlay still on top — give the OS another tick.
                    _force_hide(my_hwnd)
                    time.sleep(0.04)
                    QApplication.processEvents()

                if detected == my_hwnd:
                    detected = 0  # give up; force screen-mode fallback

                self.result_hwnd = detected
                self.result_rotation = self._rotation
                self._finish()
            else:
                # Too small — treat as a misclick, reset.
                self._origin = None
                self._current = None
                self.update()

    def keyPressEvent(self, ev):
        if ev.key() == Qt.Key_Escape:
            self.result = None
            self.result_hwnd = 0
            self.result_rotation = 0.0
            self._finish()
        elif ev.key() == Qt.Key_0 and self._origin is not None:
            # Snap rotation back to 0 mid-drag
            self._rotation = 0.0
            self.update()

    # ---- paint ----
    def _draw_existing(self, p: QPainter) -> None:
        """Outline every already-watched region (dashed, labeled)."""
        if not self._existing:
            return
        p.setFont(QFont("Segoe UI", 9))
        for reg in self._existing:
            lx, ly, lw, lh = self._logical_rect(reg)
            rect = QRect(round(lx), round(ly), round(lw), round(lh))
            color = (
                _SPEAKER_OUTLINE if reg.get("mode") == "speaker"
                else _DIALOGUE_OUTLINE
            )
            rotation = float(reg.get("rotation", 0.0))

            if abs(rotation) > 0.001:
                p.save()
                cx, cy = rect.center().x(), rect.center().y()
                p.translate(cx, cy)
                p.rotate(rotation)
                p.translate(-cx, -cy)

            pen = QPen(color, 2, Qt.DashLine)
            p.setPen(pen)
            p.drawRect(rect)

            # Resize handles: corner and side-midpoint dots (drawn inside
            # the rotation transform so they ride along with the rect).
            p.setPen(QPen(QColor(20, 20, 20, 200), 1))
            p.setBrush(color)
            hw, hh = rect.width() / 2.0, rect.height() / 2.0
            for sx, sy in _HANDLES.values():
                p.drawEllipse(
                    QPoint(
                        round(rect.x() + (sx + 1) * hw),
                        round(rect.y() + (sy + 1) * hh),
                    ),
                    _HANDLE_DRAW_R, _HANDLE_DRAW_R,
                )
            p.setBrush(Qt.NoBrush)

            if abs(rotation) > 0.001:
                p.restore()

            label = reg.get("label", "")
            if label:
                p.fillRect(
                    rect.x(), max(0, rect.y() - 20), 8 + 7 * len(label), 18,
                    QColor(0, 0, 0, 180),
                )
                p.setPen(color)
                p.drawText(rect.x() + 4, max(13, rect.y() - 6), label)

    def paintEvent(self, ev):
        p = QPainter(self)
        p.setRenderHint(QPainter.Antialiasing)
        # Dim the whole screen.
        p.fillRect(self.rect(), QColor(0, 0, 0, 110))

        # Show what's already being watched (under the selection drawing so
        # the in-progress pick always reads on top).
        self._draw_existing(p)

        # Punch a hole for the current selection (rotated if applicable).
        if self._origin and self._current:
            sel = QRect(self._origin, self._current).normalized()
            cx, cy = sel.center().x(), sel.center().y()
            rotated = abs(self._rotation) > 0.001

            if rotated:
                p.save()
                p.translate(cx, cy)
                p.rotate(self._rotation)
                p.translate(-cx, -cy)

            p.setCompositionMode(QPainter.CompositionMode_Clear)
            p.fillRect(sel, Qt.transparent)
            p.setCompositionMode(QPainter.CompositionMode_SourceOver)

            pen = QPen(QColor(80, 200, 255), 2)
            p.setPen(pen)
            p.drawRect(sel)

            if rotated:
                p.restore()

            # Size + rotation label, drawn unrotated for legibility.
            p.setFont(QFont("Segoe UI", 10))
            label = f"{sel.width()} x {sel.height()}"
            if rotated:
                label += f"   {self._rotation:+.0f}\u00B0"
            label_w = 80 + (60 if rotated else 0)
            p.fillRect(sel.x(), max(0, sel.y() - 22), label_w, 20, QColor(0, 0, 0, 180))
            p.setPen(QColor(255, 255, 255))
            p.drawText(sel.x() + 6, max(14, sel.y() - 6), label)

        # Help text top-center.
        p.setPen(QColor(255, 255, 255))
        p.setFont(QFont("Segoe UI", 12))
        help_text = (
            "Drag to select.   Scroll to rotate (Shift = 10\u00B0).   "
            "0 to reset rotation.   Esc to cancel."
        )
        if self._existing:
            help_text += (
                "\nDrag an outline's edge to move a watched region, "
                "its dots to resize it."
            )
        p.drawText(
            self.rect().adjusted(0, 30, 0, 0),
            Qt.AlignHCenter | Qt.AlignTop,
            help_text,
        )


def pick_region(
    existing: list[dict] | None = None,
) -> tuple[tuple[int, int, int, int] | None, int, float, list[dict]]:
    """Open the picker and return (region_tuple, hwnd, rotation, existing).

    region_tuple is (x, y, w, h) in physical pixels, or None if cancelled.
    hwnd is the underlying top-level window HWND at release, or 0.
    rotation is CW degrees applied via mouse wheel during the drag, or 0.0.
    existing is an optional list of already-watched regions to outline on
    the overlay: {x, y, w, h, rotation, label, mode} in physical pixels.
    They are editable — the user can drag an outline's dashed line to move
    it or its handle dots to resize it. Edited entries come back with
    updated geometry and "edited": True (also when the pick itself was
    cancelled with Esc, so a session can be adjust-only).
    """
    app = QApplication.instance() or QApplication(sys.argv)
    overlay = _Overlay(existing=existing)
    # Use show() instead of showFullScreen() so our virtual-desktop geometry
    # (which can span multiple monitors) is honored.
    overlay.show()
    overlay.raise_()
    overlay.activateWindow()
    app.exec()
    # Drain any pending delete events so the next pick_region call starts
    # with a clean slate.
    QApplication.processEvents()
    return (
        overlay.result,
        overlay.result_hwnd,
        overlay.result_rotation,
        overlay._existing,
    )


if __name__ == "__main__":
    r, h, rot, ex = pick_region()
    print("region:", r, "hwnd:", h, "rotation:", rot, "edited:", ex)
