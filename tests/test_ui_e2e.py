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
        # Deliberately different from the ini default (af_heart): the UI must
        # show the live no-speaker voice, not the ini value.
        "no_speaker_voice": "kokoro:am_michael",
        "profiles": {
            "vein": {
                "process": "vein-win64-test.exe",
                "window": {"w": 2000, "h": 1000},
                "regions": [
                    {"rel_x": 50, "rel_y": 140, "w": 400, "h": 150,
                     "rotation": 0.0, "label": "dialogue1", "mode": "dialogue"},
                    {"rel_x": 60, "rel_y": 40, "w": 200, "h": 60,
                     "rotation": 0.0, "label": "speaker1", "mode": "speaker"},
                ],
                "apply_on_launch": False,
                "applied": True,
            },
        },
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
      set_no_speaker_voice: (v) => { window.__calls.push(["set_no_speaker_voice", v]); return Promise.resolve(null); },
      get_profiles: () => Promise.resolve(%s),
      profile_save: (n) => { window.__calls.push(["profile_save", n]); return Promise.resolve(null); },
      profile_apply: (n) => { window.__calls.push(["profile_apply", n]); return Promise.resolve(null); },
      profile_unapply: (n) => { window.__calls.push(["profile_unapply", n]); return Promise.resolve(null); },
      profile_delete: (n) => { window.__calls.push(["profile_delete", n]); return Promise.resolve(null); },
      profile_auto: (n, on) => { window.__calls.push(["profile_auto", n, on]); return Promise.resolve(null); },
    }};
    """ % (json.dumps(state), json.dumps(state["profiles"]))
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


def test_no_speaker_voice_shows_live_value_and_saves(page):
    # Shows the live voice from speakers.json, not the ini default
    assert page.input_value("#default-voice") == "kokoro:am_michael"
    page.select_option("#default-voice", "kokoro:bf_emma")
    page.click("#btn-save")
    page.wait_for_function(
        'window.__calls.some(c => c[0] === "set_no_speaker_voice")')
    assert ["set_no_speaker_voice", "kokoro:bf_emma"] in _calls(page)


def test_hotkey_edit_shows_restart_hint(page):
    box = page.locator('#hotkeys input[data-k="PickRegion"]')
    box.fill("F9")
    assert page.is_visible("#restart-hint")
    assert page.is_visible("#savebar")


def test_profile_card_renders_preview(page):
    row = page.locator("#profile-list .profile")
    assert row.count() == 1
    assert "vein-win64-test.exe" in row.text_content()
    assert page.locator("#profile-list svg rect").count() == 2   # two boxes
    assert page.is_checked("#profile-list .p-applied")           # applied=True


def test_profile_auto_toggle_sends_command(page):
    page.check("#profile-list .p-auto")
    page.wait_for_function(
        'window.__calls.some(c => c[0] === "profile_auto")')
    assert ["profile_auto", "vein", True] in _calls(page)


def test_profile_save_button(page):
    page.fill("#profile-name", "persona5")
    page.click("#btn-profile-save")
    page.wait_for_function(
        'window.__calls.some(c => c[0] === "profile_save")')
    assert ["profile_save", "persona5"] in _calls(page)
    assert page.input_value("#profile-name") == ""


def test_pool_checkboxes_enable_when_use_all_off(page):
    page.uncheck("#use-all-voices")
    first = page.locator("#voice-grid input.voice-cb").first
    assert first.is_enabled()
    page.check("#use-all-voices")
    assert first.is_disabled()
