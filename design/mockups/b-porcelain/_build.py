import os, math
OUT = os.path.dirname(os.path.abspath(__file__))

# ---------------- icons (24 grid, drawn at 18px; stroke-width 2 renders as 1.5px) ----------------
ICONS = {
    "wave": '<path d="M4 10v4M8 7v10M12 4v16M16 8v8M20 10v4"/>',
    "plus": '<path d="M12 5v14M5 12h14"/>',
    "play": '<path d="M7.5 5.5l11 6.5-11 6.5z"/>',
    "pause": '<path d="M9 5.5v13M15 5.5v13"/>',
    "stop": '<rect x="6.5" y="6.5" width="11" height="11" rx="1.5"/>',
    "replay": '<path d="M4 12a8 8 0 1 0 2.5-5.8L4 8.5"/><path d="M4 4v4.5h4.5"/>',
    "keyboard": '<rect x="2.5" y="6" width="19" height="12" rx="2"/><path d="M6.5 10h1M10.5 10h1M14.5 10h1M8 14h8"/>',
    "mouse": '<rect x="6" y="3" width="12" height="18" rx="6"/><path d="M12 7v3"/>',
    "volume": '<path d="M11 5L6.5 9H3.5v6h3l4.5 4z"/><path d="M15.5 9a4.5 4.5 0 0 1 0 6M18.5 6a8.5 8.5 0 0 1 0 12"/>',
    "scan": '<path d="M3 8V5.5A2.5 2.5 0 0 1 5.5 3H8M16 3h2.5A2.5 2.5 0 0 1 21 5.5V8M21 16v2.5a2.5 2.5 0 0 1-2.5 2.5H16M8 21H5.5A2.5 2.5 0 0 1 3 18.5V16"/>',
    "monitor": '<rect x="3" y="4" width="18" height="13" rx="2"/><path d="M8 21h8M12 17v4"/>',
    "check": '<path d="M5 12.5l4.5 4.5L19 7.5"/>',
    "chev-r": '<path d="M9.5 6l6 6-6 6"/>',
    "chev-l": '<path d="M14.5 6l-6 6 6 6"/>',
    "chev-d": '<path d="M6 9.5l6 6 6-6"/>',
    "arrow-r": '<path d="M5 12h14M13 6l6 6-6 6"/>',
    "search": '<circle cx="11" cy="11" r="6.5"/><path d="M20 20l-4.2-4.2"/>',
    "zap": '<path d="M13 3L5 13.5h6L10 21l8-10.5h-6z"/>',
    "sliders": '<path d="M4 7h9M17 7h3M4 17h3M11 17h9"/><circle cx="15" cy="7" r="2"/><circle cx="9" cy="17" r="2"/>',
    "refresh": '<path d="M20 12a8 8 0 0 1-14 5.3L4 15.5M4 20v-4.5h4.5M4 12a8 8 0 0 1 14-5.3L20 8.5M20 4v4.5h-4.5"/>',
    "power": '<path d="M12 3.5v8"/><path d="M7 6.5a7.5 7.5 0 1 0 10 0"/>',
    "layers": '<rect x="3" y="4" width="18" height="16" rx="2"/><rect x="13" y="7" width="5" height="3" rx="1"/>',
    "cursor": '<path d="M5 4l5.5 15 2.2-6.3L19 10.5z"/>',
    "square": '<rect x="4" y="4" width="16" height="16" rx="2"/><path d="M12 8.5v7M8.5 12h7"/>',
    "trash": '<path d="M4 7h16M9.5 7V4.5h5V7M6.5 7l1 13h9l1-13"/>',
    "camera": '<path d="M4 8h3l2-3h6l2 3h3v11H4z"/><circle cx="12" cy="13" r="3.5"/>',
    "gauge": '<path d="M4.5 17a8 8 0 1 1 15 0"/><path d="M12 13l3.5-4"/><circle cx="12" cy="13" r="1"/>',
    "download": '<path d="M12 4v11M7 10l5 5 5-5M5 20h14"/>',
    "gamepad": '<rect x="2.5" y="6.5" width="19" height="11" rx="5"/><path d="M7 12h4M9 10v4M15.5 11h.5M17.5 13h.5"/>',
    "info": '<circle cx="12" cy="12" r="8.5"/><path d="M12 11v5M12 8v.5"/>',
    "w-min": '<path d="M7 12h10"/>',
    "w-max": '<rect x="7" y="7" width="10" height="10" rx="1"/>',
    "w-x": '<path d="M7 7l10 10M17 7L7 17"/>',
    "x": '<path d="M7 7l10 10M17 7L7 17"/>',
    "music": '<path d="M9 18V6l10-2v12"/><circle cx="6.5" cy="18" r="2.5"/><circle cx="16.5" cy="16" r="2.5"/>',
    "zoom": '<circle cx="11" cy="11" r="6.5"/><path d="M20 20l-4.2-4.2M8.5 11h5M11 8.5v5"/>',
    "pencil": '<path d="M5 19l1-4L15.5 5.5a2 2 0 0 1 3 3L9 18z"/>',
    "skip": '<path d="M6 6l8 6-8 6z"/><path d="M18 6v12"/>',
    "minus": '<path d="M7 12h10"/>',
    "text": '<path d="M5 7V5h14v2M12 5v14M9 19h6"/>',
    "tray": '<path d="M4 13h4l1.5 3h5L16 13h4"/><path d="M4 13l2-7h12l2 7v6H4z"/>',
}

def ic(name, extra="", lg=False):
    sw = "1.125" if lg else "2"   # 1.5px at 18px; 1.5px at 32px for hero glyphs
    return (f'<svg class="i{(" " + extra) if extra else ""}" viewBox="0 0 24 24" fill="none" stroke="currentColor" '
            f'stroke-width="{sw}" stroke-linecap="round" stroke-linejoin="round">{ICONS[name]}</svg>')

WINCTL = f'<div class="winctl"><span>{ic("w-min")}</span><span>{ic("w-max")}</span><span>{ic("w-x")}</span></div>'

def topbar(crumbs):
    parts = []
    for i, c in enumerate(crumbs or []):
        if i:
            parts.append('<span class="sl">/</span>')
        parts.append(f"<b>{c}</b>" if i == len(crumbs) - 1 else f"<span>{c}</span>")
    return f'<div class="topbar"><div class="crumb">{"".join(parts)}</div>{WINCTL}</div>'

def page(title, body, css="", dark=False):
    theme = ' data-theme="dark"' if dark else ""
    return f'''<!doctype html>
<html lang="en"{theme}>
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=1440">
<title>{title}</title>
<link rel="stylesheet" href="tokens.css">
<style>
{css}
</style>
</head>
<body>
<div class="window">
{body}
</div>
</body>
</html>
'''

BRAND = f'<div class="brand"><span class="logo">{ic("wave")}</span>DialogReader</div>'

def nav_row(lead, title, sub, cls="", trail=""):
    return (f'<a class="nav-item{(" " + cls) if cls else ""}">{lead}<span class="nav-text"><span class="nav-title">{title}</span>'
            f'<span class="nav-sub">{sub}</span></span>{trail}</a>')

def lead_icon(name):
    return f'<span class="lead icon">{ic(name)}</span>'

