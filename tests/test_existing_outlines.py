"""Tests for _existing_outlines — the watched-region outlines handed to the
region picker so the overlay can show what is already being watched."""
from types import SimpleNamespace

import main
from main import _existing_outlines


def _region(
    name="dialogue1",
    mode="dialogue",
    bbox=(100, 200, 400, 150),
    hwnd=0,
    capture_mode="auto",
    rotation=0.0,
    target=None,
    rel=(0, 0),
    use_window_mode=True,
    game_mode=False,
    pw_usable=True,
):
    x, y, w, h = bbox
    tw, th = target if target else (w, h)
    cap = SimpleNamespace(
        bbox={"left": x, "top": y, "width": w, "height": h},
        hwnd=hwnd,
        capture_mode=capture_mode,
        rotation=rotation,
        target_w=tw,
        target_h=th,
        rel_x=rel[0],
        rel_y=rel[1],
        # Which backend actually reads the pixels decides whether the outline
        # follows the window; being bound to an hwnd is not enough (#24).
        use_window_mode=use_window_mode,
        game_mode=game_mode,
        _pw_usable=pw_usable,
    )
    return SimpleNamespace(name=name, mode=mode, capture=cap)


def test_plain_region_uses_bbox():
    out = _existing_outlines([_region()])
    assert out == [
        {
            "x": 100, "y": 200, "w": 400, "h": 150,
            "rotation": 0.0, "label": "dialogue1", "mode": "dialogue",
        }
    ]


def test_window_region_follows_moved_window(monkeypatch):
    # Picked when the window was at (0, 0); window has since moved to
    # (50, 80). The outline must track it, like capture does.
    monkeypatch.setattr(main, "get_window_rect", lambda h: (50, 80, 800, 600))
    r = _region(bbox=(100, 200, 400, 150), hwnd=7, rel=(100, 200))
    out = _existing_outlines([r])
    assert out[0]["x"] == 150
    assert out[0]["y"] == 280


def test_forced_screen_region_never_follows(monkeypatch):
    monkeypatch.setattr(main, "get_window_rect", lambda h: (50, 80, 800, 600))
    r = _region(hwnd=7, capture_mode="screen", rel=(100, 200))
    out = _existing_outlines([r])
    assert (out[0]["x"], out[0]["y"]) == (100, 200)


def test_dead_window_falls_back_to_bbox(monkeypatch):
    def boom(h):
        raise OSError("window gone")

    monkeypatch.setattr(main, "get_window_rect", boom)
    r = _region(hwnd=7, rel=(100, 200))
    out = _existing_outlines([r])
    assert (out[0]["x"], out[0]["y"]) == (100, 200)


def test_rotated_region_reports_original_rect_and_rotation():
    # A rotated region's bbox is the padded axis-aligned bounding box; the
    # outline must be the user's original (smaller) rect centered inside
    # it, carrying the rotation.
    r = _region(
        bbox=(90, 190, 420, 170),  # padded bbox
        target=(400, 150),         # original drag dims
        rotation=10.0,
    )
    out = _existing_outlines([r])
    assert out[0]["w"] == 400
    assert out[0]["h"] == 150
    assert out[0]["rotation"] == 10.0
    # centered inside the bbox: center (300, 275) -> top-left (100, 200)
    assert (out[0]["x"], out[0]["y"]) == (100, 200)


def test_speaker_region_keeps_mode():
    out = _existing_outlines([_region(name="speaker1", mode="speaker")])
    assert out[0]["mode"] == "speaker"
    assert out[0]["label"] == "speaker1"


def test_game_mode_region_with_black_printwindow_does_not_follow(monkeypatch):
    # Game mode falls back to SCREEN grabs when the pick-time probe found
    # PrintWindow unusable, so those pixels come from fixed screen
    # coordinates and the outline must stay put (#24).
    monkeypatch.setattr(main, "get_window_rect", lambda h: (50, 80, 800, 600))
    r = _region(hwnd=7, rel=(100, 200), use_window_mode=False,
                game_mode=True, pw_usable=False)
    out = _existing_outlines([r])
    assert (out[0]["x"], out[0]["y"]) == (100, 200)


def test_game_mode_region_with_working_printwindow_follows(monkeypatch):
    monkeypatch.setattr(main, "get_window_rect", lambda h: (50, 80, 800, 600))
    r = _region(hwnd=7, rel=(100, 200), use_window_mode=False,
                game_mode=True, pw_usable=True)
    out = _existing_outlines([r])
    assert (out[0]["x"], out[0]["y"]) == (150, 280)
