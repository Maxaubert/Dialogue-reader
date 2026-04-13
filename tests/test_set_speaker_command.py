"""Verify that `SET_SPEAKER:<name>` goes through handle_command and sets the speaker."""
import tempfile
from pathlib import Path
from unittest.mock import MagicMock

from speakers import SpeakerManager


def _mgr(pool=("voice:a", "voice:b")) -> SpeakerManager:
    td = tempfile.mkdtemp()
    return SpeakerManager(voice_pool=list(pool), save_path=Path(td) / "s.json")


def test_set_speaker_command_updates_current_speaker():
    from main import handle_command

    mgr = _mgr()
    tts = MagicMock()
    tts.speak = MagicMock()
    state = {"paused": False, "last_spoken": "", "candidate": ""}

    handle_command(
        "SET_SPEAKER:Alice",
        regions=[],
        tts=tts,
        speaker_mgr=mgr,
        state=state,
        debug=False,
    )

    assert mgr.current_speaker == "Alice"
    assert "Alice" in mgr.assignments


def test_set_speaker_command_trims_whitespace_and_strips_prefix():
    from main import handle_command

    mgr = _mgr()
    tts = MagicMock()
    state = {"paused": False, "last_spoken": "", "candidate": ""}

    handle_command(
        "SET_SPEAKER:  Bob Smith  ",
        regions=[],
        tts=tts,
        speaker_mgr=mgr,
        state=state,
        debug=False,
    )

    assert mgr.current_speaker == "Bob Smith"


def test_set_speaker_command_ignores_empty_name():
    from main import handle_command

    mgr = _mgr()
    tts = MagicMock()
    state = {"paused": False, "last_spoken": "", "candidate": ""}

    handle_command(
        "SET_SPEAKER:",
        regions=[],
        tts=tts,
        speaker_mgr=mgr,
        state=state,
        debug=False,
    )

    assert mgr.current_speaker == ""
