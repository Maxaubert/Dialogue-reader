"""
Direct window capture via Win32 PrintWindow.

Captures a window's contents without going through the desktop compositor.
This bypasses Windows Magnifier (Fullscreen mode), display zoom, and any
overlapping windows — we get the raw underlying window pixels.

Usage:
    from window_capture import find_window_at, capture_window, get_window_title
    hwnd = find_window_at(screen_x, screen_y)
    print(get_window_title(hwnd))
    rgb = capture_window(hwnd)   # numpy uint8 (H, W, 3) or None on failure

Limitations:
- DirectX/OpenGL games may return blank pixels (PrintWindow can't introspect
  some GPU rendering paths). Fall back to screen capture for those.
- Minimized windows return blank.
"""

from __future__ import annotations

import ctypes
from ctypes import wintypes

import numpy as np
import win32gui
import win32ui


_PW_RENDERFULLCONTENT = 0x00000002  # critical for DWM-composited windows
_GA_ROOT = 2

_user32 = ctypes.windll.user32


def find_window_at(x: int, y: int) -> int:
    """Top-level window HWND containing screen point (x, y), or 0."""
    hwnd = _user32.WindowFromPoint(wintypes.POINT(x, y))
    if not hwnd:
        return 0
    return _user32.GetAncestor(hwnd, _GA_ROOT) or hwnd


def get_window_rect(hwnd: int) -> tuple[int, int, int, int]:
    """Return (left, top, width, height) of the window in screen coords."""
    left, top, right, bot = win32gui.GetWindowRect(hwnd)
    return left, top, right - left, bot - top


def get_window_title(hwnd: int) -> str:
    try:
        return win32gui.GetWindowText(hwnd) or "<untitled>"
    except Exception:
        return "<unknown>"


def capture_window(hwnd: int) -> np.ndarray | None:
    """Capture the window's contents as a (H, W, 3) RGB uint8 array.
    Returns None if the capture fails (window closed, minimized, GPU app)."""
    try:
        left, top, right, bot = win32gui.GetWindowRect(hwnd)
    except Exception:
        return None

    w, h = right - left, bot - top
    if w <= 0 or h <= 0:
        return None

    hwnd_dc = win32gui.GetWindowDC(hwnd)
    if not hwnd_dc:
        return None

    mfc_dc = None
    save_dc = None
    bitmap = None
    try:
        mfc_dc = win32ui.CreateDCFromHandle(hwnd_dc)
        save_dc = mfc_dc.CreateCompatibleDC()
        bitmap = win32ui.CreateBitmap()
        bitmap.CreateCompatibleBitmap(mfc_dc, w, h)
        save_dc.SelectObject(bitmap)

        result = _user32.PrintWindow(
            hwnd, save_dc.GetSafeHdc(), _PW_RENDERFULLCONTENT
        )
        if result != 1:
            return None

        info = bitmap.GetInfo()
        raw = bitmap.GetBitmapBits(True)
        arr = np.frombuffer(raw, dtype=np.uint8).reshape(
            info["bmHeight"], info["bmWidth"], 4
        )
        # BGRA -> RGB
        return np.ascontiguousarray(arr[:, :, [2, 1, 0]])
    except Exception:
        return None
    finally:
        if bitmap is not None:
            try:
                win32gui.DeleteObject(bitmap.GetHandle())
            except Exception:
                pass
        if save_dc is not None:
            try:
                save_dc.DeleteDC()
            except Exception:
                pass
        if mfc_dc is not None:
            try:
                mfc_dc.DeleteDC()
            except Exception:
                pass
        try:
            win32gui.ReleaseDC(hwnd, hwnd_dc)
        except Exception:
            pass
