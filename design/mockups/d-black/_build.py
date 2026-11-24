"""Builds the d-black mockups. Run: python _build.py  (writes the .html files next to this script).
Source of truth for tokens, text and motion: research/dialogue-reader/2026-11-24-redesign-brief.md."""
import os, html as H
OUT = os.path.dirname(os.path.abspath(__file__))

def write(name, s):
    with open(os.path.join(OUT, name), "w", encoding="utf-8", newline="\n") as f:
        f.write(s)

# ---------- icons: 1.5 px line icons, 24 grid ----------
P = {
 "chev-down": '<path d="m6 9 6 6 6-6"/>',
 "chev-left": '<path d="m15 18-6-6 6-6"/>',
 "chev-right": '<path d="m9 18 6-6-6-6"/>',
 "play": '<path d="M7 4.5v15l12-7.5z"/>',
 "pause": '<path d="M8 5v14M16 5v14"/>',
 "stop": '<rect x="6" y="6" width="12" height="12" rx="2"/>',
 "replay": '<path d="M3 12a9 9 0 1 0 9-9 9.75 9.75 0 0 0-6.74 2.74L3 8"/><path d="M3 3v5h5"/>',
 "volume": '<path d="M11 5 6 9H3v6h3l5 4z"/><path d="M15.5 8.5a5 5 0 0 1 0 7"/>',
 "zap": '<path d="M13 2 4 14h7l-1 8 9-12h-7z"/>',
 "settings": '<path d="M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z"/><circle cx="12" cy="12" r="3"/>',
 "x": '<path d="M18 6 6 18M6 6l12 12"/>',
 "minus": '<path d="M5 12h14"/>',
 "square": '<rect x="5" y="5" width="14" height="14" rx="1.5"/>',
 "plus": '<path d="M5 12h14"/><path d="M12 5v14"/>',
 "search": '<circle cx="11" cy="11" r="7"/><path d="m20 20-3.5-3.5"/>',
 "pencil": '<path d="M17 3a2.85 2.83 0 1 1 4 4L7.5 20.5 2 22l1.5-5.5Z"/>',
 "camera": '<path d="M14.5 4h-5L7 7H4a2 2 0 0 0-2 2v9a2 2 0 0 0 2 2h16a2 2 0 0 0 2-2V9a2 2 0 0 0-2-2h-3z"/><circle cx="12" cy="13" r="3"/>',
 "trash": '<path d="M3 6h18M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6M8 6V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"/>',
 "download": '<path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><path d="m7 10 5 5 5-5"/><path d="M12 15V3"/>',
 "upload": '<path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><path d="m17 8-5-5-5 5"/><path d="M12 3v12"/>',
 "refresh": '<path d="M3 12a9 9 0 0 1 9-9 9.75 9.75 0 0 1 6.74 2.74L21 8"/><path d="M21 3v5h-5"/><path d="M21 12a9 9 0 0 1-9 9 9.75 9.75 0 0 1-6.74-2.74L3 16"/><path d="M8 16H3v5"/>',
 "check": '<path d="M20 6 9 17l-5-5"/>',
 "draw": '<path d="M4 8V5a1 1 0 0 1 1-1h3M16 4h3a1 1 0 0 1 1 1v3M20 16v3a1 1 0 0 1-1 1h-3M8 20H5a1 1 0 0 1-1-1v-3"/><path d="M12 9v6M9 12h6"/>',
 "k-dialogue": '<path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"/>',
 "k-speaker": '<circle cx="12" cy="8" r="4"/><path d="M20 21a8 8 0 0 0-16 0"/>',
 "k-pickups": '<path d="M21 8 12 3 3 8v8l9 5 9-5z"/><path d="m3 8 9 5 9-5"/><path d="M12 13v8"/>',
 "k-prompts": '<rect x="3" y="4" width="18" height="16" rx="3"/><rect x="7" y="7.5" width="10" height="6" rx="1.5"/>',
 "window": '<rect x="3" y="4" width="18" height="16" rx="2"/><path d="M3 9h18"/>',
 "external": '<path d="M15 3h6v6"/><path d="M10 14 21 3"/><path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"/>',
 "power": '<path d="M12 2v10"/><path d="M18.4 6.6a9 9 0 1 1-12.77.04"/>',
 "gauge": '<path d="m12 14 4-4"/><path d="M3.34 19a10 10 0 1 1 17.32 0"/>',
 "sliders": '<path d="M4 6h10M18 6h2M4 12h4M12 12h8M4 18h12M20 18h0"/><circle cx="16" cy="6" r="2"/><circle cx="10" cy="12" r="2"/><circle cx="18" cy="18" r="2"/>',
 "volume-x": '<path d="M11 5 6 9H3v6h3l5 4z"/><path d="m22 9-6 6M16 9l6 6"/>',
 "arrow-right": '<path d="M5 12h14"/><path d="m12 5 7 7-7 7"/>',
 "key": '<rect x="2" y="6" width="20" height="12" rx="2"/><path d="M6 10h.01M10 10h.01M14 10h.01M18 10h.01M7 14h10"/>',
 "k-ignore":'<path d="M10.73 5.08A10.43 10.43 0 0 1 12 5c7 0 10 7 10 7a13.16 13.16 0 0 1-1.67 2.68"/><path d="M6.61 6.61A13.53 13.53 0 0 0 2 12s3 7 10 7a9.74 9.74 0 0 0 5.39-1.61"/><path d="M9.88 9.88a3 3 0 1 0 4.24 4.24"/><path d="m2 2 20 20"/>',
}
def ic(n, cls="", strike=False, id=""):
    c = f"ic {cls}".strip()
    s = '<path class="strike" d="M3 3l18 18"/>' if strike else ""
    i = f' id="{id}"' if id else ""
    return f'<svg class="{c}"{i} viewBox="0 0 24 24" aria-hidden="true">{P[n]}{s}</svg>'

def head(title, css="", body_cls=""):
    return ('<!doctype html>\n<html lang="en"><head><meta charset="utf-8">'
            f'<meta name="viewport" content="width=1440"><title>{title}</title>'
            f'<link rel="stylesheet" href="tokens.css"><style>{css}</style></head>\n<body class="{body_cls}">\n')

LOGO = ('<svg width="20" height="20" viewBox="0 0 24 24" aria-hidden="true" style="display:block">'
        '<rect width="24" height="24" rx="6" fill="#F2F2F2"/>'
        '<rect x="6.5" y="9" width="2" height="6" rx="1" fill="#000"/>'
        '<rect x="11" y="6" width="2" height="12" rx="1" fill="#000"/>'
        '<rect x="15.5" y="8" width="2" height="8" rx="1" fill="#000"/></svg>')

WINCTL = f'<div class="winctl"><span>{ic("minus")}</span><span>{ic("square")}</span><span class="close">{ic("x")}</span></div>'
TIPS = {"live": "Reading", "paused": "Paused", "off": "Game not running"}

def strip(mode="ov", game="Red Dead Redemption 2", page="Settings", status="live", tip=False):
    """mode ov: game name switcher + gear. mode sub: back chevron + page name, no gear."""
    sc = "" if status == "live" else status
    lead_game = (f'<div class="gname" id="lead-game" title="Switch game">'
                 f'<span class="roll"><span id="gname-t">{game}</span></span>{ic("chev-down")}</div>')
    lead_back = f'<div class="back" id="lead-back" title="Back (Esc)">{ic("chev-left")}<span id="back-t">{page}</span></div>'
    gear = f'<span class="iconbtn gear" id="gear" title="Settings">{ic("settings")}</span>'
    if mode == "ov":
        lead = lead_game; tail_gear = gear
    else:
        lead = lead_back; tail_gear = ""
    if mode == "both":  # motion page carries both and fades between them
        lead = lead_game + lead_back.replace('class="back"', 'class="back" style="opacity:0;pointer-events:none"')
        tail_gear = gear
    hide = ' style="opacity:0;pointer-events:none"'
    dot_style = ""
    if mode == "setup":  # setup: the brand only. The game name, dot and gear fade in on Finish.
        lead = (f'<div class="brand" id="lead-brand">{LOGO}DialogReader</div>'
                + lead_game.replace('class="gname"', 'class="gname"' + hide))
        tail_gear = gear.replace('class="iconbtn gear"', 'class="iconbtn gear"' + hide)
        dot_style = hide
    t = " showtip" if tip else ""
    return (f'<header class="strip"><div class="lead">{lead}</div><div class="tail">'
            f'<span class="sdot {sc}{t}" id="sdot"{dot_style}><i></i><span class="tip">{TIPS[status]}</span></span>'
            f'{tail_gear}{WINCTL}</div></header>')

# ---------- game art ----------
SCENE_RDR = '''<svg class="scene" viewBox="0 0 1600 900" preserveAspectRatio="xMidYMid slice" aria-hidden="true">
<defs>
<linearGradient id="sky" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#232323"/><stop offset=".55" stop-color="#686866"/><stop offset="1" stop-color="#BCBCB9"/></linearGradient>
<linearGradient id="snow" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#E6E6E4"/><stop offset="1" stop-color="#B2B2AF"/></linearGradient>
<linearGradient id="shade" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#050505" stop-opacity="0"/><stop offset="1" stop-color="#050505" stop-opacity=".62"/></linearGradient>
</defs>
<rect width="1600" height="900" fill="url(#sky)"/>
<polygon fill="#7A7A78" points="0,520 170,370 310,450 470,300 640,430 790,330 960,470 1120,320 1300,440 1450,350 1600,430 1600,900 0,900"/>
<polygon fill="#9C9C99" points="0,600 260,520 520,590 800,540 1100,600 1400,530 1600,580 1600,900 0,900"/>
<polygon fill="url(#snow)" points="0,660 400,630 900,662 1300,636 1600,656 1600,900 0,900"/>
<g fill="#2A2A2A"><polygon points="430,640 470,520 510,640"/><polygon points="480,646 512,560 544,646"/><polygon points="1050,650 1095,500 1140,650"/><polygon points="1110,654 1140,570 1170,654"/><polygon points="1350,640 1380,560 1410,640"/></g>
<rect y="620" width="1600" height="280" fill="url(#shade)"/>
<g font-family="Segoe UI, sans-serif">
<rect x="100" y="60" width="266" height="60" rx="6" fill="#0A0A0A" fill-opacity=".6"/>
<rect x="116" y="74" width="32" height="32" rx="4" fill="#C9A35A"/>
<text x="164" y="99" font-size="24" fill="#FFFFFF">+1 Ginseng</text>
<rect x="1240" y="700" width="260" height="48" rx="6" fill="#0A0A0A" fill-opacity=".6"/>
<text x="1370" y="731" text-anchor="middle" font-size="21" fill="#FFFFFF">Hold E to talk</text>
<circle cx="200" cy="770" r="80" fill="#141414" fill-opacity=".7" stroke="#D4D4D4" stroke-width="3"/>
<path d="M200 748 l10 24 h-20z" fill="#FFFFFF"/>
<text x="800" y="786" text-anchor="middle" font-size="24" font-weight="600" fill="#E9C46A">Arthur</text>
<text x="800" y="834" text-anchor="middle" font-size="27" fill="#FFFFFF">We need to get these horses moving before the snow buries us.</text>
</g></svg>'''

SCENE_SAT = '''<svg class="scene" viewBox="0 0 1600 900" preserveAspectRatio="xMidYMid slice" aria-hidden="true">
<defs><linearGradient id="sky2" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#D9773A"/><stop offset="1" stop-color="#F4CF96"/></linearGradient>
<linearGradient id="shade2" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#000" stop-opacity="0"/><stop offset="1" stop-color="#000" stop-opacity=".55"/></linearGradient></defs>
<rect width="1600" height="900" fill="url(#sky2)"/><circle cx="1180" cy="260" r="110" fill="#FBE7C2"/>
<polygon fill="#B7825A" points="0,600 400,540 900,600 1600,500 1600,900 0,900"/>
<g fill="#2C3740"><rect x="220" y="320" width="100" height="360"/><rect x="360" y="430" width="220" height="250"/><rect x="620" y="270" width="70" height="410"/><rect x="0" y="660" width="1600" height="240"/></g>
<rect x="360" y="500" width="960" height="26" fill="#E8A24E"/>
<rect y="620" width="1600" height="280" fill="url(#shade2)"/>
<g font-family="Segoe UI, sans-serif">
<rect x="100" y="60" width="300" height="60" rx="6" fill="#000" fill-opacity=".5"/>
<text x="124" y="99" font-size="24" fill="#FFFFFF">+12 Iron ore</text>
<text x="800" y="786" text-anchor="middle" font-size="24" font-weight="600" fill="#FFD27A">Ada</text>
<text x="800" y="834" text-anchor="middle" font-size="27" fill="#FFFFFF">Welcome back, pioneer. Your shift starts now.</text>
</g></svg>'''

ART = {
 "rdr2": ('<linearGradient id="a-rdr2" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#282828"/><stop offset="1" stop-color="#A8A8A6"/></linearGradient>',
          '<rect width="160" height="90" fill="url(#a-rdr2)"/><polygon fill="#787876" points="0,52 22,36 38,44 60,26 82,42 104,30 128,46 160,34 160,90 0,90"/>'
          '<polygon fill="#E4E4E2" points="0,66 50,61 100,66 160,60 160,90 0,90"/>'
          '<g fill="#2A2A2A"><polygon points="24,68 30,50 36,68"/><polygon points="33,69 38,56 43,69"/><polygon points="118,66 124,46 130,66"/></g>'),
 "satis": ('<linearGradient id="a-satis" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#D9773A"/><stop offset="1" stop-color="#F4CF96"/></linearGradient>',
           '<rect width="160" height="90" fill="url(#a-satis)"/><circle cx="118" cy="30" r="12" fill="#FBE7C2"/>'
           '<polygon fill="#B7825A" points="0,62 40,54 90,60 160,50 160,90 0,90"/>'
           '<g fill="#2C3740"><rect x="22" y="34" width="10" height="34"/><rect x="36" y="44" width="22" height="24"/><rect x="62" y="28" width="7" height="40"/><rect x="0" y="66" width="160" height="24"/></g>'),
 "p4g": ('<linearGradient id="a-p4g" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#F7E07A"/><stop offset="1" stop-color="#D9A900"/></linearGradient>',
         '<rect width="160" height="90" fill="url(#a-p4g)"/>'
         '<g fill="#3B3320"><rect x="10" y="40" width="26" height="50"/><rect x="40" y="52" width="20" height="38"/><rect x="104" y="36" width="22" height="54"/><rect x="130" y="48" width="30" height="42"/></g>'),
 "kcd2": ('<linearGradient id="a-kcd2" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#9FB6CC"/><stop offset="1" stop-color="#E6E0C6"/></linearGradient>',
          '<rect width="160" height="90" fill="url(#a-kcd2)"/><polygon fill="#5D7A49" points="0,58 40,46 80,54 120,42 160,50 160,90 0,90"/>'
          '<polygon fill="#2F4527" points="0,72 60,64 120,72 160,66 160,90 0,90"/>'),
}
def thumb(g):
    d, body = ART[g]
    return f'<span class="thumb"><svg viewBox="0 0 160 90" preserveAspectRatio="xMidYMid slice" aria-hidden="true"><defs>{d}</defs>{body}</svg></span>'

# zones in 1600 x 900 picture space: kind, name, x1, y1, x2, y2
ZONES_RDR = [
 ("speaker", "Speaker", 712, 754, 888, 798),
 ("dialogue", "Subtitles", 360, 800, 1240, 850),
 ("pickups", "Pickups", 88, 48, 378, 132),
 ("prompts", "Prompts", 1228, 690, 1512, 758),
 ("ignore", "Minimap", 112, 682, 288, 858),
]
ZONES_SAT = [
 ("speaker", "Speaker", 740, 754, 860, 798),
 ("dialogue", "Subtitles", 420, 800, 1180, 850),
 ("pickups", "Pickups", 88, 48, 412, 132),
]
KIND = {"dialogue": "Dialogue", "speaker": "Speaker", "pickups": "Pickups", "prompts": "Prompts", "ignore": "Ignore"}

def pct(x1, y1, x2, y2):
    return f"left:{x1/16:.3f}%;top:{y1/9:.3f}%;width:{(x2-x1)/16:.3f}%;height:{(y2-y1)/9:.3f}%"

def ov_outlines(zones, live="dialogue", off=("prompts",)):
    out = []
    for k, n, *r in zones:
        cls = "zo" + (" live" if k == live else "") + (" dim" if k in off or k == "ignore" else "")
        out.append(f'<span class="{cls}" data-k="{k}" style="{pct(*r)}"></span>')
    return "".join(out)

# zone strip: one chip per kind (brief 4.1.3), Ignore zones are not listed. Click toggles reading that kind.
ZS_KINDS = {"rdr2": [("speaker", True), ("dialogue", True), ("pickups", True), ("prompts", False)],
            "satis": [("speaker", True), ("dialogue", True), ("pickups", True)]}
def zchip(k, on):
    return (f'<span class="zi{"" if on else " off"}" data-k="{k}" title="{KIND[k]}: {"on" if on else "off"}">'
            f'{ic("k-" + k, strike=True)}{KIND[k]}</span>')
def zs_items(g):
    return "".join(zchip(k, on) for k, on in ZS_KINDS[g])

def zn_outlines(zones, sel="Subtitles", off=("prompts",)):
    out = []
    for k, n, *r in zones:
        cls = "zz" + (" sel" if n == sel else "") + (" dim" if k in off else "")
        tag = f'<span class="ztag">{ic("k-" + k)}{KIND[k]}<span class="tag zoff">off</span></span>'
        hd = ""
        if n == sel:
            for hx, hy in [(0,0),(50,0),(100,0),(0,50),(100,50),(0,100),(50,100),(100,100)]:
                hd += f'<i style="left:{hx}%;top:{hy}%"></i>'
        out.append(f'<span class="{cls}" data-k="{k}" style="{pct(*r)}">{tag}{hd}</span>')
    return "".join(out)

