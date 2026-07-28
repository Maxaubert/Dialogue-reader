"""Tests for MediaGate: pause playing media sessions when speech starts,
resume only the ones we paused after a quiet period."""
import threading
import time

from media_gate import MediaGate


class FakeSession:
    def __init__(self, app_id, status="playing", can_pause=True, can_play=True,
                 pause_raises=False):
        self.app_id = app_id
        self._status = status
        self.can_pause = can_pause
        self.can_play = can_play
        self._pause_raises = pause_raises
        self.pause_calls = 0
        self.play_calls = 0

    @property
    def is_playing(self):
        return self._status == "playing"

    @property
    def is_paused(self):
        return self._status == "paused"

    def pause(self):
        if self._pause_raises:
            raise RuntimeError("winrt says no")
        self.pause_calls += 1
        self._status = "paused"

    def play(self):
        self.play_calls += 1
        self._status = "playing"


def _gate(sessions, delay_ms=30):
    return MediaGate(resume_delay_ms=delay_ms,
                     session_source=lambda: list(sessions))


def _wait_for(cond, timeout=2.0):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if cond():
            return True
        time.sleep(0.01)
    return False


# ---- pausing ---------------------------------------------------------------

def test_pauses_only_playing_pausable_sessions():
    playing = FakeSession("spotify")
    already_paused = FakeSession("browser", status="paused")
    unpausable = FakeSession("weird", can_pause=False)
    gate = _gate([playing, already_paused, unpausable])
    gate.speech_started()
    assert _wait_for(lambda: playing.pause_calls == 1)
    assert already_paused.pause_calls == 0
    assert unpausable.pause_calls == 0


def test_per_session_pause_errors_do_not_stop_others():
    bad = FakeSession("bad", pause_raises=True)
    good = FakeSession("good")
    gate = _gate([bad, good])
    gate.speech_started()
    assert _wait_for(lambda: good.pause_calls == 1)


def test_session_source_errors_are_swallowed():
    def boom():
        raise RuntimeError("no media manager")
    gate = MediaGate(resume_delay_ms=30, session_source=boom)
    gate.speech_started()
    gate.speech_ended()
    time.sleep(0.1)
    gate.shutdown()


# ---- resuming --------------------------------------------------------------

def test_resumes_our_session_after_quiet_period():
    s = FakeSession("spotify")
    gate = _gate([s])
    gate.speech_started()
    # Wait on GATE state, not the fake's status: _pause_all flips the session
    # first and registers the id afterwards, so syncing on is_paused could
    # race ahead of the bookkeeping the resume depends on (issue #22).
    assert _wait_for(lambda: s.is_paused and gate._paused_ids)
    gate.speech_ended()
    assert _wait_for(lambda: s.play_calls == 1)


def test_never_resumes_sessions_it_did_not_pause():
    s = FakeSession("browser", status="paused")
    gate = _gate([s])
    gate.speech_started()
    gate.speech_ended()
    time.sleep(0.15)
    assert s.play_calls == 0


def test_skips_session_the_user_manually_resumed():
    s = FakeSession("spotify")
    gate = _gate([s])
    gate.speech_started()
    # Wait on GATE state, not the fake's status: _pause_all flips the session
    # first and registers the id afterwards, so syncing on is_paused could
    # race ahead of the bookkeeping the resume depends on (issue #22).
    assert _wait_for(lambda: s.is_paused and gate._paused_ids)
    s._status = "playing"          # user hit play themselves mid-speech
    gate.speech_ended()
    time.sleep(0.15)
    assert s.play_calls == 0


def test_new_speech_during_grace_period_cancels_resume():
    s = FakeSession("spotify")
    gate = _gate([s])
    gate.speech_started()
    # Wait on GATE state, not the fake's status: _pause_all flips the session
    # first and registers the id afterwards, so syncing on is_paused could
    # race ahead of the bookkeeping the resume depends on (issue #22).
    assert _wait_for(lambda: s.is_paused and gate._paused_ids)
    gate.speech_ended()
    gate.speech_started()          # next line arrived within the grace period
    time.sleep(0.15)
    assert s.play_calls == 0


