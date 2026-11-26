# Builds the v06 HTML mockups (static HTML, no JS) and their headless screenshots.
# Run: python _build.py [--no-shots] [page ...]
import os, random, subprocess, sys

OUT = os.path.dirname(os.path.abspath(__file__))
SHOTS = os.path.join(OUT, "shots")
EDGE = r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe"

# ---------- icons (one outline style, 24 grid, 1.75 stroke) ----------
P = {
 "min": '<path d="M5 12h14"/>',
 "max": '<rect x="5" y="5" width="14" height="14" rx="1"/>',
 "x": '<path d="M18 6 6 18M6 6l12 12"/>',
 "chev": '<path d="m6 9 6 6 6-6"/>',
 "plus": '<path d="M5 12h14M12 5v14"/>',
 "mic": '<rect x="9" y="3" width="6" height="11" rx="3"/><path d="M5 11a7 7 0 0 0 14 0M12 18v3"/>',
 "mouse": '<rect x="6" y="3" width="12" height="18" rx="6"/><path d="M12 7v4"/>',
 "settings": '<circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 1 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06A1.65 1.65 0 0 0 4.68 15a1.65 1.65 0 0 0-1.51-1H3a2 2 0 1 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06A1.65 1.65 0 0 0 9 4.68a1.65 1.65 0 0 0 1-1.51V3a2 2 0 1 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06A1.65 1.65 0 0 0 19.4 9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 1 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z"/>',
 "play": '<path d="M8 5.5v13l10-6.5z"/>',
 "pause": '<path d="M9 5v14M15 5v14"/>',
 "stop": '<rect x="6.5" y="6.5" width="11" height="11" rx="1.5"/>',
 "refresh": '<path d="M21 12a9 9 0 0 1-15.5 6.2L3 16"/><path d="M3 21v-5h5"/><path d="M3 12A9 9 0 0 1 18.5 5.8L21 8"/><path d="M21 3v5h-5"/>',
 "arrow-l": '<path d="M19 12H5M11 18l-6-6 6-6"/>',
 "arrow-r": '<path d="M5 12h14M13 6l6 6-6 6"/>',
 "check": '<path d="M20 6 9 17l-5-5"/>',
 "check-c": '<circle cx="12" cy="12" r="9"/><path d="m8.5 12 2.5 2.5 4.5-5"/>',
 "info": '<circle cx="12" cy="12" r="9"/><path d="M12 16v-5M12 8h.01"/>',
 "alert": '<path d="M12 3 2 20h20z"/><path d="M12 10v4M12 17h.01"/>',
 "window": '<rect x="3" y="4" width="18" height="16" rx="2"/><path d="M3 9h18"/>',
 "trash": '<path d="M4 7h16M10 11v6M14 11v6M6 7l1 13h10l1-13M9 7V4h6v3"/>',
 "camera": '<path d="M14.5 4h-5L7 7H4a2 2 0 0 0-2 2v9a2 2 0 0 0 2 2h16a2 2 0 0 0 2-2V9a2 2 0 0 0-2-2h-3z"/><circle cx="12" cy="13" r="3"/>',
 "zone": '<path d="M4 8V5a1 1 0 0 1 1-1h3M16 4h3a1 1 0 0 1 1 1v3M20 16v3a1 1 0 0 1-1 1h-3M8 20H5a1 1 0 0 1-1-1v-3"/>',
 "key": '<rect x="2" y="6" width="20" height="12" rx="2"/><path d="M6 10h.01M10 10h.01M14 10h.01M18 10h.01M7 14h10"/>',
 "tray": '<path d="M4 13h4l2 3h4l2-3h4"/><path d="M4 13 6 5h12l2 8v6H4z"/>',
 "crosshair": '<path d="M12 2v7M12 15v7M2 12h7M15 12h7"/>',
 "ban": '<circle cx="12" cy="12" r="9"/><path d="m5.7 5.7 12.6 12.6"/>',
 "clock": '<circle cx="12" cy="12" r="9"/><path d="M12 7v5l3 2"/>',
 "leaf": '<path d="M5 19c0-8 5-13 14-14-1 9-6 14-14 14z"/><path d="M5 19 13 11"/>',
 "bubble": '<path d="M4 5h16a1 1 0 0 1 1 1v10a1 1 0 0 1-1 1H10l-5 4v-4H4a1 1 0 0 1-1-1V6a1 1 0 0 1 1-1z"/><path d="M8 10h8M8 13h5"/>',
 "user": '<circle cx="12" cy="8" r="4"/><path d="M4 21a8 8 0 0 1 16 0"/>',
 "box": '<path d="M21 8 12 3 3 8v8l9 5 9-5z"/><path d="m3 8 9 5 9-5M12 13v8"/>',
}

def ic(n, cls=""):
    return f'<svg class="i {cls}" viewBox="0 0 24 24">{P[n]}</svg>'

KIND_ICON = {"dialogue": "bubble", "speaker": "user", "pickups": "box", "prompts": "key", "ignore": "ban"}

def kic(k):
    return f'<span class="kic {k}">{ic(KIND_ICON[k])}</span>'

def logo(s=20):
    bars = "".join(f'<rect x="{x}" y="{10 - h / 2:.1f}" width="1.8" height="{h}" rx="0.9" fill="#0B1020"/>'
                   for x, h in [(6.6, 3), (9.4, 7), (12.2, 10), (15.0, 7), (17.8, 3)])
    return (f'<svg class="logo" width="{s}" height="{s}" viewBox="0 0 24 24">'
            f'<path d="M5 2.5h14a3 3 0 0 1 3 3v9a3 3 0 0 1-3 3h-8.5L6 21.5v-4H5a3 3 0 0 1-3-3v-9a3 3 0 0 1 3-3z" fill="#8AA2FF"/>'
            f'{bars}</svg>')

# ---------- game covers: initials tile = FALLBACK for real box art (see tokens.css) ----------
GAMES = [
    ("Red Dead<br>Redemption 2", "RD", "#D4553A", "#4A1A12"),
    ("Satisfactory", "SF", "#F0A23A", "#2A5C7E"),
    ("Persona 4<br>Golden", "P4", "#F2C53A", "#8A5608"),
    ("Clair Obscur:<br>Expedition 33", "CO", "#B58AD6", "#3B2A5C"),
    ("Kingdom Come:<br>Deliverance II", "KC", "#9DB86E", "#33482A"),
]

def cover(i):
    _, ini, a, b = GAMES[i]
    return f'<div class="cover" style="background:linear-gradient(135deg,{a},{b})">{ini}</div>'

