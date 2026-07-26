"""OCR.set_engines hot-swaps the per-role engine choice, lazily initializing
any engine that was not loaded at startup."""
import pytest

from ocr import OCR


def _ocr(monkeypatch):
    inits = []
    monkeypatch.setattr(OCR, "_init_winocr", lambda self: inits.append("winocr"))
    monkeypatch.setattr(OCR, "_init_easyocr", lambda self: inits.append("easyocr"))
    o = OCR(dialogue_engine="winocr", speaker_engine="winocr")
    inits.clear()
    return o, inits


def test_set_engines_updates_roles(monkeypatch):
    o, _ = _ocr(monkeypatch)
    o.set_engines("winocr", "easyocr")
    assert o._dialogue_engine == "winocr"
    assert o._speaker_engine == "easyocr"


def test_set_engines_inits_newly_needed_engine(monkeypatch):
    o, inits = _ocr(monkeypatch)
    o.set_engines("easyocr", "easyocr")
    assert inits == ["easyocr"]


def test_set_engines_skips_already_loaded(monkeypatch):
    o, inits = _ocr(monkeypatch)
    o.set_engines("winocr", "winocr")
    assert inits == []


def test_set_engines_rejects_unknown(monkeypatch):
    o, _ = _ocr(monkeypatch)
    with pytest.raises(ValueError):
        o.set_engines("tesseract", "winocr")
    # roles untouched after the failed swap
    assert o._dialogue_engine == "winocr"
    assert o._speaker_engine == "winocr"
