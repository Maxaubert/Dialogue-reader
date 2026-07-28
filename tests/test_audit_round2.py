"""Regression tests for the round-2 audit findings (issue #23)."""
import json
from types import SimpleNamespace

import pytest

import main as main_mod
import ui.api as ui_api
from main import WatchedRegion, handle_command
from profiles import ProfileStore


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


def _region(name, process="", text="", mode="dialogue"):
    r = WatchedRegion(name=name, capture=_FakeCapture(hwnd=111), mode=mode,
                      process=process)
    r.last_text = text
    return r


_OUTLINE = {"x": 0, "y": 0, "w": 400, "h": 150, "rotation": 0.0,
            "label": "dialogue1", "mode": "dialogue"}


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


# ---- speech must be scoped to the batch's foreground, not the current one ---

def test_focus_change_during_ocr_does_not_drop_the_line(monkeypatch):
    """The batch was OCR'd while the game was focused; if the user alt-tabs
    before the result lands, the line must still be spoken (the round-1
    scoping fix re-queried the foreground at result time and lost it)."""
    spoken = []
    tts = SimpleNamespace(
        speak=lambda text, voice=None, pause_media=True: spoken.append(text),
        stop=lambda: None)
    speaker_mgr = SimpleNamespace(voice_for_current=lambda: None,
                                  set_current=lambda *a, **k: None,
                                  current_speaker="")
    fg = _region("dialogue1", process="vein.exe")
    regions = [fg]
    state = {"generation": 0, "last_spoken": "", "candidate": "",
             "speaker_candidate": "", "paused": False}
    # The user is now on the browser; the batch was submitted from the game.
    monkeypatch.setattr(main_mod, "_foreground_exe", lambda: "chrome.exe")

    def poll():
        fg.has_pending_frame = True
        main_mod._apply_ocr_result(
            SimpleNamespace(generation=0, error=None,
                            texts={"dialogue1": "The gate is sealed."},
                            speaker_texts={}, fg_exe="vein.exe"),
            regions, state, speaker_mgr, tts, debug=False)

    poll()
    fg.last_text = ""
    poll()
    assert spoken, "line was dropped because focus moved during OCR"


def test_background_text_still_excluded(monkeypatch):
    """The round-1 fix must keep working: a region belonging to a process
    that was NOT the batch's foreground never joins the utterance."""
    spoken = []
    tts = SimpleNamespace(
        speak=lambda text, voice=None, pause_media=True: spoken.append(text),
        stop=lambda: None)
    speaker_mgr = SimpleNamespace(voice_for_current=lambda: None,
                                  set_current=lambda *a, **k: None,
                                  current_speaker="")
    fg = _region("dialogue1", process="vein.exe")
    bg = _region("dialogue2", process="chrome.exe", text="Background chatter.")
    regions = [fg, bg]
    state = {"generation": 0, "last_spoken": "", "candidate": "",
             "speaker_candidate": "", "paused": False}
    monkeypatch.setattr(main_mod, "_foreground_exe", lambda: "vein.exe")

    def poll():
        fg.has_pending_frame = True
        main_mod._apply_ocr_result(
            SimpleNamespace(generation=0, error=None,
                            texts={"dialogue1": "Foreground line."},
                            speaker_texts={}, fg_exe="vein.exe"),
            regions, state, speaker_mgr, tts, debug=False)

    poll()
    fg.last_text = ""
    poll()
    assert spoken and "Background chatter." not in spoken[-1]


# ---- profile apply must not destroy regions it cannot replace --------------

def test_failed_apply_leaves_existing_regions_intact(tmp_path, monkeypatch):
    store, state, tts = _profile_env(tmp_path, monkeypatch)
    store.snapshot("vein", "vein.exe", (0, 0, 2000, 1000), [_OUTLINE])

    def boom(*a, **k):
        raise RuntimeError("capture init failed")

    monkeypatch.setattr(main_mod, "RegionCapture", boom)
    live = _region("dialogue1", process="vein.exe")
    regions = [live]
    handle_command("PROFILE_APPLY:vein", regions, tts, SimpleNamespace(),
                   state, debug=False, profiles=store)
    assert regions == [live], "live regions destroyed by a failed apply"


