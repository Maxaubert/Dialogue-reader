"""Backend for the settings app.

Reads/writes dialogue_reader.ini (preserving comments and formatting) and
talks to the running reader over its UDP command server. Exposed to the
pywebview frontend as the `Api` bridge class.
"""

from __future__ import annotations

import os
import re
import socket
import time
from pathlib import Path

import psutil

_REPO = Path(__file__).parent.parent
_DEFAULT_INI = _REPO / "dialogue_reader.ini"
_DEFAULT_SPEAKERS = _REPO / "speakers.json"
_DEFAULT_PROFILES = _REPO / "profiles.json"
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


def _drop_duplicate_options(text: str) -> str:
    """Return `text` with every repeated key in a section removed, keeping the
    FIRST occurrence (configparser's own precedence when it does not raise)."""
    out: list[str] = []
    section: str | None = None
    seen: set[tuple[str, str]] = set()
    for line in text.splitlines():
        stripped = line.strip()
        if stripped.startswith("[") and stripped.endswith("]"):
            section = stripped[1:-1].lower()
            out.append(line)
            continue
        m = _KV_RE.match(line)
        if m and section is not None:
            slot = (section, m.group(2).strip().lower())
            if slot in seen:
                continue
            seen.add(slot)
        out.append(line)
    return "\n".join(out) + "\n"


def update_ini_text(text: str, values: dict[tuple[str, str], str]) -> str:
    """Return `text` with each (section, key) set to its new value. Existing
    lines keep their exact whitespace/separator style; comments and unrelated
    lines are untouched. Missing keys are appended to their section, missing
    sections appended at the end."""
    # Ini keys and sections are case-insensitive to both configparser and
    # AHK's IniRead, so matching them case-sensitively appended a SECOND
    # differently-cased entry -- a duplicate option that makes the whole file
    # unreadable and silently reverts every setting (issue #22). Keys are
    # normalized for lookup; the file's own spelling is preserved on rewrite.
    pending = {(s.lower(), k.lower()): (k, v) for (s, k), v in values.items()}
    result: list[str] = []
    current: str | None = None

    def flush(section: str | None) -> None:
        """Append pending keys for `section` before we leave it, keeping the
        section's trailing blank lines after the inserted keys."""
        if section is None:
            return
        sl = section.lower()
        adds = [(key, orig, v) for (s, key), (orig, v) in list(pending.items())
                if s == sl]
        if not adds:
            return
        tail: list[str] = []
        while result and result[-1].strip() == "":
            tail.insert(0, result.pop())
        for key, orig, v in adds:
            result.append(f"{orig}={v}")
            del pending[(sl, key)]
        result.extend(tail)

    written: set[tuple[str, str]] = set()   # (section, key) already emitted
    for line in text.splitlines():
        stripped = line.strip()
        if stripped.startswith("[") and stripped.endswith("]"):
            flush(current)
            current = stripped[1:-1]
            result.append(line)
            continue
        m = _KV_RE.match(line)
        if m and current is not None:
            key = m.group(2).strip().lower()
            slot = (current.lower(), key)
            if slot in pending:
                _orig, val = pending.pop(slot)
                result.append(
                    f"{m.group(1)}{m.group(2)}{m.group(3)}{val}"
                )
                written.add(slot)
                continue
            if slot in written:
                # A pre-existing duplicate of a key we already wrote. Dropping
                # it HEALS the file: configparser raises DuplicateOptionError
                # on the second copy and every later section is then lost,
                # silently reverting settings to defaults (issue #23).
                print(f"[settings] removing duplicate {m.group(2).strip()!r} "
                      f"in [{current}]")
                continue
            written.add(slot)
        result.append(line)
    flush(current)

    sections: dict[str, list[tuple[str, str]]] = {}
    for (_s, _k), (orig, v) in pending.items():
        # Use the caller's spelling for a section we are creating fresh.
        disp = next(s for (s, k) in values if k == orig)
        sections.setdefault(disp, []).append((orig, v))
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
    except configparser.DuplicateOptionError as e:
        # configparser aborts AT the duplicate, so every later section is
        # missing and would read back as SCHEMA defaults -- which the next
        # save would then write over the user's real settings. Heal a copy
        # in memory and re-parse so the values are still correct (#24).
        print(f"[settings] {Path(ini_path).name} has a duplicate key "
              f"({e.option!r} in [{e.section}]); using the first value. "
              f"Saving from this page removes the duplicate.")
        try:
            text = Path(ini_path).read_text(encoding="utf-8")
            cp = configparser.ConfigParser()
            cp.read_string(_drop_duplicate_options(text))
        except Exception as e2:
            print(f"[settings] could not recover {Path(ini_path).name}: {e2}")
    except Exception as e:
        print(f"[settings] could not read {Path(ini_path).name}: {e}")
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


