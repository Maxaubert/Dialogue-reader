"""
Windows Magnifier zoom detection.

Reads the current fullscreen magnification factor via magnification.dll's
`MagGetFullscreenTransform`. Returns 1.0 when Magnifier is not active or
at 100% zoom, >1.0 when the user has zoomed in.

Use case: when the user uses Windows Magnifier to inspect something on
screen, pixel positions shift. If the dialogue reader keeps polling in
this state it can pick up a random name as "speaker" or mis-OCR already-
stored dialogue. Call `is_zoomed()` in the main loop to decide whether
to skip a poll cycle.

Windows-only. On non-Windows or when magnification.dll is absent, all
functions return as if Magnifier is not active (level=1.0, zoomed=False).
"""

from __future__ import annotations

import ctypes
import ctypes.wintypes
import sys
import threading


_EPSILON = 1e-3  # float comparison slack for the 1.0 "unzoomed" state


class _MagState:
    """Lazy, thread-safe wrapper around MagInitialize + MagGetFullscreenTransform.

    MagInitialize is per-process and must be called exactly once before
    the query functions work. We defer it to first use so importing this
    module has no side effects if the caller never calls is_zoomed().
    """

    def __init__(self) -> None:
        self._lock = threading.Lock()
        self._initialized = False
        self._available = False
        self._dll: ctypes.WinDLL | None = None

    def _ensure_initialized(self) -> bool:
        with self._lock:
            if self._initialized:
                return self._available
            self._initialized = True
            if sys.platform != "win32":
                return False
            try:
                dll = ctypes.WinDLL("magnification.dll")
            except OSError:
                return False
            dll.MagInitialize.restype = ctypes.wintypes.BOOL
            dll.MagGetFullscreenTransform.argtypes = [
                ctypes.POINTER(ctypes.c_float),
                ctypes.POINTER(ctypes.c_int),
                ctypes.POINTER(ctypes.c_int),
            ]
            dll.MagGetFullscreenTransform.restype = ctypes.wintypes.BOOL
            if not dll.MagInitialize():
                return False
            self._dll = dll
            self._available = True
            return True

    def get_level(self) -> float:
        """Return the current fullscreen magnification factor. 1.0 means
        Magnifier is at 100% (or not active). Returns 1.0 on any error
        so callers can treat "unknown" as "not zoomed"."""
        if not self._ensure_initialized() or self._dll is None:
            return 1.0
        level = ctypes.c_float(1.0)
        xo = ctypes.c_int(0)
        yo = ctypes.c_int(0)
        try:
            ok = self._dll.MagGetFullscreenTransform(
                ctypes.byref(level), ctypes.byref(xo), ctypes.byref(yo)
            )
        except OSError:
            return 1.0
        if not ok:
            return 1.0
        # Magnifier may briefly return 0.0 while transitioning between
        # states; treat anything <=0 as unzoomed.
        if level.value <= 0.0:
            return 1.0
        return float(level.value)


_state = _MagState()


# Windows class name of the desktop-sized window Magnifier draws into when
# fullscreen zoom is actively being rendered. The process can be running
# with a >1.0 transform configured but this window HIDDEN — that's the
# state after the user closes Magnifier but before the OS clears the
# saved transform, or when the MIG overlay sets the transform but isn't
# currently showing a zoom. We need to check both.
_FULLSCREEN_MAG_CLASS = "Screen Magnifier Fullscreen Window"

_user32: ctypes.WinDLL | None = None
if sys.platform == "win32":
    try:
        _user32 = ctypes.WinDLL("user32")
        _user32.FindWindowW.argtypes = [ctypes.wintypes.LPCWSTR, ctypes.wintypes.LPCWSTR]
        _user32.FindWindowW.restype = ctypes.wintypes.HWND
        _user32.IsWindowVisible.argtypes = [ctypes.wintypes.HWND]
        _user32.IsWindowVisible.restype = ctypes.wintypes.BOOL
    except OSError:
        _user32 = None


def _fullscreen_magnifier_visible() -> bool:
    """True iff the Magnifier's fullscreen rendering window is currently
    visible. Cheap: one FindWindow + IsWindowVisible call."""
    if _user32 is None:
        return False
    try:
        hwnd = _user32.FindWindowW(_FULLSCREEN_MAG_CLASS, None)
    except OSError:
        return False
    if not hwnd:
        return False
    return bool(_user32.IsWindowVisible(hwnd))


def get_magnification_level() -> float:
    """Current fullscreen magnification factor. 1.0 means not zoomed."""
    return _state.get_level()


def is_zoomed() -> bool:
    """True iff Windows Magnifier is currently zooming the screen.

    Requires BOTH:
      - transform level > 1.0 (via MagGetFullscreenTransform)
      - the fullscreen-rendering window exists and is visible

    The second check matters because Magnifier (and third-party tools
    like the MIG overlay) often keep a saved transform of e.g. 4.0 even
    when the user isn't actively zoomed — the rendering window is hidden
    in that state, so the screen looks normal regardless of the saved
    transform.
    """
    if get_magnification_level() <= 1.0 + _EPSILON:
        return False
    return _fullscreen_magnifier_visible()
