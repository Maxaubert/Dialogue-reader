"""Builds the Graphite mockup HTML pages and index from shared fragments.
Run: python _build.py .   (tokens.css is edited by hand; this script never overwrites it)"""
import os, sys, random

OUT = sys.argv[1]
SCR = os.path.dirname(os.path.abspath(__file__))

ICONS = {
    "mark": "M4 10v4 M8 7v10 M12 4v16 M16 8v8 M20 11v2",
    "min": "M6 12h12", "max": "M6.5 6.5h11v11h-11z", "close": "M6.5 6.5l11 11 M17.5 6.5l-11 11",
    "plus": "M12 5v14 M5 12h14",
    "search": "M11 17.5a6.5 6.5 0 1 0 0-13 6.5 6.5 0 0 0 0 13z M20 20l-4.4-4.4",
    "refresh": "M19.5 11A7.5 7.5 0 0 0 6.2 7 M4.5 4.5V8H8 M4.5 13A7.5 7.5 0 0 0 17.8 17 M19.5 19.5V16H16",
    "info": "M12 20.5a8.5 8.5 0 1 0 0-17 8.5 8.5 0 0 0 0 17z M12 11v5 M12 8h.01",
    "back": "M19 12H5 M10.5 6.5 5 12l5.5 5.5", "next": "M5 12h14 M13.5 6.5 19 12l-5.5 5.5",
    "chev": "M10 7l5 5-5 5", "down": "M7 10l5 5 5-5",
    "monitor": "M3.5 5h17v11.5h-17z M8.5 20h7 M12 16.5V20",
    "frame": "M4 8.5V4h4.5 M15.5 4H20v4.5 M20 15.5V20h-4.5 M8.5 20H4v-4.5",
    "speaker": "M10.5 5.5 6.5 9H3.5v6h3l4 3.5z M15 9a4.5 4.5 0 0 1 0 6 M18 6.5a8 8 0 0 1 0 11",
    "pad": "M7.5 6h9a5 5 0 0 1 5 5v2.5a3.5 3.5 0 0 1-6.3 2.1L14 14h-4l-1.2 1.6a3.5 3.5 0 0 1-6.3-2.1V11a5 5 0 0 1 5-5z M7 10v3 M5.5 11.5h3 M15.5 11h.01 M17.5 12.5h.01",
    "pause": "M8.5 5v14 M15.5 5v14", "play": "M8 5.5v13l10.5-6.5z", "stop": "M7 7h10v10H7z",
    "replay": "M4.5 12a7.5 7.5 0 1 0 2.2-5.3 M4.5 4.5v3.8h3.8",
    "check": "M5 12.5l4.5 4.5L19 7.5",
    "undo": "M9 14 4.5 9.5 9 5 M4.5 9.5h10a5 5 0 0 1 0 10H12",
    "camera": "M3.5 8h4l1.8-2.5h5.4L16.5 8h4v11h-17z M12 16.2a3 3 0 1 0 0-6 3 3 0 0 0 0 6z",
    "cursor": "M5.5 4l13 6-5.6 1.9L11 17.5z",
    "bolt": "M13 3 5.5 13.5h6L10.5 21 18.5 10h-6z",
    "keyboard": "M3.5 6.5h17v11h-17z M7 10h.01 M10.5 10h.01 M14 10h.01 M17 10h.01 M8 14h8",
    "eye": "M2.5 12S6 5.5 12 5.5 21.5 12 21.5 12 18 18.5 12 18.5 2.5 12 2.5 12z M12 14.8a2.8 2.8 0 1 0 0-5.6 2.8 2.8 0 0 0 0 5.6z",
    "eyeoff": "M4 4l16 16 M10 5.7A9 9 0 0 1 12 5.5c6 0 9.5 6.5 9.5 6.5a16 16 0 0 1-2.6 3.4 M6.5 7.3C4 9 2.5 12 2.5 12S6 18.5 12 18.5a9 9 0 0 0 4-1",
    "trash": "M4.5 7h15 M9.5 7V4.5h5V7 M6.5 7l.8 12.5h9.4L17.5 7",
    "settings": "M4 7h9 M17 7h3 M15 5v4 M4 17h3 M11 17h9 M9 15v4",
    "skip": "M12 20.5a8.5 8.5 0 1 0 0-17 8.5 8.5 0 0 0 0 17z M6 6l12 12",
    "download": "M12 4v11 M7.5 10.5 12 15l4.5-4.5 M5 19.5h14",
    "lock": "M6 10.5h12v9H6z M8.5 10.5V8a3.5 3.5 0 0 1 7 0v2.5",
}

def ic(name, cls=""):
    return f'<svg class="i {cls}" viewBox="0 0 24 24"><path d="{ICONS[name]}"/></svg>'

def wave(n, h, played, seed=3, w=None):
    rnd = random.Random(seed)
    out = []
    for k in range(n):
        v = 0.25 + 0.75 * abs((rnd.random() + rnd.random()) / 2 - 0.15)
        hh = max(4, round(h * min(1, v)))
        cls = "" if k < played else ' class="rest"'
        out.append(f'<i{cls} style="height:{hh}px"></i>')
    return f'<div class="wave" style="height:{h}px">{"".join(out)}</div>'

