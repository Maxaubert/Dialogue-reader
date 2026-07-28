"""
Region capture + change detection.

Two backends:
- Screen mode (mss): grabs absolute screen pixels. Fast and simple, but
  Windows Magnifier and display zoom alter what's captured.
- Window mode (PrintWindow): grabs the underlying window's contents and
  crops to the region. Immune to Magnifier/zoom and follows the window
  if the user moves it. Falls back to screen mode if PrintWindow returns
  blank/None (e.g., GPU-rendered DirectX games).
"""

from __future__ import annotations

import math
import time
import hashlib
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
    # OpenCV's positive angle is COUNTER-clockwise, and `rotation_deg` is the
    # user's clockwise tilt, so undoing it needs +rotation_deg. Passing the
    # negative rotated the same way again and handed OCR text at double the
    # tilt (issue #26).
    M = cv2.getRotationMatrix2D(center, rotation_deg, 1.0)
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


VALID_CAPTURE_MODES = ("auto", "screen", "window")

# Pick-time stability probe (auto mode). We sample the region at the same
# cadence the runtime poll loop uses and ask: would the pixel-hash gate ever
# open here? If the longest run of identical hashes is shorter than the
# stable_ms requirement, the content is animated and only game mode (OCR-based
# change detection) can work. The probe runs ONCE, at pick time — content is
# expected to churn later (e.g. after an in-game letter closes), so the
# decision is never revisited.
_PROBE_SAMPLES = 10
_PROBE_INTERVAL = 1.0 / 12.0


def _longest_stable_run_ms(hashes: list[str], interval_ms: float) -> float:
    """Duration of the longest run of consecutive identical hashes, where a
    run of k identical samples spans (k-1) * interval_ms."""
    if not hashes:
        return 0.0
    best = cur = 1
    for a, b in zip(hashes, hashes[1:]):
        cur = cur + 1 if a == b else 1
        best = max(best, cur)
    return (best - 1) * interval_ms


def _decide_auto_mode(pw_ok: bool, pw_slow: bool, screen_stable: bool) -> str:
    """Classify a region at pick time.

    Returns "game" (periodic OCR + text dedup), "window" (PrintWindow +
    pixel-hash gate), or "screen" (screen grab + pixel-hash gate).
    Animated pixels always win: the pixel-hash gate can never open on them,
    no matter how the frames are sourced.
    """
    if not screen_stable:
        return "game"
    if pw_ok and pw_slow:
        return "game"
    if pw_ok:
        return "window"
    return "screen"


def _frame_usable(frame: np.ndarray | None) -> bool:
    """True if a captured frame contains actual content. PrintWindow on
    some GPU-rendered games (e.g. UE5/DX12) returns a full-size all-BLACK
    bitmap — non-None and non-empty, but OCRing it reads nothing forever.
    Those frames must never be preferred over a screen grab."""
    if frame is None or frame.size == 0:
        return False
    return (frame.sum(axis=-1) > 30).mean() > 0.02


def _frames_roughly_match(
    a: np.ndarray | None, b: np.ndarray | None, max_mean_diff: float = 20.0
) -> bool:
    """True if two frames show approximately the same content. Used to catch
    PrintWindow serving a stale/frozen surface: it answers fast with valid
    pixels that no longer match what's actually on screen."""
    if a is None or b is None or a.shape != b.shape:
        return False
    da = a[::8, ::8].astype(np.int16)
    db = b[::8, ::8].astype(np.int16)
    return float(np.abs(da - db).mean()) <= max_mean_diff