def sidebar(active=0, add_sel=False, settings_sel=False, empty=False):
    if empty:
        rows = '<div class="empty">No games yet. Your games show here after setup.</div>'
    else:
        rows = "".join(f'<div class="game{" sel" if active == i else ""}">{cover(i)}<div class="nm">{GAMES[i][0]}</div></div>'
                       for i in range(len(GAMES)))
    return (f'<aside class="side">{rows}<div class="div"></div>'
            f'<div class="navrow{" sel" if add_sel else ""}">{ic("plus")}Add game</div>'
            f'<div class="navrow end{" sel" if settings_sel else ""}">{ic("settings")}Settings</div></aside>')

# ---------- shell ----------
def titlebar():
    return (f'<header class="tbar">{logo(20)}DialogReader<div class="ctl">'
            f'<span>{ic("min", "sm")}</span><span>{ic("max", "sm")}</span><span>{ic("x", "sm")}</span></div></header>')

def chips(voice="Kokoro · Adam", key="Mouse 4", key_note=None):
    note = f'<span class="tag">{key_note}</span>' if key_note else ""
    return (f'<div class="chip">{ic("mic")}<span class="k">Voice:</span><span class="v">{voice}</span>{ic("chev", "sm chev")}</div>'
            f'<div class="chip">{ic("mouse")}<span class="k">Read key:</span><span class="v">{key}</span>{note}{ic("chev", "sm chev")}</div>')

def tray_note():
    return f'<div class="note">{ic("tray", "sm")}Close this window to play. DialogReader keeps running in the tray.</div>'

# bottom bar variants (rule in tokens.css)
def bbar_game(**kw):
    return f'<footer class="bbar">{chips(**kw)}{tray_note()}</footer>'

def bbar_setup(back=True, nxt="Next", icon="arrow-r"):
    b = f'<span class="btn ghost wide">{ic("arrow-l", "sm")}Back</span>' if back else ""
    ni = ic(icon, "sm") if icon else ""
    return f'<footer class="bbar"><div class="acts">{b}<span class="btn pri wide">{nxt}{ni}</span></div></footer>'

def bbar_plain():
    return f'<footer class="bbar">{tray_note()}</footer>'

def page(title, body, foot, css=""):
    return (f'<!doctype html><html lang="en"><head><meta charset="utf-8"><title>{title}</title>'
            f'<link rel="stylesheet" href="tokens.css">{css}</head><body><div class="win">{titlebar()}{body}{foot}</div></body></html>')

STEPS = 4

def head(title, status=False, eyebrow=None, step=None, lead=None, actions=""):
    top = ""
    if step:
        bars = "".join(f'<i class="{"on" if s <= step else ""}"></i>' for s in range(1, STEPS + 1))
        top = f'<div class="steps"><span class="lab">Step {step} of {STEPS}</span>{bars}</div>'
    elif eyebrow:
        top = f'<div class="eyebrow">{eyebrow}</div>'
    row = f'<div class="phrow"><div class="ptitle">{title}</div><span class="grow"></span>{actions}</div>'
    st = '<div class="status"><span class="dot"></span>Listening</div>' if status else ""
    ld = f'<div class="lead">{lead}</div>' if lead else ""
    return f'<div class="ph">{top}{row}{st}{ld}</div>'

# ---------- fake game scene (simple shapes) ----------
def scene(hud=False):
    rnd = random.Random(6)
    trees = []
    x = -10
    while x < 1620:
        if 560 < x < 1060:
            x += 26
            continue
        h = rnd.randint(60, 150)
        w = h * 0.38
        base = 610 + rnd.randint(-10, 14)
        trees.append(f'<polygon points="{x},{base} {x + w / 2:.0f},{base - h} {x + w:.0f},{base}" fill="#243041"/>'
                     f'<polygon points="{x + w * 0.3:.0f},{base - h * 0.55:.0f} {x + w / 2:.0f},{base - h} {x + w * 0.7:.0f},{base - h * 0.55:.0f}" fill="#D9E0E8" opacity="0.55"/>')
        x += rnd.randint(22, 40)
    far = "0,440 140,360 260,400 400,300 540,380 660,330 780,250 920,340 1040,280 1180,190 1320,290 1440,230 1600,300 1600,540 0,540"
    near = "0,540 160,450 300,500 460,420 620,490 780,430 920,490 1080,400 1240,470 1380,380 1520,440 1600,410 1600,620 0,620"
    caps = "".join(f'<polygon points="{px},{py} {px - 70},{py + 46} {px - 18},{py + 34} {px + 10},{py + 52} {px + 70},{py + 46}" fill="#E8EDF3"/>'
                   for px, py in [(400, 300), (780, 250), (1040, 280), (1180, 190), (1440, 230)])
    caps2 = "".join(f'<polygon points="{px},{py} {px - 50},{py + 34} {px},{py + 26} {px + 50},{py + 34}" fill="#C9D2DD"/>'
                    for px, py in [(160, 450), (460, 420), (1080, 400), (1380, 380)])
    riders = "".join(
        f'<g transform="translate({rx},{ry}) scale({s})"><ellipse cx="0" cy="28" rx="20" ry="11" fill="#3A2E27"/>'
        f'<rect x="-6" y="-2" width="12" height="24" rx="4" fill="#2A2522"/><ellipse cx="0" cy="-4" rx="11" ry="3" fill="#1E1A18"/>'
        f'<rect x="-14" y="30" width="4" height="18" fill="#3A2E27"/><rect x="10" y="30" width="4" height="18" fill="#3A2E27"/></g>'
        for rx, ry, s in [(790, 530, 1.2), (850, 520, 1.0), (900, 528, 1.1), (950, 524, 0.9)])
    # game HUD compass, bottom left (the Ignore zone sits on it)
    compass = ('<g transform="translate(118,802)"><circle r="62" fill="#1E1A16" opacity="0.8" stroke="#CFC6B4" stroke-width="3"/>'
               '<path d="M0 -40 L9 0 L0 40 L-9 0z" fill="#CFC6B4" opacity="0.9"/>'
               + "".join(f'<text x="{tx}" y="{ty}" text-anchor="middle" font-family="Georgia,serif" font-size="18" fill="#EDE8DD">{t}</text>'
                         for t, tx, ty in [("N", 0, -44), ("E", 50, 6), ("S", 0, 58), ("W", -50, 6)])
               + '</g>')
    hudtxt = ""
    if hud:
        hudtxt = ('<rect x="300" y="742" width="1000" height="50" rx="6" fill="#0B0D12" opacity="0.55"/>'
                  '<text x="800" y="776" text-anchor="middle" font-family="Georgia,serif" font-size="30" fill="#FFFFFF">'
                  '<tspan fill="#E6C56E">Arthur:</tspan> We need to get these horses moving before the snow buries us.</text>'
                  '<rect x="136" y="64" width="250" height="64" rx="4" fill="#0B0D12" opacity="0.7"/>'
                  '<text x="160" y="92" font-family="Georgia,serif" font-size="22" fill="#FFFFFF">Ginseng</text>'
                  '<text x="160" y="116" font-family="Georgia,serif" font-size="16" fill="#C9C4B8">Picked up</text>'
                  '<rect x="1266" y="812" width="200" height="42" rx="6" fill="#0B0D12" opacity="0.55"/><rect x="1330" y="820" width="26" height="26" rx="3" fill="none" stroke="#EDE8DD" stroke-width="2"/>'
                  '<text x="1343" y="839" text-anchor="middle" font-family="Segoe UI,sans-serif" font-size="15" fill="#EDE8DD">E</text>'
                  '<text x="1280" y="840" font-family="Georgia,serif" font-size="20" fill="#EDE8DD">Hold</text>'
                  '<text x="1366" y="840" font-family="Georgia,serif" font-size="20" fill="#EDE8DD">to talk</text>')
    return f'''<svg class="scene" viewBox="0 0 1600 900" preserveAspectRatio="xMidYMid slice">
<defs><linearGradient id="sky" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#3B475B"/><stop offset="0.55" stop-color="#8794A8"/><stop offset="1" stop-color="#C2CAD6"/></linearGradient>
<linearGradient id="snow" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#E4E9EF"/><stop offset="1" stop-color="#B9C3D0"/></linearGradient></defs>
<rect width="1600" height="900" fill="url(#sky)"/>
<ellipse cx="300" cy="120" rx="320" ry="60" fill="#55617A" opacity="0.55"/><ellipse cx="1150" cy="90" rx="380" ry="54" fill="#4C586F" opacity="0.6"/>
<polygon points="{far}" fill="#A7B2C1"/>{caps}
<polygon points="{near}" fill="#6B788B"/>{caps2}
{"".join(trees)}
<polygon points="0,610 1600,580 1600,900 0,900" fill="url(#snow)"/>
<polygon points="760,585 880,585 1180,900 520,900" fill="#CCD4DE"/>
{riders}
<polygon points="150,900 210,600 310,500 470,500 560,600 620,900" fill="#2C3345"/>
<path d="M250 560 Q400 640 540 600" stroke="#5A4434" stroke-width="10" fill="none"/>
<ellipse cx="390" cy="468" rx="44" ry="48" fill="#3A2F28"/>
<ellipse cx="390" cy="432" rx="120" ry="22" fill="#2A231F"/><rect x="335" y="380" width="110" height="54" rx="14" fill="#2F2722"/>
<polygon points="600,900 630,660 670,610 720,640 745,900" fill="#3B2F27"/><polygon points="640,640 650,585 668,622" fill="#3B2F27"/><polygon points="690,622 708,580 716,636" fill="#3B2F27"/>
{compass}
{hudtxt}
</svg>'''

