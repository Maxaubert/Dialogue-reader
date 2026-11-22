import os, random
# the mockup folder is the folder of this script
OUT = os.path.dirname(os.path.abspath(__file__))

P = {
 "chev-down": '<path d="m6 9 6 6 6-6"/>',
 "chev-right": '<path d="m9 18 6-6-6-6"/>',
 "arrow-left": '<path d="m12 19-7-7 7-7"/><path d="M19 12H5"/>',
 "arrow-right": '<path d="M5 12h14"/><path d="m12 5 7 7-7 7"/>',
 "check": '<path d="M20 6 9 17l-5-5"/>',
 "plus": '<path d="M5 12h14"/><path d="M12 5v14"/>',
 "search": '<circle cx="11" cy="11" r="7"/><path d="m20 20-3.5-3.5"/>',
 "refresh": '<path d="M3 12a9 9 0 0 1 9-9 9.75 9.75 0 0 1 6.74 2.74L21 8"/><path d="M21 3v5h-5"/><path d="M21 12a9 9 0 0 1-9 9 9.75 9.75 0 0 1-6.74-2.74L3 16"/><path d="M8 16H3v5"/>',
 "play": '<path d="M7 4.5v15l12-7.5z"/>',
 "pause": '<rect x="6" y="5" width="4" height="14" rx="1"/><rect x="14" y="5" width="4" height="14" rx="1"/>',
 "stop": '<rect x="6" y="6" width="12" height="12" rx="2"/>',
 "replay": '<path d="M3 12a9 9 0 1 0 9-9 9.75 9.75 0 0 0-6.74 2.74L3 8"/><path d="M3 3v5h5"/>',
 "volume": '<path d="M11 5 6 9H3v6h3l5 4z"/><path d="M15.5 8.5a5 5 0 0 1 0 7"/><path d="M18.5 5.5a9 9 0 0 1 0 13"/>',
 "keyboard": '<rect width="20" height="14" x="2" y="5" rx="2"/><path d="M6 9h.01M10 9h.01M14 9h.01M18 9h.01M8 13h.01M12 13h.01M16 13h.01M7 16h10"/>',
 "mouse": '<rect x="6" y="3" width="12" height="18" rx="6"/><path d="M12 3v7M6 10h12"/>',
 "window": '<rect x="2" y="4" width="20" height="16" rx="2"/><path d="M2 9h20"/>',
 "zone": '<path d="M5 3a2 2 0 0 0-2 2M19 3a2 2 0 0 1 2 2M21 19a2 2 0 0 1-2 2M5 21a2 2 0 0 1-2-2M9 3h1M9 21h1M14 3h1M14 21h1M3 9v1M21 9v1M3 14v1M21 14v1"/>',
 "settings": '<path d="M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z"/><circle cx="12" cy="12" r="3"/>',
 "x": '<path d="M18 6 6 18M6 6l12 12"/>',
 "minus": '<path d="M5 12h14"/>',
 "square": '<rect x="5" y="5" width="14" height="14" rx="1.5"/>',
 "grid": '<rect width="7" height="7" x="3" y="3" rx="1.5"/><rect width="7" height="7" x="14" y="3" rx="1.5"/><rect width="7" height="7" x="14" y="14" rx="1.5"/><rect width="7" height="7" x="3" y="14" rx="1.5"/>',
 "pointer": '<path d="M5 3.5 19 10l-6.2 1.8L11 18z"/>',
 "camera": '<path d="M14.5 4h-5L7 7H4a2 2 0 0 0-2 2v9a2 2 0 0 0 2 2h16a2 2 0 0 0 2-2V9a2 2 0 0 0-2-2h-3z"/><circle cx="12" cy="13" r="3"/>',
 "trash": '<path d="M3 6h18M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6M8 6V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"/>',
 "more": '<circle cx="5" cy="12" r=".75"/><circle cx="12" cy="12" r=".75"/><circle cx="19" cy="12" r=".75"/>',
 "zap": '<path d="M13 2 4 14h7l-1 8 9-12h-7z"/>',
 "download": '<path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><path d="m7 10 5 5 5-5"/><path d="M12 15V3"/>',
 "clock": '<circle cx="12" cy="12" r="9"/><path d="M12 7v5l3 2"/>',
 "info": '<circle cx="12" cy="12" r="9"/><path d="M12 16v-4M12 8h.01"/>',
 "upload": '<path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><path d="m17 8-5-5-5 5"/><path d="M12 3v12"/>',
 "ban": '<circle cx="12" cy="12" r="9"/><path d="m5.7 5.7 12.6 12.6"/>',
 "pencil": '<path d="M17 3a2.85 2.83 0 1 1 4 4L7.5 20.5 2 22l1.5-5.5Z"/>',
 "audio": '<path d="M2 10v3M6 6v11M10 3v18M14 8v7M18 5v13M22 10v3"/>',
 # kind icons (round 3): one neutral line icon per zone kind
 "k-dialogue": '<path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"/>',
 "k-speaker": '<circle cx="12" cy="8" r="4"/><path d="M20 21a8 8 0 0 0-16 0"/>',
 "k-pickups": '<path d="M21 8 12 3 3 8v8l9 5 9-5z"/><path d="m3 8 9 5 9-5"/><path d="M12 13v8"/>',
 "k-prompts": '<rect x="3" y="4" width="18" height="16" rx="3"/><rect x="7" y="7.5" width="10" height="6" rx="1.5"/>',
 "k-ignore": '<path d="M10.73 5.08A10.43 10.43 0 0 1 12 5c7 0 10 7 10 7a13.16 13.16 0 0 1-1.67 2.68"/><path d="M6.61 6.61A13.53 13.53 0 0 0 2 12s3 7 10 7a9.74 9.74 0 0 0 5.39-1.61"/><path d="M9.88 9.88a3 3 0 1 0 4.24 4.24"/><path d="m2 2 20 20"/>',
}
def ic(n, style=""):
    s = f' style="{style}"' if style else ""
    return f'<svg class="ic" viewBox="0 0 24 24"{s}>{P[n]}</svg>'

LOGO = ('<svg width="{s}" height="{s}" viewBox="0 0 24 24" style="flex:none;display:block">'
        '<rect width="24" height="24" rx="6" fill="#EDF0F6"/>'
        '<rect x="6.5" y="9" width="2" height="6" rx="1" fill="#07090E"/>'
        '<rect x="11" y="6" width="2" height="12" rx="1" fill="#07090E"/>'
        '<rect x="15.5" y="8" width="2" height="8" rx="1" fill="#5B7CFF"/></svg>')
def logo(s=24): return LOGO.replace("{s}", str(s))

WINCTL = f'<div class="winctl"><span>{ic("minus")}</span><span>{ic("square")}</span><span>{ic("x")}</span></div>'
BRAND = f'<div class="brand">{logo()}DialogReader</div>'

def head(title, css=""):
    st = f"<style>{css}</style>" if css else ""
    return (f'<!doctype html>\n<html lang="en"><head><meta charset="utf-8">'
            f'<meta name="viewport" content="width=1440"><title>{title}</title>'
            f'<link rel="stylesheet" href="tokens.css">{st}</head>\n<body>\n')

def kbd(t, sm=False): return f'<span class="kbd{" sm" if sm else ""}">{t}</span>'

# ---------- game art (fake screenshots, simple shapes) ----------
ART = {
 "rdr2": ('<linearGradient id="a-rdr2" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#1F2E45"/><stop offset="1" stop-color="#9AAEC4"/></linearGradient>',
          '<rect width="160" height="90" fill="url(#a-rdr2)"/><polygon fill="#6E8299" points="0,52 22,36 38,44 60,26 82,42 104,30 128,46 160,34 160,90 0,90"/>'
          '<polygon fill="#E2E8EF" points="0,66 50,61 100,66 160,60 160,90 0,90"/>'
          '<g fill="#25303C"><polygon points="24,68 30,50 36,68"/><polygon points="33,69 38,56 43,69"/><polygon points="118,66 124,46 130,66"/></g>'),
 "satis": ('<linearGradient id="a-satis" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#D9773A"/><stop offset="1" stop-color="#F4CF96"/></linearGradient>',
           '<rect width="160" height="90" fill="url(#a-satis)"/><circle cx="118" cy="30" r="12" fill="#FBE7C2"/>'
           '<polygon fill="#B7825A" points="0,62 40,54 90,60 160,50 160,90 0,90"/>'
           '<g fill="#2C3740"><rect x="22" y="34" width="10" height="34"/><rect x="36" y="44" width="22" height="24"/><rect x="62" y="28" width="7" height="40"/><rect x="0" y="66" width="160" height="24"/></g>'
           '<rect x="36" y="50" width="96" height="3" fill="#E8A24E"/>'),
 "p4g": ('<linearGradient id="a-p4g" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#F7E07A"/><stop offset="1" stop-color="#D9A900"/></linearGradient>',
         '<rect width="160" height="90" fill="url(#a-p4g)"/>'
         '<g fill="#3B3320"><rect x="10" y="40" width="26" height="50"/><rect x="40" y="52" width="20" height="38"/><rect x="104" y="36" width="22" height="54"/><rect x="130" y="48" width="30" height="42"/></g>'
         '<path d="M0 30 Q80 44 160 26" stroke="#3B3320" stroke-width="1" fill="none"/><rect x="70" y="58" width="26" height="20" rx="2" fill="#1B1710"/><rect x="73" y="61" width="20" height="14" fill="#F7E07A" fill-opacity=".5"/>'),
 "e33": ('<linearGradient id="a-e33" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#A93A47"/><stop offset=".6" stop-color="#E2AE6E"/><stop offset="1" stop-color="#F2E1C2"/></linearGradient>',
         '<rect width="160" height="90" fill="url(#a-e33)"/><rect x="98" y="8" width="8" height="60" fill="#2A1016"/>'
         '<polygon fill="#5A2430" points="0,70 60,64 160,68 160,90 0,90"/><polygon fill="#2A1016" points="0,78 30,70 54,80 0,90"/>'),
 "kcd2": ('<linearGradient id="a-kcd2" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#9FB6CC"/><stop offset="1" stop-color="#E6E0C6"/></linearGradient>',
          '<rect width="160" height="90" fill="url(#a-kcd2)"/><polygon fill="#5D7A49" points="0,58 40,46 80,54 120,42 160,50 160,90 0,90"/>'
          '<g fill="#3A3A36"><rect x="104" y="30" width="16" height="20"/><rect x="108" y="22" width="6" height="10"/><rect x="96" y="38" width="34" height="12"/></g>'
          '<polygon fill="#2F4527" points="0,72 60,64 120,72 160,66 160,90 0,90"/>'),
}
def art(g):
    d, body = ART[g]
    return f'<svg viewBox="0 0 160 90" preserveAspectRatio="xMidYMid slice" aria-hidden="true"><defs>{d}</defs>{body}</svg>'
def thumb(g, style="", cls=""):
    return f'<span class="thumb {cls}" style="{style}">{art(g)}</span>'

# ---------- scene ----------
SCENE = '''<svg class="scene" viewBox="0 0 1600 900" preserveAspectRatio="xMidYMid slice" aria-hidden="true">
<defs>
<linearGradient id="sky" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#1B2638"/><stop offset=".55" stop-color="#5A6F8C"/><stop offset="1" stop-color="#B4C2D2"/></linearGradient>
<linearGradient id="snow" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#E3E9F0"/><stop offset="1" stop-color="#AEBACA"/></linearGradient>
<linearGradient id="shade" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#05070B" stop-opacity="0"/><stop offset="1" stop-color="#05070B" stop-opacity=".62"/></linearGradient>
</defs>
<rect width="1600" height="900" fill="url(#sky)"/>
<polygon fill="#71849C" points="0,520 170,370 310,450 470,300 640,430 790,330 960,470 1120,320 1300,440 1450,350 1600,430 1600,900 0,900"/>
<polygon fill="#94A5B9" points="0,600 260,520 520,590 800,540 1100,600 1400,530 1600,580 1600,900 0,900"/>
<polygon fill="url(#snow)" points="0,660 400,630 900,662 1300,636 1600,656 1600,900 0,900"/>
<g fill="#26303D"><polygon points="430,640 470,520 510,640"/><polygon points="480,646 512,560 544,646"/><polygon points="1050,650 1095,500 1140,650"/><polygon points="1110,654 1140,570 1170,654"/><polygon points="1350,640 1380,560 1410,640"/></g>
<rect y="620" width="1600" height="280" fill="url(#shade)"/>
<rect x="100" y="60" width="266" height="60" rx="6" fill="#0A0C10" fill-opacity=".6"/>
<rect x="116" y="74" width="32" height="32" rx="4" fill="#C9A35A"/>
<text x="164" y="100" font-family="Georgia, serif" font-size="26" fill="#FFFFFF">+1 Ginseng</text>
<rect x="1240" y="700" width="260" height="48" rx="6" fill="#0A0C10" fill-opacity=".6"/>
<text x="1370" y="732" text-anchor="middle" font-family="Georgia, serif" font-size="22" fill="#FFFFFF">Hold E to talk</text>
<circle cx="200" cy="770" r="80" fill="#141820" fill-opacity=".7" stroke="#CFD6DE" stroke-width="3"/>
<path d="M200 748 l10 24 h-20z" fill="#FFFFFF"/>
<text x="800" y="786" text-anchor="middle" font-family="Georgia, serif" font-size="26" font-style="italic" fill="#E9C46A">Arthur</text>
<text x="800" y="834" text-anchor="middle" font-family="Georgia, serif" font-size="28" fill="#FFFFFF">We need to get these horses moving before the snow buries us.</text>
</svg>'''

ZONES = [  # kind, name, x1,y1,x2,y2 in 1600x900
 ("speaker", "Speaker name", 712, 754, 888, 798),
 ("dialogue", "Subtitles", 360, 800, 1240, 850),
 ("pickups", "Item pickups", 88, 48, 378, 132),
 ("prompts", "Button prompts", 1228, 690, 1512, 758),
 ("ignore", "Minimap", 112, 682, 288, 858),
]
KIND = {"dialogue": "Dialogue", "speaker": "Speaker", "pickups": "Pickups", "prompts": "Prompts", "ignore": "Ignore"}

def zones(labels=True, sel=None, off=()):
    out = []
    for k, n, x1, y1, x2, y2 in ZONES:
        st = f"left:{x1/16:.3f}%;top:{y1/9:.3f}%;width:{(x2-x1)/16:.3f}%;height:{(y2-y1)/9:.3f}%"
        cls = f"zone z-{k}" + (" sel" if n == sel else "") + (" off" if k in off else "")
        icon = ic("ban") if k == "ignore" else ""
        inner = f'<span class="ztag">{icon}{KIND[k]}{" · off" if k in off else ""}</span>' if labels else ""
        if n == sel:
            for hx, hy in [(0,0),(50,0),(100,0),(0,50),(100,50),(0,100),(50,100),(100,100)]:
                inner += f'<span class="handle" style="left:{hx}%;top:{hy}%"></span>'
        out.append(f'<div class="{cls}" style="{st}">{inner}</div>')
    return "".join(out)

def stage(style="", labels=True, sel=None, off=(), extra=""):
    return f'<div class="stage" style="{style}">{SCENE}{zones(labels, sel, off)}{extra}</div>'

# ---------- shared components ----------
def zonerow(k, n, sel=False, on=True):
    return (f'<div class="zrow{" on" if sel else ""}{"" if on else " off"}"><span class="dot k-{k}"></span>'
            f'<span class="name">{n}</span><span class="chip k-{k}">{KIND[k]}</span>'
            f'<span class="toggle{" on" if on else ""}"></span></div>')
def zonelist(sel=None, off=()):
    return "".join(zonerow(k, n, n == sel, k not in off) for k, n, *_ in ZONES)

# round 3: kind shown as a grey line icon + name (no colored pill)
def kindtag(k): return f'<span class="kind">{ic("k-" + k)}{KIND[k]}</span>'
def zonerow_i(k, n, sel=False, on=True):
    return (f'<div class="zrow{" on" if sel else ""}{"" if on else " off"}"><span class="dot k-{k}"></span>'
            f'<span class="name">{n}</span>{kindtag(k)}'
            f'<span class="toggle{" on" if on else ""}"></span></div>')
def zonelist_i(sel=None, off=()):
    return "".join(zonerow_i(k, n, n == sel, k not in off) for k, n, *_ in ZONES)

# round 4 (Max): zone rows show only a grey kind icon before the name, no colored dot, no kind label
def zonerow_x(k, n, sel=False, on=True):
    return (f'<div class="zrow{" on" if sel else ""}{"" if on else " off"}"><span class="zic" title="{KIND[k]}">{ic("k-" + k)}</span>'
            f'<span class="name">{n}</span>'
            f'<span class="toggle{" on" if on else ""}"></span></div>')
def zonelist_x(sel=None, off=()):
    return "".join(zonerow_x(k, n, n == sel, k not in off) for k, n, *_ in ZONES)

# 2026-11-22 (Max): the Kind picker shows the grey kind line icons, not five identical grey dots
def kindpicker(active="dialogue"):
    return '<div class="seg">' + "".join(f'<span class="{"on" if k==active else ""}">{ic("k-" + k)}{KIND[k]}</span>' for k in KIND) + '</div>'

def zonetools(draw=True, zoom=False):
    z = '<div class="seg"><span>Fit</span><span class="on">100%</span></div>' if zoom else ""
    return (f'<div class="ztools"><div class="seg"><span class="{"" if draw else "on"}">{ic("pointer")}Select</span>'
            f'<span class="{"on" if draw else ""}">{ic("plus")}Draw zone</span></div>'
            f'<span class="label" style="margin-left:var(--s-2)">Kind</span>{kindpicker()}'
            f'<span class="spacer"></span><span class="btn">{ic("camera")}Retake screenshot</span>{z}</div>')

def pillwave(n=14, h=14):
    import math
    return '<span class="pill-wave">' + "".join(f'<i style="height:{max(3, int(h*(0.3+0.7*abs(math.sin(i*1.3)))))}px"></i>' for i in range(n)) + '</span>'
def overlay_pill():
    return f'<span class="overlay-pill"><span class="dot"></span><span>Arthur</span>{pillwave()}</span>'

