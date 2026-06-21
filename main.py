"""
Dialogue Reader — multi-region polling loop with AHK/UDP control.

Run:
    python main.py                  # normal mode, no regions yet
    python main.py --debug          # verbose
    python main.py --pick-on-start  # immediately open the region picker

Send commands via UDP to 127.0.0.1:7849 (see command_server.py for the list).
The companion `dialogue_reader.ahk` script binds these to hotkeys.

Inside the running process:
    - Multiple regions are watched in one loop. When a region's pixels change
      and stabilize, all regions are OCR'd in order and concatenated:
      "{region 0 text}: {region 1 text}". This lets you have one region for
      the speaker name and another for the dialogue body.
    - PAUSE stops watching AND interrupts current speech. UNPAUSE resumes.
    - Closing the AHK script kills this process (no in-app shutdown button).
"""

from __future__ import annotations

import ctypes
import os
import re
import sys
import threading
import time
import queue
from dataclasses import dataclass, field
from difflib import SequenceMatcher
from pathlib import Path

import numpy as np
import sounddevice as sd

from region_picker import pick_region
from capture import RegionCapture
from ocr import OCR, OCRWorker, OCRBatchJob, OCRBatchResult, OCRRegionSpec
from magnifier import is_zoomed as _magnifier_is_zoomed, get_magnification_level as _magnifier_level
from tts import TTS, DEFAULT_VOICE
from window_capture import find_window_at, get_window_title
from command_server import CommandServer, DEFAULT_PORT
from speakers import SpeakerManager


MIN_TEXT_LEN = 2
# Per-region OCR dedup: if the new OCR text for a region is this similar to
# the region's previous OCR text AND the lengths are nearly identical, treat
# it as cosmetic jitter (Magnifier zoom, DPI re-render, anti-alias drift)
# and don't re-speak.
PER_REGION_DEDUP = 0.85
POLL_HZ = 12.0
STABLE_MS = 350

# Dialogue text stabilization: require this many consecutive identical OCR
# results (on fresh snapshots, taken ~100ms apart) before speaking. Prevents
# reading partial typewriter text. User-configurable via dialogue_reader.ini
# [Polling] TextConfirmPolls=<n>.  1 = no extra confirmation (old behavior).
TEXT_CONFIRM_POLLS = 3
TEXT_CONFIRM_INTERVAL = 0.10  # seconds between confirmation polls
TEXT_CONFIRM_MAX_MULTIPLIER = 4  # max attempts = polls * this
TEXT_CONFIRM_HARD_CAP = 30  # absolute ceiling regardless of polls setting

_PUNCT_RE = re.compile(r"[^\w\s]")


# ---- voice pool config ---------------------------------------------------
#
# The pool is read from dialogue_reader.ini's [Voices] section. Order
# matters: it's used round-robin when assigning voices to new speakers, so
# alternating M/F/M/F gives a natural spread for the first few characters.

# 12 curated Kokoro English voices, ordered F/M alternating by official grade
# (hexgrad/Kokoro-82M VOICES.md). High-quality voices for round-robin assignment.
_DEFAULT_VOICE_POOL = [
    "kokoro:af_heart",      # F  grade A   (US)
    "kokoro:am_fenrir",     # M  grade C+  (US)
    "kokoro:af_bella",      # F  grade A-  (US)
    "kokoro:am_michael",    # M  grade C+  (US)
    "kokoro:bf_emma",       # F  grade B-  (British)
    "kokoro:am_puck",       # M  grade C+  (US)
    "kokoro:af_nicole",     # F  grade B-  (US, ASMR/whisper)
    "kokoro:bm_fable",      # M  grade C   (British)
    "kokoro:af_aoede",      # F  grade C+  (US)
    "kokoro:bm_george",     # M  grade C   (British)
    "kokoro:af_kore",       # F  grade C+  (US)
    "kokoro:af_sarah",      # F  grade C+  (US)
]

_KOKORO_ALL = [v for v in _DEFAULT_VOICE_POOL if v.startswith("kokoro:")]


def _expand_voice(voice: str) -> list[str]:
    """Expand shorthand voice entries into a concrete list.

    Supported forms:
      - `kokoro:all`  -> every curated Kokoro voice
      - any other single voice passes through unchanged.
    """
    v = voice.strip()
    if not v:
        return []
    if v == "kokoro:all":
        return list(_KOKORO_ALL)
    return [v]


def _expand_pool(pool: list[str]) -> list[str]:
    """Apply _expand_voice to every entry and de-duplicate while keeping order."""
    seen: set[str] = set()
    out: list[str] = []
    for entry in pool:
        for expanded in _expand_voice(entry):
            if expanded not in seen:
                seen.add(expanded)
                out.append(expanded)
    return out


