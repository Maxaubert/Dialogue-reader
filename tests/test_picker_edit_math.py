"""Tests for the outline edit geometry: handle hit-testing and anchored
move/resize math (Word/Docs-style image handles)."""
import math

import pytest

from region_picker import _apply_edit, _hit_test

RECT = (100.0, 200.0, 400.0, 150.0)  # x, y, w, h


# ---- _hit_test ------------------------------------------------------------

def test_corner_handles():
    assert _hit_test(RECT, 0.0, (100, 200)) == "nw"
    assert _hit_test(RECT, 0.0, (500, 200)) == "ne"
    assert _hit_test(RECT, 0.0, (500, 350)) == "se"
    assert _hit_test(RECT, 0.0, (100, 350)) == "sw"


def test_side_midpoint_handles():
    assert _hit_test(RECT, 0.0, (300, 200)) == "n"   # top mid: height only
    assert _hit_test(RECT, 0.0, (300, 350)) == "s"
    assert _hit_test(RECT, 0.0, (100, 275)) == "w"   # left mid: width only
    assert _hit_test(RECT, 0.0, (500, 275)) == "e"


def test_edge_line_away_from_handles_is_move():
    assert _hit_test(RECT, 0.0, (200, 200)) == "move"   # top edge
    assert _hit_test(RECT, 0.0, (500, 240)) == "move"   # right edge
    assert _hit_test(RECT, 0.0, (200, 350)) == "move"   # bottom edge


def test_interior_and_far_outside_are_none():
    assert _hit_test(RECT, 0.0, (300, 275)) is None  # deep interior
    assert _hit_test(RECT, 0.0, (700, 500)) is None  # outside


def test_handle_hit_radius_beats_edge():
    # A point near-but-not-exactly-on a handle still resolves to the handle.
    assert _hit_test(RECT, 0.0, (494, 205)) == "ne"


def test_rotated_rect_hit_test():
    # Square rotated 90 deg about its center (300, 275): the rotated top-mid
    # handle now sits to the RIGHT of the center in screen space.
    rect = (250.0, 225.0, 100.0, 100.0)
    assert _hit_test(rect, 90.0, (350, 275)) == "n"
    # And the un-rotated top-mid location is now the "w" handle's spot.
    assert _hit_test(rect, 90.0, (300, 225)) == "w"


# ---- _apply_edit ----------------------------------------------------------

def test_move_translates():
    assert _apply_edit(RECT, 0.0, "move", 30, -10) == (130, 190, 400, 150)


def test_resize_east_keeps_left_edge():
    x, y, w, h = _apply_edit(RECT, 0.0, "e", 50, 0)
    assert (x, y) == (100, 200)
    assert (w, h) == (450, 150)


def test_resize_west_keeps_right_edge():
    x, y, w, h = _apply_edit(RECT, 0.0, "w", -50, 0)
    assert x == pytest.approx(50)
    assert w == pytest.approx(450)
    assert x + w == pytest.approx(500)  # right edge unchanged
    assert (y, h) == (200, 150)


def test_resize_north_keeps_bottom_edge():
    x, y, w, h = _apply_edit(RECT, 0.0, "n", 0, -30)
    assert y == pytest.approx(170)
    assert h == pytest.approx(180)
    assert y + h == pytest.approx(350)
    assert (x, w) == (100, 400)


def test_resize_corner_se_keeps_nw_corner():
    x, y, w, h = _apply_edit(RECT, 0.0, "se", 40, 20)
    assert (x, y) == (100, 200)
    assert (w, h) == (440, 170)


def test_side_handles_ignore_perpendicular_motion():
    # Dragging "e" vertically must not change height or y.
    x, y, w, h = _apply_edit(RECT, 0.0, "e", 0, 80)
    assert (x, y, w, h) == (100, 200, 400, 150)


def test_min_size_clamp():
    x, y, w, h = _apply_edit(RECT, 0.0, "e", -1000, 0)
    assert w == 20.0
    assert x == 100  # left edge still anchored


def test_rotated_resize_follows_local_axis():
    # Square rotated 90 deg CW: its local "e" handle points DOWN in screen
    # space, so dragging it down grows the local width.
    rect = (250.0, 225.0, 100.0, 100.0)
    x, y, w, h = _apply_edit(rect, 90.0, "e", 0, 40)
    assert w == pytest.approx(140)
    assert h == pytest.approx(100)
    # The opposite (local-west) edge is anchored: center moved down by 20.
    cx, cy = x + w / 2, y + h / 2
    assert cx == pytest.approx(300)
    assert cy == pytest.approx(295)
