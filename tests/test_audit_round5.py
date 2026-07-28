"""Regression tests for the round-5 audit findings (issue #26)."""
import threading
import time
from types import SimpleNamespace

import cv2
import numpy as np

import main as main_mod
from capture import _deskew_to_target
from media_gate import MediaGate
from ocr import OCRBatchJob, OCRRegionSpec, OCRWorker
from profiles import ProfileStore


# ---- rotated regions must be deskewed, not double-skewed ------------------

def _bar_angle(img: np.ndarray) -> float:
    """Angle of the dominant bright bar, in degrees, positive = clockwise
    on screen (y grows downward)."""
    ys, xs = np.nonzero(img[:, :, 0] > 128)
    if len(xs) < 10:
        return 0.0
    # Principal axis of the bright pixels.
    pts = np.stack([xs - xs.mean(), ys - ys.mean()])
    cov = pts @ pts.T / pts.shape[1]
    evals, evecs = np.linalg.eigh(cov)
    vx, vy = evecs[:, np.argmax(evals)]
    a = float(np.degrees(np.arctan2(vy, vx)))
    return (a + 90.0) % 180.0 - 90.0    # an axis has no direction: wrap to +-90


def _tilted_bar(tilt_cw_deg: float) -> np.ndarray:
    """A horizontal white bar rotated CLOCKWISE on screen by tilt_cw_deg,
    i.e. what the camera sees when the dialogue box is tilted."""
    img = np.zeros((200, 200, 3), dtype=np.uint8)
    cv2.rectangle(img, (40, 95), (160, 105), (255, 255, 255), -1)
    # OpenCV's positive angle is counter-clockwise, so negate for clockwise.
    m = cv2.getRotationMatrix2D((100.0, 100.0), -tilt_cw_deg, 1.0)
    return cv2.warpAffine(img, m, (200, 200))


def test_tilted_bar_helper_is_sane():
    assert abs(_bar_angle(_tilted_bar(0.0))) < 1.0
    assert _bar_angle(_tilted_bar(12.0)) > 8.0      # clockwise = positive


def test_deskew_makes_a_tilted_region_horizontal():
    """The user rotates the pick rect CW by 12 deg to match a tilted box;
    deskew must UNDO that tilt. Rotating the same way instead doubled it,
    handing OCR text at ~24 deg (issue #26)."""
    tilted = _tilted_bar(12.0)
    out = _deskew_to_target(tilted, 12.0, 200, 200)
    assert abs(_bar_angle(out)) < 2.0, (
        f"deskewed bar is still tilted {_bar_angle(out):.1f} deg")


def test_deskew_handles_counter_clockwise_rotation():
    tilted = _tilted_bar(-9.0)
    out = _deskew_to_target(tilted, -9.0, 200, 200)
    assert abs(_bar_angle(out)) < 2.0


# ---- TextConfirmPolls=1 must not silence the reader -----------------------

class _StillCapture:
    def snapshot(self):
        return np.zeros((32, 32, 3), dtype=np.uint8)


def test_confirmation_disabled_still_marks_regions_confirmed():
    """With confirm_polls=1 there is no second look to wait for, so the text
    is as confirmed as it will ever be. Reporting nothing left hash-gated
    regions permanently silent (issue #26)."""
    class Steady:
        def read(self, frame, speaker=False):
            return "A line."

    job = OCRBatchJob(
        generation=0,
        regions=[OCRRegionSpec(name="dialogue1", mode="dialogue",
                               capture=_StillCapture())],
        confirm_polls=1, debug=False, pre_snapshot_delay=0.0)
    assert OCRWorker(Steady())._process(job).confirmed == {"dialogue1"}


# ---- two auto profiles for one game must not ping-pong --------------------

_OUTLINE = {"x": 0, "y": 0, "w": 400, "h": 150, "rotation": 0.0,
            "label": "dialogue1", "mode": "dialogue"}