def _load_voice_config() -> tuple[list[str], str]:
    """Read [Voices] section from dialogue_reader.ini. Falls back to the
    built-in default pool if the file is missing or the section isn't
    there. Returns (pool, default_voice). default_voice is guaranteed to
    be inside pool."""
    ini_path = Path(__file__).parent / "dialogue_reader.ini"
    pool = list(_DEFAULT_VOICE_POOL)
    default = DEFAULT_VOICE
    if ini_path.exists():
        import configparser
        cp = configparser.ConfigParser()
        try:
            cp.read(ini_path, encoding="utf-8")
        except Exception:
            cp = None
        if cp and cp.has_section("Voices"):
            raw = cp.get("Voices", "Pool", fallback="")
            if raw.strip():
                pool = [v.strip() for v in raw.split(",") if v.strip()]
            default = cp.get("Voices", "Default", fallback=default).strip() or default
    pool = _expand_pool(pool)
    if default not in pool:
        pool.insert(0, default)
    return pool, default


def _load_speaker_assignment_strategy() -> str:
    """Read [Speakers] AssignmentStrategy from dialogue_reader.ini. Valid
    values: random, round_robin, inverse_round_robin. Unknown or missing
    values fall back to 'random' (previous behavior)."""
    ini_path = Path(__file__).parent / "dialogue_reader.ini"
    if not ini_path.exists():
        return "random"
    import configparser
    cp = configparser.ConfigParser()
    try:
        cp.read(ini_path, encoding="utf-8")
    except Exception:
        return "random"
    raw = cp.get("Speakers", "AssignmentStrategy", fallback="random").strip().lower()
    from speakers import VALID_ASSIGNMENT_STRATEGIES
    if raw in VALID_ASSIGNMENT_STRATEGIES:
        return raw
    print(f"[speakers] Invalid [Speakers] AssignmentStrategy={raw!r}, using random")
    return "random"


def _load_capture_mode() -> str:
    """Read [Capture] Mode from dialogue_reader.ini. Valid values: auto,
    screen, window. Unknown or missing values fall back to 'auto'."""
    ini_path = Path(__file__).parent / "dialogue_reader.ini"
    if not ini_path.exists():
        return "auto"
    import configparser
    cp = configparser.ConfigParser()
    try:
        cp.read(ini_path, encoding="utf-8")
    except Exception:
        return "auto"
    raw = cp.get("Capture", "Mode", fallback="auto").strip().lower()
    from capture import VALID_CAPTURE_MODES
    if raw in VALID_CAPTURE_MODES:
        return raw
    print(f"[capture] Invalid [Capture] Mode={raw!r}, using auto")
    return "auto"


def _load_ocr_config() -> tuple[str, str]:
    """Read [OCR] Dialogue / Speaker from dialogue_reader.ini. Valid values
    are 'winocr' and 'easyocr'. Unknown values fall back to the built-in
    defaults (winocr for dialogue, easyocr for speaker)."""
    dialogue_engine = "winocr"
    speaker_engine = "easyocr"
    ini_path = Path(__file__).parent / "dialogue_reader.ini"
    if not ini_path.exists():
        return dialogue_engine, speaker_engine
    import configparser
    cp = configparser.ConfigParser()
    try:
        cp.read(ini_path, encoding="utf-8")
    except Exception:
        return dialogue_engine, speaker_engine
    if not cp.has_section("OCR"):
        return dialogue_engine, speaker_engine
    from ocr import VALID_ENGINES
    d = cp.get("OCR", "Dialogue", fallback="").strip().lower()
    s = cp.get("OCR", "Speaker", fallback="").strip().lower()
    if d in VALID_ENGINES:
        dialogue_engine = d
    elif d:
        print(f"[ocr] Invalid [OCR] Dialogue={d!r}, using {dialogue_engine}")
    if s in VALID_ENGINES:
        speaker_engine = s
    elif s:
        print(f"[ocr] Invalid [OCR] Speaker={s!r}, using {speaker_engine}")
    return dialogue_engine, speaker_engine


def _load_skip_when_zoomed() -> bool:
    """Read [Magnifier] SkipWhenZoomed from dialogue_reader.ini.
    Returns True (default) when enabled or the value is missing/invalid
    — the feature is conservative and opt-out, not opt-in."""
    ini_path = Path(__file__).parent / "dialogue_reader.ini"
    if not ini_path.exists():
        return True
    import configparser
    cp = configparser.ConfigParser()
    try:
        cp.read(ini_path, encoding="utf-8")
    except Exception:
        return True
    raw = cp.get("Magnifier", "SkipWhenZoomed", fallback="true").strip().lower()
    if raw in ("false", "0", "no", "off"):
        return False
    return True