# ---------------- main sidebar ----------------
GAMES = [
    ("g-rdr", "Red Dead Redemption 2", '<span class="dot ok"></span>Reading now'),
    ("g-sat", "Satisfactory", "Played yesterday"),
    ("g-p4g", "Persona 4 Golden", "Played 3 days ago"),
    ("g-co", "Clair Obscur: Expedition 33", "Played last week"),
    ("g-kcd", "Kingdom Come: Deliverance II", "Zones not set up yet"),
]

def main_sidebar(active="rdr"):
    items = [nav_row(f'<span class="thumb {c}"></span>', n, s, "active" if (active == "rdr" and c == "g-rdr") else "") for c, n, s in GAMES]
    items.append(nav_row(lead_icon("plus"), "Add a game", "Takes about 3 minutes"))
    s = nav_row(lead_icon("sliders"), "Settings", "Startup, hotkeys, voices", "active" if active == "settings" else "")
    return f'''<aside class="sidebar">
{BRAND}
<div class="side-label">Games</div>
<nav class="nav">{"".join(items)}</nav>
<div class="spacer"></div>
<nav class="nav">{s}</nav>
</aside>'''

# ---------------- setup sidebar ----------------
STEPS = [("Choose the game window", "Red Dead Redemption 2", "The game you want read"),
         ("Draw zones", "4 zones", "Where the text shows up"),
         ("Choose a read key", "Mouse 4", "One key to read on demand"),
         ("Pick a voice", "Kokoro · Adam", "Runs on this PC"),
         ("Choose a mode", "Automatic", "Automatic or on key press")]

def flow_sidebar(now, hover=None):
    li = []
    for n, (s, done_sub, todo_sub) in enumerate(STEPS, 1):
        if n < now:
            trail = f'<span class="edit-hint">{ic("pencil")}</span>' if hover == n else ""
            li.append(nav_row(f'<span class="lead num done">{ic("check")}</span>', s, done_sub, "step-done" + (" hover" if hover == n else ""), trail))
        elif n == now:
            li.append(nav_row(f'<span class="lead num now">{n}</span>', s, todo_sub, "active"))
        else:
            li.append(nav_row(f'<span class="lead num">{n}</span>', s, todo_sub, "todo"))
    exit_row = nav_row(lead_icon("x"), "Exit setup", "Your choices so far are kept")
    return f'''<aside class="sidebar">
{BRAND}
<div class="side-label">Set up a game</div>
<nav class="nav">{"".join(li)}</nav>
<div class="spacer"></div>
<nav class="nav">{exit_row}</nav>
</aside>'''

FLOW_CSS = """
.nav-item.todo { color: var(--text-3); }
.edit-hint { color: var(--text-3); display:grid; place-items:center; }
"""

def flow_foot(step, hint="", primary="Continue", primary_icon="arrow-r"):
    b = f'<button class="btn">{ic("chev-l")}Back</button>' if step > 1 else ""
    return f'''<footer class="flow-foot"><span class="caption">Step {step} of 5</span><span class="spacer"></span><span class="small">{hint}</span>
{b}<button class="btn btn-primary">{primary}{ic(primary_icon)}</button></footer>'''

def phead(title, body=None, meta=None, lead="", actions=""):
    b = f'<p class="body">{body}</p>' if body else ""
    m = f'<div class="meta">{meta}</div>' if meta else ""
    a = f'<div class="actions">{actions}</div>' if actions else ""
    return f'<header class="phead">{lead}<div class="col"><h1 class="t5">{title}</h1>{m}{b}</div>{a}</header>'

# ---------------- the RDR2 scene ----------------
def trees():
    out = []
    spots = [(250, 545, 120), (300, 560, 150), (345, 548, 110), (395, 565, 170), (450, 552, 125), (500, 570, 95),
             (1130, 540, 110), (1180, 555, 150), (1230, 545, 120), (1290, 560, 175), (1345, 548, 130), (1400, 562, 105), (1450, 550, 140),
             (40, 590, 210), (110, 600, 250), (1520, 600, 260), (1580, 590, 200)]
    for x, y, h in spots:
        w = h * 0.3
        out.append(f'<polygon points="{x},{y-h} {x-w*0.55},{y-h*0.45} {x-w*0.3},{y-h*0.45} {x-w},{y} {x+w},{y} {x+w*0.3},{y-h*0.45} {x+w*0.55},{y-h*0.45}" fill="#26302F"/>')
    return "".join(out)

def scene(s="a", hud="text", W=1600):
    """hud = 'text' for large pictures, 'bars' for small previews (text drawn as simple bars), None for no HUD."""
    dx = (W - 1600) / 2
    hudsvg = ""
    if hud:
        common = f'''
<g transform="translate(60,62)"><rect width="44" height="44" rx="6" fill="#000" fill-opacity=".45" stroke="#fff" stroke-opacity=".55"/>
<path d="M22 34v-14M22 22c-6-2-8-8-6-12 4 1 7 5 6 12zM22 24c5-3 9-2 11 1-4 3-8 3-11-1z" fill="#C9D9A6" stroke="#C9D9A6" stroke-width="1"/></g>
<g transform="translate(135,775)"><circle r="82" fill="#1A2023" fill-opacity=".55" stroke="#fff" stroke-opacity=".45" stroke-width="2"/>
<path d="M-60 20 C-20 0 10 30 60 -10" stroke="#C7B48A" stroke-opacity=".7" stroke-width="3" fill="none"/>
<polygon points="0,-12 9,10 0,5 -9,10" fill="#fff"/></g>
<g transform="translate({W-280},690)"><rect width="40" height="40" rx="6" fill="#000" fill-opacity=".5" stroke="#fff" stroke-opacity=".7"/>'''
        if hud == "text":
            hudsvg = common + f'''
<text x="20" y="28" text-anchor="middle" font-family="Georgia,serif" font-size="22" fill="#fff">E</text>
<text x="54" y="28" font-family="Georgia,serif" font-size="23" fill="#fff">Hold E to talk</text></g>
<text x="120" y="93" font-family="Georgia,serif" font-size="26" fill="#fff">+1 Ginseng</text>
<text x="{W/2}" y="746" text-anchor="middle" font-family="Georgia,serif" font-size="26" font-weight="bold" fill="#E7C36A" stroke="#000" stroke-opacity=".45" stroke-width="3" paint-order="stroke">Arthur</text>
<text x="{W/2}" y="796" text-anchor="middle" font-family="Georgia,serif" font-size="31" fill="#fff" stroke="#000" stroke-opacity=".5" stroke-width="3" paint-order="stroke">We need to get these horses moving before the snow buries us.</text>'''
        else:
            hudsvg = common + f'''
<rect x="14" y="13" width="12" height="14" rx="2" fill="#fff"/>
<rect x="54" y="12" width="170" height="16" rx="8" fill="#fff" fill-opacity=".9"/></g>
<rect x="120" y="72" width="150" height="20" rx="10" fill="#fff" fill-opacity=".9"/>
<rect x="{W/2-50}" y="722" width="100" height="22" rx="11" fill="#E7C36A"/>
<rect x="{W/2-440}" y="772" width="880" height="26" rx="13" fill="#fff" fill-opacity=".95"/>'''
    return f'''<svg class="scene" viewBox="0 0 {W} 900" preserveAspectRatio="xMidYMid slice" xmlns="http://www.w3.org/2000/svg">
<defs>
<linearGradient id="sky{s}" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#58697F"/><stop offset=".55" stop-color="#B7C0CA"/><stop offset="1" stop-color="#E4E0D8"/></linearGradient>
<linearGradient id="snow{s}" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#F1F2F3"/><stop offset="1" stop-color="#BFC6CF"/></linearGradient>
<linearGradient id="vig{s}" x1="0" y1="0" x2="0" y2="1"><stop offset=".55" stop-color="#0E1216" stop-opacity="0"/><stop offset="1" stop-color="#0E1216" stop-opacity=".55"/></linearGradient>
</defs>
<rect width="{W}" height="900" fill="url(#sky{s})"/>
<g transform="translate({dx},0)">
<polygon points="0,470 180,340 330,420 520,250 700,400 860,300 1040,430 1220,280 1400,390 1600,310 1600,580 0,580" fill="#9DA8B5"/>
<polygon points="470,300 520,250 575,305 548,296 522,318 496,296" fill="#EEF1F4"/>
<polygon points="815,335 860,300 905,338 880,330 860,345 838,330" fill="#EEF1F4"/>
<polygon points="1170,325 1220,280 1270,322 1245,316 1222,336 1196,318" fill="#EEF1F4"/>
<polygon points="0,540 160,470 360,520 560,430 760,500 980,440 1200,505 1420,450 1600,500 1600,620 0,620" fill="#7F8B98"/>
<polygon points="0,600 260,520 560,575 840,505 1120,570 1380,515 1600,560 1600,900 0,900" fill="url(#snow{s})"/>
{trees()}
<path d="M520 900 C640 760 760 690 840 620" stroke="#A9B1BB" stroke-width="22" fill="none" stroke-opacity=".5"/>
</g>
<rect width="{W}" height="900" fill="url(#vig{s})"/>
{hudsvg}
</svg>'''