# ---------------- Scene (RDR2) ----------------
ZONES = {
    "pickups":  ("k-pickups",  "Pickups",  "left:2.5%;top:4.2%;width:17%;height:7.5%", True),
    "ignore":   ("k-ignore",   "Ignore",   "left:84%;top:3.5%;width:13.5%;height:11%", True),
    "speaker":  ("k-speaker",  "Speaker",  "left:43%;top:78.8%;width:14%;height:4.4%", False),
    "dialogue": ("k-dialogue", "Dialogue", "left:19%;top:83.8%;width:62%;height:7.4%", False),
    "prompts":  ("k-prompts",  "Prompts",  "left:83.5%;top:88.8%;width:14%;height:7.4%", False),
}

def zone(key, sel=False, off=False):
    k, label, pos, below = ZONES[key]
    if off: label += " · off"
    cls = f"zone {k}" + (" sel" if sel else "") + (" off" if off else "")
    handles = "".join(f'<span class="hd {c}"></span>' for c in ("tl", "tr", "bl", "br")) if sel else ""
    return f'<div class="{cls}" style="{pos}"><span class="zl{" below" if below else ""}">{label}</span>{handles}</div>'

def scene(zones=None, money=True, extra="", style=""):
    z = ""
    if zones is not None:
        z = "".join(zone(k, sel=(k == "dialogue" and zones.get("sel")), off=(k == "prompts" and zones.get("prompts_off")))
                    for k in ("pickups", "ignore", "speaker", "dialogue", "prompts"))
    m = '<div class="hud h-money">$142.50<br>10:42 PM</div>' if money else ""
    return (f'<div class="scene rdr" style="{style}"><div class="sun"></div><div class="m1"></div><div class="m2"></div><div class="snow"></div>'
            '<div class="pine t1"></div><div class="pine t2"></div><div class="pine t3"></div><div class="pine t4"></div>'
            '<div class="hud h-pick"><b></b>+1 Ginseng</div>' + m +
            '<div class="hud h-spk">Arthur</div>'
            '<div class="hud h-sub">We need to get these horses moving before the snow buries us.</div>'
            '<div class="hud h-prompt"><i>E</i>Hold to talk</div><div class="h-map"></div>' + z + extra + '</div>')

def ov_pill(text_who="Arthur:", text="We need to get these horses moving before the snow…", style=""):
    return (f'<div class="ov-pill" style="{style}">{wave(5, 14, 5, seed=11)}'
            f'<span><span class="who">{text_who}</span> {text}</span></div>')

# ---------------- Frame ----------------
def page(title, inner, extra_css=""):
    return f'''<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=1440">
<title>{title}</title>
<link rel="stylesheet" href="tokens.css">
<style>{extra_css}</style>
</head>
<body>
{inner}
</body>
</html>
'''

TITLEBAR = (f'<div class="titlebar"><div class="tb-drag"></div>'
            f'<div class="tb-ctrl">{ic("min")}</div><div class="tb-ctrl">{ic("max")}</div><div class="tb-ctrl">{ic("close")}</div></div>')
BRAND = f'<div class="brand"><div class="mark">{ic("mark")}</div>DialogReader</div>'

def window(sidebar, main):
    return f'<div class="window">{TITLEBAR}<div class="body">{sidebar}{main}</div></div>'

GAMES = [("gi-rdr", "Red Dead Redemption 2"), ("gi-sat", "Satisfactory"), ("gi-p4g", "Persona 4 Golden"),
         ("gi-co", "Clair Obscur: Expedition 33"), ("gi-kcd", "Kingdom Come: Deliverance II")]

def main_sidebar(active):
    items = []
    for i, (gi, name) in enumerate(GAMES):
        a = " active" if (active == "rdr" and i == 0) else ""
        dot = '<span class="status-dot"></span>' if i == 0 else ""
        items.append(f'<div class="nav-item{a}"><span class="game-icon {gi}"></span><span class="nav-name">{name}</span>{dot}</div>')
    s_active = " active" if active == "settings" else ""
    return (f'<aside class="sidebar">{BRAND}<div class="nav-label">Games</div>{"".join(items)}'
            f'<div class="nav-item">{ic("plus")}<span class="nav-name">Add a game</span></div>'
            f'<div class="spacer"></div><div class="nav-sep"></div>'
            f'<div class="nav-item{s_active}">{ic("settings")}<span class="nav-name">Settings</span></div></aside>')

STEPS = [("Game window", "Red Dead Redemption 2"), ("Text zones", "5 zones"), ("Read key", "Mouse 4"),
         ("Voice", "Kokoro · Adam"), ("Reading mode", "Automatic")]

