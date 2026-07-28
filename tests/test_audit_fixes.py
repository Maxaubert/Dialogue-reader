"""Regression tests for the round-1 audit findings (issue #22)."""
import json
from types import SimpleNamespace

import main as main_mod
import ui.api as ui_api
from main import WatchedRegion, _active_regions, handle_command
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


def _region(name, process="", text="", mode="dialogue", hwnd=111):
    r = WatchedRegion(name=name, capture=_FakeCapture(hwnd=hwnd), mode=mode,
                      process=process)
    r.last_text = text
    return r


# ---- build_speech must respect per-process scoping -------------------------

def test_speech_excludes_background_process_regions():
    """A background game's stale text must never be glued onto the foreground
    app's line -- polling is scoped, so composing must be too."""
    fg = _region("dialogue1", process="vein.exe", text="The gate is sealed.")
    bg = _region("dialogue2", process="chrome.exe", text="Let's go.")
    glob = _region("dialogue3", process="", text="Global note.")
    speech = main_mod.build_speech(_active_regions([fg, bg, glob], "vein.exe"))
    assert "The gate is sealed." in speech
    assert "Global note." in speech
    assert "Let's go." not in speech


# The end-to-end version of the above (driving _apply_ocr_result) now lives in
# test_audit_round2.py::test_background_text_still_excluded. After issue #23 the
# batch carries the foreground it was BUILT from, so the result object needs an
# fg_exe field that this file's fake result predates.


# ---- PROFILE_APPLY must not collide labels with live regions ---------------

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


_OUTLINE = {"x": 0, "y": 0, "w": 400, "h": 150, "rotation": 0.0,
            "label": "dialogue1", "mode": "dialogue"}


def test_profile_apply_renames_labels_that_collide(tmp_path, monkeypatch):
    store, state, tts = _profile_env(tmp_path, monkeypatch)
    store.snapshot("vein", "vein.exe", (0, 0, 2000, 1000), [_OUTLINE])
    # Another game already owns the label "dialogue1".
    other = _region("dialogue1", process="persona5.exe")
    regions = [other]
    handle_command("PROFILE_APPLY:vein", regions, tts, SimpleNamespace(),
                   state, debug=False, profiles=store)
    names = [r.name for r in regions]
    assert len(names) == len(set(names)), f"duplicate labels: {names}"
    assert other in regions                      # the other game's box is intact


def test_profile_apply_keeps_label_when_free(tmp_path, monkeypatch):
    store, state, tts = _profile_env(tmp_path, monkeypatch)
    store.snapshot("vein", "vein.exe", (0, 0, 2000, 1000), [_OUTLINE])
    regions = []
    handle_command("PROFILE_APPLY:vein", regions, tts, SimpleNamespace(),
                   state, debug=False, profiles=store)
    assert [r.name for r in regions] == ["dialogue1"]


def test_applying_second_profile_for_same_process_clears_the_first(tmp_path, monkeypatch):
    store, state, tts = _profile_env(tmp_path, monkeypatch)
    store.snapshot("vein-a", "vein.exe", (0, 0, 2000, 1000), [_OUTLINE])
    store.snapshot("vein-b", "vein.exe", (0, 0, 2000, 1000), [_OUTLINE])
    regions = []
    handle_command("PROFILE_APPLY:vein-a", regions, tts, SimpleNamespace(),
                   state, debug=False, profiles=store)
    handle_command("PROFILE_APPLY:vein-b", regions, tts, SimpleNamespace(),
                   state, debug=False, profiles=store)
    assert store.get("vein-b")["applied"] is True
    assert store.get("vein-a")["applied"] is False


# ---- a bad command must never kill the reader ------------------------------

def test_handle_command_survives_a_handler_exception(tmp_path, monkeypatch):
    store, state, tts = _profile_env(tmp_path, monkeypatch)
    store.snapshot("vein", "vein.exe", (0, 0, 2000, 1000), [_OUTLINE])

    def boom(h):
        raise OSError("window vanished")

    monkeypatch.setattr(main_mod, "get_window_rect", boom)
    # Must not propagate: the poll loop would die and take the reader with it.
    handle_command("PROFILE_APPLY:vein", [], tts, SimpleNamespace(),
                   state, debug=False, profiles=store)


def test_profile_apply_tolerates_malformed_profile(tmp_path, monkeypatch):
    store, state, tts = _profile_env(tmp_path, monkeypatch)
    p = tmp_path / "profiles.json"
    p.write_text(json.dumps({"profiles": {"bad": {
        "process": "vein.exe", "window": {"w": 100, "h": 100},
        "regions": [{"label": "x", "w": 10}],      # missing rel_x/rel_y
        "apply_on_launch": True, "applied": False}}}), encoding="utf-8")
    store = ProfileStore(p)
    handle_command("PROFILE_APPLY:bad", [], tts, SimpleNamespace(),
                   state, debug=False, profiles=store)