# zone boxes in scene coordinates (1600 x 900)
ZONES = {
    "pickup":   (44, 48, 284, 72, "Pickups"),
    "speaker":  (718, 712, 164, 46, "Speaker"),
    "dialogue": (300, 764, 1000, 46, "Dialogue"),
    "prompt":   (1306, 676, 266, 68, "Prompts"),
    "ignore":   (45, 685, 180, 180, "Ignore"),
}

def zone(kind, tag=True, extra="", handles=False, label=None):
    x, y, w, h, name = ZONES[kind]
    st = f"left:{x/16:.3f}%;top:{y/9:.3f}%;width:{w/16:.3f}%;height:{h/9:.3f}%"
    below = " below" if kind == "dialogue" else ""   # keeps the Dialogue tag clear of the Speaker box
    t = f'<span class="zone-tag{below}">{label or name}</span>' if tag else ""
    hd = '<i class="h tl"></i><i class="h tr"></i><i class="h bl"></i><i class="h br"></i>' if handles else ""
    return f'<div class="zone k-{kind}{(" " + extra) if extra else ""}" style="{st}">{t}{hd}</div>'

def mini_zones(prompt_off=False):
    """Small previews: colored outlines only, no tags."""
    return (zone("pickup", False, "thin") + zone("speaker", False, "thin") + zone("dialogue", False, "thin")
            + zone("prompt", False, "thin off" if prompt_off else "thin") + zone("ignore", False, "thin"))

KINDS = [("dialogue", "Dialogue"), ("speaker", "Speaker"), ("pickup", "Pickups"), ("prompt", "Prompts"), ("ignore", "Ignore")]
KNAME = dict(KINDS)

def chip(k, extra=""):
    return f'<span class="chip k-{k}{(" " + extra) if extra else ""}">{KNAME[k]}</span>'

def all_chips(prompt_off=False):
    return "".join(chip(k, "off" if (prompt_off and k == "prompt") else "") for k, _ in KINDS)

def kind_seg(on):
    return '<div class="seg sm kinds">' + "".join(
        f'<span class="k-{k}{" on" if k == on else ""}"><i class="dot"></i>{n}</span>' for k, n in KINDS) + "</div>"

files = {}

# ================= 01 welcome =================
css01 = """
.wel { flex:1; display:grid; grid-template-columns: 464px 1fr; gap: var(--sp-8); align-items:center; padding: 0 96px var(--topbar-h); }
.hero-logo { width: 56px; height: 56px; border-radius: var(--r-3); background: var(--ink); color: var(--on-ink); display:grid; place-items:center; margin-bottom: var(--sp-5); }
.hero-logo .i { width: var(--icon-lg); height: var(--icon-lg); }
.wel h1 { margin-bottom: var(--sp-3); }
.wel .body { margin-bottom: var(--sp-6); }
.wel-steps { list-style:none; display:flex; flex-direction:column; gap: var(--sp-2); margin-bottom: var(--sp-6); }
.wel-steps li { display:flex; align-items:center; gap: var(--sp-3); height: 40px; font-size: var(--fs-3); }
.preview { padding: var(--sp-3); }
.pill { position:absolute; right: var(--sp-4); top: var(--sp-4); display:flex; align-items:center; gap: var(--sp-2); height: var(--lead); padding: 0 var(--sp-3); border-radius: var(--r-pill); background: rgba(20,20,18,.84); color:#F2F0EB; font-size: var(--fs-2); border: 1px solid rgba(255,255,255,.14); }
.pill b { color:#fff; font-weight:600; }
.pill .dot.ok { background:#4CBF7A; }
.preview-cap { display:flex; align-items:center; gap: var(--sp-2); padding: var(--sp-3) var(--sp-1) var(--sp-1); }
"""
body01 = f'''<main class="content">
{topbar(None)}
<div class="wel">
 <div>
  <span class="hero-logo">{ic("wave", lg=True)}</span>
  <h1 class="t5">Hear your games, not just read them</h1>
  <p class="body">DialogReader watches the parts of the screen you choose and reads the text out loud as it appears. Set up a game once, then just play.</p>
  <ol class="wel-steps">
   <li><span class="lead num">1</span>Choose the game window</li>
   <li><span class="lead num">2</span>Mark where the text shows up</li>
   <li><span class="lead num">3</span>Pick a key and a voice</li>
   <li><span class="lead num">4</span>Choose when it reads</li>
  </ol>
  <div class="row" style="gap:var(--sp-3)"><button class="btn btn-primary btn-lg">Set up your first game{ic("arrow-r")}</button><button class="btn btn-ghost btn-lg">Not now</button></div>
  <p class="caption" style="margin-top:var(--sp-3)">Takes about 3 minutes. Everything runs on this PC.</p>
 </div>
 <div class="card preview">
  <div class="shot">{scene("w", "bars")}{mini_zones()}
   <div class="pill"><span class="dot ok"></span><b>Arthur:</b>We need to get these horses moving…</div></div>
  <div class="preview-cap">{all_chips()}<span class="spacer"></span><span class="caption">Each kind of text gets its own zone</span></div>
 </div>
</div>
</main>'''
files["01-welcome.html"] = page("Welcome", body01, css01)

# ================= 02 pick window =================
def mini(cls, shapes=""):
    return f'<div class="wthumb {cls}">{shapes}</div>'