# ---------- Overview ----------
# Geometry, 1440 x 900 window. Even 24 px margins on all sides of the content.
# Left column: frame 24..1080 x 100..824 holds the 16:9 stage, letterboxed. Zone strip 832..876.
# Right column: Now panel 1096..1416 x 100..876 (320 wide).
OV_CSS = """
.ov-tb{position:absolute;left:24px;right:24px;top:52px;height:32px;display:flex;align-items:center;gap:8px}
.frame{position:absolute;z-index:1;left:24px;top:100px;width:1056px;height:724px;border-radius:var(--r-lg);background:var(--surface-1);border:1px solid var(--line)}
.stage{position:absolute;z-index:2;left:24px;top:165px;width:1056px;height:594px;border-radius:var(--r-lg);overflow:hidden;background:var(--surface-1);cursor:pointer}
.stage::after{content:"";position:absolute;inset:0;border-radius:var(--r-lg);border:1px solid var(--line);pointer-events:none;transition:border-color var(--t-fast) var(--ease-out)}
.stage:hover::after{border-color:var(--line-2)}
.stage .scene{position:absolute;inset:0;width:100%;height:100%;display:block}
.stage .sc{position:absolute;inset:0}
.deco{position:absolute;inset:0;z-index:2}
.zz:not(.dim) .zoff{display:none}
/* Every outline has a black keyline outside it, so it holds on bright games. Slint: a second Rectangle, 1 px larger
   (2 px for the live outline), with a black border. One extra Rectangle per outline, no stacked shadows. */
.zo{position:absolute;border:1px solid rgba(255,255,255,.38);border-radius:var(--r-sm);box-shadow:0 0 0 1px rgba(0,0,0,.6);transition:border-color var(--t-fast) var(--ease-out)}
.zo.dim{border-color:rgba(255,255,255,.16);box-shadow:0 0 0 1px rgba(0,0,0,.3)}
.zo.live{border:2px solid var(--live);box-shadow:0 0 0 2px rgba(0,0,0,.6)}
.zo.live.dim{border:1px solid rgba(255,255,255,.16);box-shadow:0 0 0 1px rgba(0,0,0,.3)}
.zz{position:absolute;border:1px solid rgba(255,255,255,.7);border-radius:var(--r-sm);background:rgba(0,0,0,.18);box-shadow:0 0 0 1px rgba(0,0,0,.6)}
.zz.dim{border-color:rgba(255,255,255,.3);background:rgba(0,0,0,.3);box-shadow:0 0 0 1px rgba(0,0,0,.3)}
.zz.sel{border:2px solid var(--on);box-shadow:0 0 0 1px rgba(0,0,0,.6)}
.zz i{position:absolute;width:8px;height:8px;margin:-4px 0 0 -4px;background:var(--on);border:1px solid #000;border-radius:var(--r-pill)}
.ztag{position:absolute;left:-1px;bottom:calc(100% + 4px);display:inline-flex;align-items:center;gap:4px;height:24px;padding:0 8px;border-radius:var(--r-sm);background:var(--surface-1);border:1px solid rgba(255,255,255,.1);font-size:var(--fs-small);color:var(--text);white-space:nowrap}
/* no room above the box (top of the picture): the tag goes under it; no room under it either: inside, top left */
.zz.tagb .ztag{bottom:auto;top:calc(100% + 4px)}
.zz.tagi .ztag{bottom:auto;top:4px;left:4px}
.ztag .ic{width:13px;height:13px;color:var(--text-2)}
.ztag .tag{height:18px;padding:0 8px;margin-left:4px}
.zz.dim .ztag{color:var(--text-2)}
.zz.sel .ztag{background:var(--on);color:var(--on-ink);border-color:var(--on);box-shadow:0 0 0 1px rgba(0,0,0,.6)}
.zz.sel .ztag .ic{color:var(--on-ink)}
.zz.sel .ztag .tag{color:var(--on-ink);border-color:rgba(0,0,0,.35)}
.stage .empty{position:absolute;inset:0;display:flex;flex-direction:column;align-items:center;justify-content:center;gap:12px;color:var(--text-2);font-size:var(--fs-body)}
.stage.not-running{cursor:default;background:transparent}
.stage.not-running::after{border-color:transparent}
.zs{position:absolute;left:24px;top:832px;width:1056px;height:44px;display:flex;align-items:center;gap:4px}
#zs-items{display:flex;gap:4px}
.zs.disabled .zi{color:var(--text-off);pointer-events:none}
.zs.disabled .zi .ic{color:var(--text-off)}
.panel.now{position:absolute;left:1096px;top:100px;width:320px;height:776px;display:flex;flex-direction:column;overflow:hidden}
.np{padding:16px 16px 12px;display:flex;flex-direction:column;gap:8px;flex:none;min-height:186px}
.np .who{font-size:var(--fs-small);color:var(--text-2)}
.np .line{font:var(--fw-hero) var(--fs-hero)/1.4 var(--font);color:var(--text);letter-spacing:-.1px}
.np .ctl{display:flex;align-items:center;gap:4px;margin-top:auto;padding-top:8px}
.np .ctl .wave{flex:1}
.np.empty{flex:1;align-items:center;justify-content:center}
.np.empty .line{font:var(--fw-body) var(--fs-body)/1.43 var(--font);color:var(--text-2)}
.list{position:relative;flex:1;border-top:1px solid var(--line);padding:4px;overflow:hidden;display:flex;flex-direction:column}
.list .lrow{flex:none}
.foot{height:44px;flex:none;display:flex;align-items:center;justify-content:center;border-top:1px solid var(--line)}
.foot .btn{height:32px;color:var(--text-2);font-weight:var(--fw-body)}
.foot .btn .ic{width:14px;height:14px}
"""

WAVE_H = [6,10,16,9,20,13,7,15,22,12,8,17,11,19,9,6,13,18,10,7,14,20,12,8,5,11,16,9]
def wave(flat=False, n=28):
    bars = "".join(f'<i style="height:{h}px"></i>' for h in WAVE_H[:n])
    return f'<div class="wave{" flat" if flat else ""}" id="wave">{bars}</div>'

# Two long lines on purpose: one wraps to two lines, one is cut after two lines (full text on hover).
RECENT_RDR = [
 ("pickups", "", "+1 Ginseng", False),
 ("prompts", "", "Hold E to talk", True),
 ("dialogue", "Dutch", "We ride at first light. Nobody stays behind.", False),
 ("dialogue", "Arthur", "I hear you.", False),
 ("pickups", "", "+1 Bourbon", False),
 ("dialogue", "Hosea", "If we do not get the wagons through the pass before dark, we lose the whole camp to the cold, and I will not bury anyone else this winter.", False),
 ("dialogue", "Sadie", "Keep your head down, Mr. Morgan.", False),
 ("pickups", "", "+2 Arrows", False),
 ("dialogue", "Arthur", "Easy, girl. Easy.", False),
 ("dialogue", "Javier", "The law is two miles back.", False),
 ("pickups", "", "+1 Oregano", False),
 ("dialogue", "Arthur", "Then we go now.", False),
 ("dialogue", "Charles", "The horses need rest.", False),
]
LIST_MAX = 16
def lrow(kind, who, text, off, i=0, meta="", tag=None):
    """A list row (Overview Now list, Activity). off rows are dimmed and carry a small tag instead of replay."""
    w = f"<b>{who}:</b> " if who else ""
    m = f'<span class="meta">{meta}</span>' if meta else ""
    t = tag or ("off" if off else "")
    end = f'<span class="tag">{t}</span>' if t else f'<span class="iconbtn sm rp" title="Replay">{ic("replay")}</span>'
    return (f'<div class="lrow{" off" if off or tag else ""}">'
            f'<span class="k" title="{KIND[kind]}">{ic("k-" + kind)}</span><span class="tx">{w}{H.escape(text)}</span>{end}{m}</div>')

def zone_strip(off=False):
    return (f'<div class="zs{" disabled" if off else ""}" id="ov-zs"><span id="zs-items">{zs_items("rdr2")}</span>'
            f'<span class="spacer"></span>'
            f'<span class="iconbtn{" disabled" if off else ""}" id="pencil" title="Edit zones">{ic("pencil")}</span></div>')

PAUSE_BTN = (f'<span class="swapic">{ic("pause", id="ic-pause")}{ic("play", "fill", id="ic-resume")}</span>'
             f'<span id="pause-t">Pause</span>')

def overview_body(state="live", hover_row=None):
    off = state == "off"
    dis = " disabled" if off else ""
    tb = (f'<div class="ov-tb" id="ov-tb">'
          f'<span class="btn primary{dis}" id="read-now">{ic("play", "fill")}Read now</span>'
          f'<span class="btn ghost{dis}" id="pause">{PAUSE_BTN}</span>'
          f'<span class="spacer"></span>'
          f'<span class="opt" id="opt-mode" title="Mode">{ic("zap")}<span id="mode-t">Automatic</span></span>'
          f'<span class="opt" id="opt-voice" title="Voice">{ic("volume")}Adam</span>'
          f'<span class="opt" id="opt-key" title="Read key">{ic("key")}<span id="key-t">Mouse 4</span></span>'
          f'</div>')
    frame = '<div class="frame" id="frame"></div>'
    if off:
        stage = ('<div class="stage not-running" id="stage"><div class="empty"><p>Game not running.</p>'
                 '<span class="btn" id="off-switch">Switch game</span></div></div>')
    else:
        stage = (f'<div class="stage" id="stage" title="Edit zones">'
                 f'<div class="sc" id="sc-a">{SCENE_RDR}</div>'
                 f'<div class="deco" id="ov-deco"><div class="deco" id="ov-deco-c">{ov_outlines(ZONES_RDR)}</div></div>'
                 '<div class="deco zlayer" id="zn-deco" style="opacity:0"></div>'
                 '</div>')
    if off:
        np = '<div class="np empty" id="np"><div class="line">Nothing read yet.</div></div>'
        lst = ""
        foot = ""
    else:
        np = (f'<div class="np" id="np"><div class="who" id="np-who">Arthur</div>'
              f'<div class="line" id="np-line">We need to get these horses moving before the snow buries us.</div>'
              f'<div class="ctl">{wave()}<span class="iconbtn sm" id="np-replay" title="Replay">{ic("replay")}</span>'
              f'<span class="iconbtn sm" id="np-stop" title="Stop">{ic("stop")}</span></div></div>')
        rows = "".join(lrow(*r, i=i) for i, r in enumerate(RECENT_RDR))
        if hover_row is not None:
            rows = rows.replace('class="lrow"', 'class="lrow is-hover"', 1) if hover_row == 0 else rows
        lst = f'<div class="list" id="list">{rows}</div>'
        foot = f'<div class="foot"><span class="btn ghost" id="show-all">Show all{ic("chev-right")}</span></div>'
    panel = f'<div class="panel now" id="ov-panel">{np}{lst}{foot}</div>'
    return tb + frame + stage + zone_strip(off) + panel

# ---------- shared JS: motion core, list rows, keycaps, zone editor, switcher ----------
# Every interactive page loads MCORE first. Every tween is A to B on opacity, translate, scale or a rect, so Slint can do it.
MCORE = r"""<script>
/* Motion tokens: brief section 1.2 */
const M={fast:120,base:180,page:260,exit:140,stagger:40,glide:360,
  out:'cubic-bezier(0.2, 0.7, 0.2, 1)',strong:'cubic-bezier(0.16, 1, 0.3, 1)',in:'cubic-bezier(0.4, 0, 1, 1)'};
/* Page push, Max 2026-11-24: slight overlap. The old page fades out and the new page slides in 80 ms later.
   Forward: out 100 ease-in, in 260 strong after 80. Back: out 120 ease-in, in 200 strong after 80.
   Title strip: old label out 80 ease-in, new label in 160 strong after 80. */
const PUSH={out:100,inDelay:80,in:260,backOut:120,backInDelay:80,backIn:200,stripOut:80,stripIn:160,stripDelay:80};
const Q=new URLSearchParams(location.search);
const $=id=>document.getElementById(id);
let SLOW=1, INSTANT=false;
const REDUCE=matchMedia('(prefers-reduced-motion: reduce)').matches;
window.onerror=(m,s,l)=>{($('selftest')||document.body).append('ERROR '+m+' line '+l+'\n')};

/* tween(el,{prop:[from,to]},ms,ease,delay). transform parts: x,y,s. Rect parts: left,top,width,height.
   Reduce motion: anything that moves jumps (0 ms), opacity fades over 80 ms. */
function tween(el,p,ms,ease,delay=0){
  if(REDUCE&&!INSTANT&&('opacity' in p)&&Object.keys(p).length>1){
    const {opacity,...rest}=p; anim(el,rest,0,ease,0); return anim(el,{opacity},80,ease,0);
  }
  return anim(el,p,ms,ease,delay);
}
function anim(el,p,ms,ease,delay){
  const cs=getComputedStyle(el), cur=readT(el), f={},t={};
  const T=(o)=>`translate(${o.x}px,${o.y}px) scale(${o.s})`;
  let hasT=false; const tf={...cur},tt={...cur};
  for(const k in p){
    let [a,b]=p[k];
    if(k==='x'||k==='y'||k==='s'){hasT=true; tf[k]=a??cur[k]; tt[k]=b;}
    else{ f[k]=a??cs[k]; t[k]=typeof b==='number'&&k!=='opacity'?b+'px':String(b); if(typeof f[k]==='number'&&k!=='opacity')f[k]+='px'; }
  }
  if(hasT){f.transform=T(tf);t.transform=T(tt);}
  let d=ms*SLOW, dl=delay*SLOW;
  if(REDUCE){ if('opacity' in p){d=80;dl=0;} else {d=0;dl=0;} }
  if(INSTANT){d=0;dl=0;}
  return el.animate([f,t],{duration:d,delay:dl,easing:ease,fill:'both'});
}
function readT(el){const t=getComputedStyle(el).transform; if(!t||t==='none') return {x:0,y:0,s:1}; const m=new DOMMatrixReadOnly(t); return {x:m.e,y:m.f,s:m.a};}
function live(el,on){el.style.pointerEvents=on?'auto':'none'}
function bez(x1,y1,x2,y2){const A=(a,b)=>1-3*b+3*a,B=(a,b)=>3*b-6*a,C=a=>3*a,f=(t,a,b)=>((A(a,b)*t+B(a,b))*t+C(a))*t,d=(t,a,b)=>3*A(a,b)*t*t+2*B(a,b)*t+C(a);
  return x=>{let t=x;for(let i=0;i<8;i++){const e=f(t,x1,x2)-x;const s=d(t,x1,x2);if(Math.abs(e)<1e-5||!s)break;t-=e/s;}return f(t,y1,y2);};}
const EOS=bez(0.16,1,0.3,1);
/* ?ms=N freezes every running animation at N ms (for mid-transition screenshots) */
function freeze(){const ms=Q.get('ms'); if(ms==null) return; document.getAnimations().forEach(a=>{a.pause(); a.currentTime=+ms;});}
function bindSlow(){const s=$('m-slow'); if(s) s.onchange=e=>{SLOW=e.target.checked?4:1};}
</script>"""

LIST_JS = r"""<script>
/* List rows: two lines, then cut (Max 2026-11-24). Hovering a cut row for 200 ms lays the full row over the rows below:
   the same row, unclamped, on surface-2 with a hairline and a shadow, fading in 120 ms. It leaves when the pointer leaves it. */
function bindList(list){
  let t=null, card=null;
  function hide(){ if(!card) return; const c=card; card=null; tween(c,{opacity:[null,0]},M.fast,M.in).onfinish=()=>c.remove(); }
  function show(r){
    hide(); const c=r.cloneNode(true); c.classList.add('lfull','is-hover'); c.classList.remove('gone');
    c.style.left=r.offsetLeft+'px'; c.style.width=r.offsetWidth+'px'; c.style.top=r.offsetTop+'px'; c.style.transform='none';
    list.append(c); card=c;
    const room=list.clientHeight-4; if(getComputedStyle(list).overflowY==='hidden'&&r.offsetTop+c.offsetHeight>room) c.style.top=Math.max(4,room-c.offsetHeight)+'px';
    tween(c,{opacity:[0,1]},M.fast,M.out);
    c.addEventListener('mouseleave',hide);
  }
  list.addEventListener('mouseover',e=>{
    const r=e.target.closest('.lrow'); if(!r||r.classList.contains('lfull')) return;
    clearTimeout(t); const tx=r.querySelector('.tx');
    if(card&&!card.contains(e.target)) hide();
    if(!tx||tx.scrollHeight<=tx.clientHeight+1) return;
    t=setTimeout(()=>show(r),INSTANT?0:200);
  });
  list.addEventListener('mouseleave',()=>{clearTimeout(t); hide();});
  return {hide, show};
}
</script>"""

KEYCAP_JS = r"""<script>
/* Keycaps: click one, it pulses until a key is pressed (900 ms loop). Esc cancels. Mouse 4 and 5 count. */
let capturing=null, capOld='';
function keyName(e){const m={' ':'Space','Control':'Ctrl','Escape':'Esc','ArrowUp':'Up','ArrowDown':'Down','ArrowLeft':'Left','ArrowRight':'Right'}; return m[e.key]||(e.key.length===1?e.key.toUpperCase():e.key);}
function endCapture(label){
  const k=capturing; if(!k) return; capturing=null; k.classList.remove('capture'); k.textContent=label;
  tween(k,{opacity:[0,1]},M.fast,M.out);
  if(window.onKeySet) onKeySet(k,label);
}
function bindKeycaps(root){
  root.addEventListener('click',e=>{const k=e.target.closest('.kbd'); if(k&&!k.classList.contains('sm')&&!capturing){capturing=k; capOld=k.textContent; k.textContent='Press a key'; k.classList.add('capture'); e.stopPropagation();}});
}
/* the capture swallows the key before the page shortcuts (Esc, Ctrl K, Enter) see it */
addEventListener('keydown',e=>{ if(!capturing) return; e.preventDefault(); e.stopImmediatePropagation();
  if(e.key==='Escape') endCapture(capOld); else if(!['Shift','Control','Alt','Meta'].includes(e.key)) endCapture(keyName(e)); },true);
addEventListener('mousedown',e=>{ if(capturing&&(e.button===3||e.button===4)){e.preventDefault(); endCapture('Mouse '+(e.button+1));} },true);
addEventListener('click',e=>{ if(capturing&&!e.target.closest('.kbd.capture')) endCapture(capOld); });
</script>"""