# ---------- chrome ----------
STEPS = ["Game", "Zones", "Read key", "Voice", "Mode"]
def obbar(cur):  # cur: 0 = welcome (brand only), 1..5
    st = ""
    if cur:
        parts = []
        for i, s in enumerate(STEPS, 1):
            if i < cur: cls, n = "step done", ic("check")
            elif i == cur: cls, n = "step on", str(i)
            else: cls, n = "step", str(i)
            if i > 1: parts.append('<span class="step-sep"></span>')
            parts.append(f'<div class="{cls}"><span class="n">{n}</span>{s}</div>')
        st = f'<div class="steps">{"".join(parts)}</div>'
    return f'<header class="bar">{BRAND}{st}<div class="bar-right">{WINCTL}</div></header>\n'

def obfoot(primary="Continue", picon="arrow-right"):
    return (f'<footer class="foot"><div class="foot-in"><span class="btn btn-lg">{ic("arrow-left")}Back</span>'
            f'<span class="spacer"></span><span class="btn btn-primary btn-lg">{primary}{ic(picon)}</span></div></footer>\n')

def obhead(title, lead):
    return f'<div class="ob-head"><h1>{title}</h1><p class="lead">{lead}</p></div>'

def nav(active):
    tabs = [("Overview", "grid"), ("Zones", "zone"), ("Settings", "settings")]
    t = "".join(f'<div class="tab{" on" if n == active else ""}">{ic(i)}{n}</div>' for n, i in tabs)
    return (f'<header class="bar">{BRAND}'
            f'<div class="switcher">{thumb("rdr2")}<span class="name">Red Dead Redemption 2</span>{ic("chev-down")}{kbd("Ctrl K")}</div>'
            f'<span class="vsep"></span><nav class="tabs">{t}</nav>'
            f'<div class="bar-right"><span class="status"><span class="dot"></span>Reading</span>'
            f'{WINCTL}</div></header>\n')

def write(name, html):
    with open(os.path.join(OUT, name), "w", encoding="utf-8") as f:
        f.write(html + "</body></html>\n")

# ================= 01 welcome =================
css01 = """
.wrap{width:var(--col);margin:0 auto;height:100%;display:flex;align-items:center;gap:var(--s-7)}
.left{width:400px;flex:none}
.steplist{display:flex;flex-direction:column;gap:var(--s-4);margin:var(--s-6) 0}
.steplist .row{align-items:flex-start;gap:var(--s-3)}
.num{width:24px;height:24px;border-radius:50%;background:var(--surface-3);color:var(--text);font-size:var(--fs-1);font-weight:600;display:flex;align-items:center;justify-content:center;flex:none}
.right{flex:1;min-width:0;overflow:hidden}
.right .stage{border-radius:var(--r-lg) var(--r-lg) 0 0}
.nowline{display:flex;align-items:center;gap:var(--s-3);padding:var(--s-4) var(--s-5);border-top:1px solid var(--line)}
"""
pill = f'<div style="position:absolute;top:12px;right:12px">{overlay_pill()}</div>'
html = head("Welcome", css01) + obbar(0) + f'''<main class="page kgrey"><div class="wrap">
<section class="left">
<h1>Hear the text in your games.</h1>
<p class="lead" style="margin-top:var(--s-3)">DialogReader watches your game and reads dialogue, item pickups and other on-screen text out loud, with a voice that runs on your PC.</p>
<div class="steplist">
 <div class="row"><span class="num">1</span><div><div style="font-weight:500">Pick your game</div><div class="muted">Start the game first. DialogReader finds it.</div></div></div>
 <div class="row"><span class="num">2</span><div><div style="font-weight:500">Mark where text shows up</div><div class="muted">Draw boxes over subtitles, names and pop-ups.</div></div></div>
 <div class="row"><span class="num">3</span><div><div style="font-weight:500">Choose a key, a voice and a mode</div><div class="muted">Then play. The window can stay closed.</div></div></div>
</div>
<div class="row g3"><span class="btn btn-primary btn-lg">Get started{ic("arrow-right")}</span><span class="btn btn-lg">{ic("upload")}Import settings</span></div>
<p class="small faint" style="margin-top:var(--s-4)">Takes about two minutes. You can change everything later.</p>
</section>
<section class="right card">
{stage(labels=False, extra=pill)}
<div class="nowline"><span class="btn btn-icon">{ic("volume")}</span><div class="grow"><div class="trunc"><span class="muted" style="font-weight:500">Arthur:</span> We need to get these horses moving before the snow buries us.</div><div class="small faint">Read just now · Kokoro · Adam</div></div><span class="chip k-dialogue">Dialogue</span></div>
</section>
</div></main>
'''
write("01-welcome.html", html)

# ================= 02 pick window =================
css02 = """
.grid3{display:grid;grid-template-columns:repeat(3,1fr);gap:var(--s-4)}
.win{padding:var(--s-2);position:relative}
.win .thumb{width:100%;height:120px;border-radius:var(--r-md)}
.win .meta{padding:var(--s-3) var(--s-2) var(--s-1)}
.badge{position:absolute;top:16px;right:16px;width:24px;height:24px;border-radius:50%;background:var(--accent);color:#fff;display:flex;align-items:center;justify-content:center;box-shadow:var(--shadow-1)}
.lrow{display:flex;align-items:center;gap:var(--s-3);height:52px;padding:0 var(--s-4);border-top:1px solid var(--line)}
.lrow:first-child{border-top:0}
.sec{margin:var(--s-6) 0 var(--s-3)}
"""
def wincard(g, name, exe, sel=False):
    b = f'<span class="badge">{ic("check")}</span>' if sel else ""
    return (f'<div class="card win{" sel" if sel else ""}">{thumb(g)}{b}'
            f'<div class="meta"><div style="font-weight:500">{name}</div><div class="small faint">{exe}</div></div></div>')
def lrow(name, exe):
    return f'<div class="lrow"><span class="itile">{ic("window")}</span><span style="font-weight:500">{name}</span><span class="small faint">{exe}</span></div>'
html = head("Pick window", css02) + obbar(1) + f'''<main class="page"><div class="ob">
{obhead("Pick your game", "Start your game first. It shows up here while it is running.")}
<div class="ob-body">
<div class="row g3"><div class="input" style="width:360px">{ic("search")}<span class="ph">Search windows</span></div><span class="spacer"></span><span class="btn">{ic("refresh")}Refresh</span></div>
<div class="sec row g2"><span class="label">Games running now</span><span class="count">3</span></div>
<div class="grid3">
{wincard("rdr2", "Red Dead Redemption 2", "RDR2.exe · 3840 × 2160 · borderless", True)}
{wincard("satis", "Satisfactory", "FactoryGame.exe · 2560 × 1440 · windowed")}
{wincard("p4g", "Persona 4 Golden", "P4G.exe · 1920 × 1080 · borderless")}
</div>
<div class="sec row g2"><span class="label">Other windows</span><span class="count">3</span></div>
<div class="card">{lrow("Discord", "Discord.exe")}{lrow("Spotify", "Spotify.exe")}{lrow("Zen Browser", "zen.exe")}</div>
</div>
</div></main>
''' + obfoot()
write("02-pick-window.html", html)

# ================= 03 draw zones =================
css03 = """
.work{width:1216px;margin:var(--s-5) auto 0}
.split{display:flex;gap:var(--s-5);margin-top:var(--s-4)}
.panel{flex:1;min-width:0;display:flex;flex-direction:column;overflow:hidden}
.krow{display:flex;gap:var(--s-3);align-items:center;height:28px}
"""
kinds = [("dialogue", "Spoken lines"), ("speaker", "Who is talking"), ("pickups", "Item pop-ups"), ("prompts", "Button hints"), ("ignore", "Never read")]
klist = "".join(f'<div class="krow"><span class="kicon">{ic("k-" + k)}</span><span style="font-weight:500;width:72px">{KIND[k]}</span><span class="muted">{d}</span></div>' for k, d in kinds)
html = head("Draw zones", css03) + obbar(2) + f'''<main class="page kgrey">
<div class="ob">{obhead("Mark where text shows up", "Drag on the screenshot to draw a zone, then pick what kind of text it holds.")}</div>
<div class="work">
{zonetools(draw=True)}
<div class="split">
{stage(style="width:872px;flex:none", sel="Subtitles")}
<aside class="card panel">
<div class="card-head"><h3>Zones</h3><span class="count">5</span></div>
<div>{zonelist_x(sel="Subtitles")}</div>
<div style="margin-top:auto;padding:var(--s-3) var(--s-5) var(--s-4);border-top:1px solid var(--line)"><div class="label" style="margin-bottom:var(--s-1)">What each kind does</div>{klist}</div>
</aside>
</div>
</div></main>
''' + obfoot()
write("03-draw-zones.html", html)

# ================= 04 read key (round 3: calmer layout, round 4 fixes) =================
css04 = """
.kcol{max-width:600px;margin-top:var(--s-6)}
.keycard{display:flex;flex-direction:column;align-items:center;text-align:center;padding:var(--s-6) var(--s-6) var(--s-5)}
.bigkey{display:inline-flex;align-items:center;gap:var(--s-3);height:72px;padding:0 var(--s-6);margin:var(--s-3) 0 var(--s-4);border-radius:var(--r-lg);border:1px solid var(--line-2);border-bottom-width:3px;background:var(--surface-2);font-size:var(--fs-5);font-weight:600;letter-spacing:-0.02em}
.bigkey .ic{width:24px;height:24px;color:var(--text-2)}
.listen{display:inline-flex;align-items:center;gap:var(--s-2);margin-top:var(--s-3);color:var(--text-3);font-size:var(--fs-1)}
.listen .dot{width:6px;height:6px;background:var(--accent)}
.alts{padding:0;overflow:hidden;margin-top:var(--s-3)}
.alt{display:flex;align-items:center;gap:var(--s-4);height:56px;padding:0 var(--s-5);border-top:1px solid var(--line)}
.alt:first-child{border-top:0}
.alt .kbd{min-width:84px;justify-content:center}
"""
def alt(k, d): return f'<div class="alt">{kbd(k)}<span class="grow muted">{d}</span><span class="btn">Use</span></div>'
html = head("Read key", css04) + obbar(3) + f'''<main class="page"><div class="ob">
{obhead("Choose your read key", "Press it while you play to read every zone out loud right away. Pick a key the game does not use.")}
<div class="kcol">
<div class="card keycard">
<span class="label">Your read key</span>
<div class="bigkey">{ic("mouse")}Mouse 4</div>
<p class="muted">Side buttons on a mouse rarely clash with game controls.</p>
<span class="btn" style="margin-top:var(--s-5)">{ic("keyboard")}Press a different key</span>
<span class="listen"><span class="dot"></span>Or just press any key or mouse button now</span>
</div>
<div class="label" style="margin-top:var(--s-6)">Other good choices</div>
<div class="card alts">{alt("Mouse 5", "The other side button")}{alt("F8", "Rarely used by games")}{alt("Caps Lock", "Easy to reach with your left hand")}</div>
<div class="row g3" style="margin-top:var(--s-5);color:var(--text-2)">{ic("info")}<span>The key works while the game is in front. DialogReader never takes focus.</span></div>
</div>
</div></main>
''' + obfoot()
write("04-read-key.html", html)

# ================= 05 voice =================
random.seed(4)
def wave(n, played, h=40):
    import math
    bars = []
    for i in range(n):
        v = 0.25 + 0.75 * abs(math.sin(i * 0.55) * 0.6 + random.random() * 0.4)
        bars.append(f'<i class="{"" if i < played else "off"}" style="height:{max(4, int(v*h))}px"></i>')
    return '<div class="wave">' + "".join(bars) + '</div>'
css05 = """
.cols{display:flex;gap:var(--s-5);align-items:stretch}
.voices{flex:1;display:flex;flex-direction:column;overflow:hidden}
.vrow{display:flex;align-items:center;gap:var(--s-3);height:60px;padding:0 var(--s-5);border-top:1px solid var(--line)}
.vrow:first-of-type{border-top:0}
.vrow.on{background:var(--surface-2)}
.vrow .ck{color:var(--accent);display:flex}
.test{width:400px;flex:none;display:flex;flex-direction:column}
.field{display:flex;flex-direction:column;gap:var(--s-2)}
.val{font:400 var(--fs-1)/1 var(--mono);color:var(--text-2);margin-left:auto}
"""
voices = [("Adam", "American English · Male", True), ("Bella", "American English · Female", False), ("Michael", "American English · Male", False), ("Emma", "British English · Female", False), ("George", "British English · Male", False), ("Sky", "American English · Female", False)]
vhtml = "".join(f'<div class="vrow{" on" if s else ""}"><span class="itile{" on" if s else ""}">{ic("audio")}</span><div class="grow"><div style="font-weight:500">{n}</div><div class="small faint">{d}</div></div>' + (f'<span class="chip k-ok nodot">{ic("check")}Selected</span>' if s else '') + '</div>' for n, d, s in voices)
html = head("Voice", css05) + obbar(4) + f'''<main class="page"><div class="ob">
{obhead("Pick a voice", "Voices run on your PC. Nothing is sent online.")}
<div class="ob-body cols">
<section class="card voices">
<div class="card-head"><h3>Kokoro voices</h3><span class="count">11</span><span class="spacer btn btn-ghost">{ic("download")}Get more voices</span></div>
{vhtml}
</section>
<section class="card test">
<div class="card-head"><h3>Test it</h3><span class="spacer small faint">Kokoro · Adam</span></div>
<div class="col g5 pad" style="flex:1">
<div class="field"><span class="label">Sample line</span><div class="input textarea">Arthur: We need to get these horses moving before the snow buries us.</div></div>
<div class="field"><div class="row"><span class="label">Speed</span><span class="val">1.1×</span></div><div class="slider"><div class="fill" style="width:55%"></div><span class="knob" style="left:55%"></span></div></div>
<div class="field"><div class="row"><span class="label">Volume</span><span class="val">80%</span></div><div class="slider"><div class="fill" style="width:80%"></div><span class="knob" style="left:80%"></span></div></div>
<div class="card" style="background:var(--surface-2);padding:var(--s-3) var(--s-4);margin-top:auto"><div class="row g3">{wave(34, 20)}<span class="val">0:02 / 0:04</span></div></div>
<span class="btn btn-lg" style="width:100%">{ic("stop")}Stop</span>
</div>
</section>
</div>
</div></main>
''' + obfoot()
write("05-voice.html", html)

# ================= 06 mode =================
css06 = """
.opts{display:grid;grid-template-columns:1fr 1fr;gap:var(--s-4)}
.opt{padding:var(--s-5);display:flex;flex-direction:column;gap:var(--s-3)}
.opt .icb{width:40px;height:40px;border-radius:var(--r-md);background:var(--surface-3);display:flex;align-items:center;justify-content:center}
.opt.sel .icb{background:var(--accent-soft);color:var(--accent)}
.sum .set-row{min-height:48px}
.sum .k{width:120px;color:var(--text-2)}
"""
html = head("Mode", css06) + obbar(5) + f'''<main class="page kgrey"><div class="ob">
{obhead("How should it read?", "You can switch any time from the main window.")}
<div class="ob-body opts">
<div class="card opt sel"><div class="row"><span class="icb">{ic("zap")}</span><span class="spacer radio on"></span></div><h3>Automatic</h3><p>Reads new text the moment it appears in a zone.</p><p class="muted">Good for story games and long talks.</p></div>
<div class="card opt"><div class="row"><span class="icb">{ic("mouse")}</span><span class="spacer radio"></span></div><h3>On key press</h3><p>Reads every zone once when you press your read key.</p><p class="muted">Good when you only want some lines.</p></div>
</div>
<div class="label" style="margin:var(--s-6) 0 var(--s-3)">Your setup for Red Dead Redemption 2</div>
<div class="card sum">
<div class="set-row"><span class="k">Game</span>{thumb("rdr2", "width:24px;height:24px;border-radius:var(--r-xs)")}<span class="t">Red Dead Redemption 2</span></div>
<div class="set-row"><span class="k">Zones</span><span class="t">5 zones</span><span class="row g2"><span class="dot k-dialogue"></span><span class="dot k-speaker"></span><span class="dot k-pickups"></span><span class="dot k-prompts"></span><span class="dot k-ignore"></span></span></div>
<div class="set-row"><span class="k">Read key</span>{kbd("Mouse 4")}</div>
<div class="set-row"><span class="k">Voice</span><span class="t">Kokoro · Adam</span><span class="muted">1.1× speed</span></div>
</div>
</div></main>
''' + obfoot("Finish", "check")
write("06-mode.html", html)

# ================= 10 main =================
css10 = """
.mw{display:flex;flex-direction:column;gap:var(--s-4)}
.top{display:grid;grid-template-columns:2fr 1fr 1fr 1fr;gap:var(--s-4)}
.bottom{flex:1;min-height:0;display:grid;grid-template-columns:5fr 7fr;gap:var(--s-4)}
.bottom .card{display:flex;flex-direction:column;overflow:hidden}
.stagewrap{padding:var(--s-4) var(--s-5);border-bottom:1px solid var(--line)}
"""
LOG = [
 ("21:42:18", "dialogue", "Arthur:", "We need to get these horses moving before the snow buries us.", ""),
 ("21:42:11", "pickups", "", "+1 Ginseng", ""),
 ("21:42:09", "prompts", "", "Hold E to talk", "Prompts off"),
 ("21:41:57", "dialogue", "Dutch:", "We ride at first light. Nobody stays behind.", ""),
 ("21:41:50", "dialogue", "Arthur:", "I hear you.", ""),
 ("21:41:49", "dialogue", "Arthur:", "I hear you.", "Already read"),
 ("21:41:32", "pickups", "", "+2 Yarrow", ""),
 ("21:40:58", "dialogue", "Sadie:", "Then stop talking and start digging.", ""),
 ("21:40:41", "pickups", "", "Donated to camp: $5.00", ""),
 ("21:40:12", "dialogue", "Charles:", "Tracks lead north. Two riders, maybe three.", ""),
 ("21:39:55", "pickups", "", "+1 Venison", ""),
 ("21:39:30", "dialogue", "Arthur:", "Keep your voice down.", ""),
 ("21:39:02", "dialogue", "Javier:", "The wagon wheel is cracked. It will not last the night.", ""),
]
def logrows(n=13):
    r = []
    for t, k, who, line, skip in LOG[:n]:
        w = f'<span class="who">{who}</span> ' if who else ""
        end = f'<span class="chip k-off">{skip}</span>' if skip else f'<span class="btn btn-icon btn-ghost" title="Replay">{ic("replay")}</span>'
        r.append(f'<div class="log-row{" skip" if skip else ""}"><span class="time">{t}</span><span><span class="chip k-{k}">{KIND[k]}</span></span><span class="line trunc">{w}{line}</span>{end}</div>')
    return "".join(r)
