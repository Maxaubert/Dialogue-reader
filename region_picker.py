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


class _Overlay(QWidget):
    def __init__(self):
        super().__init__()
        # No Qt.Tool flag — Tool windows do not trigger lastWindowClosed,
        # which causes app.exec() to hang forever after the user makes a pick.
        self.setWindowFlags(
            Qt.FramelessWindowHint
            | Qt.WindowStaysOnTopHint
        )
        self.setAttribute(Qt.WA_TranslucentBackground)
        self.setAttribute(Qt.WA_DeleteOnClose)
        self.setCursor(Qt.CrossCursor)

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

    # ---- input ----
    def mousePressEvent(self, ev):
        if ev.button() == Qt.LeftButton:
            self._origin = ev.position().toPoint()
            self._current = self._origin
            self.update()

    def mouseMoveEvent(self, ev):
        if self._origin is not None:
            self._current = ev.position().toPoint()
            self.update()

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
    def paintEvent(self, ev):
        p = QPainter(self)
        p.setRenderHint(QPainter.Antialiasing)
        # Dim the whole screen.
        p.fillRect(self.rect(), QColor(0, 0, 0, 110))

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
        p.drawText(
            self.rect().adjusted(0, 30, 0, 0),
            Qt.AlignHCenter | Qt.AlignTop,
            "Drag to select.   Scroll to rotate (Shift = 10\u00B0).   "
            "0 to reset rotation.   Esc to cancel.",
        )


def pick_region() -> tuple[tuple[int, int, int, int] | None, int, float]:
    """Open the picker and return (region_tuple, hwnd, rotation_degrees).

    region_tuple is (x, y, w, h) in physical pixels, or None if cancelled.
    hwnd is the underlying top-level window HWND at release, or 0.
    rotation_degrees is CW rotation applied via mouse wheel during the
    drag, or 0.0 if the user didn't scroll.
    """
    app = QApplication.instance() or QApplication(sys.argv)
    overlay = _Overlay()
    # Use show() instead of showFullScreen() so our virtual-desktop geometry
    # (which can span multiple monitors) is honored.
    overlay.show()
    overlay.raise_()
    overlay.activateWindow()
    app.exec()
    # Drain any pending delete events so the next pick_region call starts
    # with a clean slate.
    QApplication.processEvents()
    return overlay.result, overlay.result_hwnd, overlay.result_rotation


if __name__ == "__main__":
    r, h, rot = pick_region()
    print("region:", r, "hwnd:", h, "rotation:", rot)