ZONE_JS = r"""<script>
/* Zone editor, shared by 11 Zones, setup step 2, 16 Zone edit and the Zones layer of 20 Motion.
   Zones live in picture space, 1600 x 900, and render as percentages, so the same code works at any stage size
   (and while the stage grows). Drag on empty picture: a new box (kind = the kind shown in the picker).
   Click a box: select it. Drag it: move. Drag a handle: resize. Delete or Backspace: remove.
   "Draw zone" (when there is a toolbar) lets a drag start on top of another box. */
const KN={dialogue:'Dialogue',speaker:'Speaker',pickups:'Pickups',prompts:'Prompts',ignore:'Ignore'};
const HANDLES=[['nw',0,0],['n',50,0],['ne',100,0],['w',0,50],['e',100,50],['sw',0,100],['s',50,100],['se',100,100]];
function ZoneEd(o){
  const L=o.layer, bar=o.bar||null, P=o.picker||null;
  const seg=(bar&&bar.querySelector('.seg.kinds'))||(P&&P.querySelector('.seg.kinds'));
  const trash=bar&&bar.querySelector('.zt-trash'), drawB=bar&&bar.querySelector('.zt-draw'), cam=bar&&bar.querySelector('.zt-cam');
  const E={zones:o.zones.map(z=>({...z})),sel:o.sel??-1,kind:'dialogue',drawMode:false,enabled:o.enabled??true};
  const pct=z=>`left:${z.x1/16}%;top:${z.y1/9}%;width:${(z.x2-z.x1)/16}%;height:${(z.y2-z.y1)/9}%`;
  const clamp=(v,a,b)=>Math.max(a,Math.min(b,v));
  const node=i=>L.querySelector(`.zz[data-i="${i}"]`);
  function render(){
    L.querySelectorAll('.zz').forEach(n=>n.remove());
    E.zones.forEach((z,i)=>{const d=document.createElement('span'); d.className='zz'+(i===E.sel?' sel':''); d.dataset.k=z.k; d.dataset.i=i; d.style.cssText=pct(z);
      const del=i===E.sel&&o.del?`<span class="zdel" title="Delete zone">${ICON.x}</span>`:'';
      d.innerHTML=`<span class="ztag">${ICON['k-'+z.k]}${KN[z.k]}<span class="tag zoff">off</span>${del}</span>`+
        (i===E.sel?HANDLES.map(([h,x,y])=>`<i data-h="${h}" style="left:${x}%;top:${y}%"></i>`).join(''):'');
      L.append(d); tagSide(d,z);});
    if(P) L.append(P);
    sync(); if(o.onRender) o.onRender();
  }
  /* The tag sits above its box. At the top of the picture there is no room, so it goes under the box;
     if there is no room under it either, inside the box at the top left. Rust: the same test on the zone rect. */
  const TAGH=28;
  function tagSide(d,z){ const Hh=L.offsetHeight/900;
    const below=z.y1*Hh<TAGH, inside=below&&(900-z.y2)*Hh<TAGH;
    d.classList.toggle('tagb',below&&!inside); d.classList.toggle('tagi',inside); }
  let pickFor=-1;
  function placePicker(){
    if(E.sel<0||!E.enabled){ if(pickFor>=0){pickFor=-1; tween(P,{opacity:[null,0],s:[null,.96]},M.exit,M.in); live(P,false);} if(o.onLayout) o.onLayout(); return; }
    const z=E.zones[E.sel], W=L.offsetWidth/1600, Hh=L.offsetHeight/900, px=o.padX||0, n=node(E.sel), tb=n&&n.classList.contains('tagb');
    /* under the box (and under its tag when the tag is there too); flips above the box at the bottom edge */
    let left=clamp(z.x1*W, px+8, L.offsetWidth-px-P.offsetWidth-8), top=z.y2*Hh+8+(tb?TAGH:0), up=false;
    if(top+P.offsetHeight>L.offsetHeight-8){ top=z.y1*Hh-(tb?0:TAGH)-8-P.offsetHeight; up=true; }
    P.style.left=left+'px'; P.style.top=top+'px'; P.style.transformOrigin=up?'0 100%':'0 0';
    if(pickFor!==E.sel){pickFor=E.sel; tween(P,{opacity:[0,1],s:[.96,1]},140,M.out); live(P,true);}
    if(o.onLayout) o.onLayout();
  }
  function sync(){
    const k=E.sel>=0?E.zones[E.sel].k:E.kind;
    if(seg) [...seg.children].forEach(s=>s.classList.toggle('on',s.dataset.k===k));
    if(trash) trash.classList.toggle('disabled',E.sel<0);
    if(drawB) drawB.classList.toggle('on',E.drawMode);
    L.classList.toggle('zedit',E.enabled); L.classList.toggle('drawmode',E.drawMode);
    if(o.hint) o.hint.hidden=E.zones.length>0;
    if(P) placePicker();
  }
  function pt(e){const r=L.getBoundingClientRect(); return {x:(e.clientX-r.left)/r.width*1600, y:(e.clientY-r.top)/r.height*900};}
  let drag=null;
  L.addEventListener('mousedown',e=>{
    if(!E.enabled||e.button!==0) return;
    if(P&&P.contains(e.target)) return;
    if(e.target.closest('.zdel')){ del(); e.preventDefault(); return; }
    const zz=e.target.closest('.zz'), p=pt(e);
    if(zz&&!E.drawMode){ const i=+zz.dataset.i, h=e.target.dataset&&e.target.dataset.h;
      if(E.sel!==i){E.sel=i; render();}
      drag={mode:h||'move',i,p0:p,z0:{...E.zones[i]}}; e.preventDefault(); return; }
    const d=document.createElement('span'); d.className='zz drawing'; d.style.cssText=pct({x1:p.x,y1:p.y,x2:p.x,y2:p.y}); L.append(d);
    drag={mode:'draw',p0:p,el:d}; e.preventDefault();
  });
  addEventListener('mousemove',e=>{ if(!drag) return; const p=pt(e);
    if(drag.mode==='draw'){ const z={x1:clamp(Math.min(p.x,drag.p0.x),0,1600),y1:clamp(Math.min(p.y,drag.p0.y),0,900),x2:clamp(Math.max(p.x,drag.p0.x),0,1600),y2:clamp(Math.max(p.y,drag.p0.y),0,900)};
      drag.el.style.cssText=pct(z); drag.z=z; return; }
    const z=E.zones[drag.i], a=drag.z0, dx=p.x-drag.p0.x, dy=p.y-drag.p0.y, m=drag.mode;
    if(m==='move'){ const w=a.x2-a.x1,h=a.y2-a.y1; z.x1=clamp(a.x1+dx,0,1600-w); z.y1=clamp(a.y1+dy,0,900-h); z.x2=z.x1+w; z.y2=z.y1+h; }
    else { if(m.includes('w')) z.x1=clamp(a.x1+dx,0,a.x2-24); if(m.includes('e')) z.x2=clamp(a.x2+dx,a.x1+24,1600);
           if(m.includes('n')) z.y1=clamp(a.y1+dy,0,a.y2-16); if(m.includes('s')) z.y2=clamp(a.y2+dy,a.y1+16,900); }
    node(drag.i).style.cssText=pct(z); tagSide(node(drag.i),z); if(P) placePicker();
  });
  addEventListener('mouseup',()=>{ if(!drag) return; const d=drag; drag=null;
    if(d.mode!=='draw') return;
    d.el.remove(); const z=d.z;
    if(z&&z.x2-z.x1>=24&&z.y2-z.y1>=16){ E.zones.push({k:E.sel>=0?E.zones[E.sel].k:E.kind,...z}); E.sel=E.zones.length-1; E.drawMode=false; render(); if(o.onAdd) o.onAdd(); }
    else if(E.sel>=0){ E.sel=-1; render(); }
  });
  if(seg) seg.addEventListener('click',e=>{const s=e.target.closest('span[data-k]'); if(!s||!E.enabled) return;
    if(E.sel>=0){E.zones[E.sel].k=s.dataset.k; render();} else {E.kind=s.dataset.k; sync();}});
  if(trash) trash.addEventListener('click',()=>del());
  if(drawB) drawB.addEventListener('click',()=>{E.drawMode=!E.drawMode; sync();});
  if(cam) cam.addEventListener('click',()=>{ const pic=L.parentNode.querySelector('.sc,.ob-pic'); if(pic) tween(pic,{opacity:[.3,1]},M.base,M.out);
    const t=bar.querySelector('.time'); if(t){const d=new Date(); t.textContent=String(d.getHours()).padStart(2,'0')+':'+String(d.getMinutes()).padStart(2,'0');} });
  function del(){ if(E.sel<0) return; E.zones.splice(E.sel,1); E.sel=-1; render(); }
  E.key=e=>{ if(!E.enabled) return false;
    if((e.key==='Delete'||e.key==='Backspace')&&E.sel>=0){del(); return true;}
    if(e.key==='Escape'&&E.drawMode){E.drawMode=false; sync(); return true;}
    return false; };
  E.set=z=>{E.zones=z.map(x=>({...x})); E.sel=-1; render();};
  E.enable=b=>{E.enabled=b; if(!b) E.drawMode=false; sync();};
  E.render=render; E.del=del; E.select=i=>{E.sel=i; render();};
  render(); return E;
}
</script>"""

SW_JS = r"""<script>
/* Game switcher (brief 3.1 row 7): scrim 0 to 0.6, panel opacity, y -8 to 0, scale .98 to 1. Open 180 ease-out, close 140 ease-in.
   Typing filters the list. One highlight glides to the row: 360 ms for the mouse, 120 ms for the arrow keys. */
const SW={open:false,hi:0,canOpen:()=>true,onPick:r=>{},cur:()=>0};
const ROWH=48;
function allRows(){return [...document.querySelectorAll('#swl .gr')]}
function rows(){return allRows().filter(r=>!r.hidden)}
function glide(i,fast){SW.hi=i; tween($('hl'),{y:[null,i*ROWH]},fast?M.fast:M.glide,M.strong);}
function filterSw(){
  const v=$('sw-in').value.trim().toLowerCase();
  allRows().forEach(r=>r.hidden=!!v&&!r.dataset.name.toLowerCase().includes(v));
  const n=rows().length; $('sw-empty').hidden=n>0; $('hl').style.visibility=n?'':'hidden';
  if(v&&n){SW.hi=0; tween($('hl'),{y:[0,0]},0,M.out);}
}
function openSw(){
  if(SW.open||!SW.canOpen()) return; SW.open=true; $('sw').classList.add('open'); $('sw-in').value=''; filterSw();
  tween($('sw-scrim'),{opacity:[0,.6]},M.base,M.out);
  tween($('swp'),{opacity:[0,1],y:[-8,0],s:[.98,1]},M.base,M.out);
  const c=SW.cur(); SW.hi=c; tween($('hl'),{y:[c*ROWH,c*ROWH]},0,M.out);
  setTimeout(()=>{ if(SW.open) $('sw-in').focus({preventScroll:true}); },0);
}
function closeSw(){
  if(!SW.open) return; SW.open=false; $('sw').classList.remove('open'); $('sw-in').blur();
  tween($('sw-scrim'),{opacity:[null,0]},M.exit,M.in);
  tween($('swp'),{opacity:[null,0],y:[null,-8],s:[null,.98]},M.exit,M.in);
}
function pickSw(r){ if(!r) return; closeSw(); SW.onPick(r); }
allRows().forEach(r=>{ r.onmouseenter=()=>{const i=rows().indexOf(r); if(i>=0&&i!==SW.hi) glide(i,false)}; r.onclick=()=>pickSw(r); });
$('sw-in').addEventListener('input',filterSw);
$('sw-scrim').onclick=()=>closeSw();
$('add-game').onclick=()=>closeSw();
/* returns true when the switcher used the key */
function swKey(e){
  if((e.ctrlKey||e.metaKey)&&(e.key==='k'||e.key==='K')){e.preventDefault(); SW.open?closeSw():openSw(); return true;}
  if(!SW.open) return false;
  if(e.key==='Escape'){closeSw(); return true;}
  const n=rows().length;
  if((e.key==='ArrowDown'||e.key==='ArrowUp')&&n){e.preventDefault(); glide((SW.hi+(e.key==='ArrowDown'?1:n-1))%n,true); return true;}
  if(e.key==='Enter'){pickSw(rows()[SW.hi]); return true;}
  return false;
}
/* the game name rolls: old text goes up 8 px and fades (100 in), new text comes from below (180 out, 60 later) */
function rollName(name,d=0){
  const roll=document.querySelector('.gname .roll'), old=$('gname-t');
  const nu=old.cloneNode(); nu.id='gname-t'; nu.textContent=name; old.id='';
  nu.style.position='absolute'; nu.style.left='0'; nu.style.top='0'; roll.append(nu);
  roll.style.width=Math.max(old.offsetWidth,nu.offsetWidth)+'px';
  tween(nu,{opacity:[0,0],y:[8,8]},0,M.out);
  tween(old,{opacity:[1,0],y:[0,-8]},100,M.in,d).onfinish=()=>{old.remove(); nu.style.position=''; roll.style.width='';};
  tween(nu,{opacity:[0,1],y:[8,0]},180,M.out,d+60);
}
</script>"""

def js_consts():
    icons = ["k-dialogue", "k-speaker", "k-pickups", "k-prompts", "k-ignore", "replay", "x", "check", "play", "stop"]
    return "<script>const ICON={" + ",".join(f"'{k}':'{ic(k)}'" for k in icons) + "};</script>"

def zones_json(zones, off=()):
    import json
    return json.dumps([{"k": k, "x1": x1, "y1": y1, "x2": x2, "y2": y2} for k, n, x1, y1, x2, y2 in zones])

MOCK_SLOW = '<label><input type="checkbox" id="m-slow"> Slow motion x4</label>'

# ---------- shared CSS ----------
SW_CSS = """
.sw{position:absolute;inset:0;z-index:30;pointer-events:none}
.sw.open{pointer-events:auto}
.scrim{position:absolute;inset:40px 0 0 0;background:#000;opacity:0}
.swp{position:absolute;left:12px;top:44px;width:480px;border-radius:var(--r-lg);background:var(--surface-1);border:1px solid rgba(255,255,255,.1);box-shadow:0 12px 24px rgba(0,0,0,.6);opacity:0;transform-origin:30% 0;overflow:hidden}
.sws{display:flex;align-items:center;gap:12px;height:var(--h-media);padding:0 12px 0 16px;border-bottom:1px solid var(--line);color:var(--text-2)}
.sws .ic{color:var(--text-2)}
.sws input{flex:1;min-width:0;height:100%;background:transparent;border:0;outline:0;color:var(--text);font:inherit;caret-color:var(--text)}
.sws input::placeholder{color:var(--text-2);opacity:1}
.swl{position:relative;padding:4px}
.sw-empty{height:var(--h-media);display:flex;align-items:center;padding:0 12px;color:var(--text-2)}
.sw-empty[hidden],.gr[hidden]{display:none}
.swf{padding:4px;border-top:1px solid var(--line)}
.hl{position:absolute;left:4px;right:4px;top:4px;height:var(--h-media);border-radius:var(--r);background:var(--surface-3)}
.gr{position:relative;display:flex;align-items:center;gap:12px;height:var(--h-media);padding:0 12px 0 8px;border-radius:var(--r);cursor:pointer;transition:transform var(--t-fast) var(--ease-out)}
.gr .gn{flex:1}
.gr .gm{color:var(--text-2);font-size:var(--fs-small)}
.thumb{width:48px;height:27px;border-radius:var(--r-sm);overflow:hidden;flex:none;display:block;border:1px solid var(--line)}
.thumb svg{display:block;width:100%;height:100%}
.thumb.plus{display:grid;place-items:center;color:var(--text-2);background:var(--surface-2)}
.add .gn{color:var(--text-2)}
.add:hover{background:var(--surface-2)}
.add:hover .gn{color:var(--text)}
"""

ZN_CSS = """
.zn-tb{position:absolute;left:32px;right:32px;top:52px;height:32px;display:flex;align-items:center;gap:8px}
.zn-tb .vsep{width:1px;height:20px;background:var(--line);margin:0 4px}
.zn-tb .time{color:var(--text-2);margin:0 8px 0 0}
.zn-tb .btn.on{background:var(--surface-3);border-color:var(--on)}
.seg.kinds span{gap:8px}
.seg.kinds span .ic{width:14px;height:14px}
.seg.kinds span.on .ic{color:var(--on-ink)}
.zlayer.zedit{cursor:crosshair}
.zlayer.zedit .zz{cursor:move}
.zlayer.drawmode .zz{cursor:crosshair}
.zz i[data-h=nw],.zz i[data-h=se]{cursor:nwse-resize}
.zz i[data-h=ne],.zz i[data-h=sw]{cursor:nesw-resize}
.zz i[data-h=n],.zz i[data-h=s]{cursor:ns-resize}
.zz i[data-h=w],.zz i[data-h=e]{cursor:ew-resize}
.zz.drawing{border:1px solid var(--on);background:rgba(255,255,255,.08);box-shadow:0 0 0 1px rgba(0,0,0,.6)}
/* delete sits at the end of the selected box's tag, clear of the resize handles */
.zdel{display:grid;place-items:center;width:18px;height:18px;margin:0 -4px 0 4px;border-radius:var(--r-sm);color:var(--on-ink);cursor:pointer;transition:background var(--t-fast) var(--ease-out)}
.zdel .ic{width:12px;height:12px}
.zdel:hover{background:rgba(0,0,0,.1)}
.zhint{position:absolute;left:50%;top:50%;transform:translate(-50%,-50%);padding:8px 16px;border-radius:var(--r-lg);background:var(--surface-1);border:1px solid rgba(255,255,255,.1);color:var(--text);pointer-events:none;z-index:3;white-space:nowrap}
.zhint[hidden]{display:none}
.zn-stage{cursor:default}
.zn-stage:hover::after{border-color:var(--line)}
.zpick{position:absolute;z-index:4;padding:4px;border-radius:var(--r-lg);background:var(--surface-1);border:1px solid rgba(255,255,255,.1);box-shadow:0 12px 24px rgba(0,0,0,.6);opacity:0;pointer-events:none;cursor:default}
"""

KIND_TIPS = {"dialogue": "Spoken lines", "speaker": "Who is talking", "pickups": "Item pop-ups", "prompts": "Button hints", "ignore": "Never read"}
def kinds_seg():
    return ('<span class="seg kinds">' + "".join(f'<span data-k="{k}" title="{KIND_TIPS[k]}">{ic("k-" + k)}{KIND[k]}</span>' for k in KIND) + '</span>')

def zn_toolbar(done=True, extra_cls="", style=""):
    """The merged Zones toolbar: draw, kind, delete | retake, time, Done. One row, on black, no box."""
    st = f' style="{style}"' if style else ""
    return (f'<div class="zn-tb {extra_cls}" id="zn-tb"{st}>'
            f'<span class="btn zt-draw" title="Draw a zone, also over another zone">{ic("draw")}Draw zone</span><span class="vsep"></span>'
            f'{kinds_seg()}<span class="iconbtn zt-trash" title="Delete zone">{ic("trash")}</span>'
            f'<span class="spacer"></span><span class="iconbtn zt-cam" title="Retake screenshot">{ic("camera")}</span><span class="time">21:30</span>'
            + ('<span class="btn primary" id="zn-done">Done</span>' if done else '') + '</div>')

# Zones page geometry, shared with 20-motion (the stage grows from R_OV to R_ZN)
R_OV = dict(left=24, top=165, width=1056, height=594)
R_ZN = dict(left=32, top=104, width=1376, height=774)
def rect_css(r):
    return f"left:{r['left']}px;top:{r['top']}px;width:{r['width']}px;height:{r['height']}px"
def rect_js(name, r):
    return f"const {name}={{left:{r['left']},top:{r['top']},width:{r['width']},height:{r['height']}}};"

# ---------- switcher ----------
GAMES = [("rdr2", "Red Dead Redemption 2", "Just now"), ("satis", "Satisfactory", "2 days ago"),
         ("p4g", "Persona 4 Golden", "Last week"), ("kcd2", "Kingdom Come: Deliverance II", "In March")]

def switcher(cur="rdr2"):
    """data-m is the last-played label. The current game shows "Reading" instead."""
    rows = "".join(f'<div class="gr" data-g="{g}" data-name="{n}" data-m="{m}">{thumb(g)}<span class="gn">{n}</span><span class="gm">{"Reading" if g == cur else m}</span></div>'
                   for g, n, m in GAMES)
    return (f'<div class="sw" id="sw"><div class="scrim" id="sw-scrim"></div>'
            f'<div class="swp" id="swp"><div class="sws">{ic("search")}<input id="sw-in" placeholder="Search games" spellcheck="false" autocomplete="off"><span class="kbd sm">Ctrl K</span></div>'
            f'<div class="swl" id="swl"><div class="hl" id="hl"></div>{rows}<div class="sw-empty" id="sw-empty" hidden>No games found.</div></div>'
            f'<div class="swf"><div class="gr add" id="add-game" title="Opens setup"><span class="thumb plus">{ic("plus")}</span><span class="gn">Add game</span></div></div></div></div>')