def rail(current):
    out = []
    for i, (name, sub) in enumerate(STEPS, 1):
        if i < current:
            out.append(f'<div class="nav-item step done"><span class="n">{ic("check")}</span><span class="step-text"><span>{name}</span><span class="step-sub">{sub}</span></span></div>')
        elif i == current:
            out.append(f'<div class="nav-item step active"><span class="n">{i}</span><span class="step-text">{name}</span></div>')
        else:
            out.append(f'<div class="nav-item step"><span class="n">{i}</span><span class="step-text">{name}</span></div>')
    return (f'<aside class="sidebar">{BRAND}<div class="nav-label">Set up</div><div class="v" style="gap:4px">{"".join(out)}</div>'
            f'<div class="sidebar-foot">You can change any of this later.</div></aside>')

def ob(step, eyebrow, title, lead, body, note="", primary="Continue", back=True):
    b = f'<span class="btn ghost lg flush-l">{ic("back")}Back</span>' if back else ""
    n = f'<span class="faint xs">{note}</span>' if note else ""
    main = (f'<main class="ob"><div class="ob-main"><div class="ob-eyebrow">{eyebrow}</div><h1 class="ob-title">{title}</h1>'
            f'<p class="ob-lead">{lead}</p><div class="ob-body">{body}</div></div>'
            f'<div class="ob-foot">{b}{n}<span class="spacer"></span><span class="btn primary lg">{primary}{ic("next")}</span></div></main>')
    return window(rail(step), main)

def game_head(tab):
    head = (f'<div class="page-head"><span class="game-icon gi-rdr lg"></span>'
            f'<div><div class="h" style="gap:12px"><span class="page-title">Red Dead Redemption 2</span><span class="pill ok"><span class="dot"></span>Reading</span></div>'
            f'<div class="faint xs">RDR2.exe · 3840 × 2160 · Borderless</div></div>'
            f'<div class="actions"><span class="btn">{ic("replay")}Repeat last line</span><span class="btn">{ic("pause")}Pause reading</span></div></div>')
    tabs = [("Overview", ""), ("Zones", "5"), ("History", "128")]
    t = "".join(f'<div class="tab{" active" if n == tab else ""}">{n}{f"<span class=count>{c}</span>" if c else ""}</div>' for n, c in tabs)
    return head + f'<div class="tabs">{t}</div>'

def kinds_picker():
    return ('<div class="seg kinds"><span class="k-dialogue on">Dialogue</span><span class="k-speaker">Speaker</span>'
            '<span class="k-pickups">Pickups</span><span class="k-prompts">Prompts</span><span class="k-ignore">Ignore</span></div>')

KIND_HELP = [("k-dialogue", "Dialogue", "What characters say. Read in full."),
             ("k-speaker", "Speaker", "Who is talking. Read before the line."),
             ("k-pickups", "Pickups", "Items and short notifications."),
             ("k-prompts", "Prompts", "Button hints. You can turn these off."),
             ("k-ignore", "Ignore", "Never read anything here.")]

def kinds_card(style=""):
    rows = "".join(f'<div class="v" style="flex:1;justify-content:center;gap:4px;padding:0 16px;border-top:1px solid var(--line-1)">'
                   f'<span><span class="chip {k}">{n}</span></span><span class="muted xs">{d}</span></div>' for k, n, d in KIND_HELP)
    return (f'<div class="card v" style="{style}"><div class="card-head"><span class="card-title">Zone kinds</span></div>'
            f'<div class="v grow" style="margin-top:-1px">{rows}</div></div>')

pages = {}

# ---------------- 01 Welcome ----------------
feats = [("monitor", "Pick your game", "Choose the window it runs in."),
         ("frame", "Mark where text appears", "Draw boxes over a screenshot."),
         ("speaker", "Choose a voice and a key", "Test them before you play."),
         ("pad", "Play", "Reads from the tray while you play.")]
feat_html = "".join(f'<div class="opt h" style="flex:1;padding:0 20px"><div class="h" style="align-items:flex-start;gap:16px">'
                    f'<span class="tile">{ic(i)}</span><div><div class="strong">{t}</div><div class="muted xs" style="margin-top:2px">{d}</div></div></div></div>'
                    for i, t, d in feats)
welcome_body = (f'<div class="h grow" style="gap:24px;align-items:stretch">'
                f'<div class="card v" style="width:760px;flex:none">'
                f'<div class="grow" style="position:relative">{scene(money=False, style="aspect-ratio:auto;height:100%")}{ov_pill(style="position:absolute;right:16px;top:16px;height:32px;font-size:12px")}</div>'
                f'<div class="h" style="gap:12px;height:56px;padding:0 16px;border-top:1px solid var(--line-1)">'
                f'<span class="btn icon-only sm">{ic("pause")}</span>{wave(64, 20, 30)}'
                f'<span class="muted xs" style="margin-left:auto">Kokoro · Adam</span></div></div>'
                f'<div class="v grow" style="gap:8px">{feat_html}</div></div>')
pages["01-welcome"] = ("Welcome", ob(0, "Welcome", "Hear the text in your games.",
    "DialogReader reads dialogue, item pickups and other on-screen text aloud while you play.",
    welcome_body, note="Setup takes about two minutes. Everything runs on this PC.", primary="Get started", back=False))

