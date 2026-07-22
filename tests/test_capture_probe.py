"""Tests for the pick-time capture-mode probe (issue #3).

Auto mode must classify a region by sampling pixel stability at pick time:
animated content => game mode (OCR-based change detection), regardless of
PrintWindow latency. The decision is made once, at pick time — never later.
"""
import numpy as np
import pytest

import capture
from capture import (
    RegionCapture,
    _decide_auto_mode,
    _frames_roughly_match,
    _longest_stable_run_ms,
)


# ---- _longest_stable_run_ms ----------------------------------------------

def test_stable_run_all_identical():
    assert _longest_stable_run_ms(["a"] * 10, 83.0) == pytest.approx(9 * 83.0)


def test_stable_run_alternating_is_zero():
    assert _longest_stable_run_ms(["a", "b"] * 5, 83.0) == 0.0


def test_stable_run_middle_run():
    hashes = ["a", "b", "b", "b", "b", "c"]
    assert _longest_stable_run_ms(hashes, 100.0) == pytest.approx(300.0)


def test_stable_run_empty():
    assert _longest_stable_run_ms([], 83.0) == 0.0


# ---- _decide_auto_mode ----------------------------------------------------

def test_unstable_screen_always_means_game_mode():
    # The Vane case: even a fast, valid PrintWindow must not win when the
    # region's pixels churn — the pixel-hash gate would deadlock.
    assert _decide_auto_mode(pw_ok=True, pw_slow=False, screen_stable=False) == "game"
    assert _decide_auto_mode(pw_ok=False, pw_slow=False, screen_stable=False) == "game"


def test_stable_with_fast_printwindow_is_window_mode():
    assert _decide_auto_mode(pw_ok=True, pw_slow=False, screen_stable=True) == "window"


def test_stable_with_slow_printwindow_is_game_mode():
    assert _decide_auto_mode(pw_ok=True, pw_slow=True, screen_stable=True) == "game"


def test_stable_without_printwindow_is_screen_gate():
    assert _decide_auto_mode(pw_ok=False, pw_slow=False, screen_stable=True) == "screen"


# ---- _frames_roughly_match ------------------------------------------------

def test_identical_frames_match():
    a = np.full((50, 100, 3), 128, dtype=np.uint8)
    assert _frames_roughly_match(a, a.copy())


def test_divergent_frames_do_not_match():
    a = np.full((50, 100, 3), 128, dtype=np.uint8)
    b = np.full((50, 100, 3), 210, dtype=np.uint8)
    assert not _frames_roughly_match(a, b)


def test_shape_mismatch_does_not_match():
    a = np.full((50, 100, 3), 128, dtype=np.uint8)
    b = np.full((60, 100, 3), 128, dtype=np.uint8)
    assert not _frames_roughly_match(a, b)


def test_none_does_not_match():
    a = np.full((50, 100, 3), 128, dtype=np.uint8)
    assert not _frames_roughly_match(None, a)


# ---- RegionCapture auto-mode integration ---------------------------------

REGION = (10, 10, 100, 50)  # x, y, w, h -> frames are (50, 100, 3)
HWND = 5


def _static_frame(value: int = 128) -> np.ndarray:
    return np.full((50, 100, 3), value, dtype=np.uint8)


def _full_window_frame(value: int = 128) -> np.ndarray:
    return np.full((600, 800, 3), value, dtype=np.uint8)


@pytest.fixture
def patched_window(monkeypatch):
    """Stub the win32 layer: an 800x600 window at the origin, no sleeps."""
    monkeypatch.setattr(capture, "get_window_rect", lambda h: (0, 0, 800, 600))
    monkeypatch.setattr(capture, "get_window_title", lambda h: "Game")
    monkeypatch.setattr(capture.time, "sleep", lambda s: None)


