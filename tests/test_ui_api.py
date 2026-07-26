"""Tests for the settings-app backend: comment-preserving ini writes, typed
settings reads, UDP command client, and reader status probe."""
import socket

import pytest

from ui.api import (
    Api,
    english_voices,
    read_settings,
    reader_running,
    send_command,
    update_ini_text,
)


INI = """; top comment stays
[Hotkeys]
PickRegion=F1

[Media]
; keep this comment
PauseDuringSpeech = true
ResumeDelayMs=1000

[Voices]
Default=kokoro:af_heart
Pool=kokoro:all
"""


# ---- update_ini_text -------------------------------------------------------

def test_replaces_value_preserving_comments_and_style():
    out = update_ini_text(INI, {("Media", "PauseDuringSpeech"): "false"})
    assert "; keep this comment" in out
    assert "; top comment stays" in out
    assert "PauseDuringSpeech = false" in out        # spacing style kept
    assert "ResumeDelayMs=1000" in out               # untouched line verbatim


def test_replaces_multiple_values():
    out = update_ini_text(INI, {
        ("Media", "ResumeDelayMs"): "2500",
        ("Voices", "Default"): "kokoro:bf_emma",
    })
    assert "ResumeDelayMs=2500" in out
    assert "Default=kokoro:bf_emma" in out
    assert "PauseDuringSpeech = true" in out


def test_adds_missing_key_to_existing_section():
    out = update_ini_text(INI, {("Media", "NewKey"): "x"})
    media = out.split("[Media]")[1].split("[")[0]
    assert "NewKey=x" in media


def test_creates_missing_section_at_end():
    out = update_ini_text(INI, {("Launcher", "HideConsole"): "false"})
    assert "[Launcher]" in out
    assert "HideConsole=false" in out


# ---- read_settings ---------------------------------------------------------

def test_read_settings_types(tmp_path):
    p = tmp_path / "d.ini"
    p.write_text(INI, encoding="utf-8")
    s = read_settings(p)
    assert s["Media"]["PauseDuringSpeech"] is True
    assert s["Media"]["ResumeDelayMs"] == 1000
    assert s["Voices"]["Default"] == "kokoro:af_heart"
    assert s["Hotkeys"]["PickRegion"] == "F1"


def test_read_settings_defaults_for_missing(tmp_path):
    s = read_settings(tmp_path / "nope.ini")
    assert s["Media"]["PauseDuringSpeech"] is True
    assert s["Capture"]["Mode"] == "auto"
    assert s["OCR"]["Dialogue"] == "winocr"


def test_roundtrip(tmp_path):
    p = tmp_path / "d.ini"
    p.write_text(INI, encoding="utf-8")
    out = update_ini_text(p.read_text(encoding="utf-8"),
                          {("Media", "ResumeDelayMs"): "1500"})
    p.write_text(out, encoding="utf-8")
    assert read_settings(p)["Media"]["ResumeDelayMs"] == 1500


# ---- UDP client and status probe -------------------------------------------

def test_send_command_reaches_udp_listener():
    rx = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    rx.bind(("127.0.0.1", 0))
    rx.settimeout(2.0)
    port = rx.getsockname()[1]
    send_command("RELOAD_CONFIG", port=port)
    data, _ = rx.recvfrom(1024)
    rx.close()
    assert data.decode() == "RELOAD_CONFIG"


def test_reader_running_probe():
    probe = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    probe.bind(("127.0.0.1", 0))
    port = probe.getsockname()[1]
    assert reader_running(port=port) is True       # port held = reader up
    probe.close()
    assert reader_running(port=port) is False


# ---- voices ----------------------------------------------------------------

def test_english_voices_fallback_is_nonempty(tmp_path):
    voices = english_voices(voices_bin=tmp_path / "missing.bin")
    assert voices
    assert all(v.startswith("kokoro:") for v in voices)
    assert "kokoro:af_heart" in voices


# ---- Api bridge ------------------------------------------------------------

def test_api_save_writes_ini_and_reloads(tmp_path, monkeypatch):
    p = tmp_path / "d.ini"
    p.write_text(INI, encoding="utf-8")
    sent = []
    monkeypatch.setattr("ui.api.send_command", lambda cmd, **kw: sent.append(cmd))
    api = Api(ini_path=p)
    api.save_settings({"Media": {"ResumeDelayMs": 2000}})
    assert read_settings(p)["Media"]["ResumeDelayMs"] == 2000
    assert sent == ["RELOAD_CONFIG"]


def test_api_preview_and_live(monkeypatch, tmp_path):
    sent = []
    monkeypatch.setattr("ui.api.send_command", lambda cmd, **kw: sent.append(cmd))
    api = Api(ini_path=tmp_path / "d.ini")
    api.preview_voice("kokoro:bf_emma")
    api.live_command("TOGGLE_PAUSE")
    assert sent == ["PREVIEW_VOICE:kokoro:bf_emma", "TOGGLE_PAUSE"]


def test_api_get_state_shape(tmp_path, monkeypatch):
    p = tmp_path / "d.ini"
    p.write_text(INI, encoding="utf-8")
    monkeypatch.setattr("ui.api.reader_running", lambda **kw: True)
    api = Api(ini_path=p)
    state = api.get_state()
    assert state["running"] is True
    assert state["settings"]["Media"]["ResumeDelayMs"] == 1000
    assert state["voices"]