# ---------- activity ----------
ACTIVITY = [
 ("dialogue", "Arthur", "We need to get these horses moving before the snow buries us.", None, "21:42"),
 ("pickups", "", "+1 Ginseng", None, "21:42"),
 ("prompts", "", "Hold E to talk", "off", "21:42"),
 ("dialogue", "Dutch", "We ride at first light. Nobody stays behind.", None, "21:41"),
 ("dialogue", "Arthur", "I hear you.", None, "21:41"),
 ("dialogue", "Arthur", "I hear you.", "repeat", "21:41"),
 ("pickups", "", "+1 Bourbon", None, "21:41"),
 ("dialogue", "Hosea", "If we do not get the wagons through the pass before dark, we lose the whole camp to the cold, and I will not bury anyone else this winter.", None, "21:40"),
 ("dialogue", "Sadie", "Then stop talking and start digging.", None, "21:40"),
 ("pickups", "", "Donated to camp: $5.00", None, "21:40"),
 ("dialogue", "Charles", "Tracks lead north. Two riders, maybe three.", None, "21:40"),
 ("pickups", "", "+2 Arrows", None, "21:39"),
 ("dialogue", "Arthur", "Keep your voice down.", None, "21:39"),
 ("prompts", "", "Hold E to talk", "off", "21:39"),
 ("dialogue", "Javier", "The wagon wheel is cracked. It will not last the night, and the next town is a full day east of here, past the river and the old mill, so either we fix it with what we have tonight or we leave the wagon and carry the rest on the horses.", None, "21:39"),
 ("pickups", "", "+1 Venison", None, "21:38"),
 ("dialogue", "Dutch", "Have a little faith, Arthur.", None, "21:38"),
 ("dialogue", "Arthur", "Easy, girl. Easy.", None, "21:37"),
 ("pickups", "", "+1 Oregano", None, "21:37"),
 ("dialogue", "Hosea", "We should keep moving.", None, "21:36"),
 ("dialogue", "Charles", "The horses need rest.", None, "21:36"),
]

AC_CSS = """
.ac-scroll{position:absolute;left:0;right:0;top:40px;bottom:0;overflow-y:auto}
.ac-col{width:720px;margin:0 auto;padding:16px 0 48px}
.ac-head{display:flex;align-items:center;justify-content:space-between;margin:8px 0 8px 4px}
.ac-head h2{font:var(--fw-small) var(--fs-small)/16px var(--font);color:var(--text-2)}
.ac-list{position:relative;padding:4px}
.ac-list .lrow[hidden]{display:none}
.ac-list .lrow .rp{top:4px}
.ac-list .lrow:not(.off):hover .meta,.ac-list .lrow.is-hover:not(.off) .meta{opacity:0}
.ac-list .lrow .meta{min-width:32px;text-align:right}
.ac-empty{display:flex;flex-direction:column;align-items:flex-start;gap:12px;padding:16px}
.ac-empty p{color:var(--text-2)}
.ac-empty[hidden]{display:none}
"""

def activity_body():
    rows = "".join(lrow(k, w, t, False, meta=tm, tag=tag) for k, w, t, tag, tm in ACTIVITY)
    return ('<div class="ac-scroll" id="ac-scroll"><div class="ac-col">'
            '<div class="ac-head" id="ac-head"><h2>Today</h2><span class="seg" id="ac-filter"><span class="on" data-f="all">All</span><span data-f="read">Read</span><span data-f="skip" title="Kinds that are off, and repeats">Skipped</span></span></div>'
            f'<div class="panel ac-list" id="ac-list">{rows}'
            f'<div class="ac-empty" id="ac-empty" hidden><p>Nothing read yet.</p><span class="btn primary">{ic("play","fill")}Read now</span></div></div>'
            '</div></div>')

AC_JS = r"""<script>
/* Activity filter: All, Read, Skipped (rows with an off or repeat tag) */
function bindActivity(){
  $('ac-filter').onclick=e=>{const s=e.target.closest('span[data-f]'); if(!s) return;
    [...s.parentNode.children].forEach(x=>x.classList.toggle('on',x===s));
    const f=s.dataset.f; $('ac-list').querySelectorAll('.lrow:not(.lfull)').forEach(r=>{const sk=r.classList.contains('off'); r.hidden=f==='read'?sk:f==='skip'?!sk:false;});};
}
/* Activity enter (transition 6): the page comes from y +24 with a fade, the first 6 rows stagger 30 ms.
   Rows after the 6th share the 6th row's delay, so no hole opens in the middle of the list.
   The last row ends 80 + 5 x 30 + 260 = 490 ms after the click. */
function activityIn(delay){
  tween($('ac-head'),{opacity:[0,1]},M.page,M.strong,delay);
  tween($('ac-list'),{opacity:[0,1],y:[24,0]},M.page,M.strong,delay);
  [...$('ac-list').querySelectorAll('.lrow:not([hidden])')].forEach((r,i)=>tween(r,{opacity:[0,1],y:[8,0]},M.page,M.strong,delay+Math.min(i,5)*30));
}
</script>"""

# ---------- 10 overview ----------
def ov_page():
    css = OV_CSS + """
.mock .seg span{font-size:var(--fs-small)}
"""
    js = MCORE + LIST_JS + r"""
<script>
/* lime is the live accent (Max 2026-11-24) */
function apply(){
  const state=Q.get('state')||'live';
  document.querySelectorAll('[data-v]').forEach(a=>{const[k,v]=a.dataset.v.split('=');a.classList.toggle('on',state===v)});
  $('v-live').hidden=state!=='live'; $('v-off').hidden=state!=='off';
}
document.querySelectorAll('[data-v]').forEach(a=>a.onclick=()=>{const[k,v]=a.dataset.v.split('=');Q.set(k,v);apply();try{history.replaceState(null,'','?'+Q)}catch(e){}});
apply();
const LST=bindList($('list'));
/* ?hover=1: the full-text card of the first cut row, for the screenshot */
if(Q.get('hover')){ const r=[...$('list').querySelectorAll('.lrow')].find(r=>{const t=r.querySelector('.tx');return t.scrollHeight>t.clientHeight+1});
  if(r){INSTANT=true; LST.show(r); INSTANT=false;} }
</script>"""
    body = (f'<div class="app" id="v-live">{strip("ov")}{overview_body("live")}</div>'
            f'<div class="app" id="v-off" hidden>{strip("ov", status="off")}{overview_body("off")}</div>'
            '<div class="mock"><b>Mockup controls</b>'
            '<span>State</span><span class="seg"><span data-v="state=live">Reading</span><span data-v="state=off">Game not running</span></span>'
            '<span>Hover a cut line in the list to see all of it.</span></div>')
    write("10-overview.html", head("10 Overview", css) + body + js + "\n</body></html>\n")

# ---------- 12 settings ----------
ST_CSS = """
.st-scroll{position:absolute;left:0;right:0;top:40px;bottom:0;overflow-y:auto}
.st-col{width:640px;margin:0 auto;padding:16px 0 48px}
.st-col h2{font:var(--fw-small) var(--fs-small)/16px var(--font);color:var(--text-2);margin:32px 0 8px 4px}
.st-col h2:first-child{margin-top:8px}
.srow .val2{color:var(--text-2)}
.st-col .srow > .row{display:flex;align-items:center;gap:8px}
"""
T_ON, T_OFF = '<span class="toggle on"></span>', '<span class="toggle"></span>'
def srow(label, ctl, small=""):
    s = f"<small>{small}</small>" if small else ""
    return f'<div class="srow"><div class="lbl">{label}{s}</div><div class="row">{ctl}</div></div>'
def sec(title, rows, sid=""):
    return f'<h2 id="{sid}">{title}</h2><div class="panel">{"".join(rows)}</div>'
def slider(v, frac):
    return (f'<span class="slider"><span class="track"><span class="fill" style="width:{frac*100:.0f}%"></span>'
            f'<span class="knob" style="left:{frac*100:.0f}%"></span></span><span class="val">{v}</span></span>')

def settings_body():
    return ('<div class="st-scroll" id="st-scroll"><div class="st-col">'
        + sec("Startup", [srow("Start with Windows", T_ON), srow("Open minimized", T_ON), srow("Show in taskbar", T_OFF)], "s-startup")
        + sec("In-game overlay", [srow("Status pill", '<span class="seg"><span class="on">Top right</span><span>Bottom</span><span>Off</span></span>'),
                                  srow("Show speaker name", T_ON)], "s-overlay")
        + sec("Keys", [srow("Read key", '<span class="kbd" data-key="read" title="Click to change">Mouse 4</span>'),
                       srow("Menu key", '<span class="kbd" title="Click to change">F7</span>')], "s-keys")
        + sec("Voice", [srow("Narrator", f'<span class="field" style="width:200px">Adam<span style="color:var(--text-2)">US · Male</span>{ic("chev-down", "chev")}</span>'
                              f'<span class="iconbtn" title="Play a sample">{ic("play")}</span>'),
                        srow("Speed", slider("1.1x", .4)), srow("Volume", slider("80%", .8)),
                        srow("Voices", '<span class="val2">Kokoro 11 · Windows 11</span><span class="btn">Add voices</span>')], "s-voice")
        + sec("App", [srow("0.2.0, up to date", '<span class="btn">Check now</span>'), srow("Auto-update", T_ON),
                      srow("Game profile", '<span class="btn">Export</span><span class="btn">Import</span>')], "s-app")
        + '</div></div>')

def st_page():
    css = ST_CSS + ".full .app{height:auto;min-height:900px}.full .st-scroll{position:relative;top:40px;padding-bottom:40px}"
    js = "<script>if(new URLSearchParams(location.search).get('full'))document.body.classList.add('full')</script>"
    body = f'<div class="app">{strip("sub", page="Settings")}{settings_body()}</div>'
    write("12-settings.html", head("12 Settings", css) + body + js + "\n</body></html>\n")

# ---------- 20 motion ----------
MOTION_CSS = OV_CSS + ST_CSS + SW_CSS + ZN_CSS + AC_CSS + """
.layer{position:absolute;inset:0}
#st,#zn,#ac{pointer-events:none;z-index:3}
#st .st-scroll{top:40px}
.sc{will-change:opacity}
#sc-b{opacity:0}
.mock .btn{height:28px;font-size:var(--fs-small)}
.mock label{display:inline-flex;gap:8px;align-items:center}
#selftest{white-space:pre;font:12px/1.5 var(--mono);color:var(--text-2);padding:12px 24px}
"""