css02 = FLOW_CSS + """
.grp { font-size: var(--fs-1); font-weight:600; color: var(--text-3); margin-bottom: var(--sp-3); line-height: var(--lh-1); }
.grid { display:grid; grid-template-columns: repeat(3, 1fr); gap: var(--sp-4); margin-bottom: var(--sp-5); }
.win { padding: var(--sp-2); }
.wthumb { aspect-ratio: 16/9; border-radius: var(--r-2); position:relative; overflow:hidden; }
.wthumb.g-sat { background: linear-gradient(180deg,#3A8FB7 0%,#F4C27A 55%,#6C8A3E 56%,#4E6A2C 100%); }
.wthumb.g-p4g { background: linear-gradient(180deg,#F7D23E 0%,#E8A317 60%,#2A2A2A 61%,#3A3A3A 100%); }
.wthumb .blk { position:absolute; border-radius: 2px; }
.win-meta { display:flex; align-items:center; gap: var(--sp-3); padding: var(--sp-3) var(--sp-2) var(--sp-1); }
.win-meta .col { min-width:0; }
.others { flex:1; min-height:0; display:flex; flex-direction:column; overflow:hidden; }
.search { width: 280px; }
"""
sat_shapes = '<i class="blk" style="left:16%;bottom:44%;width:14%;height:20%;background:#E8892F"></i><i class="blk" style="left:32%;bottom:44%;width:8%;height:30%;background:#C9CCD1"></i><i class="blk" style="left:62%;bottom:44%;width:20%;height:12%;background:#E8892F"></i>'
p4_shapes = '<i class="blk" style="left:10%;top:62%;width:80%;height:30%;background:#111;box-shadow:inset 0 0 0 2px #F7D23E"></i><i class="blk" style="left:14%;top:68%;width:40%;height:5%;background:#fff"></i>'
def other_row(name, exe):
    return f'<div class="set-row"><span class="tile">{ic("monitor")}</span><div class="lbl"><span>{name}</span><span class="caption">{exe}</span></div></div>'
body02 = f'''{flow_sidebar(1)}
<main class="content">
{topbar(["Set up a game", "Choose the game window"])}
<div class="page">
{phead("Which game do you want read aloud?", "Start the game, then pick its window. DialogReader only looks at the window you choose.",
       actions=f'<div class="input search">{ic("search")}<span class="ph">Search open windows</span></div><button class="btn">{ic("refresh")}Refresh</button>')}
<div class="grp">Games that are running</div>
<div class="grid">
 <div class="card choice win sel"><div class="shot wthumb">{scene("t", "bars")}</div>
  <div class="win-meta"><span class="thumb g-rdr"></span><div class="col"><span class="t3">Red Dead Redemption 2</span><span class="caption">RDR2.exe · 3840 × 2160 · borderless</span></div></div></div>
 <div class="card choice win">{mini("g-sat", sat_shapes)}
  <div class="win-meta"><span class="thumb g-sat"></span><div class="col"><span class="t3">Satisfactory</span><span class="caption">FactoryGame.exe · 2560 × 1440 · windowed</span></div></div></div>
 <div class="card choice win">{mini("g-p4g", p4_shapes)}
  <div class="win-meta"><span class="thumb g-p4g"></span><div class="col"><span class="t3">Persona 4 Golden</span><span class="caption">P4G.exe · 1920 × 1080 · borderless</span></div></div></div>
</div>
<section class="card others">
 <div class="card-head"><span class="t3">Other open windows</span><span class="caption">Don't see your game? Start it, then press Refresh.</span></div>
 {other_row("Steam", "steam.exe")}{other_row("Discord", "Discord.exe")}{other_row("Spotify", "Spotify.exe")}{other_row("File Explorer", "explorer.exe")}
</section>
</div>
{flow_foot(1, "Red Dead Redemption 2 selected")}
</main>'''
files["02-pick-window.html"] = page("Choose the game window", body02, css02)

# ================= 03 draw zones =================
css03 = FLOW_CSS + """
.stage { flex:1; }
.stage .shot { width: 1040px; margin: 0 auto; }
"""
body03 = f'''{flow_sidebar(2)}
<main class="content">
{topbar(["Set up a game", "Draw zones"])}
<div class="page">
{phead("Mark where the text shows up", "Pick a kind, then drag a box over that part of the picture. Draw one box for each place text appears.",
       actions=f'<button class="btn">{ic("camera")}Retake picture</button>')}
<section class="stage">
 <div class="toolbar"><div class="seg sm"><span>{ic("cursor")}Select</span><span class="on">{ic("square")}Draw zone</span></div>
  <span class="sep"></span><span class="caption">Kind</span>{kind_seg("prompt")}
  <span class="spacer"></span><span class="caption">Drag to draw a box</span><button class="btn btn-sm">{ic("zoom")}Fit</button></div>
 <div class="shot">{scene("d")}{zone("pickup")}{zone("speaker")}{zone("dialogue")}{zone("prompt", extra="sel", handles=True)}</div>
</section>
</div>
{flow_foot(2, "4 zones · Prompts can be turned off later")}
</main>'''
files["03-draw-zones.html"] = page("Draw zones", body03, css03)

# ================= 04 read key =================
css04 = FLOW_CSS + """
.keywrap { flex:1; min-height:0; display:grid; grid-template-columns: minmax(0,1fr) 368px; gap: var(--sp-5); }
.capture { display:flex; flex-direction:column; align-items:center; justify-content:center; text-align:center; padding: var(--sp-6); }
.bigkey { display:flex; align-items:center; gap: var(--sp-3); height: 88px; padding: 0 var(--sp-6); border-radius: var(--r-3); border: 1px solid var(--line-strong); border-bottom-width: 4px; background: var(--surface); box-shadow: var(--shadow-2); font-family: var(--font-display); font-size: var(--fs-5); font-weight: 600; margin: var(--sp-5) 0; }
.bigkey .i { width: var(--icon-lg); height: var(--icon-lg); color: var(--text-2); }
.does { margin-top: var(--sp-5); padding-top: var(--sp-5); border-top: 1px solid var(--line); width: 440px; display:flex; flex-direction:column; gap: var(--sp-1); }
.log4 { display:flex; flex-direction:column; }
.log4 .set-row .caption { width: 56px; font-variant-numeric: tabular-nums; }
.picks { margin-top:auto; padding: var(--sp-5); border-top: 1px solid var(--line); }
.picks .caption { font-weight:600; margin-bottom: var(--sp-3); display:block; }
.picks .wrap { display:flex; flex-wrap:wrap; gap: var(--sp-2); }
"""
body04 = f'''{flow_sidebar(3, hover=1)}
<main class="content">
{topbar(["Set up a game", "Choose a read key"])}
<div class="page">
{phead("Press the key you want to use", "Use a key or mouse button that the game does not need. Press it now and DialogReader will remember it.")}
<div class="keywrap">
 <div class="card capture">
  <span class="badge">{ic("check")}Got it</span>
  <div class="bigkey">{ic("mouse", lg=True)}Mouse 4</div>
  <span class="body">The back button on the side of your mouse.</span>
  <div class="does">
   <span class="small">In <b>Automatic</b> mode the key replays the last line.</span>
   <span class="small">In <b>On key press</b> mode it reads every zone.</span>
  </div>
  <span class="caption" style="margin-top:var(--sp-5)">Press another key at any time to change it.</span>
 </div>
 <section class="card log4">
  <div class="card-head"><span class="t3">Keys pressed just now</span><span class="caption">Newest first</span></div>
  <div class="set-row"><span class="caption">21:09:42</span><span class="kbd">{ic("mouse")}Mouse 4</span><span class="spacer"></span><span class="badge">{ic("check")}Chosen</span></div>
  <div class="set-row"><span class="caption">21:09:39</span><span class="kbd">F8</span></div>
  <div class="set-row"><span class="caption">21:09:37</span><span class="kbd">Caps Lock</span></div>
  <div class="set-row"><span class="caption">21:09:35</span><span class="kbd">{ic("mouse")}Mouse 5</span></div>
  <div class="set-row"><span class="caption">21:09:31</span><span class="kbd">Insert</span></div>
  <div class="picks"><span class="caption">Good choices</span><div class="wrap"><span class="kbd">{ic("mouse")}Mouse 4</span><span class="kbd">{ic("mouse")}Mouse 5</span><span class="kbd">F8</span><span class="kbd">Caps Lock</span><span class="kbd">Insert</span></div></div>
 </section>
</div>
</div>
{flow_foot(3, "Read key: Mouse 4")}
</main>'''
files["04-read-key.html"] = page("Choose a read key", body04, css04)

