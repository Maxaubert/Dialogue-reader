"""Tests for hot-applying config: the RELOAD_CONFIG command re-reads the ini
and applies everything that does not require a restart, and PREVIEW_VOICE
speaks a sample through the reader's loaded TTS."""
from types import SimpleNamespace

import main as main_mod
from main import _reload_config, handle_command


class FakeTTS:
    def __init__(self, gate=None):
        self._gate = gate
        self.default_voice = None
        self.spoken = []

    @property
    def media_gate(self):
        return self._gate

    def set_media_gate(self, gate):
        self._gate = gate

    def set_default_voice(self, voice):
        self.default_voice = voice

    def speak(self, text, voice=None):
        self.spoken.append((text, voice))


class FakeGate:
    def __init__(self):
        self.delay_ms = None

    def set_resume_delay_ms(self, ms):
        self.delay_ms = ms


class FakeOCR:
    def __init__(self):
        self.engines = None

    def set_engines(self, dialogue, speaker):
        self.engines = (dialogue, speaker)


def _patch_loaders(monkeypatch, media=(True, 1000)):
    monkeypatch.setattr(main_mod, "_load_voice_config",
                        lambda: (["kokoro:af_heart", "kokoro:am_michael"], "kokoro:af_heart"))
    monkeypatch.setattr(main_mod, "_load_speaker_assignment_strategy",
                        lambda: "round_robin")
    monkeypatch.setattr(main_mod, "_load_capture_mode", lambda: "screen")
    monkeypatch.setattr(main_mod, "_load_text_confirm_polls", lambda: 3)
    monkeypatch.setattr(main_mod, "_load_skip_when_zoomed", lambda: False)
    monkeypatch.setattr(main_mod, "_load_ocr_config", lambda: ("easyocr", "winocr"))
    monkeypatch.setattr(main_mod, "_load_media_config", lambda: media)


def _state():
    return {"capture_mode": "auto", "text_confirm_polls": 2,
            "skip_when_zoomed": True}


def test_reload_applies_everything(monkeypatch):
    _patch_loaders(monkeypatch)
    tts, mgr, ocr = FakeTTS(FakeGate()), SimpleNamespace(), FakeOCR()
    state = _state()
    _reload_config(tts, mgr, state, ocr=ocr)
    assert mgr.voice_pool == ["kokoro:af_heart", "kokoro:am_michael"]
    assert mgr.assignment_strategy == "round_robin"
    assert tts.default_voice == "kokoro:af_heart"
    assert state["capture_mode"] == "screen"
    assert state["text_confirm_polls"] == 3
    assert state["skip_when_zoomed"] is False
    assert ocr.engines == ("easyocr", "winocr")


def test_reload_updates_existing_gate_delay(monkeypatch):
    _patch_loaders(monkeypatch, media=(True, 2500))
    gate = FakeGate()
    tts = FakeTTS(gate)
    _reload_config(tts, SimpleNamespace(), _state(), ocr=FakeOCR())
    assert tts.media_gate is gate          # same gate kept, not rebuilt
    assert gate.delay_ms == 2500


def test_reload_creates_gate_when_newly_enabled(monkeypatch):
    _patch_loaders(monkeypatch, media=(True, 1000))
    created = []
    monkeypatch.setattr("media_gate.MediaGate",
                        lambda **kw: created.append(kw) or "new-gate")
    tts = FakeTTS(gate=None)
    _reload_config(tts, SimpleNamespace(), _state(), ocr=FakeOCR())
    assert tts.media_gate == "new-gate"
    assert created[0]["resume_delay_ms"] == 1000


def test_reload_removes_gate_when_disabled(monkeypatch):
    _patch_loaders(monkeypatch, media=(False, 1000))
    tts = FakeTTS(FakeGate())
    _reload_config(tts, SimpleNamespace(), _state(), ocr=FakeOCR())
    assert tts.media_gate is None


def test_reload_survives_ocr_engine_error(monkeypatch):
    _patch_loaders(monkeypatch)

    class BoomOCR:
        def set_engines(self, d, s):
            raise RuntimeError("no such engine")

    tts = FakeTTS(FakeGate())
    _reload_config(tts, SimpleNamespace(), _state(), ocr=BoomOCR())
    assert tts.default_voice == "kokoro:af_heart"   # rest still applied


# ---- command dispatch ------------------------------------------------------

def test_handle_command_reload(monkeypatch):
    called = {}
    monkeypatch.setattr(main_mod, "_reload_config",
                        lambda tts, mgr, state, ocr=None, debug=False:
                        called.setdefault("args", (tts, mgr, state, ocr)))
    tts, mgr, state = FakeTTS(), SimpleNamespace(), _state()
    handle_command("RELOAD_CONFIG", [], tts, mgr, state, debug=False, ocr="the-ocr")
    assert called["args"] == (tts, mgr, state, "the-ocr")


def test_handle_command_preview_voice():
    tts = FakeTTS()
    handle_command("PREVIEW_VOICE:kokoro:bf_emma", [], tts, SimpleNamespace(),
                   _state(), debug=False)
    assert len(tts.spoken) == 1
    assert tts.spoken[0][1] == "kokoro:bf_emma"


def test_handle_command_preview_voice_empty_is_ignored():
    tts = FakeTTS()
    handle_command("PREVIEW_VOICE:", [], tts, SimpleNamespace(),
                   _state(), debug=False)
    assert tts.spoken == []
