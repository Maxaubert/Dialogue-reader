"""Game profiles: named snapshots of one process's regions.

Regions are stored window-relative (offsets from the game window's top-left,
plus the window size at save time). Re-applying scales the coordinates to the
window's current size, so a profile saved in 1440p borderless still lands
right in 1080p windowed.

The reader process is the ONLY writer of profiles.json; the settings UI reads
the file for display and mutates through UDP commands (PROFILE_SAVE, ...).
"""

from __future__ import annotations

import json
import os
import threading
from pathlib import Path


def scale_regions(profile: dict, window_rect: tuple[int, int, int, int]) -> list[dict]:
    """Convert a profile's window-relative regions to absolute screen rects
    for the window currently at `window_rect` (x, y, w, h), scaling for any
    size difference from save time."""
    wx, wy, cur_w, cur_h = window_rect
    saved_w = max(1, int(profile.get("window", {}).get("w", cur_w)))
    saved_h = max(1, int(profile.get("window", {}).get("h", cur_h)))
    sx = cur_w / saved_w
    sy = cur_h / saved_h
    out = []
    for r in profile.get("regions", []):
        out.append({
            "x": wx + round(r["rel_x"] * sx),
            "y": wy + round(r["rel_y"] * sy),
            "w": max(1, round(r["w"] * sx)),
            "h": max(1, round(r["h"] * sy)),
            "rotation": float(r.get("rotation", 0.0)),
            "label": r["label"],
            "mode": r.get("mode", "dialogue"),
        })
    return out


