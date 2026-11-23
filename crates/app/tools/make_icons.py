"""Draws the DialogReader logo (ui/icons/logo.svg) as the tray icon and the exe/installer icon.

    python crates/app/tools/make_icons.py

Writes `crates/app/ui/icons/tray.png` (32 x 32, the Slint SystemTrayIcon needs pixels, not SVG) and
`crates/app/app.ico` (16 to 256 px, embedded in DialogReader.exe by build.rs and used by the installer).
The shapes are the ones in logo.svg: a light rounded square and three bars, the last one accent blue.
Needs Pillow. Run it again only when the logo changes; the outputs are committed.
"""
from __future__ import annotations

from pathlib import Path

from PIL import Image, ImageDraw

APP = Path(__file__).resolve().parent.parent
SS = 16  # supersampling factor

# logo.svg, viewBox 0 0 24 24
TILE = ("#EDF0F6", (0, 0, 24, 24), 6)
BARS = (
    ("#07090E", (6.5, 9, 2, 6)),
    ("#07090E", (11, 6, 2, 12)),
    ("#5B7CFF", (15.5, 8, 2, 8)),
)


def draw(size: int) -> Image.Image:
    big = size * SS
    k = big / 24
    img = Image.new("RGBA", (big, big), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)
    color, (x, y, w, h), r = TILE
    d.rounded_rectangle((x * k, y * k, (x + w) * k - 1, (y + h) * k - 1), radius=r * k, fill=color)
    for color, (x, y, w, h) in BARS:
        d.rounded_rectangle((x * k, y * k, (x + w) * k - 1, (y + h) * k - 1), radius=1 * k, fill=color)
    return img.resize((size, size), Image.LANCZOS)


def main() -> None:
    draw(32).save(APP / "ui" / "icons" / "tray.png")
    sizes = [16, 20, 24, 32, 40, 48, 64, 256]
    icons = [draw(s) for s in sizes]
    icons[-1].save(APP / "app.ico", sizes=[(s, s) for s in sizes], append_images=icons[:-1])
    print("wrote tray.png and app.ico")


if __name__ == "__main__":
    main()