def _load_text_confirm_polls() -> int:
    """Read [Polling] TextConfirmPolls from dialogue_reader.ini.
    Returns TEXT_CONFIRM_POLLS if missing, unparseable, or < 1."""
    ini_path = Path(__file__).parent / "dialogue_reader.ini"
    if not ini_path.exists():
        return TEXT_CONFIRM_POLLS
    import configparser
    cp = configparser.ConfigParser()
    try:
        cp.read(ini_path, encoding="utf-8")
    except Exception:
        return TEXT_CONFIRM_POLLS
    raw = cp.get("Polling", "TextConfirmPolls", fallback="").strip()
    if not raw:
        return TEXT_CONFIRM_POLLS
    try:
        n = int(raw)
    except ValueError:
        return TEXT_CONFIRM_POLLS
    return max(1, n)


# ---- singleton enforcement ------------------------------------------------
#
# py.exe (the Python launcher) spawns python.exe as a child process. Killing
# py.exe via AHK's ProcessClose() does NOT cascade to python.exe, so it can
# survive as an orphan holding the UDP port and continuing to OCR/speak.
#
# Each instance records its PID in a lock file next to this script. On startup
# we read that file and, if it names a different live process, terminate it
# (newest launch wins) before binding our own UDP socket and opening audio.
# The parent watchdog below is the other safety net: it kills this process
# when the launching AHK dies.

_LOCK_PATH = Path(__file__).parent / "dialogue_reader.lock"


def _terminate_pid(pid: int) -> None:
    PROCESS_TERMINATE = 0x0001
    handle = ctypes.windll.kernel32.OpenProcess(PROCESS_TERMINATE, False, pid)
    if not handle:
        return
    try:
        ctypes.windll.kernel32.TerminateProcess(handle, 0)
    finally:
        ctypes.windll.kernel32.CloseHandle(handle)


def _claim_singleton(lock_path: Path = _LOCK_PATH) -> None:
    """Ensure this is the only running instance. If lock_path records a prior
    instance's PID that is still alive (and not our own), terminate it, wait
    briefly for Windows to release its UDP socket and audio streams, then
    record our own PID. A missing, empty, or unparseable lock file just means
    'no known prior instance' and we proceed to write ours."""
    my_pid = os.getpid()
    try:
        prior_pid = int(lock_path.read_text(encoding="utf-8").strip())
    except (OSError, ValueError):
        prior_pid = None
    if prior_pid is not None and prior_pid != my_pid and _is_process_alive(prior_pid):
        print(f"[singleton] killing prior instance pid {prior_pid}")
        try:
            _terminate_pid(prior_pid)
        except Exception as e:
            print(f"[singleton] failed to kill {prior_pid}: {e}")
        # Let Windows release the UDP socket and sounddevice streams.
        time.sleep(0.7)
    try:
        lock_path.write_text(str(my_pid), encoding="utf-8")
    except OSError as e:
        print(f"[singleton] could not write lock file: {e}")


# ---- parent watchdog ------------------------------------------------------
#
# AHK launches us with --parent-pid <ahk-pid>. We start a tiny background
# thread that polls every second and immediately terminates this process
# when the parent AHK is gone. This is the only kill path that works
# regardless of *how* AHK dies — graceful exit, ProcessClose from a
# launcher, taskkill /F, crash, system shutdown.

def _is_process_alive(pid: int) -> bool:
    PROCESS_QUERY_LIMITED_INFORMATION = 0x1000
    STILL_ACTIVE = 259
    handle = ctypes.windll.kernel32.OpenProcess(
        PROCESS_QUERY_LIMITED_INFORMATION, False, pid
    )
    if not handle:
        return False
    try:
        code = ctypes.c_ulong(0)
        ctypes.windll.kernel32.GetExitCodeProcess(handle, ctypes.byref(code))
        return code.value == STILL_ACTIVE
    finally:
        ctypes.windll.kernel32.CloseHandle(handle)


def _watch_parent_thread(parent_pid: int) -> None:
    while True:
        time.sleep(1.0)
        if not _is_process_alive(parent_pid):
            print(f"[watchdog] parent {parent_pid} died, exiting", flush=True)
            # os._exit bypasses Python's cleanup handlers — fine here
            # because we want instant death and the OS will reclaim
            # everything anyway.
            os._exit(0)


def _start_parent_watchdog() -> None:
    """Parse --parent-pid from argv and start a thread that exits us when
    that PID dies. No-op if the flag isn't present."""
    for i, arg in enumerate(sys.argv):
        if arg == "--parent-pid" and i + 1 < len(sys.argv):
            try:
                parent_pid = int(sys.argv[i + 1])
            except ValueError:
                return
            if parent_pid > 0 and _is_process_alive(parent_pid):
                threading.Thread(
                    target=_watch_parent_thread,
                    args=(parent_pid,),
                    daemon=True,
                ).start()
                print(f"[watchdog] watching parent pid {parent_pid}", flush=True)
            return


