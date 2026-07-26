"""Per-process region scoping: regions bind to the exe of the window they
were drawn over; F1 manages only the focused process's regions; only the
foreground process's regions (plus globals) are polled; dead windows prune."""
from types import SimpleNamespace

import main as main_mod
from main import (
    WatchedRegion,
    _active_regions,
    _prune_dead_regions,
    handle_command,
    open_region_manager,
)


class _FakeCapture:
    def __init__(self, region=(0, 0, 100, 50), hwnd=0, poll_hz=12.0,
                 stable_ms=350, verbose=False, rotation=0.0,
                 capture_mode="auto"):
        self.region = region
        self.hwnd = hwnd
        self.rotation = rotation
        self.capture_mode = capture_mode
        self.bbox = {"left": region[0], "top": region[1],
                     "width": region[2], "height": region[3]}
        self.target_w = region[2]
        self.target_h = region[3]
        self.rel_x = self.rel_y = 0
        self.use_window_mode = False
        self.game_mode = False


def _region(name, process="", hwnd=0, mode="dialogue"):
    return WatchedRegion(name=name, capture=_FakeCapture(hwnd=hwnd),
                         mode=mode, process=process)


def _state():
    return {"generation": 0, "capture_mode": "auto", "last_spoken": "",
            "candidate": "", "speaker_candidate": ""}


# ---- _active_regions -------------------------------------------------------

def test_active_regions_filters_by_foreground_exe():
    game = _region("dialogue1", process="vein.exe")
    browser = _region("dialogue2", process="chrome.exe")
    glob = _region("dialogue3", process="")
    active = _active_regions([game, browser, glob], "vein.exe")
    assert active == [game, glob]


def test_active_regions_desktop_focus_keeps_only_globals():
    game = _region("dialogue1", process="vein.exe")
    glob = _region("dialogue2", process="")
    assert _active_regions([game, glob], "") == [glob]


# ---- _prune_dead_regions ---------------------------------------------------

def test_prune_removes_regions_with_dead_windows(monkeypatch):
    monkeypatch.setattr(main_mod, "is_window", lambda h: h == 111)
    alive = _region("dialogue1", process="vein.exe", hwnd=111)
    dead = _region("dialogue2", process="gone.exe", hwnd=222)
    glob = _region("dialogue3", process="", hwnd=0)
    regions = [alive, dead, glob]
    state = _state()
    _prune_dead_regions(regions, state)
    assert regions == [alive, glob]
    assert state["generation"] == 1


def test_prune_noop_when_all_alive(monkeypatch):
    monkeypatch.setattr(main_mod, "is_window", lambda h: True)
    regions = [_region("dialogue1", process="vein.exe", hwnd=111)]
    state = _state()
    _prune_dead_regions(regions, state)
    assert len(regions) == 1
    assert state["generation"] == 0


# ---- manager scoping -------------------------------------------------------

def _patch_manager(monkeypatch, fake_manage):
    monkeypatch.setattr(main_mod, "RegionCapture", _FakeCapture)
    monkeypatch.setattr(main_mod, "manage_regions", fake_manage)
    monkeypatch.setattr(main_mod, "find_window_at", lambda x, y: 555)
    monkeypatch.setattr(main_mod, "get_window_process", lambda h: "vein.exe")


def test_manager_shows_only_target_process_regions(monkeypatch):
    seen = {}

    def fake_manage(outlines, mode="dialogue", poll_commands=None,
                    reserved_labels=None):
        seen["outlines"] = [o["label"] for o in outlines]
        seen["reserved"] = set(reserved_labels or [])
        return SimpleNamespace(outlines=outlines, deleted=[], unhandled=[])

    _patch_manager(monkeypatch, fake_manage)
    regions = [
        _region("dialogue1", process="vein.exe"),
        _region("dialogue2", process="chrome.exe"),
    ]
    open_region_manager(regions, debug=False, mode="dialogue",
                        state=_state(), target_process="vein.exe")
    assert seen["outlines"] == ["dialogue1"]
    # Hidden chrome region's label must stay reserved against collisions.
    assert "dialogue2" in seen["reserved"]


def test_manager_created_region_gets_process(monkeypatch):
    def fake_manage(outlines, mode="dialogue", poll_commands=None,
                    reserved_labels=None):
        outlines = list(outlines)
        outlines.append({
            "x": 5, "y": 6, "w": 300, "h": 90, "rotation": 0.0,
            "label": "dialogue1", "mode": "dialogue",
            "created": True, "hwnd": 555,
        })
        return SimpleNamespace(outlines=outlines, deleted=[], unhandled=[])

    _patch_manager(monkeypatch, fake_manage)
    regions = []
    open_region_manager(regions, debug=False, mode="dialogue",
                        state=_state(), target_process="vein.exe")
    assert regions[0].process == "vein.exe"


def test_manager_blank_for_unseen_process_keeps_others(monkeypatch):
    def fake_manage(outlines, mode="dialogue", poll_commands=None,
                    reserved_labels=None):
        assert outlines == []              # blank canvas for the new app
        return SimpleNamespace(outlines=[], deleted=[], unhandled=[])

    _patch_manager(monkeypatch, fake_manage)
    keep = _region("dialogue1", process="vein.exe")
    regions = [keep]
    open_region_manager(regions, debug=False, mode="dialogue",
                        state=_state(), target_process="chrome.exe")
    assert regions == [keep]               # untouched


# ---- scoped clear ----------------------------------------------------------

def test_clear_regions_only_foreground_process(monkeypatch):
    monkeypatch.setattr(main_mod, "_foreground_exe", lambda: "vein.exe")
    game = _region("dialogue1", process="vein.exe")
    browser = _region("dialogue2", process="chrome.exe")
    regions = [game, browser]
    state = _state()
    state["paused"] = False
    handle_command("CLEAR_REGIONS", regions, SimpleNamespace(), SimpleNamespace(),
                   state, debug=False)
    assert regions == [browser]