# ================= 05 voice =================
VOICES = [("Adam", "American English · Deep, calm"), ("Bella", "American English · Bright, warm"), ("Michael", "American English · Clear, neutral"),
          ("Nicole", "American English · Quiet, close"), ("Emma", "British English · Soft, warm"), ("George", "British English · Low, steady"),
          ("Lewis", "British English · Dry, even"), ("Isabella", "British English · Light, quick")]
css05 = FLOW_CSS + """
.vwrap { flex:1; min-height:0; display:grid; grid-template-columns: minmax(0,1fr) 368px; gap: var(--sp-5); }
.vlist { display:flex; flex-direction:column; min-height:0; }
.vrows { padding: 0 var(--sp-2) var(--sp-2); display:flex; flex-direction:column; gap: var(--sp-1); }
.voice { display:flex; align-items:center; gap: var(--sp-4); height: var(--row-set); padding: 0 var(--sp-4); border-radius: var(--r-2); }
.voice.sel { background: var(--sel-fill); }
.voice .col { width: 240px; }
.voice .progress { max-width: 320px; }
.try { display:flex; flex-direction:column; }
.ctl { display:flex; flex-direction:column; gap: var(--sp-2); padding: var(--sp-4) var(--sp-5); border-top: 1px solid var(--line); }
.ctl .row { gap: var(--sp-3); }
.val { width: 44px; text-align:right; font-size: var(--fs-2); font-weight:600; }
.quote { margin: 0 var(--sp-5) var(--sp-4); padding: var(--sp-3) var(--sp-4); border-radius: var(--r-2); background: var(--surface-2); font-size: var(--fs-3); }
.play-row { display:flex; gap: var(--sp-3); padding: 0 var(--sp-5) var(--sp-5); }
.try .set-row:last-child { margin-top:auto; }
"""
vrows = []
for i, (n, d) in enumerate(VOICES):
    if i == 0:
        vrows.append(f'<div class="voice sel"><div class="col"><span class="t3">Kokoro · {n}</span><span class="caption">{d}</span></div><span class="spacer"></span><div class="progress"><i style="width:38%"></i></div><span class="caption">0:02</span><button class="btn btn-sm btn-icon">{ic("stop")}</button></div>')
    else:
        vrows.append(f'<div class="voice"><div class="col"><span class="t3">Kokoro · {n}</span><span class="caption">{d}</span></div><span class="spacer"></span><button class="btn btn-sm btn-icon">{ic("play")}</button></div>')
body05 = f'''{flow_sidebar(4)}
<main class="content">
{topbar(["Set up a game", "Pick a voice"])}
<div class="page">
{phead("Pick a voice", "All voices run on this PC, so nothing is sent online. Press play to hear one.")}
<div class="vwrap">
 <section class="card vlist"><div class="card-head"><span class="t3">Voices</span><span class="caption">8 installed</span></div><div class="vrows">{"".join(vrows)}</div></section>
 <section class="card try">
  <div class="card-head"><span class="t3">Try it</span><span class="caption">Kokoro · Adam</span><span class="spacer"></span><span class="badge">{ic("volume")}Playing</span></div>
  <div class="quote">Arthur: We need to get these horses moving before the snow buries us.</div>
  <div class="play-row"><button class="btn">{ic("stop")}Stop</button><button class="btn btn-ghost">{ic("refresh")}Another line</button></div>
  <div class="ctl"><span class="small">Speed</span><div class="row"><div class="slider"><span class="fill" style="width:55%"></span><span class="knob" style="left:55%"></span></div><span class="val">1.1×</span></div></div>
  <div class="ctl"><span class="small">Volume</span><div class="row"><div class="slider"><span class="fill" style="width:80%"></span><span class="knob" style="left:80%"></span></div><span class="val">80%</span></div></div>
  <div class="set-row"><div class="lbl"><span>Use for new games</span><span class="caption">You can still pick a voice per game</span></div><span class="toggle"></span></div>
 </section>
</div>
</div>
{flow_foot(4, "Voice: Kokoro · Adam")}
</main>'''
files["05-voice.html"] = page("Pick a voice", body05, css05)

