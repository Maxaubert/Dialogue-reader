"""Every PortAudio call must be serialized through one process-wide lock.

sounddevice's module-level play/stop/wait operate on ONE global stream whose
close() is unguarded (`Pa_CloseStream(self._ptr); self._ptr = NULL`), and cffi
releases the GIL across that C call. Two threads reaching it together close the
same PaStream twice -> access violation (the 0xc0000005 crash, issue #20/#22).
The TTS workers, the main loop's cue beeps and stop()/shutdown() all touch that
global, so they must all take the same lock.
"""
import threading
import time
from types import SimpleNamespace

import numpy as np
import pytest

import audio
import main as main_mod
import tts as tts_mod
from tts import TTS


class RecordingSd:
    """Fails loudly if two threads are inside a PortAudio call at once."""

    def __init__(self):
        self.inside = 0
        self.overlaps = 0
        self.calls = []
        self._probe_lock = threading.Lock()

    def _enter(self, name):
        with self._probe_lock:
            self.inside += 1
            if self.inside > 1:
                self.overlaps += 1
            self.calls.append(name)
        time.sleep(0.01)          # widen the window a real C call would have
        with self._probe_lock:
            self.inside -= 1

    def play(self, *a, **k):
        self._enter("play")

    def stop(self):
        self._enter("stop")

    def wait(self):
        self._enter("wait")


@pytest.fixture(autouse=True)
def _fresh_sd(monkeypatch):
    rec = RecordingSd()
    monkeypatch.setattr(audio, "sd", rec)
    return rec


def _wait_for(cond, timeout=3.0):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if cond():
            return True
        time.sleep(0.01)
    return False


# ---- the audio module ------------------------------------------------------

def test_play_and_stop_never_overlap(_fresh_sd):
    threads = [threading.Thread(target=audio.play,
                                args=(np.zeros(10, dtype=np.float32), 24000))
               for _ in range(4)]
    threads += [threading.Thread(target=audio.stop) for _ in range(4)]
    for t in threads:
        t.start()
    for t in threads:
        t.join()
    assert _fresh_sd.overlaps == 0
    assert len(_fresh_sd.calls) == 8


def test_play_blocking_holds_the_lock_for_the_whole_call(_fresh_sd):
    # A blocking cue must not let another thread's stop() close the stream it
    # is parked on -- that is the exact double-close pattern.
    done = threading.Event()

    def cue():
        audio.play(np.zeros(10, dtype=np.float32), 24000, blocking=True)
        done.set()

    threading.Thread(target=cue).start()
    time.sleep(0.002)
    audio.stop()
    assert done.wait(3.0)
    assert _fresh_sd.overlaps == 0


def test_errors_are_contained(monkeypatch):
    class Boom:
        def play(self, *a, **k):
            raise RuntimeError("no device")

        def stop(self):
            raise RuntimeError("no device")

    monkeypatch.setattr(audio, "sd", Boom())
    audio.play(np.zeros(4, dtype=np.float32), 24000)   # must not raise
    audio.stop()


# ---- call sites --------------------------------------------------------

def test_tts_module_does_not_touch_sounddevice_directly():
    # tts.py must go through audio.py; a stray `import sounddevice` there
    # would reintroduce an unserialized path to the global stream.
    assert not hasattr(tts_mod, "sd"), "tts.py must not hold a sounddevice handle"


def test_main_module_does_not_touch_sounddevice_directly():
    assert not hasattr(main_mod, "sd"), "main.py must not hold a sounddevice handle"


def test_tts_speak_and_cue_never_overlap(_fresh_sd, monkeypatch):
    """The live crash shape: a cue beep on the main thread while a TTS worker
    is playing/stopping."""
    class FakeKokoro:
        def synth(self, text, name, speed=1.0):
            return np.zeros(240, dtype=np.float32), 24000

    monkeypatch.setattr(TTS, "_ensure_default_loaded", lambda self: None)
    t = TTS()
    monkeypatch.setattr(t, "_get_kokoro", lambda: FakeKokoro())

    stop_flag = threading.Event()

    def cue_spam():
        while not stop_flag.is_set():
            main_mod._play_cue(main_mod._PAUSE_CUE)

    spammer = threading.Thread(target=cue_spam, daemon=True)
    spammer.start()
    for _ in range(6):
        t.speak("line")
        time.sleep(0.02)
        t.stop()
    stop_flag.set()
    spammer.join(timeout=3.0)
    time.sleep(0.1)
    assert _fresh_sd.overlaps == 0