def main_body():
    return f'''<main class="page"><div class="mw">
<div class="top">
<div class="card stat"><span class="label">Status</span><div class="slot"><span class="v"><span class="dot"></span>Reading automatically</span><span class="btn">{ic("play")}Read now{kbd("Mouse 4", True)}</span><span class="btn">{ic("pause")}Pause</span></div><span class="meta">Window found · 3840 × 2160 borderless · 118 lines in 42 min</span></div>
<div class="card stat"><span class="label">Mode</span><div class="slot"><span class="v">{ic("zap")}Automatic</span><span class="btn">Change</span></div><span class="meta">Reads new text as it appears.</span></div>
<div class="card stat"><span class="label">Voice</span><div class="slot"><span class="v">Kokoro · Adam</span><span class="btn">{ic("play")}Test</span></div><span class="meta">Speed 1.1× · Volume 80%</span></div>
<div class="card stat"><span class="label">Read key</span><div class="slot"><span class="v">{kbd("Mouse 4")}</span><span class="btn">Change</span></div><span class="meta">Reads every zone right now.</span></div>
</div>
<div class="bottom">
<section class="card">
<div class="card-head"><h3>Zones</h3><span class="count">5</span><span class="spacer btn">{ic("zone")}Edit zones</span></div>
<div class="stagewrap">{stage(labels=False, off=("prompts",))}</div>
<div>{zonelist(off=("prompts",))}</div>
</section>
<section class="card">
<div class="card-head"><h3>Activity</h3><span class="spacer"></span><div class="seg"><span class="on">All</span><span>Read</span><span>Skipped</span></div></div>
<div>{logrows()}</div>
</section>
</div>
</div></main>
'''
# round 3: page 10 gets its own layout; 14 keeps the original main_body()
css10b = """
.mw{display:flex;flex-direction:column;gap:var(--s-4)}
.sbar{display:flex;align-items:center;gap:var(--s-3);height:56px;padding:0 var(--s-3) 0 var(--s-5);flex:none}
.sbar .st{display:flex;align-items:center;gap:var(--s-2);font-weight:500;white-space:nowrap}
.sbar .vsep{margin:0 var(--s-2)}
.sbar .opt{display:inline-flex;align-items:center;gap:var(--s-2);height:var(--h-ctl);padding:0 var(--s-2);border-radius:var(--r-md);color:var(--text);white-space:nowrap}
.sbar .opt .k{color:var(--text-3);font-size:var(--fs-1);font-weight:500}
.sbar .opt > .ic{color:var(--text-3)}
.zcard{flex:none;display:flex;flex-direction:column;overflow:hidden}
.zbody{height:300px;display:flex}
.zbody .stagewrap{padding:var(--s-4) var(--s-5);height:100%;border-right:1px solid var(--line)}
.zbody .stage{height:100%;width:auto}
.zbody .zl{flex:1;min-width:0;display:flex;flex-direction:column}
.zbody .zl .zrow{flex:1;height:auto}
.acard{flex:1;min-height:0;overflow:hidden}
.acard .seg > span{height:22px}
.acard .card-head{height:44px}
"""
def logrows_i(n=6):
    r = []
    for t, k, who, line, skip in LOG[:n]:
        w = f'<span class="who">{who}</span> ' if who else ""
        end = f'<span class="small faint">{skip}</span>' if skip else f'<span class="btn btn-icon btn-ghost" title="Replay">{ic("replay")}</span>'
        r.append(f'<div class="log-row compact{" skip" if skip else ""}"><span class="time">{t}</span>{kindtag(k)}<span class="line trunc">{w}{line}</span>{end}</div>')
    return "".join(r)
def main_body10():
    return f'''<main class="page"><div class="mw">
<div class="card sbar">
<span class="st"><span class="dot"></span>Reading automatically</span><span class="small faint">118 lines in 42 min</span>
<span class="spacer"></span>
<span class="btn">{ic("play")}Read now{kbd("Mouse 4", True)}</span><span class="btn">{ic("pause")}Pause</span>
<span class="vsep"></span>
<span class="opt"><span class="k">Mode</span>{ic("zap")}Automatic{ic("chev-down")}</span>
<span class="opt"><span class="k">Voice</span>Kokoro · Adam{ic("chev-down")}</span><span class="btn btn-icon btn-ghost" title="Test voice">{ic("play")}</span>
<span class="opt"><span class="k">Key</span>{kbd("Mouse 4", True)}{ic("chev-down")}</span>
</div>
<section class="card zcard">
<div class="card-head"><h3>Zones</h3><span class="count">5</span><span class="spacer btn">{ic("zone")}Edit zones</span></div>
<div class="zbody"><div class="stagewrap">{stage(labels=False, off=("prompts",))}</div><div class="zl">{zonelist_x(off=("prompts",))}</div></div>
</section>
<section class="card acard">
<div class="card-head"><h3>Recent activity</h3><span class="spacer"></span><div class="seg"><span class="on">All</span><span>Read</span><span>Skipped</span></div><span class="btn btn-ghost">Show all{ic("chev-right")}</span></div>
<div>{logrows_i(8)}</div>
</section>
</div></main>
'''
write("10-main.html", head("Overview", css10b) + nav("Overview") + main_body10())

# ================= 11 zones editor =================
css11 = """
.ed{display:flex;height:100%}
.canvas{flex:1;min-width:0;display:flex;flex-direction:column;background:#05070B}
.tb{height:64px;padding:0 var(--page-x);border-bottom:1px solid var(--line);flex:none}
.cv{flex:1;display:flex;align-items:center;justify-content:center;padding:var(--page-x)}
.sb{display:flex;align-items:center;gap:var(--s-3);height:44px;padding:0 var(--page-x);border-top:1px solid var(--line);flex:none}
.side{width:360px;flex:none;border-left:1px solid var(--line);background:var(--surface);display:flex;flex-direction:column}
.side .card-head{height:64px}
.sec{padding:var(--s-4) var(--s-5);border-top:1px solid var(--line);display:flex;flex-direction:column;gap:var(--s-3)}
.found{padding:var(--s-3);border-radius:var(--r-md);background:var(--surface-2);border:1px solid var(--line)}
"""
html = head("Zones", css11) + nav("Zones") + f'''<main class="page kgrey"><div class="ed">
<section class="canvas">
<div class="tb ztools">{zonetools(draw=False)[len('<div class="ztools">'):-len('</div>')]}</div>
<div class="cv">{stage(style="width:1016px", sel="Subtitles", off=("prompts",))}</div>
<div class="sb"><span class="small faint">Drag on the screenshot to draw a zone. Drag a corner to resize.</span><span class="spacer small faint">Screenshot 21:30 · 3840 × 2160 · Fit to window</span></div>
</section>
<aside class="side">
<div class="card-head"><h3>Zones</h3><span class="count">5</span><span class="spacer btn">{ic("plus")}Add zone</span></div>
<div>{zonelist_x(sel="Subtitles", off=("prompts",))}</div>
<div class="sec" style="margin-top:auto"><span class="btn btn-danger" style="justify-content:flex-start;padding-left:0">{ic("trash")}Delete zone</span></div>
</aside>
</div></main>
'''
write("11-zones.html", html)

# ================= 12 settings =================
css12 = """
.mw{display:flex;gap:var(--s-7);overflow:hidden}
.snav{width:200px;flex:none;display:flex;flex-direction:column;gap:2px}
.snav span{display:flex;align-items:center;height:var(--h-ctl);padding:0 var(--s-3);border-radius:var(--r-md);color:var(--text-2);font-weight:500}
.snav span.on{background:var(--surface-2);color:var(--text)}
.scol{width:720px;margin:0 auto;display:flex;flex-direction:column;gap:var(--s-6)}
.set-row.hover{background:var(--surface-2)}
"""
def srow(t, d, ctl, cls=""):
    dd = f'<div class="d">{d}</div>' if d else ""
    return f'<div class="set-row {cls}"><div class="grow"><div class="t">{t}</div>{dd}</div>{ctl}</div>'
def ssec(title, rows):
    return f'<section><div class="sec-h">{title}</div><div class="card">{"".join(rows)}</div></section>'
T_ON, T_OFF = '<span class="toggle on"></span>', '<span class="toggle"></span>'
def keys(*k, edit=False):
    e = f'<span class="btn btn-ghost">{ic("pencil")}Edit</span>' if edit else f'<span class="btn btn-ghost" style="visibility:hidden">{ic("pencil")}Edit</span>'
    return '<span class="row g1">' + "".join(kbd(x) for x in k) + '</span>' + e
html = head("Settings", css12) + nav("Settings") + f'''<main class="page"><div class="mw">
<nav class="snav"><span class="on">Startup</span><span>In-game overlay</span><span>Shortcuts</span><span>Voices</span><span>Updates</span></nav>
<div class="scol">
{ssec("Startup", [srow("Start with Windows", "Runs in the tray, ready when a game starts.", T_ON), srow("Open minimized", "Skip the main window at startup.", T_ON), srow("Show in the taskbar", "Otherwise only the tray icon shows.", T_OFF)])}
{ssec("In-game overlay", [srow("Position", "Where the small status pill sits.", '<div class="seg"><span class="on">Top right</span><span>Bottom center</span><span>Off</span></div>'), srow("Show the speaker", "Otherwise only the status dot shows.", T_ON)])}
{ssec("Shortcuts", [srow("Pause or resume reading", "Works in every game.", keys("Ctrl", "Shift", "P")), srow("Replay the last line", "", keys("Ctrl", "Shift", "R"), "hover") .replace(' style="visibility:hidden"', ''), srow("Stop speaking", "", keys("Ctrl", "Shift", "S")), srow("Open DialogReader", "", keys("Ctrl", "Shift", "D"))])}
{ssec("Voices", [srow("Kokoro", "Installed · 11 voices · 312 MB", '<span class="btn">Manage</span>'), srow("Piper", "Not installed · 48 voices · about 60 MB each", f'<span class="btn">{ic("download")}Download</span>'), srow("Default voice for new games", "", f'<div class="input select" style="width:200px"><span>Kokoro · Adam</span>{ic("chev-down")}</div>')])}
{ssec("Updates", [srow("Version 0.4.0", "You have the latest version.", f'<span class="btn">{ic("refresh")}Check now</span>'), srow("Install updates automatically", "Only when no game is running.", T_ON)])}
</div>
</div></main>
'''
write("12-settings.html", html)

# ================= 13 overlay =================
css13 = """
.frame{position:absolute;inset:0}
.frame svg{width:100%;height:100%;display:block}
.pos{position:absolute;top:24px;right:24px}
"""
html = head("Overlay", css13) + f'''<div class="frame">{SCENE}</div>
<div class="pos">{overlay_pill()}</div>
'''
write("13-overlay.html", html)

# ================= 14 game switcher =================
css14 = css10 + """
.scrim{position:absolute;inset:0;background:var(--scrim)}
.pal{position:absolute;top:44px;left:150px;width:520px;background:var(--surface);border:1px solid var(--line-2);border-radius:var(--r-lg);box-shadow:var(--shadow-pop);overflow:hidden}
.pal .q{display:flex;align-items:center;gap:var(--s-3);height:52px;padding:0 var(--s-4);border-bottom:1px solid var(--line)}
.pal .grp{padding:var(--s-2)}
.pal .gl{padding:var(--s-2) var(--s-3) var(--s-1)}
.it{display:flex;align-items:center;gap:var(--s-3);height:48px;padding:0 var(--s-3);border-radius:var(--r-md);border:1px solid transparent}
.it.hi{background:var(--surface-3);border-color:var(--line-2)}
.it .thumb{width:32px;height:32px;border-radius:var(--r-sm)}
.pal .ft{display:flex;align-items:center;gap:var(--s-4);height:40px;padding:0 var(--s-4);border-top:1px solid var(--line);color:var(--text-2);font-size:var(--fs-1)}
"""
def it(g, name, meta, right="", hi=False):
    return f'<div class="it{" hi" if hi else ""}">{thumb(g)}<div class="grow"><div style="font-weight:500">{name}</div><div class="small faint">{meta}</div></div>{right}</div>'
pal = f'''<div class="scrim"></div><div class="pal">
<div class="q">{ic("search")}<span class="faint grow">Search games</span>{kbd("Esc", True)}</div>
<div class="grp"><div class="gl label">Running now</div>
{it("rdr2", "Red Dead Redemption 2", "Reading · 5 zones", '<span class="faint row">' + ic("check") + '</span>')}
{it("satis", "Satisfactory", "Paused · 4 zones", kbd("Enter", True), True)}
</div>
<div class="grp" style="padding-top:0"><div class="gl label">Your games</div>
{it("p4g", "Persona 4 Golden", "Last played 2 days ago · 3 zones")}
{it("e33", "Clair Obscur: Expedition 33", "Last played last week · 4 zones")}
{it("kcd2", "Kingdom Come: Deliverance II", "Last played Oct 30 · 2 zones")}
</div>
<div class="grp" style="border-top:1px solid var(--line)"><div class="it"><span class="itile">{ic("plus")}</span><span class="grow" style="font-weight:500">Add a game</span>{kbd("Ctrl N", True)}</div></div>
<div class="ft"><span class="row g1">{kbd("↑", True)}{kbd("↓", True)}Move</span><span class="row g1">{kbd("Enter", True)}Switch</span></div>
</div>'''
html = head("Game switcher", css14) + nav("Overview") + main_body() + pal + "\n"
write("14-game-switcher.html", html)

# ================= index =================
screens = [("01-welcome", "Welcome"), ("02-pick-window", "Pick your game"), ("03-draw-zones", "Draw zones"), ("04-read-key", "Read key"), ("05-voice", "Voice"), ("06-mode", "Mode and finish"),
           ("10-main", "Main window · Overview"), ("14-game-switcher", "Game switcher (Ctrl K)"), ("11-zones", "Zone editor"), ("12-settings", "App settings"), ("13-overlay", "In-game overlay")]
cssi = """
body{width:auto;height:auto;overflow:auto;display:block;padding:var(--s-7)}
.sheet{display:grid;grid-template-columns:repeat(3,1fr);gap:var(--s-5);margin-top:var(--s-6)}
.sheet a{color:inherit;text-decoration:none;display:flex;flex-direction:column;gap:var(--s-2)}
.sheet img{width:100%;aspect-ratio:1440/900;border-radius:var(--r-md);border:1px solid var(--line-2);display:block}
.sw{display:flex;gap:var(--s-4);flex-wrap:wrap;margin-top:var(--s-4)}
"""
cards = "".join(f'<a class="card" style="padding:var(--s-3)" href="{f}.html"><img src="shots/{f}.png" alt="{t}"><div class="row g2" style="padding:0 var(--s-1)"><span class="mono small faint">{f[:2]}</span><span style="font-weight:500">{t}</span></div></a>' for f, t in screens)
sw = "".join(f'<span class="chip k-{k}">{KIND[k]}</span>' for k in KIND)
html = head("Midnight mockups", cssi) + f'''<div class="row g3">{logo(32)}<div><h2>DialogReader · Midnight</h2><div class="muted">Direction C, round 2. Deep blue-black, top tabs, game switcher on Ctrl K, one 960px onboarding column, one ZoneRow and one kind picker everywhere.</div></div></div>
<div class="sw">{sw}<span class="chip k-ok">Reading</span></div>
<div class="sheet">{cards}</div>
'''
write("index.html", html)
print("ok")

# ================= round 5 variants (Max: bigger game picture, slimmer zone list, sleeker read key) =================
# New files only: 10-main-a/b/c.html, 04-read-key-a/b/c.html, 04-read-key-c2.html. Existing pages above are unchanged.
# Round 5b (review fixes): one short top bar for all overview variants, cards sized to content, top aligned.
CSS_SBAR = """
.sbar{display:flex;align-items:center;gap:var(--s-2);height:48px;padding:0 var(--s-2) 0 var(--s-5);flex:none}
.sbar .st{display:flex;align-items:center;gap:var(--s-2);font-weight:500;white-space:nowrap}
.sbar .vsep{margin:0 var(--s-2)}
.sbar .opt{display:inline-flex;align-items:center;gap:var(--s-2);height:var(--h-ctl);padding:0 var(--s-2);border-radius:var(--r-md);color:var(--text);white-space:nowrap}
.sbar .opt > .ic{color:var(--text-3)}
"""
def sbar():
    # short bar: status, two actions, then mode / voice / key as icon + value (no text labels)
    return (f'<div class="card sbar"><span class="st"><span class="dot"></span>Reading</span><span class="spacer"></span>'
            f'<span class="btn">{ic("play")}Read now</span><span class="btn">{ic("pause")}Pause</span>'
            f'<span class="vsep"></span><span class="opt" title="Mode">{ic("zap")}Automatic{ic("chev-down")}</span>'
            f'<span class="opt" title="Voice">{ic("volume")}Kokoro · Adam{ic("chev-down")}</span>'
            f'<span class="opt" title="Read key">{ic("mouse")}{kbd("Mouse 4", True)}{ic("chev-down")}</span></div>')

# two-line recent row: grey kind icon, line, time (+ skip reason), replay
def recent_rows(n=5):
    r = []
    for t, k, who, line, skip in LOG[:n]:
        w = f'<span class="who">{who}</span> ' if who else ""
        meta = f'{t} · {skip}' if skip else t
        end = "" if skip else f'<span class="btn btn-icon btn-ghost" title="Replay">{ic("replay")}</span>'
        r.append(f'<div class="rr{" skip" if skip else ""}"><span class="ki" title="{KIND[k]}">{ic("k-" + k)}</span>'
                 f'<div class="grow"><div class="ln trunc">{w}{line}</div><div class="mt">{meta}</div></div>{end}</div>')
    return "".join(r)