MOTION_JS = r"""
<script>
let page='ov', game='rdr2', switching=false, paused=false;
const ovParts=['ov-tb','ov-zs','ov-panel','frame'];
const LST=bindList($('list'));
bindList($('ac-list')); bindActivity();

/* title strip: game name <-> back + page name. Sequential: old label out 80, new label in 160 after 80. */
function stripTo(sub,name){
  const g=$('lead-game'), b=$('lead-back'), gear=$('gear');
  const out=el=>{tween(el,{opacity:[null,0]},PUSH.stripOut,M.in); live(el,false);};
  const inn=el=>{tween(el,{opacity:[null,1]},PUSH.stripIn,M.strong,PUSH.stripDelay); live(el,true);};
  if(sub){ const hidden=+getComputedStyle(b).opacity<0.05; out(g); out(gear); inn(b);
    if(hidden||INSTANT) $('back-t').textContent=name; else setTimeout(()=>{$('back-t').textContent=name},PUSH.stripOut*SLOW);
  } else { out(b); inn(g); inn(gear); }
}

/* Settings section jump: 300 ms ease-out-strong on the scroll, only when Settings is already open.
   From the Overview, Settings opens at the section and the group flashes surface-2 for 600 ms. */
function scrollTo2(el,to,ms){ if(INSTANT||REDUCE){el.scrollTop=to;return;} const from=el.scrollTop, t0=performance.now(), D=ms*SLOW;
  (function step(now){const k=Math.min(1,Math.max(0,(now-t0)/D)); el.scrollTop=from+(to-from)*EOS(k); if(k<1) requestAnimationFrame(step);})(t0);}
function flash(el,delay){ if(INSTANT) return;
  el.animate([{backgroundColor:'#121212'},{backgroundColor:'#121212',offset:.5},{backgroundColor:'#0A0A0A'}],{duration:600*SLOW,delay:REDUCE?0:delay*SLOW,easing:'linear'}); }
function gotoSection(section,delay){
  const sc=$('st-scroll'), h=$(section); if(!h) return;
  const to=Math.min(h.offsetTop-16, sc.scrollHeight-sc.clientHeight);
  if(page==='settings'&&delay===0) scrollTo2(sc,to,300); else sc.scrollTop=to;
  flash(h.nextElementSibling,delay);
}

/* 5. Overview -> Settings. Push: the Overview leaves (100 ease-in, x -16), Settings comes in 80 ms later (260 strong, from x +32). */
function overviewOut(dx){ LST.hide(); for(const id of [...ovParts,'stage']){tween($(id),{opacity:[null,0],x:[null,dx]},PUSH.out,M.in); live($(id),false);} }
function overviewIn(){ for(const id of [...ovParts,'stage']){tween($(id),{opacity:[null,1],x:[null,0]},PUSH.backIn,M.strong,PUSH.backInDelay); live($(id),true);} }
function toSettings(section){
  if(page!=='ov'||SW.open) return; page='settings';
  $('st-scroll').scrollTop=0;
  if(section) gotoSection(section,PUSH.inDelay+PUSH.in);
  overviewOut(-16);
  tween($('st'),{opacity:[null,1],x:[32,0]},PUSH.in,M.strong,PUSH.inDelay); live($('st'),true);
  stripTo(true,'Settings');
}
function fromSettings(){
  page='ov';
  tween($('st'),{opacity:[null,0],x:[null,32]},PUSH.backOut,M.in); live($('st'),false);
  overviewIn(); stripTo(false);
}

/* 6. Overview -> Activity: the Now list rises. The Overview fades (100 ease-in), Activity comes from y +24 80 ms later,
   the first 6 rows stagger 30 ms. Back: Activity leaves down 16 px (120 ease-in), the Overview comes back 80 ms later. */
function toActivity(){
  if(page!=='ov'||SW.open) return; page='activity';
  $('ac-scroll').scrollTop=0;
  overviewOut(0);
  tween($('ac'),{opacity:[0,1]},0,M.out); live($('ac'),true);
  activityIn(PUSH.inDelay);
  stripTo(true,'Activity');
}
function fromActivity(){
  page='ov';
  tween($('ac'),{opacity:[null,0],y:[null,16]},PUSH.backOut,M.in); live($('ac'),false);
  overviewIn(); stripTo(false);
}

/* 4. Overview -> Zones: the stage grows from its Overview rect to the Zones rect (the real 11-zones geometry) */
function toZones(){
  if(page!=='ov'||SW.open) return; page='zones'; LST.hide();
  const s=$('stage');
  tween(s,{left:[null,R_ZN.left],top:[null,R_ZN.top],width:[null,R_ZN.width],height:[null,R_ZN.height]},300,M.strong);
  s.style.cursor='default'; s.title='';
  tween($('ov-panel'),{opacity:[null,0],x:[null,16]},300,M.strong); live($('ov-panel'),false);
  for(const id of ['ov-tb','ov-zs','frame']){tween($(id),{opacity:[null,0]},M.exit,M.in); live($(id),false);}
  tween($('ov-deco'),{opacity:[null,0]},M.exit,M.in);
  tween($('zn'),{opacity:[0,1]},M.base,M.out,120); live($('zn-tb'),true);
  tween($('zn-deco'),{opacity:[0,1]},M.base,M.out,120);
  ZE.enable(true);
  stripTo(true,'Zones');
}
function fromZones(){
  page='ov'; ZE.enable(false); if(ZE.sel>=0) ZE.select(-1);
  const s=$('stage');
  tween(s,{left:[null,R_OV.left],top:[null,R_OV.top],width:[null,R_OV.width],height:[null,R_OV.height]},240,M.strong);
  s.style.cursor=''; s.title='Edit zones';
  tween($('zn'),{opacity:[null,0]},M.exit,M.in); live($('zn-tb'),false);
  tween($('zn-deco'),{opacity:[null,0]},M.exit,M.in);
  tween($('ov-panel'),{opacity:[null,1],x:[null,0]},240,M.strong); live($('ov-panel'),true);
  for(const id of ['ov-tb','ov-zs','frame']){tween($(id),{opacity:[null,1]},240,M.strong); live($(id),true);}
  tween($('ov-deco'),{opacity:[null,1]},240,M.strong);
  stripTo(false);
}

/* 7. Game switcher: the same markup and code as 14-game-switcher.html */
const PICK_DELAY=80;
SW.canOpen=()=>page==='ov';
SW.cur=()=>Math.max(0,rows().findIndex(r=>r.dataset.g===game));
SW.onPick=r=>switchGame(r.dataset.g,r.dataset.name,PICK_DELAY);
function markCur(){allRows().forEach(r=>{r.querySelector('.gm').textContent=r.dataset.g===game?'Reading':r.dataset.m})}

/* 8. Switch game: name rolls, picture crossfades, Now, list and zone strip fade out and in. 240 ms total.
   Only content fades here (#np, #list, #zs-items, #ov-deco-c), never the page-level containers. */
const DATA={
 rdr2:{who:'Arthur',line:'We need to get these horses moving before the snow buries us.'},
 satis:{who:'Ada',line:'Welcome back, pioneer. Your shift starts now.'},
};
let now={...DATA.rdr2};
function switchGame(g,name,d=0){
  if(g===game) return;
  if(!DATA[g]){g='satis'; name='Satisfactory';}
  if(g===game) return;
  game=g; markCur(); now={...DATA[g]}; switching=true; LST.hide();
  rollName(name,d);
  const a=$('sc-a'), b=$('sc-b'); const inc=g==='satis'?b:a, outg=g==='satis'?a:b;
  inc.style.zIndex=1; outg.style.zIndex=0;
  tween(inc,{opacity:[0,1]},180,M.out,d+60).onfinish=()=>{ if(game===g){outg.getAnimations().forEach(x=>x.cancel()); outg.style.opacity=0;} };
  ZE.set(g==='satis'?ZJ_SAT:ZJ_RDR);
  const swap={
    'np':()=>setNow(now.who,now.line),
    'list':()=>fillList(g),
    'ov-deco-c':()=>{$('ov-deco-c').innerHTML=g==='satis'?DECO_SAT:DECO_RDR; syncOutlines();},
    'zs-items':()=>{$('zs-items').innerHTML=g==='satis'?ZS_SAT:ZS_RDR; syncOutlines();},
  };
  let left=4;
  for(const id in swap){
    tween($(id),{opacity:[null,0]},100,M.in,d).onfinish=()=>{ swap[id](); tween($(id),{opacity:[0,1]},140,M.out); if(--left===0) switching=false; };
  }
}

/* Zone chips: click to turn reading of that kind on or off. The matching outlines dim with it. */
function syncOutlines(){
  const off=new Set([...document.querySelectorAll('#zs-items .zi.off')].map(c=>c.dataset.k));
  document.querySelectorAll('#ov-deco-c .zo').forEach(z=>z.classList.toggle('dim',off.has(z.dataset.k)||z.dataset.k==='ignore'));
  document.querySelectorAll('#zn-deco .zz').forEach(z=>z.classList.toggle('dim',off.has(z.dataset.k)));
}
$('zs-items').onclick=e=>{const c=e.target.closest('.zi'); if(!c) return; const off=c.classList.toggle('off');
  c.title=c.textContent+': '+(off?'off':'on'); syncOutlines();};

/* The Zones layer: the real Zones toolbar and editor (11-zones.html) */
const ZE=ZoneEd({layer:$('zn-deco'),bar:$('zn-tb'),zones:ZJ_RDR,enabled:false,onRender:()=>{ if(typeof syncOutlines==='function') syncOutlines(); }});

/* New line: the new row comes in from y -12 with a fade, rows below move down by its height. 180 ms ease-out.
   Rows are one or two lines tall, so the push distance is the new row's own height. */
const POOL={
 rdr2:[['Dutch','Arthur, we have a plan. We take the train at the pass, and nobody gets hurt if we move fast.'],['','+1 Wild carrot'],['Arthur','Stay close. The pass is narrow.'],['Charles','Tracks lead up to the ridge.'],['','+1 Pelt'],['Sadie','Let us get to work.']],
 satis:[['Ada','Power is at ninety percent.'],['','+30 Copper ore'],['Ada','The space elevator is ready.'],['','+5 Wire']],
};
let pi=0, speakT=null;
function kindIcon(who,text){return who?'k-dialogue':(text.startsWith('+')?'k-pickups':'k-prompts')}
function rowEl(who,text,off){
  const d=document.createElement('div'); d.className='lrow'+(off?' off':'');
  const k=kindIcon(who,text); const kn=k==='k-dialogue'?'Dialogue':k==='k-pickups'?'Pickups':'Prompts';
  d.innerHTML=`<span class="k" title="${kn}">${ICON[k]}</span><span class="tx">${who?'<b>'+who+':</b> ':''}${text}</span>`+(off?'<span class="tag">off</span>':`<span class="iconbtn sm rp" title="Replay">${ICON.replay}</span>`);
  return d;
}
function setNow(who,line){$('np-who').textContent=who||' '; $('np-line').textContent=line;}
function readLine(){
  if(page!=='ov'||switching||paused) return; LST.hide();
  const list=$('list'); const old=[...list.querySelectorAll('.lrow:not(.gone):not(.lfull)')];
  const r=rowEl(now.who,now.line,false); list.prepend(r); const h=r.offsetHeight;
  tween(r,{opacity:[0,1],y:[-12,0]},M.base,M.out);
  old.forEach((el,i)=>{ tween(el,{y:[-h,0]},M.base,M.out);
    if(i+1>=LIST_MAX){el.classList.add('gone'); tween(el,{opacity:[null,0]},M.exit,M.in).onfinish=()=>el.remove();}});
  const p=POOL[game][pi++%POOL[game].length];
  now={who:p[0],line:p[1]}; setNow(now.who,now.line);
  [$('np-who'),$('np-line')].forEach(el=>tween(el,{opacity:[0,1],y:[4,0]},M.base,M.out));
  speak(2400);
}
/* Waveform moves only while speech plays, flat otherwise */
function speak(ms){
  const w=$('wave'); w.classList.remove('flat'); clearInterval(speakT);
  const bars=[...w.children]; let n=0;
  speakT=setInterval(()=>{bars.forEach(b=>b.style.height=(4+Math.round(Math.random()*18))+'px'); if((n+=120)>=ms*SLOW) stopSpeak();},120);
}
function stopSpeak(){clearInterval(speakT); $('wave').classList.add('flat');}
function fillList(g){
  const list=$('list'); list.innerHTML='';
  const src=g==='satis'?[['','+12 Iron ore'],['Ada','Hub upgrade complete.'],['','+4 Concrete'],['Ada','Mind the gap, pioneer.']]:RDR_ROWS;
  src.forEach(s=>list.append(rowEl(s[0],s[1],s[2])));
}

/* Pause / Resume: the icon crossfades 120 ms (brief 3.2), the status dot follows */
function setPause(p){
  paused=p; $('pause-t').textContent=p?'Resume':'Pause';
  tween($('ic-pause'),{opacity:[null,p?0:1]},M.fast,M.out); tween($('ic-resume'),{opacity:[null,p?1:0]},M.fast,M.out);
  $('sdot').classList.toggle('paused',p); $('sdot').querySelector('.tip').textContent=p?'Paused':'Reading';
  $('read-now').classList.toggle('disabled',p); if(p) stopSpeak();
}

/* Settings controls: toggles, segmented, keycaps */
$('st').addEventListener('click',e=>{
  const t=e.target.closest('.toggle'); if(t&&!t.classList.contains('disabled')){t.classList.toggle('on'); return;}
  const s=e.target.closest('.seg > span'); if(s){[...s.parentNode.children].forEach(x=>x.classList.toggle('on',x===s)); return;}
});
bindKeycaps($('st'));
window.onKeySet=(k,label)=>{ if(k.dataset.key==='read') $('key-t').textContent=label; };

/* wiring */
$('gear').onclick=()=>toSettings();
$('lead-game').onclick=()=>SW.open?closeSw():openSw();
$('lead-back').onclick=()=>back();
$('stage').onclick=()=>toZones();
$('pencil').onclick=()=>toZones();
$('zn-done').onclick=()=>fromZones();
$('show-all').onclick=()=>toActivity();
$('read-now').onclick=()=>readLine();
$('pause').onclick=()=>setPause(!paused);
$('opt-mode').onclick=()=>{const t=$('mode-t'); t.textContent=t.textContent==='Automatic'?'On key press':'Automatic';};
$('opt-voice').onclick=()=>toSettings('s-voice');
$('opt-key').onclick=()=>toSettings('s-keys');
$('np-replay').onclick=()=>{ if(!paused) speak(2400); };
$('np-stop').onclick=()=>stopSpeak();
$('list').onclick=e=>{ if(e.target.closest('.rp')&&!paused) speak(1600); };
function back(){ if(page==='settings') fromSettings(); else if(page==='zones') fromZones(); else if(page==='activity') fromActivity(); }
addEventListener('keydown',e=>{
  if(swKey(e)) return;
  if(page==='zones'&&ZE.key(e)){e.preventDefault(); return;}
  if(e.key==='Escape'){ back(); return; }
});
$('m-line').onclick=()=>readLine();
bindSlow();
$('wave').classList.add('flat');
markCur();

/* static states for screenshots: ?s=settings|zones|switcher|activity|line, add -mid (and -back-mid) with &ms=N */
(function(){
  const s=Q.get('s');
  if(Q.get('test')) return selftest();
  const from=Q.get('from');
  if(from){ INSTANT=true; if(from==='zones') toZones(); if(from==='activity') toActivity(); INSTANT=false; setTimeout(back,200); return; }
  if(!s) return;
  const mid=s.endsWith('-mid'); let base=s.replace('-mid','');
  const backMid=base.endsWith('-back'); base=base.replace('-back','');
  if(!mid||backMid) INSTANT=true;
  if(base==='settings') toSettings(); if(base==='zones') toZones(); if(base==='activity') toActivity();
  if(base==='switcher'){openSw(); glide(1,true);}
  if(base==='line'){ readLine(); }
  if(backMid){ INSTANT=false; back(); }
  if(mid) freeze();
  INSTANT=false;
})();

/* self-test: real events through the same handlers, zero durations, result written into #selftest */
function selftest(){
  INSTANT=true; const out=[]; const ok=(n,c)=>out.push((c?'PASS ':'FAIL ')+n);
  const op=id=>+getComputedStyle($(id)).opacity;
  const key=(k,ctrl)=>dispatchEvent(new KeyboardEvent('keydown',{key:k,ctrlKey:!!ctrl,bubbles:true}));
  const mouse=(el,type,x,y)=>el.dispatchEvent(new MouseEvent(type,{bubbles:true,clientX:x,clientY:y,button:0}));
  $('gear').click(); ok('gear opens settings',page==='settings'&&op('st')===1&&op('stage')===0&&op('frame')===0&&$('back-t').textContent==='Settings');
  ok('outgoing overview takes no clicks',getComputedStyle($('stage')).pointerEvents==='none');
  const tg=document.querySelector('#st .toggle'); const was=tg.classList.contains('on'); tg.click(); ok('settings toggle clicks',tg.classList.contains('on')!==was);
  const kc=document.querySelector('#st .kbd'); kc.click(); ok('keycap starts capture',kc.classList.contains('capture'));
  key('Escape'); ok('esc cancels capture, stays in settings',page==='settings'&&kc.textContent==='Mouse 4');
  kc.click(); key('F8'); ok('new read key shows on overview',kc.textContent==='F8'&&$('key-t').textContent==='F8');
  key('Escape'); ok('esc returns from settings',page==='ov'&&op('st')===0&&op('stage')===1);
  $('stage').click(); const r=$('stage').getBoundingClientRect(); ok('stage click grows into zones',page==='zones'&&Math.round(r.width)===R_ZN.width&&op('zn')===1);
  const z0=document.querySelector('#zn-deco .zz[data-k="pickups"]').getBoundingClientRect();
  mouse(document.querySelector('#zn-deco .zz[data-k="pickups"] .ztag'),'mousedown',z0.left+4,z0.top-8); mouse(window,'mouseup',z0.left+4,z0.top-8);
  ok('click selects a zone, kind shows in the toolbar',ZE.sel>=0&&document.querySelector('#zn-tb .seg.kinds .on').dataset.k==='pickups');
  document.querySelector('#zn-tb .seg.kinds span[data-k="speaker"]').click();
  ok('kind button changes the zone',ZE.zones[ZE.sel].k==='speaker');
  const n0=ZE.zones.length; key('Delete'); ok('delete removes the zone',ZE.zones.length===n0-1&&ZE.sel===-1);
  const L=$('zn-deco').getBoundingClientRect(), x=L.left+L.width*.45, y=L.top+L.height*.3;
  mouse($('zn-deco'),'mousedown',x,y); mouse(window,'mousemove',x+120,y+60); mouse(window,'mouseup',x+120,y+60);
  ok('drag on the picture draws a zone',ZE.zones.length===n0&&ZE.sel===n0-1);
  key('Escape'); const r2=$('stage').getBoundingClientRect(); ok('esc shrinks stage back',page==='ov'&&Math.round(r2.width)===R_OV.width);
  $('pencil').click(); ok('pencil opens zones',page==='zones'); $('zn-done').click(); ok('done returns',page==='ov');
  const chip=document.querySelector('#zs-items .zi[data-k="pickups"]'); chip.click();
  ok('zone chip turns a kind off',chip.classList.contains('off')&&document.querySelector('#ov-deco-c .zo[data-k="pickups"]').classList.contains('dim'));
  chip.click();
  $('show-all').click(); ok('show all opens activity',page==='activity'&&op('ac')===1&&$('back-t').textContent==='Activity');
  document.querySelector('#ac-filter span[data-f="skip"]').click();
  ok('activity filter shows skipped only',[...$('ac-list').querySelectorAll('.lrow')].filter(r=>!r.hidden).every(r=>r.classList.contains('off')));
  document.querySelector('#ac-filter span[data-f="all"]').click();
  key('Escape'); ok('esc returns from activity',page==='ov'&&op('ac')===0&&op('stage')===1);
  const long=[...$('list').querySelectorAll('.lrow .tx')].find(t=>t.scrollHeight>t.clientHeight+1);
  ok('a long line stops after two lines',!!long&&long.clientHeight<=40);
  if(long){ LST.show(long.closest('.lrow')); const c=$('list').querySelector('.lfull'); ok('hover card shows the whole line',!!c&&c.querySelector('.tx').scrollHeight<=c.querySelector('.tx').clientHeight+1); LST.hide(); }
  $('pause').click(); ok('pause pauses',paused&&$('pause-t').textContent==='Resume'); $('pause').click(); ok('resume',!paused);
  $('lead-game').click(); ok('game name opens switcher',SW.open&&op('swp')===1);
  key('Escape'); ok('esc closes switcher first',!SW.open&&page==='ov');
  key('k',true); ok('ctrl k opens switcher',SW.open);
  $('sw-in').value='sat'; $('sw-in').dispatchEvent(new Event('input')); ok('typing filters the games',rows().length===1&&rows()[0].dataset.g==='satis');
  $('sw-in').value='zzz'; $('sw-in').dispatchEvent(new Event('input')); ok('no match shows one line',rows().length===0&&!$('sw-empty').hidden);
  $('sw-in').value=''; $('sw-in').dispatchEvent(new Event('input'));
  key('ArrowDown'); ok('arrow moves highlight',SW.hi===1);
  rows()[1].click(); ok('pick switches game',game==='satis'&&!SW.open);
  setTimeout(()=>{ ok('game name rolled',$('gname-t').textContent==='Satisfactory');
    ok('only the current game says Reading',[...document.querySelectorAll('#swl .gm')].filter(m=>m.textContent==='Reading').length===1);
    const zo=document.querySelector('#ov-deco-c .zo.live').getBoundingClientRect(); const st=document.elementsFromPoint(zo.left+1,zo.top+zo.height/2);
    const iz=st.findIndex(e=>e.classList.contains('zo')), isc=st.findIndex(e=>e.classList.contains('sc'));
    ok('outlines paint above the new picture',iz>=0&&iz<isc);
    ok('zones follow the game',ZE.zones.length===3);
    ok('strip shows kinds only',[...document.querySelectorAll('#zs-items .zi')].map(c=>c.textContent).join(',')==='Speaker,Dialogue,Pickups');
    const n1=$('list').querySelectorAll('.lrow').length; $('read-now').click();
    ok('read now adds a row',$('list').querySelectorAll('.lrow').length===n1+1);
    $('opt-key').click(); ok('key opens settings at keys',page==='settings'&&$('st-scroll').scrollTop>0);
    $('lead-back').click(); ok('back chevron returns',page==='ov');
    $('selftest').textContent+=out.join('\n')+'\nDONE';
  },50);
}
</script>"""

def motion_page():
    rdr_rows = "const RDR_ROWS=[" + ",".join(f"[{repr(w)},{repr(t)},{'true' if o else 'false'}]" for k, w, t, o in RECENT_RDR) + "];"
    consts = (f"<script>{rdr_rows}const LIST_MAX={LIST_MAX};{rect_js('R_OV', R_OV)}{rect_js('R_ZN', R_ZN)}"
              f"const DECO_RDR={repr(ov_outlines(ZONES_RDR))},DECO_SAT={repr(ov_outlines(ZONES_SAT, off=()))};"
              f"const ZJ_RDR={zones_json(ZONES_RDR)},ZJ_SAT={zones_json(ZONES_SAT)};"
              f"const ZS_RDR={repr(zs_items('rdr2'))},ZS_SAT={repr(zs_items('satis'))};</script>")
    ov = overview_body("live").replace('</div><div class="deco" id="ov-deco">', f'</div><div class="sc" id="sc-b">{SCENE_SAT}</div><div class="deco" id="ov-deco">', 1)
    assert 'id="sc-b"' in ov and 'id="zn-deco"' in ov
    body = (f'<div class="app">{strip("both")}'
            f'<div class="layer" id="ovl" style="pointer-events:none"><div style="pointer-events:auto">{ov}</div></div>'
            f'<div class="layer" id="zn" style="opacity:0">{zn_toolbar(style="pointer-events:none")}</div>'
            f'<div class="layer" id="st" style="opacity:0">{settings_body()}</div>'
            f'<div class="layer" id="ac" style="opacity:0">{activity_body()}</div>'
            f'{switcher()}</div>'
            '<div class="mock"><b>Try it</b><span>Gear: Settings</span><span>Stage or pencil: Zones</span><span>Show all: Activity</span>'
            '<span>Game name or Ctrl K: switcher</span><span>Esc or back: return</span>'
            '<span class="btn" id="m-line">New line (same as Read now)</span>'
            f'{MOCK_SLOW}</div>'
            '<div id="selftest"></div>')
    write("20-motion.html", head("20 Motion", MOTION_CSS) + body + js_consts() + consts
          + MCORE + LIST_JS + KEYCAP_JS + ZONE_JS + SW_JS + AC_JS + MOTION_JS + "\n</body></html>\n")

# ---------- 11 zones ----------
MOCK_CSS = """
.mock .btn{height:28px;font-size:var(--fs-small)}
.mock label{display:inline-flex;gap:8px;align-items:center}
.mock .seg{height:28px}
.mock .seg span{font-size:var(--fs-small)}
"""

def zones_page():
    css = OV_CSS + ZN_CSS + MOCK_CSS
    stage = (f'<div class="stage zn-stage" id="stage" style="{rect_css(R_ZN)}">'
             f'<div class="sc" id="sc-a">{SCENE_RDR}</div>'
             '<div class="deco zlayer" id="zl"></div><div class="zhint" id="zhint" hidden>Drag to box text.</div></div>')
    body = (f'<div class="app">{strip("sub", page="Zones")}{zn_toolbar()}{stage}</div>'
            '<div class="mock"><b>Try it</b><span>Drag on the picture: new zone</span><span>Click a zone: select. Drag it or a handle.</span>'
            '<span>Delete: remove</span><span class="btn" id="m-enter">Play enter (stage grows)</span>'
            '<span>Zones</span><span class="seg" id="m-state"><span class="on" data-s="five">Five</span><span data-s="none">None</span></span>'
            f'{MOCK_SLOW}<span>Done, back or Esc: return (opens 20-motion)</span></div>')
    js = r"""<script>
/* Prompts are off on the Overview, so their zone shows dimmed with "(off)" */
function dimOff(){document.querySelectorAll('#zl .zz').forEach(z=>z.classList.toggle('dim',z.dataset.k==='prompts'));}
const ZE=ZoneEd({layer:$('zl'),bar:$('zn-tb'),zones:Q.get('state')==='none'?[]:ZJ,sel:Q.get('state')==='none'?-1:1,hint:$('zhint'),onRender:dimOff});
/* ?state=top: a box at the top edge of the picture, selected. Its tag goes under the box. */
if(Q.get('state')==='top'){ ZE.zones.push({k:'pickups',x1:640,y1:4,x2:980,y2:50}); ZE.select(ZE.zones.length-1); }
document.querySelectorAll('#m-state span').forEach(s=>s.classList.toggle('on',(s.dataset.s==='none')===(Q.get('state')==='none')));
$('m-state').onclick=e=>{const s=e.target.closest('span[data-s]'); if(!s) return; [...s.parentNode.children].forEach(x=>x.classList.toggle('on',x===s)); ZE.set(s.dataset.s==='none'?[]:ZJ);};
/* Enter (transition 4, the Zones end): the stage grows from the Overview rect, the toolbar fades in 120 ms later */
function enter(){
  tween($('stage'),{left:[R_OV.left,R_ZN.left],top:[R_OV.top,R_ZN.top],width:[R_OV.width,R_ZN.width],height:[R_OV.height,R_ZN.height]},300,M.strong);
  tween($('zn-tb'),{opacity:[0,1]},M.base,M.out,120); tween($('zl'),{opacity:[0,1]},M.base,M.out,120);
}
$('m-enter').onclick=enter;
const leave=()=>{location.href='20-motion.html?from=zones'};
$('zn-done').onclick=leave; $('lead-back').onclick=leave;
addEventListener('keydown',e=>{ if(ZE.key(e)){e.preventDefault(); return;} if(e.key==='Escape') leave(); });
bindSlow();
if(Q.get('enter')){ enter(); freeze(); }
</script>"""
    consts = f"<script>{rect_js('R_OV', R_OV)}{rect_js('R_ZN', R_ZN)}const ZJ={zones_json(ZONES_RDR)};</script>"
    write("11-zones.html", head("11 Zones", css) + body + js_consts() + consts + MCORE + ZONE_JS + js + "\n</body></html>\n")