# zones at the same relative spots on every screen (percent of the frame)
ZONES = {
    "pickups": (3, 9, 25, 13),
    "speaker": (21, 79, 11, 9),
    "dialogue": (34, 77, 42, 13),
    "prompts": (78.5, 79, 19, 9),
    "ignore": (2.2, 80.5, 10.4, 17.5),
}
ZLABEL = {"dialogue": "Dialogue", "speaker": "Speaker", "pickups": "Pickups", "prompts": "Prompts", "ignore": "Ignore"}

def zone(k, label=None, extra_cls="", handles=False):
    l, t, w, h = ZONES[k]
    body = {
        "pickups": f'<span style="color:#E8E2D2">{ic("leaf")}</span><span>Ginseng<br><span style="font-size:12px;color:#C9C4B8">Picked up</span></span>',
        "speaker": "Arthur",
        "dialogue": "We need to get these horses moving before the snow buries us.",
        "prompts": 'Hold <span class="gk">E</span> to talk',
        "ignore": "",
    }[k]
    hd = ""
    if handles:
        hd = "".join(f'<span class="hdl" style="{p}"></span>' for p in
                     ["left:-5px;top:-5px", "right:-5px;top:-5px", "left:-5px;bottom:-5px", "right:-5px;bottom:-5px"])
    return (f'<div class="zone {k} {extra_cls}" style="left:{l}%;top:{t}%;width:{w}%;height:{h}%">'
            f'<span class="zl">{label or ZLABEL[k]}</span>{body}{hd}</div>')

def frame(w, h, zones_html="", hud=False, cap=None):
    c = f'<span class="cap">{ic("camera", "sm")}{cap}</span>' if cap else ""
    return f'<div class="frame" style="width:{w}px;height:{h}px">{scene(hud)}{zones_html}{c}</div>'

def std_zones(sel=None, drawing=False):
    z = zone("ignore") + zone("pickups") + zone("speaker") + zone("dialogue", handles=(sel == "dialogue"))
    if drawing:
        z += zone("prompts", label="Prompts", extra_cls="drawing")
        z += f'<span style="position:absolute;left:calc(97.5% - 10px);top:calc(88% - 10px);color:#0B0E14">{ic("crosshair")}</span>'
    else:
        z += zone("prompts", label="Prompts · Off", extra_cls="off")
    return z

# ---------- shared blocks ----------
KINDS = [
    ("dialogue", "Dialogue", "Main spoken text: NPC lines, narration, subtitles"),
    ("speaker", "Speaker", "Who is talking, read before the line"),
    ("pickups", "Pickups", "Item and reward messages, like +1 Ginseng"),
    ("prompts", "Prompts", "Button hints, like Hold E to talk"),
    ("ignore", "Ignore", "Never read: compass, clock, HUD numbers"),
]

def legend():
    rows = ""
    for k, n, d in KINDS:
        off = '<span class="tag">Off</span>' if k == "prompts" else ""
        rows += (f'<div class="row"><span class="t"><span class="sw {k}{" off" if off else ""}"></span>{n}{off}</span>'
                 f'<span class="muted grow">{d}</span></div>')
    return f'<div class="card legend">{rows}</div>'

def srow(t, d, ctl, h=64):
    return f'<div class="row" style="min-height:{h}px"><div class="grow"><div class="t">{t}</div><div class="d">{d}</div></div>{ctl}</div>'

def wave(n=26, seed=3, dim_from=18):
    r = random.Random(seed)
    return '<div class="wave">' + "".join(
        f'<i class="{"dim" if i >= dim_from else ""}" style="height:{r.choice([4, 6, 8, 12, 16, 20, 24]) if i < dim_from else 4}px"></i>'
        for i in range(n)) + '</div>'