def _is_reader_process(info: dict) -> bool:
    """True only for OUR supervisor or reader child.

    Matching a substring of the joined command line killed innocent
    bystanders: an editor with dialogue_reader.ahk open, a shell whose
    command line merely mentioned the path, or a second checkout of the repo
    (issue #22). Require both a plausible image name and an argument that
    resolves to exactly one of our two entry points."""
    name = (info.get("name") or "").lower()
    if name not in ("autohotkey.exe", "autohotkey64.exe", "autohotkey32.exe",
                    "autohotkeyu64.exe", "autohotkeyux.exe",
                    "py.exe", "python.exe", "pythonw.exe"):
        return False
    targets = {str((_REPO / n).resolve()).lower()
               for n in ("dialogue_reader.ahk", "main.py")}
    for arg in info.get("cmdline") or []:
        try:
            p = Path(arg)
            # Only ABSOLUTE arguments count. A bare "main.py" would otherwise
            # resolve against OUR cwd -- which is this repo -- and match an
            # unrelated `python main.py` in someone else's project (#24).
            if not p.is_absolute():
                continue
            if str(p.resolve()).lower() in targets:
                return True
        except (OSError, ValueError):
            continue
    return False


def restart_reader() -> bool:
    """Kill the AHK supervisor and its Python child, then relaunch the AHK
    script. Needed only for [Hotkeys]/[Launcher] changes."""
    # Ask the reader to stop first: killing the supervisor bypasses its own
    # cleanup, and a hard kill leaves any media the gate paused paused
    # forever (issue #23). The kills below are the backstop.
    try:
        send_command("QUIT")
        # Wait for it to actually exit (it resumes paused media first)
        # rather than guessing a duration; the kills below are the backstop.
        deadline = time.monotonic() + 2.5
        while time.monotonic() < deadline and reader_running():
            time.sleep(0.05)
    except Exception:
        pass
    for p in psutil.process_iter(["name", "cmdline"]):
        try:
            if _is_reader_process(p.info):
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


def read_profiles(profiles_path: Path | str = _DEFAULT_PROFILES) -> dict:
    """Profiles as written by the reader (the single writer). Read-only."""
    import json
    try:
        data = json.loads(Path(profiles_path).read_text(encoding="utf-8"))
        profiles = data.get("profiles", {})
        return profiles if isinstance(profiles, dict) else {}
    except Exception:
        return {}


# ---- pywebview bridge -------------------------------------------------------

_LIVE_COMMANDS = {
    "TOGGLE_PAUSE", "SPEED_UP", "SPEED_DOWN",
    "CYCLE_VOICE", "CYCLE_VOICE_PREV", "RELOAD_CONFIG",
}


class Api:
    """Methods here are callable from JS as window.pywebview.api.<name>()."""

    def __init__(self, ini_path: Path | str | None = None,
                 speakers_path: Path | str | None = None,
                 profiles_path: Path | str | None = None):
        self._ini = Path(ini_path) if ini_path else _DEFAULT_INI
        self._speakers = Path(speakers_path) if speakers_path else _DEFAULT_SPEAKERS
        self._profiles = Path(profiles_path) if profiles_path else _DEFAULT_PROFILES

    def get_state(self) -> dict:
        return {
            "settings": read_settings(self._ini),
            "schema": SCHEMA,
            "voices": english_voices(),
            "running": reader_running(),
            "no_speaker_voice": no_speaker_voice(self._speakers, self._ini),
            "profiles": read_profiles(self._profiles),
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

    # ---- profiles (mutations go through the reader over UDP) ----

    def profile_save(self, name: str) -> None:
        send_command(f"PROFILE_SAVE:{name.strip()}")

    def profile_apply(self, name: str) -> None:
        send_command(f"PROFILE_APPLY:{name.strip()}")

    def profile_unapply(self, name: str) -> None:
        send_command(f"PROFILE_UNAPPLY:{name.strip()}")

    def profile_delete(self, name: str) -> None:
        send_command(f"PROFILE_DELETE:{name.strip()}")

    def profile_auto(self, name: str, on: bool) -> None:
        send_command(f"PROFILE_AUTO:{name.strip()}:{1 if on else 0}")

    def get_profiles(self) -> dict:
        return read_profiles(self._profiles)

    def reader_status(self) -> bool:
        return reader_running()

    def restart_reader(self) -> bool:
        return restart_reader()