# ---- audio cues ----------------------------------------------------------
_BEEP_RATE = 22050


def _make_beep(frequencies: list[float], tone_ms: int = 80) -> np.ndarray:
    """Build a short two-tone sine-wave cue with click-free fade in/out."""
    samples = int(_BEEP_RATE * tone_ms / 1000)
    t = np.arange(samples) / _BEEP_RATE
    fade_n = max(1, int(samples * 0.15))
    fade_in = np.linspace(0.0, 1.0, fade_n, dtype=np.float32)
    fade_out = np.linspace(1.0, 0.0, fade_n, dtype=np.float32)
    pieces = []
    for freq in frequencies:
        wave = (np.sin(2 * np.pi * freq * t) * 0.25).astype(np.float32)
        wave[:fade_n] *= fade_in
        wave[-fade_n:] *= fade_out
        pieces.append(wave)
    return np.concatenate(pieces)


# Pause descends (stop) — unpause ascends (go).
_PAUSE_CUE = _make_beep([700.0, 350.0])
_UNPAUSE_CUE = _make_beep([350.0, 700.0])


def _play_cue(audio: np.ndarray) -> None:
    """Play a short audio cue, blocking until done. Safe alongside TTS:
    sd.play() preempts any current playback, and the cue is brief enough
    that holding the main loop for it is fine."""
    try:
        sd.play(audio, samplerate=_BEEP_RATE, blocking=True)
    except Exception:
        pass


@dataclass
class WatchedRegion:
    name: str
    capture: RegionCapture
    # "dialogue" — OCR'd text is spoken aloud (default).
    # "speaker"  — OCR'd text becomes the current_speaker name and is NOT
    #              spoken; it's only used to look up which voice to use
    #              for subsequent dialogue.
    mode: str = "dialogue"
    # Most recent OCR result. Used by build_speech() to compose the utterance.
    last_text: str = ""
    # The OCR text from the last time this region contributed to a speak()
    # call. Used as the dedup anchor — new OCRs are compared against this,
    # not against last_text, so OCR jitter between speaks doesn't drift the
    # baseline.
    last_spoken_text: str = ""
    has_pending_frame: bool = False


def _normalize(s: str) -> str:
    """Lowercase + strip punctuation + collapse whitespace."""
    return " ".join(_PUNCT_RE.sub("", s.lower()).split())


def _similar(a: str, b: str) -> float:
    """Character-level similarity (0.0 - 1.0) using difflib's ratio."""
    a_norm = _normalize(a)
    b_norm = _normalize(b)
    if not a_norm or not b_norm:
        return 0.0
    return SequenceMatcher(None, a_norm, b_norm).ratio()


def _is_cosmetic_change(new_text: str, baseline: str) -> bool:
    """True if `new_text` is essentially the same as `baseline` — Magnifier
    zoom, DPI re-render, anti-alias drift, OCR jitter on the same content.

    Rules, in order:
    1. Identical after normalize -> cosmetic.
    2. Very high similarity (>= 0.92) -> cosmetic. This catches OCR jitter
       that inserts/removes/moves small fragments (stray glyphs, "c)",
       partial words) anywhere in the text.
    3. One is a prefix of the other with a substantial difference -> NOT
       cosmetic. This is typing or progressive dialogue reveal.
    4. Moderate similarity (>= PER_REGION_DEDUP) with small length delta
       -> cosmetic.
    """
    if not baseline or not new_text:
        return False
    a, b = _normalize(new_text), _normalize(baseline)
    if not a or not b:
        return False

    if a == b:
        return True

    sim = SequenceMatcher(None, a, b).ratio()

    # Very high similarity = OCR jitter regardless of where the diff is.
    if sim >= 0.92:
        return True

    # Prefix extension/contraction with substantial difference = real
    # change (typing or progressive dialogue reveal).
    if a.startswith(b) or b.startswith(a):
        return False

    # Moderate similarity with small length delta = cosmetic jitter.
    if abs(len(a) - len(b)) <= 3 and sim >= PER_REGION_DEDUP:
        return True

    return False


def _safe_print(prefix: str, value: str) -> None:
    """Print without crashing on a non-ascii window title in cp1252 console."""
    try:
        print(f"{prefix}{value}")
    except UnicodeEncodeError:
        print(f"{prefix}{value.encode('ascii', 'replace').decode()}")


