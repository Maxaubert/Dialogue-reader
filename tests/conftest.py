"""Add the worktree root to sys.path so tests can import project modules."""
import sys
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parent.parent
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))


@pytest.fixture(autouse=True)
def block_commands_to_the_live_reader(monkeypatch):
    """Refuse any UDP command aimed at the real reader's port.

    The developer runs the suite while their own reader is live on
    127.0.0.1:7849. `restart_reader()` sends QUIT before killing, so a test
    that called it without patching send_command shut the running reader
    down mid-game (issue #24). Tests that genuinely exercise the socket bind
    their own port and are unaffected; anything else fails loudly here
    instead of reaching across into the user's session.
    """
    import ui.api as ui_api

    def guarded(cmd, host="127.0.0.1", port=ui_api._DEFAULT_PORT):
        raise AssertionError(
            f"test tried to send {cmd!r} to the live reader at {host}:{port}. "
            f"Patch ui.api.send_command in this test."
        )

    monkeypatch.setattr(ui_api, "send_command", guarded)
