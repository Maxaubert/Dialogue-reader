"""
Speaker → voice mapping with persistent storage and round-robin assignment.

A "speaker region" feeds names into SpeakerManager.set_current(). The first
time a name is seen, it's auto-assigned the next voice in the configured
pool (which is ordered female/male/female/male/... so the default spread
covers both genders). The user can cycle the current speaker's voice via
SpeakerManager.cycle_current_voice() — pressing the cycle hotkey repeatedly
walks through the pool until they find a voice they like.

State persists to a JSON file next to the script so assignments survive
restarts and re-runs.
"""

from __future__ import annotations

import json
import threading
from pathlib import Path


class SpeakerManager:
    def __init__(self, voice_pool: list[str], save_path: Path):
        if not voice_pool:
            raise ValueError("voice_pool must contain at least one voice")
        self.voice_pool = list(voice_pool)
        self.save_path = save_path
        self.current_speaker: str = ""
        # name -> voice_name
        self.assignments: dict[str, str] = {}
        # name -> index in voice_pool (so cycle picks the *next* voice)
        self.cycle_index: dict[str, int] = {}
        # Counter used to round-robin auto-assignment of new speakers.
        self._next_auto_index: int = 0
        self._lock = threading.Lock()
        self._load()

    # ---- persistence ----

    def _load(self) -> None:
        if not self.save_path.exists():
            return
        try:
            data = json.loads(self.save_path.read_text(encoding="utf-8"))
        except (json.JSONDecodeError, OSError):
            return
        self.assignments = dict(data.get("assignments", {}))
        self.cycle_index = {k: int(v) for k, v in data.get("cycle_index", {}).items()}
        self._next_auto_index = int(data.get("next_auto_index", len(self.assignments)))

    def _save(self) -> None:
        try:
            self.save_path.write_text(
                json.dumps(
                    {
                        "assignments": self.assignments,
                        "cycle_index": self.cycle_index,
                        "next_auto_index": self._next_auto_index,
                    },
                    indent=2,
                    ensure_ascii=False,
                ),
                encoding="utf-8",
            )
        except OSError:
            pass

    # ---- speaker tracking ----

    def set_current(self, name: str) -> str | None:
        """Update the current speaker. If the name is brand new, auto-assign
        the next voice in the pool. Returns the assigned voice name (new or
        existing), or None if `name` is empty."""
        name = (name or "").strip()
        if not name:
            return None
        with self._lock:
            self.current_speaker = name
            if name not in self.assignments:
                idx = self._next_auto_index % len(self.voice_pool)
                self.assignments[name] = self.voice_pool[idx]
                self.cycle_index[name] = idx
                self._next_auto_index += 1
                self._save()
            return self.assignments[name]

    def cycle_current_voice(self, direction: int = 1) -> tuple[str, str] | None:
        """Cycle the current speaker's voice along the pool.
        direction = +1 → next voice. direction = -1 → previous voice.
        Returns (speaker_name, new_voice_name), or None if no current
        speaker is set."""
        with self._lock:
            name = self.current_speaker
            if not name:
                return None
            current_idx = self.cycle_index.get(name, -1)
            # Python's % wraps negatives correctly: (-1) % 10 == 9
            new_idx = (current_idx + direction) % len(self.voice_pool)
            new_voice = self.voice_pool[new_idx]
            self.assignments[name] = new_voice
            self.cycle_index[name] = new_idx
            self._save()
            return (name, new_voice)

    def voice_for(self, name: str) -> str | None:
        return self.assignments.get((name or "").strip()) if name else None

    def voice_for_current(self) -> str | None:
        return self.assignments.get(self.current_speaker) if self.current_speaker else None