# ================= 06 mode =================
css06 = FLOW_CSS + """
.modes { display:grid; grid-template-columns: 1fr 1fr; gap: var(--sp-5); margin-bottom: var(--sp-5); }
.mode { padding: var(--sp-5); display:flex; flex-direction:column; gap: var(--sp-3); }
.mode .tile-lg { background: var(--surface-2); border-color: var(--surface-2); }
.mode.sel .tile-lg { background: var(--ink); border-color: var(--ink); color: var(--on-ink); }
.mode ul { list-style:none; display:flex; flex-direction:column; gap: var(--sp-2); }
.mode li { display:flex; align-items:center; gap: var(--sp-2); font-size: var(--fs-2); color: var(--text-2); }
.mode li .i { color: var(--text-3); }
.example { margin-top: var(--sp-2); border-radius: var(--r-2); background: var(--surface-2); padding: var(--sp-4); display:flex; flex-direction:column; gap: var(--sp-3); }
.ex-row { display:flex; align-items:center; gap: var(--sp-3); font-size: var(--fs-2); }
.ex-row .txt { flex:1; min-width:0; white-space:nowrap; overflow:hidden; text-overflow:ellipsis; }
.ex-row .i { color: var(--text-3); }
.sumhead { display:flex; align-items:baseline; gap: var(--sp-3); margin-bottom: var(--sp-3); }
.summary { flex:none; }
.summary .set-row:first-child { border-top: 0; }
.summary .lbl { flex-direction:row; align-items:center; gap: var(--sp-4); }
.summary .lbl .caption { width: 96px; }
.summary .lbl .t3 { font-weight: 500; }
"""
body06 = f'''{flow_sidebar(5)}
<main class="content">
{topbar(["Set up a game", "Choose a mode"])}
<div class="page">
{phead("When should DialogReader read?", "You can switch between these at any time, even while you play.")}
<div class="modes">
 <div class="card choice mode sel"><span class="tile-lg">{ic("zap")}</span><span class="t4">Automatic</span>
  <span class="body">Reads new text as soon as it shows up.</span>
  <ul><li>{ic("check")}Best for story games with lots of talking</li><li>{ic("check")}Mouse 4 replays the last line</li></ul>
  <div class="example">
   <div class="ex-row">{chip("speaker")}<span class="txt">Arthur</span><span class="caption">read at once</span></div>
   <div class="ex-row">{chip("dialogue")}<span class="txt">We need to get these horses moving before the snow…</span><span class="caption">read at once</span></div></div></div>
 <div class="card choice mode"><span class="tile-lg">{ic("keyboard")}</span><span class="t4">On key press</span>
  <span class="body">Reads every zone once when you press Mouse 4.</span>
  <ul><li>{ic("check")}Best for menus, books and item text</li><li>{ic("check")}Stays quiet until you ask</li></ul>
  <div class="example">
   <div class="ex-row"><span class="kbd">{ic("mouse")}Mouse 4</span>{ic("arrow-r")}{chip("speaker")}{chip("dialogue")}{chip("pickup")}</div>
   <div class="ex-row"><span class="txt muted">Text outside your zones is never read.</span></div></div></div>
</div>
<div class="sumhead"><span class="t3">Your setup</span><span class="caption">You can change any of this later on the game's page.</span></div>
<div class="card summary">
 <div class="set-row"><span class="thumb g-rdr"></span><div class="lbl"><span class="caption">Game</span><span class="t3">Red Dead Redemption 2</span></div><button class="btn btn-sm btn-ghost">Change</button></div><div class="set-row"><span class="tile">{ic("scan")}</span><div class="lbl"><span class="caption">Zones</span><span class="t3">4 zones · Dialogue, Speaker, Pickups, Prompts</span></div><button class="btn btn-sm btn-ghost">Change</button></div><div class="set-row"><span class="tile">{ic("mouse")}</span><div class="lbl"><span class="caption">Read key</span><span class="t3">Mouse 4</span></div><button class="btn btn-sm btn-ghost">Change</button></div><div class="set-row"><span class="tile">{ic("volume")}</span><div class="lbl"><span class="caption">Voice</span><span class="t3">Kokoro · Adam · 1.1×</span></div><button class="btn btn-sm btn-ghost">Change</button></div>
</div>
</div>
{flow_foot(5, "Reading starts when you finish", primary="Finish and start reading", primary_icon="check")}
</main>'''
files["06-mode.html"] = page("Choose a mode", body06, css06)

# ================= game header + tabs (10, 11) =================
def game_head(tab, actions):
    tabs = "".join(f'<a class="{"on" if t == tab else ""}">{t}</a>' for t in ["Overview", "Zones", "History"])
    meta = '<span class="dot ok"></span><span class="okt">Reading</span><span>·</span><span>RDR2.exe</span><span>·</span><span>3840 × 2160</span>'
    return phead("Red Dead Redemption 2", meta=meta, lead='<span class="thumb lg g-rdr"></span>', actions=actions) + f'<nav class="tabs">{tabs}</nav>'

# ================= 10 main =================
LOG = [
    ("21:14:08", "dialogue", "Arthur", "We need to get these horses moving before the snow buries us.", "now"),
    ("21:14:02", "dialogue", "Dutch", "We'll find shelter up ahead. Keep together, all of you.", ""),
    ("21:13:51", "pickup", None, "+1 Ginseng", ""),
    ("21:13:40", "prompt", None, "Hold E to talk", "skip"),
    ("21:13:22", "dialogue", "John", "I can't feel my hands, Arthur.", ""),
    ("21:12:58", "pickup", None, "+1 Valerian Root", ""),
    ("21:12:41", "dialogue", "Javier", "There's a mining town up the pass. Colter, they call it.", ""),
    ("21:12:20", "dialogue", "Arthur", "Keep your eyes open. Those Pinkertons ain't far behind.", ""),
    ("21:12:03", "pickup", None, "+1 Rabbit Pelt", ""),
]
css10 = """
.grid10 { display:grid; grid-template-columns: minmax(0,1fr) 512px; gap: var(--sp-5); align-items:stretch; }
.log { position:relative; min-height:0; }
.log-in { position:absolute; inset:0; display:flex; flex-direction:column; overflow:hidden; border-radius: inherit; }
.lrow { display:grid; grid-template-columns: 56px 88px 1fr 88px; gap: var(--sp-3); align-items:start; padding: var(--sp-3) var(--sp-5); border-top: 1px solid var(--line); flex:none; }
.lrow > .caption { line-height: var(--chip-h); font-variant-numeric: tabular-nums; }
.lrow .txt { font-size: var(--fs-3); line-height: var(--chip-h); }
.lrow .who { font-weight: 600; color: var(--text-2); }
.lrow .end { display:flex; justify-content:flex-end; }
.lrow .end .btn { margin-top: -2px; }
.lrow.now { background: var(--sel-fill); }
.lrow.skip .txt { color: var(--text-3); }
.right { display:flex; flex-direction:column; gap: var(--sp-5); }
.zprev { padding: 0 var(--sp-5); }
.legend { display:flex; justify-content:space-between; padding: var(--sp-4) var(--sp-5) var(--sp-5); }
"""
lrows = []
for t, k, who, txt, st in LOG:
    w = f'<span class="who">{who}:</span> ' if who else ""
    if st == "skip":
        txt_html = f'<div class="txt">{txt}<span class="caption" style="display:block">Skipped · prompts are turned off</span></div>'
        end = '<span class="end"></span>'
    elif st == "now":
        txt_html = f'<div class="txt">{w}{txt}</div>'
        end = '<span class="end"><span class="badge ink">Speaking</span></span>'
    else:
        txt_html = f'<div class="txt">{w}{txt}</div>'
        end = f'<span class="end"><button class="btn btn-sm btn-icon btn-ghost">{ic("replay")}</button></span>'
    lrows.append(f'<div class="lrow {st}"><span class="caption">{t}</span>{chip(k)}{txt_html}{end}</div>')