def test_valid_rejects_non_numeric_coordinates(tmp_path):
    p = tmp_path / "profiles.json"
    p.write_text(json.dumps({"profiles": {
        "good": {"process": "a.exe", "window": {"w": 100, "h": 100},
                 "regions": [{"rel_x": 1, "rel_y": 2, "w": 3, "h": 4,
                              "label": "dialogue1", "mode": "dialogue"}],
                 "apply_on_launch": False, "applied": False},
        "stringy": {"process": "b.exe", "window": {"w": 100, "h": 100},
                    "regions": [{"rel_x": "120", "rel_y": 2, "w": 3, "h": 4,
                                 "label": "dialogue1"}],
                    "apply_on_launch": True, "applied": False},
        "missing": {"process": "c.exe", "window": {"w": 100, "h": 100},
                    "regions": [{"label": "x", "w": 10}],
                    "apply_on_launch": True, "applied": False},
    }}), encoding="utf-8")
    store = ProfileStore(p)
    assert store.names() == ["good"]


def test_store_drops_malformed_profiles_at_load(tmp_path):
    p = tmp_path / "profiles.json"
    p.write_text(json.dumps({"profiles": {
        "ok": {"process": "a.exe", "window": {"w": 10, "h": 10},
               "regions": [], "apply_on_launch": False, "applied": False},
        "no_process": {"window": {"w": 10, "h": 10}, "regions": []},
        "not_a_dict": ["nope"],
    }}), encoding="utf-8")
    store = ProfileStore(p)
    assert store.names() == ["ok"]
    assert store.get("no_process") is None


def test_corrupt_store_is_quarantined_not_silently_emptied(tmp_path):
    p = tmp_path / "profiles.json"
    p.write_text('{"profiles": {"vein": {', encoding="utf-8")
    store = ProfileStore(p)
    assert store.names() == []
    assert p.with_suffix(".corrupt").exists()


# ---- watcher must not stack duplicate applies ------------------------------

def test_auto_pending_claims_so_a_blocked_main_thread_gets_one_apply(tmp_path):
    store = ProfileStore(tmp_path / "profiles.json")
    store.snapshot("vein", "vein.exe", (0, 0, 2000, 1000), [_OUTLINE])
    store.set_auto("vein", True)
    running = {"vein.exe"}
    first = store.claim_auto(running)
    assert first == ["vein"]
    # Main thread is blocked (region manager open); the watcher ticks again.
    assert store.claim_auto(running) == []
    assert store.claim_auto(running) == []


def test_claim_is_released_when_the_apply_completes(tmp_path, monkeypatch):
    store, state, tts = _profile_env(tmp_path, monkeypatch)
    store.snapshot("vein", "vein.exe", (0, 0, 2000, 1000), [_OUTLINE])
    store.set_auto("vein", True)
    assert store.claim_auto({"vein.exe"}) == ["vein"]
    handle_command("PROFILE_APPLY:vein", [], tts, SimpleNamespace(),
                   state, debug=False, profiles=store)
    # Applied now, so nothing further is pending even after the claim clears.
    assert store.claim_auto({"vein.exe"}) == []


def test_claim_released_when_the_game_exits(tmp_path):
    store = ProfileStore(tmp_path / "profiles.json")
    store.snapshot("vein", "vein.exe", (0, 0, 2000, 1000), [_OUTLINE])
    store.set_auto("vein", True)
    assert store.claim_auto({"vein.exe"}) == ["vein"]
    store.mark_unapplied_for_process("vein.exe")     # game closed
    assert store.claim_auto({"vein.exe"}) == ["vein"]   # relaunch re-applies


# ---- the poll loop must survive a capture failure --------------------------

