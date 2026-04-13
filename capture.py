"""
Region capture + change detection.

Two backends:
- Screen mode (mss): grabs absolute screen pixels. Fast and simple, but
  Windows Magnifier and display zoom alter what's captured.
- Window mode (PrintWindow): grabs the underlying window's contents and
  crops to the region. Immune to Magnifier/zoom and follows the window
  if the user moves it. Falls back to screen mode if PrintWindow returns
  blank/None (e.g., GPU-rendered DirectX games).

`stable_frames()` blocks until pixels in the region change AND stay still
for `stable_ms`. That's how we avoid OCR'ing partially-revealed text.
"""

from __future__ import annotations

import math
import time
import hashlib
from collections.abc import Iterator

import cv2
import mss
import numpy as np

import ctypes

from window_capture import capture_window, get_window_rect, get_window_title

_user32 = ctypes.windll.user32
_GA_ROOT = 2


def _is_target_foreground(hwnd: int) -> bool:
    """True if the target window (or its root ancestor) is the foreground window."""
    fg = _user32.GetForegroundWindow()
    if not fg:
        return True  # can't tell — assume yes
    fg_root = _user32.GetAncestor(fg, _GA_ROOT) or fg
    hwnd_root = _user32.GetAncestor(hwnd, _GA_ROOT) or hwnd
    return fg_root == hwnd_root


def _hash_frame(arr: np.ndarray) -> str:
    """Cheap perceptual-ish hash: downsample, quantize, hash."""
    small = arr[::8, ::8]
    if small.shape[-1] == 4:
        small = small[..., :3]
    quantized = (small >> 3).astype(np.uint8)
    return hashlib.md5(quantized.tobytes()).hexdigest()


def _rotated_bbox(x: int, y: int, w: int, h: int, rotation_deg: float) -> tuple[float, float, float, float]:
    """Return (left, top, right, bottom) of the axis-aligned bounding box
    that fully contains the rectangle (x,y,w,h) rotated by `rotation_deg`
    around its own center."""
    cx = x + w / 2.0
    cy = y + h / 2.0
    a = math.radians(rotation_deg)
    cos_a = math.cos(a)
    sin_a = math.sin(a)
    corners = ((x, y), (x + w, y), (x + w, y + h), (x, y + h))
    xs: list[float] = []
    ys: list[float] = []
    for px, py in corners:
        dx = px - cx
        dy = py - cy
        xs.append(cx + dx * cos_a - dy * sin_a)
        ys.append(cy + dx * sin_a + dy * cos_a)
    return min(xs), min(ys), max(xs), max(ys)