def main10():
    return f'''{main_sidebar("rdr")}
<main class="content">
{topbar(["Games", "Red Dead Redemption 2"])}
<div class="page">
{game_head("Overview", f'<button class="btn">{ic("pause")}Pause reading</button>')}
<div class="grid10">
 <section class="card log"><div class="log-in">
  <div class="card-head"><span class="t3">Recently read</span><span class="caption">Press replay to hear a line again</span><span class="spacer"></span><button class="btn btn-sm btn-ghost">See all{ic("chev-r")}</button></div>
  {"".join(lrows)}
 </div></section>
 <div class="right">
  <section class="card">
   <div class="card-head"><span class="t3">How it reads</span><span class="caption">For this game</span></div>
   <div class="set-row"><span class="tile">{ic("zap")}</span><div class="lbl"><span>Mode</span></div><div class="seg sm"><span class="on">Automatic</span><span>On key press</span></div></div>
   <div class="set-row"><span class="tile">{ic("volume")}</span><div class="lbl"><span>Voice</span><span class="caption">Kokoro · Adam · 1.1×</span></div><button class="btn btn-sm">{ic("play")}Test</button></div>
   <div class="set-row"><span class="tile">{ic("keyboard")}</span><div class="lbl"><span>Read key</span><span class="caption">Replays the last line</span></div><button class="kbd btnish">{ic("mouse")}Mouse 4<span class="edit">{ic("pencil")}</span></button></div>
   <div class="set-row"><span class="tile">{ic("layers")}</span><div class="lbl"><span>In-game overlay</span><span class="caption">Top right · shows the last line</span></div><span class="toggle on"></span></div>
  </section>
  <section class="card">
   <div class="card-head"><span class="t3">Zones</span><span class="caption">5 zones · prompts off</span><span class="spacer"></span><button class="btn btn-sm">{ic("scan")}Edit zones</button></div>
   <div class="zprev"><div class="shot">{scene("m", "bars")}{mini_zones(prompt_off=True)}</div></div>
   <div class="legend">{all_chips(prompt_off=True)}</div>
  </section>
 </div>
</div>
</div>
</main>'''
files["10-main.html"] = page("Red Dead Redemption 2", main10(), css10)
files["10-main-dark.html"] = page("Red Dead Redemption 2 (dark)", main10(), css10, dark=True)

# ================= 11 zones =================
css11 = """
.ed { flex:1; min-height:0; display:grid; grid-template-columns: minmax(0,1fr) 360px; gap: var(--sp-5); }
.left { display:flex; flex-direction:column; gap: var(--sp-5); min-height:0; }
.stage .shot { width: 680px; margin: 0 auto; }
.test { flex:none; }
.trow { display:grid; grid-template-columns: 88px 1fr 160px; gap: var(--sp-3); align-items:center; padding: 0 var(--sp-5); height: var(--row-compact); border-top: 1px solid var(--line); font-size: var(--fs-2); }
.trow .txt { white-space:nowrap; overflow:hidden; text-overflow:ellipsis; }
.st { display:flex; align-items:center; gap: var(--sp-2); font-size: var(--fs-1); color: var(--text-3); }
.st.read { color: var(--text-2); font-weight: 600; }
.st .i { color: inherit; }
.panel { display:flex; flex-direction:column; min-height:0; }
.zitem .lbl { flex-direction:row; align-items:center; gap: var(--sp-3); }
.set-row.zitem .lbl > .chip { font-size: var(--fs-1); font-weight: 600; line-height: var(--lh-1); }
.zitem.sel { background: var(--sel-fill); }
.insp { padding: var(--sp-4) var(--sp-5); border-top: 1px solid var(--line); display:flex; flex-direction:column; gap: var(--sp-3); background: var(--sel-fill); }
.insp .row { gap: var(--sp-3); }
.insp .lab { width: 64px; font-size: var(--fs-2); color: var(--text-2); flex:none; }
.insp .input { flex:1; }
.insp .val { font-size: var(--fs-2); color: var(--text-2); }
"""
ZL = [("dialogue", "Bottom center", True, True), ("speaker", "Above the subtitles", True, False),
      ("pickup", "Top left", True, False), ("prompt", "Right side · turned off", False, False),
      ("ignore", "Minimap · never read", True, False)]
zitems = []
for k, where, on, sel in ZL:
    zitems.append(f'<div class="set-row zitem{" sel" if sel else ""}"><div class="lbl">{chip(k, "" if on else "off")}<span class="caption">{where}</span></div><span class="toggle{" on" if on else ""}"></span></div>')
    if sel:
        zitems.append(f'''<div class="insp">
<div class="row"><span class="lab">Kind</span><div class="input select k-dialogue"><i class="dot kind"></i>Dialogue{ic("chev-d", "chev")}</div></div>
<div class="row"><span class="lab">Size</span><span class="val">Bottom center · 1000 × 46 px</span></div>
<div class="row"><button class="btn btn-sm btn-ghost btn-danger btn-flush">{ic("trash")}Delete zone</button></div>
</div>''')
def st(kind):
    if kind == "read":
        return f'<span class="st read">{ic("check")}Read</span>'
    if kind == "skip":
        return f'<span class="st">{ic("skip")}Skipped · turned off</span>'
    return f'<span class="st">{ic("minus")}Never read</span>'
body11 = f'''{main_sidebar("rdr")}
<main class="content">
{topbar(["Games", "Red Dead Redemption 2", "Zones"])}
<div class="page">
{game_head("Zones", f'<button class="btn">{ic("camera")}Retake picture</button><button class="btn btn-primary">{ic("play")}Test read</button>')}
<div class="ed">
 <div class="left">
  <section class="stage">
   <div class="toolbar"><div class="seg sm"><span class="on">{ic("cursor")}Select</span><span>{ic("square")}Draw zone</span></div>
    <span class="spacer"></span><span class="caption">Drag a corner to resize</span><button class="btn btn-sm">{ic("zoom")}Fit</button></div>
   <div class="shot">{scene("z")}{zone("pickup")}{zone("speaker")}{zone("dialogue", extra="sel", handles=True)}{zone("prompt", extra="off", label="Prompts · off")}{zone("ignore")}</div>
  </section>
  <section class="card test">
   <div class="card-head"><span class="t3">Last test read</span><span class="caption">21:10:03 · from this picture</span></div>
   <div class="trow">{chip("speaker")}<span class="txt">Arthur</span>{st("read")}</div>
   <div class="trow">{chip("dialogue")}<span class="txt">We need to get these horses moving before the snow buries us.</span>{st("read")}</div>
   <div class="trow">{chip("pickup")}<span class="txt">+1 Ginseng</span>{st("read")}</div>
   <div class="trow">{chip("prompt")}<span class="txt muted">Hold E to talk</span>{st("skip")}</div>
   <div class="trow">{chip("ignore")}<span class="txt muted">Minimap</span>{st("never")}</div>
  </section>
 </div>
 <section class="card panel">
  <div class="card-head"><span class="t3">Zones</span><span class="caption">5 zones</span><span class="spacer"></span><button class="btn btn-sm">{ic("plus")}Add zone</button></div>
  {"".join(zitems)}
 </section>
</div>
</div>
</main>'''
files["11-zones.html"] = page("Zones", body11, css11)

# ================= 12 settings =================
css12 = """
.sgrid { display:grid; grid-template-columns: 1fr 1fr; gap: var(--sp-5); align-items:start; }
.sgrid > .col { gap: var(--sp-5); }
.set-row .slider { flex: none; width: 160px; }
.val { width: 40px; text-align:right; font-size: var(--fs-2); font-weight:600; }
.combo { display:flex; gap: var(--sp-1); align-items:center; }
.combo .plus { font-size: var(--fs-1); color: var(--text-3); }
.set-row .input.select { width: 200px; }
"""
def srow(icon, label, cap, ctl):
    c = f'<span class="caption">{cap}</span>' if cap else ""
    return f'<div class="set-row"><span class="tile">{ic(icon)}</span><div class="lbl"><span>{label}</span>{c}</div>{ctl}</div>'
def card(title, cap, rows):
    return f'<section class="card"><div class="card-head"><span class="t3">{title}</span><span class="caption">{cap}</span></div>{rows}</section>'