def kind_tools(active=None, extra=""):
    b = '<span class="faint b lab">Draw new:</span>'
    for k, n in [("dialogue", "Dialogue"), ("speaker", "Speaker"), ("pickups", "Pickups"), ("prompts", "Prompts"), ("ignore", "Ignore")]:
        b += f'<span class="btn{" on" if k == active else ""}"><span class="sw {k}"></span>{n}</span>'
    return f'<div class="toolrow">{b}<span class="grow"></span>{extra}</div>'

def write(name, html):
    with open(os.path.join(OUT, name), "w", encoding="utf-8") as f:
        f.write(html)

EXTRA_CSS = """<style>
.ell { white-space:nowrap; overflow:hidden; text-overflow:ellipsis; min-width:0; }
.stat { flex:1; background:var(--surface-2); border:1px solid var(--line); border-radius:var(--r-md); padding:var(--s-3); display:flex; flex-direction:column; gap:2px; }
.stat .v { font-size:var(--fs-4); font-weight:700; line-height:26px; }
.stat .l { font-size:var(--fs-1); color:var(--text-2); }
.now { font-size:var(--fs-4); font-weight:600; line-height:28px; }
.now .who { color:var(--text-2); }
.toolrow { display:flex; align-items:center; gap:var(--s-2); }
.toolrow .btn { padding:0 var(--s-3); }
.toolrow .lab { white-space:nowrap; margin-right:var(--s-1); }
.wrow { display:flex; align-items:center; gap:var(--s-4); padding:var(--s-3); border:1px solid var(--line); border-radius:var(--r-lg); background:var(--surface-2); }
.thumb { width:112px; height:63px; border-radius:var(--r-md); overflow:hidden; position:relative; flex:none; border:1px solid var(--line-2); }
.num { width:24px; height:24px; border-radius:50%; background:var(--surface-3); display:grid; place-items:center; font-size:var(--fs-1); font-weight:700; flex:none; }
.kv { display:flex; justify-content:space-between; padding:var(--s-2) 0; border-bottom:1px solid var(--line); }
.kv:last-child { border-bottom:none; }
.kv span:first-child { color:var(--text-2); }
.field { display:flex; flex-direction:column; gap:6px; }
.field .lbl { font-size:var(--fs-1); color:var(--text-2); font-weight:600; }
.grid2 { display:grid; grid-template-columns:1fr 1fr; }
.grid2 .row:nth-last-child(-n+2) { border-bottom:none; }
.grid2 .row:nth-child(odd) { border-right:1px solid var(--line); }
.log .row { align-items:flex-start; padding-top:12px; padding-bottom:12px; }
.log .row .kic, .log .row .ibtn { margin-top:2px; }
</style>"""

def page3(title, side, main, right, foot):
    body = f'<div class="body3">{side}<main class="main">{main}</main><aside class="right">{right}</aside></div>'
    return page(title, body, foot, EXTRA_CSS)

FW, FH = 728, 410
PAGES = []
def add(name, html):
    PAGES.append(name)
    write(name, html)

# =====================================================================
# 10-main (game page)
# =====================================================================
LOG = [
    ("dialogue", "<b>Dutch:</b> We're going to Colter. It's safer up in the hills.", "10:24:17", None),
    ("pickups", "+1 Ginseng", "10:24:09", None),
    ("prompts", "Hold E to talk", "10:23:57", "Skipped"),
    ("dialogue", "<b>Micah:</b> Snow's only getting worse up here, boys.", "10:23:41", None),
]

def log_rows(items):
    out = ""
    for k, t, tm, badge in items:
        b = f' <span class="tag">{badge}</span>' if badge else ""
        out += (f'<div class="row">{kic(k)}'
                f'<div class="grow"><div class="clamp2">{t}{b}</div><div class="d" style="margin-top:2px">{tm}</div></div>'
                f'<span class="ibtn">{ic("play", "sm")}</span></div>')
    return out

MODE_SEG = '<div class="seg"><span class="on">Automatic</span><span>On key press</span></div>'
main10 = (head("Red Dead Redemption 2", status=True,
               actions=f'<span class="btn">{ic("pause", "sm")}Pause reading</span>')
          + f'<div class="toolrow">{MODE_SEG}<span class="grow"></span>'
            f'<span class="btn">{ic("zone", "sm")}Edit zones</span></div>'
          + frame(FW, FH, std_zones(), cap="Last capture 10:24:17") + legend())
right10 = (
    f'<div class="card"><div class="hd">Now reading<span class="r"><span class="sw dialogue"></span>Dialogue</span></div>'
    f'<div class="bd"><div class="now"><span class="who">Arthur:</span> We need to get these horses moving before the snow buries us.</div>'
    f'<div style="display:flex;align-items:center;gap:12px">{wave()}<span class="grow"></span>'
    f'<span class="ibtn">{ic("stop", "sm")}</span></div></div></div>'
    f'<div class="card fill log"><div class="hd">Recent log</div>{log_rows(LOG)}</div>'
    '<div class="card"><div class="hd">This session<span class="r">Since 09:52</span></div>'
    '<div class="bd" style="flex-direction:row;gap:8px">'
    '<div class="stat"><span class="v">128</span><span class="l">Lines read</span></div>'
    '<div class="stat"><span class="v">14</span><span class="l">Skipped</span></div>'
    '<div class="stat"><span class="v">0.3 s</span><span class="l">Avg. delay</span></div></div></div>')
add("10-main.html", page3("Red Dead Redemption 2 - DialogReader", sidebar(0), main10, right10,
                          bbar_game(key_note="On key press only")))

# =====================================================================
# s01 welcome (not a step; first run, so no sidebar and no chips)
# =====================================================================
def feature(n, icon, t, d):
    return (f'<div class="card" style="width:232px"><div class="bd" style="gap:8px;padding:20px">'
            f'<div style="display:flex;align-items:center;gap:8px"><span class="num">{n}</span><span style="color:var(--text-2)">{ic(icon)}</span></div>'
            f'<div class="b" style="font-size:16px">{t}</div><div class="muted">{d}</div></div></div>')

