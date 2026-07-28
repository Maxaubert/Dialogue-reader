"""Pause other media (YouTube, Spotify, ...) while TTS speaks, resume after.

Uses Windows Global System Media Transport Controls (GSMTC) sessions: only
apps that register a media session are touched. Games never register one, so
the game being read can never be paused by this.

Lifecycle, driven by tts.TTS:
    gate.speech_started()  -> pause every playing session, remember which
    gate.speech_ended()    -> after resume_delay_ms of quiet, resume only the
                              remembered sessions that are still paused
    gate.shutdown()        -> resume immediately (never leave media stuck)

A new speech_started() during the quiet period cancels the pending resume,
so bursts of dialogue lines do not stutter the media.
"""

from __future__ import annotations

import asyncio
import threading


class MediaGate:
    # Bounded retries for a resume whose session enumeration failed.
    _RETRY_DELAY = 0.25
    _MAX_RETRIES = 8

    def __init__(self, resume_delay_ms: int = 1000, session_source=None,
                 verbose: bool = False):
        self._resume_delay = max(0, int(resume_delay_ms)) / 1000.0
        self._session_source = session_source or _GsmtcSource()
        self._verbose = verbose
        self._paused_ids: set[str] = set()
        self._timer: threading.Timer | None = None
        self._lock = threading.Lock()
        # Serializes pause against resume. Without it a slow pause worker and
        # the resume timer interleaved: the resume enumerated first, found
        # nothing owed, and the pause landed after it -- media stayed paused
        # forever (issue #22).
        self._work_lock = threading.RLock()
        # Bumped by every speech_started(). _resume_paused captures it and
        # re-checks before issuing any play(), because Timer.cancel() is a
        # no-op once the callback has already begun running.
        self._epoch = 0
        self._pause_thread: threading.Thread | None = None
        # Set by shutdown() so a pause worker still in flight stops instead
        # of re-pausing media we just resumed on the way out.
        self._closing = False
        self._retries = 0

    def set_resume_delay_ms(self, resume_delay_ms: int) -> None:
        """Hot-apply a new quiet-period length (RELOAD_CONFIG). Takes effect
        on the next speech_ended()."""
        self._resume_delay = max(0, int(resume_delay_ms)) / 1000.0

    # ---- lifecycle ----

    def speech_started(self) -> None:
        """Cancel any pending resume, then pause playing sessions. The pause
        itself runs on a worker thread so speak() never blocks on winrt."""
        self._cancel_timer()
        with self._lock:
            self._epoch += 1
            epoch = self._epoch
            self._retries = 0        # a fresh utterance resets the budget
        t = threading.Thread(target=self._pause_all, args=(epoch,), daemon=True)
        with self._lock:
            self._pause_thread = t
        t.start()

    def speech_ended(self) -> None:
        """Start (or restart) the quiet-period timer that resumes media."""
        with self._lock:
            if self._timer is not None:
                self._timer.cancel()
            epoch = self._epoch
            self._timer = threading.Timer(
                self._resume_delay, self._resume_paused, kwargs={"epoch": epoch})
            self._timer.daemon = True
            self._timer.start()

    def shutdown(self) -> None:
        """Resume immediately; quitting must never leave media stuck paused.
        Waits briefly for an in-flight pause so we never exit having just
        paused something with nothing left to un-pause it."""
        self._closing = True
        self._cancel_timer()
        with self._lock:
            t = self._pause_thread
        if t is not None and t.is_alive():
            # Short: the AHK supervisor gives us a fixed grace period before
            # taskkill, so a long join would just get us killed mid-wait and
            # leave the media paused anyway (issue #24).
            t.join(timeout=0.6)
        # epoch=None: resume unconditionally. Run it on a worker with a hard
        # deadline: the supervisor kills us on a fixed timer, and the resume
        # can block inside winrt session enumeration, not just on the lock
        # (issue #26). Better to exit having tried than to be killed waiting.
        done = threading.Thread(
            target=self._resume_paused,
            kwargs={"epoch": None, "lock_timeout": 0.6},
            daemon=True,
        )
        done.start()
        done.join(timeout=1.2)
        if done.is_alive():
            self._log("resume did not finish before the shutdown deadline")
        close = getattr(self._session_source, "close", None)
        if close is not None:
            try:
                close()      # stop the winrt loop thread we own
            except Exception as e:
                self._log(f"session source close failed: {e}")

    # ---- internals ----

    def _cancel_timer(self) -> None:
        with self._lock:
            if self._timer is not None:
                self._timer.cancel()
                self._timer = None

    def _pause_all(self, epoch: int) -> None:
        with self._work_lock:          # never interleave with a resume
            for s in self._sessions():
                if self._closing:
                    # shutdown() may already have resumed everything; pausing
                    # now would strand the user's media (issue #26).
                    self._log("pause abandoned: shutting down")
                    return
                try:
                    if s.is_playing and s.can_pause:
                        s.pause()
                        with self._lock:
                            self._paused_ids.add(s.app_id)
                        self._log(f"paused {s.app_id}")
                except Exception as e:
                    self._log(f"pause failed for {s.app_id}: {e}")

    def _resume_paused(self, epoch: int | None = None,
                       lock_timeout: float | None = None) -> None:
        """Un-pause what we paused. `epoch` is the speech generation this
        resume was armed for; if newer speech has started since, abandon it
        (Timer.cancel() cannot stop a callback that already began running).
        epoch=None forces the resume, used by shutdown().

        `lock_timeout` bounds the wait for an in-flight pause; on timeout we
        resume WITHOUT the lock rather than block. shutdown() needs that: the
        supervisor kills us on a fixed deadline, so blocking here just got us
        killed mid-wait with the media still paused (issue #25)."""
        if lock_timeout is not None:
            if not self._work_lock.acquire(timeout=lock_timeout):
                self._log("work lock busy; resuming without it")
                self._resume_body(epoch)
                return
            try:
                self._resume_body(epoch)
            finally:
                self._work_lock.release()
            return
        with self._work_lock:          # waits out an in-flight pause
            self._resume_body(epoch)

    def _resume_body(self, epoch: int | None) -> None:
        """The resume itself. Callers hold (or have deliberately skipped)
        _work_lock."""
        with self._lock:
            self._timer = None
            if epoch is not None and epoch != self._epoch:
                self._log("resume abandoned: speech resumed")
                return
            ids = set(self._paused_ids)
        if not ids:
            return
        sessions = self._sessions()      # can block on winrt for seconds
        if not sessions:
            # winrt threw or returned nothing. Clearing _paused_ids here left
            # the user's media paused with nobody left to resume it (#26);
            # keep them and try again shortly.
            self._log("resume found no sessions; retrying shortly")
            self._retry_resume(epoch)
            return
        if self._stale(epoch):
            # Speech restarted while we were enumerating: leave the ids in
            # place so the next quiet period still un-pauses them.
            self._log("resume abandoned: speech resumed")
            return
        done: set[str] = set()
        for s in sessions:
            if self._stale(epoch):
                break
            try:
                # Only resume what we paused AND what is still paused, so a
                # user's manual play/pause in between is never fought.
                if s.app_id in ids and s.is_paused and s.can_play:
                    s.play()
                    self._log(f"resumed {s.app_id}")
                    done.add(s.app_id)
                elif s.app_id in ids:
                    done.add(s.app_id)   # no longer ours to resume
            except Exception as e:
                self._log(f"resume failed for {s.app_id}: {e}")
        # Forget only what we actually dealt with; anything we could not
        # reach stays remembered for the next attempt.
        with self._lock:
            self._paused_ids -= done

    def _retry_resume(self, epoch: int | None) -> None:
        """Re-arm a short resume attempt after a failed enumeration, bounded
        so a permanently broken winrt cannot spin forever."""
        if self._closing:
            return
        with self._lock:
            if self._retries >= self._MAX_RETRIES:
                self._log("giving up on resume after repeated failures")
                self._paused_ids.clear()
                return
            self._retries += 1
            if self._timer is not None:
                self._timer.cancel()
            self._timer = threading.Timer(
                self._RETRY_DELAY, self._resume_paused, kwargs={"epoch": epoch})
            self._timer.daemon = True
            self._timer.start()

    def _stale(self, epoch: int | None) -> bool:
        """True when newer speech has started since this resume was armed."""
        if epoch is None:
            return False
        with self._lock:
            return epoch != self._epoch

    def _sessions(self) -> list:
        try:
            return list(self._session_source())
        except Exception as e:
            self._log(f"session enumeration failed: {e}")
            return []

    def _log(self, msg: str) -> None:
        if self._verbose:
            print(f"[media] {msg}", flush=True)