def test_two_auto_profiles_for_one_game_settle(tmp_path, monkeypatch):
    s = ProfileStore(tmp_path / "profiles.json")
    for n in ("vein-a", "vein-b"):
        s.snapshot(n, "vein.exe", (0, 0, 2000, 1000), [_OUTLINE])
        s.set_auto(n, True)
    monkeypatch.setattr(main_mod, "find_process_window", lambda exe: 999)
    sent = []
    for _ in range(4):                     # four watcher ticks
        main_mod._profile_watch_tick(s, running={"vein.exe"}, enqueue=sent.append)
        for cmd in list(sent):             # main thread consumes them
            if cmd.startswith("PROFILE_APPLY:"):
                s.set_applied_exclusive(cmd.split(":", 1)[1], "vein.exe")
        if len(sent) > 3:
            break
    assert len(sent) <= 2, f"profiles ping-ponged: {sent}"


# ---- a transient winrt failure must not strand paused media ---------------

class _Session:
    def __init__(self, app_id="spotify"):
        self.app_id = app_id
        self._status = "playing"
        self.can_pause = self.can_play = True
        self.play_calls = 0

    @property
    def is_playing(self):
        return self._status == "playing"

    @property
    def is_paused(self):
        return self._status == "paused"

    def pause(self):
        self._status = "paused"

    def play(self):
        self.play_calls += 1
        self._status = "playing"


def _wait_for(cond, timeout=3.0):
    end = time.monotonic() + timeout
    while time.monotonic() < end:
        if cond():
            return True
        time.sleep(0.01)
    return False


def test_enumeration_failure_during_resume_keeps_the_ids():
    """If winrt throws while the resume enumerates, the remembered ids must
    survive so the next attempt can still un-pause (issue #26)."""
    s = _Session()
    calls = {"n": 0}

    def source():
        calls["n"] += 1
        if calls["n"] == 2:            # the resume's enumeration
            raise RuntimeError("winrt hiccup")
        return [s]

    gate = MediaGate(resume_delay_ms=20, session_source=source)
    gate.speech_started()
    assert _wait_for(lambda: s.is_paused and gate._paused_ids)
    gate.speech_ended()
    assert _wait_for(lambda: s.play_calls == 1, timeout=3.0), \
        "media stranded paused after a transient enumeration failure"


# ---- shutdown must bound its wait even with ids to resume -----------------

def test_shutdown_is_bounded_with_a_pending_resume():
    """The round-4 test only covered the empty-ids path, so the unbounded
    shutdown it was meant to guard was still live (issue #26)."""
    s = _Session()
    release = threading.Event()
    calls = {"n": 0}

    def source():
        calls["n"] += 1
        if calls["n"] == 1:
            return [s]                 # pause registers an id
        release.wait(timeout=5.0)      # then wedge
        return [s]

    gate = MediaGate(resume_delay_ms=10_000, session_source=source)
    gate.speech_started()
    assert _wait_for(lambda: gate._paused_ids)
    t0 = time.monotonic()
    gate.shutdown()
    elapsed = time.monotonic() - t0
    release.set()
    assert elapsed < 3.0, f"shutdown blocked {elapsed:.1f}s"


def test_pause_worker_cannot_re_pause_after_shutdown():
    """A pause worker still running when shutdown resumes must not undo it."""
    s = _Session()
    gate_started = threading.Event()
    release = threading.Event()
    calls = {"n": 0}

    def source():
        calls["n"] += 1
        if calls["n"] == 1:
            gate_started.set()
            release.wait(timeout=5.0)   # pause is slow
        return [s]

    gate = MediaGate(resume_delay_ms=10_000, session_source=source)
    gate.speech_started()
    assert gate_started.wait(2.0)
    gate.shutdown()
    release.set()
    time.sleep(0.4)
    assert not s.is_paused, "pause worker re-paused media after shutdown"
