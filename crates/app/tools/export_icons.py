"""Export icons and fake game art from the d-black mockups for the Slint UI.

Reads design/mockups/d-black/_build.py, runs it with its output folder pointed at a temp folder
(so the mockups are not touched), and writes the icons it has (older icons from c-midnight that d-black does not
draw stay as they are):

- ui/icons/<name>.svg      line icons, stroke drawn for 16 px (1.5 px on screen)
- ui/icons/<name>-sm.svg   the same icon, stroke drawn for 12 px (2 px on screen, as in tokens.css)
- ui/icons/<name>-lg.svg   the same icon, stroke drawn for 22 px (1.5 px on screen)
- ui/icons/<name>-fill.svg filled form (play, stop)
- ui/icons/logo.svg        the app logo
- ui/art/<game>.svg        game thumbnails (160 x 90)
- ui/art/scene.svg         the fake game screenshot (1600 x 900, d-black SCENE_RDR: neutral greys, Segoe UI)
- ui/art/scene-1440.png    with --png only: that scene drawn by headless Edge at 1440 x 900 ("cover", as in the
                           mockups), for the full-window overlay previews. Skipped with a note when Edge is missing.

All icons are white. Slint draws them with `colorize`, so one file serves every color.

Run from anywhere: python -I crates/app/tools/export_icons.py [--png]
"""

import os
import re
import shutil
import subprocess
import sys
import tempfile
import time

HERE = os.path.dirname(os.path.abspath(__file__))
APP = os.path.dirname(HERE)
REPO = os.path.dirname(os.path.dirname(APP))
BUILD = os.path.join(REPO, "design", "mockups", "d-black", "_build.py")


def load_build():
    src = open(BUILD, encoding="utf-8").read()
    tmp = tempfile.mkdtemp(prefix="dr-mockups-")
    src = re.sub(r'^OUT = .*$', lambda m: f'OUT = r"{tmp}"', src, count=1, flags=re.M)
    ns = {"__name__": "_build"}
    exec(compile(src, BUILD, "exec"), ns)  # writes the HTML pages into tmp only
    return ns


def icon_svg(body, stroke):
    return ('<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" '
            f'stroke="#FFFFFF" stroke-width="{stroke}" stroke-linecap="round" stroke-linejoin="round">{body}</svg>\n')


def fill_svg(body):
    return ('<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" '
            f'fill="#FFFFFF" stroke="none">{body}</svg>\n')


EDGE = r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe"


def file_url(path):
    return "file:///" + path.replace("\\", "/")


def render_scene_png(svg_path, png_path):
    """Draws the scene with headless Edge (no window) at 1440 x 900, as the mockup pages do."""
    if not os.path.exists(EDGE):
        print("Edge not found: ui/art/scene-1440.png not updated")
        return
    tmp = tempfile.mkdtemp(prefix="dr-scene-")
    page = os.path.join(tmp, "scene.html")
    with open(page, "w", encoding="utf-8") as f:
        f.write('<!doctype html><html><head><style>html,body{margin:0;background:#07090E}'
                'img{display:block;width:1440px;height:900px;object-fit:cover}</style></head>'
                f'<body><img src="{file_url(svg_path)}"></body></html>')
    out = os.path.join(tmp, "scene.png")
    for attempt in range(5):  # headless Edge sometimes exits before it writes the file
        subprocess.run([EDGE, "--headless=new", "--disable-gpu", "--hide-scrollbars", "--virtual-time-budget=5000",
                        "--user-data-dir=" + os.path.join(tmp, f"profile{attempt}"), "--window-size=1440,900",
                        "--screenshot=" + out, file_url(page)],
                       stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, check=False)
        for _ in range(50):  # it can write the file a moment after the launcher exits
            if os.path.exists(out) and os.path.getsize(out) > 0:
                time.sleep(0.2)
                shutil.copyfile(out, png_path)
                return
            time.sleep(0.2)
    print("Edge did not write the scene: ui/art/scene-1440.png not updated")


def main():
    ns = load_build()
    icons = os.path.join(APP, "ui", "icons")
    art = os.path.join(APP, "ui", "art")
    os.makedirs(icons, exist_ok=True)
    os.makedirs(art, exist_ok=True)
    n = 0
    for name, body in ns["P"].items():
        # The mockups draw strokes with vector-effect: non-scaling-stroke, so the on-screen width is fixed.
        # In a 24 unit viewBox shown at S px, an on-screen width of W px needs W * 24 / S units.
        for suffix, stroke in (("", 2.25), ("-sm", 4), ("-lg", 1.64)):
            with open(os.path.join(icons, f"{name}{suffix}.svg"), "w", encoding="utf-8") as f:
                f.write(icon_svg(body, stroke))
        n += 1
    for name in ("play", "stop"):
        with open(os.path.join(icons, f"{name}-fill.svg"), "w", encoding="utf-8") as f:
            f.write(fill_svg(ns["P"][name]))
    logo = (ns["LOGO"].replace('width="20" height="20"', 'width="24" height="24"')
            .replace(' aria-hidden="true" style="display:block"', ' xmlns="http://www.w3.org/2000/svg"'))
    with open(os.path.join(icons, "logo.svg"), "w", encoding="utf-8") as f:
        f.write(logo + "\n")
    for g, (defs, body) in ns["ART"].items():
        with open(os.path.join(art, f"{g}.svg"), "w", encoding="utf-8") as f:
            f.write('<svg xmlns="http://www.w3.org/2000/svg" width="640" height="360" viewBox="0 0 160 90">'
                    f'<defs>{defs}</defs>{body}</svg>\n')
    scene = ns["SCENE_RDR"].replace('class="scene" ',
                                    'xmlns="http://www.w3.org/2000/svg" width="1600" height="900" ')
    scene = scene.replace(' aria-hidden="true"', "")
    with open(os.path.join(art, "scene.svg"), "w", encoding="utf-8") as f:
        f.write(scene + "\n")
    if "--png" in sys.argv:
        render_scene_png(os.path.join(art, "scene.svg"), os.path.join(art, "scene-1440.png"))
    print(f"{n} icons, {len(ns['ART'])} thumbnails and the scene written to {APP}\\ui")


if __name__ == "__main__":
    main()
