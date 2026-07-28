"""Regression tests for the round-4 audit findings (issue #25)."""
from types import SimpleNamespace

import numpy as np

import main as main_mod
import ui.api as ui_api
from main import WatchedRegion
from ocr import OCRBatchJob, OCRRegionSpec, OCRWorker
from profiles import ProfileStore


# ---- "confirmed" must mean the text HELD, not that we tried ---------------

class _ChangingCapture:
    """Pixels differ on every snapshot: the confirm loop can never converge."""

    def __init__(self):
        self.n = 0

    def snapshot(self):
        self.n += 1
        # Big enough to survive the hash's ::8 downsample, and far enough
        # apart to survive its >>3 quantization, so each frame really differs.
        return np.full((32, 32, 3), (self.n * 40) % 255, dtype=np.uint8)


class _StillCapture:
    def snapshot(self):
        return np.zeros((4, 4, 3), dtype=np.uint8)


class _TypewriterOCR:
    """Text that never settles: every read differs, like a long typewriter
    animation the confirm loop gives up on."""

    def __init__(self):
        self.n = 0

    def read(self, frame, speaker=False):
        self.n += 1
        return "The king said" + "." * self.n


def _job(capture, polls=2):
    return OCRBatchJob(
        generation=0,
        regions=[OCRRegionSpec(name="dialogue1", mode="dialogue",
                               capture=capture)],
        confirm_polls=polls, debug=False,
        pre_snapshot_delay=0.0, confirm_interval=0.0,
    )


def test_unconverged_text_is_not_reported_as_confirmed():
    res = OCRWorker(_TypewriterOCR())._process(_job(_ChangingCapture()))
    assert res.confirmed == set(), (
        "mid-animation text was reported as confirmed, which disables the "
        "jitter guard and speaks partial lines")


def test_steady_text_is_reported_as_confirmed():
    class Steady:
        def read(self, frame, speaker=False):
            return "A complete line."

    res = OCRWorker(Steady())._process(_job(_StillCapture()))
    assert res.confirmed == {"dialogue1"}


def _apply(confirmed, texts=None, speaker_texts=None, region_names=("dialogue1",),
           speaker_region=False, current_speaker=""):
    spoken = []
    tts = SimpleNamespace(
        speak=lambda text, voice=None, pause_media=True: spoken.append(text),
        stop=lambda: None)
    seen = {"name": current_speaker}

    def set_current(name, fuzzy=True):
        seen["name"] = name
        return "kokoro:af_heart"

    speaker_mgr = SimpleNamespace(
        voice_for_current=lambda: None,
        set_current=set_current,
        current_speaker=current_speaker)
    regions = []
    for n in region_names:
        cap = SimpleNamespace(
            hwnd=0, capture_mode="window", use_window_mode=True,
            game_mode=False, rotation=0.0, rel_x=0, rel_y=0,
            bbox={"left": 0, "top": 0, "width": 10, "height": 10},
            target_w=10, target_h=10, poll_once=lambda: None)
        r = WatchedRegion(name=n, capture=cap,
                          mode="speaker" if n.startswith("speaker") else "dialogue",
                          process="game.exe")
        r.has_pending_frame = True
        regions.append(r)
    state = {"generation": 0, "last_spoken": "", "candidate": "",
             "speaker_candidate": "", "paused": False}
    result = SimpleNamespace(
        generation=0, error=None, fg_exe="game.exe",
        texts=texts or {"dialogue1": "A complete line."},
        speaker_texts=speaker_texts or {},
        confirmed=confirmed)
    main_mod._apply_ocr_result(result, regions, state, speaker_mgr, tts,
                               debug=False)
    return spoken, state, seen


def test_partial_text_is_held_back_when_not_confirmed():
    spoken, state, _ = _apply(confirmed=set(),
                              texts={"dialogue1": "The king said you sh"})
    assert spoken == []
    assert state["candidate"] == "The king said you sh"


def test_confirmed_text_is_spoken_immediately():
    spoken, _, _ = _apply(confirmed={"dialogue1"})
    assert spoken == ["A complete line."]