# ---------------- 02 Pick window ----------------
def win_row(thumb, name, meta, on=False):
    return (f'<div class="opt{" on" if on else ""} h" style="gap:16px;padding:12px 16px 12px 12px">'
            f'<div style="width:112px;flex:none;border-radius:6px;overflow:hidden;border:1px solid var(--line-2)">{thumb}</div>'
            f'<div class="grow"><div class="md strong">{name}</div><div class="faint xs" style="margin-top:2px">{meta}</div></div>'
            f'<span class="radio{" on" if on else ""}"></span></div>')
sat = '<div class="scene sat"><div class="b1"></div><div class="b2"></div><div class="b3"></div><div class="belt"></div><div class="belt2"></div></div>'
disc = '<div class="scene app discord"><div class="bar"></div><div class="side"></div><div class="ln" style="top:20%;width:40%"></div><div class="ln" style="top:30%;width:55%"></div><div class="ln" style="top:40%;width:30%"></div><div class="ln" style="top:50%;width:48%"></div></div>'
spot = '<div class="scene app spotify"><div class="bar"></div><div class="side"></div><div class="art"></div><div class="ln" style="left:52%;top:30%;width:30%"></div><div class="ln" style="left:52%;top:40%;width:20%"></div></div>'
pick_body = (f'<div class="h" style="gap:24px;align-items:stretch;height:100%">'
    f'<div class="v grow">'
    f'<div class="h" style="gap:8px"><div class="input grow">{ic("search")}<span class="ph">Search windows</span></div><span class="btn">{ic("refresh")}Refresh</span></div>'
    f'<div class="nav-label" style="padding:0;margin-top:16px">Games</div>'
    f'<div class="v" style="gap:8px">{win_row(scene(), "Red Dead Redemption 2", "RDR2.exe · 3840 × 2160 · Borderless", True)}{win_row(sat, "Satisfactory", "FactoryGame.exe · 2560 × 1440 · Windowed")}</div>'
    f'<div class="nav-label" style="padding:0;margin-top:16px">Other windows</div>'
    f'<div class="v" style="gap:8px">{win_row(disc, "Discord", "Discord.exe · 1600 × 900")}{win_row(spot, "Spotify", "Spotify.exe · 1280 × 800")}</div>'
    f'<div class="hint" style="margin-top:auto">{ic("info")}Game missing? Start it, then press Refresh.</div></div>'
    f'<div class="card v" style="width:416px;flex:none"><div class="card-head"><span class="card-title">Selected window</span></div>'
    f'<div style="padding:16px 16px 8px"><div style="border-radius:6px;overflow:hidden;border:1px solid var(--line-2)">{scene()}</div></div>'
    f'<div class="row" style="border-top:0"><div class="row-text muted">Game</div><div>Red Dead Redemption 2</div></div>'
    f'<div class="row"><div class="row-text muted">Program</div><div>RDR2.exe</div></div>'
    f'<div class="row"><div class="row-text muted">Size</div><div class="num">3840 × 2160</div></div>'
    f'<div class="row"><div class="row-text muted">Display mode</div><div>Borderless</div></div>'
    f'<div class="card-foot" style="margin-top:auto">{ic("lock")}DialogReader only looks inside this window.</div></div></div>')
pages["02-pick-window"] = ("Pick window", ob(1, "Step 1 of 5", "Choose your game window",
    "Start your game, then pick its window from the list.", pick_body))

# ---------------- 03 Draw zones ----------------
zones_tb = (f'<div class="toolbar"><span class="btn">{ic("plus")}Add zone</span>{kinds_picker()}<span class="vsep"></span>'
            f'<span class="btn ghost">{ic("undo")}Undo</span><span class="btn ghost">{ic("camera")}New screenshot</span>')
draw_body = (zones_tb + '<span class="spacer"></span><span class="faint xs">5 zones</span></div>'
    f'<div class="h" style="gap:24px;margin-top:16px;align-items:stretch">'
    f'<div class="canvas" style="width:808px;flex:none">{scene(zones={"sel": True})}'
    f'<div class="card-foot" style="background:var(--bg-3)">{ic("cursor")}Drag on the screenshot to draw a zone. Drag a corner to resize it.</div></div>'
    f'{kinds_card("flex:1")}</div>')
pages["03-draw-zones"] = ("Draw zones", ob(2, "Step 2 of 5", "Mark where text appears",
    "Draw a box around each place where text shows up, and pick what kind of text it is.", draw_body))

# ---------------- 04 Read key ----------------
events = [("21:40:12", "Mouse 4", "Pressed", '<span class="xs strong" style="color:var(--accent-text)">Set as read key</span>'),
          ("21:40:12", "Mouse 4", "Released", ""),
          ("21:40:10", "Mouse 1", "Pressed", '<span class="xs faint">Left click is not allowed</span>'),
          ("21:40:09", "E", "Pressed", '<span class="xs faint">Common game key</span>'),
          ("21:40:08", "W", "Pressed", '<span class="xs faint">Common game key</span>'),
          ("21:40:08", "Shift", "Pressed", '<span class="xs faint">Common game key</span>'),
          ("21:40:07", "Tab", "Pressed", '<span class="xs faint">Common game key</span>'),
          ("21:40:07", "Esc", "Pressed", '<span class="xs faint">Cancels</span>')]
