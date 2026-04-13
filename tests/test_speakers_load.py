import json
import tempfile
from pathlib import Path

from speakers import SpeakerManager


def _make_mgr(data: dict) -> SpeakerManager:
    """Build a SpeakerManager from a fabricated speakers.json dict."""
    td = tempfile.mkdtemp()
    sp = Path(td) / "speakers.json"
    sp.write_text(json.dumps(data), encoding="utf-8")
    return SpeakerManager(
        voice_pool=["voice:a", "voice:b"],
        save_path=sp,
    )


def test_load_skips_entry_with_null_cycle_index():
    mgr = _make_mgr({
        "assignments": {"Alice": "voice:a", "Bob": "voice:b"},
        "cycle_index": {"Alice": 0, "Bob": None},
        "next_auto_index": 2,
    })
    # Both assignments survive because they are independent of cycle_index.
    assert mgr.assignments == {"Alice": "voice:a", "Bob": "voice:b"}
    # Only the valid cycle_index entry is loaded.
    assert mgr.cycle_index == {"Alice": 0}


def test_load_survives_string_next_auto_index():
    mgr = _make_mgr({
        "assignments": {"Alice": "voice:a"},
        "cycle_index": {"Alice": 0},
        "next_auto_index": "oops",
    })
    assert mgr.assignments == {"Alice": "voice:a"}
    # Falls back to len(assignments).
    assert mgr._next_auto_index == 1


def test_load_skips_non_string_assignment_values():
    mgr = _make_mgr({
        "assignments": {"Alice": "voice:a", "Broken": 42},
        "cycle_index": {},
        "next_auto_index": 0,
    })
    # Non-string voice value is dropped, valid ones survive.
    assert mgr.assignments == {"Alice": "voice:a"}


def test_load_handles_missing_fields():
    mgr = _make_mgr({"assignments": {"Alice": "voice:a"}})
    assert mgr.assignments == {"Alice": "voice:a"}
    assert mgr.cycle_index == {}