def _make_auto(monkeypatch, screen_frames, window_frame):
    """Build an auto-mode RegionCapture with fake capture backends.

    screen_frames: callable(index) -> frame for each successive screen grab.
    window_frame: full-window frame for PrintWindow, or None for failure.
    """
    calls = {"screen": 0}

    def fake_screen(self):
        i = calls["screen"]
        calls["screen"] += 1
        return screen_frames(i)

    monkeypatch.setattr(RegionCapture, "_grab_screen", fake_screen)
    monkeypatch.setattr(capture, "capture_window", lambda h: window_frame)
    return RegionCapture(REGION, hwnd=HWND, capture_mode="auto")


def test_churning_pixels_pick_game_mode(monkeypatch, patched_window):
    # Every screen sample differs (animated 3D scene) — must land in game
    # mode even though PrintWindow is fast and returns valid pixels.
    cap = _make_auto(
        monkeypatch,
        screen_frames=lambda i: _static_frame((i * 16) % 256),
        window_frame=_full_window_frame(),
    )
    assert cap.game_mode
    assert not cap.use_window_mode


def test_static_pixels_with_fast_printwindow_pick_window_mode(
    monkeypatch, patched_window
):
    cap = _make_auto(
        monkeypatch,
        screen_frames=lambda i: _static_frame(),
        window_frame=_full_window_frame(),
    )
    assert cap.use_window_mode
    assert not cap.game_mode


def test_frozen_printwindow_frame_falls_back_to_screen_gate(
    monkeypatch, patched_window
):
    # PrintWindow answers fast with a stale surface that doesn't match the
    # screen — trusting it would speak once and then go silent forever.
    cap = _make_auto(
        monkeypatch,
        screen_frames=lambda i: _static_frame(128),
        window_frame=_full_window_frame(210),
    )
    assert not cap.use_window_mode
    assert not cap.game_mode


def test_black_printwindow_with_static_screen_is_screen_gate(
    monkeypatch, patched_window
):
    cap = _make_auto(
        monkeypatch,
        screen_frames=lambda i: _static_frame(),
        window_frame=_full_window_frame(0),
    )
    assert not cap.use_window_mode
    assert not cap.game_mode


def test_no_hwnd_with_churning_pixels_still_picks_game_mode(monkeypatch):
    # Window detection failed at pick time, but the pixels churn — the
    # stability sample alone must route the region into game mode.
    monkeypatch.setattr(capture.time, "sleep", lambda s: None)

    def fake_screen(self):
        fake_screen.i += 1
        return _static_frame((fake_screen.i * 16) % 256)

    fake_screen.i = 0
    monkeypatch.setattr(RegionCapture, "_grab_screen", fake_screen)
    cap = RegionCapture(REGION, hwnd=0, capture_mode="auto")
    assert cap.game_mode


# ---- foreground guards ----------------------------------------------------

def test_forced_screen_mode_blanks_when_target_not_foreground(
    monkeypatch, patched_window
):
    monkeypatch.setattr(
        RegionCapture, "_grab_screen", lambda self: _static_frame()
    )
    cap = RegionCapture(REGION, hwnd=HWND, capture_mode="screen")
    monkeypatch.setattr(capture, "_is_target_foreground", lambda h: False)
    frame = cap._grab()
    assert frame.shape == (50, 100, 3)
    assert not frame.any()


def test_forced_screen_mode_grabs_when_target_foreground(
    monkeypatch, patched_window
):
    monkeypatch.setattr(
        RegionCapture, "_grab_screen", lambda self: _static_frame()
    )
    cap = RegionCapture(REGION, hwnd=HWND, capture_mode="screen")
    monkeypatch.setattr(capture, "_is_target_foreground", lambda h: True)
    assert cap._grab().any()


def test_game_mode_screen_fallback_blanks_when_target_not_foreground(
    monkeypatch, patched_window
):
    cap = _make_auto(
        monkeypatch,
        screen_frames=lambda i: _static_frame((i * 16) % 256),
        window_frame=_full_window_frame(),
    )
    assert cap.game_mode
    # At runtime PrintWindow starts failing and the target loses focus:
    # the screen fallback must not capture whatever window is on top.
    monkeypatch.setattr(capture, "capture_window", lambda h: None)
    monkeypatch.setattr(capture, "_is_target_foreground", lambda h: False)
    frame = cap._grab()
    assert not frame.any()


