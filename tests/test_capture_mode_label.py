"""Tests for the capture-mode description shown when a region is added."""
from types import SimpleNamespace

from main import _capture_mode_label


def _cap(capture_mode="auto", use_window_mode=False, game_mode=False):
    return SimpleNamespace(
        capture_mode=capture_mode,
        use_window_mode=use_window_mode,
        game_mode=game_mode,
    )


def test_forced_screen():
    assert "SCREEN" in _capture_mode_label(_cap(capture_mode="screen"))
    assert "forced" in _capture_mode_label(_cap(capture_mode="screen"))


def test_forced_window():
    assert "WINDOW" in _capture_mode_label(_cap(capture_mode="window"))
    assert "forced" in _capture_mode_label(_cap(capture_mode="window"))


def test_auto_window():
    label = _capture_mode_label(_cap(use_window_mode=True))
    assert "WINDOW" in label
    assert "forced" not in label


def test_auto_game_mode_mentions_animation():
    label = _capture_mode_label(_cap(game_mode=True))
    assert "GAME" in label
    assert "animated" in label.lower()


def test_auto_screen_gate():
    assert "SCREEN" in _capture_mode_label(_cap())