CSS_RR = """
.rr{display:flex;align-items:center;gap:var(--s-3);height:52px;padding:0 var(--s-2) 0 var(--s-4);border-top:1px solid var(--line)}
.rr:first-child{border-top:0}
.rr .ki{display:inline-flex;color:var(--text-3);flex:none}
.rr .who{color:var(--text-2);font-weight:500}
.rr .mt{font:400 var(--fs-1)/1.4 var(--mono);color:var(--text-3)}
.rr.skip .ln{color:var(--text-3)}
.rr .btn-icon{width:28px;height:28px}
.foot-link{border-top:1px solid var(--line);height:40px;flex:none;display:flex;align-items:center;justify-content:center;color:var(--text-2);font-weight:500;gap:var(--s-1)}
"""
EDITBTN = f'<span class="btn btn-icon btn-ghost" title="Edit zones">{ic("pencil")}</span>'
SHOWALL = f'<div class="foot-link">Show all{ic("chev-right")}</div>'

# ---- 10-main-a: hero picture + 56px now bar, right rail (zones, then recent), all top aligned ----
css10A = CSS_SBAR + CSS_RR + """
.mw{display:flex;flex-direction:column;gap:var(--s-3);padding:var(--s-5)}
.hero{flex:1;min-height:0;display:flex;align-items:flex-start;gap:var(--s-3)}
.pic{width:1100px;flex:none;overflow:hidden;display:flex;flex-direction:column}
.pic .stage{width:100%;border-radius:0;flex:none}
.now{height:56px;flex:none;display:flex;align-items:center;gap:var(--s-3);padding:0 var(--s-2) 0 var(--s-5);border-top:1px solid var(--line)}
.now .who{color:var(--text-2);font-weight:500}
.rail{flex:1;min-width:0;display:flex;flex-direction:column;gap:var(--s-3)}
.rail .card{overflow:hidden;display:flex;flex-direction:column}
.rail .card-head{height:44px;padding:0 var(--s-2) 0 var(--s-4)}
.rail .zrow{height:40px;padding:0 var(--s-4)}
"""
html = head("Overview A", css10A) + nav("Overview") + f'''<main class="page"><div class="mw">
{sbar()}
<div class="hero">
<section class="card pic">
{stage(labels=False, off=("prompts",))}
<div class="now"><span class="dot"></span><div class="grow trunc"><span class="who">Arthur:</span> We need to get these horses moving before the snow buries us.</div>{pillwave(18, 16)}<span class="btn btn-icon btn-ghost" title="Replay">{ic("replay")}</span><span class="btn btn-icon btn-ghost" title="Stop">{ic("stop")}</span></div>
</section>
<aside class="rail">
<section class="card"><div class="card-head"><h3>Zones</h3><span class="count">5</span><span class="spacer"></span>{EDITBTN}</div>
<div>{zonelist_x(off=("prompts",))}</div></section>
<section class="card"><div class="card-head"><h3>Recent</h3></div>
<div>{recent_rows(5)}</div>
{SHOWALL}</section>
</aside>
</div>
</div></main>
'''
write("10-main-a.html", html)

# ---- 10-main-b: bigger picture with a zone strip under it (overflow chip), rail = now reading + recent ----
css10B = CSS_SBAR + CSS_RR + """
.mw{display:flex;flex-direction:column;gap:var(--s-3);padding:var(--s-5)}
.hero{flex:1;min-height:0;display:flex;align-items:flex-start;gap:var(--s-3)}
.pic{width:1120px;flex:none;overflow:hidden;display:flex;flex-direction:column}
.pic .stage{width:100%;border-radius:0;flex:none}
.zstrip{display:flex;align-items:center;height:52px;flex:none;padding:0 var(--s-2) 0 var(--s-4);border-top:1px solid var(--line)}
.zviz{flex:1;min-width:0;display:flex;align-items:center;overflow:hidden;position:relative}
.zviz::after{content:"";position:absolute;right:0;top:0;bottom:0;width:48px;background:linear-gradient(90deg, rgba(12,16,24,0), var(--surface))}
.zp{display:inline-flex;align-items:center;gap:var(--s-2);height:36px;padding:0 var(--s-4) 0 var(--s-1);margin-right:var(--s-3);border-right:1px solid var(--line);font-weight:500;white-space:nowrap;flex:none}
.zp .zic{display:inline-flex;color:var(--text-2)}
.zp .toggle{margin-left:var(--s-1)}
.zp.off{color:var(--text-2)}
.zp.off .zic{color:var(--text-3)}
.zmore{display:inline-flex;align-items:center;gap:var(--s-1);height:28px;padding:0 var(--s-2) 0 var(--s-3);margin:0 var(--s-2);border-radius:var(--r-pill);background:var(--surface-3);color:var(--text-2);font-size:var(--fs-1);font-weight:600;flex:none}
.rail{flex:1;min-width:0;display:flex;flex-direction:column;gap:var(--s-3)}
.rail .card{overflow:hidden;display:flex;flex-direction:column}
.rail .card-head{height:44px;padding:0 var(--s-2) 0 var(--s-4)}
.np{padding:var(--s-4);display:flex;flex-direction:column;gap:var(--s-3)}
.np .who{color:var(--text-2);font-weight:500}
.np .txt{font-size:var(--fs-3);line-height:1.4}
.np .ctl{display:flex;align-items:center;gap:var(--s-2)}
"""
# a game with 8 zones: 5 fit, the rest sit behind the +3 chip (opens a list)
ZONES_B = [(k, n) for k, n, *_ in ZONES] + [("dialogue", "Letters"), ("pickups", "Shop prices"), ("dialogue", "Journal")]
def zstrip(off=("prompts",)):
    items = "".join(f'<span class="zp{" off" if k in off else ""}"><span class="zic" title="{KIND[k]}">{ic("k-" + k)}</span>{n}<span class="toggle{"" if k in off else " on"}"></span></span>' for k, n in ZONES_B)
    return f'<div class="zstrip"><div class="zviz">{items}</div><span class="zmore" title="3 more zones">+3{ic("chev-down")}</span>{EDITBTN}</div>'
html = head("Overview B", css10B) + nav("Overview") + f'''<main class="page kgrey"><div class="mw">
{sbar()}
<div class="hero">
<section class="card pic">
{stage(labels=False, off=("prompts",))}
{zstrip()}
</section>
<aside class="rail">
<section class="card np">
<div class="txt"><span class="who">Arthur:</span> We need to get these horses moving before the snow buries us.</div>
<div class="ctl">{pillwave(18, 16)}<span class="spacer"></span><span class="btn btn-icon btn-ghost" title="Replay">{ic("replay")}</span><span class="btn btn-icon btn-ghost" title="Stop">{ic("stop")}</span></div>
</section>
<section class="card"><div class="card-head"><h3>Recent</h3></div>
<div>{recent_rows(5)}</div>
{SHOWALL}</section>
</aside>
</div>
</div></main>
'''
write("10-main-b.html", html)

# ---- 10-main-c: focus layout. Centered column, zones in a movable, collapsible panel, recent under it ----
GRIP = ('<svg class="ic" viewBox="0 0 24 24"><circle cx="9" cy="6" r=".9"/><circle cx="15" cy="6" r=".9"/>'
        '<circle cx="9" cy="12" r=".9"/><circle cx="15" cy="12" r=".9"/><circle cx="9" cy="18" r=".9"/><circle cx="15" cy="18" r=".9"/></svg>')
css10C = CSS_SBAR + """
.mw{display:flex;flex-direction:column;align-items:center;padding:var(--s-4) var(--s-5) var(--s-5)}
.colw{width:940px;display:flex;flex-direction:column;gap:var(--s-3)}
.pic .stage{width:100%;border-radius:var(--r-lg);border:1px solid var(--line)}
.zov{position:absolute;top:var(--s-3);right:var(--s-3);width:232px;background:rgba(12,16,24,0.94);border:1px solid var(--line-2);border-radius:var(--r-md);box-shadow:var(--shadow-pop);overflow:hidden}
.zov .hd{display:flex;align-items:center;gap:var(--s-1);height:36px;padding:0 var(--s-1) 0 var(--s-1);border-bottom:1px solid var(--line);font-size:13px;font-weight:600}
.zov .hd .grip{display:inline-flex;width:20px;justify-content:center;color:var(--text-3)}
.zov .hd .count{margin-left:var(--s-1)}
.zov .hd .btn-icon{width:28px;height:28px}
.zov .zrow{height:32px;padding:0 var(--s-3);gap:var(--s-2);font-size:13px}
.rec{overflow:hidden;display:flex;flex-direction:column}
.rec .card-head{height:36px;padding:0 var(--s-1) 0 var(--s-4)}
.rec .card-head h3{font-size:13px}
.rc{display:grid;grid-template-columns:60px 16px minmax(0,1fr) auto;align-items:center;gap:var(--s-3);height:32px;padding:0 var(--s-2) 0 var(--s-4);border-top:1px solid var(--line);font-size:13px}
.rc .time{font:400 var(--fs-1)/1 var(--mono);color:var(--text-3)}
.rc .ki{display:inline-flex;color:var(--text-3)}
.rc .who{color:var(--text-2);font-weight:500}
.rc.skip .line{color:var(--text-3)}
.rc .btn-icon{width:26px;height:26px}
.rc .why{font-size:var(--fs-1);color:var(--text-3)}
"""
def rc_rows(n=5):
    r = []
    for t, k, who, line, skip in LOG[:n]:
        w = f'<span class="who">{who}</span> ' if who else ""
        end = f'<span class="why">{skip}</span>' if skip else f'<span class="btn btn-icon btn-ghost" title="Replay">{ic("replay")}</span>'
        r.append(f'<div class="rc{" skip" if skip else ""}"><span class="time">{t}</span><span class="ki" title="{KIND[k]}">{ic("k-" + k)}</span><span class="line trunc">{w}{line}</span>{end}</div>')
    return "".join(r)
zov = (f'<div class="zov"><div class="hd"><span class="grip" title="Drag to move">{GRIP}</span>Zones<span class="count">5</span><span class="spacer"></span>'
       f'{EDITBTN}<span class="btn btn-icon btn-ghost" title="Collapse">{ic("minus")}</span></div>{zonelist_x(off=("prompts",))}</div>')
html = head("Overview C", css10C) + nav("Overview") + f'''<main class="page"><div class="mw"><div class="colw">
{sbar()}
<div class="pic">{stage(labels=False, off=("prompts",), extra=zov)}</div>
<section class="card rec"><div class="card-head"><h3>Recent</h3><span class="spacer"></span><span class="btn btn-ghost" style="height:28px">Show all{ic("chev-right")}</span></div>
<div>{rc_rows(5)}</div></section>
</div></div></main>
'''
write("10-main-c.html", html)

# ---- 04-read-key-a: one big key, centered ----
css04A = """
.ctr{height:100%;display:flex;flex-direction:column;align-items:center;justify-content:center;gap:var(--s-5);padding-bottom:var(--s-6)}
.hugekey{display:inline-flex;align-items:center;gap:var(--s-4);height:112px;padding:0 var(--s-7);border-radius:16px;border:1px solid var(--line-2);border-bottom-width:4px;background:var(--surface-2);font-size:48px;font-weight:600;letter-spacing:-0.03em}
.hugekey .ic{width:36px;height:36px;color:var(--text-3)}
"""
html = head("Read key A", css04A) + obbar(3) + f'''<main class="page"><div class="ctr">
<h1>Your read key</h1>
<div class="hugekey">{ic("mouse")}Mouse 4</div>
<span class="btn btn-lg">{ic("keyboard")}Change</span>
</div></main>
''' + obfoot()
write("04-read-key-a.html", html)

# ---- 04-read-key-b: listening state. Capture field with ring, clear button, three quick picks ----
css04B = """
.ctr{height:100%;display:flex;flex-direction:column;align-items:center;justify-content:center;gap:var(--s-5);padding-bottom:var(--s-6)}
.kf{width:520px;height:88px;display:flex;align-items:center;gap:var(--s-4);padding:0 var(--s-3) 0 var(--s-5);border-radius:var(--r-lg);border:1px solid var(--accent);outline:1px solid rgba(91,124,255,0.45);outline-offset:2px;background:var(--surface)}
.kf .kv{font-size:var(--fs-5);font-weight:600;letter-spacing:-0.02em}
.kf .ic.big{width:28px;height:28px;color:var(--text-3)}
.picks{display:flex;align-items:center;gap:var(--s-2)}
.picks .or{font-size:13px;color:var(--text-2);margin-right:var(--s-1)}
.picks .kbd{height:32px;padding:0 var(--s-3);font-size:13px;color:var(--text)}
"""
html = head("Read key B", css04B) + obbar(3) + f'''<main class="page"><div class="ctr">
<h1>Press a key</h1>
<div class="kf">{ic("mouse").replace('class="ic"', 'class="ic big"')}<span class="kv grow">Mouse 4</span><span class="btn btn-icon btn-ghost" title="Clear">{ic("x")}</span></div>
<div class="picks"><span class="or">Or</span>{kbd("Mouse 5")}{kbd("F8")}{kbd("Caps Lock")}</div>
</div></main>
''' + obfoot()
write("04-read-key-b.html", html)

# ---- 04-read-key-c: drawing of the key. Mouse with the rear side button (Mouse 4) lit; c2 = keyboard key ----
MOUSE_SVG = """<svg width="200" height="280" viewBox="0 0 200 280" aria-hidden="true" style="display:block;flex:none">
<rect x="40" y="20" width="120" height="240" rx="60" fill="#0C1018" stroke="#283143" stroke-width="2"/>
<path d="M40 100 H160 M100 20 V100" stroke="#283143" stroke-width="2" fill="none"/>
<rect x="94" y="44" width="12" height="30" rx="6" fill="#1A2232" stroke="#283143" stroke-width="2"/>
<rect x="30" y="118" width="14" height="34" rx="5" fill="#1A2232" stroke="#283143" stroke-width="2"/>
<rect x="30" y="158" width="14" height="34" rx="5" fill="#5B7CFF"/>
</svg>"""
KEYCAP_SVG = """<svg width="200" height="200" viewBox="0 0 200 200" aria-hidden="true" style="display:block;flex:none">
<rect x="12" y="12" width="176" height="176" rx="28" fill="#0C1018" stroke="#5B7CFF" stroke-width="3"/>
<rect x="34" y="28" width="132" height="124" rx="18" fill="#1A2232" stroke="#283143" stroke-width="2"/>
<text x="100" y="106" text-anchor="middle" font-family="Geist, Segoe UI, sans-serif" font-size="44" font-weight="600" fill="#EDF0F6">F8</text>
</svg>"""
css04C = """
.ctr{height:100%;display:flex;align-items:center;justify-content:center;gap:var(--s-7);padding-bottom:var(--s-6)}
.side{display:flex;flex-direction:column;align-items:flex-start;gap:var(--s-3)}
.side .lb{font-size:13px;font-weight:500;color:var(--text-2)}
.side .kv{font-size:56px;font-weight:600;letter-spacing:-0.03em;line-height:1}
.side .btn{margin-top:var(--s-3)}
"""
def readkey_c(title, drawing, key):
    return head(title, css04C) + obbar(3) + f'''<main class="page"><div class="ctr">
{drawing}
<div class="side"><span class="lb">Read key</span><div class="kv">{key}</div><span class="btn btn-lg">{ic("keyboard")}Change</span></div>
</div></main>
''' + obfoot()
write("04-read-key-c.html", readkey_c("Read key C", MOUSE_SVG, "Mouse 4"))
write("04-read-key-c2.html", readkey_c("Read key C2", KEYCAP_SVG, "F8"))
print("round 5 ok")

# ---- 04-read-key-d (round 6, Max): one card per action. Name + what it does + its key + Change. No "Or" ----
def obbar_l(cur, label):  # obbar with step 3 renamed (obbar itself stays untouched)
    st = []
    for i, s in enumerate(["Game", "Zones", label, "Voice", "Mode"], 1):
        if i < cur: cls, n = "step done", ic("check")
        elif i == cur: cls, n = "step on", str(i)
        else: cls, n = "step", str(i)
        if i > 1: st.append('<span class="step-sep"></span>')
        st.append(f'<div class="{cls}"><span class="n">{n}</span>{s}</div>')
    return f'<header class="bar">{BRAND}<div class="steps">{"".join(st)}</div><div class="bar-right">{WINCTL}</div></header>\n'
css04D = """
.ctr{height:100%;display:flex;flex-direction:column;align-items:center;justify-content:center;gap:var(--s-6);padding-bottom:var(--s-6)}
.kgrid{width:680px;display:flex;flex-direction:column;gap:var(--s-3)}
.kc{display:flex;align-items:center;gap:var(--s-4);height:72px;padding:0 var(--s-3) 0 var(--s-5)}
.kc .nm{font-size:var(--fs-3);font-weight:500}
.kc .ds{font-size:13px;color:var(--text-3);margin-top:2px}
.kc .itile{width:36px;height:36px}
.kc .itile .ic{width:18px;height:18px}
.kc .cap{display:flex;align-items:center;justify-content:center;gap:var(--s-2);height:36px;min-width:104px;padding:0 var(--s-3);border-radius:var(--r-md);background:var(--surface-3);border:1px solid var(--line-2);border-bottom:3px solid #05070B;font:600 13px/1 var(--font);color:var(--text)}
.kc .cap .ic{width:15px;height:15px;color:var(--text-2)}
.kc .btn-ghost{color:var(--text-3)}
.kc.main{height:104px;padding:0 var(--s-3) 0 var(--s-6);border-color:var(--accent);background:linear-gradient(90deg,rgba(91,124,255,0.10),rgba(91,124,255,0.02) 60%),var(--surface)}
.kc.main .nm{font-size:var(--fs-4);font-weight:600;letter-spacing:-0.015em}
.kc.main .ds{font-size:var(--fs-2);color:var(--text-2);margin-top:var(--s-1)}
.kc.main .itile{width:44px;height:44px}
.kc.main .itile .ic{width:22px;height:22px}
.kc.main .cap{height:46px;min-width:132px;font-size:var(--fs-3);border-bottom-width:4px}
.kc.main .cap .ic{width:18px;height:18px}
"""
FILLED = "fill:currentColor;stroke:none"
def keycard(name, desc, key, icon, keyicon=None, main=False, fill=False):
    k = f'<span class="cap">{ic(keyicon) if keyicon else ""}{key}</span>'
    return (f'<div class="card kc{" main" if main else ""}"><span class="itile{" on" if main else ""}">{ic(icon, FILLED if fill else "")}</span>'
            f'<div class="grow"><div class="nm">{name}</div><div class="ds">{desc}</div></div>'
            f'{k}<span class="btn btn-ghost">Change</span></div>')
