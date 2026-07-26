"""Playwright end-to-end tests for the settings page, run against a local
HTTP server with a stubbed window.pywebview.api bridge."""
import functools
import http.server
import json
import threading
from pathlib import Path

import pytest

from ui.api import english_voices, read_settings

UI_DIR = Path(__file__).parent.parent / "ui"


@pytest.fixture(scope="module")
def ui_server():
    handler = functools.partial(
        http.server.SimpleHTTPRequestHandler, directory=str(UI_DIR))
    handler.log_message = lambda *a, **k: None
    srv = http.server.ThreadingHTTPServer(("127.0.0.1", 0), handler)
    threading.Thread(target=srv.serve_forever, daemon=True).start()
    yield f"http://127.0.0.1:{srv.server_address[1]}"
    srv.shutdown()


@pytest.fixture(scope="module")
def page(ui_server):
    from playwright.sync_api import sync_playwright
    state = {
        "settings": read_settings(Path("does-not-exist.ini")),  # pure defaults
        "voices": english_voices(),
        "running": True,
    }
    stub = """
    window.__calls = [];
    window.pywebview = { api: {
      get_state: () => Promise.resolve(%s),
      save_settings: (v) => { window.__calls.push(["save_settings", v]); return Promise.resolve(true); },
      preview_voice: (v) => { window.__calls.push(["preview_voice", v]); return Promise.resolve(null); },
      live_command: (c) => { window.__calls.push(["live_command", c]); return Promise.resolve(null); },
      reader_status: () => Promise.resolve(true),
      restart_reader: () => { window.__calls.push(["restart_reader"]); return Promise.resolve(true); },
    }};
    """ % json.dumps(state)
    with sync_playwright() as p:
        browser = p.chromium.launch(headless=True)
        ctx = browser.new_context()
        ctx.add_init_script(stub)
        pg = ctx.new_page()
        pg.goto(ui_server + "/index.html")
        pg.wait_for_selector("#voice-grid input.voice-cb")
        yield pg
        browser.close()


def _calls(page):
    return page.evaluate("window.__calls")


def test_page_loads_with_state(page):
    assert page.text_content("#status") == "reader running"
    n = page.locator("#voice-grid input.voice-cb").count()
    assert n >= 20                                  # English Kokoro voices
    assert page.is_checked("#use-all-voices")       # Pool default kokoro:all
    assert page.is_checked('[data-s="Media"][data-k="PauseDuringSpeech"]')


def test_change_and_save_roundtrip(page):
    page.uncheck('[data-s="Media"][data-k="PauseDuringSpeech"]')
    assert page.is_visible("#savebar")
    page.click("#btn-save")
    page.wait_for_function("window.__calls.length > 0")
    name, values = _calls(page)[-1]
    assert name == "save_settings"
    assert values["Media"]["PauseDuringSpeech"] is False
    assert values["Voices"]["Pool"] == "kokoro:all"
    assert page.is_hidden("#savebar")
    page.check('[data-s="Media"][data-k="PauseDuringSpeech"]')


def test_live_pause_button(page):
    page.click("#btn-pause")
    assert ["live_command", "TOGGLE_PAUSE"] in _calls(page)


def test_preview_default_voice(page):
    page.click("#preview-default")
    calls = _calls(page)
    assert calls[-1][0] == "preview_voice"
    assert calls[-1][1].startswith("kokoro:")


def test_hotkey_edit_shows_restart_hint(page):
    box = page.locator('#hotkeys input[data-k="PickRegion"]')
    box.fill("F9")
    assert page.is_visible("#restart-hint")
    assert page.is_visible("#savebar")


def test_pool_checkboxes_enable_when_use_all_off(page):
    page.uncheck("#use-all-voices")
    first = page.locator("#voice-grid input.voice-cb").first
    assert first.is_enabled()
    page.check("#use-all-voices")
    assert first.is_disabled()