T_ON, T_OFF = '<span class="toggle on"></span>', '<span class="toggle"></span>'
def keys(*ks):
    return '<div class="combo">' + '<span class="plus">+</span>'.join(f'<span class="kbd">{k}</span>' for k in ks) + "</div>"
def segsm(*opts, on=0):
    oc = ' class="on"'
    return '<div class="seg sm">' + "".join(f'<span{oc if i == on else ""}>{o}</span>' for i, o in enumerate(opts)) + "</div>"
body12 = f'''{main_sidebar("settings")}
<main class="content">
{topbar(["Settings"])}
<div class="page">
{phead("Settings", "These apply to every game. Each game keeps its own zones, mode, key and voice.", lead=f'<span class="tile-lg">{ic("sliders")}</span>')}
<div class="sgrid">
 <div class="col">
  {card("Startup", "When DialogReader opens", srow("power", "Start with Windows", "Opens quietly in the tray", T_ON) + srow("gamepad", "Start reading when a game opens", "Only for games you have set up", T_ON) + srow("tray", "Close to the tray", "Keeps reading after you close this window", T_ON))}
  {card("In-game overlay", "The small pill over your game", srow("layers", "Position", None, segsm("Top right", "Bottom center", "Hidden")) + srow("info", "Show the last line read", "Next to the status dot", T_ON) + srow("text", "Text size", None, segsm("Small", "Medium", "Large", on=1)))}
  {card("Updates", "From GitHub releases", srow("download", "DialogReader 0.4.0", "Up to date · checked today at 09:12", '<button class="btn btn-sm">Check now</button>') + srow("refresh", "Install updates automatically", "Only when no game is running", T_ON) + srow("info", "Update channel", "Beta gets new features first", segsm("Stable", "Beta")))}
 </div>
 <div class="col">
  {card("Hotkeys", "Work while you play", srow("mouse", "Read now", "Set on each game's page", f'<span class="caption">RDR2</span><span class="kbd">{ic("mouse")}Mouse 4</span>') + srow("pause", "Pause or resume reading", None, keys("Ctrl", "Shift", "P")) + srow("replay", "Replay the last line", None, keys("Ctrl", "Shift", "R")) + srow("skip", "Skip the current line", None, keys("Ctrl", "Shift", "S")))}
  {card("Voices", "Used by new games", srow("volume", "Voice for new games", None, '<div class="input select">Kokoro · Adam' + ic("chev-d", "chev") + "</div>") + srow("gauge", "Speed", None, '<div class="slider"><span class="fill" style="width:55%"></span><span class="knob" style="left:55%"></span></div><span class="val">1.1×</span>') + srow("download", "Installed voices", "8 voices · 320 MB on disk", '<button class="btn btn-sm">Manage</button>'))}
  {card("Sound", "While a line is spoken", srow("music", "Pause music and videos", "Spotify, browsers and other players", T_ON) + srow("volume", "Lower the game volume", "Turns the game down by half", T_OFF))}
 </div>
</div>
</div>
</main>'''
files["12-settings.html"] = page("Settings", body12, css12)

# ================= 13 overlay =================
css13 = """
.game { position:absolute; inset:0; }
.game svg { width:100%; height:100%; display:block; }
.ovs { position:absolute; top: var(--sp-5); right: var(--sp-5); display:flex; flex-direction:column; align-items:flex-end; gap: var(--sp-2); }
.ov { display:flex; align-items:center; gap: var(--sp-2); height: var(--lead); padding: 0 var(--sp-3); border-radius: var(--r-pill); background: rgba(20,20,18,.84); border: 1px solid rgba(255,255,255,.14); color: rgba(242,240,235,.82); font-size: var(--fs-2); box-shadow: 0 4px 12px rgba(0,0,0,.25); white-space: nowrap; }
.ov .dot.ok { background: #4CBF7A; }
.ov .dot.off { background: #8F8B83; }
.ov .state { font-weight: 600; color: #fff; }
.ov .sep { color: rgba(242,240,235,.4); }
.ov .name { font-weight: 600; color: #fff; }
.ann { position:absolute; top: 112px; right: var(--sp-5); font-size: var(--fs-1); color: rgba(255,255,255,.7); }
"""
body13 = f'''<div class="game">{scene("o", W=1440)}</div>
<div class="ovs">
 <div class="ov"><span class="dot ok"></span><span class="state">Speaking</span><span class="sep">·</span><span class="name">Arthur:</span><span>We need to get these horses moving before the…</span></div>
 <div class="ov"><span class="dot off"></span><span class="state">Paused</span><span class="sep">·</span><span>Ctrl + Shift + P to resume</span></div>
</div>'''
files["13-overlay.html"] = page("In-game overlay", body13, css13)

# ================= index =================
SCREENS = [("01-welcome", "Welcome"), ("02-pick-window", "1 · Choose the game window"), ("03-draw-zones", "2 · Draw zones"),
           ("04-read-key", "3 · Choose a read key"), ("05-voice", "4 · Pick a voice"), ("06-mode", "5 · Choose a mode"),
           ("10-main", "Main window"), ("10-main-dark", "Main window, dark"), ("11-zones", "Zone editor"),
           ("12-settings", "Settings"), ("13-overlay", "In-game overlay")]
cards = "".join(f'<a class="card sheet" href="{f}.html"><img src="shots/{f}.png" alt="{n}"><div class="sm"><span class="t3">{n}</span><span class="caption">{f}.html</span></div></a>' for f, n in SCREENS)
idx = f'''<!doctype html>
<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Porcelain mockups</title>
<link rel="stylesheet" href="tokens.css">
<style>
body {{ padding: var(--sp-7) var(--sp-7) var(--sp-8); }}
header {{ max-width: 1360px; margin: 0 auto var(--sp-6); }}
header .body {{ margin-top: var(--sp-2); max-width: 720px; }}
.kinds {{ display:flex; gap: var(--sp-2); margin-top: var(--sp-4); flex-wrap:wrap; }}
.sheet-grid {{ max-width: 1360px; margin: 0 auto; display:grid; grid-template-columns: repeat(3, 1fr); gap: var(--sp-5); }}
.sheet {{ overflow:hidden; display:block; }}
.sheet img {{ width:100%; display:block; aspect-ratio: 16/10; object-fit: cover; border-bottom: 1px solid var(--line); }}
.sm {{ display:flex; align-items:center; justify-content:space-between; padding: var(--sp-3) var(--sp-4); }}
@media (max-width: 900px) {{ body {{ padding: var(--sp-5) var(--sp-4); }} .sheet-grid {{ grid-template-columns: 1fr; }} }}
</style></head>
<body>
<header><div class="caption" style="font-weight:600">DialogReader · Direction B · revision 2</div><h1 class="t5">Porcelain</h1>
<p class="body">A calm light UI: off-white surfaces, one ink accent, the same sidebar, header and tabs on every screen. Color is kept for the five zone kinds, plus green for "Reading".</p>
<div class="kinds">{all_chips()}</div></header>
<main class="sheet-grid">{cards}</main>
</body></html>
'''
files["index.html"] = idx

for name, html in files.items():
    with open(os.path.join(OUT, name), "w", encoding="utf-8", newline="\n") as f:
        f.write(html)
print("wrote", len(files))