welcome = (
    '<div style="display:flex;flex-direction:column;align-items:center;justify-content:center;gap:24px;min-height:0">'
    f'{logo(88)}'
    '<div style="text-align:center"><div class="ptitle">Welcome to DialogReader</div>'
    '<div class="lead" style="margin-top:8px">DialogReader reads the text in your games out loud.</div></div>'
    '<div style="display:flex;gap:16px;margin-top:16px">'
    + feature(1, "window", "Pick your game", "Choose the game window to watch.")
    + feature(2, "zone", "Mark where text appears", "Draw boxes over subtitles and pop-ups.")
    + feature(3, "mouse", "Choose how it reads", "Right away, or when you press a key.")
    + feature(4, "mic", "Pick a voice", "Then close the window and play.")
    + '</div><div class="faint" style="display:flex;align-items:center;gap:8px">' + ic("clock", "sm")
    + '4 short steps. About two minutes.</div></div>')
foot1 = f'<footer class="bbar"><div class="acts"><span class="btn pri wide">Get started{ic("arrow-r", "sm")}</span></div></footer>'
add("s01-welcome.html", page("Welcome - DialogReader", welcome, foot1, EXTRA_CSS))

# =====================================================================
# setup steps: first run, so the sidebar is empty and "Add game" stays selected
# =====================================================================
SETUP_SIDE = sidebar(-1, add_sel=True, empty=True)

# ---- step 1: pick window ----
def mini_window(kind):
    if kind == "game":
        return f'<div class="thumb" style="background:#2A3446">{scene()}</div>'
    if kind == "chat":
        return ('<div class="thumb" style="background:#1E2130;display:flex">'
                '<div style="width:20px;background:#15171F"></div><div style="width:26px;background:#191C27"></div>'
                '<div style="flex:1;padding:8px;display:flex;flex-direction:column;gap:6px">'
                + "".join(f'<div style="height:5px;width:{w}%;background:#3A3F55;border-radius:2px"></div>' for w in (70, 50, 80, 40))
                + '</div></div>')
    if kind == "browser":
        return ('<div class="thumb" style="background:#F1F3F6">'
                '<div style="height:12px;background:#D9DEE6"></div><div style="padding:8px;display:flex;flex-direction:column;gap:6px">'
                + "".join(f'<div style="height:5px;width:{w}%;background:#B9C1CE;border-radius:2px"></div>' for w in (60, 90, 75))
                + '</div></div>')
    if kind == "music":
        return ('<div class="thumb" style="background:#121212;display:flex;align-items:flex-end;padding:8px;gap:6px">'
                '<div style="width:36px;height:36px;border-radius:4px;background:linear-gradient(135deg,#1DB954,#0E5A2A)"></div>'
                '<div style="flex:1;display:flex;flex-direction:column;gap:5px"><div style="height:5px;width:80%;background:#3A3A3A;border-radius:2px"></div>'
                '<div style="height:5px;width:50%;background:#3A3A3A;border-radius:2px"></div></div></div>')
    return ('<div class="thumb" style="background:#1B2838;padding:8px;display:grid;grid-template-columns:repeat(4,1fr);gap:4px">'
            + "".join(f'<div style="background:{c};border-radius:2px"></div>' for c in
                      ("#B8472E", "#E0962E", "#E9B92A", "#8E6CA3", "#6E7F58", "#2E5A7A", "#4A6A8A", "#7A4A06"))
            + '</div>')

def wrow(kind, name, meta, sel=False):
    r = '<span class="check">' + ic("check") + '</span>' if sel else '<span class="radio"></span>'
    return (f'<div class="wrow{" sel" if sel else ""}">{mini_window(kind)}'
            f'<div class="grow"><div class="b" style="font-size:16px">{name}</div><div class="meta" style="margin-top:2px">{meta}</div></div>{r}</div>')

main2 = (head("Pick your game", step=1, lead="Start your game, then choose its window below.")
         + f'<div class="card fill"><div class="hd">Running windows<span class="r"><span class="btn">{ic("refresh", "sm")}Refresh</span></span></div>'
         + '<div class="bd">'
         + wrow("game", "Red Dead Redemption 2", "RDR2.exe · 3840 × 2160 · Borderless", True)
         + wrow("chat", "Discord", "Discord.exe · 1600 × 900 · Windowed")
         + wrow("browser", "Google Chrome", "chrome.exe · 1920 × 1080 · Windowed")
         + wrow("library", "Steam", "steamwebhelper.exe · 1280 × 800 · Windowed")
         + wrow("music", "Spotify", "Spotify.exe · 1200 × 800 · Windowed")
         + f'<div class="faint" style="display:flex;gap:8px;align-items:center">{ic("info", "sm")}Don’t see your game? Start it, then press Refresh.</div>'
         + '</div></div>')
right2 = (
    f'<div class="card"><div class="hd">Preview</div><div class="bd">{frame(294, 165)}'
    '<div><div class="kv"><span>Window</span><span class="b">Red Dead Redemption 2</span></div>'
    '<div class="kv"><span>Size</span><span class="b">3840 × 2160</span></div>'
    '<div class="kv"><span>Display mode</span><span class="b">Borderless</span></div></div></div></div>'
    f'<div class="card fill"><div class="hd">{ic("info", "sm")}Good to know</div><div class="bd muted">'
    '<div>Borderless or windowed mode works best.</div>'
    '<div>Exclusive full screen can give a black picture. Switch the game to borderless in its video settings.</div>'
    '<div>DialogReader never sends clicks or keys to the game. It only looks at the picture.</div></div></div>')
add("s02-pick-window.html", page3("Pick your game - DialogReader", SETUP_SIDE, main2, right2, bbar_setup(back=False)))

# ---- step 2: draw zones ----
drawn = [("dialogue", "Dialogue", "On", "ok"), ("speaker", "Speaker", "On", "ok"), ("pickups", "Pickups", "On", "ok"),
         ("ignore", "Ignore", "Never read", ""), ("prompts", "Prompts", "Drawing now", "")]
drawn_rows = "".join(f'<div class="row" style="min-height:40px"><span class="sw {k}"></span><span class="t grow">{n}</span>'
                     f'<span class="tag {c}">{s}</span></div>' for k, n, s, c in drawn)
drawn_rows += f'<div class="row" style="min-height:40px"><span class="faint">{ic("plus", "sm")}</span><span class="faint grow">Pick a kind above to add more</span></div>'
main3 = (head("Mark where text appears", step=2, lead="Draw a box over each place where the game shows text.",
              actions=f'<span class="btn">{ic("camera", "sm")}Retake</span>')
         + kind_tools("prompts")
         + frame(FW, FH, std_zones(drawing=True), cap="Captured 10:20:41")
         + f'<div class="card"><div class="hd">Drawn so far<span class="r">5 zones</span></div><div class="grid2">{drawn_rows}</div></div>')
