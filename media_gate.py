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
    def __init__(self, resume_delay_ms: int = 1000, session_source=None,
                 verbose: bool = False):
        self._resume_delay = max(0, int(resume_delay_ms)) / 1000.0
        self._session_source = session_source or _GsmtcSource()
        self._verbose = verbose
        self._paused_ids: set[str] = set()
        self._timer: threading.Timer | None = None
        self._lock = threading.Lock()

    # ---- lifecycle ----

    def speech_started(self) -> None:
        """Cancel any pending resume, then pause playing sessions. The pause
        itself runs on a worker thread so speak() never blocks on winrt."""
        self._cancel_timer()
        threading.Thread(target=self._pause_all, daemon=True).start()

    def speech_ended(self) -> None:
        """Start (or restart) the quiet-period timer that resumes media."""
        with self._lock:
            if self._timer is not None:
                self._timer.cancel()
            self._timer = threading.Timer(self._resume_delay, self._resume_paused)
            self._timer.daemon = True
            self._timer.start()

    def shutdown(self) -> None:
        """Resume immediately; quitting must never leave media stuck paused."""
        self._cancel_timer()
        self._resume_paused()

    # ---- internals ----

    def _cancel_timer(self) -> None:
        with self._lock:
            if self._timer is not None:
                self._timer.cancel()
                self._timer = None

    def _pause_all(self) -> None:
        for s in self._sessions():
            try:
                if s.is_playing and s.can_pause:
                    s.pause()
                    with self._lock:
                        self._paused_ids.add(s.app_id)
                    self._log(f"paused {s.app_id}")
            except Exception as e:
                self._log(f"pause failed for {s.app_id}: {e}")

    def _resume_paused(self) -> None:
        with self._lock:
            self._timer = None
            ids, self._paused_ids = self._paused_ids, set()
        if not ids:
            return
        for s in self._sessions():
            try:
                # Only resume what we paused AND what is still paused, so a
                # user's manual play/pause in between is never fought.
                if s.app_id in ids and s.is_paused and s.can_play:
                    s.play()
                    self._log(f"resumed {s.app_id}")
            except Exception as e:
                self._log(f"resume failed for {s.app_id}: {e}")

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
        threading.Thread(target=self._loop.run_forever, daemon=True).start()
        self._manager = None

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