def test_region_poll_failure_does_not_kill_the_loop(monkeypatch):
    class Exploding:
        def poll_once(self):
            raise RuntimeError("gdi32.GetDIBits() failed")

    good = _region("dialogue1", process="vein.exe")
    good.capture = SimpleNamespace(poll_once=lambda: None)
    bad = _region("dialogue2", process="vein.exe")
    bad.capture = Exploding()
    # Must not raise, and must still poll the healthy regions.
    changed = main_mod._poll_regions([bad, good], debug=False)
    assert changed is False


# ---- graceful shutdown -----------------------------------------------------

def test_quit_command_unwinds_the_poll_loop():
    """QUIT must escape the poll loop's `except Exception` guard so the
    finally block runs: that is what resumes paused media and releases the
    singleton lock. KeyboardInterrupt (a BaseException) does exactly that."""
    tts = SimpleNamespace(speak=lambda *a, **k: None)
    with pytest.raises(KeyboardInterrupt):
        handle_command("QUIT", [], tts, SimpleNamespace(), {}, debug=False)

    # ...and a plain `except Exception` around dispatch must NOT swallow it.
    try:
        handle_command("QUIT", [], tts, SimpleNamespace(), {}, debug=False)
    except Exception:                       # noqa: BLE001 - the point of the test
        pytest.fail("QUIT was swallowed by an `except Exception` guard")
    except KeyboardInterrupt:
        pass


def test_restart_reader_asks_before_killing(monkeypatch):
    """restart_reader TerminateProcess-es the supervisor, which skips AHK's
    own cleanup, so it must send QUIT itself or media stays paused."""
    sent = []
    monkeypatch.setattr(ui_api, "send_command", lambda cmd, **kw: sent.append(cmd))
    monkeypatch.setattr(ui_api.psutil, "process_iter", lambda attrs=None: [])
    monkeypatch.setattr(ui_api.os, "startfile", lambda p: None)
    monkeypatch.setattr(ui_api.time, "sleep", lambda s: None)
    ui_api.restart_reader()
    assert "QUIT" in sent


# ---- ini healing -----------------------------------------------------------

def test_update_ini_text_heals_a_preexisting_duplicate_key():
    import configparser
    ini = ("[Media]\nResumeDelayMs=1000\nresumedelayms=500\n\n"
           "[Voices]\nDefault=kokoro:am_michael\n")
    with pytest.raises(configparser.DuplicateOptionError):
        configparser.ConfigParser().read_string(ini)   # broken before
    out = ui_api.update_ini_text(ini, {("Media", "ResumeDelayMs"): "2500"})
    cp = configparser.ConfigParser()
    cp.read_string(out)                                 # healed
    assert cp.get("Media", "ResumeDelayMs") == "2500"
    assert cp.get("Voices", "Default") == "kokoro:am_michael"


def test_read_settings_reports_a_broken_ini(tmp_path, capsys):
    p = tmp_path / "d.ini"
    p.write_text("[Media]\nResumeDelayMs=1000\nresumedelayms=500\n",
                 encoding="utf-8")
    ui_api.read_settings(p)
    assert "duplicate" in capsys.readouterr().out.lower()


# ---- only audio.py may import sounddevice ----------------------------------

def test_no_module_except_audio_imports_sounddevice():
    """Enforced at source level: a mock-based test cannot catch a new direct
    import, which is exactly how the native crash came back."""
    import ast
    from pathlib import Path
    repo = Path(main_mod.__file__).parent
    offenders = []
    for py in repo.glob("*.py"):
        if py.name == "audio.py":
            continue
        tree = ast.parse(py.read_text(encoding="utf-8"))
        for node in ast.walk(tree):
            if isinstance(node, ast.Import):
                names = [a.name for a in node.names]
            elif isinstance(node, ast.ImportFrom):
                names = [node.module or ""]
            else:
                continue
            if any(n.split(".")[0] == "sounddevice" for n in names):
                offenders.append(f"{py.name}:{node.lineno}")
    assert offenders == [], f"only audio.py may import sounddevice: {offenders}"