# ---------- 14 game switcher ----------
def switcher_page():
    css = OV_CSS + SW_CSS + MOCK_CSS
    body = (f'<div class="app">{strip("ov")}{overview_body("live")}{switcher()}</div>'
            '<div class="mock"><b>Try it</b><span>Game name or Ctrl K: open</span><span>Type to search. Up, Down, Enter. Esc closes.</span>'
            '<span>State</span><span class="seg" id="m-state"><span data-s="open">Open</span><span data-s="filter">Search "sat"</span><span data-s="empty">No match</span><span data-s="closed">Closed</span></span>'
            f'{MOCK_SLOW}</div>')
    js = r"""<script>
let cur='rdr2';
SW.cur=()=>Math.max(0,rows().findIndex(r=>r.dataset.g===cur));
SW.onPick=r=>{ if(r.dataset.g===cur) return; cur=r.dataset.g;
  allRows().forEach(x=>x.querySelector('.gm').textContent=x.dataset.g===cur?'Reading':x.dataset.m); rollName(r.dataset.name,80); };
$('lead-game').onclick=()=>SW.open?closeSw():openSw();
addEventListener('keydown',e=>{swKey(e)});
function state(s,instant){
  document.querySelectorAll('#m-state span').forEach(x=>x.classList.toggle('on',x.dataset.s===s));
  INSTANT=!!instant;
  if(s==='closed'){closeSw();}
  else { if(!SW.open) openSw();
    $('sw-in').value=s==='filter'?'sat':s==='empty'?'zzz':''; filterSw();
    if(s==='open') glide(1,true); }
  INSTANT=false;
}
$('m-state').onclick=e=>{const x=e.target.closest('span[data-s]'); if(x) state(x.dataset.s);};
bindSlow();
const s0=(Q.get('s')||'open').replace('-mid','');
if((Q.get('s')||'').endsWith('-mid')){ openSw(); freeze(); document.querySelectorAll('#m-state span').forEach(x=>x.classList.toggle('on',x.dataset.s==='open')); }
else state(s0,true);
</script>"""
    write("14-game-switcher.html", head("14 Game switcher", css) + body + js_consts() + MCORE + SW_JS + js + "\n</body></html>\n")

# ---------- 17 activity ----------
def activity_page():
    css = AC_CSS + MOCK_CSS
    body = (f'<div class="app">{strip("sub", page="Activity")}{activity_body()}</div>'
            '<div class="mock"><b>Try it</b><span>Hover a cut line: full text</span><span>All, Read, Skipped filter the list</span>'
            '<span class="btn" id="m-enter">Play enter</span>'
            '<span>State</span><span class="seg" id="m-state"><span class="on" data-s="lines">Lines</span><span data-s="empty">Empty</span></span>'
            f'{MOCK_SLOW}<span>Back or Esc: return (opens 20-motion)</span></div>')
    js = r"""<script>
const LST=bindList($('ac-list')); bindActivity();
function setState(s){
  document.querySelectorAll('#m-state span').forEach(x=>x.classList.toggle('on',x.dataset.s===s));
  const e=s==='empty'; $('ac-list').querySelectorAll('.lrow:not(.lfull)').forEach(r=>r.hidden=e); $('ac-empty').hidden=!e;
  $('ac-filter').style.visibility=e?'hidden':'';
}
$('m-state').onclick=e=>{const x=e.target.closest('span[data-s]'); if(x) setState(x.dataset.s);};
$('m-enter').onclick=()=>activityIn(0);
const leave=()=>{location.href='20-motion.html?from=activity'};
$('lead-back').onclick=leave;
addEventListener('keydown',e=>{ if(e.key==='Escape') leave(); });
bindSlow();
if(Q.get('state')==='empty') setState('empty');
if(Q.get('enter')){ activityIn(0); freeze(); }
if(Q.get('hover')){ const r=[...$('ac-list').querySelectorAll('.lrow')].find(r=>{const t=r.querySelector('.tx');return t.scrollHeight>t.clientHeight+1}); if(r){INSTANT=true; LST.show(r); INSTANT=false;} }
</script>"""
    write("17-activity.html", head("17 Activity", css) + body + js_consts() + MCORE + LIST_JS + AC_JS + js + "\n</body></html>\n")

# ---------- in-game overlays: the game picture fills the window ----------
PIC_CSS = """
.game{position:relative;width:1440px;height:900px;overflow:hidden;background:#000}
.gpic{position:absolute;left:-80px;top:0;width:1600px;height:900px}
.gpic .scene{width:100%;height:100%;display:block}
.float{background:var(--surface-1);border:1px solid rgba(255,255,255,.1);box-shadow:0 12px 24px rgba(0,0,0,.6)}
"""
GAME_PIC = f'<div class="gpic">{SCENE_RDR}</div>'

# ---------- 13 pill ----------
def pill_page():
    css = PIC_CSS + MOCK_CSS + """
.pill{position:absolute;display:flex;align-items:center;gap:8px;height:32px;padding:0 12px;border-radius:var(--r-pill);opacity:0;white-space:nowrap}
.pill i{width:8px;height:8px;border-radius:50%;background:var(--live);flex:none}
.pill.noname span{display:none}
.pill.tr{right:24px;top:24px}
.pill.bt{bottom:24px}
"""
    body = ('<div class="app game">' + GAME_PIC +
            '<div class="pill float tr" id="pill"><i></i><span id="pill-who">Arthur</span></div></div>'
            '<div class="mock"><b>Try it</b><span class="btn" id="m-one">Play a line</span><span class="btn" id="m-two">Two lines in a row</span>'
            '<span>Status pill</span><span class="seg" id="m-pos"><span class="on" data-p="tr">Top right</span><span data-p="bt">Bottom</span></span>'
            '<label><input type="checkbox" id="m-name" checked> Show speaker name</label>'
            f'{MOCK_SLOW}</div>')
    js = r"""<script>
/* Pill (transition 9): show 180 ease-out, opacity 0 to 1 and y 6 to 0. After the line ends: 600 ms hold, then hide 200 ease-in.
   A new line during the hold cancels the hide; only the name crossfades (120 ms). */
const P=$('pill'); let shown=false, endT=null, hideT=null;
function place(){ if(P.classList.contains('bt')) P.style.left=((1440-P.offsetWidth)/2)+'px'; else P.style.left=''; }
function say(who,ms=2400){
  clearTimeout(endT); clearTimeout(hideT);
  if(shown){ $('pill-who').textContent=who; place(); tween($('pill-who'),{opacity:[0,1]},M.fast,M.out); }
  else { $('pill-who').textContent=who; place(); shown=true; tween(P,{opacity:[0,1],y:[6,0]},M.base,M.out); }
  endT=setTimeout(()=>{ hideT=setTimeout(()=>{ shown=false; tween(P,{opacity:[null,0]},200,M.in); },600*SLOW); },ms*SLOW);
}
$('m-one').onclick=()=>say('Arthur');
$('m-two').onclick=()=>{ say('Arthur',1400); setTimeout(()=>say('Dutch'),1600*SLOW); };
$('m-pos').onclick=e=>{const s=e.target.closest('span[data-p]'); if(!s) return; [...s.parentNode.children].forEach(x=>x.classList.toggle('on',x===s));
  P.classList.remove('tr','bt'); P.classList.add(s.dataset.p); place();};
$('m-name').onchange=e=>{P.classList.toggle('noname',!e.target.checked); place();};
bindSlow();
const s=Q.get('s')||'';
if(Q.get('pos')==='bt') document.querySelector('#m-pos span[data-p="bt"]').click();
if(s==='shown'){ INSTANT=true; say('Arthur',1e9); INSTANT=false; }
if(s==='show-mid'){ say('Arthur',1e9); freeze(); }
</script>"""
    write("13-pill.html", head("13 Pill", css) + body + MCORE + js + "\n</body></html>\n")

# ---------- 15 quick menu ----------
VOICES = [("Adam", "US · Male"), ("Bella", "US · Female"), ("Michael", "US · Male"), ("Emma", "UK · Female"), ("George", "UK · Male"), ("Sky", "US · Female")]

def quick_menu_page():
    css = PIC_CSS + MOCK_CSS + """
.qm{position:absolute;left:520px;top:150px;width:400px;border-radius:var(--r-lg);transform-origin:50% 0;opacity:0;pointer-events:none}
.qm-h{display:flex;align-items:center;gap:8px;height:48px;padding:0 16px;border-bottom:1px solid var(--line)}
.qm-h .g{flex:1;font-weight:var(--fw-strong);white-space:nowrap;overflow:hidden;text-overflow:ellipsis}
.qm-h .st{display:flex;align-items:center;gap:8px;color:var(--text-2);font-size:var(--fs-small)}
.qm-h .st i{width:8px;height:8px;border-radius:50%;background:var(--live)}
.qm-h .st.stopped i{background:var(--text-2)}
.ql{position:relative;padding:4px}
.ql .sep{height:1px;background:var(--line);margin:4px -4px}
.qhl{position:absolute;left:4px;right:4px;top:0;height:var(--h-row);border-radius:var(--r);background:var(--surface-3)}
.qr{position:relative;display:flex;align-items:center;gap:12px;height:var(--h-row);padding:0 8px 0 12px;border-radius:var(--r);cursor:pointer;user-select:none}
.qr > .ic,.qr > .swapic{color:var(--text-2)}
.qr .lb{flex:1;white-space:nowrap}
.qr .v{display:flex;align-items:center;gap:4px;color:var(--text-2);white-space:nowrap}
.qr .v > .ic{width:14px;height:14px}
.qr .slider .track{width:112px}
.qr .slider .val{color:var(--text)}
.qr.armed > .ic,.qr.armed .lb,.qr.armed .v{color:var(--danger)}
.qv{position:absolute;width:248px;border-radius:var(--r-lg);opacity:0;pointer-events:none}
.qv .qr,.qv .qhl{height:var(--h-list)}
.qv .ql .qr .meta{color:var(--text-2);font-size:var(--fs-small)}
.qv .qr .ck{width:16px;height:16px;color:var(--text)}
.qv .qr .ck .ic{width:16px;height:16px}
"""
    def qrow(a, icon, label, val="", cls="", rid=""):
        i = f' id="{rid}"' if rid else ""
        return f'<div class="qr {cls}" data-a="{a}"{i}>{icon}<span class="lb">{label}</span><span class="v">{val}</span></div>'
    SEP = '<div class="sep"></div>'
    def ids(sl, n):
        return sl.replace('class="fill"', f'class="fill" id="{n}-f"').replace('class="knob"', f'class="knob" id="{n}-k"').replace('class="val"', f'class="val" id="{n}-v"')
    stop_ic = f'<span class="swapic">{ic("stop", id="ic-stop")}{ic("play", "fill", id="ic-resume")}</span>'
    rows = (qrow("zones", ic("draw"), "Edit zones", ic("chev-right")) + SEP
            + qrow("stop", stop_ic, '<span id="stop-t">Stop</span>')
            + qrow("mute", ic("volume-x"), "Mute", '<span class="toggle" id="mute-t"></span>') + SEP
            + qrow("voice", ic("k-speaker"), "Voice", f'<span id="voice-v">Adam</span>{ic("chev-right")}', rid="voice-row")
            + qrow("speed", ic("gauge"), "Speed", ids(slider("1.1x", .4), "speed"))
            + qrow("vol", ic("volume"), "Volume", ids(slider("80%", .8), "vol"))
            + qrow("mode", ic("zap"), "Mode", '<span class="seg" id="mode-s"><span class="on" data-m="0">Automatic</span><span data-m="1">On key press</span></span>') + SEP
            + qrow("settings", ic("settings"), "Settings", ic("external"))
            + qrow("quit", ic("power"), "Quit", '<span id="quit-v"></span>', rid="quit-row"))
    vrows = "".join(f'<div class="qr" data-v="{n}"><span class="ck">{ic("check") if n == "Adam" else ""}</span><span class="lb">{n}</span><span class="meta">{m}</span></div>' for n, m in VOICES)
    body = ('<div class="app game">' + GAME_PIC +
            '<div class="qm float" id="qm"><div class="qm-h"><span class="g">Red Dead Redemption 2</span><span class="st" id="qm-st"><i></i><span id="qm-st-t">Reading</span></span></div>'
            f'<div class="ql" id="ql"><div class="qhl" id="qhl"></div>{rows}</div></div>'
            f'<div class="qv float" id="qv"><div class="ql" id="vl"><div class="qhl" id="vhl"></div>{vrows}</div></div></div>'
            '<div class="mock"><b>Try it</b><span class="btn" id="m-key">Menu key (F7)</span><span>Up, Down: move. Left, Right: change. Enter: choose. Esc: close.</span>'
            '<span>State</span><span class="seg" id="m-state"><span data-s="open">Open</span><span data-s="voice">Voice list</span><span data-s="stopped">Stopped</span><span data-s="quit">Quit pressed once</span></span>'
            f'{MOCK_SLOW}</div>')
    js = r"""<script>
/* Quick menu (transition 10): open 160 ease-out, opacity and scale .97 to 1 from the top centre. Close 120 ease-in.
   The voice list opens beside the Voice row: opacity and x -8 to 0. One highlight glides: 360 ms mouse, 120 ms keys. */
let open=false, sub=false, hi=0, vhi=0, stopped=false, armed=false, speed=1.1, vol=80, mode=0, voice='Adam';
const qm=$('qm'), qv=$('qv');
const QR=()=>[...$('ql').querySelectorAll('.qr')], VR=()=>[...$('vl').querySelectorAll('.qr')];
function hlTo(el,row,fast){ tween(el,{y:[null,row.offsetTop]},fast===0?0:fast?M.fast:M.glide,M.strong); }
function setHi(i,fast){ if(i!==hi) disarm(); hi=i; hlTo($('qhl'),QR()[i],fast); }
function setVhi(i,fast){ vhi=i; hlTo($('vhl'),VR()[i],fast); }
function openMenu(){ if(open) return; open=true; live(qm,true); tween(qm,{opacity:[0,1],s:[.97,1]},160,M.out); setHi(0,0); }
function closeMenu(){ if(!open) return; if(sub) closeSub(); open=false; live(qm,false); tween(qm,{opacity:[null,0],s:[null,.97]},120,M.in); disarm(); }
function openSub(){
  if(sub) return; sub=true; const r=$('voice-row');
  qv.style.left=(qm.offsetLeft+qm.offsetWidth+8)+'px'; qv.style.top=(qm.offsetTop+r.offsetTop+$('ql').offsetTop-4)+'px';
  live(qv,true); tween(qv,{opacity:[0,1],x:[-8,0]},160,M.out);
  setVhi(Math.max(0,VR().findIndex(x=>x.dataset.v===voice)),0);
}
function closeSub(){ if(!sub) return; sub=false; live(qv,false); tween(qv,{opacity:[null,0],x:[null,-8]},120,M.in); }
function disarm(){ if(!armed) return; armed=false; $('quit-row').classList.remove('armed'); $('quit-v').textContent=''; }
function pickVoice(i){ const r=VR()[i]; voice=r.dataset.v; $('voice-v').textContent=voice;
  VR().forEach(x=>x.querySelector('.ck').innerHTML=x===r?ICON.check:''); const ck=r.querySelector('.ck .ic'); if(ck) tween(ck,{s:[.6,1]},M.base,M.out); closeSub(); }
function setVol(v){ vol=Math.max(0,Math.min(100,v)); $('vol-f').style.width=vol+'%'; $('vol-k').style.left=vol+'%'; $('vol-v').textContent=vol+'%'; }
/* Speed: the same slider as Settings. The app's range, 0.5x to 2.0x in steps of 0.1 (SPEED_MIN, SPEED_MAX in quick_menu.rs) */
function setSpeed(v){ speed=Math.max(.5,Math.min(2,Math.round(v*10)/10)); const f=Math.round((speed-.5)/1.5*100);
  $('speed-f').style.width=f+'%'; $('speed-k').style.left=f+'%'; $('speed-v').textContent=speed.toFixed(1)+'x'; }
function setMode(m){ mode=m; $('mode-s').querySelectorAll('span').forEach(s=>s.classList.toggle('on',+s.dataset.m===m)); }
function act(a,dir){
  switch(a){
    case 'zones': closeMenu(); break;
    case 'stop': stopped=!stopped; $('stop-t').textContent=stopped?'Resume':'Stop';
      tween($('ic-stop'),{opacity:[null,stopped?0:1]},M.fast,M.out); tween($('ic-resume'),{opacity:[null,stopped?1:0]},M.fast,M.out);
      $('qm-st').classList.toggle('stopped',stopped); $('qm-st-t').textContent=stopped?'Stopped':'Reading'; break;
    case 'mute': $('mute-t').classList.toggle('on'); break;
    case 'voice': openSub(); break;
    case 'speed': setSpeed(speed+(dir||0)*.1); break;
    case 'vol': setVol(vol+(dir||0)*10); break;
    case 'mode': setMode(dir?(dir>0?1:0):1-mode); break;
    case 'settings': closeMenu(); break;
    case 'quit': if(armed){closeMenu();} else {armed=true; $('quit-row').classList.add('armed'); $('quit-v').textContent='Press again to quit'; tween($('quit-v'),{opacity:[0,1]},M.fast,M.out);} break;
  }
}
QR().forEach((r,i)=>{ r.onmouseenter=()=>{ if(i!==hi) setHi(i,false); if(sub&&r.dataset.a!=='voice') closeSub(); };
  r.onclick=e=>{ const m=e.target.closest('#mode-s span'); if(m){setMode(+m.dataset.m); return;}
    const t=e.target.closest('.track'); if(t){const b=t.getBoundingClientRect(), f=Math.max(0,Math.min(1,(e.clientX-b.left)/b.width));
      if(r.dataset.a==='vol') setVol(Math.round(f*10)*10); else setSpeed(.5+f*1.5); return;}
    act(r.dataset.a); }; });
VR().forEach((r,i)=>{ r.onmouseenter=()=>{ if(i!==vhi) setVhi(i,false); }; r.onclick=()=>pickVoice(i); });
addEventListener('keydown',e=>{
  if(e.key==='F7'){e.preventDefault(); open?closeMenu():openMenu(); return;}
  if(!open) return;
  const ADJ=['speed','vol','mode'];
  if(e.key==='Escape'){ sub?closeSub():closeMenu(); return; }
  if(e.key==='ArrowDown'||e.key==='ArrowUp'){ e.preventDefault(); const d=e.key==='ArrowDown'?1:-1;
    if(sub){const n=VR().length; setVhi((vhi+d+n)%n,true);} else {const n=QR().length; setHi((hi+d+n)%n,true);} return; }
  const a=QR()[hi].dataset.a;
  if(e.key==='ArrowLeft'){ e.preventDefault(); if(sub) closeSub(); else if(ADJ.includes(a)) act(a,-1); return; }
  if(e.key==='ArrowRight'){ e.preventDefault(); if(!sub&&a==='voice') openSub(); else if(!sub&&ADJ.includes(a)) act(a,1); return; }
  if(e.key==='Enter'||e.key===' '){ e.preventDefault(); if(sub) pickVoice(vhi); else act(a); }
});
$('m-key').onclick=()=>open?closeMenu():openMenu();
function state(s){
  document.querySelectorAll('#m-state span').forEach(x=>x.classList.toggle('on',x.dataset.s===s));
  INSTANT=true; openMenu(); closeSub(); if(stopped) act('stop'); disarm();
  if(s==='voice'){ setHi(QR().findIndex(r=>r.dataset.a==='voice'),0); openSub(); }
  if(s==='stopped'){ act('stop'); setHi(QR().findIndex(r=>r.dataset.a==='stop'),0); }
  if(s==='quit'){ const i=QR().findIndex(r=>r.dataset.a==='quit'); setHi(i,0); act('quit'); }
  INSTANT=false;
}
$('m-state').onclick=e=>{const x=e.target.closest('span[data-s]'); if(x) state(x.dataset.s);};
bindSlow();
const s=Q.get('s')||'open';
if(s.endsWith('-mid')){ if(s==='voice-mid'){INSTANT=true; openMenu(); setHi(QR().findIndex(r=>r.dataset.a==='voice'),0); INSTANT=false; openSub();} else openMenu(); freeze(); }
else if(s!=='closed') state(s);
</script>"""
    write("15-quick-menu.html", head("15 Quick menu", css) + body + js_consts() + MCORE + js + "\n</body></html>\n")