ev_html = "".join(f'<div class="row"><span class="faint xs num" style="width:64px;flex:none">{t}</span><span class="key-cell"><span class="kbd">{k}</span></span>'
                  f'<span class="row-text muted">{w}</span>{r}</div>' for t, k, w, r in events)
key_body = (f'<div class="h grow" style="gap:24px;align-items:stretch">'
    f'<div class="card v" style="flex:1;align-items:center;justify-content:center;text-align:center;gap:16px;padding:32px">'
    f'<span class="pill accent">{ic("check")}Key captured</span>'
    f'<div class="h" style="height:88px;padding:0 32px;border-radius:14px;border:1px solid var(--accent-line);border-bottom-width:3px;background:var(--bg-4)">'
    f'<span style="font-size:var(--t-xl);font-weight:600;letter-spacing:-0.01em">Mouse 4</span></div>'
    f'<div class="muted">Side button (back). Press another key to change it.</div>'
    f'<span class="btn" style="margin-top:8px">{ic("refresh")}Choose again</span></div>'
    f'<div class="card" style="flex:1"><div class="card-head"><span class="card-title">Live input</span><span class="spacer"></span>'
    f'<span class="pill"><span class="dot" style="color:var(--accent-text)"></span>Listening</span></div>{ev_html}</div></div>'
    f'<div class="note" style="margin-top:16px">{ic("info")}<span>Pick a key your game does not use. Mouse side buttons work well. When reading on key press, this key reads every zone once.</span></div>')
pages["04-read-key"] = ("Read key", ob(3, "Step 3 of 5", "Choose your read key",
    "Press any key, mouse button or controller button. It reads on demand and repeats the last line.", key_body))

# ---------------- 05 Voice ----------------
voices = [("Kokoro · Adam", "American English · Male · Calm"), ("Kokoro · Michael", "American English · Male · Deep"),
          ("Kokoro · Bella", "American English · Female · Bright"), ("Kokoro · Nicole", "American English · Female · Soft"),
          ("Kokoro · Emma", "British English · Female · Warm"), ("Kokoro · George", "British English · Male · Steady"),
          ("Kokoro · Lewis", "British English · Male · Gravelly")]
v_html = "".join(f'<div class="opt{" on" if i == 0 else ""} h" style="gap:12px;height:60px;padding:0 10px 0 16px">'
                 f'<span class="radio{" on" if i == 0 else ""}"></span><div class="grow"><div class="strong">{n}</div><div class="faint xs">{d}</div></div>'
                 f'<span class="btn ghost icon-only sm">{ic("play")}</span></div>' for i, (n, d) in enumerate(voices))
voice_body = (f'<div class="h grow" style="gap:24px;align-items:stretch">'
    f'<div class="v" style="width:440px;flex:none;gap:8px"><div class="nav-label" style="padding:0">Voices on this PC</div>{v_html}'
    f'<div class="hint" style="margin-top:auto">{ic("download")}More voices in Settings</div></div>'
    f'<div class="v grow" style="gap:8px"><div class="nav-label" style="padding:0">Sound</div>'
    f'<div class="card">'
    f'<div class="row"><div class="row-text"><div class="row-label">Speed</div><div class="row-help">How fast lines are spoken</div></div>'
    f'<div class="row-ctrl" style="width:260px;gap:12px"><div class="slider"><span class="track"></span><span class="fill" style="width:55%"></span><span class="knob" style="left:55%"></span></div><span class="num" style="width:36px;text-align:right">1.1×</span></div></div>'
    f'<div class="row"><div class="row-text"><div class="row-label">Volume</div><div class="row-help">Relative to your game</div></div>'
    f'<div class="row-ctrl" style="width:260px;gap:12px"><div class="slider"><span class="track"></span><span class="fill" style="width:80%"></span><span class="knob" style="left:80%"></span></div><span class="num" style="width:36px;text-align:right">80%</span></div></div></div>'
    f'<div class="card v grow" style="margin-top:8px"><div class="card-head"><span class="card-title">Test the voice</span><span class="spacer"></span><span class="pill accent"><span class="dot"></span>Playing</span></div>'
    f'<div class="v grow" style="padding:16px;gap:16px">'
    f'<div class="seg" style="align-self:flex-start"><span class="on">Dialogue</span><span>Pickup</span><span>Your own text</span></div>'
    f'<div class="input multi readonly" style="height:72px">Arthur: We need to get these horses moving before the snow buries us.</div>'
    f'<div class="grow h" style="justify-content:center;border-radius:var(--r-sm);background:var(--bg-1);border:1px solid var(--line-1)">{wave(84, 56, 40, seed=7)}</div>'
    f'<div class="h" style="gap:16px"><span class="btn">{ic("stop", "accent-icon")}Stop</span><span class="spacer"></span><span class="faint xs num">0:02 / 0:04</span></div>'
    f'</div></div></div></div>')