# ---- PrintWindow latency: transient spikes must not misclassify ----------

def _scripted_monotonic(monkeypatch, values):
    """time.monotonic returns the scripted values, then keeps ticking."""
    state = {"i": 0, "last": values[-1]}

    def fake():
        if state["i"] < len(values):
            v = values[state["i"]]
        else:
            state["last"] += 0.0001
            v = state["last"]
        state["i"] += 1
        return v

    monkeypatch.setattr(capture.time, "monotonic", fake)


def test_transient_slow_printwindow_still_picks_window_mode(
    monkeypatch, patched_window
):
    # First grab spikes to 80ms (system contention), retries come back at
    # 10ms — the region must still land in window mode.
    _scripted_monotonic(
        monkeypatch, [0.0, 0.080, 0.100, 0.110, 0.200, 0.210]
    )
    cap = _make_auto(
        monkeypatch,
        screen_frames=lambda i: _static_frame(),
        window_frame=_full_window_frame(),
    )
    assert cap.use_window_mode
    assert not cap.game_mode


def test_consistently_slow_printwindow_picks_game_mode(
    monkeypatch, patched_window
):
    _scripted_monotonic(
        monkeypatch, [0.0, 0.080, 0.100, 0.180, 0.200, 0.280]
    )
    cap = _make_auto(
        monkeypatch,
        screen_frames=lambda i: _static_frame(),
        window_frame=_full_window_frame(),
    )
    assert cap.game_mode
    assert not cap.use_window_mode


# ---- black PrintWindow frames at runtime (the VEIN silence bug) ----------

def test_game_mode_ignores_black_window_frames(monkeypatch, patched_window):
    # Probe: PrintWindow valid, screen churns -> game mode. At runtime the
    # game's PrintWindow starts returning all-BLACK frames (DX12 readback
    # failure). They are non-None and non-empty, but OCRing them reads
    # nothing forever — the grab must fall through to screen capture.
    cap = _make_auto(
        monkeypatch,
        screen_frames=lambda i: _static_frame((i * 16) % 256),
        window_frame=_full_window_frame(),
    )
    assert cap.game_mode
    monkeypatch.setattr(capture, "capture_window", lambda h: _full_window_frame(0))
    monkeypatch.setattr(capture, "_is_target_foreground", lambda h: True)
    frame = cap._grab()
    assert frame.any()          # the screen frame, not the black crop


def test_game_mode_skips_printwindow_when_probe_found_it_black(
    monkeypatch, patched_window
):
    # Probe already saw a black PrintWindow -> don't call it per-frame at
    # runtime (each call costs a GPU readback and can flicker the game).
    calls = {"n": 0}

    def counting_black(h):
        calls["n"] += 1
        return _full_window_frame(0)

    monkeypatch.setattr(capture, "capture_window", counting_black)
    caps_calls_during_probe = None
    cap = _make_auto(
        monkeypatch,
        screen_frames=lambda i: _static_frame((i * 16) % 256),
        window_frame=None,  # ignored; counting_black overrides below
    )
    # _make_auto re-patches capture_window; restore the counter for runtime.
    monkeypatch.setattr(capture, "capture_window", counting_black)
    monkeypatch.setattr(capture, "_is_target_foreground", lambda h: True)
    assert cap.game_mode
    calls["n"] = 0
    frame = cap._grab()
    assert frame.any()
    assert calls["n"] == 0      # PrintWindow not even attempted


def test_window_mode_black_blip_falls_through_to_screen(
    monkeypatch, patched_window
):
    cap = _make_auto(
        monkeypatch,
        screen_frames=lambda i: _static_frame(),
        window_frame=_full_window_frame(),
    )
    assert cap.use_window_mode
    monkeypatch.setattr(capture, "capture_window", lambda h: _full_window_frame(0))
    monkeypatch.setattr(capture, "_is_target_foreground", lambda h: True)
    assert cap._grab().any()
