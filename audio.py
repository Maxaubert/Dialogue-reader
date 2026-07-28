"""The one and only door to PortAudio.

sounddevice's module-level `play`/`stop`/`wait` all drive a SINGLE global
stream (`sd._last_callback`). Its teardown is unguarded:

    err = _lib.Pa_CloseStream(self._ptr)
    self._ptr = _ffi.NULL

There is no lock, `stop()` never clears the global, and cffi releases the GIL
across the C call. So two threads can be inside `Pa_CloseStream` on the same
PaStream pointer at once -- a double free inside PortAudio, which surfaces as
an 0xc0000005 access violation and kills the process (issue #20/#22).

This project drives audio from several threads: TTS synthesis workers, the
12 Hz main loop (cue beeps, stop(), shutdown()), and command handling. Every
one of them must funnel through the module-level lock here. Nothing else in
the codebase may import sounddevice.
"""

from __future__ import annotations

import threading

import sounddevice as sd

# Re-entrant: play(blocking=True) holds it while sounddevice internally calls
# its own stop(), and a caller may legitimately nest stop() inside play().
_LOCK = threading.RLock()


def play(data, samplerate: int, blocking: bool = False) -> None:
    """Play `data`. Errors are swallowed: a dead audio device must never take
    down the reader (it already survives a missing Kokoro engine)."""
    with _LOCK:
        try:
            sd.play(data, samplerate=samplerate, blocking=blocking)
        except Exception as e:
            print(f"[audio] play failed: {e}", flush=True)


def stop() -> None:
    """Stop playback and close the global stream."""
    with _LOCK:
        try:
            sd.stop()
        except Exception as e:
            print(f"[audio] stop failed: {e}", flush=True)