html = head("Read key D", css04D) + obbar_l(3, "Keys") + f'''<main class="page"><div class="ctr">
<h1>Your keys</h1>
<div class="kgrid">
{keycard("Read zones now", "Reads all your zones out loud", "Mouse 4", "volume", "mouse", main=True)}
{keycard("Repeat last line", "Says the last line again", "Mouse 5", "replay", "mouse")}
{keycard("Stop speaking", "Ends the line it is reading now", "F9", "stop", fill=True)}
{keycard("Pause reading", "Turns reading off until you press it again", "F8", "pause", fill=True)}
</div>
</div></main>
''' + obfoot()
write("04-read-key-d.html", html)
print("round 6 ok")


# ======================================================================
# ===== BEGIN 15-quick-menu (round 7, Max): in-game quick menu A/B/C =====
# Self-contained: only adds icons to P and writes 15-quick-menu-*.html.
# Round 7b: reviewer fixes (no pill while open, current line, toggle state,
# Hold F8 menu key, speed row everywhere, danger Quit, round-O keycaps).
# ======================================================================
P.update({
 "mute": '<path d="M11 5 6 9H3v6h3l5 4z"/><path d="m22 9-6 6M16 9l6 6"/>',
 "power": '<path d="M12 2v10"/><path d="M18.4 6.6a9 9 0 1 1-12.77.04"/>',
 "gauge": '<path d="m12 14 4-4"/><path d="M3.34 19a10 10 0 1 1 17.32 0"/>',
 "app": '<rect x="3" y="4" width="18" height="16" rx="2"/><path d="M8 10v4M12 8v8M16 11v2"/>',
})
QM_GAME = "Red Dead Redemption 2"
QM_LINE = "We need to get these horses moving before the snow buries us."
# (icon, label, key, state): state None = one-shot action, False/True = toggle (label swaps to Unmute / Resume when on)
QM_ACTIONS = [("mute", "Mute", "M", False), ("stop", "Stop speaking", "S", None), ("pause", "Pause reading", "P", False),
              ("replay", "Repeat last line", "R", None), ("volume", "Read zones now", "Space", None)]
QM_ZONES = [("dialogue", "Subtitles", "1", True), ("speaker", "Speaker name", "2", True),
            ("pickups", "Item pickups", "3", True), ("prompts", "Button prompts", "4", False)]
QM_APP = [("app", "Open DialogReader", "D"), ("settings", "Settings", "O"), ("power", "Quit", "Q")]
QM_HINTS = [("&uarr;&darr;", "Move"), ("&larr;&rarr;", "Change"), ("Enter", "Select"), ("Esc", "Close")]
def qm_frame(): return f'<div class="qm-frame">{SCENE}</div>'
def qm_k(t, cls=""): return f'<span class="kbd qk {cls}">{t}</span>'
def qm_hints(h=QM_HINTS): return '<div class="qm-hints">' + "".join(f'<span>{qm_k(k)}{d}</span>' for k, d in h) + '</div>'
def qm_keys():  # how the menu relates to the manual read key: tap F8 reads, hold F8 opens / closes this menu
    return (f'<div class="qm-hints qm-keys"><span>{qm_k("F8")}Read now</span>'
            f'<span>Hold {qm_k("F8")}Open / close menu</span></div>')
def qm_now(): return f'<div class="qm-now"><span class="spk">Arthur</span>{QM_LINE}</div>'
QM_SLIDER = '<div class="slider"><div class="fill" style="width:70%"></div><span class="knob" style="left:70%"></span></div><span class="v" style="width:32px;text-align:right">70%</span>'
QM_BASE = """
.qm-frame{position:absolute;inset:0}
.qm-frame svg{width:100%;height:100%;display:block}
.qk{font-family:inherit;font-weight:600}
.qm-hints{display:flex;align-items:center;gap:var(--s-4);color:var(--text-3);font-size:var(--fs-1);font-weight:500}
.qm-hints span{display:inline-flex;align-items:center;gap:6px;white-space:nowrap}
.qm-hints .kbd{min-width:var(--kbd-sm);height:var(--kbd-sm);padding:0 6px}
.qm-live{display:inline-flex;align-items:center;gap:6px;color:var(--ok);font-size:var(--fs-1);font-weight:500}
.qm-live .dot{width:6px;height:6px}
.qm-now{font-size:13px;line-height:1.45;color:var(--text-2);background:var(--surface);border:1px solid var(--line);border-radius:var(--r-md);padding:8px 10px}
.qm-now .spk{color:var(--text);font-weight:600;margin-right:6px}
.hold{color:var(--text-3);font-size:11px;font-weight:500}
"""

# ---------- A: side panel docked right (WeMod style), narrow so the subtitle stays visible ----------
cssQA = QM_BASE + """
.qa{position:absolute;top:0;right:0;bottom:0;width:330px;background:#0A0D14;border-left:1px solid var(--line-2);box-shadow:-16px 0 40px rgba(0,0,0,0.45);display:flex;flex-direction:column}
.qa .hd{display:flex;align-items:center;gap:var(--s-3);padding:var(--s-4) var(--s-4) var(--s-3)}
.qa .hd .thumb{width:40px;height:40px;border-radius:var(--r-md)}
.qa .hd .gn{font-size:15px;font-weight:600;letter-spacing:-0.01em;line-height:1.25}
.qa .qm-now{margin:0 var(--s-4) var(--s-2)}
.qa .sec{padding:0 var(--s-2)}
.qa .lb{display:flex;align-items:center;height:26px;padding:0 var(--s-2);color:var(--text-3);font-size:11px;font-weight:500;letter-spacing:0.05em;text-transform:uppercase}
.qa .r{position:relative;display:flex;align-items:center;gap:10px;height:36px;padding:0 var(--s-2);border-radius:var(--r-md);font-weight:500;font-size:13px}
.qa .r > .ic{color:var(--text-3);width:16px;height:16px}
.qa .r .v{color:var(--text-2);font-size:12px}
.qa .r.hi{background:var(--surface-3)}
.qa .r.hi::before{content:"";position:absolute;left:-8px;top:8px;bottom:8px;width:3px;border-radius:2px;background:var(--accent)}
.qa .r.hi > .ic{color:var(--text)}
.qa .r.off .nm{color:var(--text-3)}
.qa .r.q .nm,.qa .r.q > .ic{color:var(--danger)}
.qa .r .qk{min-width:24px;height:22px;padding:0 6px;font-size:11px}
.qa .div{height:1px;background:var(--line);margin:var(--s-2) var(--s-4)}
.qa .seg > span{height:22px;padding:0 var(--s-2);font-size:11px}
.qa .slider{width:72px}
.qa .ft{margin-top:auto;border-top:1px solid var(--line);padding:var(--s-3) var(--s-4);display:flex;flex-direction:column;gap:var(--s-2)}
.qa .ft .qm-hints{justify-content:space-between;gap:var(--s-2);font-size:11px}
.qa .ft .qm-hints .kbd{min-width:18px;height:18px;padding:0 4px;font-size:10px}
.qa .ft .qm-keys{justify-content:flex-start;gap:var(--s-4)}
"""
def qa_row(icon, name, key, right="", cls=""):
    return f'<div class="r {cls}">{ic(icon)}<span class="grow nm">{name}</span>{right}{qm_k(key)}</div>'
def qa_tog(on): return f'<span class="toggle{" on" if on else ""}"></span>'
qa_act = "".join(qa_row(i, n, k, "" if s is None else qa_tog(s), "hi" if n == "Stop speaking" else "") for i, n, k, s in QM_ACTIONS)
qa_set = (qa_row("keyboard", "Mode", "A", '<div class="seg"><span class="on">Automatic</span><span>On key press</span></div>')
          + qa_row("audio", "Voice", "V", '<span class="v">Adam</span>')
          + qa_row("gauge", "Speed", "[ ]", '<span class="v">1.1&times;</span>')
          + qa_row("volume", "Volume", "&minus; +", QM_SLIDER))
qa_zon = "".join(qa_row("k-" + k, n, key, qa_tog(on), "" if on else "off") for k, n, key, on in QM_ZONES)
qa_app = "".join(qa_row(i, n, k, '<span class="hold">Hold</span>' if n == "Quit" else "", "q" if n == "Quit" else "") for i, n, k in QM_APP)
html = head("Quick menu A", cssQA) + qm_frame() + f'''<aside class="qa">
<div class="hd">{thumb("rdr2")}<div class="grow"><div class="gn">{QM_GAME}</div><span class="qm-live"><span class="dot"></span>Reading</span></div></div>
{qm_now()}
<div class="sec">{qa_act}</div>
<div class="div"></div>
<div class="sec"><div class="lb">Quick settings</div>{qa_set}</div>
<div class="div"></div>
<div class="sec"><div class="lb">Zones</div>{qa_zon}</div>
<div class="div"></div>
<div class="sec">{qa_app}</div>
<div class="ft">{qm_hints()}{qm_keys()}</div>
</aside>
'''
write("15-quick-menu-a.html", html)

# ---------- B: centered card of big tiles, no scrim, kept above the subtitle ----------
cssQB = QM_BASE + """
.qb{position:absolute;left:50%;top:48px;width:720px;margin-left:-360px;background:#0A0D14;border:1px solid var(--line-2);border-radius:16px;box-shadow:var(--shadow-pop);padding:var(--s-4);display:flex;flex-direction:column;gap:var(--s-3)}
.qb .hd{display:flex;align-items:center;gap:var(--s-3)}
.qb .hd .thumb{width:36px;height:36px;border-radius:var(--r-md)}
.qb .hd .gn{font-size:var(--fs-3);font-weight:600;letter-spacing:-0.01em;line-height:1.2}
.qb .tiles{display:grid;grid-template-columns:repeat(5,1fr);gap:var(--s-2)}
.qb .t{position:relative;height:96px;border-radius:var(--r-lg);background:var(--surface-2);border:1px solid var(--line-2);display:flex;flex-direction:column;align-items:center;justify-content:center;gap:10px;font-weight:500;font-size:13px}
.qb .t .ic{width:24px;height:24px;color:var(--text-3);stroke-width:1.75}
.qb .t .qk{position:absolute;top:7px;right:7px;min-width:var(--kbd-sm);height:var(--kbd-sm);padding:0 5px;font-size:11px}
.qb .t .toggle{position:absolute;top:9px;left:9px;width:26px;height:16px}
.qb .t .toggle::after{top:3px;left:3px;width:10px;height:10px}
.qb .t .toggle.on::after{left:13px}
.qb .t.hi,.qb .a.hi{background:var(--surface-3);border-color:var(--accent);box-shadow:0 0 0 1px var(--accent)}
.qb .t.hi .ic{color:var(--text)}
.qb .two{display:grid;grid-template-columns:1fr 1fr;gap:var(--s-2)}
.qb .box{border-radius:var(--r-lg);background:var(--surface);border:1px solid var(--line);padding:var(--s-1) var(--s-3)}
.qb .r{display:flex;align-items:center;gap:10px;height:38px;border-top:1px solid var(--line);font-weight:500;font-size:13px}
.qb .r:first-child{border-top:0}
.qb .r > .ic{color:var(--text-3);width:16px;height:16px}
.qb .r .v{color:var(--text-2);font-size:12px}
.qb .r.off .nm{color:var(--text-3)}
.qb .r .qk,.qb .a .qk{min-width:24px;height:22px;padding:0 6px;font-size:11px}
.qb .seg > span{height:22px;padding:0 var(--s-2);font-size:11px}
.qb .slider{width:84px}
.qb .apps{display:grid;grid-template-columns:repeat(3,1fr);gap:var(--s-2)}
.qb .a{display:flex;align-items:center;gap:10px;height:40px;padding:0 var(--s-3);border-radius:var(--r-lg);background:var(--surface-2);border:1px solid var(--line-2);font-weight:500;font-size:13px}
.qb .a > .ic{color:var(--text-3);width:16px;height:16px}
.qb .a.q{background:var(--surface);border-color:var(--line)}
.qb .a.q .nm,.qb .a.q > .ic{color:var(--danger)}
.qb .hn{display:flex;flex-direction:column;align-items:center;gap:6px;border-top:1px solid var(--line);margin:0 calc(-1 * var(--s-4));padding:var(--s-3) var(--s-4) 0}
.qb .hn .qm-hints{gap:var(--s-3)}
.qb .hn .sep{width:1px;height:16px;background:var(--line-2);align-self:center}
"""
QB_HINTS = [("Tab", "Next group"), ("&uarr;&darr;&larr;&rarr;", "Move"), ("Enter", "Select"), ("Esc", "Close")]
def qb_row(icon, name, key, right="", cls=""):
    return f'<div class="r {cls}">{ic(icon)}<span class="grow nm">{name}</span>{right}{qm_k(key)}</div>'
qb_tiles = "".join(f'<div class="t{" hi" if n == "Mute" else ""}">{"" if s is None else qa_tog(s)}{qm_k(k)}{ic(i)}<span>{n}</span></div>' for i, n, k, s in QM_ACTIONS)
qb_set = (qb_row("keyboard", "Mode", "A", '<div class="seg"><span class="on">Automatic</span><span>On key press</span></div>')
          + qb_row("audio", "Voice", "V", '<span class="v">Adam</span>')
          + qb_row("gauge", "Speed", "[ ]", '<span class="v">1.1&times;</span>')
          + qb_row("volume", "Volume", "&minus; +", QM_SLIDER))
qb_zon = "".join(qb_row("k-" + k, n, key, qa_tog(on), "" if on else "off") for k, n, key, on in QM_ZONES)
qb_app = "".join(f'<div class="a{" q" if n == "Quit" else ""}">{ic(i)}<span class="grow nm">{n}</span>{"<span class=hold>Hold</span>" if n == "Quit" else ""}{qm_k(k)}</div>' for i, n, k in QM_APP)
html = head("Quick menu B", cssQB) + qm_frame() + f'''<div class="qb">
<div class="hd">{thumb("rdr2")}<div class="grow"><div class="gn">{QM_GAME}</div><span class="qm-live"><span class="dot"></span>Reading</span></div></div>
{qm_now()}
<div class="tiles">{qb_tiles}</div>
<div class="two"><div class="box">{qb_set}</div><div class="box">{qb_zon}</div></div>
<div class="apps">{qb_app}</div>
<div class="hn">{qm_hints(QB_HINTS)}{qm_keys()}</div>
</div>
'''
write("15-quick-menu-b.html", html)

# ---------- C: slim list in the pill's corner (RivaTuner / Steam overlay style) ----------
cssQC = QM_BASE + """
.qc{position:absolute;top:24px;right:24px;width:340px;background:#0A0D14;border:1px solid var(--line-2);border-radius:var(--r-lg);box-shadow:var(--shadow-pop);overflow:hidden}
.qc .hd{display:flex;align-items:center;gap:var(--s-2);height:44px;padding:0 var(--s-3);border-bottom:1px solid var(--line)}
.qc .hd .gn{flex:1;min-width:0;font-weight:600;white-space:nowrap;overflow:hidden;text-overflow:ellipsis}
.qc .grp{padding:var(--s-1) 0;border-bottom:1px solid var(--line)}
.qc .r{position:relative;display:flex;align-items:center;gap:10px;height:30px;padding:0 var(--s-3);font-size:13px;font-weight:500}
.qc .r .qk{flex:none;width:48px;min-width:48px;height:var(--kbd-sm);padding:0 4px;font-size:11px;background:transparent;color:var(--text-3)}
.qc .r .zi{display:inline-flex;width:14px;color:var(--text-3)}
.qc .r .zi svg{width:14px;height:14px}
.qc .r .nm{flex:1;min-width:0;white-space:nowrap}
.qc .r .v{color:var(--text-2);font-family:var(--mono);font-size:12px}
.qc .r.hi{background:var(--accent)}
.qc .r.hi .qk{border-color:rgba(255,255,255,0.45);color:#FFFFFF}
.qc .r.hi .nm,.qc .r.hi .zi{color:#FFFFFF}
.qc .r.off .nm{color:var(--text-3)}
.qc .r.q .nm,.qc .r.q .zi{color:var(--danger)}
.qc .r .v.on-t{color:var(--accent);font-weight:600}
.qc .r .v.off-t{color:var(--text-3);opacity:0.7}
.qc .r .v .ar{color:var(--text-3)}
.qc .vb{display:inline-flex;gap:2px}
.qc .vb i{display:block;width:4px;height:10px;border-radius:1px;background:var(--line-2)}
.qc .vb i.f{background:var(--text-2)}
.qc .ft{padding:var(--s-2) var(--s-3);display:flex;flex-direction:column;gap:6px}
.qc .ft .qm-hints{gap:var(--s-3);font-size:11px;justify-content:space-between}
.qc .ft .qm-keys{justify-content:flex-start;gap:var(--s-4)}
.qc .ft .qm-hints .kbd{min-width:18px;height:18px;padding:0 4px;font-size:10px}
"""
def qc_row(key, icon, name, val="", cls=""):
    return f'<div class="r {cls}">{qm_k(key)}<span class="zi">{ic(icon)}</span><span class="nm">{name}</span>{val}</div>'
def onoff(on): return f'<span class="v {"on-t" if on else "off-t"}">{"ON" if on else "OFF"}</span>'
def qc_pick(t): return f'<span class="v"><span class="ar">&lsaquo;</span> {t} <span class="ar">&rsaquo;</span></span>'
qc_vol = '<span class="vb">' + "".join(f'<i class="{"f" if j < 7 else ""}"></i>' for j in range(10)) + '</span><span class="v" style="width:30px;text-align:right">70%</span>'
qc_act = "".join(qc_row(k, i, n, "" if s is None else onoff(s), "hi" if n == "Read zones now" else "") for i, n, k, s in QM_ACTIONS)
qc_set = (qc_row("A", "keyboard", "Mode", qc_pick("AUTO")) + qc_row("V", "audio", "Voice", qc_pick("Adam"))
          + qc_row("[ ]", "gauge", "Speed", qc_pick("1.1&times;")) + qc_row("&minus; +", "volume", "Volume", qc_vol))
qc_zon = "".join(qc_row(key, "k-" + k, n, onoff(on), "" if on else "off") for k, n, key, on in QM_ZONES)
qc_app = "".join(qc_row(k, i, n, '<span class="hold">HOLD</span>' if n == "Quit" else "", "q" if n == "Quit" else "") for i, n, k in QM_APP)
html = head("Quick menu C", cssQC) + qm_frame() + f'''<div class="qc">
<div class="hd">{logo(18)}<span class="gn">{QM_GAME}</span><span class="qm-live"><span class="dot"></span>Reading</span></div>
<div class="grp">{qc_act}</div>
<div class="grp">{qc_set}</div>
<div class="grp">{qc_zon}</div>
<div class="grp">{qc_app}</div>
<div class="ft">{qm_hints()}{qm_keys()}</div>
</div>
'''
write("15-quick-menu-c.html", html)
print("round 7 quick menu ok")
# ===== END 15-quick-menu =====