pages["05-voice"] = ("Voice", ob(4, "Step 4 of 5", "Pick a voice",
    "Choose how lines sound. Play a sample before you decide.", voice_body))

# ---------------- 06 Mode ----------------
def mode_card(on, icon, title, pill, line, sub, example):
    p = '<span class="pill accent">Recommended</span>' if pill else ""
    return (f'<div class="opt{" on" if on else ""} v" style="flex:1;padding:20px;gap:12px">'
            f'<div class="h" style="gap:12px"><span class="tile{" accent" if on else ""}">{ic(icon)}</span>'
            f'<span class="md" style="font-weight:600">{title}</span>{p}<span class="spacer"></span><span class="radio{" on" if on else ""}"></span></div>'
            f'<div>{line}</div><div class="muted xs">{sub}</div>'
            f'<div class="h" style="gap:12px;margin-top:auto;height:48px;padding:0 12px;border-radius:var(--r-sm);background:var(--bg-1);border:1px solid var(--line-1)">{example}</div></div>')
summary = [("Game window", "Red Dead Redemption 2"), ("Text zones", "5 zones"), ("Read key", '<span class="kbd">Mouse 4</span>'),
           ("Voice", "Kokoro · Adam, 1.1×"), ("In-game overlay", "Top right")]
sum_html = "".join(f'<div class="row"><div class="row-text muted">{k}</div><div>{v}</div><span class="btn ghost sm flush-r" style="margin-left:8px">Change</span></div>' for k, v in summary)
mode_body = (f'<div class="h" style="gap:16px;align-items:stretch">'
    + mode_card(True, "bolt", "Automatic", True, "Reads new text in your zones as soon as it appears.", "Best for story games. Short pop-ups are caught too.",
                 '<span class="chip k-dialogue">Dialogue</span><span class="grow ellip">Arthur: We need to get these horses moving…</span><span class="xs" style="color:var(--accent-text)">Read at once</span>')
    + mode_card(False, "keyboard", "On key press", False, 'Reads every zone once when you press <span class="kbd">Mouse 4</span>', "Nothing is read on its own. Best for busy screens.",
                 '<span class="kbd">Mouse 4</span><span class="grow muted">Reads Dialogue, Speaker and Pickups once</span>')
    + f'</div><div class="card" style="margin-top:24px"><div class="card-head"><span class="card-title">Your setup</span></div>{sum_html}</div>')
pages["06-mode"] = ("Reading mode", ob(5, "Step 5 of 5", "How should it read?",
    "Choose when DialogReader speaks. You can switch at any time.", mode_body,
    note="DialogReader then moves to the tray while you play.", primary="Finish setup"))

# ---------------- 10 Main ----------------
LIVE = [("k-dialogue", "Dialogue", "21:42:18", "Arthur: We need to get these horses moving before the snow buries us.", "speaking"),
        ("k-pickups", "Pickups", "21:42:11", "+1 Ginseng", ""),
        ("k-prompts", "Prompts", "21:42:09", "Hold E to talk", "Skipped · Prompts are off"),
        ("k-dialogue", "Dialogue", "21:41:57", "Dutch: Get everyone up to the ridge. We camp there tonight.", ""),
        ("k-dialogue", "Dialogue", "21:41:52", "Dutch: Get everyone up to the ridge. We camp there tonight.", "Skipped · Already read"),
        ("k-pickups", "Pickups", "21:41:40", "+1 Yarrow", ""),
        ("k-dialogue", "Dialogue", "21:41:22", "Sadie: Wind's turning. Best be quick about it.", ""),
        ("k-dialogue", "Dialogue", "21:41:09", "Hosea: Keep the wagons close. This pass gets narrow.", ""),
        ("k-pickups", "Pickups", "21:40:48", "+1 Oregano", "")]
def live_row(k, n, t, line, state):
    speaking = state == "speaking"
    skipped = state.startswith("Skipped")
    right = ""
    if speaking:
        right = f'<span class="xs strong h" style="gap:6px;color:var(--accent-text)">{wave(4, 12, 4, seed=5)}Speaking</span>'
    elif skipped:
        right = f'<span class="xs faint h" style="gap:6px">{state}</span>'
    btn = (f'<span class="btn ghost icon-only sm">{ic("stop", "accent-icon")}</span>' if speaking
           else f'<span class="btn ghost icon-only sm">{ic("replay")}</span>')
    bg = "background:var(--accent-soft);" if speaking else ""
    col = "var(--text-3)" if skipped else "var(--text-1)"
    return (f'<div class="h" style="gap:12px;padding:12px 10px 12px 16px;border-top:1px solid var(--line-1);align-items:flex-start;{bg}">'
            f'<div class="grow"><div class="h" style="gap:8px;margin-bottom:4px"><span class="chip {k}">{n}</span><span class="faint xs num">{t}</span><span class="spacer"></span>{right}</div>'
            f'<div style="color:{col}">{line}</div></div>{btn}</div>')