class RegionCapture:
    def __init__(
        self,
        region: tuple[int, int, int, int],
        hwnd: int = 0,
        poll_hz: float = 12.0,
        stable_ms: int = 350,
        verbose: bool = False,
        rotation: float = 0.0,
        capture_mode: str = "auto",
    ):
        if capture_mode not in VALID_CAPTURE_MODES:
            raise ValueError(
                f"Unknown capture_mode: {capture_mode!r}. "
                f"Valid: {VALID_CAPTURE_MODES}"
            )
        self.capture_mode = capture_mode
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

        self.use_window_mode = False
        self._binarize_hash = False  # True = game mode (OCR-based change detection)
        # Whether PrintWindow is worth calling at runtime. Forced window
        # mode always tries; auto mode learns from the probe (a game whose
        # PrintWindow came back black shouldn't pay a per-frame GPU
        # readback that can also flicker the game).
        self._pw_usable = capture_mode != "screen"
        if capture_mode == "window":
            # User forced PrintWindow. Skip the probe — even if slow
            # we're committed. Needed when Magnifier-immunity matters more
            # than blink-free capture.
            if hwnd:
                self.use_window_mode = True
        elif capture_mode == "screen":
            # User forced mss. No PrintWindow probe at all — important
            # because the probe itself triggers a game redraw that can
            # visibly blink. Game-mode polling since game backgrounds
            # animate behind dialogue.
            self._binarize_hash = True
        else:  # capture_mode == "auto"
            self._probe_auto_mode()

        # A black frame returned when the target window is not foreground,
        # so we never accidentally OCR a random overlapping window.
        self._blank_frame = np.zeros(
            (self.target_h, self.target_w, 3), dtype=np.uint8
        )

        # Polling state for poll_once() — used when one outer loop drives
        # multiple regions.
        self._current_hash: str = ""
        self._last_yielded_hash: str = ""
        self._stable_since: float = 0.0
        self._initialized = False
        self._game_poll_count: int = 0

    # ---- pick-time probe ----

    # PrintWindow latency threshold. Normal apps: <10ms. Browsers/games with
    # GPU rendering: 60-200ms+ because PW_RENDERFULLCONTENT forces a
    # GPU→system-memory readback of the entire window. That makes 12 Hz
    # polling impossible, so slow PrintWindow also means game mode.
    _GRAB_SLOW_MS = 50

    def _probe_auto_mode(self) -> None:
        """Classify this region once, at pick time (auto mode only).

        Two measurements feed _decide_auto_mode:
        - One PrintWindow grab: does window capture work here, and how fast?
        - ~1s of screen samples at the runtime poll cadence: do the pixels
          hold still long enough for the pixel-hash gate to ever open?

        Sets use_window_mode / _binarize_hash accordingly. The decision is
        final — game content is expected to animate later even when the
        picked area (a letter, a menu) is static right now, so re-probing
        after pick time would misclassify.
        """
        pw_ok = False
        pw_slow = False
        pw_frame = None
        if self.hwnd:
            t0 = time.monotonic()
            pw_frame = self._grab_window()
            grab_ms = (time.monotonic() - t0) * 1000
            if grab_ms > self._GRAB_SLOW_MS:
                # A single slow sample can be transient system contention,
                # not intrinsic PrintWindow cost. Retry and take the min —
                # the fastest observed grab is the honest estimate.
                for _ in range(2):
                    t0 = time.monotonic()
                    retry = self._grab_window()
                    grab_ms = min(grab_ms, (time.monotonic() - t0) * 1000)
                    if retry is not None:
                        pw_frame = retry
            if pw_frame is not None and pw_frame.size > 0:
                non_black_ratio = (pw_frame.sum(axis=-1) > 30).mean()
                if non_black_ratio > 0.05:
                    pw_ok = True
                    pw_slow = grab_ms > self._GRAB_SLOW_MS
        self._pw_usable = pw_ok

        hashes: list[str] = []
        last_screen: np.ndarray | None = None
        for i in range(_PROBE_SAMPLES):
            if i:
                time.sleep(_PROBE_INTERVAL)
            last_screen = self._grab_screen()
            hashes.append(_hash_frame(last_screen))
        screen_stable = (
            _longest_stable_run_ms(hashes, _PROBE_INTERVAL * 1000)
            >= self.stable_seconds * 1000
        )

        mode = _decide_auto_mode(pw_ok, pw_slow, screen_stable)
        if mode == "window" and not _frames_roughly_match(pw_frame, last_screen):
            print(
                "[capture] PrintWindow frame doesn't match the screen "
                "(stale/frozen surface) — using screen capture instead."
            )
            mode = "screen"

        if mode == "game":
            self._binarize_hash = True
            if not screen_stable:
                print(
                    "[capture] Animated content detected at pick time — "
                    "using game mode (OCR-based change detection)."
                )
            else:
                print(
                    f"[capture] PrintWindow works but is too slow "
                    f"(>{self._GRAB_SLOW_MS}ms) — using game mode."
                )
        elif mode == "window":
            self.use_window_mode = True

    @property
    def game_mode(self) -> bool:
        """True when this region uses game-mode polling (periodic frames +
        OCR-text change detection) instead of the pixel-hash gate."""
        return self._binarize_hash

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
        # Forced "screen" mode: never touch PrintWindow, even as fallback.
        # PrintWindow calls cause a game redraw that can visibly flicker
        # on DirectX titles — the whole point of forcing screen is to
        # avoid that. Still blank when the target isn't foreground so an
        # overlapping window is never OCR'd.
        if self.capture_mode == "screen":
            if self.hwnd and not _is_target_foreground(self.hwnd):
                return self._blank_frame
            return self._grab_screen()

        if self.use_window_mode:
            frame = self._grab_window()
            if _frame_usable(frame):
                return frame
            # PrintWindow blipped or went black — fall through to screen.

        if self._binarize_hash:
            # Game mode: prefer PrintWindow because it's immune to
            # Magnifier — but only when the probe found it usable, and
            # never trust an all-black frame (UE5/DX12 readbacks can turn
            # black at runtime; OCRing them is silent failure). If it
            # blips, return a screen grab instead of nothing. A Magnifier
            # artifact is better than losing the frame — unless the target
            # isn't even foreground, in which case a screen grab would
            # capture whatever window is on top.
            if self._pw_usable:
                frame = self._grab_window()
                if _frame_usable(frame):
                    return frame
            if self.hwnd and not _is_target_foreground(self.hwnd):
                return self._blank_frame
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