# ---------- 16 zone edit ----------
def zone_edit_page():
    css = OV_CSS + PIC_CSS + ZN_CSS + MOCK_CSS + """
.ze{position:absolute;inset:0;pointer-events:none}
.ze-shade{position:absolute;inset:0;background:rgba(0,0,0,.45);opacity:0}
.ze-layer{position:absolute;left:-80px;top:0;width:1600px;height:900px;opacity:0}
.ze-bar{position:absolute;top:24px;left:0;right:0;margin:0 auto;width:max-content;display:flex;align-items:center;gap:16px;height:48px;padding:0 8px 0 16px;border-radius:var(--r-lg);color:var(--text-2);opacity:0}
.ze-bar.bc{top:auto;bottom:24px}
.ze-bar.tr{left:auto;right:24px;margin:0}
.ze-bar.tl{right:auto;left:24px;margin:0}
/* over the shade, zones are lifted (white 8%), never darker than the game around them */
.ze-layer .zz,.ze-layer .zz.dim{background:rgba(255,255,255,.08)}
"""
    body = ('<div class="app game">' + GAME_PIC +
            '<div class="ze" id="ze"><div class="ze-shade" id="ze-shade"></div>'
            f'<div class="ze-layer zlayer" id="zel"><div class="zpick" id="zpick">{kinds_seg()}</div></div>'
            '<div class="ze-bar float" id="ze-bar"><span>Drag to add · Esc to finish</span><span class="btn primary" id="ze-done">Done</span></div></div></div>'
            '<div class="mock"><b>Try it</b><span class="btn" id="m-show">Edit zones (from the quick menu)</span>'
            '<span>Drag on the game: new zone, the kind picker pops under it. Click a zone: select. Delete: remove.</span>'
            f'{MOCK_SLOW}</div>')
    js = r"""<script>
/* Zone edit (transition 11): the shade fades in (180), the outlines and tags 60 ms later (180), all ease-out.
   The kind picker pops under the selected box: opacity and scale .96 to 1, 140 ease-out. Done or Esc: everything out 140 ease-in. */
/* The bar moves out of the way. Home is top centre. When the selected box, its tag or the kind picker would sit
   under it, the bar fades out (140 ease-in) and comes back (180 ease-out, y 8 px) at the first place clear of every
   zone, tag and the picker: bottom centre, top right, top left. When home is clear of every zone again, it goes home the same way. */
let shown=false, barAt='tc', barBusy=false;
const BAR_AT=['tc','bc','tr','tl'];
const rectOf=e=>e.getBoundingClientRect();
function selRects(){ const s=document.querySelector('#zel .zz.sel'); if(!s) return [];
  const r=[rectOf(s),rectOf(s.querySelector('.ztag'))]; if($('zpick').style.pointerEvents==='auto') r.push(rectOf($('zpick'))); return r; }
function allRects(){ const r=[]; document.querySelectorAll('#zel .zz').forEach(z=>{r.push(rectOf(z)); r.push(rectOf(z.querySelector('.ztag')));});
  if($('zpick').style.pointerEvents==='auto') r.push(rectOf($('zpick'))); return r; }
function barRect(at){ const g=rectOf(document.querySelector('.game')), w=$('ze-bar').offsetWidth, h=48;
  const top=at[0]==='t'?g.top+24:g.bottom-24-h, left=at[1]==='c'?g.left+(g.width-w)/2:at[1]==='r'?g.right-24-w:g.left+24;
  return {left:left-8,right:left+w+8,top:top-8,bottom:top+h+8}; }
const hit=(a,b)=>a.left<b.right&&a.right>b.left&&a.top<b.bottom&&a.bottom>b.top;
const clear=(at,rs)=>!rs.some(r=>hit(r,barRect(at)));
function moveBar(to){ if(to===barAt) return; barBusy=true; const b=$('ze-bar');
  tween(b,{opacity:[null,0]},M.exit,M.in).onfinish=()=>{ barAt=to; BAR_AT.forEach(c=>b.classList.toggle(c,c===to&&c!=='tc'));
    tween(b,{opacity:[0,1],y:[to[0]==='b'?8:-8,0]},M.base,M.out); barBusy=false; }; }
function keepBarClear(){ if(!shown||barBusy) return; const sel=selRects();
  if(barAt!=='tc'&&clear('tc',allRects())){ moveBar('tc'); return; }
  if(clear(barAt,sel)) return;
  const all=allRects(), to=BAR_AT.find(at=>clear(at,all))||BAR_AT.find(at=>clear(at,sel)); if(to) moveBar(to); }
const ZE=ZoneEd({layer:$('zel'),picker:$('zpick'),zones:ZJ,del:true,padX:80,enabled:false,onLayout:keepBarClear});
function show(){ if(shown) return; shown=true; live($('ze'),true);
  tween($('ze-shade'),{opacity:[0,1]},M.base,M.out);
  tween($('zel'),{opacity:[0,1]},M.base,M.out,60);
  tween($('ze-bar'),{opacity:[0,1],y:[-8,0]},M.base,M.out);
  ZE.enable(true); }
function hide(){ if(!shown) return; shown=false; live($('ze'),false); if(ZE.sel>=0) ZE.select(-1); ZE.enable(false);
  for(const id of ['ze-shade','zel']) tween($(id),{opacity:[null,0]},M.exit,M.in);
  tween($('ze-bar'),{opacity:[null,0],y:[null,-8]},M.exit,M.in); }
$('ze-done').onclick=hide; $('m-show').onclick=show;
addEventListener('keydown',e=>{ if(!shown) return; if(ZE.key(e)){e.preventDefault(); return;} if(e.key==='Escape') hide(); });
bindSlow();
const s=Q.get('s')||'shown';
if(s==='shown'||s==='new'||s==='top'){ INSTANT=true; show();
  if(s==='new'){ ZE.zones.push({k:'dialogue',x1:1010,y1:150,x2:1370,y2:262}); ZE.select(ZE.zones.length-1); }
  /* ?s=top: a box drawn at the very top, under the bar: the tag goes under the box and the bar moves to the bottom */
  if(s==='top'){ ZE.zones.push({k:'pickups',x1:640,y1:4,x2:980,y2:50}); ZE.select(ZE.zones.length-1); }
  INSTANT=false; }
if(s==='show-mid'){ show(); freeze(); }
</script>"""
    consts = f"<script>const ZJ={zones_json(ZONES_RDR)};</script>"
    write("16-zone-edit.html", head("16 Zone edit", css) + body + js_consts() + consts + MCORE + ZONE_JS + js + "\n</body></html>\n")

# ---------- 01 setup: welcome + 5 steps in one file ----------
SETUP_CSS = OV_CSS + SW_CSS + ZN_CSS + MOCK_CSS + """
.ob{position:absolute;left:0;right:0;top:40px;bottom:72px}
.step{position:absolute;inset:0;opacity:0;pointer-events:none}
.col{position:absolute;left:240px;top:48px;width:960px}
.col h1,.wl h1{font:var(--fw-title) var(--fs-title)/32px var(--font-display);letter-spacing:-.4px}
.col .sub,.wl .sub{color:var(--text-2);margin-top:4px}
.blk{margin-top:24px}
.hh{font:var(--fw-small) var(--fs-small)/16px var(--font);color:var(--text-2);margin:0 0 8px 4px}
/* footer: Back, the 5 dots, Continue. No bar, no line. */
.obf{position:absolute;left:0;right:0;bottom:0;height:72px;opacity:0;pointer-events:none}
.obf .btn{position:absolute;top:16px}
#ob-back{left:228px}
#ob-next{right:240px}
#ob-next .ic{color:var(--on-ink)}
.dots{position:absolute;left:0;right:0;top:28px;margin:0 auto;width:max-content;display:flex;gap:8px}
.dots i{display:block;width:8px;height:8px;border-radius:var(--r-pill);background:var(--text-off);transition:background var(--t-fast) var(--ease-out)}
.dots i.on{background:var(--on)}
/* welcome */
.wl{position:absolute;left:240px;width:960px;top:48px;display:flex;align-items:flex-start;gap:64px}
.wl-l{width:400px;flex:none}
.wl-steps{list-style:none;margin-top:32px;display:flex;flex-direction:column;gap:12px}
.wl-steps li{display:flex;gap:12px;align-items:center}
.wl-steps li span{width:24px;height:24px;border-radius:var(--r-pill);border:1px solid var(--line-2);display:grid;place-items:center;font-size:var(--fs-small);color:var(--text-2);flex:none}
.wl-act{display:flex;gap:8px;margin-top:32px}
.wl-r{flex:1;min-width:0}
.wl-art{position:relative;aspect-ratio:16/9;border-radius:var(--r-lg);overflow:hidden;background:var(--surface-1)}
.wl-art::after{content:"";position:absolute;inset:0;border-radius:var(--r-lg);border:1px solid var(--line);pointer-events:none}
.wl-art .scene{position:absolute;inset:0;width:100%;height:100%;display:block}
.wl-pill{position:absolute;right:12px;top:12px;display:flex;align-items:center;gap:8px;height:28px;padding:0 12px 0 10px;border-radius:var(--r-pill);background:var(--surface-1);border:1px solid rgba(255,255,255,.1);box-shadow:0 8px 24px rgba(0,0,0,.6);font-size:var(--fs-small)}
.wl-pill i{width:8px;height:8px;border-radius:50%;background:var(--live)}
/* step 1: pick the game */
.pk-s{display:flex;width:100%;cursor:text}
.pk-s .ic{color:var(--text-2)}
.pk-s input,.ob input{flex:1;min-width:0;height:100%;background:transparent;border:0;outline:0;color:var(--text);font:inherit;caret-color:var(--text)}
.ob input::placeholder{color:var(--text-2);opacity:1}
.pk-g{margin-top:24px}
.pk-g[hidden],.pr[hidden]{display:none}
.pk-l{position:relative;padding:4px}
.pk-l .hl{top:0;opacity:0}
.pr{position:relative;display:flex;align-items:center;gap:12px;height:var(--h-media);padding:0 12px 0 8px;border-radius:var(--r);cursor:pointer;transition:background var(--t-fast) var(--ease-out)}
.pr:not(.sel):hover{background:var(--surface-2)}
.pr.other{color:var(--text-2)}
.pr .wi{width:48px;height:27px;display:grid;place-items:center;color:var(--text-2);flex:none}
/* step 2: zones */
.ob-tb{position:static;margin-top:24px}
.ob-stage{position:relative;margin-top:12px;width:960px;height:540px;border-radius:var(--r-lg);overflow:hidden;background:var(--surface-1)}
.ob-stage::after{content:"";position:absolute;inset:0;border-radius:var(--r-lg);border:1px solid var(--line);pointer-events:none;z-index:5}
.ob-pic{position:absolute;inset:0}
.ob-pic .scene{width:100%;height:100%;display:block}
/* step 4: voice */
.vx{display:flex;gap:24px;align-items:flex-start}
.vcol{width:560px}
.vcol2{flex:1}
.vl{position:relative;padding:4px}
.vl .hl{top:0;height:var(--h-list)}
.vr{position:relative;display:flex;align-items:center;gap:12px;height:var(--h-list);padding:0 4px 0 12px;border-radius:var(--r);cursor:pointer;transition:background var(--t-fast) var(--ease-out)}
.vr:not(.sel):hover{background:var(--surface-2)}
.vr .meta{color:var(--text-2);font-size:var(--fs-small)}
.vr .wave{height:16px;gap:2px;display:none}
.vr .wave i{width:2px}
.vr.playing .wave{display:flex}
.vr .swapic .ic + .ic{opacity:0}
.vempty{display:flex;align-items:center;justify-content:space-between;padding:12px 8px 12px 12px;color:var(--text-2)}
.vempty[hidden],.vr[hidden]{display:none}
.vcol2 .srow .slider .track{width:120px}
/* step 5: mode */
.mcs{display:flex;gap:16px}
.mc{flex:1;padding:20px;border-radius:var(--r-lg);background:var(--surface-1);border:1px solid var(--line);display:flex;flex-direction:column;gap:4px;cursor:pointer;transition:background var(--t-fast) var(--ease-out),border-color var(--t-fast) var(--ease-out),transform var(--t-fast) var(--ease-out)}
.mc:active{transform:scale(.97)}
.mc .ic{margin-bottom:12px;color:var(--text-2);transition:color var(--t-fast) var(--ease-out)}
.mc b{font-weight:var(--fw-strong)}
.mc span{color:var(--text-2)}
.mc:hover{border-color:var(--line-2)}
.mc.on{background:var(--surface-2);border-color:var(--on)}
.mc.on .ic{color:var(--text)}
.sum{margin-top:24px;color:var(--text-2)}
#ovl{z-index:1}
"""

