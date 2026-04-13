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
import subprocess
import sys
import threading
import time
import functools
import queue
from dataclasses import dataclass, field
from difflib import SequenceMatcher
from pathlib import Path

import numpy as np
import sounddevice as sd
from PIL import Image

from region_picker import pick_region
from capture import RegionCapture
from ocr import OCR
from tts import TTS, DEFAULT_VOICE
from window_capture import find_window_at, get_window_title
from command_server import CommandServer, DEFAULT_PORT
from speakers import SpeakerManager


print = functools.partial(print, flush=True)


MIN_TEXT_LEN = 2
# Per-region OCR dedup: if the new OCR text for a region is this similar to
# the region's previous OCR text AND the lengths are nearly identical, treat
# it as cosmetic jitter (Magnifier zoom, DPI re-render, anti-alias drift)
# and don't re-speak.
PER_REGION_DEDUP = 0.85
POLL_HZ = 12.0
STABLE_MS = 350

_PUNCT_RE = re.compile(r"[^\w\s]")


# ---- voice pool config ---------------------------------------------------
#
# The pool is read from dialogue_reader.ini's [Voices] section. Order
# matters: it's used round-robin when assigning voices to new speakers, so
# alternating M/F/M/F gives a natural spread for the first few characters.

_DEFAULT_VOICE_POOL = [
    "kokoro:af_heart",                   # F  US, natural (Kokoro)
    "piper:en_US-lessac-high",           # F  US, clean diction (high-quality Piper)
    "kokoro:am_michael",                 # M  US, natural (Kokoro)
    "piper:en_US-hfc_female-medium",     # F  US, higher-quality Piper
    "kokoro:bf_emma",                    # F  British (Kokoro)
    "piper:en_GB-alan-medium",           # M  British
    "kokoro:am_adam",                    # M  US, deeper (Kokoro)
    "piper:en_US-ryan-medium",           # M  US, neutral
]


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
    if default not in pool:
        pool.insert(0, default)
    return pool, default


# ---- singleton enforcement ------------------------------------------------
#
# py.exe (the Python launcher) spawns python.exe as a child process. Killing
# py.exe via AHK's ProcessClose() does NOT cascade to python.exe — it
# survives as an orphan, holding the UDP port and continuing to OCR/speak.
# Three orphans = the "reads 3 times" symptom; an orphan owning the port =
# the "starts paused (but the new instance silently crashed)" symptom.
#
# At startup we enumerate every other python.exe / pythonw.exe / py.exe
# whose command line references this main.py and TerminateProcess them
# before binding our own UDP socket.

_NO_WINDOW_FLAGS = getattr(subprocess, "CREATE_NO_WINDOW", 0)


def _find_orphan_pids() -> list[int]:
    """Other python interpreter processes whose command line points at this
    main.py. We deliberately do NOT include py.exe in the search because it
    is the *launcher* — it's the parent of our own current python.exe, and
    killing it causes the current process to die or destabilize. The actual
    interpreter (python.exe / pythonw.exe) is what holds the UDP port and
    runs the OCR/TTS loop, so that's the only thing worth killing."""
    my_pid = os.getpid()
    main_py = str(Path(__file__).resolve()).lower()

    try:
        result = subprocess.run(
            [
                "powershell.exe", "-NoProfile", "-NonInteractive", "-Command",
                "Get-CimInstance Win32_Process -Filter "
                "\"Name='python.exe' OR Name='pythonw.exe'\" | "
                "Where-Object { $_.CommandLine -ne $null } | "
                "ForEach-Object { \"$($_.ProcessId)|$($_.CommandLine)\" }"
            ],
            capture_output=True,
            text=True,
            timeout=10,
            creationflags=_NO_WINDOW_FLAGS,
        )
    except (FileNotFoundError, subprocess.TimeoutExpired):
        return []

    pids: list[int] = []
    for line in result.stdout.splitlines():
        if "|" not in line:
            continue
        pid_str, _, cmdline = line.partition("|")
        try:
            pid = int(pid_str.strip())
        except ValueError:
            continue
        if pid == my_pid:
            continue
        if main_py in cmdline.lower():
            pids.append(pid)
    return pids


def _terminate_pid(pid: int) -> None:
    PROCESS_TERMINATE = 0x0001
    handle = ctypes.windll.kernel32.OpenProcess(PROCESS_TERMINATE, False, pid)
    if not handle:
        return
    try:
        ctypes.windll.kernel32.TerminateProcess(handle, 0)
    finally:
        ctypes.windll.kernel32.CloseHandle(handle)


