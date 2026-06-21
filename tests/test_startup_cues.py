"""Guards the startup cue constants and their wording."""
import numpy as np

import main


def test_startup_phrase_wording():
    assert main._STARTUP_PHRASE == "OCR starting up"


def test_ready_phrase_wording():
    assert main._READY_PHRASE == "OCR ready"


def test_ready_cue_is_audio():
    assert isinstance(main._READY_CUE, np.ndarray)
    assert main._READY_CUE.size > 0


def test_ready_cue_distinct_from_unpause_cue():
    # The ready chime must not be the same samples as the unpause cue, so the
    # two signals are audibly different.
    if main._READY_CUE.shape == main._UNPAUSE_CUE.shape:
        assert not np.array_equal(main._READY_CUE, main._UNPAUSE_CUE)
