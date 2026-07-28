"""A dialogue box that appears once and stays still must be spoken.

Pixel-hash-gated regions (window/screen mode) yield exactly ONE frame per
stable image: capture.poll_once() guards on _last_yielded_hash. The outer
candidate layer then required a SECOND batch carrying the same text before
speaking, which for static text never arrives -- so the line was OCR'd,
stored as a candidate, and never read aloud (issue #24). The OCR worker's
own confirm loop already re-snapshots and re-OCRs the text; when it says the
text held, that IS the confirmation.
"""
from types import SimpleNamespace

import main as main_mod
from main import WatchedRegion


class _StaticCapture:
    """Yields one frame for a change, then nothing (the real hash gate)."""

    def __init__(self):
        self.hwnd = 0
        self.capture_mode = "window"
        self.use_window_mode = True
        self.game_mode = False
        self.rotation = 0.0
        self.bbox = {"left": 0, "top": 0, "width": 100, "height": 50}
        self.target_w, self.target_h = 100, 50
        self.rel_x = self.rel_y = 0
        self._yielded = False

    def poll_once(self):
        if self._yielded:
            return None
        self._yielded = True
        return object()


def _speak_once(confirmed_names):
    """Run one OCR result through the applier; return what was spoken."""
    spoken = []
    tts = SimpleNamespace(
        speak=lambda text, voice=None, pause_media=True: spoken.append(text),
        stop=lambda: None)
    speaker_mgr = SimpleNamespace(voice_for_current=lambda: None,
                                  set_current=lambda *a, **k: None,
                                  current_speaker="")
    r = WatchedRegion(name="dialogue1", capture=_StaticCapture(),
                      mode="dialogue", process="game.exe")
    r.has_pending_frame = True
    state = {"generation": 0, "last_spoken": "", "candidate": "",
             "speaker_candidate": "", "paused": False}
    result = SimpleNamespace(
        generation=0, error=None, fg_exe="game.exe",
        texts={"dialogue1": "The gate is sealed."},
        speaker_texts={}, confirmed=confirmed_names)
    main_mod._apply_ocr_result(result, [r], state, speaker_mgr, tts, debug=False)
    return spoken


def test_worker_confirmed_text_is_spoken_from_one_batch():
    assert _speak_once({"dialogue1"}) == ["The gate is sealed."]


def test_unconfirmed_text_still_waits_for_a_second_look():
    # confirm_polls == 1 (no worker-side confirmation): the jitter guard
    # must still hold the first sighting back.
    assert _speak_once(set()) == []


def test_static_region_yields_only_one_frame():
    """Pins the capture behaviour the fix depends on: no second batch is
    coming for a region whose pixels stopped changing."""
    cap = _StaticCapture()
    assert cap.poll_once() is not None
    assert cap.poll_once() is None
    assert main_mod._poll_regions(
        [WatchedRegion(name="d1", capture=cap, mode="dialogue")], debug=False
    ) is False


def test_worker_reports_confirmation(monkeypatch):
    """The worker must actually populate `confirmed` for regions whose text
    it verified, otherwise the fix above is dead code."""
    from ocr import OCRBatchJob, OCRRegionSpec, OCRWorker

    class FakeOCR:
        def read(self, frame, speaker=False):
            return "Hello there."

    class FakeCapture:
        def snapshot(self):
            import numpy as np
            return np.zeros((4, 4, 3), dtype=np.uint8)

    worker = OCRWorker(FakeOCR())
    job = OCRBatchJob(
        generation=0,
        regions=[OCRRegionSpec(name="dialogue1", mode="dialogue",
                               capture=FakeCapture())],
        confirm_polls=2, debug=False,
        pre_snapshot_delay=0.0, confirm_interval=0.0,
    )
    res = worker._process(job)
    assert res.texts["dialogue1"] == "Hello there."
    assert "dialogue1" in res.confirmed


def test_worker_reports_nothing_when_confirmation_is_off():
    from ocr import OCRBatchJob, OCRRegionSpec, OCRWorker

    class FakeOCR:
        def read(self, frame, speaker=False):
            return "Hello there."

    class FakeCapture:
        def snapshot(self):
            import numpy as np
            return np.zeros((4, 4, 3), dtype=np.uint8)

    worker = OCRWorker(FakeOCR())
    job = OCRBatchJob(
        generation=0,
        regions=[OCRRegionSpec(name="dialogue1", mode="dialogue",
                               capture=FakeCapture())],
        confirm_polls=1, debug=False, pre_snapshot_delay=0.0,
    )
    assert worker._process(job).confirmed == set()
