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

VALID_ASSIGNMENT_STRATEGIES = ("random", "round_robin", "inverse_round_robin")


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
    def __init__(
        self,
        voice_pool: list[str],
        save_path: Path,
        assignment_strategy: str = "random",
    ):
        if not voice_pool:
            raise ValueError("voice_pool must contain at least one voice")
        if assignment_strategy not in VALID_ASSIGNMENT_STRATEGIES:
            raise ValueError(
                f"Unknown assignment_strategy: {assignment_strategy!r}. "
                f"Valid: {VALID_ASSIGNMENT_STRATEGIES}"
            )
        self.voice_pool = list(voice_pool)
        self.save_path = save_path
        self.assignment_strategy = assignment_strategy
        self.current_speaker: str = ""
        # name -> voice_name
        self.assignments: dict[str, str] = {}
        # name -> index in voice_pool (so cycle picks the *next* voice)
        self.cycle_index: dict[str, int] = {}
        # Counter used by round-robin strategies for auto-assigning new
        # speakers. Random strategy ignores it but still increments it
        # so switching between strategies mid-session behaves sensibly.
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
        if not isinstance(data, dict):
            return

        # Assignments: keep only entries where both key and value are strings.
        raw_assignments = data.get("assignments", {})
        if isinstance(raw_assignments, dict):
            self.assignments = {
                name: voice
                for name, voice in raw_assignments.items()
                if isinstance(name, str) and isinstance(voice, str)
            }

        # cycle_index: drop entries that can't be coerced to int.
        raw_cycle = data.get("cycle_index", {})
        if isinstance(raw_cycle, dict):
            clean: dict[str, int] = {}
            for name, idx in raw_cycle.items():
                if not isinstance(name, str):
                    continue
                try:
                    clean[name] = int(idx)
                except (TypeError, ValueError):
                    continue
            self.cycle_index = clean

        # next_auto_index: fall back to number of assignments on malformed input.
        raw_next = data.get("next_auto_index")
        try:
            self._next_auto_index = int(raw_next) if raw_next is not None else len(self.assignments)
        except (TypeError, ValueError):
            self._next_auto_index = len(self.assignments)

        # Note: we intentionally do NOT prune assignments whose voice is no
        # longer in the pool. If the user removes a voice from the Pool in
        # the ini, characters already assigned that voice keep it — the
        # Pool only governs NEW auto-assignments and F2 cycling. Use F2 to
        # reassign a character from the current pool if you want to move
        # them off an out-of-pool voice.

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
        or None if no unique match.

        The extension (the part that goes beyond the stored name) must look
        like a real name continuation — space followed by alphanumerics —
        so OCR garbage like 'B-King' -> 'B-King [' doesn't hijack the
        stored entry."""
        matches = []
        for k in self.assignments:
            if len(k) < self._PREFIX_MIN_LEN or len(k) >= len(name):
                continue
            if not name.startswith(k):
                continue
            extension = name[len(k):]
            # Must be a leading space followed by alphanumerics (or nothing).
            if not extension.startswith(" "):
                continue
            tail = extension[1:]
            if not tail or not all(c.isalnum() for c in tail):
                continue
            matches.append(k)
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
        matches = self._shrink_candidates(name)
        if not matches:
            return None
        if len(matches) == 1:
            return matches[0]
        # Ambiguous — prefer current speaker if it's one of the matches.
        if self.current_speaker in matches:
            return self.current_speaker
        return None

    def _shrink_candidates(self, name: str) -> list[str]:
        """All stored speaker names that start with `name` and are longer."""
        return [
            k for k in self.assignments
            if len(k) > len(name) and k.startswith(name)
        ]

    def _has_garbled_extension(self, name: str) -> bool:
        """True if `name` extends an existing stored name with OCR garbage
        (e.g. stored 'B-King' + new 'B-King [' — space-then-non-alnum).
        Used to reject creating new entries for clearly-garbled reads."""
        for k in self.assignments:
            if len(k) < self._PREFIX_MIN_LEN or len(k) >= len(name):
                continue
            if not name.startswith(k):
                continue
            tail = name[len(k):]
            # A real extension looks like ' <alnum>+' — anything else
            # (bracket, bullet, punctuation after space) is garbage.
            if not tail.startswith(" "):
                continue
            rest = tail[1:]
            if rest and not all(c.isalnum() for c in rest):
                return True
        return False

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
                # Prefix shrink: new reading is a truncation of a stored
                # longer name. Checked BEFORE prefix extension so 'B-King L'
                # binds to the full 'B-King Leader' rather than extending
                # a shorter stored 'B-King'.
                shrink = self._prefix_shrink_match(name)
                if shrink:
                    self.current_speaker = shrink
                    return self.assignments[shrink]
                # Prefix extension: OCR previously read a truncated name
                # (e.g. 'B-King') and now reads the full one (e.g. 'B-King
                # Leader'). Upgrade the stored key to the longer form and
                # keep the voice.
                ext = self._prefix_extension_match(name)
                if ext:
                    voice = self.assignments.pop(ext)
                    idx = self.cycle_index.pop(ext, 0)
                    self.assignments[name] = voice
                    self.cycle_index[name] = idx
                    self.current_speaker = name
                    self._save()
                    return voice
                # If `name` is a prefix of multiple stored names but none of
                # them is the current speaker, it's an ambiguous truncation
                # of an unknown one (e.g. 'B-King' when we have 'B-King
                # Leader' and 'B-King Thug'). Don't create a new entry;
                # keep current_speaker so the next, longer OCR read resolves
                # the ambiguity.
                if self._shrink_candidates(name):
                    return self.assignments.get(self.current_speaker)
                # `name` starts with a stored name but the extension looks
                # like OCR garbage (e.g. stored 'B-King' + new 'B-King [').
                # Don't create an entry for the garbled form.
                if self._has_garbled_extension(name):
                    return self.assignments.get(self.current_speaker)
            # New speaker — pick a voice from the pool per the configured
            # strategy. Assignment persists in speakers.json, so the same
            # character keeps their voice across restarts regardless of
            # which strategy produced the initial pick.
            self.current_speaker = name
            idx = self._pick_new_voice_index()
            self.assignments[name] = self.voice_pool[idx]
            self.cycle_index[name] = idx
            self._next_auto_index += 1
            self._save()
            return self.assignments[name]

    def _pick_new_voice_index(self) -> int:
        """Return the pool index to use for a brand-new speaker, according
        to the configured assignment_strategy. Must be called with the
        lock held."""
        n = len(self.voice_pool)
        if self.assignment_strategy == "round_robin":
            return self._next_auto_index % n
        if self.assignment_strategy == "inverse_round_robin":
            return (n - 1 - self._next_auto_index) % n
        # Default/random: uniform pick across the whole pool.
        return random.randrange(n)

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

    def voice_for_current(self) -> str | None:
        """Return the voice for the current speaker. Falls back to the
        cycled 'no-speaker' default voice if one has been set."""
        if self.current_speaker:
            return self.assignments.get(self.current_speaker)
        return self.assignments.get(DEFAULT_SPEAKER_KEY)