def add_region(
    regions: list[WatchedRegion],
    debug: bool,
    mode: str = "dialogue",
    capture_mode: str = "auto",
) -> None:
    """Open the region picker and append the result as a new watched region.

    mode="dialogue" → text gets read aloud (the normal case).
    mode="speaker"  → text becomes the current speaker name; not spoken,
                      used to look up which voice to use for dialogue.
    """
    label = "speaker name" if mode == "speaker" else "dialogue"
    print(f"[dialogue-reader] Pick the {label} region to watch...")
    region, hwnd, rotation = pick_region()
    if not region:
        print("[dialogue-reader] Region pick cancelled.")
        return

    x, y, w, h = region
    rot_label = f"  rotation={rotation:+.0f}\u00B0" if abs(rotation) > 0.001 else ""
    print(f"[dialogue-reader] Region picked: x={x} y={y} w={w} h={h}{rot_label}")

    # The picker now captures the underlying HWND at release time, while it
    # still knows exactly where the user clicked AND has hidden the overlay.
    # If for some reason that failed (returned 0), fall back to a fresh
    # WindowFromPoint call here.
    if not hwnd:
        hwnd = find_window_at(x + w // 2, y + h // 2)

    if hwnd:
        _safe_print(
            "[dialogue-reader] Window under region: ",
            f"{get_window_title(hwnd)} (hwnd={hwnd})",
        )
    else:
        print("[dialogue-reader] No window detected — falling back to screen capture.")

    cap = RegionCapture(
        region,
        hwnd=hwnd,
        poll_hz=POLL_HZ,
        stable_ms=STABLE_MS,
        verbose=False,
        rotation=rotation,
        capture_mode=capture_mode,
    )
    if cap.capture_mode == "screen":
        print("[dialogue-reader] Using SCREEN capture mode (forced — PrintWindow disabled).")
    elif cap.capture_mode == "window":
        print("[dialogue-reader] Using WINDOW capture mode (forced PrintWindow).")
    elif cap.use_window_mode:
        print("[dialogue-reader] Using WINDOW capture mode (immune to Magnifier/zoom).")
    else:
        print("[dialogue-reader] Using SCREEN capture mode.")

    n_existing = len([r for r in regions if r.mode == mode])
    name = f"{mode}{n_existing + 1}"
    regions.append(WatchedRegion(name=name, capture=cap, mode=mode))
    print(f"[dialogue-reader] Added {name} (mode={mode}). Total regions: {len(regions)}")


def build_speech(regions: list[WatchedRegion]) -> str:
    """Concatenate dialogue-region text into one utterance. Speaker-mode
    regions are excluded — their text feeds the SpeakerManager, not the
    spoken output."""
    parts = [
        r.last_text.strip()
        for r in regions
        if r.mode == "dialogue" and r.last_text.strip()
    ]
    if not parts:
        return ""
    if len(parts) == 1:
        return parts[0]
    return ". ".join(parts)


def _restart_last(tts: TTS, speaker_mgr: SpeakerManager, state: dict) -> None:
    """Stop any current speech and re-speak the last spoken utterance with
    the current TTS settings. Doubles as an audio cue when adjusting speed."""
    last = state.get("last_spoken", "")
    if not last:
        return
    voice = speaker_mgr.voice_for_current()
    tts.speak(last, voice=voice)


def handle_command(
    cmd: str,
    regions: list[WatchedRegion],
    tts: TTS,
    speaker_mgr: SpeakerManager,
    state: dict,
    debug: bool,
) -> None:
    if cmd == "PICK_REGION":
        add_region(regions, debug=debug, mode="dialogue", capture_mode=state["capture_mode"])
    elif cmd == "PICK_SPEAKER":
        add_region(regions, debug=debug, mode="speaker", capture_mode=state["capture_mode"])
    elif cmd == "CLEAR_REGIONS":
        regions.clear()
        # Also reset speak-history so freshly-picked regions that happen
        # to show the same text as the last line we spoke aren't silently
        # skipped via the "speech identical to last spoken" dedup.
        state["last_spoken"] = ""
        state["candidate"] = ""
        state["speaker_candidate"] = ""
        # Invalidate any OCR batch still running on the worker — its
        # results reference captures that no longer belong to any region.
        state["generation"] += 1
        print("[dialogue-reader] Cleared all regions.")
    elif cmd == "CYCLE_VOICE":
        result = speaker_mgr.cycle_current_voice(direction=1)
        if result is None:
            print("[dialogue-reader] CYCLE_VOICE: no current speaker yet")
        else:
            speaker, new_voice = result
            print(f"[dialogue-reader] {speaker} -> {new_voice}")
            tts.speak(f"Voice changed for {speaker}", voice=new_voice)
    elif cmd == "CYCLE_VOICE_PREV":
        result = speaker_mgr.cycle_current_voice(direction=-1)
        if result is None:
            print("[dialogue-reader] CYCLE_VOICE_PREV: no current speaker yet")
        else:
            speaker, new_voice = result
            print(f"[dialogue-reader] {speaker} -> {new_voice} (back)")
            tts.speak(f"Voice changed for {speaker}", voice=new_voice)
    elif cmd.startswith("SET_SPEAKER:"):
        name = cmd[len("SET_SPEAKER:"):].strip()
        if name:
            voice = speaker_mgr.set_current(name)
            if voice:
                _safe_print(
                    "[speakers] current = ",
                    f"{speaker_mgr.current_speaker!r} voice={voice} (manual SET_SPEAKER)",
                )
    elif cmd == "PAUSE":
        if not state["paused"]:
            state["paused"] = True
            # Drop any OCR batch in flight — speaking its result after the
            # user pauses would be surprising.
            state["generation"] += 1
            tts.stop()
            _play_cue(_PAUSE_CUE)
            print("[dialogue-reader] PAUSED (watching + speech stopped)")
    elif cmd == "UNPAUSE":
        if state["paused"]:
            state["paused"] = False
            _play_cue(_UNPAUSE_CUE)
            print("[dialogue-reader] UNPAUSED")
    elif cmd == "TOGGLE_PAUSE":
        if state["paused"]:
            state["paused"] = False
            _play_cue(_UNPAUSE_CUE)
            print("[dialogue-reader] UNPAUSED")
        else:
            state["paused"] = True
            state["generation"] += 1
            tts.stop()
            _play_cue(_PAUSE_CUE)
            print("[dialogue-reader] PAUSED")
    elif cmd == "SPEED_UP":
        tts.set_speed(round(tts.get_speed() + 0.1, 2))
        print(f"[dialogue-reader] TTS speed -> {tts.get_speed():.2f}x")
        _restart_last(tts, speaker_mgr, state)
    elif cmd == "SPEED_DOWN":
        tts.set_speed(round(tts.get_speed() - 0.1, 2))
        print(f"[dialogue-reader] TTS speed -> {tts.get_speed():.2f}x")
        _restart_last(tts, speaker_mgr, state)
    else:
        print(f"[dialogue-reader] Unknown command: {cmd!r}")


def _apply_ocr_result(
    result: OCRBatchResult,
    regions: list[WatchedRegion],
    state: dict,
    speaker_mgr: SpeakerManager,
    tts: TTS,
    debug: bool,
) -> None:
    """Consume a completed OCR batch: update per-region state, resolve the
    speaker-candidate handshake, and (if dialogue changed) run the full
    speech dedup + candidate machinery that used to live inline in the
    main loop. Caller is responsible for checking `result.generation`
    matches `state["generation"]` before calling."""
    if result.error:
        if debug:
            print(f"[ocr] batch failed: {result.error}", flush=True)
        for r in regions:
            r.has_pending_frame = False
        return

    any_dialogue_changed = False
    speaker_candidate = state["speaker_candidate"]

    for r in regions:
        if r.name not in result.texts:
            continue
        new_text = result.texts[r.name]
        cosmetic = _is_cosmetic_change(new_text, r.last_spoken_text)
        if debug:
            print(
                f"[ocr {r.name} mode={r.mode}] {new_text!r} "
                f"(cosmetic vs last-spoken={cosmetic})"
            )

        if r.mode == "speaker":
            text_clean = new_text.strip()
            if not text_clean:
                # Pixels changed but OCR found no readable name — clear
                # current speaker so the next dialogue line uses the default
                # voice rather than inheriting the previous speaker's.
                if speaker_mgr.current_speaker:
                    _safe_print(
                        "[speakers] cleared (was ",
                        f"{speaker_mgr.current_speaker!r}) — pixels changed but OCR returned nothing",
                    )
                    speaker_mgr.current_speaker = ""
                speaker_candidate = ""
                r.last_spoken_text = ""
            elif not cosmetic:
                if text_clean != speaker_candidate:
                    speaker_candidate = text_clean
                    if debug:
                        _safe_print(
                            "[speakers] candidate = ",
                            f"{text_clean!r} (waiting for confirmation)",
                        )
                else:
                    speaker_candidate = ""
                    voice = speaker_mgr.set_current(text_clean)
                    if voice:
                        _safe_print(
                            "[speakers] current = ",
                            f"{speaker_mgr.current_speaker!r} voice={voice}",
                        )
                    r.last_spoken_text = text_clean
            else:
                # Cosmetic (speaker name unchanged) — clear any stale
                # candidate so dialogue isn't stuck in [hold] forever.
                if speaker_candidate:
                    speaker_candidate = ""
        else:
            if not cosmetic:
                any_dialogue_changed = True

        r.last_text = new_text
        r.has_pending_frame = False

    state["speaker_candidate"] = speaker_candidate

    if not any_dialogue_changed:
        if debug:
            print("[skip] no dialogue change")
        return

    speech = build_speech(regions)
    if len(speech) < MIN_TEXT_LEN:
        state["candidate"] = ""
    elif speech == state["last_spoken"]:
        state["candidate"] = ""
        if debug:
            print("[skip] speech identical to last spoken")
    elif (
        state["last_spoken"]
        and _similar(speech, state["last_spoken"]) >= 0.92
    ):
        state["candidate"] = ""
        if debug:
            print(
                f"[skip] speech too similar to last spoken "
                f"({_similar(speech, state['last_spoken']):.2f})"
            )
    elif not state["candidate"] or _similar(speech, state["candidate"]) < 0.92:
        # First time seeing this text — stash as candidate, don't speak
        # yet. One more matching poll is needed to confirm it's not jitter.
        state["candidate"] = speech
        if debug:
            print("[candidate] new text, waiting for confirmation poll")
    elif speaker_candidate and speaker_candidate != speaker_mgr.current_speaker:
        # Dialogue confirmed but speaker still pending — hold one more poll
        # rather than attributing the new line to the previous speaker.
        if debug:
            print(
                f"[hold] dialogue ready but waiting for speaker "
                f"candidate {speaker_candidate!r} to confirm"
            )
    else:
        state["candidate"] = ""
        voice = speaker_mgr.voice_for_current()
        speaker_label = (
            f" ({speaker_mgr.current_speaker})"
            if speaker_mgr.current_speaker else ""
        )
        print(f"[speak{speaker_label}] {speech}")
        tts.speak(speech, voice=voice)
        state["last_spoken"] = speech
        for rr in regions:
            if rr.mode == "dialogue":
                rr.last_spoken_text = rr.last_text


def _build_batch_job(
    regions: list[WatchedRegion],
    generation: int,
    confirm_polls: int,
    debug: bool,
) -> OCRBatchJob | None:
    """Build an OCRBatchJob for the worker if any region has a pending
    change. All speaker regions are always included so a new name bubble
    that appeared slightly after the dialogue update gets OCR'd before we
    attribute the line to the previous speaker."""
    specs: list[OCRRegionSpec] = []
    for r in regions:
        if r.has_pending_frame or r.mode == "speaker":
            specs.append(OCRRegionSpec(
                name=r.name, mode=r.mode, capture=r.capture,
            ))
    # Require at least one actually-changed region to bother submitting.
    if not any(r.has_pending_frame for r in regions):
        return None
    return OCRBatchJob(
        generation=generation,
        regions=specs,
        confirm_polls=confirm_polls,
        debug=debug,
        pre_snapshot_delay=0.15,
        confirm_interval=TEXT_CONFIRM_INTERVAL,
        confirm_max_multiplier=TEXT_CONFIRM_MAX_MULTIPLIER,
        confirm_hard_cap=TEXT_CONFIRM_HARD_CAP,
    )


def main() -> int:
    debug = "--debug" in sys.argv
    pick_on_start = "--pick-on-start" in sys.argv

    # Claim the singleton BEFORE touching heavy stuff (OCR/TTS models, UDP
    # socket, audio device). This terminates any prior instance recorded in
    # the lock file so it is not still holding the port or the sound output.
    _claim_singleton()
    # Watch the parent AHK so we exit if/when it dies, no matter how.
    _start_parent_watchdog()

    # Read voice config from the ini file (if present). The launcher's INI
    # is the canonical source so the AHK side and Python side stay in sync.
    voice_pool, default_voice = _load_voice_config()
    text_confirm_polls = _load_text_confirm_polls()
    if debug:
        print(f"[dialogue-reader] TextConfirmPolls = {text_confirm_polls}")

    capture_mode = _load_capture_mode()
    print(f"[dialogue-reader] Capture mode: {capture_mode}")

    skip_when_zoomed = _load_skip_when_zoomed()
    print(f"[dialogue-reader] Magnifier SkipWhenZoomed: {skip_when_zoomed}")

    dialogue_engine, speaker_engine = _load_ocr_config()
    print(
        f"[dialogue-reader] Loading OCR engines "
        f"(dialogue={dialogue_engine}, speaker={speaker_engine})..."
    )
    ocr = OCR(
        debug=debug,
        dialogue_engine=dialogue_engine,
        speaker_engine=speaker_engine,
    )
    print("[dialogue-reader] Loading TTS engine...")
    tts = TTS(voice=default_voice, speed=1.1)

    speakers_path = Path(__file__).parent / "speakers.json"
    assignment_strategy = _load_speaker_assignment_strategy()
    speaker_mgr = SpeakerManager(
        voice_pool=voice_pool,
        save_path=speakers_path,
        assignment_strategy=assignment_strategy,
    )
    print(
        f"[speakers] {len(speaker_mgr.assignments)} speaker(s) loaded from "
        f"{speakers_path.name} (assignment={assignment_strategy})"
    )

    print(f"[dialogue-reader] Starting UDP command server on 127.0.0.1:{DEFAULT_PORT}")
    server = CommandServer(port=DEFAULT_PORT)
    try:
        server.start()
    except OSError as e:
        # Port still held by a just-terminated prior instance; give Windows a
        # moment to release it, then retry once before giving up.
        print(f"[singleton] UDP bind failed ({e}); retrying after a short wait")
        time.sleep(0.5)
        try:
            server = CommandServer(port=DEFAULT_PORT)
            server.start()
        except OSError as e2:
            print(f"[fatal] could not bind UDP port {DEFAULT_PORT}: {e2}")
            print("[fatal] another instance is still holding the port. Aborting.")
            return 1

    regions: list[WatchedRegion] = []
    # generation bumps invalidate any OCR batch currently running on the
    # worker (pause/clear_regions). speaker_candidate lives here so the
    # result-applier helper can see/update it across ticks.
    state = {
        "paused": False,
        "last_spoken": "",
        "candidate": "",
        "generation": 0,
        "speaker_candidate": "",
        "capture_mode": capture_mode,
        "zoomed": False,
    }

    ocr_worker = OCRWorker(ocr)
    print("[dialogue-reader] OCR worker thread started.")

    print("[dialogue-reader] Ready. Use the AHK script (or send UDP commands) to control.")
    print("[dialogue-reader] PICK_REGION to add a region. Ctrl+C to quit.")
    print()

    if pick_on_start:
        add_region(regions, debug=debug, capture_mode=capture_mode)

    poll_interval = 1.0 / POLL_HZ

    try:
        while True:
            # 1. Drain commands. This runs every tick regardless of OCR
            #    state, so hotkeys (PICK_REGION, CYCLE_VOICE, PAUSE…) are
            #    handled within one poll_interval of being pressed.
            while not server.queue.empty():
                try:
                    cmd = server.queue.get_nowait()
                except queue.Empty:
                    break
                handle_command(cmd, regions, tts, speaker_mgr, state, debug=debug)

            # 2. Apply any OCR result the worker just finished. poll_result
            #    is non-blocking. Stale generations (from pause/clear that
            #    happened while OCR was in flight) are discarded.
            result = ocr_worker.poll_result()
            if result is not None:
                if result.generation == state["generation"]:
                    _apply_ocr_result(
                        result, regions, state, speaker_mgr, tts, debug=debug
                    )
                elif debug:
                    print(
                        f"[ocr] discarding stale result "
                        f"(gen {result.generation} != {state['generation']})"
                    )

            if state["paused"] or not regions:
                time.sleep(poll_interval)
                continue

            # 2b. Skip polling while Windows Magnifier is zoomed in. Pixel
            #     positions shift when zoomed, so speaker regions tend to
            #     pick up stray text and dialogue regions misfire. We
            #     still drain commands and apply in-flight OCR results
            #     (done above); only NEW region polling/submission pauses.
            #     Debug mode logs the raw level even when detection
            #     thinks nothing changed — useful for diagnosing setups
            #     where the Magnification API reports stale values.
            if skip_when_zoomed:
                zoomed_now = _magnifier_is_zoomed()
                if zoomed_now != state["zoomed"]:
                    level = _magnifier_level()
                    state["zoomed"] = zoomed_now
                    if zoomed_now:
                        print(
                            f"[dialogue-reader] Magnifier zoomed "
                            f"(level={level:.2f}) — polling paused"
                        )
                    else:
                        print(
                            f"[dialogue-reader] Magnifier back at 100% "
                            f"(level={level:.2f}) — polling resumed"
                        )
                if zoomed_now:
                    time.sleep(poll_interval)
                    continue

            # 3. Poll regions for pixel changes. Only submit a new batch
            #    if the worker is idle — no point queueing work that'll
            #    run against pixels from 2s ago by the time it gets out.
            if not ocr_worker.busy:
                any_changed = False
                for r in regions:
                    frame = r.capture.poll_once()
                    if frame is not None:
                        r.has_pending_frame = True
                        any_changed = True

                if any_changed:
                    job = _build_batch_job(
                        regions,
                        generation=state["generation"],
                        confirm_polls=text_confirm_polls,
                        debug=debug,
                    )
                    if job is not None:
                        ocr_worker.submit(job)

            time.sleep(poll_interval)
    except KeyboardInterrupt:
        print("\n[dialogue-reader] Bye.")
    finally:
        try:
            tts.shutdown()
        except Exception:
            pass
        server.stop()
    return 0


if __name__ == "__main__":
    sys.exit(main())