legend = "".join(f'<span class="chip bare {k}">{n}</span>' for k, n, _ in KIND_HELP)
main_body = (f'<div class="page-body">'
    f'<div class="v" style="width:624px;flex:none;gap:16px">'
    f'<div class="card"><div class="card-head"><span class="card-title">Zones</span><span class="h" style="gap:12px;margin-left:8px">{legend}</span><span class="spacer"></span>'
    f'<span class="btn ghost flush-r">Edit<svg class="i" viewBox="0 0 24 24"><path d="{ICONS["chev"]}"/></svg></span></div>'
    f'<div class="zones-compact">{scene(zones={"prompts_off": True})}</div></div>'
    f'<div class="card grow">'
    f'<div class="row"><div class="row-text"><div class="row-label">Reading mode</div><div class="row-help">Reads new text as soon as it appears</div></div><div class="row-ctrl"><div class="seg"><span class="on">Automatic</span><span>On key press</span></div></div></div>'
    f'<div class="row"><div class="row-text"><div class="row-label">Voice</div><div class="row-help">Speed 1.1× · Volume 80%</div></div><div class="row-ctrl"><div class="select" style="width:180px"><span class="val">Kokoro · Adam</span>{ic("down")}</div><span class="btn icon-only">{ic("play")}</span></div></div>'
    f'<div class="row"><div class="row-text"><div class="row-label">Read key</div><div class="row-help">Reads on demand and repeats the last line</div></div><div class="row-ctrl"><span class="kbd">Mouse 4</span><span class="btn">Change</span></div></div>'
    f'<div class="row"><div class="row-text"><div class="row-label">Read prompts</div><div class="row-help">Button hints like “Hold E to talk”</div></div><div class="row-ctrl"><span class="toggle"></span></div></div>'
    f'<div class="row"><div class="row-text"><div class="row-label">Show overlay in game</div><div class="row-help">Status and the last line read</div></div><div class="row-ctrl"><span class="toggle on"></span></div></div>'
    f'</div></div>'
    f'<div class="card v grow"><div class="card-head" style="border-bottom:0"><span class="card-title">Live</span><span class="faint xs">Lines read in this session</span><span class="spacer"></span>'
    f'<span class="btn ghost flush-r">See all<svg class="i" viewBox="0 0 24 24"><path d="{ICONS["chev"]}"/></svg></span></div>'
    + "".join(live_row(*r) for r in LIVE) + '</div></div>')
pages["10-main"] = ("Main window", window(main_sidebar("rdr"), f'<main class="content">{game_head("Overview")}{main_body}</main>'))

# ---------------- 11 Zones ----------------
zl = [("k-dialogue", "Subtitles", "Dialogue", True, False), ("k-speaker", "Speaker name", "Speaker", False, False),
      ("k-pickups", "Item pickups", "Pickups", False, False), ("k-prompts", "Button hints", "Prompts · off", False, True),
      ("k-ignore", "Money and clock", "Ignore", False, False)]
zl_html = "".join(f'<div class="li{" sel" if s else ""}{" off" if o else ""}"><span class="swatch {k}"{" style=opacity:.4" if o else ""}></span>'
                  f'<div class="grow ellip"><span class="strong">{n}</span> <span class="{"" if o else "muted "}xs">{kn}</span></div>'
                  f'<span class="btn ghost icon-only sm">{ic("eyeoff" if o else "eye")}</span></div>' for k, n, kn, s, o in zl)
zones_body = (f'<div style="padding:16px 32px 0">{zones_tb}<span class="spacer"></span><span class="hint">{ic("check")}Saved</span></div></div>'
    f'<div class="page-body" style="padding-top:16px">'
    f'<div class="v" style="width:816px;flex:none;gap:16px">'
    f'<div class="canvas">{scene(zones={"sel": True, "prompts_off": True})}'
    f'<div class="card-foot" style="background:var(--bg-3)">{ic("cursor")}Drag on the screenshot to draw a zone. Drag a corner to resize it.</div></div>'
    f'<div class="card v grow"><div class="card-head"><span class="swatch k-dialogue"></span><span class="card-title">Subtitles</span><span class="spacer"></span>'
    f'<span class="btn ghost icon-only sm flush-r">{ic("trash")}</span></div>'
    f'<div class="h grow" style="gap:16px;padding:0 16px;align-items:center">'
    f'<div class="field" style="width:168px"><span class="field-label">Kind</span><div class="select"><span class="chip bare k-dialogue">Dialogue</span><span class="val"></span>{ic("down")}</div></div>'
    f'<div class="field" style="width:168px"><span class="field-label">Name</span><div class="input">Subtitles</div></div>'
    f'<div class="field grow"><span class="field-label">Text found right now</span><div class="input readonly"><span class="ellip">We need to get these horses moving before the snow buries us.</span></div></div>'
    f'<div class="field"><span class="field-label">&nbsp;</span><span class="btn">{ic("play")}Read it</span></div>'
    f'</div></div></div>'
    f'<div class="v grow" style="gap:16px"><div class="card"><div class="card-head" style="border-bottom:0"><span class="card-title">Zones</span><span class="faint xs">5</span></div>{zl_html}</div>'
    f'{kinds_card("flex:1")}</div></div>')