class ProfileStore:
    """profiles.json access. All mutations save immediately under a lock
    (the launch watcher thread and the main loop both touch the store)."""

    def __init__(self, path: Path | str):
        self.path = Path(path)
        self._lock = threading.Lock()
        self._profiles: dict[str, dict] = {}
        # Auto-applies handed to the command queue but not yet consumed.
        # In-memory only: a restart re-evaluates from scratch.
        self._claimed: set[str] = set()
        # Profiles the user explicitly unapplied. The watcher must not undo
        # that decision while the game is still running (issue #24); it
        # clears when the game exits or the user applies again.
        self._suppressed: set[str] = set()
        self._load()

    # ---- persistence ----

    @staticmethod
    def _valid(p) -> bool:
        """A profile we can actually apply. A malformed entry (hand-edited, or
        written by another version) used to raise KeyError deep inside
        scale_regions and take the whole reader down, and with apply-on-launch
        set that became a relaunch loop (issue #22)."""
        if not isinstance(p, dict) or not p.get("process"):
            return False
        if not isinstance(p.get("regions"), list):
            return False
        win = p.get("window")
        if win is not None:
            if not isinstance(win, dict):
                return False
            for k in ("w", "h"):
                v = win.get(k)
                if isinstance(v, bool) or not isinstance(v, (int, float)) or v <= 0:
                    return False
        for r in p["regions"]:
            if not isinstance(r, dict):
                return False
            if not all(k in r for k in ("rel_x", "rel_y", "w", "h", "label")):
                return False
            rot = r.get("rotation")
            if rot is not None and (isinstance(rot, bool)
                                    or not isinstance(rot, (int, float))):
                return False
            # Coordinates must be NUMBERS, not merely present: a hand-edited
            # "120" sails through scale_regions' arithmetic and blows up
            # halfway through building the replacement regions (issue #23).
            for k in ("rel_x", "rel_y", "w", "h"):
                if isinstance(r[k], bool) or not isinstance(r[k], (int, float)):
                    return False
            if not isinstance(r["label"], str) or not r["label"]:
                return False
        return True

    def _load(self) -> None:
        try:
            raw = self.path.read_text(encoding="utf-8")
        except OSError:
            return
        try:
            data = json.loads(raw)
        except ValueError:
            # A torn write (the process is routinely force-killed) must not
            # silently become an empty store that we then overwrite: keep the
            # evidence aside so the data is recoverable.
            self._quarantine(raw)
            return
        profiles = data.get("profiles", {}) if isinstance(data, dict) else {}
        if not isinstance(profiles, dict):
            return
        for name, p in profiles.items():
            if not isinstance(name, str):
                continue
            if self._valid(p):
                self._profiles[name] = p
            else:
                print(f"[profiles] dropping malformed profile {name!r}",
                      flush=True)

    def _quarantine(self, raw: str) -> None:
        bad = self.path.with_suffix(".corrupt")
        try:
            bad.write_text(raw, encoding="utf-8")
            print(f"[profiles] {self.path.name} is corrupt; saved a copy as "
                  f"{bad.name} and starting empty", flush=True)
        except OSError:
            pass

    def _save(self) -> None:
        """Atomic: write a sibling temp file then os.replace it over the
        target. A kill mid-write previously left a truncated file that the
        next start silently reset to empty, losing every profile."""
        tmp = self.path.with_suffix(".tmp")
        try:
            tmp.write_text(
                json.dumps({"profiles": self._profiles}, indent=2,
                           ensure_ascii=False),
                encoding="utf-8",
            )
            os.replace(tmp, self.path)
        except OSError:
            try:
                tmp.unlink(missing_ok=True)
            except OSError:
                pass

    # ---- reads ----

    def names(self) -> list[str]:
        with self._lock:
            return list(self._profiles)

    def get(self, name: str) -> dict | None:
        with self._lock:
            p = self._profiles.get(name)
            return dict(p) if p is not None else None

    def auto_pending(self, running_exes: set[str]) -> list[str]:
        """Profiles that want apply-on-launch, are not applied, and whose
        game is currently running."""
        with self._lock:
            return [
                name for name, p in self._profiles.items()
                if p.get("apply_on_launch")
                and not p.get("applied")
                and p.get("process") in running_exes
                and name not in self._claimed
                and name not in self._suppressed
            ]

    def claim_auto(self, running_exes: set[str]) -> list[str]:
        """auto_pending(), but each name is handed out ONCE.

        The watcher runs every 3s while the main thread may be blocked for a
        minute inside the region manager, so an unclaimed check queued the
        same apply twenty times; the backlog then replayed, each copy wiping
        the regions the user had just drawn (issue #23). The claim is
        released when the apply completes (set_applied_exclusive) or when the
        game exits (mark_unapplied_for_process)."""
        names = self.auto_pending(running_exes)
        with self._lock:
            self._claimed.update(names)
        return names

    # ---- mutations (reader main loop / watcher only) ----

    def snapshot(self, name: str, process: str,
                 window_rect: tuple[int, int, int, int],
                 outlines: list[dict]) -> None:
        """Store `outlines` (absolute screen rects: x, y, w, h, rotation,
        label, mode) relative to `window_rect` under profile `name`."""
        wx, wy, ww, wh = window_rect
        prev = self._profiles.get(name, {})
        with self._lock:
            self._profiles[name] = {
                "process": process,
                "window": {"w": int(ww), "h": int(wh)},
                "regions": [
                    {
                        "rel_x": int(o["x"]) - wx,
                        "rel_y": int(o["y"]) - wy,
                        "w": int(o["w"]),
                        "h": int(o["h"]),
                        "rotation": float(o.get("rotation", 0.0)),
                        "label": o["label"],
                        "mode": o.get("mode", "dialogue"),
                    }
                    for o in outlines
                ],
                "apply_on_launch": bool(prev.get("apply_on_launch", False)),
                "applied": False,
            }
            self._save()

    def delete(self, name: str) -> None:
        with self._lock:
            self._profiles.pop(name, None)
            self._save()

    def set_auto(self, name: str, on: bool) -> None:
        with self._lock:
            if name in self._profiles:
                self._profiles[name]["apply_on_launch"] = bool(on)
                self._save()

    def set_applied(self, name: str, on: bool) -> None:
        with self._lock:
            if name in self._profiles:
                self._profiles[name]["applied"] = bool(on)
                if not on:
                    # An explicit unapply is a user decision; keep the
                    # watcher from re-applying it under them (issue #24).
                    self._suppressed.add(name)
                    self._claimed.discard(name)
                self._save()

    def release_claim(self, name: str) -> None:
        """Let the watcher consider `name` again (the apply finished, failed,
        or its game vanished)."""
        with self._lock:
            self._claimed.discard(name)

    def set_applied_exclusive(self, name: str, process: str) -> None:
        """Mark `name` applied and clear every other profile for the same
        process. Only one layout can be on screen per game, and two profiles
        both flagged applied made a later delete rip out the live regions of
        whichever one was actually showing (issue #22)."""
        with self._lock:
            for other, p in self._profiles.items():
                if other != name and p.get("process") == process:
                    p["applied"] = False
            if name in self._profiles:
                self._profiles[name]["applied"] = True
            self._claimed.discard(name)      # the queued apply is consumed
            self._suppressed.discard(name)   # applying again clears an unapply
            self._save()

    def mark_unapplied_for_process(self, process: str) -> None:
        """Game exited: its profiles become re-applicable on next launch."""
        with self._lock:
            changed = False
            for name, p in self._profiles.items():
                if p.get("process") != process:
                    continue
                self._claimed.discard(name)     # re-armable on next launch
                self._suppressed.discard(name)  # a relaunch is a fresh start
                if p.get("applied"):
                    p["applied"] = False
                    changed = True
            if changed:
                self._save()

    def reset_applied(self) -> None:
        """Fresh reader start: nothing is applied yet."""
        with self._lock:
            for p in self._profiles.values():
                p["applied"] = False
            self._save()