# ---- foreground exe cache --------------------------------------------------

def test_failed_exe_lookup_is_not_cached(monkeypatch):
    """A transient lookup failure must not permanently mark the window as
    'no process' -- that silently deactivates the game's regions forever."""
    main_mod._EXE_CACHE.clear()
    monkeypatch.setattr(main_mod, "get_foreground_window", lambda: 777)
    monkeypatch.setattr(main_mod, "is_window", lambda h: True)
    answers = iter(["", "vein.exe"])
    monkeypatch.setattr(main_mod, "get_window_process", lambda h: next(answers))
    assert main_mod._foreground_exe() == ""
    assert main_mod._foreground_exe() == "vein.exe"   # retried, not stuck


def test_cache_entry_dropped_when_window_dies(monkeypatch):
    """HWNDs are recycled: a dead window's cached exe must not be served for
    whatever new window inherits the handle."""
    main_mod._EXE_CACHE.clear()
    monkeypatch.setattr(main_mod, "get_foreground_window", lambda: 777)
    alive = {"v": True}
    monkeypatch.setattr(main_mod, "is_window", lambda h: alive["v"])
    names = iter(["vein.exe", "notepad.exe"])
    monkeypatch.setattr(main_mod, "get_window_process", lambda h: next(names))
    assert main_mod._foreground_exe() == "vein.exe"
    alive["v"] = False                       # the game exits
    assert main_mod._foreground_exe() == "notepad.exe"


# ---- ini editing is case-insensitive ---------------------------------------

def test_update_ini_text_matches_keys_case_insensitively():
    """configparser and AHK's IniRead are both case-insensitive; writing a
    second differently-cased key produced a duplicate option that made the
    whole config unreadable."""
    ini = "[Media]\npauseduringspeech = true\n"
    out = ui_api.update_ini_text(ini, {("Media", "PauseDuringSpeech"): "false"})
    assert out.lower().count("pauseduringspeech") == 1
    assert "false" in out
    import configparser
    cp = configparser.ConfigParser()
    cp.read_string(out)                       # must not raise DuplicateOptionError
    assert cp.get("Media", "PauseDuringSpeech") == "false"


def test_update_ini_text_matches_section_case_insensitively():
    ini = "[media]\nResumeDelayMs=1000\n"
    out = ui_api.update_ini_text(ini, {("Media", "ResumeDelayMs"): "2500"})
    assert out.count("[media]") == 1
    assert "[Media]" not in out               # no duplicate section appended
    assert "ResumeDelayMs=2500" in out


# ---- restart_reader must not kill bystanders -------------------------------

class _FakeProc:
    def __init__(self, pid, name, cmdline):
        self.pid = pid
        self.info = {"name": name, "cmdline": cmdline, "exe": name}
        self.killed = False

    def kill(self):
        self.killed = True


def _restart_with(monkeypatch, procs):
    monkeypatch.setattr(ui_api.psutil, "process_iter", lambda attrs=None: list(procs))
    monkeypatch.setattr(ui_api.os, "startfile", lambda p: None)
    ui_api.restart_reader()
    return [p for p in procs if p.killed]


def test_restart_reader_spares_an_editor_with_the_script_open(monkeypatch):
    repo = str(ui_api._REPO)
    editor = _FakeProc(1, "notepad++.exe",
                       ["notepad++.exe", repo + r"\dialogue_reader.ahk"])
    ahk = _FakeProc(2, "AutoHotkey64.exe",
                    ["AutoHotkey64.exe", repo + r"\dialogue_reader.ahk"])
    reader = _FakeProc(3, "python.exe", ["python.exe", repo + r"\main.py",
                                         "--debug"])
    shell = _FakeProc(4, "pwsh.exe",
                      ["pwsh.exe", "-c", "gh issue create dialogue_reader.ahk"])
    killed = _restart_with(monkeypatch, [editor, ahk, reader, shell])
    assert editor not in killed
    assert shell not in killed
    assert ahk in killed and reader in killed


def test_restart_reader_ignores_a_same_named_script_elsewhere(monkeypatch):
    other = _FakeProc(5, "AutoHotkey64.exe",
                      [r"AutoHotkey64.exe", r"C:\other\dialogue_reader.ahk"])
    killed = _restart_with(monkeypatch, [other])
    assert killed == []


def test_restart_reader_survives_a_vanished_process(monkeypatch):
    class Vanishing(_FakeProc):
        @property
        def info(self):
            raise ui_api.psutil.NoSuchProcess(9)

        @info.setter
        def info(self, v):
            pass

    ahk = _FakeProc(2, "AutoHotkey64.exe",
                    ["AutoHotkey64.exe", str(ui_api._REPO) + r"\dialogue_reader.ahk"])
    killed = _restart_with(monkeypatch, [Vanishing(9, "x", []), ahk])
    assert ahk in killed