# ======================================================================
# ===== BEGIN round 8 (Max): 04-read-key-d two keys + centered quick menus =====
# Runs after everything above and overwrites 04-read-key-d.html and
# 15-quick-menu-a/b/c.html. Earlier code is left as it was.
# ======================================================================
import math
P.update({
 "r8-menu": '<rect x="3" y="4" width="18" height="16" rx="2"/><path d="M7 9h10M7 12h10M7 15h6"/>',
 "r8-mute": '<path d="M11 5 6 9H3v6h3l5 4z"/><path d="m22 9-6 6M16 9l6 6"/>',
 "r8-open": '<path d="M15 3h6v6"/><path d="M10 14 21 3"/><path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"/>',
 "r8-mode": '<path d="M4 7h10M18 7h2M4 17h2M10 17h10"/><circle cx="16" cy="7" r="2"/><circle cx="8" cy="17" r="2"/>',
 "r8-play": '<path d="M8 5.5v13l10-6.5z"/>',
 "r8-pause": '<path d="M9 5v14M15 5v14"/>',
})

# ---------- 04-read-key-d: only the read key and the quick menu key ----------
html = head("Read key D", css04D) + obbar_l(3, "Keys") + f'''<main class="page"><div class="ctr">
<h1>Your keys</h1>
<div class="kgrid">
{keycard("Read zones now", "Reads all your zones out loud", "Mouse 4", "volume", "mouse", main=True)}
{keycard("Quick menu", "Opens the in-game menu", "F7", "r8-menu")}
</div>
</div></main>
''' + obfoot()
write("04-read-key-d.html", html)

# ---------- shared bits for the three centered quick menus ----------
R8_GAME = "Red Dead Redemption 2"
def r8_frame(): return f'<div class="q8-frame">{SCENE}</div>'   # game only, no status pill while the menu is open
def r8_k(t): return f'<span class="q8k">{t}</span>'
def r8_status(): return '<span class="q8-st"><span class="q8-dot"></span>Reading</span>'
def r8_hints(h): return '<div class="q8-hints">' + "".join(f'<span>{r8_k(k)}{d}</span>' for k, d in h) + '</div>'
R8_SLIDER = '<div class="slider"><div class="fill" style="width:70%"></div><span class="knob" style="left:70%"></span></div>'
R8_BASE = """
.q8-frame{position:absolute;inset:0}
.q8-frame svg{width:100%;height:100%;display:block}
.q8{position:absolute;left:50%;top:50%;background:#0A0D14;border:1px solid var(--line-2);box-shadow:var(--shadow-pop)}
.q8k{display:inline-flex;align-items:center;justify-content:center;min-width:20px;height:20px;padding:0 5px;border-radius:5px;background:var(--surface-2);border:1px solid var(--line-2);font:500 11px/1 var(--font);color:var(--text-2)}
.q8-st{display:inline-flex;align-items:center;gap:6px;font-size:12px;font-weight:500;color:var(--text-2)}
.q8-dot{width:6px;height:6px;border-radius:50%;background:var(--accent)}
.q8-hints{display:flex;align-items:center;gap:var(--s-4);font-size:12px;font-weight:500;color:var(--text-3)}
.q8-hints > span{display:inline-flex;align-items:center;gap:6px;white-space:nowrap}
.q8 .slider .knob{width:12px;height:12px;margin:-6px 0 0 -6px}
"""

# ---------- A: command panel (Raycast style) ----------
cssA8 = R8_BASE + """
.qa8{width:400px;margin:-226px 0 0 -200px;border-radius:14px;overflow:hidden}
.qa8 .hd{display:flex;align-items:center;gap:10px;height:48px;padding:0 var(--s-4)}
.qa8 .hd .gn{flex:1;font-size:14px;font-weight:600;letter-spacing:-0.005em}
.qa8 .ln{height:1px;background:var(--line)}
.qa8 .grp{padding:6px}
.qa8 .r{position:relative;display:flex;align-items:center;gap:12px;height:36px;padding:0 10px;border-radius:8px;font-size:13px;font-weight:500;color:var(--text)}
.qa8 .r > .ic{color:var(--text-3)}
.qa8 .r .nm{flex:1}
.qa8 .r.hi{background:var(--surface-3)}
.qa8 .r.hi > .ic{color:var(--text)}
.qa8 .r.hi::before{content:"";position:absolute;left:0;top:9px;bottom:9px;width:2px;border-radius:1px;background:var(--accent)}
.qa8 .r .v{display:inline-flex;align-items:center;gap:4px;font-size:13px;color:var(--text-2)}
.qa8 .r .v .ic{width:14px;height:14px;color:var(--text-3)}
.qa8 .seg{padding:2px}
.qa8 .seg > span{height:22px;padding:0 10px;font-size:12px}
.qa8 .seg > span.on{background:var(--accent);color:#FFFFFF;box-shadow:none}
.qa8 .slider{width:120px}
.qa8 .pct{width:30px;text-align:right;font-size:12px;color:var(--text-2);font-variant-numeric:tabular-nums}
.qa8 .ft{display:flex;align-items:center;gap:var(--s-2);height:36px;padding:0 var(--s-4);border-top:1px solid var(--line);background:#080B11}
.qa8 .ft .nm{flex:1;font-size:12px;font-weight:500;color:var(--text-3)}
"""
def a8_row(icon, name, right="", cls="", style=""):
    return f'<div class="r {cls}">{ic(icon, style)}<span class="nm">{name}</span>{right}</div>'
FILL8 = "fill:currentColor;stroke:none"
a8_act = (a8_row("r8-play", "Read now", r8_k("R"), "hi", "stroke-width:1.5")
          + a8_row("stop", "Stop", r8_k("S"))
          + a8_row("r8-mute", "Mute", r8_k("M"))
          + a8_row("r8-pause", "Pause", r8_k("P")))
a8_set = (a8_row("r8-mode", "Mode", '<div class="seg"><span class="on">Automatic</span><span>On key press</span></div>')
          + a8_row("audio", "Voice", f'<span class="v">Adam{ic("chev-down")}</span>')
          + a8_row("volume", "Volume", R8_SLIDER + '<span class="pct">70%</span>'))
a8_app = a8_row("r8-open", "Open DialogReader") + a8_row("settings", "Settings")
html = head("Quick menu A", cssA8) + r8_frame() + f'''<div class="q8 qa8">
<div class="hd"><span class="gn">{R8_GAME}</span>{r8_status()}</div>
<div class="ln"></div>
<div class="grp">{a8_act}</div>
<div class="ln"></div>
<div class="grp">{a8_set}</div>
<div class="ln"></div>
<div class="grp">{a8_app}</div>
<div class="ft">{logo(14)}<span class="nm">DialogReader</span>{r8_hints([("&uarr;&darr;", "Move"), ("Esc", "Close")])}</div>
</div>
'''
write("15-quick-menu-a.html", html)

# ---------- B: radial wheel, 9 equal segments, small hub ----------
W8 = 380; C8 = W8 / 2; RO = 182; RI = 84; RH = 76
B8_ITEMS = [("r8-play", "Read now", ""), ("stop", "Stop", ""), ("r8-mute", "Mute", ""), ("r8-pause", "Pause", ""),
            ("r8-mode", "Mode", "Automatic"), ("audio", "Voice", "Adam"), ("volume", "Volume", "70%"),
            ("r8-open", "DialogReader", ""), ("settings", "Settings", "")]
def pt(r, a):
    t = math.radians(a)
    return f"{C8 + r * math.sin(t):.2f} {C8 - r * math.cos(t):.2f}"
def sector(a0, a1, ro, ri):
    return (f"M{pt(ro, a0)} A{ro} {ro} 0 0 1 {pt(ro, a1)} L{pt(ri, a1)} A{ri} {ri} 0 0 0 {pt(ri, a0)} Z")
n8 = len(B8_ITEMS); step = 360 / n8
svg = [f'<svg class="wh" width="{W8}" height="{W8}" viewBox="0 0 {W8} {W8}" aria-hidden="true">',
       f'<circle cx="{C8}" cy="{C8}" r="{RO}" fill="#0A0D14" stroke="#283143" stroke-width="1"/>']
a0 = -step / 2
svg.append(f'<path d="{sector(a0 + 0.6, a0 + step - 0.6, RO - 1.5, RI + 0.5)}" fill="#1A2232"/>')
svg.append(f'<path d="M{pt(RO - 3, a0 + 3)} A{RO - 3} {RO - 3} 0 0 1 {pt(RO - 3, a0 + step - 3)}" fill="none" stroke="#5B7CFF" stroke-width="3" stroke-linecap="round"/>')
for j in range(n8):
    a = -step / 2 + j * step
    svg.append(f'<path d="M{pt(RI, a)} L{pt(RO, a)}" stroke="#1C2331" stroke-width="1"/>')
svg.append(f'<circle cx="{C8}" cy="{C8}" r="{RI}" fill="none" stroke="#1C2331" stroke-width="1"/>')
svg.append(f'<circle cx="{C8}" cy="{C8}" r="{RH}" fill="#0C1018" stroke="#283143" stroke-width="1"/>')
svg.append('</svg>')
labs = []
RM = (RO + RI) / 2 + 2
for j, (i, n, v) in enumerate(B8_ITEMS):
    a = math.radians(j * step)
    x = C8 + RM * math.sin(a); y = C8 - RM * math.cos(a)
    cls = "sg hi" if j == 0 else "sg"
    vv = f'<span class="vl">{v}</span>' if v else ""
    labs.append(f'<div class="{cls}" style="left:{x:.1f}px;top:{y:.1f}px">{ic(i)}<span class="lb">{n}</span>{vv}</div>')
cssB8 = R8_BASE + """
.qb8{position:absolute;left:50%;top:50%;width:380px;height:380px;margin:-214px 0 0 -190px}
.qb8 .wh{position:absolute;inset:0;display:block}
.qb8 .sg{position:absolute;width:96px;height:64px;margin:-32px 0 0 -48px;display:flex;flex-direction:column;align-items:center;justify-content:center;gap:5px;text-align:center}
.qb8 .sg .ic{width:18px;height:18px;color:var(--text-3)}
.qb8 .sg .lb{font-size:12px;font-weight:500;color:var(--text);line-height:1}
.qb8 .sg .vl{font-size:11px;color:var(--text-3);line-height:1;margin-top:-1px}
.qb8 .sg.hi .ic{color:var(--text)}
.qb8 .hub{position:absolute;left:50%;top:50%;width:140px;margin:-34px 0 0 -70px;display:flex;flex-direction:column;align-items:center;gap:6px;text-align:center}
.qb8 .hub .t{font-size:17px;font-weight:600;letter-spacing:-0.01em;line-height:1.1}
.qb8 .hub .g{font-size:11px;color:var(--text-3);line-height:1.2}
.qb8 .hub .q8-st{font-size:11px}
.qb8 .bar8{position:absolute;left:50%;top:400px;width:300px;margin-left:-150px;height:32px;border-radius:16px;background:#0A0D14;border:1px solid var(--line-2);display:flex;justify-content:center}
.qb8 .bar8 .q8-hints{gap:var(--s-4)}
.qb8 .bar8 .q8k{height:18px;min-width:18px;font-size:10px;padding:0 4px}
"""
html = head("Quick menu B", cssB8) + r8_frame() + f'''<div class="qb8">
{"".join(svg)}
{"".join(labs)}
<div class="hub">{r8_status()}<div class="t">Read now</div><div class="g">{R8_GAME}</div></div>
<div class="bar8">{r8_hints([("&larr;&rarr;", "Move"), ("Enter", "Select"), ("Esc", "Close")])}</div>
</div>
'''
write("15-quick-menu-b.html", html)

# ---------- C: icon dock, label only under the focused button ----------
cssC8 = R8_BASE + """
.qc8{width:560px;margin:-120px 0 0 -280px;border-radius:16px;padding:var(--s-4) var(--s-5) 0}
.qc8 .hd{display:flex;align-items:center;gap:10px;height:20px}
.qc8 .hd .gn{flex:1;font-size:14px;font-weight:600;letter-spacing:-0.005em}
.qc8 .dock{display:flex;align-items:flex-start;justify-content:center;gap:14px;margin-top:22px}
.qc8 .b{width:56px;display:flex;flex-direction:column;align-items:center;gap:8px}
.qc8 .b .o{width:52px;height:52px;border-radius:50%;border:1px solid var(--line-2);display:flex;align-items:center;justify-content:center;color:var(--text-2)}
.qc8 .b .o .ic{width:20px;height:20px}
.qc8 .b .l{height:14px;font-size:12px;font-weight:500;line-height:14px;white-space:nowrap;color:var(--text)}
.qc8 .b.hi .o{border:2px solid var(--accent);background:var(--surface-2);color:var(--text)}
.qc8 .vs{width:1px;height:28px;background:var(--line-2);margin:12px 4px 0}
.qc8 .ln{height:1px;background:var(--line);margin:18px calc(-1 * var(--s-5)) 0}
.qc8 .set{display:flex;align-items:center;justify-content:space-between;height:52px}
.qc8 .set .k{font-size:12px;font-weight:500;color:var(--text-3)}
.qc8 .set .c{display:flex;align-items:center;gap:var(--s-2)}
.qc8 .set .sep{width:1px;height:16px;background:var(--line-2)}
.qc8 .seg{padding:2px}
.qc8 .seg > span{height:22px;padding:0 9px;font-size:12px}
.qc8 .seg > span.on{background:var(--accent);color:#FFFFFF;box-shadow:none}
.qc8 .sel8{display:inline-flex;align-items:center;gap:4px;height:26px;padding:0 6px 0 10px;border-radius:var(--r-md);border:1px solid var(--line-2);font-size:12px;font-weight:500}
.qc8 .sel8 .ic{width:14px;height:14px;color:var(--text-3)}
.qc8 .slider{width:72px}
.qc8 .pct{font-size:12px;color:var(--text-2);font-variant-numeric:tabular-nums}
.qc8 .ft{display:flex;justify-content:center;height:36px;align-items:center;border-top:1px solid var(--line);margin:0 calc(-1 * var(--s-5));background:#080B11;border-radius:0 0 16px 16px}
.qc8 .ft .q8k{height:18px;min-width:18px;font-size:10px;padding:0 4px}
"""
def c8_btn(icon, label, hi=False):
    return f'<div class="b{" hi" if hi else ""}"><span class="o">{ic(icon)}</span><span class="l">{label if hi else ""}</span></div>'
c8_dock = (c8_btn("r8-play", "Read now", True) + c8_btn("stop", "Stop") + c8_btn("r8-mute", "Mute") + c8_btn("r8-pause", "Pause")
           + '<span class="vs"></span>' + c8_btn("r8-open", "Open DialogReader") + c8_btn("settings", "Settings"))
c8_set = (f'<div class="c"><span class="k">Mode</span><div class="seg"><span class="on">Automatic</span><span>On key press</span></div></div>'
          f'<span class="sep"></span><div class="c"><span class="k">Voice</span><span class="sel8">Adam{ic("chev-down")}</span></div>'
          f'<span class="sep"></span><div class="c">{ic("volume", "width:14px;height:14px;color:var(--text-3)")}{R8_SLIDER}<span class="pct">70%</span></div>')
html = head("Quick menu C", cssC8) + r8_frame() + f'''<div class="q8 qc8">
<div class="hd"><span class="gn">{R8_GAME}</span>{r8_status()}</div>
<div class="dock">{c8_dock}</div>
<div class="ln"></div>
<div class="set">{c8_set}</div>
<div class="ft">{r8_hints([("&larr;&rarr;", "Move"), ("Enter", "Select"), ("Esc", "Close")])}</div>
</div>
'''
write("15-quick-menu-c.html", html)
print("round 8 ok")
# ===== END round 8 =====

# ======================================================================
# ===== BEGIN round 9 (review fixes): 04-read-key-d + quick menus A/B/C =====
# Runs after round 8 and overwrites the same four pages again.
# Larger type, one shared hint pill under every menu, no brand footer.
# ======================================================================
P.update({
 "r9-panel": '<rect x="3" y="3" width="18" height="18" rx="3"/><path d="M3 8.5h18"/><path d="M7 12.5h10M7 16h6"/>',
})

# ---------- 04-read-key-d: panel icon, clearer text, keycaps line up ----------
css04D9 = css04D + """
.kc .cap{min-width:132px}
"""
html = head("Read key D", css04D9) + obbar_l(3, "Keys") + f'''<main class="page"><div class="ctr">
<h1>Your keys</h1>
<div class="kgrid">
{keycard("Read zones now", "Reads all your zones out loud", "Mouse 4", "volume", "mouse", main=True)}
{keycard("Quick menu", "Opens DialogReader over your game", "F7", "r9-panel")}
</div>
</div></main>
''' + obfoot()
write("04-read-key-d.html", html)

# ---------- shared bits ----------
def r9_k(t): return f'<span class="q9k{" ar" if "&" in t else ""}">{t}</span>'
def r9_pill(move):
    h = [(move, "Move"), ("Enter", "Select"), ("Esc", "Close")]
    return '<div class="q9-pill">' + "".join(f'<span>{r9_k(k)}{d}</span>' for k, d in h) + '</div>'
