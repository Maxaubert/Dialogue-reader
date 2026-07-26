"""Tests for [Media] config parsing: PauseDuringSpeech / ResumeDelayMs."""
from main import _load_media_config


def _ini(tmp_path, content):
    p = tmp_path / "dialogue_reader.ini"
    p.write_text(content, encoding="utf-8")
    return p


def test_defaults_when_section_missing(tmp_path):
    assert _load_media_config(_ini(tmp_path, "[Voices]\n")) == (True, 1000)


def test_missing_file_uses_defaults(tmp_path):
    assert _load_media_config(tmp_path / "nope.ini") == (True, 1000)


def test_disable(tmp_path):
    enabled, _ = _load_media_config(
        _ini(tmp_path, "[Media]\nPauseDuringSpeech = false\n"))
    assert enabled is False


def test_custom_delay(tmp_path):
    assert _load_media_config(
        _ini(tmp_path, "[Media]\nResumeDelayMs = 2500\n")) == (True, 2500)


def test_invalid_values_fall_back(tmp_path):
    cfg = _load_media_config(
        _ini(tmp_path, "[Media]\nPauseDuringSpeech = banana\nResumeDelayMs = soon\n"))
    assert cfg == (True, 1000)
