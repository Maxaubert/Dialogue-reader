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
import random
import threading
from pathlib import Path


# Reserved pseudo-speaker name for the "no speaker detected" state. Treating
# it as a regular entry lets the user cycle its voice with the usual hotkey.
DEFAULT_SPEAKER_KEY = "__default__"


def _is_one_edit(longer: str, shorter: str) -> bool:
    """True if `shorter` is `longer` with exactly one character removed."""
    if len(longer) != len(shorter) + 1:
        return False
    i = 0
    j = 0
    diffs = 0
    while i < len(longer) and j < len(shorter):
        if longer[i] != shorter[j]:
            diffs += 1
            if diffs > 1:
                return False
            i += 1  # skip the extra char in longer
        else:
            i += 1
            j += 1
    return True


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

        # Drop any stored assignment whose voice isn't in the current pool
        # (user edited the Pool in the ini). These speakers will be
        # reassigned on next speak() via set_current()'s round-robin.
        pool_set = set(self.voice_pool)
        stale = [
            name for name, voice in self.assignments.items()
            if voice not in pool_set
        ]
        for name in stale:
            del self.assignments[name]
            self.cycle_index.pop(name, None)
        if stale:
            self._save()

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

    def _fuzzy_match(self, name: str) -> str | None:
        """Find an existing speaker whose name differs from `name` by at
        most one character (insertion, deletion, or substitution). Returns
        the matched name, or None."""
        for known in self.assignments:
            if len(known) == len(name):
                # substitution: exactly one char differs
                if sum(a != b for a, b in zip(known, name)) == 1:
                    return known
            elif len(known) == len(name) + 1:
                # deletion: `name` is `known` with one char removed
                if _is_one_edit(known, name):
                    return known
            elif len(known) == len(name) - 1:
                # insertion: `name` is `known` with one char added
                if _is_one_edit(name, known):
                    return known
        return None

    # Minimum common-prefix length for prefix-extension / prefix-shrink
    # matches. Prevents short OCR partials like "B" matching "Bob" & "Ben"
    # by accident.
    _PREFIX_MIN_LEN = 4

    def _prefix_extension_match(self, name: str) -> str | None:
        """Find an existing speaker whose name is a proper prefix of `name`
        (the new reading extends the stored truncated one). E.g. stored
        'B-King' + new 'B-King Leader'. Returns the stored shorter name,
        or None if no unique match."""
        matches = [
            k for k in self.assignments
            if len(k) >= self._PREFIX_MIN_LEN
            and len(k) < len(name)
            and name.startswith(k)
        ]
        if len(matches) != 1:
            return None
        return matches[0]

    def _prefix_shrink_match(self, name: str) -> str | None:
        """Find an existing speaker whose name starts with `name` (the new
        reading is a truncation of a stored longer one). E.g. stored
        'B-King Leader' + new 'B-King'. Prefers the current speaker if
        ambiguous, else returns the unique match or None."""
        if len(name) < self._PREFIX_MIN_LEN:
            return None
        matches = [
            k for k in self.assignments
            if len(k) > len(name) and k.startswith(name)
        ]
        if not matches:
            return None
        if len(matches) == 1:
            return matches[0]
        # Ambiguous — prefer current speaker if it's one of the matches.
        if self.current_speaker in matches:
            return self.current_speaker
        return None

    def set_current(self, name: str, fuzzy: bool = True) -> str | None:
        """Update the current speaker. Looks up by exact match first, then
        by single-character fuzzy match (if fuzzy=True). If still no match,
        creates a new speaker with the next voice in the pool. Returns the
        assigned voice name, or None if `name` is empty."""
        name = (name or "").strip()
        if not name:
            return None
        with self._lock:
            # Exact match.
            if name in self.assignments:
                self.current_speaker = name
                return self.assignments[name]
            # Fuzzy match: single char mismatch.
            if fuzzy:
                match = self._fuzzy_match(name)
                if match:
                    self.current_speaker = match
                    return self.assignments[match]
                # Prefix extension: OCR previously read a truncated name
                # (e.g. 'B-King') and now read the full one (e.g. 'B-King
                # Leader'). Upgrade the stored key to the longer form and
                # keep the voice assignment.
                ext = self._prefix_extension_match(name)
                if ext:
                    voice = self.assignments.pop(ext)
                    idx = self.cycle_index.pop(ext, 0)
                    self.assignments[name] = voice
                    self.cycle_index[name] = idx
                    self.current_speaker = name
                    self._save()
                    return voice
                # Prefix shrink: OCR truncated the name mid-dialogue. Reuse
                # the stored longer name's voice without creating a new entry.
                shrink = self._prefix_shrink_match(name)
                if shrink:
                    self.current_speaker = shrink
                    return self.assignments[shrink]
            # New speaker — pick a random voice from the pool. Random is
            # preferred over round-robin when the pool is very large and we
            # want variety across runs / sessions. The assignment persists
            # in speakers.json so the same character keeps their voice.
            self.current_speaker = name
            idx = random.randrange(len(self.voice_pool))
            self.assignments[name] = self.voice_pool[idx]
            self.cycle_index[name] = idx
            self._next_auto_index += 1
            self._save()
            return self.assignments[name]

    def cycle_current_voice(self, direction: int = 1) -> tuple[str, str] | None:
        """Reroll the current speaker's voice. With a 1000+ voice pool, linear
        cycling takes forever to sweep, so we pick a random one instead (but
        still avoid re-picking the exact current voice). `direction` is kept
        in the signature for compatibility with the CycleVoicePrev hotkey but
        is ignored — both next and prev simply re-roll."""
        with self._lock:
            name = self.current_speaker or DEFAULT_SPEAKER_KEY
            current_voice = self.assignments.get(name)
            if len(self.voice_pool) == 1:
                new_voice = self.voice_pool[0]
                new_idx = 0
            else:
                # Sample until we land on something different from current.
                for _ in range(10):
                    new_idx = random.randrange(len(self.voice_pool))
                    if self.voice_pool[new_idx] != current_voice:
                        break
                new_voice = self.voice_pool[new_idx]
            self.assignments[name] = new_voice
            self.cycle_index[name] = new_idx
            self._save()
            display = "(no speaker)" if name == DEFAULT_SPEAKER_KEY else name
            return (display, new_voice)

    def voice_for(self, name: str) -> str | None:
        return self.assignments.get((name or "").strip()) if name else None

    def voice_for_current(self) -> str | None:
        """Return the voice for the current speaker. Falls back to the
        cycled 'no-speaker' default voice if one has been set."""
        if self.current_speaker:
            return self.assignments.get(self.current_speaker)
        return self.assignments.get(DEFAULT_SPEAKER_KEY)