right3 = (
    '<div class="card"><div class="hd">Draw a zone</div><div class="bd">'
    '<div style="display:flex;gap:12px;align-items:center"><span class="num">1</span><span>Pick a kind above the screenshot.</span></div>'
    '<div style="display:flex;gap:12px;align-items:center"><span class="num">2</span><span>Drag a box over the text.</span></div>'
    '<div style="display:flex;gap:12px;align-items:center"><span class="num">3</span><span>Drag a corner to fix the size.</span></div></div></div>'
    '<div class="card"><div class="hd">What each kind is for</div>'
    + "".join(f'<div class="row" style="min-height:44px"><span class="sw {k}"></span><span class="muted grow"><b style="color:var(--text)">{n}.</b> {d}</span></div>'
              for k, n, d in KINDS)
    + '</div>'
    f'<div class="card fill"><div class="hd">{ic("info", "sm")}Good to know</div><div class="bd muted">'
    '<div>Only Dialogue is needed. Add the others now or later.</div>'
    '<div>No text on screen? Talk to someone in the game, then press Retake.</div></div></div>')
add("s03-draw-zones.html", page3("Mark where text appears - DialogReader", SETUP_SIDE, main3, right3, bbar_setup()))

# ---- step 3: how should it read (mode first, key only for On key press) ----
modes = (
    '<div style="display:flex;gap:16px">'
    '<div class="opt"><span class="radio" style="margin-top:2px"></span><div><div class="b" style="font-size:16px">Automatic</div>'
    '<div class="muted" style="margin-top:4px">Reads new text as soon as it appears. No key needed.</div></div></div>'
    '<div class="opt sel"><span class="radio on" style="margin-top:2px"></span><div><div class="b" style="font-size:16px">On key press</div>'
    '<div class="muted" style="margin-top:4px">Reads every zone once when you press your read key.</div></div></div></div>')
cap = (
    '<div class="card"><div class="hd">Read key<span class="r">Only for On key press</span></div>'
    '<div class="bd" style="flex-direction:row;align-items:center;gap:16px;padding:20px 16px">'
    f'<span style="color:var(--text-2)">{ic("mouse").replace(chr(34) + "i " + chr(34), chr(34) + "i" + chr(34) + " style=" + chr(34) + "width:40px;height:40px;stroke-width:1.5" + chr(34))}</span>'
    '<div class="grow"><div style="display:flex;align-items:center;gap:12px"><span style="font-size:28px;font-weight:700;line-height:36px">Mouse 4</span>'
    f'<span class="check">{ic("check")}</span></div>'
    '<div class="muted">Press any key or mouse button to change it.</div></div>'
    '<span class="btn ghost">Clear</span></div></div>')
sug = (
    '<div class="card fill"><div class="hd">Keys that work well</div>'
    + "".join(f'<div class="row"><span style="width:168px;display:flex;gap:6px">{"".join(f"<span class=kbd>{k}</span>" for k in ks)}</span>'
              f'<span class="muted grow">{d}</span><span class="btn">Use</span></div>'
              for ks, d in [(["Mouse 4", "Mouse 5"], "Side buttons. Few games use them."),
                            (["F8", "F9"], "Free in most games."),
                            (["Num 0"], "Easy to find without looking."),
                            (["Middle click"], "Press the mouse wheel. Fine if the game does not zoom.")])
    + f'<div class="warnline">{ic("alert", "sm")}Avoid keys the game already uses, like <span class=kbd>Esc</span> <span class=kbd>Enter</span> <span class=kbd>W A S D</span></div>'
    + '</div>')
main4 = head("How should it read?", step=3, lead="Choose when DialogReader speaks. You can switch any time on the game page.") + modes + cap + sug
right4 = (
    '<div class="card"><div class="hd">Check the key</div><div class="bd">'
    '<div class="muted">Press Mouse 4 now. DialogReader shows when it sees the press.</div>'
    f'<div class="okbox">{ic("check-c")}<span class="grow">Mouse 4 seen at 10:21:04</span></div></div></div>'
    f'<div class="card fill"><div class="hd">{ic("info", "sm")}Good to know</div><div class="bd muted">'
    '<div>The key works while the game has focus.</div>'
    '<div>DialogReader does not block the key. The game still gets the press too, so pick a key the game does not use.</div>'
    '<div>In Automatic mode the key is not used.</div>'
    '<div>You can change the key later from the Read key chip on the game page.</div></div></div>')
add("s04-read-key.html", page3("How should it read - DialogReader", SETUP_SIDE, main4, right4, bbar_setup()))

# ---- step 4: voice + final check ----
VOICES = [("Adam", "US English · Male", True), ("Michael", "US English · Male", False),
          ("Bella", "US English · Female", False), ("Sarah", "US English · Female", False),
          ("Emma", "UK English · Female", False), ("George", "UK English · Male", False)]
vrows = "".join(
    f'<div class="row{" sel" if s else ""}">'
    f'<span class="{"radio on" if s else "radio"}"></span><div class="grow"><div class="t">Kokoro · {n}</div><div class="d">{m}</div></div>'
    f'<span class="ibtn">{ic("play", "sm")}</span></div>' for n, m, s in VOICES)
SLIDER = '<div style="display:flex;align-items:center;gap:16px;width:300px"><div class="slider"><div class="f" style="width:{p}%"></div><div class="h" style="left:{p}%"></div></div><span class="b" style="width:36px;text-align:right">{v}</span></div>'
main5 = (head("Pick a voice", step=4, lead="Choose how it sounds. Check your setup, then press Finish.")
         + f'<div class="card fill"><div class="hd">Voices<span class="r">Runs on this PC, no internet needed</span></div>{vrows}</div>'
         + '<div class="card"><div class="hd">Sound</div>'
         + srow("Speed", "How fast the voice speaks.", SLIDER.format(p=55, v="1.1×"), 58)
         + srow("Volume", "Separate from the game volume.", SLIDER.format(p=80, v="80%"), 58)
         + srow("Pause music while reading", "Pauses Spotify, YouTube and other media, then resumes.", '<span class="tog on"></span>', 58)
         + '</div>')
right5 = (
    '<div class="card"><div class="hd">Test sentence</div><div class="bd">'
    '<div class="now"><span class="who">Arthur:</span> We need to get these horses moving before the snow buries us.</div>'
    f'<div style="display:flex;align-items:center;gap:12px">{wave(22, 5, 14)}<span class="grow"></span><span class="ibtn">{ic("pause", "sm")}</span></div></div></div>'
    '<div class="card fill"><div class="hd">Check your setup</div><div class="bd" style="gap:0">'
    '<div class="kv"><span>Game</span><span class="b">Red Dead Redemption 2</span></div>'
    '<div class="kv"><span>Zones</span><span class="b">5 zones</span></div>'
    '<div class="kv"><span>Reading</span><span class="b">On key press</span></div>'
    '<div class="kv"><span>Read key</span><span class="b">Mouse 4</span></div>'
    '<div class="kv"><span>Voice</span><span class="b">Kokoro · Adam</span></div>'
    '<div class="kv"><span>Speed and volume</span><span class="b">1.1× · 80%</span></div>'
    '<div class="kv"><span>Pause music</span><span class="b">On</span></div>'
    '<div class="faint" style="margin-top:16px">You can change all of this later on the game page.</div></div></div>')