def _deskew_to_target(
    bbox_img: np.ndarray, rotation_deg: float, target_w: int, target_h: int
) -> np.ndarray:
    """Rotate `bbox_img` by `-rotation_deg` around its center (undoing the
    user's CW rotation) and crop the center to the target dimensions."""
    bh, bw = bbox_img.shape[:2]
    if bh == 0 or bw == 0:
        return bbox_img
    center = (bw / 2.0, bh / 2.0)
    M = cv2.getRotationMatrix2D(center, -rotation_deg, 1.0)
    rotated = cv2.warpAffine(
        bbox_img, M, (bw, bh),
        flags=cv2.INTER_CUBIC,
        borderMode=cv2.BORDER_CONSTANT,
        borderValue=(255, 255, 255),
    )
    # Crop centered to the requested target size, clamped to what we have.
    out_w = min(target_w, bw)
    out_h = min(target_h, bh)
    x_off = max(0, (bw - out_w) // 2)
    y_off = max(0, (bh - out_h) // 2)
    return np.ascontiguousarray(rotated[y_off:y_off + out_h, x_off:x_off + out_w])


class RegionCapture:
    def __init__(
        self,
        region: tuple[int, int, int, int],
        hwnd: int = 0,
        poll_hz: float = 12.0,
        stable_ms: int = 350,
        verbose: bool = False,
        rotation: float = 0.0,
    ):
        x, y, w, h = region
        self.rotation = float(rotation)
        # The "target" size — what callers see after we deskew. Same as
        # the user's drag dimensions, regardless of rotation.
        self.target_w = w
        self.target_h = h

        # If rotated, we need to grab a slightly bigger axis-aligned area
        # so that after rotating the captured pixels back to upright we
        # have full coverage of the user's tilted rect. The bounding box
        # math is in _rotated_bbox.
        if abs(self.rotation) > 0.001:
            bx0, by0, bx1, by1 = _rotated_bbox(x, y, w, h, self.rotation)
            pad = 4  # extra pixels around the bbox to avoid edge artifacts
            cap_x = int(math.floor(bx0)) - pad
            cap_y = int(math.floor(by0)) - pad
            cap_w = int(math.ceil(bx1 - bx0)) + 2 * pad
            cap_h = int(math.ceil(by1 - by0)) + 2 * pad
        else:
            cap_x, cap_y, cap_w, cap_h = x, y, w, h

        self.bbox = {"left": cap_x, "top": cap_y, "width": cap_w, "height": cap_h}
        self.poll_interval = 1.0 / poll_hz
        self.stable_seconds = stable_ms / 1000.0
        self.verbose = verbose

        self.hwnd = hwnd
        # Region's offset relative to the window's top-left at pick time.
        # We store relative coords so the capture follows the window if
        # the user moves it. For rotated regions these are the offsets to
        # the BOUNDING BOX, not the original tilted rect.
        if hwnd:
            wx, wy, _, _ = get_window_rect(hwnd)
            self.rel_x = cap_x - wx
            self.rel_y = cap_y - wy
            self.rel_w = cap_w
            self.rel_h = cap_h
            self.window_title = get_window_title(hwnd)
        else:
            self.rel_x = self.rel_y = self.rel_w = self.rel_h = 0
            self.window_title = ""

        # Decide capture mode at startup. We try window capture once and
        # see if it returns non-blank pixels AND is fast enough. DirectX/
        # OpenGL games often return valid pixels via PrintWindow but at
        # catastrophic latency (100-500ms+) because PW_RENDERFULLCONTENT
        # forces a GPU→system-memory readback of the entire window. That
        # makes 12 Hz polling impossible and the stable-hash check can
        # stall for minutes. Screen capture via mss is fine for fullscreen
        # games where Magnifier/zoom aren't a concern.
        _GRAB_SLOW_MS = 50  # anything above this = too slow for real-time
        # Normal apps: <10ms. Browsers/games with GPU rendering: 60-200ms+.
        # 50ms cleanly separates the two — no edge cases where one region
        # on a window gets game mode and another doesn't.
        self.use_window_mode = False
        self._binarize_hash = False  # set True when falling back from slow window capture
        if hwnd:
            t0 = time.monotonic()
            test = self._grab_window()
            grab_ms = (time.monotonic() - t0) * 1000
            if test is not None and test.size > 0:
                non_black_ratio = (test.sum(axis=-1) > 30).mean()
                if non_black_ratio > 0.05:
                    if grab_ms > _GRAB_SLOW_MS:
                        print(
                            f"[capture] PrintWindow returned pixels but took "
                            f"{grab_ms:.0f}ms (>{_GRAB_SLOW_MS}ms) — falling "
                            f"back to fast screen capture."
                        )
                        # Screen capture of a game will have animated
                        # backgrounds behind dialogue. Binarize the hash
                        # so only text changes trigger, not flickering
                        # lighting/particles.
                        self._binarize_hash = True
                    else:
                        self.use_window_mode = True

        # A black frame returned when the target window is not foreground,
        # so we never accidentally OCR a random overlapping window.
        self._blank_frame = np.zeros(
            (self.target_h, self.target_w, 3), dtype=np.uint8
        )

        # Polling state for poll_once() — used when one outer loop drives
        # multiple regions. stable_frames() ignores these.
        self._current_hash: str = ""
        self._last_yielded_hash: str = ""
        self._stable_since: float = 0.0
        self._initialized = False
        self._game_poll_count: int = 0

    # ---- backends ----

    def _grab_screen(self) -> np.ndarray:
        # Lazy mss instance per call is fine — it's cheap.
        with mss.mss() as sct:
            shot = sct.grab(self.bbox)
            arr = np.frombuffer(shot.rgb, dtype=np.uint8).reshape(
                shot.height, shot.width, 3
            )
        if abs(self.rotation) > 0.001:
            arr = _deskew_to_target(arr, self.rotation, self.target_w, self.target_h)
        return arr

    def _grab_window(self) -> np.ndarray | None:
        full = capture_window(self.hwnd)
        if full is None:
            return None
        H, W = full.shape[:2]
        # Clamp the relative crop to the current window bounds in case the
        # window was resized smaller after pick time.
        x0 = max(0, min(self.rel_x, W - 1))
        y0 = max(0, min(self.rel_y, H - 1))
        x1 = max(x0 + 1, min(self.rel_x + self.rel_w, W))
        y1 = max(y0 + 1, min(self.rel_y + self.rel_h, H))
        cropped = np.ascontiguousarray(full[y0:y1, x0:x1])
        if abs(self.rotation) > 0.001:
            cropped = _deskew_to_target(
                cropped, self.rotation, self.target_w, self.target_h
            )
        return cropped

    def _grab(self) -> np.ndarray:
        if self.use_window_mode:
            frame = self._grab_window()
            if frame is not None and frame.size > 0:
                return frame
            # PrintWindow blipped — fall through to screen.

        if self._binarize_hash:
            # Game mode: prefer PrintWindow (immune to Magnifier), but
            # if it blips (returns None), fall through to screen capture
            # instead of returning blank. A brief Magnifier artifact is
            # better than losing the frame entirely.
            frame = self._grab_window()
            if frame is not None and frame.size > 0:
                return frame
            # PrintWindow blipped — use screen capture as fallback.
            return self._grab_screen()

        # Non-game screen capture: blank frame when target isn't
        # foreground to avoid reading the wrong window.
        if self.hwnd and not _is_target_foreground(self.hwnd):
            return self._blank_frame

        return self._grab_screen()

    # ---- public api ----

    def snapshot(self) -> np.ndarray:
        """Grab a single frame right now (no change detection, no loop)."""
        return self._grab()

    # In game mode (screen capture fallback for a GPU app), pixel hashing
    # is unreliable: animated backgrounds behind semi-transparent dialogue
    # overlays cause constant hash changes OR the binarized hash is too
    # coarse to detect new text. Instead, we return a frame at ~2 Hz and
    # let the caller's OCR + text-dedup handle change detection.
    _GAME_POLL_INTERVAL = 3  # return every 3rd poll ≈ 4 Hz at 12 Hz

    def poll_once(self) -> np.ndarray | None:
        """Single non-blocking poll. Returns a frame iff the region has
        changed AND been stable for `stable_ms` since the change. Otherwise
        returns None. Designed for an outer loop driving multiple regions."""

        # Game mode: skip pixel hashing, just return frames periodically.
        # The caller's text-based dedup (OCR + _is_cosmetic_change) is far
        # more reliable for animated game UIs.
        #
        # We throttle BEFORE grabbing: at ~2 Hz we can afford the slow
        # PrintWindow path (100ms) which is immune to Magnifier/zoom.
        # At 12 Hz we couldn't (12 × 100ms > 1 second).
        if self._binarize_hash:
            self._game_poll_count += 1
            if self._game_poll_count % self._GAME_POLL_INTERVAL != 0:
                return None
            return self._grab()

        frame = self._grab()

        new_hash = _hash_frame(frame)

        if not self._initialized:
            self._current_hash = new_hash
            self._stable_since = time.monotonic()
            self._initialized = True
            return None

        if new_hash != self._current_hash:
            self._current_hash = new_hash
            self._stable_since = time.monotonic()
            return None

        if (
            time.monotonic() - self._stable_since >= self.stable_seconds
            and self._current_hash != self._last_yielded_hash
        ):
            self._last_yielded_hash = self._current_hash
            return frame

        return None

    def stable_frames(self) -> Iterator[np.ndarray]:
        """Yield a frame every time the region changes and then stabilizes."""
        last_yielded_hash = None
        current_hash = _hash_frame(self._grab())
        stable_since = time.monotonic()
        last_heartbeat = time.monotonic()
        poll_count = 0

        while True:
            time.sleep(self.poll_interval)
            frame = self._grab()
            new_hash = _hash_frame(frame)
            poll_count += 1

            if self.verbose and time.monotonic() - last_heartbeat > 3.0:
                print(
                    f"[capture] {poll_count} polls, "
                    f"hash={current_hash[:8]} "
                    f"stable_for={time.monotonic() - stable_since:.1f}s",
                    flush=True,
                )
                last_heartbeat = time.monotonic()

            if new_hash != current_hash:
                if self.verbose:
                    print(f"[capture] change detected ({new_hash[:8]})", flush=True)
                current_hash = new_hash
                stable_since = time.monotonic()
                continue

            if (
                time.monotonic() - stable_since >= self.stable_seconds
                and current_hash != last_yielded_hash
            ):
                last_yielded_hash = current_hash
                if self.verbose:
                    print("[capture] stable -> yielding frame", flush=True)
                yield frame