# ---- default winrt-backed session source -----------------------------------


class _GsmtcSession:
    """Sync adapter over one winrt GSMTC session."""

    _PLAYING = 4  # GlobalSystemMediaTransportControlsSessionPlaybackStatus
    _PAUSED = 5

    def __init__(self, raw, run):
        self._raw = raw
        self._run = run
        self.app_id = raw.source_app_user_model_id
        info = raw.get_playback_info()
        self._status = int(info.playback_status)
        self.can_pause = bool(info.controls.is_pause_enabled)
        self.can_play = bool(info.controls.is_play_enabled)

    @property
    def is_playing(self) -> bool:
        return self._status == self._PLAYING

    @property
    def is_paused(self) -> bool:
        return self._status == self._PAUSED

    def pause(self) -> None:
        self._run(self._op(self._raw.try_pause_async()))

    def play(self) -> None:
        self._run(self._op(self._raw.try_play_async()))

    @staticmethod
    async def _op(op):
        return await op


class _GsmtcSource:
    """Callable returning the current media sessions. Owns a daemon thread
    with an asyncio loop, because the winrt session API is async."""

    def __init__(self):
        self._loop = asyncio.new_event_loop()
        self._thread = threading.Thread(target=self._loop.run_forever, daemon=True)
        self._thread.start()
        self._manager = None

    def close(self) -> None:
        """Stop the loop and join its thread. Without this, every RELOAD_CONFIG
        that rebuilt the gate left another live asyncio loop thread behind
        (issue #22)."""
        if self._loop.is_closed():
            return
        try:
            self._loop.call_soon_threadsafe(self._loop.stop)
        except RuntimeError:
            return
        self._thread.join(timeout=2.0)
        try:
            self._loop.close()
        except RuntimeError:
            pass

    def _run(self, coro, timeout: float = 3.0):
        return asyncio.run_coroutine_threadsafe(coro, self._loop).result(timeout)

    async def _snapshot(self) -> list:
        if self._manager is None:
            from winrt.windows.media.control import (
                GlobalSystemMediaTransportControlsSessionManager as _Manager,
            )
            self._manager = await _Manager.request_async()
        return [_GsmtcSession(raw, self._run)
                for raw in self._manager.get_sessions()]

    def __call__(self) -> list:
        return self._run(self._snapshot())