pages["11-zones"] = ("Zone editor", window(main_sidebar("rdr"), f'<main class="content">{game_head("Zones")}{zones_body}</main>'))

# ---------------- 12 Settings ----------------
def card(title, rows):
    return f'<div class="card"><div class="card-head"><span class="card-title">{title}</span></div>{"".join(rows)}</div>'
def row(label, help, ctrl):
    return f'<div class="row"><div class="row-text"><div class="row-label">{label}</div><div class="row-help">{help}</div></div><div class="row-ctrl">{ctrl}</div></div>'
def hk(k): return f'<span class="kbd">{k}</span><span class="btn">Change</span>'
sel = lambda v, w: f'<div class="select" style="width:{w}px"><span class="val">{v}</span>{ic("down")}</div>'
left = (card("Startup", [row("Start with Windows", "Opens in the tray when you sign in", '<span class="toggle on"></span>'),
                         row("Open the main window on start", "Off keeps it in the tray only", '<span class="toggle"></span>')])
        + card("Voices", [row("Default voice for new games", "Each game can use its own voice", sel("Kokoro · Adam", 200)),
                          row("Installed voices", "Kokoro · 8 voices · 312 MB", '<span class="btn">Manage</span>')])
        + card("Updates", [row("Version 0.4.0", "You have the latest version", '<span class="btn">Check now</span>'),
                           row("Install updates automatically", "Only when no game is running", '<span class="toggle on"></span>')]))
right = (card("In-game overlay", [row("Show overlay", "Status and the last line read, on top of the game", '<span class="toggle on"></span>'),
                                  row("Position", "Where the overlay sits", '<div class="seg"><span class="on">Top right</span><span>Bottom center</span></div>'),
                                  row("Hide after", "Fades out when nothing is read", sel("5 seconds", 128))])
         + card("Hotkeys", [row("Pause or resume reading", "Works while the game has focus", hk("Ctrl Alt P")),
                            row("Repeat last line", "Says the last line again", hk("Ctrl Alt R")),
                            row("Stop speaking", "Cuts off the current line", hk("Ctrl Alt S")),
                            row("Show or hide overlay", "Shows the status pill in the game", hk("Ctrl Alt O"))]))
set_main = (f'<main class="content"><div class="page-head"><div><div class="page-title">Settings</div><div class="faint xs">These apply to every game</div></div></div>'
            f'<div class="page-body"><div class="v grow" style="gap:16px">{left}</div><div class="v grow" style="gap:16px">{right}</div></div></main>')
pages["12-settings"] = ("Settings", window(main_sidebar("settings"), set_main))

# ---------------- 13 Overlay ----------------
ov = (f'<div style="position:relative;width:1440px;height:900px">{scene(money=False, style="aspect-ratio:auto;height:900px")}'
      + ov_pill(text="We need to get these horses moving before the snow…", style="position:absolute;right:24px;top:24px") + '</div>')
pages["13-overlay"] = ("Overlay", ov)

for name, (title, html) in pages.items():
    with open(os.path.join(OUT, name + ".html"), "w", encoding="utf-8") as f:
        f.write(page(title, html))

# ---------------- index ----------------
LABELS = {"01-welcome": "Welcome", "02-pick-window": "Pick window", "03-draw-zones": "Draw zones", "04-read-key": "Read key",
          "05-voice": "Voice", "06-mode": "Reading mode", "10-main": "Main window", "11-zones": "Zone editor",
          "12-settings": "Settings", "13-overlay": "In-game overlay"}
cells = "".join(f'<a href="{n}.html" style="display:block;color:inherit;text-decoration:none">'
                f'<div style="border:1px solid var(--line-2);border-radius:10px;overflow:hidden;background:var(--bg-0)"><img src="shots/{n}.png" style="display:block;width:100%;aspect-ratio:16/10" alt="{l}"></div>'
                f'<div class="h" style="gap:8px;margin-top:8px"><span class="faint xs num">{n[:2]}</span><span class="strong">{l}</span></div></a>'
                for n, l in LABELS.items())
idx = (f'<div style="padding:32px 48px"><div class="h" style="gap:12px;margin-bottom:24px"><div class="mark">{ic("mark")}</div>'
       f'<span class="page-title">DialogReader · Graphite</span><span class="faint xs" style="margin-top:4px">Direction A · dark graphite, one accent, left sidebar · round 2</span>'
       f'<span class="spacer"></span><span class="h" style="gap:8px">{"".join(f"<span class=\"chip {k}\">{n}</span>" for k, n, _ in KIND_HELP)}</span></div>'
       f'<div style="display:grid;grid-template-columns:repeat(4,1fr);gap:24px 20px">{cells}</div></div>')
with open(os.path.join(OUT, "index.html"), "w", encoding="utf-8") as f:
    f.write(page("Graphite mockups", idx, "html,body{height:auto;min-height:900px;overflow:auto}"))

print("ok", len(pages))