def setup_page():
    run = [("rdr2", "Red Dead Redemption 2"), ("satis", "Satisfactory"), ("p4g", "Persona 4 Golden")]
    other = ["Discord", "Spotify", "Zen Browser"]
    def pk_group(gid, label, rows):
        return (f'<div class="pk-g" id="pk-{gid}"><h2 class="hh">{label}</h2>'
                f'<div class="panel pk-l"><div class="hl"></div>{rows}</div></div>')
    pk_rows = (pk_group("run", "Running", "".join(f'<div class="pr" data-name="{n}">{thumb(g)}<span>{n}</span></div>' for g, n in run))
               + pk_group("other", "Other", "".join(f'<div class="pr other" data-name="{n}"><span class="wi">{ic("window")}</span><span>{n}</span></div>' for n in other)))
    art_outlines = ov_outlines(ZONES_RDR, off=())
    welcome = ('<section class="step" id="step0"><div class="wl">'
        '<div class="wl-l">'
        '<h1 class="sx">Hear the text in your games.</h1>'
        '<p class="sub sx">Reads game dialogue and pop-ups aloud. Runs on your PC.</p>'
        '<ol class="wl-steps sx"><li><span>1</span>Pick your game</li><li><span>2</span>Mark where text shows up</li><li><span>3</span>Choose a key, a voice and a mode</li></ol>'
        f'<div class="wl-act sx"><span class="btn primary" id="ob-start">Get started</span><span class="btn">{ic("upload")}Import settings</span></div>'
        '</div>'
        f'<div class="wl-r sx"><div class="wl-art">{SCENE_RDR}<div class="deco">{art_outlines}</div>'
        '<div class="wl-pill"><i></i>Arthur</div></div></div>'
        '</div></section>')
    s1 = ('<section class="step" id="step1"><div class="col">'
        '<h1 class="sx">Pick your game</h1><p class="sub sx">Start your game, then pick it.</p>'
        f'<div class="pk blk sx" id="pk-l"><label class="field pk-s">{ic("search")}<input id="pk-in" placeholder="Search windows" spellcheck="false" autocomplete="off"></label>'
        f'{pk_rows}</div>'
        '</div></section>')
    s2 = ('<section class="step" id="step2"><div class="col">'
        '<h1 class="sx">Mark where text shows up</h1><p class="sub sx">Drag to box text. Pick its kind.</p>'
        f'<div class="sx">{zn_toolbar(done=False, extra_cls="ob-tb")}</div>'
        f'<div class="ob-stage sx"><div class="ob-pic">{SCENE_RDR}</div><div class="deco zlayer" id="ob-zl"></div><div class="zhint" id="ob-hint" hidden>Drag to box text.</div></div>'
        '</div></section>')
    s3 = ('<section class="step" id="step3"><div class="col">'
        '<h1 class="sx">Your keys</h1><p class="sub sx">Click a key to change it.</p>'
        '<div class="panel blk sx" id="keys">'
        + srow("Read key", '<span class="kbd" data-key="read" id="k-read">Mouse 4</span>')
        + srow("Menu key", '<span class="kbd" id="k-menu">F7</span>')
        + '</div></div></section>')
    vrows = "".join(f'<div class="vr" data-v="{n}"><span>{n}</span><span class="meta">{m}</span><span class="spacer"></span>{wave(n=8).replace(" id=" + chr(34) + "wave" + chr(34), "")}'
                    f'<span class="iconbtn sm vplay" title="Play a sample"><span class="swapic">{ic("play")}{ic("stop")}</span></span></div>' for n, m in VOICES)
    s4 = ('<section class="step" id="step4"><div class="col">'
        '<h1 class="sx">Pick a voice</h1><p class="sub sx">Runs offline.</p>'
        '<div class="vx blk sx"><div class="vcol"><h2 class="hh">Voices</h2>'
        f'<div class="panel vl" id="vl"><div class="hl" id="v-hl"></div>{vrows}'
        '<div class="vempty" id="vempty" hidden><span>No voices found.</span><span class="btn">Add voices</span></div></div></div>'
        '<div class="vcol2"><h2 class="hh">Sound</h2><div class="panel">'
        + srow("Speed", slider("1.1x", .4)) + srow("Volume", slider("80%", .8))
        + '</div></div></div></div></section>')
    s5 = ('<section class="step" id="step5"><div class="col">'
        '<h1 class="sx">How should it read?</h1>'
        f'<div class="mcs blk sx" id="mcs"><div class="mc on" data-m="auto">{ic("zap", "lg")}<b>Automatic</b><span>Reads new text as it appears.</span></div>'
        f'<div class="mc" data-m="key">{ic("key", "lg")}<b>On key press</b><span>Reads when you press your key.</span></div></div>'
        '<p class="sum sx" id="sum">5 zones · Mouse 4 · Adam 1.1x</p>'
        '</div></section>')
    footer = (f'<footer class="obf" id="obf"><span class="btn ghost" id="ob-back">{ic("chev-left")}Back</span>'
              '<span class="dots" id="dots"><i></i><i></i><i></i><i></i><i></i></span>'
              f'<span class="btn primary" id="ob-next">Continue{ic("arrow-right")}</span></footer>')
    ov = f'<div class="layer" id="ovl" style="position:absolute;inset:0;opacity:0;pointer-events:none">{overview_body("live")}</div>'
    body = (f'<div class="app">{strip("setup")}{ov}<div class="ob" id="ob">{welcome}{s1}{s2}{s3}{s4}{s5}</div>{footer}</div>'
            '<div class="mock"><b>Try it</b><span>Get started, Back, Continue, Enter</span><span class="btn" id="m-restart">Restart</span>'
            '<span>Voices</span><span class="seg" id="m-voices"><span class="on" data-v="1">Found</span><span data-v="0">None</span></span>'
            f'{MOCK_SLOW}</div>')
    js = r"""<script>
/* Setup flow (transitions 1, 2 and 3).
   Step to step: the old step fades out (100 ease-in) and moves 16 px away, the new step slides in 80 ms later from 24 px
   (260 ease-out-strong), its parts 40 ms apart (at most 6). Forward comes from the right, Back from the left.
   Back uses the back push: out 120 ease-in, in 200 ease-out-strong 80 ms later.
   The active dot widens 8 to 20 px (260 ease-out-strong). Finish: the setup fades out (140 ease-in), the Overview comes in
   80 ms later: stage scale .98 to 1, then the top controls, the Now panel and the zone strip, 40 ms apart, 320 ease-out-strong. */
const STEPS=[...document.querySelectorAll('.step')];
const parts=sec=>[...sec.querySelectorAll('.sx')];
let cur=-1;
function setDots(n,instant){ [...$('dots').children].forEach((d,i)=>{const on=i===n-1; d.classList.toggle('on',on); tween(d,{width:[null,on?20:8]},instant?0:M.page,M.strong);}); }
function setNext(n){ $('ob-next').innerHTML=(n===5?'Finish':'Continue')+ICON[n===5?'check':'arrow-right']; }
function goStep(n){
  if(n===cur||n<0||n>5) return; const dir=n>cur?1:-1, a=STEPS[cur], b=STEPS[n];
  const fwd=dir>0, tOut=fwd?PUSH.out:PUSH.backOut, tIn=fwd?PUSH.in:PUSH.backIn, tDel=fwd?PUSH.inDelay:PUSH.backInDelay;
  if(a){ tween(a,{opacity:[null,0],x:[null,-16*dir]},tOut,M.in); live(a,false); }
  tween(b,{opacity:[1,1],x:[0,0]},0,M.out); live(b,true);
  parts(b).forEach((c,i)=>tween(c,{opacity:[0,1],x:[24*dir,0]},tIn,M.strong,tDel+Math.min(i,5)*M.stagger));
  const f=$('obf');
  if(n>=1&&cur<1){ tween(f,{opacity:[null,1]},tIn,M.strong,tDel); live(f,true); }
  if(n===0&&cur>=1){ tween(f,{opacity:[null,0]},tOut,M.in); live(f,false); }
  if(n>=1) setDots(n,cur<1);
  setNext(n); cur=n; if(n===5) updateSum();
}
function next(){ if(cur===5) finish(); else if(cur>=0&&cur<5) goStep(cur+1); }
function back(){ if(cur>=1&&cur<=5) goStep(cur-1); }
function finish(){
  if(cur!==5) return; cur=6;
  tween($('ob'),{opacity:[null,0]},M.exit,M.in); live($('ob'),false);
  tween($('obf'),{opacity:[null,0]},M.exit,M.in); live($('obf'),false);
  tween($('lead-brand'),{opacity:[null,0]},PUSH.stripOut,M.in);
  for(const id of ['lead-game','sdot','gear']){ tween($(id),{opacity:[null,1]},PUSH.stripIn,M.strong,PUSH.stripDelay); live($(id),true); }
  const o=$('ovl'); tween(o,{opacity:[1,1]},0,M.out); live(o,true);
  tween($('frame'),{opacity:[0,1]},320,M.strong,80);
  tween($('stage'),{opacity:[0,1],s:[.98,1]},320,M.strong,80);
  ['ov-tb','ov-panel','ov-zs'].forEach((id,i)=>tween($(id),{opacity:[0,1],y:[8,0]},320,M.strong,80+(i+1)*M.stagger));
}
$('ob-start').onclick=()=>goStep(1);
$('ob-next').onclick=next; $('ob-back').onclick=back;
addEventListener('keydown',e=>{ if(cur===2&&ZE.key(e)){e.preventDefault(); return;}
  if(e.key==='Enter'){ e.preventDefault(); if(cur===0) goStep(1); else next(); } });

/* step 1: Running and Other are two panels, each with one highlight. Inside a panel the highlight glides to the
   picked row (360 ms). Into the other panel: the old fill fades out and the new one fades in on the row (120 ms each).
   Typing filters the rows; a group with no rows left hides with its label. */
const PR=()=>[...$('pk-l').querySelectorAll('.pr')];
const hlOf=r=>r.closest('.pk-l').querySelector('.hl');
let picked=null;
function pick(r,fast){ const old=picked; PR().forEach(x=>x.classList.toggle('sel',x===r)); picked=r; const h=hlOf(r); h.style.visibility='';
  if(old&&hlOf(old)===h){ tween(h,{y:[null,r.offsetTop],opacity:[null,1]},fast===0?0:M.glide,M.strong); return; }
  if(old) tween(hlOf(old),{opacity:[null,0]},fast===0?0:M.fast,M.in);
  tween(h,{y:[r.offsetTop,r.offsetTop]},0,M.out); tween(h,{opacity:[null,1]},fast===0?0:M.fast,M.out); }
PR().forEach(r=>r.onclick=()=>pick(r));
$('pk-in').addEventListener('input',()=>{
  const v=$('pk-in').value.trim().toLowerCase();
  PR().forEach(r=>r.hidden=!!v&&!r.dataset.name.toLowerCase().includes(v));
  document.querySelectorAll('.pk-g').forEach(g=>g.hidden=![...g.querySelectorAll('.pr')].some(r=>!r.hidden));
  const h=hlOf(picked); if(picked.hidden) h.style.visibility='hidden'; else { h.style.visibility=''; tween(h,{y:[null,picked.offsetTop]},0,M.out); }
});

/* step 2: the same zone editor and toolbar as 11-zones */
const ZE=ZoneEd({layer:$('ob-zl'),bar:$('zn-tb'),zones:ZJ,hint:$('ob-hint')});

/* step 3: keycaps */
bindKeycaps($('keys'));

/* step 4: voices. The picked voice gets the glide fill. The play button previews: icon crossfade, a small live waveform for 2 s. */
const VRS=()=>[...$('vl').querySelectorAll('.vr')];
let voice=VRS()[0], playT=null, waveT=null;
function pickV(r,fast){ VRS().forEach(x=>x.classList.toggle('sel',x===r)); voice=r; tween($('v-hl'),{y:[null,r.offsetTop]},fast===0?0:M.glide,M.strong); }
function stopPlay(){ clearTimeout(playT); clearInterval(waveT); VRS().forEach(r=>{ if(r.classList.contains('playing')){ r.classList.remove('playing'); const [p,s]=r.querySelectorAll('.swapic .ic'); tween(p,{opacity:[null,1]},M.fast,M.out); tween(s,{opacity:[null,0]},M.fast,M.out);} }); }
function play(r){ const was=r.classList.contains('playing'); stopPlay(); if(was) return;
  r.classList.add('playing'); const [p,s]=r.querySelectorAll('.swapic .ic'); tween(p,{opacity:[null,0]},M.fast,M.out); tween(s,{opacity:[null,1]},M.fast,M.out);
  const bars=[...r.querySelectorAll('.wave i')]; waveT=setInterval(()=>bars.forEach(b=>b.style.height=(3+Math.round(Math.random()*12))+'px'),120);
  playT=setTimeout(stopPlay,2000*SLOW); }
VRS().forEach(r=>r.onclick=e=>{ if(e.target.closest('.vplay')){ play(r); return; } pickV(r); });
$('m-voices').onclick=e=>{const x=e.target.closest('span[data-v]'); if(!x) return; [...x.parentNode.children].forEach(y=>y.classList.toggle('on',y===x));
  const none=x.dataset.v==='0'; VRS().forEach(r=>r.hidden=none); $('v-hl').style.visibility=none?'hidden':''; $('vempty').hidden=!none; updateSum();};

/* step 5: mode cards. Selection is the white border plus surface-2 fill, nothing else. */
$('mcs').onclick=e=>{const c=e.target.closest('.mc'); if(!c) return; $('mcs').querySelectorAll('.mc').forEach(x=>x.classList.toggle('on',x===c));};
function updateSum(){
  const none=!$('vempty').hidden;
  $('sum').textContent=ZE.zones.length+' zones · '+$('k-read').textContent+' · '+(none?'No voice yet':voice.dataset.v+' 1.1x');
  $('gname-t').textContent=picked&&!picked.classList.contains('other')?picked.dataset.name:'Red Dead Redemption 2';
}
$('m-restart').onclick=()=>{location.href=location.pathname};
bindSlow();

/* ?s=N: step N. ?s=N-mid&ms=T: frozen T ms into the step from N-1 to N. ?s=done, ?s=done-mid&ms=T: the Finish transition. */
(function(){
  INSTANT=true; pick(PR()[0],0); pickV(VRS()[0],0);
  const s=Q.get('s')||'0', mid=s.endsWith('-mid'), b=s.replace('-mid','');
  if(Q.get('voices')==='0') document.querySelector('#m-voices span[data-v="0"]').click();
  if(b==='done'){ goStep(5); if(mid) INSTANT=false; finish(); }
  else { const n=+b; goStep(mid?n-1:n); INSTANT=false; if(mid) goStep(n); }
  INSTANT=false; if(mid) freeze();
})();
</script>"""
    consts = f"<script>const ZJ={zones_json(ZONES_RDR)};ICON['arrow-right']='{ic('arrow-right')}';</script>"
    write("01-setup.html", head("01 Setup", SETUP_CSS) + body + js_consts() + consts + MCORE + KEYCAP_JS + ZONE_JS + js + "\n</body></html>\n")

# ---------- 00 tokens ----------
TOK_CSS = """
.sheet{width:1440px;padding:40px 48px 64px;display:grid;grid-template-columns:1fr 1fr;gap:40px 48px}
.sheet h1{grid-column:1/-1;font:var(--fw-title) var(--fs-title)/1.2 var(--font-display);letter-spacing:-.4px}
.sheet h1 + p{grid-column:1/-1;color:var(--text-2);margin-top:-32px}
.blk h2{font:var(--fw-strong) var(--fs-body)/20px var(--font);margin-bottom:12px}
.blk.wide{grid-column:1/-1}
.sw-grid{display:grid;grid-template-columns:repeat(7,1fr);gap:12px}
.chip{border:1px solid var(--line);border-radius:var(--r-lg);overflow:hidden;background:var(--surface-1)}
.chip .c{height:56px;border-bottom:1px solid var(--line)}
.chip .m{padding:8px 12px 12px;display:flex;flex-direction:column;gap:2px}
.chip .m span{font-size:var(--fs-small);color:var(--text-2)}
.ts{display:flex;flex-direction:column;gap:4px;padding:12px 16px;border-top:1px solid var(--line)}
.ts:first-child{border-top:0}
.ts small{font-size:var(--fs-small);color:var(--text-2)}
.line2{display:flex;align-items:center;gap:12px;flex-wrap:wrap;padding:12px 16px;border-top:1px solid var(--line)}
.line2:first-child{border-top:0}
.line2 .k{width:120px;color:var(--text-2);font-size:var(--fs-small)}
.mtab{width:100%;border-collapse:collapse}
.mtab td{padding:8px 16px;border-top:1px solid var(--line);font-size:var(--fs-small);color:var(--text-2)}
.mtab tr:first-child td{border-top:0}
.mtab td:first-child{color:var(--text);font-size:var(--fs-body);width:150px}
.demo{position:relative;height:40px;width:220px;border-radius:var(--r);background:var(--surface-2)}
.demo i{position:absolute;left:4px;top:4px;width:32px;height:32px;border-radius:6px;background:var(--on)}
.mtab.go .demo i{transform:translateX(180px)}
.float{width:340px;border-radius:var(--r-lg);background:var(--surface-1);border:1px solid rgba(255,255,255,.1);box-shadow:0 12px 24px rgba(0,0,0,.6);padding:4px}
.dotrow{display:flex;gap:20px;align-items:center}
.dotrow span{display:inline-flex;gap:8px;align-items:center;font-size:var(--fs-small);color:var(--text-2)}
.dotrow i{width:8px;height:8px;border-radius:50%;display:block}
.lstatic .lrow{position:relative}
"""

def tokens_page():
    def chip(name, val, use, sw=None):
        sw = sw or val
        return f'<div class="chip"><div class="c" style="background:{sw}"></div><div class="m">{name}<span>{val}</span><span>{use}</span></div></div>'
    colours = "".join([
        chip("bg", "#000000", "Window, strip, page"), chip("surface-1", "#0A0A0A", "Stage frame, panels"),
        chip("surface-2", "#121212", "Hover, fields, keys"), chip("surface-3", "#1A1A1A", "Pressed, selected"),
        chip("line", "White 8%", "Hairlines", "rgba(255,255,255,.08)"), chip("line-2", "White 14%", "Control borders", "rgba(255,255,255,.14)"),
        chip("on", "#FFFFFF", "Primary, toggle on"),
        chip("text", "#F2F2F2", "Primary text"), chip("text-2", "#9A9A9A", "Secondary text"),
        chip("text-3", "#808080", "Off but readable"), chip("text-off", "#5C5C5C", "Disabled only"),
        chip("live", "#C5FA09", "Reading now only"),
        chip("danger", "#F0867E", "Delete, errors"), chip("close hover", "#C42B1C", "Close button"),
    ])
    type_ = ('<div class="panel">'
        '<div class="ts"><span style="font:var(--fw-title) var(--fs-title)/1.2 var(--font-display);letter-spacing:-.4px">Pick your game</span><small>Title. 26 px. Drawn 500, Slint 600, -0.4 px</small></div>'
        '<div class="ts"><span style="font:var(--fw-hero) var(--fs-hero)/1.4 var(--font)">We need to get these horses moving.</span><small>Hero, the line being read. 20 px. Drawn 400, Slint 500</small></div>'
        '<div class="ts"><span>Start with Windows</span><small>Body. 14 px. Drawn 400, Slint 500. Buttons and headings: drawn 500, Slint 600</small></div>'
        '<div class="ts"><span style="font-size:var(--fs-small);color:var(--text-2)">2 days ago</span><small>Small. 12.5 px, text-2. Drawn 400, Slint 500</small></div>'
        '<div class="ts"><span><span class="kbd">Mouse 4</span> <span class="kbd">Ctrl K</span></span><small>Mono, keycaps only. Cascadia Mono</small></div>'
        '<div class="ts"><span style="display:flex;gap:24px"><span style="font-weight:400">Weight 400 Adam</span><span style="font-weight:500">Weight 500 Adam</span><span style="font-weight:600">Weight 600 Adam</span></span><small>Font check, Segoe UI Variable. The three weights must look different</small></div>'
        '</div>')
    buttons = ('<div class="panel">'
        f'<div class="line2"><span class="k">Primary</span><span class="btn primary">{ic("play","fill")}Read now</span><span class="btn primary is-hover">Hover</span><span class="btn primary disabled">Disabled</span></div>'
        f'<div class="line2"><span class="k">Secondary</span><span class="btn">Check now</span><span class="btn is-hover">Hover</span><span class="btn is-press">Pressed</span><span class="btn disabled">Disabled</span></div>'
        f'<div class="line2"><span class="k">Ghost, danger</span><span class="btn ghost">{ic("pause")}Pause</span><span class="btn ghost">Show all{ic("chev-right")}</span><span class="btn danger">{ic("trash")}Delete zone</span></div>'
        f'<div class="line2"><span class="k">Icon, option</span><span class="iconbtn">{ic("settings")}</span><span class="iconbtn" style="background:var(--surface-2);color:var(--text)">{ic("replay")}</span><span class="opt">{ic("zap")}Automatic</span><span class="opt">{ic("key")}Mouse 4</span></div>'
        '</div>')
    controls = ('<div class="panel">'
        '<div class="line2"><span class="k">Toggle, Settings</span><span class="toggle on"></span><span class="toggle"></span><span class="toggle on disabled"></span></div>'
        f'<div class="line2"><span class="k">Zone chip</span>{zchip("dialogue", True)}{zchip("prompts", False)}<span style="color:var(--text-2);font-size:var(--fs-small)">Overview strip. Click turns a kind on or off</span></div>'
        '<div class="line2"><span class="k">Keycap</span><span class="kbd">Mouse 4</span><span class="kbd">F7</span><span class="kbd capture">Press a key</span></div>'
        '<div class="line2"><span class="k">Segmented</span><span class="seg"><span class="on">Top right</span><span>Bottom</span><span>Off</span></span></div>'
        f'<div class="line2"><span class="k">Field, slider</span><span class="field" style="width:160px">Adam{ic("chev-down","chev")}</span>{slider("1.1x", .4)}</div>'
        '<div class="line2"><span class="k">Status dot</span><span class="dotrow"><span><i style="background:var(--live)"></i>Reading</span><span><i style="background:var(--text-2)"></i>Paused</span><span><i style="background:var(--text-off)"></i>Game not running</span></span></div>'
        f'<div class="line2"><span class="k">Waveform</span>{wave(n=20)}{wave(flat=True, n=20)}<span style="color:var(--text-2);font-size:var(--fs-small)">Moves only while speech plays</span></div>'
        '</div>')
    rows = ('<div class="panel">'
        + srow("Start with Windows", T_ON) + srow("Read key", '<span class="kbd">Mouse 4</span>')
        + srow("Game profile", '<span class="btn">Export</span><span class="btn">Import</span>')
        + '</div><div class="panel lstatic" style="margin-top:16px;padding:4px">'
        + lrow("dialogue", "Dutch", "We ride at first light.", False, 0).replace('class="lrow"', 'class="lrow is-hover"')
        + lrow("pickups", "", "+1 Ginseng", False, 0)
        + lrow("prompts", "", "Hold E to talk", True, 0)
        + lrow("dialogue", "Hosea", "If we do not get the wagons through the pass before dark, we lose the whole camp to the cold.", False, 0)
        + '</div><p style="color:var(--text-2);font-size:var(--fs-small);margin-top:8px">Long lines wrap to two lines, then cut. See the hover card on 10-overview.</p>')
    motion = ('<div class="panel"><table class="mtab">'
        '<tr><td>ease-out</td><td>cubic-bezier(0.2, 0.7, 0.2, 1). Entries, hover, menus. Shown at 180 ms</td><td><div class="demo"><i style="transition:transform 180ms var(--ease-out)"></i></div></td></tr>'
        '<tr><td>ease-out-strong</td><td>cubic-bezier(0.16, 1, 0.3, 1). Page push, stage grow, glide. Shown at 260 ms</td><td><div class="demo"><i style="transition:transform 260ms var(--ease-out-strong)"></i></div></td></tr>'
        '<tr><td>ease-in</td><td>cubic-bezier(0.4, 0, 1, 1). Every exit. Shown at 140 ms</td><td><div class="demo"><i style="transition:transform 140ms var(--ease-in)"></i></div></td></tr>'
        '<tr><td>Durations</td><td colspan="2">Fast 120 ms, base 180, page 260, exit 140, stagger 40 (at most 6 items), glide 360. Press: scale 0.97. Reduce motion: 0, fades 80 ms.</td></tr>'
        '<tr><td>Page push</td><td colspan="2">Slight overlap (Max, 2026-11-24). Old page out 100 ms ease-in, new page in 260 ms ease-out-strong, 80 ms after the old one starts. Back: out 120, in 200, 80 ms apart.</td></tr>'
        '</table></div><p style="color:var(--text-2);font-size:var(--fs-small);margin-top:8px">Click the table to play the three curves.</p>')
    panel = (f'<div class="float lstatic"><div class="lrow is-hover"><span class="k">{ic("k-dialogue")}</span><span class="tx">Floating panel: menus, switcher</span></div>'
             f'<div class="lrow"><span class="tx" style="color:var(--text-2)">#0A0A0A, opaque, 1 px white 10%, shadow 24 px</span></div></div>')
    body = ('<div class="sheet"><h1>DialogReader tokens</h1><p>Black, neutral greys, white for on, one live hue. Text is drawn one weight lighter than in Slint.</p>'
            f'<div class="blk wide"><h2>Colour</h2><div class="sw-grid">{colours}</div></div>'
            f'<div class="blk"><h2>Type</h2>{type_}<h2 style="margin-top:24px">Rows and panel</h2>{rows}</div>'
            f'<div class="blk"><h2>Buttons</h2>{buttons}<h2 style="margin-top:24px">Controls</h2>{controls}<h2 style="margin-top:24px">Floating panel</h2>{panel}</div>'
            f'<div class="blk wide"><h2>Motion</h2>{motion}</div>'
            '</div>')
    js = "<script>document.querySelector('.mtab').onclick=e=>e.currentTarget.classList.toggle('go')</script>"
    write("00-tokens.html", head("00 Tokens", OV_CSS + TOK_CSS) + body + js + "\n</body></html>\n")

if __name__ == "__main__":
    tokens_page(); ov_page(); st_page(); motion_page()
    zones_page(); switcher_page(); activity_page(); setup_page(); pill_page(); quick_menu_page(); zone_edit_page()
    print("built")