def _kill_orphans() -> None:
    """Kill any other instance of this script before we try to start ours."""
    pids = _find_orphan_pids()
    if not pids:
        return
    print(f"[singleton] killing orphan instance(s): {pids}")
    for pid in pids:
        try:
            _terminate_pid(pid)
        except Exception as e:
            print(f"[singleton] failed to kill {pid}: {e}")
    # Give Windows a moment to release the UDP socket and clean up sounddevice
    # streams before we try to bind / open audio.
    time.sleep(0.7)


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
    pending_frame: np.ndarray | None = None


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
    2. One is a strict prefix of the other -> NOT cosmetic. This is the
       unambiguous signature of typing or progressive dialogue reveal —
       characters were added at (or removed from) the end. We always want
       to speak the latest version in this case.
    3. Otherwise: small length delta + high similarity -> cosmetic; anything
       else is treated as real content change.
    """
    if not baseline or not new_text:
        return False
    a, b = _normalize(new_text), _normalize(baseline)
    if not a or not b:
        return False

    if a == b:
        return True

    # Prefix extension/contraction = real change (typing or backspacing).
    if a.startswith(b) or b.startswith(a):
        return False

    # Otherwise allow small jitter: <=3 chars length delta, high similarity.
    if abs(len(a) - len(b)) > 3:
        return False
    sim = SequenceMatcher(None, a, b).ratio()
    return sim >= PER_REGION_DEDUP


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
    )
    if cap.use_window_mode:
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
        add_region(regions, debug=debug, mode="dialogue")
    elif cmd == "PICK_SPEAKER":
        add_region(regions, debug=debug, mode="speaker")
    elif cmd == "CLEAR_REGIONS":
        regions.clear()
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
        if not name:
            print("[dialogue-reader] SET_SPEAKER: empty name, ignored")
        else:
            voice = speaker_mgr.set_current(name)
            _safe_print(
                "[dialogue-reader] manually set current = ",
                f"'{name}' voice={voice}",
            )
            if voice:
                tts.speak(f"Speaker set to {name}", voice=voice)
    elif cmd == "PAUSE":
        if not state["paused"]:
            state["paused"] = True
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


def main() -> int:
    debug = "--debug" in sys.argv
    pick_on_start = "--pick-on-start" in sys.argv

    # Kill any orphaned instances of this script BEFORE we touch heavy stuff
    # like the OCR/TTS models or the UDP socket. Without this, leftover
    # python.exe processes from previous runs (e.g. ones AHK couldn't
    # cascade-kill via py.exe) keep watching the screen and speaking, and
    # the new instance silently crashes when it can't bind UDP.
    _kill_orphans()
    # Watch the parent AHK so we exit if/when it dies, no matter how.
    _start_parent_watchdog()

    # Read voice config from the ini file (if present). The launcher's INI
    # is the canonical source so the AHK side and Python side stay in sync.
    voice_pool, default_voice = _load_voice_config()

    print("[dialogue-reader] Loading OCR engine...")
    ocr = OCR(debug=debug)
    print("[dialogue-reader] Loading TTS engine...")
    tts = TTS(voice=default_voice, speed=1.1)

    # Pre-download (NOT pre-load into RAM) the rest of the pool so the user
    # doesn't get a download pause mid-game when they cycle to a new voice.
    # PiperVoice loading itself stays lazy via tts._get_voice().
    for voice_name in voice_pool:
        if voice_name == default_voice:
            continue
        try:
            from tts import _ensure_voice
            _ensure_voice(tts._voices_dir, voice_name)
        except Exception as e:
            print(f"[tts] could not pre-download '{voice_name}': {e}")

    speakers_path = Path(__file__).parent / "speakers.json"
    speaker_mgr = SpeakerManager(voice_pool=voice_pool, save_path=speakers_path)
    print(f"[speakers] {len(speaker_mgr.assignments)} speaker(s) loaded from {speakers_path.name}")

    print(f"[dialogue-reader] Starting UDP command server on 127.0.0.1:{DEFAULT_PORT}")
    server = CommandServer(port=DEFAULT_PORT)
    try:
        server.start()
    except OSError as e:
        # Port still held — try one more aggressive sweep, then bail.
        print(f"[singleton] UDP bind failed ({e}); retrying after another orphan sweep")
        _kill_orphans()
        time.sleep(0.5)
        try:
            server = CommandServer(port=DEFAULT_PORT)
            server.start()
        except OSError as e2:
            print(f"[fatal] could not bind UDP port {DEFAULT_PORT}: {e2}")
            print("[fatal] another instance is still holding the port. Aborting.")
            return 1

    regions: list[WatchedRegion] = []
    state = {"paused": False, "last_spoken": ""}

    print("[dialogue-reader] Ready. Use the AHK script (or send UDP commands) to control.")
    print("[dialogue-reader] PICK_REGION to add a region. Ctrl+C to quit.")
    print()

    if pick_on_start:
        add_region(regions, debug=debug)

    poll_interval = 1.0 / POLL_HZ

    try:
        while True:
            # 1. Drain any commands from AHK.
            while not server.queue.empty():
                try:
                    cmd = server.queue.get_nowait()
                except queue.Empty:
                    break
                handle_command(cmd, regions, tts, speaker_mgr, state, debug=debug)

            if state["paused"] or not regions:
                time.sleep(poll_interval)
                continue

            # 2. Poll every region. If any region has new stable content,
            #    flag it; we'll OCR everything together.
            any_changed = False
            for r in regions:
                frame = r.capture.poll_once()
                if frame is not None:
                    r.has_pending_frame = True
                    r.pending_frame = frame
                    any_changed = True

            if any_changed:
                # 3. Confirmation pass: brief sleep, then re-grab the
                #    changed regions to catch typewriter mid-animation /
                #    lazy renders. We ALSO force-snapshot every speaker
                #    region (even ones that didn't yield) so a new
                #    speaker bubble that appeared slightly after the
                #    dialogue update gets a chance to be recognised
                #    BEFORE we attribute the line to the previous speaker.
                time.sleep(0.15)
                for r in regions:
                    if r.has_pending_frame:
                        r.pending_frame = r.capture.snapshot()
                    elif r.mode == "speaker":
                        r.pending_frame = r.capture.snapshot()
                        r.has_pending_frame = True

                # 4. OCR each changed region.
                #
                # Dialogue-mode regions: per-region dedup compares the new
                # OCR text against THIS REGION'S last spoken text so OCR
                # jitter between speaks doesn't drift the baseline.
                #
                # Speaker-mode regions: same dedup, BUT if pixels changed
                # AND OCR returned nothing, we explicitly clear the current
                # speaker. Without this clear, a new character whose name
                # UI is unreadable would inherit the previous speaker's
                # voice — exactly the "Akechi spoke but it played as Ann"
                # bug. Better to fall back to default voice than misattribute.
                any_dialogue_changed = False
                for r in regions:
                    if not r.has_pending_frame:
                        continue
                    new_text = ocr.read(r.pending_frame)
                    cosmetic = _is_cosmetic_change(new_text, r.last_spoken_text)
                    if debug:
                        print(
                            f"[ocr {r.name} mode={r.mode}] {new_text!r} "
                            f"(cosmetic vs last-spoken={cosmetic})"
                        )
                    if r.mode == "speaker":
                        text_clean = new_text.strip()
                        if not text_clean:
                            # Pixels changed but OCR found no readable name.
                            # Clear the current speaker so the next dialogue
                            # uses the default voice instead of the wrong
                            # previous speaker.
                            if speaker_mgr.current_speaker:
                                _safe_print(
                                    "[speakers] cleared (was ",
                                    f"{speaker_mgr.current_speaker!r}) — pixels changed but OCR returned nothing",
                                )
                                speaker_mgr.current_speaker = ""
                            r.last_spoken_text = ""
                        elif not cosmetic:
                            voice = speaker_mgr.set_current(text_clean)
                            if voice:
                                _safe_print(
                                    "[speakers] current = ",
                                    f"{speaker_mgr.current_speaker!r} voice={voice}",
                                )
                            # Lock the dedup baseline so we don't keep
                            # "discovering" the same name every poll.
                            r.last_spoken_text = text_clean
                    else:
                        if not cosmetic:
                            any_dialogue_changed = True
                    r.last_text = new_text
                    r.has_pending_frame = False
                    r.pending_frame = None

                if not any_dialogue_changed:
                    if debug:
                        print("[skip] no dialogue change")
                else:
                    speech = build_speech(regions)
                    if len(speech) >= MIN_TEXT_LEN and speech != state["last_spoken"]:
                        voice = speaker_mgr.voice_for_current()
                        speaker_label = (
                            f" ({speaker_mgr.current_speaker})"
                            if speaker_mgr.current_speaker else ""
                        )
                        print(f"[speak{speaker_label}] {speech}")
                        tts.speak(speech, voice=voice)
                        state["last_spoken"] = speech
                        # Snapshot each dialogue region's current OCR as
                        # the new dedup baseline.
                        for rr in regions:
                            if rr.mode == "dialogue":
                                rr.last_spoken_text = rr.last_text
                    elif debug:
                        print("[skip] speech identical to last spoken")

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
