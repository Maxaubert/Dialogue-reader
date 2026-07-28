"""Regression tests for the round-3 audit findings (issue #24)."""
import json
from types import SimpleNamespace

import pytest

import main as main_mod
import ui.api as ui_api
from main import WatchedRegion, handle_command
from profiles import ProfileStore, scale_regions


# ---- no test may command the live reader -----------------------------------

def test_conftest_blocks_commands_to_the_live_reader():
    """The autouse guard must be active: a test that forgets to patch
    send_command must not reach the reader the developer is using."""
    with pytest.raises(AssertionError, match="live reader"):
        ui_api.send_command("QUIT")


def test_restart_reader_still_sends_quit_when_patched(monkeypatch):
    sent = []
    monkeypatch.setattr(ui_api, "send_command", lambda cmd, **kw: sent.append(cmd))
    monkeypatch.setattr(ui_api.time, "sleep", lambda s: None)
    monkeypatch.setattr(ui_api, "reader_running", lambda **kw: False)
    monkeypatch.setattr(ui_api.psutil, "process_iter", lambda attrs=None: [])
    monkeypatch.setattr(ui_api.os, "startfile", lambda p: None)
    ui_api.restart_reader()
    assert sent == ["QUIT"]


# ---- a broken ini must not be healed by writing defaults over it -----------

_BROKEN = ("[Media]\nResumeDelayMs=1000\nresumedelayms=500\n\n"
           "[Voices]\nDefault=kokoro:bm_george\nPool=kokoro:af_sky\n")


def test_read_settings_recovers_values_after_a_duplicate(tmp_path):
    """configparser aborts at the duplicate and drops every later section, so
    the page showed SCHEMA defaults for them; saving then wrote those
    defaults over the user's real settings (issue #24)."""
    p = tmp_path / "d.ini"
    p.write_text(_BROKEN, encoding="utf-8")
    s = ui_api.read_settings(p)
    assert s["Voices"]["Default"] == "kokoro:bm_george"   # not the default
    assert s["Voices"]["Pool"] == "kokoro:af_sky"
    assert s["Media"]["ResumeDelayMs"] == 1000            # first wins


def test_save_after_a_duplicate_preserves_untouched_settings(tmp_path, monkeypatch):
    monkeypatch.setattr(ui_api, "send_command", lambda cmd, **kw: None)
    p = tmp_path / "d.ini"
    p.write_text(_BROKEN, encoding="utf-8")
    api = ui_api.Api(ini_path=p)
    state = api.get_state()
    api.save_settings(state["settings"])                  # a plain round-trip
    after = ui_api.read_settings(p)
    assert after["Voices"]["Default"] == "kokoro:bm_george"
    assert after["Voices"]["Pool"] == "kokoro:af_sky"
    import configparser
    configparser.ConfigParser().read_string(
        p.read_text(encoding="utf-8"))                    # healed on write


# ---- unapply must stick --------------------------------------------------

_OUTLINE = {"x": 0, "y": 0, "w": 400, "h": 150, "rotation": 0.0,
            "label": "dialogue1", "mode": "dialogue"}


class _FakeCapture:
    def __init__(self, region=(0, 0, 100, 50), hwnd=0, poll_hz=12.0,
                 stable_ms=350, verbose=False, rotation=0.0, capture_mode="auto"):
        self.region = region
        self.hwnd = hwnd
        self.rotation = rotation
        self.capture_mode = capture_mode
        self.bbox = {"left": region[0], "top": region[1],
                     "width": region[2], "height": region[3]}
        self.target_w, self.target_h = region[2], region[3]
        self.rel_x = self.rel_y = 0
        self.use_window_mode = False
        self.game_mode = False


def _profile_env(tmp_path, monkeypatch):
    monkeypatch.setattr(main_mod, "RegionCapture", _FakeCapture)
    monkeypatch.setattr(main_mod, "find_process_window", lambda exe: 999)
    monkeypatch.setattr(main_mod, "get_window_rect", lambda h: (0, 0, 2000, 1000))
    monkeypatch.setattr(main_mod, "is_window", lambda h: True)
    monkeypatch.setattr(main_mod, "_foreground_exe", lambda: "vein.exe")
    store = ProfileStore(tmp_path / "profiles.json")
    state = {"generation": 0, "capture_mode": "auto", "last_spoken": "",
             "candidate": "", "speaker_candidate": ""}
    tts = SimpleNamespace(speak=lambda *a, **k: None)
    return store, state, tts


def test_unapply_is_not_undone_by_the_watcher(tmp_path, monkeypatch):
    store, state, tts = _profile_env(tmp_path, monkeypatch)
    store.snapshot("vein", "vein.exe", (0, 0, 2000, 1000), [_OUTLINE])
    store.set_auto("vein", True)
    handle_command("PROFILE_APPLY:vein", [], tts, SimpleNamespace(),
                   state, debug=False, profiles=store)
    handle_command("PROFILE_UNAPPLY:vein", [], tts, SimpleNamespace(),
                   state, debug=False, profiles=store)
    # The game is still running; the watcher must NOT re-apply it.
    assert store.claim_auto({"vein.exe"}) == []


def test_unapply_still_re_applies_on_the_next_launch(tmp_path, monkeypatch):
    store, state, tts = _profile_env(tmp_path, monkeypatch)
    store.snapshot("vein", "vein.exe", (0, 0, 2000, 1000), [_OUTLINE])
    store.set_auto("vein", True)
    handle_command("PROFILE_UNAPPLY:vein", [], tts, SimpleNamespace(),
                   state, debug=False, profiles=store)
    store.mark_unapplied_for_process("vein.exe")        # game exits
    assert store.claim_auto({"vein.exe"}) == ["vein"]   # relaunch re-applies


