"""Settings app entry: a pywebview window over ui/index.html with the Api
bridge. Closing the window hides it to the tray; the tray menu reopens it or
exits. A UDP lock on port 7850 enforces a single instance -- launching a
second copy just tells the first one to show itself."""

from __future__ import annotations

import socket
import threading
from pathlib import Path

_LOCK_PORT = 7850


def _claim_instance() -> socket.socket | None:
    """Bind the instance lock. If another instance holds it, ping it to show
    its window and return None."""
    s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    try:
        s.bind(("127.0.0.1", _LOCK_PORT))
        return s
    except OSError:
        s.close()
        ping = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        try:
            ping.sendto(b"SHOW", ("127.0.0.1", _LOCK_PORT))
        finally:
            ping.close()
        return None


def _show_listener(sock: socket.socket, window) -> None:
    while True:
        try:
            data, _ = sock.recvfrom(64)
        except OSError:
            return
        if data == b"SHOW":
            try:
                window.show()
            except Exception:
                pass


def _tray_image():
    from PIL import Image, ImageDraw
    img = Image.new("RGBA", (64, 64), (29, 32, 38, 255))
    d = ImageDraw.Draw(img)
    d.ellipse((14, 14, 50, 50), fill=(79, 140, 255, 255))
    return img


def run() -> None:
    lock = _claim_instance()
    if lock is None:
        print("[ui] already running; told the existing window to show")
        return

    import webview
    from ui.api import Api

    window = webview.create_window(
        "DialogReader Settings",
        str(Path(__file__).parent / "index.html"),
        js_api=Api(),
        width=720,
        height=880,
        background_color="#16181d",
    )

    def on_closing():
        window.hide()
        return False        # cancel the close; we live in the tray now

    window.events.closing += on_closing

    import pystray
    icon = pystray.Icon(
        "dialogue-reader-settings",
        _tray_image(),
        "DialogReader Settings",
        menu=pystray.Menu(
            pystray.MenuItem("Open", lambda i, m: window.show(), default=True),
            pystray.MenuItem("Exit", lambda i, m: (icon.stop(), window.destroy())),
        ),
    )
    threading.Thread(target=icon.run, daemon=True).start()
    threading.Thread(target=_show_listener, args=(lock, window), daemon=True).start()

    webview.start()
    icon.stop()
    lock.close()


if __name__ == "__main__":
    import sys
    sys.path.insert(0, str(Path(__file__).parent.parent))
    run()
