"""Voice pool is Kokoro-only after the scale-down."""
from main import _DEFAULT_VOICE_POOL, _expand_voice


def test_pool_is_all_kokoro():
    assert _DEFAULT_VOICE_POOL, "pool must be non-empty"
    assert all(v.startswith("kokoro:") for v in _DEFAULT_VOICE_POOL)


def test_pool_is_trimmed():
    # Trimmed to a curated set, not all 28.
    assert len(_DEFAULT_VOICE_POOL) <= 14


def test_kokoro_all_expands_to_pool():
    assert _expand_voice("kokoro:all") == [
        v for v in _DEFAULT_VOICE_POOL if v.startswith("kokoro:")
    ]


def test_unknown_voice_passes_through():
    assert _expand_voice("kokoro:af_heart") == ["kokoro:af_heart"]