add("s05-voice.html", page3("Pick a voice - DialogReader", SETUP_SIDE, main5, right5, bbar_setup(nxt="Finish", icon="check")))

# =====================================================================
# s11 zones editor (game sub-page: eyebrow = game name)
# =====================================================================
reads = ('<div class="card legend">'
         + "".join(f'<div class="row"><span class="t"><span class="sw {k}{" off" if off else ""}"></span>{n}{off}</span>'
                   f'<span class="grow ell {c}">{t}</span></div>'
                   for k, n, t, c, off in [("dialogue", "Dialogue", "We need to get these horses moving before the snow buries us.", "", ""),
                                           ("speaker", "Speaker", "Arthur", "", ""),
                                           ("pickups", "Pickups", "Ginseng · Picked up", "", ""),
                                           ("prompts", "Prompts", "Hold E to talk · not spoken", "faint", '<span class="tag">Off</span>'),
                                           ("ignore", "Ignore", "Compass · never read", "faint", "")])
         + '</div>')
main11 = (head("Edit zones", eyebrow="Red Dead Redemption 2", actions=f'<span class="btn">{ic("camera", "sm")}Retake</span>')
          + kind_tools(None)
          + frame(FW, FH, std_zones(sel="dialogue"), cap="Last capture 10:24:17") + reads)
zrows = ""
for k, n, kind, on, sel in [("dialogue", "Subtitles", "Dialogue", True, True), ("speaker", "Name above subtitles", "Speaker", True, False),
                            ("pickups", "Item pop-up", "Pickups", True, False), ("prompts", "Button hints", "Prompts", False, False),
                            ("ignore", "Compass", "Ignore", True, False)]:
    zrows += (f'<div class="row{" sel" if sel else ""}" style="min-height:52px"><span class="sw {k}{"" if on else " off"}"></span><div class="grow"><div class="t">{n}</div><div class="d">{kind}</div></div>'
              f'<span class="tog{" on" if on else ""}"></span><span class="ibtn plain">{ic("trash", "sm")}</span></div>')
right11 = (
    f'<div class="card"><div class="hd">Zones<span class="r">5 zones</span></div>{zrows}</div>'
    '<div class="card fill"><div class="hd">Selected zone</div><div class="bd">'
    '<div class="field"><span class="lbl">Name</span><div class="sel-box">Subtitles</div></div>'
    f'<div class="field"><span class="lbl">Kind</span><div class="sel-box"><span class="sw dialogue"></span>Dialogue{ic("chev", "sm chev")}</div></div>'
    '<div class="field"><span class="lbl">Text found now</span><div class="muted" style="padding:10px 12px;border-radius:8px;background:var(--surface-2);border:1px solid var(--line)">We need to get these horses moving before the snow buries us.</div></div>'
    '<div class="field"><span class="lbl">Position</span><div class="muted">34% from left · 77% from top · 42% × 13%</div></div></div></div>'
    '<div style="display:flex;gap:12px"><span class="btn ghost" style="flex:1">Cancel</span><span class="btn pri" style="flex:1">Save</span></div>')
add("s11-zones.html", page3("Edit zones - DialogReader", sidebar(0), main11, right11, bbar_game(key_note="On key press only")))

# =====================================================================
# s12 settings (app-wide: no game chips, no Listening)
# =====================================================================
slots = [("Top left", 0, 0), ("Top centre", 1, 0), ("Top right", 2, 0), ("Bottom left", 0, 1), ("Bottom centre", 1, 1), ("Bottom right", 2, 1)]
picker = ('<div style="display:flex;align-items:center;gap:24px;padding:12px 16px">'
          '<div style="width:420px;height:136px;border:2px solid var(--line-2);border-radius:8px;background:var(--surface-2);'
          'display:grid;grid-template-columns:repeat(3,1fr);grid-template-rows:1fr 1fr;padding:10px;gap:8px;flex:none">'
          + "".join(f'<span class="btn{" on" if (x, y) == (2, 0) else ""}" style="align-self:{"start" if y == 0 else "end"};justify-self:{("start", "center", "end")[x]}">{n}</span>'
                    for n, x, y in slots)
          + '</div><div class="grow"><div class="t b">Overlay position</div><div class="muted" style="margin-top:2px">'
            'Where the small status pill sits on top of your game.</div></div></div>')
main12 = (head("Settings")
          + '<div class="card"><div class="hd">General</div>'
          + srow("Start with Windows", "Open DialogReader in the tray when Windows starts.", '<span class="tog on"></span>')
          + srow("Close to tray", "Closing the window keeps reading in the background.", '<span class="tog on"></span>') + '</div>'
          + '<div class="card"><div class="hd">Overlay</div>' + picker
          + '<div style="border-top:1px solid var(--line)">' + srow("Show reading status", "The pill shows when a line is being read, and who speaks.", '<span class="tog on"></span>') + '</div></div>'
          + '<div class="card fill"><div class="hd">Voice for new games</div>'
          + srow("Default voice", "Each game can still pick its own voice.", f'<div class="sel-box">Kokoro · Adam{ic("chev", "sm chev")}</div>')
          + srow("Speed", "How fast the voice speaks.",
                 '<div style="display:flex;align-items:center;gap:16px;width:220px"><div class="slider"><div class="f" style="width:50%"></div><div class="h" style="left:50%"></div></div><span class="b">1.0×</span></div>')
          + '</div>')
