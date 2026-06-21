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


def get_magnification_level() -> float:
    """Current fullscreen magnification factor. 1.0 means not zoomed."""
    return _state.get_level()


def is_zoomed() -> bool:
    """True iff Windows Magnifier's fullscreen transform is above 1.0.

    Previous versions also required a process check or a hidden-window
    cross-check, but those fail for users whose Magnify.exe runs
    persistently (e.g. as part of the Magnifier-In-Games overlay) and
    whose API transform state is managed by that overlay. Level alone is
    the most reliable signal across those setups. If your Magnifier
    keeps a non-1.0 transform at rest, set SkipWhenZoomed=false in the
    INI to disable this check entirely.
    """
    return get_magnification_level() > 1.0 + _EPSILON


if __name__ == "__main__":
    # Live probe. Run:  py magnifier.py
    # Then zoom in / out with Win+Plus / Win+Minus and watch the level.
    import time
    print("Magnifier live probe. Zoom in/out — Ctrl+C to stop.")
    last = None
    while True:
        level = get_magnification_level()
        zoomed = is_zoomed()
        key = (round(level, 2), zoomed)
        if key != last:
            print(f"  level={level:.2f}  is_zoomed={zoomed}")
            last = key
        time.sleep(0.1)
