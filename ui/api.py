"""Backend for the settings app.

Reads/writes dialogue_reader.ini (preserving comments and formatting) and
talks to the running reader over its UDP command server. Exposed to the
pywebview frontend as the `Api` bridge class.
"""

from __future__ import annotations

import re
import socket
from pathlib import Path

_REPO = Path(__file__).parent.parent
_DEFAULT_INI = _REPO / "dialogue_reader.ini"
_DEFAULT_SPEAKERS = _REPO / "speakers.json"
_DEFAULT_PORT = 7849

# (type, default[, choices]) per section/key. Drives typed reads and the
# frontend form. Hotkeys are AHK-side: edits need a reader restart.
SCHEMA = {
    "Hotkeys": {
        "PickRegion": ("str", "F1"),
        "PickSpeakerRegion": ("str", "+F1"),
        "ClearRegions": ("str", "^F1"),
        "SpeedDown": ("str", "PgDn"),
        "SpeedUp": ("str", "PgUp"),
        "TogglePause": ("str", "End"),
        "CycleVoice": ("str", "F2"),
        "CycleVoicePrev": ("str", "^F2"),
    },
    "Media": {
        "PauseDuringSpeech": ("bool", True),
        "ResumeDelayMs": ("int", 1000),
    },
    "Capture": {"Mode": ("choice", "auto", ["auto", "screen", "window"])},
    "OCR": {
        "Dialogue": ("choice", "winocr", ["winocr", "easyocr"]),
        "Speaker": ("choice", "easyocr", ["winocr", "easyocr"]),
    },
    "Speakers": {
        "AssignmentStrategy": (
            "choice", "random",
            ["random", "round_robin", "inverse_round_robin"],
        ),
    },
    "Polling": {"TextConfirmPolls": ("int", 2)},
    "Magnifier": {"SkipWhenZoomed": ("bool", True)},
    "Voices": {
        "Default": ("str", "kokoro:af_heart"),
        "Pool": ("str", "kokoro:all"),
    },
}

# Kokoro v1.0 English voice names, used when the voices .bin isn't readable.
_ENGLISH_FALLBACK = [
    "af_alloy", "af_aoede", "af_bella", "af_heart", "af_jessica", "af_kore",
    "af_nicole", "af_nova", "af_river", "af_sarah", "af_sky",
    "am_adam", "am_echo", "am_eric", "am_fenrir", "am_liam", "am_michael",
    "am_onyx", "am_puck", "am_santa",
    "bf_alice", "bf_emma", "bf_isabella", "bf_lily",
    "bm_daniel", "bm_fable", "bm_george", "bm_lewis",
]


# ---- ini editing (comment/format preserving) --------------------------------

_KV_RE = re.compile(r"^(\s*)([^=;#\[\s][^=]*?)(\s*=\s*)(.*)$")


def update_ini_text(text: str, values: dict[tuple[str, str], str]) -> str:
    """Return `text` with each (section, key) set to its new value. Existing
    lines keep their exact whitespace/separator style; comments and unrelated
    lines are untouched. Missing keys are appended to their section, missing
    sections appended at the end."""
    pending = dict(values)
    result: list[str] = []
    current: str | None = None

    def flush(section: str | None) -> None:
        """Append pending keys for `section` before we leave it, keeping the
        section's trailing blank lines after the inserted keys."""
        if section is None:
            return
        adds = [(k, v) for (s, k), v in list(pending.items()) if s == section]
        if not adds:
            return
        tail: list[str] = []
        while result and result[-1].strip() == "":
            tail.insert(0, result.pop())
        for k, v in adds:
            result.append(f"{k}={v}")
            del pending[(section, k)]
        result.extend(tail)

    for line in text.splitlines():
        stripped = line.strip()
        if stripped.startswith("[") and stripped.endswith("]"):
            flush(current)
            current = stripped[1:-1]
            result.append(line)
            continue
        m = _KV_RE.match(line)
        if m and current is not None:
            key = m.group(2).strip()
            if (current, key) in pending:
                result.append(
                    f"{m.group(1)}{m.group(2)}{m.group(3)}"
                    f"{pending.pop((current, key))}"
                )
                continue
        result.append(line)
    flush(current)

    sections: dict[str, list[tuple[str, str]]] = {}
    for (s, k), v in pending.items():
        sections.setdefault(s, []).append((k, v))
    for s, kvs in sections.items():
        if result and result[-1].strip() != "":
            result.append("")
        result.append(f"[{s}]")
        for k, v in kvs:
            result.append(f"{k}={v}")

    return "\n".join(result) + ("\n" if text.endswith("\n") or not text else "")


def _parse(spec: tuple, raw: str | None):
    kind, default = spec[0], spec[1]
    if raw is None:
        return default
    raw = raw.strip()
    if kind == "bool":
        low = raw.lower()
        if low in ("true", "1", "yes", "on"):
            return True
        if low in ("false", "0", "no", "off"):
            return False
        return default
    if kind == "int":
        try:
            return int(raw)
        except ValueError:
            return default
    if kind == "choice":
        return raw.lower() if raw.lower() in spec[2] else default
    return raw


