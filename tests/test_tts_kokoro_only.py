"""Kokoro-only TTS invariants."""
from tts import _parse_voice, DEFAULT_VOICE


def test_bare_name_defaults_to_kokoro():
    # After the engine cut, a colon-less voice name is a Kokoro voice,
    # not a Piper voice.
    assert _parse_voice("af_heart") == ("kokoro", "af_heart")


def test_explicit_kokoro_voice_parses():
    assert _parse_voice("kokoro:am_michael") == ("kokoro", "am_michael")


def test_default_voice_is_kokoro():
    assert DEFAULT_VOICE.startswith("kokoro:")


def test_tts_module_has_no_piper_or_sherpa_symbols():
    import tts
    # Piper/Sherpa helpers must be gone from the module surface.
    for gone in ("PiperVoice", "_ensure_voice", "_voice_url_base", "_get_sherpa"):
        assert not hasattr(tts, gone), f"{gone} should be removed from tts.py"
