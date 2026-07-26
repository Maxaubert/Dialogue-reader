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
        self._load()

    # ---- persistence ----

    def _load(self) -> None:
        try:
            data = json.loads(self.path.read_text(encoding="utf-8"))
            profiles = data.get("profiles", {})
            if isinstance(profiles, dict):
                self._profiles = {
                    name: p for name, p in profiles.items()
                    if isinstance(name, str) and isinstance(p, dict)
                }
        except Exception:
            self._profiles = {}

    def _save(self) -> None:
        try:
            self.path.write_text(
                json.dumps({"profiles": self._profiles}, indent=2,
                           ensure_ascii=False),
                encoding="utf-8",
            )
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
            ]

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
                self._save()

    def mark_unapplied_for_process(self, process: str) -> None:
        """Game exited: its profiles become re-applicable on next launch."""
        with self._lock:
            changed = False
            for p in self._profiles.values():
                if p.get("process") == process and p.get("applied"):
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