right12 = (
    '<div class="card"><div class="hd">Updates</div><div class="bd">'
    f'<div style="display:flex;align-items:center;gap:12px"><span style="color:var(--ok)">{ic("check-c")}</span><span class="b">You are up to date.</span></div>'
    '<div class="meta">Checked today at 09:50</div>'
    '<span class="btn" style="align-self:flex-start">Check for updates</span></div></div>'
    '<div class="card"><div class="hd">Game profiles</div><div class="bd">'
    '<div class="muted">Zones, keys and voices for each game are saved here.</div>'
    '<div class="sel-box" style="color:var(--text-2)">%APPDATA%\\DialogReader\\profiles</div>'
    '<div style="display:flex;gap:8px"><span class="btn">Open folder</span><span class="btn ghost">Export</span><span class="btn ghost">Import</span></div></div></div>'
    '<div class="card fill"><div class="hd">About</div><div class="bd">'
    f'<div style="display:flex;align-items:center;gap:12px">{logo(40)}<div><div class="b" style="font-size:16px">DialogReader</div>'
    '<div class="muted">Reads the text in your games out loud.</div></div></div>'
    '<div><div class="kv"><span>Version</span><span class="b">0.6.0</span></div>'
    '<div class="kv"><span>Voice engine</span><span class="b">Kokoro</span></div></div>'
    '<div class="muted">Text reading and voices run on this PC. Nothing is sent online.</div>'
    '<div style="display:flex;gap:8px;margin-top:auto"><span class="btn">Report a problem</span><span class="btn ghost">Open log folder</span></div></div></div>')
add("s12-settings.html", page3("Settings - DialogReader", sidebar(-1, settings_sel=True), main12, right12, bbar_plain()))

# =====================================================================
# s13 overlay (status only: dot + Reading + speaker)
# =====================================================================
pill = (f'<div class="pill" style="top:16px;right:16px"><span class="dot"></span>{logo(16)}'
        '<span class="b">Reading</span><span class="who">· Arthur</span></div>')
ov = ('<!doctype html><html lang="en"><head><meta charset="utf-8"><title>Overlay - DialogReader</title>'
      '<link rel="stylesheet" href="tokens.css"></head><body>'
      f'<div class="frame" style="width:1440px;height:900px;border:none;border-radius:0">{scene(hud=True)}{pill}</div></body></html>')
add("s13-overlay.html", ov)

# =====================================================================
# index contact sheet
# =====================================================================
ORDER = ["s01-welcome", "s02-pick-window", "s03-draw-zones", "s04-read-key", "s05-voice",
         "10-main", "s11-zones", "s12-settings", "s13-overlay"]
NAMES = {"s01-welcome": "Welcome (first run)", "s02-pick-window": "Step 1 · Pick your game", "s03-draw-zones": "Step 2 · Mark where text appears",
         "s04-read-key": "Step 3 · How should it read?", "s05-voice": "Step 4 · Pick a voice", "10-main": "Game page",
         "s11-zones": "Edit zones", "s12-settings": "Settings", "s13-overlay": "In-game overlay"}
CONFIRM = [
    "Close to tray, the profiles folder with Export / Import, and Report a problem",
    "“Keys that work well” and the key check on step 3",
    "Volume and “Pause music while reading” (assumes the shipped media-pause feature)",
    "Session stats 128 / 14 / 0.3 s (need real data sources)",
    "Read key in Automatic mode: not used (chip says “On key press only”)",
    "Pause reading on the game page (per game, not a global quit)",
]
cards = "".join(f'<a class="sheet" href="{n}.html"><img src="shots/{n}.png" alt="{NAMES[n]}"><span>{NAMES[n]}</span></a>' for n in ORDER)
idx = ('<!doctype html><html lang="en"><head><meta charset="utf-8"><title>DialogReader v06 mockups</title>'
       '<link rel="stylesheet" href="tokens.css"><style>'
       'html,body{width:auto;height:auto;overflow:auto}body{padding:32px}'
       'h1{font-size:36px;font-weight:700;margin-bottom:8px}p{color:var(--text-2);margin-bottom:16px;font-size:16px}'
       'h2{font-size:16px;font-weight:600;margin-bottom:8px}'
       '.grid{display:grid;grid-template-columns:repeat(3,1fr);gap:24px}'
       '.sheet{display:flex;flex-direction:column;gap:8px;color:var(--text);text-decoration:none;font-weight:600}'
       '.sheet img{width:100%;border-radius:10px;border:1px solid var(--line-2);display:block}'
       '.tok{display:flex;gap:8px;flex-wrap:wrap;margin-bottom:24px}.tok span{display:flex;align-items:center;gap:8px;padding:6px 10px;border:1px solid var(--line);border-radius:8px;font-size:12px;color:var(--text-2)}'
       '.tok i{width:16px;height:16px;border-radius:4px;display:block}'
       '.confirm{margin-bottom:24px;color:var(--text-2);font-size:14px;padding-left:20px}.confirm li{margin:2px 0}'
       '</style></head><body><h1>DialogReader · v06 mockups</h1>'
       '<p>1440 × 900. Flat fills, solid borders, rounded rects, opacity and one simple shadow, so every screen can be rebuilt in Slint. '
       'No dashed lines. Tokens and screen rules live in tokens.css.</p>'
       '<div class="tok">'
       + "".join(f'<span><i style="background:var({v})"></i>{n}</span>' for n, v in
                 [("Accent / selected", "--accent"), ("Listening", "--ok"), ("Dialogue", "--z-dialogue"), ("Speaker", "--z-speaker"),
                  ("Pickups", "--z-pickups"), ("Prompts", "--z-prompts"), ("Ignore", "--z-ignore"),
                  ("Surface", "--surface"), ("Line", "--line-2")])
       + '</div><h2>To confirm with the owner before it becomes real UI</h2><ul class="confirm">'
       + "".join(f"<li>{c}</li>" for c in CONFIRM)
       + f'</ul><div class="grid">{cards}</div></body></html>')
write("index.html", idx)

# ---------- screenshots ----------
def shoot(name, h=900):
    src = "file:///" + os.path.join(OUT, name + ".html").replace("\\", "/")
    dst = os.path.join(SHOTS, name + ".png")
    import time, tempfile
    prof = tempfile.mkdtemp(prefix="dr-v06-edge-")
    t0 = time.time()
    try:
        subprocess.run([EDGE, "--headless=new", "--disable-gpu", "--hide-scrollbars", "--no-first-run",
                        f"--user-data-dir={prof}", f"--window-size=1440,{h}",
                        f"--screenshot={dst}", src], capture_output=True, timeout=60)
    except subprocess.TimeoutExpired:
        print("timeout", name)
    # msedge.exe hands off to a child process and returns at once; wait for the file to be rewritten
    ok = False
    for _ in range(120):
        if os.path.exists(dst) and os.path.getmtime(dst) >= t0 - 1 and os.path.getsize(dst) > 0:
            time.sleep(1); ok = True; break
        time.sleep(0.5)
    print(name, "ok" if ok else "STALE", flush=True)

if "--no-shots" not in sys.argv:
    os.makedirs(SHOTS, exist_ok=True)
    only = [a for a in sys.argv[1:] if not a.startswith("--")]
    for n in (only or ORDER):
        shoot(n)
    if not only:
        shoot("index", 1300)
    print("shots done")