def test_manual_apply_after_unapply_works(tmp_path, monkeypatch):
    store, state, tts = _profile_env(tmp_path, monkeypatch)
    store.snapshot("vein", "vein.exe", (0, 0, 2000, 1000), [_OUTLINE])
    store.set_auto("vein", True)
    handle_command("PROFILE_UNAPPLY:vein", [], tts, SimpleNamespace(),
                   state, debug=False, profiles=store)
    regions = []
    handle_command("PROFILE_APPLY:vein", regions, tts, SimpleNamespace(),
                   state, debug=False, profiles=store)
    assert [r.name for r in regions] == ["dialogue1"]


# ---- profile validation covers every field scale_regions touches -----------

def test_valid_rejects_bad_rotation_and_window(tmp_path):
    p = tmp_path / "profiles.json"
    p.write_text(json.dumps({"profiles": {
        "ok": {"process": "a.exe", "window": {"w": 100, "h": 100},
               "regions": [{"rel_x": 1, "rel_y": 2, "w": 3, "h": 4,
                            "label": "d1", "mode": "dialogue"}],
               "apply_on_launch": False, "applied": False},
        "bad_rotation": {"process": "b.exe", "window": {"w": 100, "h": 100},
                         "regions": [{"rel_x": 1, "rel_y": 2, "w": 3, "h": 4,
                                      "label": "d1", "rotation": "90"}],
                         "apply_on_launch": True, "applied": False},
        "bad_window": {"process": "c.exe", "window": {"w": 0, "h": 100},
                       "regions": [{"rel_x": 1, "rel_y": 2, "w": 3, "h": 4,
                                    "label": "d1"}],
                       "apply_on_launch": True, "applied": False},
        "window_not_dict": {"process": "d.exe", "window": "big",
                            "regions": [{"rel_x": 1, "rel_y": 2, "w": 3,
                                         "h": 4, "label": "d1"}],
                            "apply_on_launch": True, "applied": False},
    }}), encoding="utf-8")
    assert ProfileStore(p).names() == ["ok"]


def test_scale_regions_never_sees_a_value_valid_accepted(tmp_path):
    """Whatever _valid lets through must survive scale_regions arithmetic."""
    p = tmp_path / "profiles.json"
    p.write_text(json.dumps({"profiles": {"ok": {
        "process": "a.exe", "window": {"w": 1920, "h": 1080},
        "regions": [{"rel_x": 10, "rel_y": 20, "w": 30, "h": 40,
                     "label": "d1", "mode": "dialogue", "rotation": 12.5}],
        "apply_on_launch": False, "applied": False}}}), encoding="utf-8")
    out = scale_regions(ProfileStore(p).get("ok"), (0, 0, 960, 540))
    assert out[0]["w"] == 15 and out[0]["h"] == 20


# ---- restart_reader must not resolve relative args against ITS cwd ---------

class _FakeProc:
    def __init__(self, pid, name, cmdline):
        self.pid = pid
        self.info = {"name": name, "cmdline": cmdline, "exe": name}
        self.killed = False

    def kill(self):
        self.killed = True


def test_relative_main_py_in_another_project_is_spared(monkeypatch, tmp_path):
    """`python main.py` in an unrelated terminal must survive: a bare
    relative arg used to resolve against the settings app's own cwd, which
    IS the repo, and matched (issue #24)."""
    monkeypatch.chdir(ui_api._REPO)
    other = _FakeProc(1, "python.exe", ["python", "main.py"])
    ours = _FakeProc(2, "python.exe",
                     ["python.exe", str(ui_api._REPO / "main.py"), "--debug"])
    monkeypatch.setattr(ui_api, "send_command", lambda cmd, **kw: None)
    monkeypatch.setattr(ui_api.time, "sleep", lambda s: None)
    monkeypatch.setattr(ui_api, "reader_running", lambda **kw: False)
    monkeypatch.setattr(ui_api.psutil, "process_iter",
                        lambda attrs=None: [other, ours])
    monkeypatch.setattr(ui_api.os, "startfile", lambda p: None)
    ui_api.restart_reader()
    assert not other.killed, "killed an unrelated python main.py"
    assert ours.killed


# ---- outline positions must match the capture backend ----------------------

def test_outlines_are_absolute_for_screen_grabbing_regions(monkeypatch):
    """A game-mode region whose PrintWindow came back black captures at fixed
    screen coordinates, so its outline must NOT be reported window-relative
    (dragging the window would draw the box somewhere the capture isn't)."""
    cap = _FakeCapture(region=(100, 200, 400, 150), hwnd=555)
    cap.game_mode = True
    cap.use_window_mode = False
    cap._pw_usable = False              # PrintWindow unusable -> screen grabs
    r = WatchedRegion(name="dialogue1", capture=cap, mode="dialogue",
                      process="vein.exe")
    monkeypatch.setattr(main_mod, "get_window_rect",
                        lambda h: (9000, 9000, 800, 600))   # window moved far
    out = main_mod._existing_outlines([r])[0]
    assert (out["x"], out["y"]) == (100, 200)


def test_outlines_follow_the_window_for_window_mode_regions(monkeypatch):
    cap = _FakeCapture(region=(100, 200, 400, 150), hwnd=555)
    cap.use_window_mode = True
    cap.rel_x, cap.rel_y = 50, 60
    r = WatchedRegion(name="dialogue1", capture=cap, mode="dialogue",
                      process="vein.exe")
    monkeypatch.setattr(main_mod, "get_window_rect", lambda h: (1000, 500, 800, 600))
    out = main_mod._existing_outlines([r])[0]
    assert (out["x"], out["y"]) == (1050, 560)
