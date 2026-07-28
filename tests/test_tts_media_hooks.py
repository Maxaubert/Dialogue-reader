"""TTS must drive the MediaGate lifecycle: started on speak, ended when
playback finishes or is stopped, shutdown passed through."""
import threading
import time
from types import SimpleNamespace

import numpy as np
import pytest

import audio as audio_mod
from tts import TTS


class FakeGate:
    def __init__(self):
        self.events = []

    def speech_started(self):
        self.events.append("start")

    def speech_ended(self):
        self.events.append("end")

    def shutdown(self):
        self.events.append("shutdown")


class FakeKokoro:
    def __init__(self, block_event=None):
        self._block = block_event

    def synth(self, text, name, speed=1.0):
        if self._block is not None:
            self._block.wait(timeout=2.0)
        return np.zeros(100, dtype=np.float32), 24000


def _make_tts(monkeypatch, gate, kokoro=None):
    monkeypatch.setattr(audio_mod, "sd", SimpleNamespace(
        play=lambda *a, **k: None, stop=lambda: None, wait=lambda: None))
    monkeypatch.setattr(TTS, "_ensure_default_loaded", lambda self: None)
    t = TTS(media_gate=gate)
    monkeypatch.setattr(t, "_get_kokoro", lambda: kokoro)
    return t


def _wait_for(cond, timeout=2.0):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if cond():
            return True
        time.sleep(0.01)
    return False


def test_speak_fires_started_then_ended(monkeypatch):
    gate = FakeGate()
    t = _make_tts(monkeypatch, gate, FakeKokoro())
    t.speak("hello")
    assert _wait_for(lambda: gate.events == ["start", "end"])


def test_empty_text_fires_nothing(monkeypatch):
    gate = FakeGate()
    t = _make_tts(monkeypatch, gate, FakeKokoro())
    t.speak("")
    time.sleep(0.05)
    assert gate.events == []


def test_stop_fires_ended(monkeypatch):
    gate = FakeGate()
    t = _make_tts(monkeypatch, gate, FakeKokoro())
    t.stop()
    assert gate.events == ["end"]


def test_interrupted_utterance_fires_one_end(monkeypatch):
    # First speak blocks in synth; second speak supersedes it. Only the
    # current utterance may fire "end" -- the stale worker stays silent.
    release = threading.Event()
    gate = FakeGate()
    t = _make_tts(monkeypatch, gate, FakeKokoro(block_event=release))
    t.speak("first")
    t.speak("second")
    release.set()
    assert _wait_for(lambda: gate.events.count("end") == 1)
    time.sleep(0.1)
    assert gate.events.count("start") == 2
    assert gate.events.count("end") == 1


def test_kokoro_unavailable_still_fires_ended(monkeypatch):
    # If synthesis can't happen, media must not stay paused forever.
    gate = FakeGate()
    t = _make_tts(monkeypatch, gate, kokoro=None)
    t.speak("hello")
    assert _wait_for(lambda: gate.events == ["start", "end"])


def test_shutdown_passes_through(monkeypatch):
    gate = FakeGate()
    t = _make_tts(monkeypatch, gate, FakeKokoro())
    t.shutdown()
    assert "shutdown" in gate.events


def test_no_gate_is_fine(monkeypatch):
    t = _make_tts(monkeypatch, gate=None, kokoro=FakeKokoro())
    t.speak("hello")   # must not raise
    t.stop()
    t.shutdown()


# ---- audio-thread safety ---------------------------------------------------

def test_worker_never_calls_sd_wait(monkeypatch):
    # sounddevice's global-stream wait() is not safe against a concurrent
    # stop()/play() from another thread (native crash, issue #20). The
    # worker must time playback out instead of blocking in wait().
    waits = []
    monkeypatch.setattr(audio_mod, "sd", SimpleNamespace(
        play=lambda *a, **k: None, stop=lambda: None,
        wait=lambda: waits.append(1)))
    monkeypatch.setattr(TTS, "_ensure_default_loaded", lambda self: None)
    gate = FakeGate()
    t = TTS(media_gate=gate)
    monkeypatch.setattr(t, "_get_kokoro", lambda: FakeKokoro())
    t.speak("hello")
    assert _wait_for(lambda: gate.events == ["start", "end"])
    assert waits == []


# ---- announcements (pause_media=False) -------------------------------------

def test_announcement_does_not_touch_gate(monkeypatch):
    gate = FakeGate()
    t = _make_tts(monkeypatch, gate, FakeKokoro())
    t.speak("OCR ready", pause_media=False)
    time.sleep(0.2)
    assert gate.events == []


def test_announcement_cutting_gated_speech_closes_gate(monkeypatch):
    # Dialogue is speaking (media paused); an announcement interrupts it.
    # The gate must be closed out so media is not left stuck paused.
    release = threading.Event()
    gate = FakeGate()
    t = _make_tts(monkeypatch, gate, FakeKokoro(block_event=release))
    t.speak("dialogue line")                        # gated, blocks in synth
    t.speak("Voice changed", pause_media=False)     # cuts it off
    release.set()
    assert _wait_for(lambda: "end" in gate.events)
    time.sleep(0.15)
    assert gate.events == ["start", "end"]          # nothing from announcement


# ---- hot-apply setters -----------------------------------------------------

def test_set_media_gate_swaps_and_shuts_down_old(monkeypatch):
    old, new = FakeGate(), FakeGate()
    t = _make_tts(monkeypatch, old, FakeKokoro())
    t.set_media_gate(new)
    assert t.media_gate is new
    assert "shutdown" in old.events        # old gate releases held media


def test_set_media_gate_none_disables(monkeypatch):
    old = FakeGate()
    t = _make_tts(monkeypatch, old, FakeKokoro())
    t.set_media_gate(None)
    assert t.media_gate is None
    assert "shutdown" in old.events
    t.speak("hello")                       # must not raise without a gate


def test_set_default_voice(monkeypatch):
    t = _make_tts(monkeypatch, None, FakeKokoro())
    t.set_default_voice("kokoro:bf_emma")
    assert t._default_voice == "kokoro:bf_emma"
