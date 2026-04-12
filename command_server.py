"""
UDP command server for AHK -> Python communication.

Listens on 127.0.0.1:<port> for newline-terminated text commands and pushes
them to a queue that the main loop drains.

Commands (one per packet):
    PICK_REGION          - launch region picker (dialogue mode)
    PICK_SPEAKER         - launch region picker (speaker-name mode)
    CLEAR_REGIONS        - drop all current regions
    SPEED_UP             - bump TTS speed +0.1
    SPEED_DOWN           - bump TTS speed -0.1
    PAUSE                - stop watching + interrupt current speech
    UNPAUSE              - resume watching
    TOGGLE_PAUSE         - flip pause state
    CYCLE_VOICE          - cycle current speaker's voice to next in pool
    SET_SPEAKER:<name>   - manually set the current speaker (for OCR-
                           unreadable name UI). Name preserves case +
                           Unicode after the colon.
"""

from __future__ import annotations

import queue
import socket
import threading


DEFAULT_PORT = 7849


class CommandServer:
    def __init__(self, port: int = DEFAULT_PORT):
        self.port = port
        self.queue: queue.Queue[str] = queue.Queue()
        self._sock: socket.socket | None = None
        self._thread: threading.Thread | None = None
        self._stop = threading.Event()

    def start(self) -> None:
        self._sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        self._sock.bind(("127.0.0.1", self.port))
        # Short timeout so the recv loop can periodically check _stop.
        self._sock.settimeout(0.2)
        self._thread = threading.Thread(target=self._run, daemon=True)
        self._thread.start()

    def _run(self) -> None:
        assert self._sock is not None
        while not self._stop.is_set():
            try:
                data, _addr = self._sock.recvfrom(1024)
            except socket.timeout:
                continue
            except OSError:
                return
            text = data.decode("utf-8", errors="replace").strip()
            if not text:
                continue
            # Uppercase the command name but PRESERVE any payload that comes
            # after the first colon — so SET_SPEAKER:Haru stays "Haru" and
            # doesn't get mangled to "HARU". Also lets non-ASCII names
            # (Japanese characters etc.) survive intact.
            if ":" in text:
                head, _, tail = text.partition(":")
                cmd = head.strip().upper() + ":" + tail
            else:
                cmd = text.upper()
            self.queue.put(cmd)

    def stop(self) -> None:
        self._stop.set()
        if self._sock is not None:
            try:
                self._sock.close()
            except Exception:
                pass