def test_confirmation_of_another_region_does_not_disarm_the_guard():
    """The guard must key on the regions that actually composed the speech,
    not on the set being non-empty."""
    spoken, _, _ = _apply(confirmed={"dialogue2"},
                          texts={"dialogue1": "A complete line."})
    assert spoken == []


# ---- the first line after a speaker change must not be dropped ------------

def test_speaker_change_does_not_drop_a_confirmed_line():
    """Hash-gated regions yield one frame per stable image, so 'hold one more
    poll' for a pending speaker means the line is never spoken (issue #25).
    A worker-confirmed batch carries the speaker too: use it."""
    spoken, _, seen = _apply(
        confirmed={"dialogue1"},
        # The real worker returns every region's text in one dict.
        texts={"speaker1": "Nanako", "dialogue1": "Hello traveller."},
        region_names=("speaker1", "dialogue1"),
        current_speaker="Dojima")
    assert spoken == ["Hello traveller."]
    assert seen["name"] == "Nanako", "line was attributed to the old speaker"


# ---- an unapplied profile must re-arm when the game exits -----------------

_OUTLINE = {"x": 0, "y": 0, "w": 400, "h": 150, "rotation": 0.0,
            "label": "dialogue1", "mode": "dialogue"}


def _store(tmp_path):
    s = ProfileStore(tmp_path / "profiles.json")
    s.snapshot("vein", "vein.exe", (0, 0, 2000, 1000), [_OUTLINE])
    s.set_auto("vein", True)
    return s


def test_unapplied_profile_re_arms_after_the_game_exits(tmp_path):
    s = _store(tmp_path)
    s.set_applied_exclusive("vein", "vein.exe")     # auto-applied
    s.set_applied("vein", False)                    # user clicks Unapply
    assert s.claim_auto({"vein.exe"}) == []         # stays off while running
    s.note_process_gone("vein.exe")                 # game closed
    assert s.claim_auto({"vein.exe"}) == ["vein"]   # relaunch re-applies


def test_note_process_gone_is_safe_for_unknown_processes(tmp_path):
    s = _store(tmp_path)
    s.note_process_gone("something-else.exe")
    assert s.get("vein")["apply_on_launch"] is True


def test_watcher_re_arms_profiles_whose_game_vanished(tmp_path, monkeypatch):
    """The production path: the watcher must notify the store about EVERY
    profile whose process is gone, not only ones still flagged applied."""
    s = _store(tmp_path)
    s.set_applied_exclusive("vein", "vein.exe")
    s.set_applied("vein", False)
    main_mod._profile_watch_tick(s, running={"chrome.exe"}, enqueue=lambda c: None)
    assert s.claim_auto({"vein.exe"}) == ["vein"]


def test_watcher_enqueues_one_apply(tmp_path, monkeypatch):
    s = _store(tmp_path)
    monkeypatch.setattr(main_mod, "find_process_window", lambda exe: 999)
    sent = []
    main_mod._profile_watch_tick(s, running={"vein.exe"}, enqueue=sent.append)
    main_mod._profile_watch_tick(s, running={"vein.exe"}, enqueue=sent.append)
    assert sent == ["PROFILE_APPLY:vein"]


# ---- the test guard must actually stop restart_reader ---------------------

def test_conftest_guard_blocks_the_whole_restart_path():
    """restart_reader swallows exceptions, so guarding send_command alone let
    an unpatched test still kill the live supervisor (issue #25)."""
    import pytest
    with pytest.raises(AssertionError):
        ui_api.restart_reader()


# ---- shutdown must not block unbounded on the work lock ------------------

def test_shutdown_gives_up_on_a_wedged_pause_worker():
    import threading
    import time
    from media_gate import MediaGate

    s = SimpleNamespace(app_id="spotify", is_playing=True, is_paused=False,
                        can_pause=True, can_play=True,
                        pause=lambda: None, play=lambda: None)
    release = threading.Event()

    def wedged():
        release.wait(timeout=5.0)
        return [s]

    gate = MediaGate(resume_delay_ms=10_000, session_source=wedged)
    gate.speech_started()
    time.sleep(0.1)
    t0 = time.monotonic()
    gate.shutdown()
    elapsed = time.monotonic() - t0
    release.set()
    assert elapsed < 2.5, f"shutdown blocked {elapsed:.1f}s on the work lock"