def read_settings(ini_path: Path | str = _DEFAULT_INI) -> dict:
    """Typed settings per SCHEMA, with defaults for anything missing."""
    import configparser
    cp = configparser.ConfigParser()
    try:
        cp.read(ini_path, encoding="utf-8")
    except Exception:
        pass
    out: dict = {}
    for section, keys in SCHEMA.items():
        out[section] = {}
        for key, spec in keys.items():
            raw = cp.get(section, key, fallback=None)
            out[section][key] = _parse(spec, raw)
    return out


def _to_ini_str(value) -> str:
    if isinstance(value, bool):
        return "true" if value else "false"
    return str(value)


# ---- reader communication ---------------------------------------------------

def send_command(cmd: str, host: str = "127.0.0.1",
                 port: int = _DEFAULT_PORT) -> None:
    s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    try:
        s.sendto(cmd.encode("utf-8"), (host, port))
    finally:
        s.close()


def reader_running(port: int = _DEFAULT_PORT) -> bool:
    """The reader holds UDP `port`; if we can bind it, nothing is there."""
    s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    try:
        s.bind(("127.0.0.1", port))
        return False
    except OSError:
        return True
    finally:
        s.close()


def restart_reader() -> bool:
    """Kill the AHK supervisor and its Python child, then relaunch the AHK
    script. Needed only for [Hotkeys]/[Launcher] changes."""
    import os
    import psutil
    for p in psutil.process_iter(["cmdline"]):
        try:
            cl = " ".join(p.info["cmdline"] or [])
            if "dialogue_reader.ahk" in cl or "Dialogue-reader\\main.py" in cl:
                p.kill()
        except Exception:
            pass
    os.startfile(str(_REPO / "dialogue_reader.ahk"))
    return True


# ---- voices -----------------------------------------------------------------

def english_voices(voices_bin: Path | str | None = None) -> list[str]:
    """All English Kokoro voices as kokoro:<name>, read from the voices .bin
    (a numpy archive: keys are voice names) with a hardcoded fallback."""
    if voices_bin is None:
        voices_bin = _REPO / "voices" / "kokoro" / "voices-v1.0.bin"
    names: list[str] = []
    try:
        import numpy as np
        names = sorted(np.load(str(voices_bin)).files)
    except Exception:
        names = list(_ENGLISH_FALLBACK)
    english = [n for n in names if n[:3] in ("af_", "am_", "bf_", "bm_")]
    return [f"kokoro:{n}" for n in (english or _ENGLISH_FALLBACK)]


def no_speaker_voice(speakers_path: Path | str = _DEFAULT_SPEAKERS,
                     ini_path: Path | str = _DEFAULT_INI) -> str:
    """The voice actually used when no speaker is detected: the persisted
    __default__ assignment in speakers.json (written by F2 cycling and
    SET_NO_SPEAKER_VOICE), falling back to the ini Voices.Default."""
    import json
    try:
        data = json.loads(Path(speakers_path).read_text(encoding="utf-8"))
        v = data.get("assignments", {}).get("__default__")
        if isinstance(v, str) and v:
            return v
    except Exception:
        pass
    return read_settings(ini_path)["Voices"]["Default"]


# ---- pywebview bridge -------------------------------------------------------

_LIVE_COMMANDS = {
    "TOGGLE_PAUSE", "SPEED_UP", "SPEED_DOWN",
    "CYCLE_VOICE", "CYCLE_VOICE_PREV", "RELOAD_CONFIG",
}


class Api:
    """Methods here are callable from JS as window.pywebview.api.<name>()."""

    def __init__(self, ini_path: Path | str | None = None,
                 speakers_path: Path | str | None = None):
        self._ini = Path(ini_path) if ini_path else _DEFAULT_INI
        self._speakers = Path(speakers_path) if speakers_path else _DEFAULT_SPEAKERS

    def get_state(self) -> dict:
        return {
            "settings": read_settings(self._ini),
            "schema": SCHEMA,
            "voices": english_voices(),
            "running": reader_running(),
            "no_speaker_voice": no_speaker_voice(self._speakers, self._ini),
        }

    def save_settings(self, values: dict) -> bool:
        """values: {section: {key: typed value}}. Writes the ini preserving
        formatting, then hot-applies via RELOAD_CONFIG."""
        flat = {
            (section, key): _to_ini_str(v)
            for section, kv in values.items()
            for key, v in kv.items()
        }
        text = ""
        if self._ini.exists():
            text = self._ini.read_text(encoding="utf-8")
        self._ini.write_text(update_ini_text(text, flat), encoding="utf-8")
        send_command("RELOAD_CONFIG")
        return True

    def preview_voice(self, voice: str) -> None:
        send_command(f"PREVIEW_VOICE:{voice}")

    def set_no_speaker_voice(self, voice: str) -> None:
        """Pin the reader's live no-speaker voice (persisted by the reader
        into speakers.json, avoiding file races with its own saves)."""
        send_command(f"SET_NO_SPEAKER_VOICE:{voice}")

    def live_command(self, cmd: str) -> None:
        if cmd in _LIVE_COMMANDS:
            send_command(cmd)

    def reader_status(self) -> bool:
        return reader_running()

    def restart_reader(self) -> bool:
        return restart_reader()
