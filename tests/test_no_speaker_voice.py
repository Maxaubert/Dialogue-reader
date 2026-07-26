"""The no-speaker voice (the __default__ pseudo-speaker) must be settable
directly: from the SpeakerManager, via the SET_NO_SPEAKER_VOICE command, and
readable by the settings UI."""
from types import SimpleNamespace

from speakers import DEFAULT_SPEAKER_KEY, SpeakerManager
from main import handle_command


def _mgr(tmp_path, pool=None):
    return SpeakerManager(
        voice_pool=pool or ["kokoro:af_heart", "kokoro:am_michael", "kokoro:bf_emma"],
        save_path=tmp_path / "speakers.json",
        assignment_strategy="round_robin",
    )


# ---- SpeakerManager.set_no_speaker_voice -----------------------------------

def test_set_no_speaker_voice_used_when_no_current_speaker(tmp_path):
    m = _mgr(tmp_path)
    m.set_no_speaker_voice("kokoro:bf_emma")
    assert m.voice_for_current() == "kokoro:bf_emma"


def test_set_no_speaker_voice_persists(tmp_path):
    m = _mgr(tmp_path)
    m.set_no_speaker_voice("kokoro:am_michael")
    m2 = _mgr(tmp_path)
    assert m2.assignments[DEFAULT_SPEAKER_KEY] == "kokoro:am_michael"


def test_set_no_speaker_voice_aligns_cycle_index(tmp_path):
    # F2 after pinning must continue from the pinned voice, not jump back.
    m = _mgr(tmp_path)
    m.set_no_speaker_voice("kokoro:am_michael")     # pool index 1
    _, next_voice = m.cycle_current_voice(direction=1)
    assert next_voice == "kokoro:bf_emma"           # index 2


def test_no_speaker_voice_property(tmp_path):
    m = _mgr(tmp_path)
    assert m.no_speaker_voice is None
    m.set_no_speaker_voice("kokoro:bf_emma")
    assert m.no_speaker_voice == "kokoro:bf_emma"


# ---- command dispatch ------------------------------------------------------

class _FakeTTS:
    def __init__(self):
        self.spoken = []

    def speak(self, text, voice=None):
        self.spoken.append((text, voice))


def test_handle_command_set_no_speaker_voice(tmp_path):
    m = _mgr(tmp_path)
    tts = _FakeTTS()
    handle_command("SET_NO_SPEAKER_VOICE:kokoro:bf_emma", [], tts, m,
                   {}, debug=False)
    assert m.no_speaker_voice == "kokoro:bf_emma"
    assert tts.spoken and tts.spoken[0][1] == "kokoro:bf_emma"


def test_handle_command_set_no_speaker_voice_empty_ignored(tmp_path):
    m = _mgr(tmp_path)
    tts = _FakeTTS()
    handle_command("SET_NO_SPEAKER_VOICE:", [], tts, m, {}, debug=False)
    assert m.no_speaker_voice is None
    assert tts.spoken == []
