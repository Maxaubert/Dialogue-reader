"""Game profiles: window-relative snapshots of a process's regions that can
be re-applied (scaled to the current window) and auto-applied on launch."""
from types import SimpleNamespace

import main as main_mod
from main import WatchedRegion, handle_command
from profiles import ProfileStore, scale_regions


class _FakeCapture:
    def __init__(self, region=(100, 200, 400, 150), hwnd=111, poll_hz=12.0,
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


def _region(name="dialogue1", process="vein.exe", hwnd=111,
            rect=(100, 200, 400, 150), mode="dialogue"):
    return WatchedRegion(name=name, capture=_FakeCapture(region=rect, hwnd=hwnd),
                         mode=mode, process=process)


def _store(tmp_path):
    return ProfileStore(tmp_path / "profiles.json")


_OUTLINE = {"x": 100, "y": 200, "w": 400, "h": 150, "rotation": 0.0,
            "label": "dialogue1", "mode": "dialogue"}


# ---- store -----------------------------------------------------------------

def test_snapshot_stores_window_relative(tmp_path):
    s = _store(tmp_path)
    s.snapshot("vein", "vein.exe", (50, 60, 2000, 1000), [_OUTLINE])
    p = s.get("vein")
    assert p["process"] == "vein.exe"
    assert p["window"] == {"w": 2000, "h": 1000}
    r = p["regions"][0]
    assert (r["rel_x"], r["rel_y"], r["w"], r["h"]) == (50, 140, 400, 150)
    assert r["label"] == "dialogue1"
    assert p["apply_on_launch"] is False
    assert p["applied"] is False


def test_store_persists_and_reloads(tmp_path):
    s = _store(tmp_path)
    s.snapshot("vein", "vein.exe", (0, 0, 2000, 1000), [_OUTLINE])
    s.set_auto("vein", True)
    s2 = _store(tmp_path)
    assert s2.get("vein")["apply_on_launch"] is True


def test_reset_applied_on_startup(tmp_path):
    s = _store(tmp_path)
    s.snapshot("vein", "vein.exe", (0, 0, 2000, 1000), [_OUTLINE])
    s.set_applied("vein", True)
    s2 = _store(tmp_path)
    s2.reset_applied()
    assert s2.get("vein")["applied"] is False


def test_delete(tmp_path):
    s = _store(tmp_path)
    s.snapshot("vein", "vein.exe", (0, 0, 2000, 1000), [_OUTLINE])
    s.delete("vein")
    assert s.get("vein") is None
    assert s.names() == []


def test_mark_unapplied_for_process(tmp_path):
    s = _store(tmp_path)
    s.snapshot("vein", "vein.exe", (0, 0, 2000, 1000), [_OUTLINE])
    s.set_applied("vein", True)
    s.mark_unapplied_for_process("vein.exe")
    assert s.get("vein")["applied"] is False


def test_auto_pending(tmp_path):
    s = _store(tmp_path)
    s.snapshot("vein", "vein.exe", (0, 0, 2000, 1000), [_OUTLINE])
    s.snapshot("p5", "persona5.exe", (0, 0, 2000, 1000), [_OUTLINE])
    s.set_auto("vein", True)
    assert s.auto_pending({"vein.exe", "chrome.exe"}) == ["vein"]
    s.set_applied("vein", True)
    assert s.auto_pending({"vein.exe"}) == []
    assert s.auto_pending({"persona5.exe"}) == []   # auto not enabled


# ---- scaling ---------------------------------------------------------------

def test_scale_regions_same_size_restores_absolute(tmp_path):
    s = _store(tmp_path)
    s.snapshot("vein", "vein.exe", (50, 60, 2000, 1000), [_OUTLINE])
    out = scale_regions(s.get("vein"), (50, 60, 2000, 1000))
    assert (out[0]["x"], out[0]["y"], out[0]["w"], out[0]["h"]) == (100, 200, 400, 150)


def test_scale_regions_scales_to_new_window_size(tmp_path):
    s = _store(tmp_path)
    s.snapshot("vein", "vein.exe", (0, 0, 2000, 1000), [_OUTLINE])
    out = scale_regions(s.get("vein"), (0, 0, 1000, 500))   # half size
    assert (out[0]["x"], out[0]["y"], out[0]["w"], out[0]["h"]) == (50, 100, 200, 75)


# ---- commands --------------------------------------------------------------

def _cmd_env(tmp_path, monkeypatch, fg="vein.exe"):
    monkeypatch.setattr(main_mod, "RegionCapture", _FakeCapture)
    monkeypatch.setattr(main_mod, "_foreground_exe", lambda: fg)
    monkeypatch.setattr(main_mod, "get_window_rect",
                        lambda h: (50, 60, 2000, 1000))
    monkeypatch.setattr(main_mod, "find_process_window", lambda exe: 999)
    monkeypatch.setattr(main_mod, "is_window", lambda h: True)
    store = _store(tmp_path)
    state = {"generation": 0, "capture_mode": "auto", "last_spoken": "",
             "candidate": "", "speaker_candidate": ""}
    tts = SimpleNamespace(
        speak=lambda text, voice=None, pause_media=True: None)
    return store, state, tts


def test_profile_save_snapshots_foreground_process(tmp_path, monkeypatch):
    store, state, tts = _cmd_env(tmp_path, monkeypatch)
    regions = [_region(), _region("dialogue2", process="chrome.exe", hwnd=222)]
    handle_command("PROFILE_SAVE:vein", regions, tts, SimpleNamespace(),
                   state, debug=False, profiles=store)
    p = store.get("vein")
    assert p is not None
    assert p["process"] == "vein.exe"
    assert len(p["regions"]) == 1                   # chrome region excluded


def test_profile_apply_replaces_process_regions(tmp_path, monkeypatch):
    store, state, tts = _cmd_env(tmp_path, monkeypatch)
    store.snapshot("vein", "vein.exe", (50, 60, 2000, 1000), [_OUTLINE])
    stale = _region("dialogue9", process="vein.exe")
    other = _region("dialogue2", process="chrome.exe", hwnd=222)
    regions = [stale, other]
    handle_command("PROFILE_APPLY:vein", regions, tts, SimpleNamespace(),
                   state, debug=False, profiles=store)
    names = [r.name for r in regions]
    assert "dialogue9" not in names                 # replaced
    assert "dialogue2" in names                     # other process untouched
    assert any(r.process == "vein.exe" and r.name == "dialogue1"
               for r in regions)
    assert store.get("vein")["applied"] is True
    assert state["generation"] == 1


def test_profile_apply_game_not_running(tmp_path, monkeypatch):
    store, state, tts = _cmd_env(tmp_path, monkeypatch)
    monkeypatch.setattr(main_mod, "find_process_window", lambda exe: 0)
    store.snapshot("vein", "vein.exe", (0, 0, 2000, 1000), [_OUTLINE])
    regions = []
    handle_command("PROFILE_APPLY:vein", regions, tts, SimpleNamespace(),
                   state, debug=False, profiles=store)
    assert regions == []
    assert store.get("vein")["applied"] is False


def test_profile_unapply_removes_regions(tmp_path, monkeypatch):
    store, state, tts = _cmd_env(tmp_path, monkeypatch)
    store.snapshot("vein", "vein.exe", (0, 0, 2000, 1000), [_OUTLINE])
    store.set_applied("vein", True)
    regions = [_region()]
    handle_command("PROFILE_UNAPPLY:vein", regions, tts, SimpleNamespace(),
                   state, debug=False, profiles=store)
    assert regions == []
    assert store.get("vein")["applied"] is False


def test_profile_auto_toggle(tmp_path, monkeypatch):
    store, state, tts = _cmd_env(tmp_path, monkeypatch)
    store.snapshot("vein", "vein.exe", (0, 0, 2000, 1000), [_OUTLINE])
    handle_command("PROFILE_AUTO:vein:1", [], tts, SimpleNamespace(),
                   state, debug=False, profiles=store)
    assert store.get("vein")["apply_on_launch"] is True
    handle_command("PROFILE_AUTO:vein:0", [], tts, SimpleNamespace(),
                   state, debug=False, profiles=store)
    assert store.get("vein")["apply_on_launch"] is False


def test_profile_delete_command(tmp_path, monkeypatch):
    store, state, tts = _cmd_env(tmp_path, monkeypatch)
    store.snapshot("vein", "vein.exe", (0, 0, 2000, 1000), [_OUTLINE])
    handle_command("PROFILE_DELETE:vein", [], tts, SimpleNamespace(),
                   state, debug=False, profiles=store)
    assert store.get("vein") is None