def r9_status(): return '<span class="q9-st"><span class="q9-dot"></span>Reading</span>'
R9_SLIDER = '<div class="slider"><div class="fill" style="width:70%"></div><span class="knob" style="left:70%"></span></div>'
R9_BASE = """
.q8-frame{position:absolute;inset:0}
.q8-frame svg{width:100%;height:100%;display:block}
.q9-stage{position:absolute;inset:0;display:flex;flex-direction:column;align-items:center;justify-content:center;gap:14px}
.q9{background:#0A0D14;border:1px solid var(--line-2);box-shadow:var(--shadow-pop)}
.q9k{display:inline-flex;align-items:center;justify-content:center;min-width:22px;height:22px;padding:0 6px;border-radius:6px;background:var(--surface-2);border:1px solid var(--line-2);font:500 12px/1 var(--font);color:var(--text-2)}
.q9k.ar{letter-spacing:2px;padding:0 5px 0 7px}
.q9-st{display:inline-flex;align-items:center;gap:7px;font-size:12px;font-weight:500;color:var(--text-2)}
.q9-dot{width:6px;height:6px;border-radius:50%;background:var(--accent)}
.q9-pill{display:flex;align-items:center;gap:18px;height:38px;padding:0 16px;border-radius:19px;background:#0A0D14;border:1px solid var(--line-2);box-shadow:var(--shadow-pop);font-size:12px;font-weight:500;color:var(--text-3)}
.q9-pill > span{display:inline-flex;align-items:center;gap:7px;white-space:nowrap}
.q9 .slider .knob{width:14px;height:14px;margin:-7px 0 0 -7px}
.q9 .seg{padding:2px}
.q9 .seg > span{height:26px;padding:0 12px;font-size:13px}
.q9 .seg > span.on{background:var(--accent);color:#FFFFFF;box-shadow:none}
"""
R9_GAME = "Red Dead Redemption 2"

# ---------- A: command panel ----------
cssA9 = R9_BASE + """
.qa9{width:440px;border-radius:14px;overflow:hidden}
.qa9 .hd{display:flex;align-items:center;gap:12px;height:54px;padding:0 18px}
.qa9 .hd .gn{flex:1;font-size:15px;font-weight:600;letter-spacing:-0.01em}
.qa9 .ln{height:1px;background:var(--line)}
.qa9 .grp{padding:6px}
.qa9 .r{position:relative;display:flex;align-items:center;gap:14px;height:42px;padding:0 12px;border-radius:8px;font-size:14px;font-weight:500;color:var(--text)}
.qa9 .r > .ic{width:16px;height:16px;color:var(--text-3)}
.qa9 .r .nm{flex:1}
.qa9 .r.hi{background:var(--surface-3)}
.qa9 .r.hi > .ic{color:var(--text)}
.qa9 .r.hi::before{content:"";position:absolute;left:0;top:11px;bottom:11px;width:2px;border-radius:1px;background:var(--accent)}
.qa9 .ctl{width:214px;display:flex;align-items:center;justify-content:flex-end;gap:12px}
.qa9 .ctl .seg{display:flex;width:100%}
.qa9 .ctl .seg > span{flex:1}
.qa9 .v{display:inline-flex;align-items:center;gap:6px;font-size:13px;color:var(--text-2)}
.qa9 .v .ic{width:14px;height:14px;color:var(--text-3)}
.qa9 .ctl .slider{flex:1}
.qa9 .pct{width:34px;text-align:right;font-size:12px;color:var(--text-2);font-variant-numeric:tabular-nums}
"""
def a9_row(icon, name, right="", cls=""):
    return f'<div class="r {cls}">{ic(icon)}<span class="nm">{name}</span>{right}</div>'
def a9_ctl(x): return f'<div class="ctl">{x}</div>'
a9_act = (a9_row("r8-play", "Read now", "", "hi") + a9_row("stop", "Stop")
          + a9_row("r8-mute", "Mute") + a9_row("r8-pause", "Pause"))
a9_set = (a9_row("r8-mode", "Mode", a9_ctl('<div class="seg"><span class="on">Automatic</span><span>On key press</span></div>'))
          + a9_row("audio", "Voice", a9_ctl(f'<span class="v">Adam{ic("chev-down")}</span>'))
          + a9_row("volume", "Volume", a9_ctl(R9_SLIDER + '<span class="pct">70%</span>')))
a9_app = a9_row("r8-open", "Open DialogReader") + a9_row("settings", "Settings")
html = head("Quick menu A", cssA9) + r8_frame() + f'''<div class="q9-stage">
<div class="q9 qa9">
<div class="hd"><span class="gn">{R9_GAME}</span>{r9_status()}</div>
<div class="ln"></div>
<div class="grp">{a9_act}</div>
<div class="ln"></div>
<div class="grp">{a9_set}</div>
<div class="ln"></div>
<div class="grp">{a9_app}</div>
</div>
{r9_pill("&uarr;&darr;")}
</div>
'''
write("15-quick-menu-a.html", html)

# ---------- B: radial wheel, 8 segments, focused segment filled ----------
W9 = 440; C9 = W9 / 2; RO9 = 216; RI9 = 98; RH9 = 90; GAP9 = 3
B9_ITEMS = [("r8-play", "Read now", ""), ("stop", "Stop", ""), ("r8-mute", "Mute", ""), ("r8-pause", "Pause", ""),
            ("r8-mode", "Mode", "Automatic"), ("audio", "Voice", "Adam &middot; 70%"),
            ("r8-open", "DialogReader", ""), ("settings", "Settings", "")]
def pt9(r, a):
    t = math.radians(a)
    return f"{C9 + r * math.sin(t):.2f} {C9 - r * math.cos(t):.2f}"
def sector9(a0, a1, ro, ri):
    go = math.degrees(GAP9 / ro); gi = math.degrees(GAP9 / ri)
    return (f"M{pt9(ro, a0 + go)} A{ro} {ro} 0 0 1 {pt9(ro, a1 - go)} "
            f"L{pt9(ri, a1 - gi)} A{ri} {ri} 0 0 0 {pt9(ri, a0 + gi)} Z")
n9 = len(B9_ITEMS); step9 = 360 / n9
svg9 = [f'<svg class="wh" width="{W9}" height="{W9}" viewBox="0 0 {W9} {W9}" aria-hidden="true">',
        f'<circle cx="{C9}" cy="{C9}" r="{RO9 + 1}" fill="#06080C" stroke="#283143" stroke-width="1"/>']
for j in range(n9):
    a0 = -step9 / 2 + j * step9
    fill = "#5B7CFF" if j == 0 else "#0E131C"
    svg9.append(f'<path d="{sector9(a0, a0 + step9, RO9 - 2, RI9)}" fill="{fill}"/>')
svg9.append(f'<circle cx="{C9}" cy="{C9}" r="{RH9}" fill="#0A0D14" stroke="#283143" stroke-width="1"/>')
svg9.append('</svg>')
RM9 = (RO9 + RI9) / 2
labs9 = []
for j, (i, n, v) in enumerate(B9_ITEMS):
    a = math.radians(j * step9)
    x = C9 + RM9 * math.sin(a); y = C9 - RM9 * math.cos(a)
    cls = "sg hi" if j == 0 else "sg"
    vv = f'<span class="vl">&lsaquo; {v} &rsaquo;</span>' if v else ""
    labs9.append(f'<div class="{cls}" style="left:{x:.1f}px;top:{y:.1f}px">{ic(i)}<span class="lb">{n}</span>{vv}</div>')
cssB9 = R9_BASE + """
.qb9{position:relative;width:440px;height:440px;flex:none}
.qb9 .wh{position:absolute;inset:0;display:block}
.qb9 .sg{position:absolute;width:120px;height:42px;margin:-21px 0 0 -60px;display:flex;flex-direction:column;align-items:center;gap:8px;text-align:center}
.qb9 .sg .ic{width:20px;height:20px;color:var(--text-2)}
.qb9 .sg .lb{font-size:14px;font-weight:500;color:var(--text);line-height:14px;white-space:nowrap}
.qb9 .sg .vl{font-size:12px;color:var(--text-3);line-height:12px;margin-top:-2px;white-space:nowrap}
.qb9 .sg.hi .ic{color:#FFFFFF}
.qb9 .sg.hi .lb{color:#FFFFFF;font-weight:600}
.qb9 .hub{position:absolute;left:50%;top:50%;width:160px;height:56px;margin:-28px 0 0 -80px;display:flex;flex-direction:column;align-items:center;justify-content:center;gap:10px;text-align:center}
.qb9 .hub .t{font-size:18px;font-weight:600;letter-spacing:-0.01em;line-height:18px}
.qb9 .hub .q9-st{font-size:12px;color:var(--text-2);line-height:14px}
"""
html = head("Quick menu B", cssB9) + r8_frame() + f'''<div class="q9-stage">
<div class="qb9">
{"".join(svg9)}
{"".join(labs9)}
<div class="hub"><div class="t">Read now</div><span class="q9-st"><span class="q9-dot"></span>{R9_GAME}</span></div>
</div>
{r9_pill("&larr;&rarr;")}
</div>
'''
write("15-quick-menu-b.html", html)

# ---------- C: icon dock, grey labels under every button ----------
cssC9 = R9_BASE + """
.qc9{width:660px;border-radius:16px;overflow:hidden}
.qc9 .hd{display:flex;align-items:center;gap:12px;height:54px;padding:0 22px}
.qc9 .hd .gn{flex:1;font-size:15px;font-weight:600;letter-spacing:-0.01em}
.qc9 .dock{display:flex;align-items:flex-start;justify-content:center;gap:6px;padding:4px 0 20px}
.qc9 .b{width:88px;display:flex;flex-direction:column;align-items:center;gap:10px}
.qc9 .b .o{width:56px;height:56px;border-radius:50%;border:1px solid var(--line-2);display:flex;align-items:center;justify-content:center;color:var(--text-2)}
.qc9 .b .o .ic{width:20px;height:20px}
.qc9 .b .l{height:16px;font-size:12px;font-weight:500;line-height:16px;white-space:nowrap;color:var(--text-3)}
.qc9 .b.hi .o{border:2px solid var(--accent);background:var(--surface-2);color:var(--text)}
.qc9 .b.hi .l{color:var(--text)}
.qc9 .vs{width:1px;height:32px;background:var(--line-2);margin:12px 10px 0}
.qc9 .ln{height:1px;background:var(--line)}
.qc9 .set{display:flex;align-items:center;justify-content:space-between;height:60px;padding:0 22px}
.qc9 .set .k{font-size:12px;font-weight:500;color:var(--text-3)}
.qc9 .set .c{display:flex;align-items:center;gap:10px}
.qc9 .set .sep{width:1px;height:18px;background:var(--line-2)}
.qc9 .sel9{display:inline-flex;align-items:center;gap:6px;height:30px;padding:0 8px 0 12px;border-radius:var(--r-md);border:1px solid var(--line-2);font-size:13px;font-weight:500}
.qc9 .sel9 .ic{width:14px;height:14px;color:var(--text-3)}
.qc9 .slider{width:84px}
.qc9 .pct{font-size:12px;color:var(--text-2);font-variant-numeric:tabular-nums}
"""
def c9_btn(icon, label, hi=False):
    return f'<div class="b{" hi" if hi else ""}"><span class="o">{ic(icon)}</span><span class="l">{label}</span></div>'
c9_dock = (c9_btn("r8-play", "Read now", True) + c9_btn("stop", "Stop") + c9_btn("r8-mute", "Mute") + c9_btn("r8-pause", "Pause")
           + '<span class="vs"></span>' + c9_btn("r8-open", "DialogReader") + c9_btn("settings", "Settings"))
c9_set = (f'<div class="c"><span class="k">Mode</span><div class="seg"><span class="on">Automatic</span><span>On key press</span></div></div>'
          f'<span class="sep"></span><div class="c"><span class="k">Voice</span><span class="sel9">Adam{ic("chev-down")}</span></div>'
          f'<span class="sep"></span><div class="c"><span class="k">Volume</span>{R9_SLIDER}<span class="pct">70%</span></div>')
html = head("Quick menu C", cssC9) + r8_frame() + f'''<div class="q9-stage">
<div class="q9 qc9">
<div class="hd"><span class="gn">{R9_GAME}</span>{r9_status()}</div>
<div class="dock">{c9_dock}</div>
<div class="ln"></div>
<div class="set">{c9_set}</div>
</div>
{r9_pill("&uarr;&darr;&larr;&rarr;")}
</div>
'''
write("15-quick-menu-c.html", html)
print("round 9 ok")
# ===== END round 9 =====

# ======================================================================
# ===== BEGIN round 10 (Max): aligned key cards, new quick menu A, voice list, zone edit =====
# Runs after round 9. Overwrites 04-read-key-d.html and 15-quick-menu-a.html,
# adds 15-quick-menu-voice.html and 16-zone-edit.html, and removes the
# 15-quick-menu-b/c pages (Max picked A). Earlier code is left as it was.
# ======================================================================
P.update({
 "r10-power": '<path d="M12 3v9"/><path d="M18.36 6.64a9 9 0 1 1-12.73 0"/>',
 "r10-speed": '<path d="M3.5 18a9 9 0 1 1 17 0"/><path d="m12 14 4-5"/>',
 "r10-chev-left": '<path d="m15 18-6-6 6-6"/>',
 "r10-cross": '<path d="M12 3v18M3 12h18"/>',
})

# ---------- 04-read-key-d: both cards share one grid of columns ----------
# Same left padding, icon tile, title/description start, keycap box and Change button.
# The main card differs only in colour (accent border, tint, accent icon tile).
css04D10 = css04D + """
.kgrid{width:720px}
.kc,.kc.main{height:92px;gap:20px;padding:0 var(--s-3) 0 var(--s-5)}
.kc .itile,.kc.main .itile{width:44px;height:44px;border-radius:10px}
.kc .itile .ic,.kc.main .itile .ic{width:22px;height:22px}
.kc .nm,.kc.main .nm{font-size:18px;font-weight:600;letter-spacing:-0.01em;line-height:24px}
.kc .ds,.kc.main .ds{font-size:14px;line-height:20px;margin-top:2px;color:var(--text-3)}
.kc.main .ds{color:var(--text-2)}
.kc .cap,.kc.main .cap{box-sizing:border-box;width:140px;min-width:0;height:44px;padding:0;font-size:16px;border-bottom-width:4px}
.kc .cap .ic,.kc.main .cap .ic{width:18px;height:18px}
.kc .btn-ghost{width:84px}
"""
html = head("Read key D", css04D10) + obbar_l(3, "Keys") + f'''<main class="page"><div class="ctr">
<h1>Your keys</h1>
<div class="kgrid">
{keycard("Read zones now", "Reads all your zones out loud", "Mouse 4", "volume", "mouse", main=True)}
{keycard("Quick menu", "Opens DialogReader over your game", "F7", "r9-panel")}
</div>
</div></main>
''' + obfoot()
write("04-read-key-d.html", html)

# ---------- shared quick menu A (round 9 look, new items) ----------
cssA10 = cssA9 + """
.qa9 .r .nm.q{color:var(--text-3)}
.qa9 .r .ch{width:16px;height:16px;color:var(--text-3)}
.qa9 .v{gap:4px}
.qa9 .stp{display:flex;align-items:center;gap:2px}
.qa9 .stp .ic{width:14px;height:14px;color:var(--text-3)}
.qa9 .stp .vv{width:44px;text-align:center;font-size:13px;font-weight:500;color:var(--text);font-variant-numeric:tabular-nums}
.qa9 .toggle{width:34px;height:20px}
.qa9 .hk{font-size:12px;color:var(--text-3)}
"""
def a10_val(t): return a9_ctl(f'<span class="v">{t}{ic("chev-right")}</span>')
A10_STEP = a9_ctl(f'<span class="stp">{ic("r10-chev-left")}<span class="vv">1.1x</span>{ic("chev-right")}</span>')
A10_MODE = a9_ctl('<div class="seg"><span class="on">Automatic</span><span>On key press</span></div>')
A10_VOL = a9_ctl(R9_SLIDER + '<span class="pct">70%</span>')
def a10_menu(hi="zones", voice_list=""):
    g1 = a9_row("zone", "Manage zones", ic("chev-right"), "hi" if hi == "zones" else "")
    g2 = (a9_row("stop", "Stop")
          + a9_row("r8-mute", "Mute", '<span class="toggle"></span>'))
    g3 = (a9_row("audio", "Voice", a10_val("Adam") if not voice_list else a9_ctl(f'<span class="v">Adam{ic("chev-down")}</span>'), "hi" if hi == "voice" else "")
          + voice_list
          + a9_row("r10-speed", "Speed", A10_STEP)
          + a9_row("volume", "Volume", A10_VOL)
          + a9_row("r8-mode", "Mode", A10_MODE))
    g4 = a9_row("settings", "Settings", ic("r8-open")) + a9_row("r10-power", "Quit", "", "quit")
    return f'''<div class="q9 qa9">
<div class="hd"><span class="gn">{R9_GAME}</span>{r9_status()}</div>
<div class="ln"></div>
<div class="grp">{g1}</div>
<div class="ln"></div>
<div class="grp">{g2}</div>
<div class="ln"></div>
<div class="grp">{g3}</div>
<div class="ln"></div>
<div class="grp">{g4}</div>
</div>'''
cssA10 += """
.qa9 .r > svg.ic:last-child:not(:first-child){width:14px;height:14px;color:var(--text-3)}
.qa9 .r.quit{color:var(--text-3)}
.qa9 .r.quit > .ic{color:var(--text-3)}
"""
def a10_pill(items):
    return '<div class="q9-pill">' + "".join(f'<span>{r9_k(k)}{d}</span>' for k, d in items) + '</div>'
html = head("Quick menu A", cssA10) + r8_frame() + f'''<div class="q9-stage">
{a10_menu()}
{a10_pill([("&uarr;&darr;", "Move"), ("&larr;&rarr;", "Change"), ("Enter", "Select"), ("Esc", "Close")])}
</div>
'''
write("15-quick-menu-a.html", html)

# ---------- 15-quick-menu-voice: Voice opened in place, list under the row ----------
V10 = [("Adam", "US · Male", True), ("Bella", "US · Female", False), ("Michael", "US · Male", False),
       ("Sarah", "US · Female", False), ("Emma", "UK · Female", False)]
cssV10 = cssA10 + """
.qa9 .vl{margin:0 0 4px 30px;padding:2px 0;border-left:1px solid var(--line-2)}
.qa9 .vr{position:relative;display:flex;align-items:center;gap:10px;height:34px;margin-left:10px;padding:0 6px 0 10px;border-radius:8px;font-size:14px;font-weight:500;color:var(--text-2)}
.qa9 .vr .ck{width:16px;height:16px;color:var(--accent);stroke-width:2}
.qa9 .vr .ck.no{visibility:hidden}
.qa9 .vr .nm{flex:1}
.qa9 .vr .ds{font-size:12px;color:var(--text-3)}
.qa9 .vr .pb{width:26px;height:26px;border-radius:50%;display:flex;align-items:center;justify-content:center;color:var(--text-3)}
.qa9 .vr .pb .ic{width:12px;height:12px;fill:currentColor;stroke:none}
.qa9 .vr.sel{color:var(--text)}
.qa9 .vr.hi{background:var(--surface-3);color:var(--text)}
.qa9 .vr.hi::before{content:"";position:absolute;left:0;top:8px;bottom:8px;width:2px;border-radius:1px;background:var(--accent)}
.qa9 .vr.hi .pb{background:var(--surface-2);border:1px solid var(--line-2);color:var(--text)}
"""
def v10_row(n, d, sel, hi):
    cls = "vr" + (" sel" if sel else "") + (" hi" if hi else "")
    ck = ic("check").replace('class="ic"', 'class="ic ck"' if sel else 'class="ic ck no"')
    return f'<div class="{cls}">{ck}<span class="nm">{n}</span><span class="ds">{d}</span><span class="pb">{ic("r8-play")}</span></div>'