def test_resume_memory_clears_after_firing():
    s = FakeSession("spotify")
    gate = _gate([s])
    gate.speech_started()
    # Wait on GATE state, not the fake's status: _pause_all flips the session
    # first and registers the id afterwards, so syncing on is_paused could
    # race ahead of the bookkeeping the resume depends on (issue #22).
    assert _wait_for(lambda: s.is_paused and gate._paused_ids)
    gate.speech_ended()
    assert _wait_for(lambda: s.play_calls == 1)
    s._status = "paused"           # user paused it themselves later
    gate.speech_ended()            # stray end event, nothing owed
    time.sleep(0.15)
    assert s.play_calls == 1


# ---- races (issue #22) -----------------------------------------------------

def test_resume_already_running_is_cancelled_by_new_speech():
    """threading.Timer.cancel() is a no-op once the callback has started, so
    a resume that is already inside _resume_paused must still notice that new
    speech began and abandon the un-pause."""
    s = FakeSession("spotify")
    entered = threading.Event()
    release = threading.Event()

    class SlowSource:
        def __init__(self):
            self.calls = 0

        def __call__(self):
            self.calls += 1
            if self.calls > 1:        # the resume's re-enumeration
                entered.set()
                release.wait(timeout=2.0)
            return [s]

    gate = MediaGate(resume_delay_ms=10, session_source=SlowSource())
    gate.speech_started()
    # Wait on GATE state, not the fake's status: _pause_all flips the session
    # first and registers the id afterwards, so syncing on is_paused could
    # race ahead of the bookkeeping the resume depends on (issue #22).
    assert _wait_for(lambda: s.is_paused and gate._paused_ids)
    gate.speech_ended()
    assert entered.wait(2.0), "resume never started"
    gate.speech_started()             # next line lands mid-resume
    release.set()
    time.sleep(0.2)
    assert s.play_calls == 0, "media un-paused in the middle of speech"


def test_pause_still_in_flight_when_resume_fires_does_not_strand_media():
    """A slow pause worker must not let the resume run first and leave the
    media paused with nothing left to un-pause it."""
    s = FakeSession("spotify")
    gate_ready = threading.Event()

    class SlowPause:
        def __init__(self):
            self.calls = 0

        def __call__(self):
            self.calls += 1
            if self.calls == 1:       # the pause enumeration is the slow one
                gate_ready.set()
                time.sleep(0.3)
            return [s]

    gate = MediaGate(resume_delay_ms=20, session_source=SlowPause())
    gate.speech_started()
    assert gate_ready.wait(2.0)
    gate.speech_ended()               # fires while the pause is still running
    assert _wait_for(lambda: s.play_calls == 1, timeout=3.0), \
        "media left paused forever"


def test_shutdown_waits_for_an_in_flight_pause():
    s = FakeSession("spotify")
    started = threading.Event()

    class SlowPause:
        def __init__(self):
            self.calls = 0

        def __call__(self):
            self.calls += 1
            if self.calls == 1:
                started.set()
                time.sleep(0.25)
            return [s]

    gate = MediaGate(resume_delay_ms=10_000, session_source=SlowPause())
    gate.speech_started()
    assert started.wait(2.0)
    gate.shutdown()                   # must not exit leaving media paused
    # The invariant is the END STATE, not which path got there: shutdown
    # either resumes what the worker paused, or the worker sees _closing and
    # never pauses at all (issue #26). Both leave the media playing.
    assert not s.is_paused


# ---- hot-apply -------------------------------------------------------------

def test_set_resume_delay_ms():
    s = FakeSession("spotify")
    gate = _gate([s], delay_ms=10_000)
    gate.set_resume_delay_ms(30)
    gate.speech_started()
    # Wait on GATE state, not the fake's status: _pause_all flips the session
    # first and registers the id afterwards, so syncing on is_paused could
    # race ahead of the bookkeeping the resume depends on (issue #22).
    assert _wait_for(lambda: s.is_paused and gate._paused_ids)
    gate.speech_ended()
    assert _wait_for(lambda: s.play_calls == 1)   # new short delay in effect


# ---- shutdown --------------------------------------------------------------

def test_shutdown_resumes_immediately():
    s = FakeSession("spotify")
    gate = _gate([s], delay_ms=10_000)
    gate.speech_started()
    # Wait on GATE state, not the fake's status: _pause_all flips the session
    # first and registers the id afterwards, so syncing on is_paused could
    # race ahead of the bookkeeping the resume depends on (issue #22).
    assert _wait_for(lambda: s.is_paused and gate._paused_ids)
    gate.shutdown()
    assert s.play_calls == 1       # synchronous, no grace period