vlist = '<div class="vl">' + "".join(v10_row(n, d, s, n == "Bella") for n, d, s in V10) + '</div>'
html = head("Quick menu voice", cssV10) + r8_frame() + f'''<div class="q9-stage">
{a10_menu(hi="", voice_list=vlist)}
{a10_pill([("&uarr;&darr;", "Move"), ("Space", "Preview"), ("Enter", "Choose"), ("Esc", "Back")])}
</div>
'''
write("15-quick-menu-voice.html", html)

# ---------- 16-zone-edit: in-game zone edit mode (from Manage zones) ----------
# Frame is 1440x900 over the 1600x900 scene (slice): x shifts by -80, y is 1:1.
ZX = -80
SEL10 = "Subtitles"
NEW10 = (930, 150, 1290, 262)   # new zone being drawn, frame px
css16 = """
.frame{position:absolute;inset:0}
.frame svg{width:100%;height:100%;display:block}
.dim{position:absolute;inset:0;background:rgba(5,7,11,0.30)}
.ez{position:absolute;border:1px solid rgba(237,240,246,0.70);border-radius:4px;background:rgba(237,240,246,0.04)}
.ez .bd{position:absolute;left:-1px;bottom:100%;margin-bottom:6px;width:24px;height:24px;border-radius:6px;background:#0A0D14;border:1px solid var(--line-2);display:flex;align-items:center;justify-content:center;color:var(--text-3)}
.ez .bd .ic{width:14px;height:14px}
.ez.sel{border:1.5px solid var(--accent);background:rgba(91,124,255,0.10)}
.ez.sel .bd{left:-1.5px;color:var(--accent);border-color:rgba(91,124,255,0.55)}
.ez .hd{position:absolute;width:8px;height:8px;margin:-4px 0 0 -4px;border-radius:2px;background:#FFFFFF;border:1.5px solid var(--accent);box-sizing:border-box}
.ez .del{position:absolute;right:-13px;top:-13px;width:26px;height:26px;border-radius:50%;background:#0A0D14;border:1px solid var(--line-2);box-shadow:var(--shadow-pop);display:flex;align-items:center;justify-content:center;color:var(--text)}
.ez .del .ic{width:14px;height:14px;stroke-width:2}
.nz{position:absolute;border:1.5px solid rgba(91,124,255,0.55);border-radius:4px;background:rgba(91,124,255,0.08)}
.nz .cur{position:absolute;right:-11px;bottom:-11px;width:22px;height:22px;color:#FFFFFF}
.nz .cur .ic{width:22px;height:22px;stroke-width:1.5}
.nz .sz{position:absolute;left:100%;top:100%;margin:10px 0 0 6px;height:22px;padding:0 8px;border-radius:6px;background:#0A0D14;border:1px solid var(--line-2);display:flex;align-items:center;font-size:12px;font-weight:500;color:var(--text-2);font-variant-numeric:tabular-nums;white-space:nowrap}
.kp{position:absolute;display:flex;gap:2px;padding:4px;border-radius:10px;background:#0A0D14;border:1px solid var(--line-2);box-shadow:var(--shadow-pop)}
.kp span{width:32px;height:32px;border-radius:7px;display:flex;align-items:center;justify-content:center;color:var(--text-3)}
.kp span .ic{width:16px;height:16px}
.kp span.hi{background:var(--surface-3);color:var(--text)}
.kp .tip{position:absolute;top:100%;margin-top:6px;height:22px;padding:0 8px;border-radius:6px;background:var(--surface-3);display:flex;align-items:center;font-size:12px;font-weight:500;color:var(--text);white-space:nowrap}
.tb{position:absolute;left:50%;top:24px;transform:translateX(-50%);display:flex;align-items:center;gap:16px;height:44px;padding:0 6px 0 16px;border-radius:22px;background:#0A0D14;border:1px solid var(--line-2);box-shadow:var(--shadow-pop);font-size:12px;font-weight:500;color:var(--text-3);white-space:nowrap}
.tb .t{display:inline-flex;align-items:center;gap:8px;font-size:13px;font-weight:600;color:var(--text)}
.tb .t .ic{width:16px;height:16px;color:var(--text-2)}
.tb .vs{width:1px;height:18px;background:var(--line-2)}
.tb .h{display:inline-flex;align-items:center;gap:5px}
.tb .h b{font-weight:600;color:var(--text-2)}
.tb .dt{width:3px;height:3px;border-radius:50%;background:var(--line-2)}
.tb .done{height:32px;padding:0 16px;border-radius:16px;background:var(--accent);color:#FFFFFF;font-size:13px;font-weight:600;display:flex;align-items:center;margin-left:2px}
"""
ez = []
for k, n, x1, y1, x2, y2 in ZONES:
    x1 += ZX; x2 += ZX
    sel = n == SEL10
    inner = f'<span class="bd">{ic("k-" + k)}</span>'
    if sel:
        for hx, hy in [(0,0),(50,0),(100,0),(0,100),(50,100),(100,100),(0,50),(100,50)]:
            inner += f'<span class="hd" style="left:{hx}%;top:{hy}%"></span>'
        inner += f'<span class="del">{ic("x")}</span>'
    ez.append(f'<div class="ez{" sel" if sel else ""}" style="left:{x1}px;top:{y1}px;width:{x2-x1}px;height:{y2-y1}px">{inner}</div>')
nx1, ny1, nx2, ny2 = NEW10
nz = (f'<div class="nz" style="left:{nx1}px;top:{ny1}px;width:{nx2-nx1}px;height:{ny2-ny1}px">'
      f'<span class="cur">{ic("r10-cross")}</span><span class="sz">{nx2-nx1} &times; {ny2-ny1}</span></div>')
kinds = ["dialogue", "speaker", "pickups", "prompts", "ignore"]
kp = (f'<div class="kp" style="left:{nx1}px;top:{ny2 + 10}px">'
      + "".join(f'<span class="{"hi" if k == "dialogue" else ""}" title="{KIND[k]}">{ic("k-" + k)}</span>' for k in kinds)
      + f'<span class="tip" style="left:4px;width:auto">{KIND["dialogue"]}</span></div>')
tb = (f'<div class="tb"><span class="t">{ic("zone")}Zones</span><span class="vs"></span>'
      + '<span class="dt"></span>'.join(f'<span class="h"><b>{a}</b>to {b}</span>' for a, b in
          [("Drag", "add"), ("Click", "select"), ("Delete", "remove"), ("Esc", "finish")])
      + '<span class="done">Done</span></div>')
html = head("Zone edit", R9_BASE + css16) + f'''<div class="frame">{SCENE}</div>
<div class="dim"></div>
{"".join(ez)}
{nz}
{kp}
{tb}
'''
write("16-zone-edit.html", html)

# Max picked A: the B and C quick menus are gone.
for _n in ("15-quick-menu-b.html", "15-quick-menu-c.html"):
    if os.path.exists(os.path.join(OUT, _n)): os.remove(os.path.join(OUT, _n))
print("round 10 ok")
# ===== END round 10 =====

# ======================================================================
# ===== BEGIN round 11 (review fixes): quick menu A states, voice flyout, zone edit polish =====
# Runs after round 10. Overwrites 04-read-key-d.html, 15-quick-menu-a.html,
# 15-quick-menu-voice.html and 16-zone-edit.html. Earlier code is left as it was.
# ======================================================================
P.update({
 "r11-spk": '<path d="M11 5 6 9H3v6h3l5 4z"/><path d="M15.5 9.5a3.5 3.5 0 0 1 0 5"/>',
})

# ---------- 04-read-key-d: quick menu description matches the new menu ----------
html = head("Read key D", css04D10) + obbar_l(3, "Keys") + f'''<main class="page"><div class="ctr">
<h1>Your keys</h1>
<div class="kgrid">
{keycard("Read zones now", "Reads all your zones out loud", "Mouse 4", "volume", "mouse", main=True)}
{keycard("Quick menu", "Opens the quick menu over your game", "F7", "r9-panel")}
</div>
</div></main>
''' + obfoot()
write("04-read-key-d.html", html)

# ---------- shared quick menu A (round 11) ----------
cssA11 = cssA10 + """
.q11w{position:relative}
.qa9 .r .tx{font-size:12px;color:var(--text-3)}
.qa9 .r.quit .tx{color:var(--text-3)}
.q11s{position:absolute;left:calc(100% + 28px);top:0;width:260px;display:flex;flex-direction:column;gap:10px}
.q11s .lb{font-size:11px;font-weight:600;letter-spacing:0.06em;text-transform:uppercase;color:var(--text-3);padding-left:2px}
.q11s .q9{border-radius:12px;overflow:hidden}
.q11s .hd{display:flex;align-items:center;gap:10px;height:40px;padding:0 14px;border-bottom:1px solid var(--line)}
.q11s .hd .gn{flex:1;font-size:13px;font-weight:600;color:var(--text-2)}
.q11s .q9-dot.off{background:var(--text-3)}
.q11s .grp{padding:5px}
.q11s .r{position:relative;display:flex;align-items:center;gap:12px;height:38px;padding:0 10px;border-radius:7px;font-size:13px;font-weight:500;color:var(--text)}
.q11s .r > .ic{width:15px;height:15px;color:var(--text)}
.q11s .r .nm{flex:1}
.q11s .r.hi{background:var(--surface-3)}
.q11s .r.hi::before{content:"";position:absolute;left:0;top:10px;bottom:10px;width:2px;border-radius:1px;background:var(--accent)}
.q11s .r.arm{background:rgba(248,113,113,0.10);color:var(--danger)}
.q11s .r.arm::before{content:"";position:absolute;left:0;top:10px;bottom:10px;width:2px;border-radius:1px;background:var(--danger)}
.q11s .r.arm > .ic{color:var(--danger)}
.q11s .r .tx{font-size:12px;color:var(--text-3)}
.q11s .r.arm .tx{color:rgba(248,113,113,0.80)}
"""
def a11_status(on=True):
    return ('<span class="q9-st"><span class="q9-dot"></span>Reading</span>' if on
            else '<span class="q9-st"><span class="q9-dot off"></span>Stopped</span>')
def a11_menu(hi="zones", voice_open=False):
    g1 = a9_row("zone", "Manage zones", ic("chev-right"), "hi" if hi == "zones" else "")
    g2 = (a9_row("stop", "Stop", '<span class="tx">Play / pause</span>')
          + a9_row("r11-spk", "Mute", '<span class="toggle"></span>'))
    g3 = (a9_row("audio", "Voice", a10_val("Adam"), "hi op" if voice_open else "")
          + a9_row("r10-speed", "Speed", A10_STEP)
          + a9_row("volume", "Volume", A10_VOL)
          + a9_row("r8-mode", "Mode", A10_MODE))
    g4 = (a9_row("settings", "Settings", ic("r8-open"))
          + a9_row("r10-power", "Quit", '<span class="tx">Press twice</span>', "quit"))
    return f'''<div class="q9 qa9">
<div class="hd"><span class="gn">{R9_GAME}</span>{a11_status()}</div>
<div class="ln"></div>
<div class="grp">{g1}</div>
<div class="ln"></div>
<div class="grp">{g2}</div>
<div class="ln"></div>
<div class="grp">{g3}</div>
<div class="ln"></div>
<div class="grp">{g4}</div>
</div>'''
# Other states, shown small beside the menu: stopped (Stop becomes Resume) and Quit armed.
a11_states = f'''<div class="q11s">
<span class="lb">When stopped</span>
<div class="q9"><div class="hd"><span class="gn">{R9_GAME}</span>{a11_status(False)}</div>
<div class="grp">{a9_row("r8-play", "Resume", '<span class="tx">Play / pause</span>', "hi")}</div></div>
<span class="lb" style="margin-top:8px">After one press on Quit</span>
<div class="q9"><div class="grp">{a9_row("r10-power", "Quit", '<span class="tx">Press again to quit</span>', "arm")}</div></div>
</div>'''
html = head("Quick menu A", cssA11) + r8_frame() + f'''<div class="q9-stage">
<div class="q11w">{a11_menu()}{a11_states}</div>
{a10_pill([("&uarr;&darr;", "Move"), ("&larr;&rarr;", "Change"), ("Enter", "Select"), ("Esc", "Close")])}
</div>
'''
write("15-quick-menu-a.html", html)

# ---------- 15-quick-menu-voice: voice list as a flyout right of the Voice row ----------
# The main panel keeps menu A's size and place; only the flyout is added.
cssV11 = cssA11 + """
.q11f{position:absolute;left:calc(100% + 8px);top:208px;width:270px;border-radius:12px;overflow:hidden;padding:6px}
.q11f .vr{position:relative;display:flex;align-items:center;gap:10px;height:36px;padding:0 6px 0 10px;border-radius:8px;font-size:14px;font-weight:500;color:var(--text-2)}
.q11f .vr .ck{width:16px;height:16px;color:var(--accent);stroke-width:2}
.q11f .vr .ck.no{visibility:hidden}
.q11f .vr .nm{flex:1}
.q11f .vr .ds{font-size:12px;color:var(--text-3)}
.q11f .vr .pb{width:26px;height:26px;border-radius:50%;display:flex;align-items:center;justify-content:center;color:var(--text-3)}
.q11f .vr .pb .ic{width:12px;height:12px;fill:currentColor;stroke:none}
.q11f .vr.sel{color:var(--text)}
.q11f .vr.hi{background:var(--surface-3);color:var(--text)}
.q11f .vr.hi::before{content:"";position:absolute;left:0;top:9px;bottom:9px;width:2px;border-radius:1px;background:var(--accent)}
.q11f .vr.hi .pb{background:var(--surface-2);border:1px solid var(--line-2);color:var(--text)}
.qa9 .r.hi.op::before{background:var(--text-3)}
"""
def v11_row(n, d, sel, hi):
    cls = "vr" + (" sel" if sel else "") + (" hi" if hi else "")
    ck = ic("check").replace('class="ic"', 'class="ic ck"' if sel else 'class="ic ck no"')
    return f'<div class="{cls}">{ck}<span class="nm">{n}</span><span class="ds">{d}</span><span class="pb">{ic("r8-play")}</span></div>'
vfly = '<div class="q9 q11f">' + "".join(v11_row(n, d, s, n == "Bella") for n, d, s in V10) + '</div>'
html = head("Quick menu voice", cssV11) + r8_frame() + f'''<div class="q9-stage">
<div class="q11w">{a11_menu(hi="", voice_open=True)}{vfly}</div>
{a10_pill([("&uarr;&darr;", "Move"), ("Space", "Preview"), ("Enter", "Choose"), ("&larr;", "Back")])}
</div>
'''
write("15-quick-menu-voice.html", html)

# ---------- 16-zone-edit: undimmed zone interiors, edge margin, clearer selection ----------
EDGE = 12
css16b = css16 + """
.dim{display:none}
.dimsvg{position:absolute;inset:0;width:100%;height:100%;display:block}
.ez{border-color:rgba(237,240,246,0.62);background:none}
.ez.sel{border:2px solid var(--accent);background:rgba(91,124,255,0.14)}
.ez.sel::after{content:"";position:absolute;inset:-5px;border:3px solid rgba(91,124,255,0.22);border-radius:7px;pointer-events:none}
.ez.sel .bd{left:-2px;color:#FFFFFF;background:var(--accent);border-color:var(--accent)}
.ez .hd{width:9px;height:9px;margin:-4.5px 0 0 -4.5px}
.ez .del{right:-30px;top:-30px;width:24px;height:24px}
.ez .del .ic{width:13px;height:13px}
.nz{border:1.5px solid rgba(91,124,255,0.50);background:rgba(91,124,255,0.05)}
"""
boxes = []
ez = []
for k, n, x1, y1, x2, y2 in ZONES:
    x1 = max(x1 + ZX, EDGE); x2 = min(x2 + ZX, 1440 - EDGE)
    boxes.append((x1, y1, x2, y2))
    sel = n == SEL10
    inner = f'<span class="bd">{ic("k-" + k)}</span>'
    if sel:
        for hx, hy in [(0,0),(50,0),(100,0),(0,100),(50,100),(100,100),(0,50),(100,50)]:
            inner += f'<span class="hd" style="left:{hx}%;top:{hy}%"></span>'
        inner += f'<span class="del">{ic("x")}</span>'
    ez.append(f'<div class="ez{" sel" if sel else ""}" style="left:{x1}px;top:{y1}px;width:{x2-x1}px;height:{y2-y1}px">{inner}</div>')
nx1, ny1, nx2, ny2 = NEW10
# One even-odd path: the full frame minus every saved zone, so their interiors stay at full brightness.
# The draft zone stays dimmed, so the selected zone remains the strongest element.
dpath = "M0 0H1440V900H0Z" + "".join(f"M{a} {b}H{c}V{d}H{a}Z" for a, b, c, d in boxes)
dim = (f'<svg class="dimsvg" viewBox="0 0 1440 900" preserveAspectRatio="none">'
       f'<path d="{dpath}" fill="rgba(5,7,11,0.34)" fill-rule="evenodd"/></svg>')
nz = (f'<div class="nz" style="left:{nx1}px;top:{ny1}px;width:{nx2-nx1}px;height:{ny2-ny1}px">'
      f'<span class="cur">{ic("r10-cross")}</span><span class="sz">{nx2-nx1} &times; {ny2-ny1}</span></div>')
html = head("Zone edit", R9_BASE + css16b) + f'''<div class="frame">{SCENE}</div>
{dim}
{"".join(ez)}
{nz}
{kp}
{tb}
'''
write("16-zone-edit.html", html)
print("round 11 ok")
# ===== END round 11 =====
