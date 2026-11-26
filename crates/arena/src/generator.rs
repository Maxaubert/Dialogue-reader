//! Seeded suite generator with genre templates. Same seed, same suite (on the same fonts).

use crate::background::{self, BgTextMap};
use crate::corpus;
use crate::fonts;
use crate::model::*;
use crate::rng::{Rng, mix};
use anyhow::{Context, Result, bail};
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SuiteKind {
    Smoke,
    Standard,
    Hard,
}

impl SuiteKind {
    pub fn parse(s: &str) -> Result<Self> {
        Ok(match s {
            "smoke" => SuiteKind::Smoke,
            "standard" => SuiteKind::Standard,
            "hard" => SuiteKind::Hard,
            _ => bail!("unknown suite {s:?} (smoke, standard, hard)"),
        })
    }
    pub fn name(self) -> &'static str {
        match self {
            SuiteKind::Smoke => "smoke",
            SuiteKind::Standard => "standard",
            SuiteKind::Hard => "hard",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Difficulty {
    Easy,
    Normal,
    Hard,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Genre {
    RpgDialogue,
    PickupStack,
    Subtitles,
    ObjectiveUpdate,
    Tooltip,
    PromptFlicker,
    StaticHud,
    DamageNumbers,
    MenuOverlay,
}

impl Genre {
    pub fn name(self) -> &'static str {
        match self {
            Genre::RpgDialogue => "rpg_dialogue",
            Genre::PickupStack => "pickup_stack",
            Genre::Subtitles => "subtitles",
            Genre::ObjectiveUpdate => "objective_update",
            Genre::Tooltip => "tooltip",
            Genre::PromptFlicker => "prompt_flicker",
            Genre::StaticHud => "static_hud",
            Genre::DamageNumbers => "damage_numbers",
            Genre::MenuOverlay => "menu_overlay",
        }
    }
}

/// Genres whose text should be read.
const READ_GENRES: &[Genre] =
    &[Genre::RpgDialogue, Genre::PickupStack, Genre::Subtitles, Genre::ObjectiveUpdate, Genre::Tooltip];

/// Generator inputs that come from the PC rather than the seed.
#[derive(Clone, Debug)]
pub struct GenOptions {
    /// Folder whose subfolders are image backgrounds; `None` uses procedural backgrounds only.
    pub backgrounds_dir: Option<PathBuf>,
    /// Font families to draw from; defaults to the game-like families found on this PC.
    pub fonts: Vec<String>,
}

impl GenOptions {
    pub fn from_pc() -> Self {
        let dir = crate::data_dir().join("backgrounds");
        GenOptions { backgrounds_dir: dir.is_dir().then_some(dir), fonts: fonts::available_families() }
    }
    /// Procedural backgrounds only (used by tests, independent of arena-data).
    pub fn procedural_only() -> Self {
        GenOptions { backgrounds_dir: None, fonts: fonts::available_families() }
    }
}

/// An image background folder usable by the generator.
#[derive(Clone, Debug)]
struct ImageBg {
    /// Path as written into the scenario (relative to the repo root when possible).
    dir: String,
    first: usize,
    count: usize,
}

fn image_backgrounds(opts: &GenOptions) -> Vec<ImageBg> {
    let Some(root) = &opts.backgrounds_dir else { return Vec::new() };
    let Ok(rd) = std::fs::read_dir(root) else { return Vec::new() };
    let mut dirs: Vec<PathBuf> = rd.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
    dirs.sort();
    dirs.into_iter()
        .filter_map(|d| {
            let n = background::image_files(&d).ok()?.len();
            if n == 0 {
                return None;
            }
            let (first, last) = background::load_bg_text(&d)
                .and_then(|f| f.clean_frames)
                .filter(|[a, b]| a <= b)
                .map(|[a, b]| (a.min(n - 1), b.min(n - 1)))
                .unwrap_or((0, n - 1));
            Some(ImageBg { dir: crate::portable_path(&d), first, count: last + 1 - first })
        })
        .collect()
}

/// Generates the scenarios of a suite.
pub fn generate_suite(kind: SuiteKind, seed: u64, opts: &GenOptions) -> Vec<Scenario> {
    let images = image_backgrounds(opts);
    let n = match kind {
        SuiteKind::Smoke => 8,
        SuiteKind::Standard | SuiteKind::Hard => 40,
    };
    (0..n)
        .map(|i| {
            let s = mix(seed ^ mix(i as u64 + 1));
            generate_scenario(kind, i, s, opts, &images)
        })
        .collect()
}

/// Grows every read zone (not `hud`) by `pad` pixels per side, one side at a time, where the
/// grown zone stays in the frame, 4 px away from the other zones, and away from text items that
/// the zone did not hold before (so no new text falls inside). Models zones drawn loosely.
pub fn loosen_zones(s: &mut Scenario, pad: f32) {
    let (fw, fh) = (s.width as f32, s.height as f32);
    let orig: Vec<Rect> = s.layout.zones.iter().map(|z| z.rect).collect();
    for i in 0..s.layout.zones.len() {
        if s.layout.zones[i].kind == crate::model::ZoneKind::Hud {
            continue;
        }
        let mut r = orig[i];
        for side in 0..4 {
            let mut c = r;
            match side {
                0 => {
                    c.x -= pad;
                    c.w += pad;
                }
                1 => c.w += pad,
                2 => {
                    c.y -= pad;
                    c.h += pad;
                }
                _ => c.h += pad,
            }
            if c.x < 0.0 || c.y < 0.0 || c.right() > fw || c.bottom() > fh {
                continue;
            }
            let big = c.inflate(4.0);
            if orig.iter().enumerate().any(|(j, o)| j != i && big.intersects(o)) {
                continue;
            }
            if s.items.iter().any(|it| big.intersects(&it.rect) && !orig[i].intersects(&it.rect)) {
                continue;
            }
            r = c;
        }
        s.layout.zones[i].rect = r;
    }
}

fn generate_scenario(kind: SuiteKind, index: usize, seed: u64, opts: &GenOptions, images: &[ImageBg]) -> Scenario {
    let mut rng = Rng::new(seed);
    let (width, height, fps) = (1280u32, 720u32, 30.0);
    let diff = match kind {
        SuiteKind::Smoke => Difficulty::Easy,
        SuiteKind::Standard => Difficulty::Normal,
        SuiteKind::Hard => Difficulty::Hard,
    };

    // Genres.
    let mut genres: Vec<Genre> = match kind {
        SuiteKind::Smoke => {
            let fixed = [
                vec![Genre::RpgDialogue],
                vec![Genre::PickupStack, Genre::StaticHud],
                vec![Genre::Subtitles],
                vec![Genre::ObjectiveUpdate, Genre::StaticHud],
                vec![Genre::Tooltip],
                vec![Genre::PromptFlicker, Genre::Subtitles],
                vec![Genre::DamageNumbers, Genre::Subtitles],
                vec![Genre::MenuOverlay, Genre::ObjectiveUpdate],
            ];
            fixed[index % fixed.len()].clone()
        }
        SuiteKind::Standard | SuiteKind::Hard => {
            let mut g = vec![READ_GENRES[index % READ_GENRES.len()]];
            if rng.chance(0.35) {
                let extra = *rng.pick(READ_GENRES);
                if !g.contains(&extra) {
                    g.push(extra);
                }
            }
            let noise_p = if diff == Difficulty::Hard { 0.6 } else { 0.4 };
            if rng.chance(0.7) {
                g.push(Genre::StaticHud);
            }
            if rng.chance(noise_p) {
                g.push(Genre::DamageNumbers);
            }
            if rng.chance(noise_p * 0.6) {
                g.push(Genre::MenuOverlay);
            }
            if rng.chance(noise_p * 0.8) {
                g.push(Genre::PromptFlicker);
            }
            g
        }
    };
    // Dialogue and subtitles share the bottom of the screen; keep one of them.
    if genres.contains(&Genre::RpgDialogue) && genres.contains(&Genre::Subtitles) {
        genres.retain(|g| *g != Genre::Subtitles);
    }

    let duration_ms = match kind {
        SuiteKind::Smoke => rng.range(10_000.0, 14_000.0),
        SuiteKind::Standard => rng.range(15_000.0, 40_000.0),
        SuiteKind::Hard => rng.range(10_000.0, 30_000.0),
    }
    .round();

    // Background.
    let image_p = match diff {
        Difficulty::Easy => 0.25,
        Difficulty::Normal => 0.4,
        Difficulty::Hard => 0.5,
    };
    let background = if !images.is_empty() && rng.chance(image_p) {
        let img = rng.pick(images).clone();
        let bg_fps = *rng.pick(&[10.0, 15.0, 30.0]);
        Background::Images {
            dir: img.dir,
            fps: bg_fps,
            first_frame: img.first,
            count: Some(img.count),
            offset_ms: (rng.range(0.0, img.count as f64 / bg_fps * 1000.0)).round(),
        }
    } else {
        let kinds: &[ProceduralKind] = match diff {
            Difficulty::Easy => &[ProceduralKind::Gradient, ProceduralKind::MovingShapes],
            Difficulty::Normal => &[ProceduralKind::Gradient, ProceduralKind::MovingShapes, ProceduralKind::Noise, ProceduralKind::GrassLike],
            Difficulty::Hard => &[ProceduralKind::Noise, ProceduralKind::GrassLike, ProceduralKind::MovingShapes],
        };
        Background::Procedural { kind: *rng.pick(kinds), seed: rng.next_u64() >> 12 }
    };
    let keepouts = BgTextMap::for_background(&background, width, height).ever_active();

    let fonts = if opts.fonts.is_empty() { vec!["Segoe UI".to_string()] } else { opts.fonts.clone() };
    let mut ctx = Ctx {
        rng: rng.fork(7),
        w: width as f32,
        h: height as f32,
        dur: duration_ms,
        diff,
        fonts,
        keepouts,
        items: Vec::new(),
        item_genre: Vec::new(),
        zones: Vec::new(),
        zone_genre: Vec::new(),
        cur: genres[0],
        next_id: 0,
    };
    // Genres with a fixed zone for the whole scene go first, then read genres placed per item,
    // then noise, so the text to read gets the free places.
    let mut order = genres.clone();
    order.sort_by_key(|g| placement_rank(*g));
    for g in &order {
        ctx.cur = *g;
        match g {
            Genre::RpgDialogue => ctx.rpg_dialogue(),
            Genre::PickupStack => ctx.pickup_stack(),
            Genre::Subtitles => ctx.subtitles(),
            Genre::ObjectiveUpdate => ctx.objective_update(),
            Genre::Tooltip => ctx.tooltip(),
            Genre::PromptFlicker => ctx.prompt_flicker(),
            Genre::StaticHud => ctx.static_hud(),
            Genre::DamageNumbers => ctx.damage_numbers(),
            Genre::MenuOverlay => ctx.menu_overlay(),
        }
    }
    for it in &mut ctx.items {
        it.start_ms = it.start_ms.round().max(0.0);
        it.end_ms = it.end_ms.round().min(duration_ms);
    }
    ctx.items.retain(|it| it.end_ms - it.start_ms >= 100.0);
    finish_anims(&mut ctx.items);

    Scenario {
        id: format!("{}-{:03}", kind.name(), index),
        seed,
        genres: genres.iter().map(|g| g.name().to_string()).collect(),
        width,
        height,
        fps,
        duration_ms,
        background,
        layout: Layout { zones: ctx.zones },
        items: ctx.items,
    }
}

/// Placement order of a genre (lower first).
fn placement_rank(g: Genre) -> u8 {
    match g {
        Genre::RpgDialogue | Genre::Subtitles => 0,
        Genre::PickupStack => 1,
        Genre::ObjectiveUpdate => 2,
        Genre::Tooltip => 3,
        Genre::PromptFlicker => 4,
        Genre::StaticHud => 5,
        Genre::MenuOverlay => 6,
        Genre::DamageNumbers => 7,
    }
}

/// Last pass over the final item times. Fades take at most a quarter of the item each, so every
/// item is fully shown for at least half its time. A toast replaced in place (its counter changed)
/// does not fade out: the new text takes its place at once. `replace_of` links to items that were
/// dropped are cleared.
fn finish_anims(items: &mut [TextItem]) {
    let replaced: Vec<String> = items.iter().filter_map(|i| i.anim.replace_of.clone()).collect();
    let ids: Vec<String> = items.iter().map(|i| i.id.clone()).collect();
    for it in items.iter_mut() {
        let cap = 0.25 * it.duration_ms();
        it.anim.fade_in_ms = it.anim.fade_in_ms.min(cap).round();
        it.anim.fade_out_ms = it.anim.fade_out_ms.min(cap).round();
        if replaced.contains(&it.id) {
            it.anim.fade_out_ms = 0.0;
            it.anim.slide_out = [0.0, 0.0];
        }
        if it.anim.replace_of.as_ref().is_some_and(|r| !ids.contains(r)) {
            it.anim.replace_of = None;
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Look {
    /// Text in a box (dialogue, toasts, tooltips).
    Boxed,
    /// Free text over the scene (subtitles, damage numbers).
    Free,
    /// Either.
    Any,
}

struct Ctx {
    rng: Rng,
    w: f32,
    h: f32,
    dur: f64,
    diff: Difficulty,
    fonts: Vec<String>,
    keepouts: Vec<Rect>,
    items: Vec<TextItem>,
    /// Genre that made each item.
    item_genre: Vec<Genre>,
    zones: Vec<Zone>,
    /// Genre that made each zone.
    zone_genre: Vec<Genre>,
    /// Genre being generated.
    cur: Genre,
    next_id: usize,
}

/// Margin kept between text of different genres, px.
const GAP_PX: f32 = 4.0;

const LIGHT_COLORS: &[Color] = &[
    [255, 255, 255, 255],
    [255, 240, 200, 255],
    [230, 230, 230, 255],
    [255, 220, 120, 255],
    [200, 230, 255, 255],
    [190, 255, 190, 255],
];
const MID_COLORS: &[Color] = &[[170, 170, 160, 255], [150, 190, 150, 255], [200, 180, 140, 255], [160, 170, 200, 255]];
const PANEL_COLORS: &[Color] = &[[10, 10, 20, 255], [30, 25, 20, 255], [15, 30, 45, 255], [40, 10, 10, 255], [0, 0, 0, 255]];

impl Ctx {
    fn id(&mut self, prefix: &str) -> String {
        self.next_id += 1;
        format!("{prefix}{:03}", self.next_id)
    }

    /// Durations in ms for one item, by difficulty (spec: 0.3 to 6 s).
    fn duration(&mut self, easy: (f64, f64), normal: (f64, f64), hard: (f64, f64)) -> f64 {
        let (lo, hi) = match self.diff {
            Difficulty::Easy => easy,
            Difficulty::Normal => normal,
            Difficulty::Hard => hard,
        };
        self.rng.range(lo, hi) * 1000.0
    }

    fn size(&mut self, scale: f32) -> f32 {
        let (lo, hi) = match self.diff {
            Difficulty::Easy => (24.0, 32.0),
            Difficulty::Normal => (18.0, 30.0),
            Difficulty::Hard => (13.0, 21.0),
        };
        (self.rng.rangef(lo, hi) * scale).round()
    }

    fn style(&mut self, look: Look, size_scale: f32) -> Style {
        let font = self.rng.pick(&self.fonts).clone();
        let size_px = self.size(size_scale);
        let hard = self.diff == Difficulty::Hard;
        let color = if hard && self.rng.chance(0.35) { *self.rng.pick(MID_COLORS) } else { *self.rng.pick(LIGHT_COLORS) };
        let boxed = match look {
            Look::Boxed => self.rng.chance(if hard { 0.6 } else { 0.9 }),
            Look::Free => false,
            Look::Any => self.rng.chance(0.5),
        };
        let panel = boxed.then(|| {
            let alpha = match self.diff {
                Difficulty::Easy => self.rng.rangef(0.75, 0.92),
                Difficulty::Normal => self.rng.rangef(0.45, 0.9),
                Difficulty::Hard => self.rng.rangef(0.15, 0.5),
            };
            Panel {
                color: *self.rng.pick(PANEL_COLORS),
                alpha,
                padding: (size_px * self.rng.rangef(0.3, 0.6)).round(),
                radius: *self.rng.pick(&[0.0, 4.0, 8.0, 14.0]),
            }
        });
        let outline_p = if boxed { 0.25 } else if hard { 0.4 } else { 0.6 };
        let outline = self.rng.chance(outline_p).then(|| Outline {
            color: [0, 0, 0, 255],
            px: if hard { 1.0 } else { *self.rng.pick(&[1.0, 2.0, 2.0, 3.0]) },
        });
        let shadow_p = if boxed { 0.3 } else if hard { 0.4 } else { 0.7 };
        let shadow = self.rng.chance(shadow_p).then(|| Shadow {
            color: [0, 0, 0, if hard { 140 } else { 200 }],
            dx: self.rng.rangef(0.0, 3.0).round(),
            dy: self.rng.rangef(1.0, 3.0).round(),
            blur: self.rng.rangef(0.0, 4.0).round(),
        });
        Style {
            font,
            size_px,
            color,
            outline,
            shadow,
            panel,
            bold: self.rng.chance(0.3),
            italic: self.rng.chance(0.08),
            align: Align::Left,
            line_spacing: 1.25,
        }
    }

    fn measure(&self, st: &Style, text: &str) -> (f32, f32) {
        let (w, h) = fonts::measure(&st.font, st.bold, st.italic, st.size_px, st.line_spacing, text);
        let pad = st.panel.as_ref().map(|p| p.padding).unwrap_or(0.0);
        ((w + 2.0 * pad).ceil(), (h + 2.0 * pad).ceil())
    }

    fn wrap(&self, st: &Style, text: &str, max_w: f32) -> String {
        let pad = st.panel.as_ref().map(|p| p.padding).unwrap_or(0.0);
        fonts::wrap(&st.font, st.bold, st.italic, st.size_px, text, max_w - 2.0 * pad)
    }

    /// Inside the frame and away from background text.
    fn clear(&self, r: &Rect) -> bool {
        r.x >= 0.0
            && r.y >= 0.0
            && r.right() <= self.w
            && r.bottom() <= self.h
            && !self.keepouts.iter().any(|k| k.intersects(&r.inflate(GAP_PX)))
    }

    /// Away from the zones of other genres, so a zone reader never sees two genres in one zone.
    fn zone_free(&self, r: &Rect) -> bool {
        !self.zones.iter().zip(&self.zone_genre).any(|(z, g)| *g != self.cur && z.rect.intersects(&r.inflate(GAP_PX)))
    }

    /// No item of another genre is on screen at the same place during `start..end`.
    fn free_of_others(&self, r: &Rect, start: f64, end: f64) -> bool {
        let big = r.inflate(GAP_PX);
        !self.items.iter().zip(&self.item_genre).any(|(it, g)| *g != self.cur && it.start_ms < end && start < it.end_ms && it.rect.intersects(&big))
    }

    /// Where text of a read genre (or a prompt) may go: clear, out of other zones, not covered.
    fn ok_read(&self, r: &Rect, start: f64, end: f64) -> bool {
        self.clear(r) && self.zone_free(r) && self.free_of_others(r, start, end)
    }

    /// First candidate that passes `ok`, or `None` (the caller then skips the item).
    fn choose(&self, candidates: &[Rect], ok: impl Fn(&Rect) -> bool) -> Option<Rect> {
        candidates.iter().find(|r| ok(r)).copied()
    }

    fn zone(&mut self, name: &str, kind: ZoneKind, rect: Rect) {
        if !self.zones.iter().any(|z| z.name == name) {
            self.zones.push(Zone { name: name.to_string(), kind, rect });
            self.zone_genre.push(self.cur);
        }
    }

    fn fades(&mut self, max_in: f64, max_out: f64) -> (f64, f64) {
        let fi = if self.rng.chance(0.6) { self.rng.range(80.0, max_in).round() } else { 0.0 };
        let fo = if self.rng.chance(0.6) { self.rng.range(80.0, max_out).round() } else { 0.0 };
        (fi, fo)
    }

    fn typewriter(&mut self) -> Option<f32> {
        let (p, lo, hi) = match self.diff {
            Difficulty::Easy => (0.5, 40.0, 60.0),
            Difficulty::Normal => (0.5, 25.0, 60.0),
            Difficulty::Hard => (0.6, 40.0, 90.0),
        };
        self.rng.chance(p).then(|| self.rng.rangef(lo, hi).round())
    }

    #[allow(clippy::too_many_arguments)]
    fn push(&mut self, id: String, text: String, kind: ItemKind, rect: Rect, start: f64, end: f64, style: Style, anim: Anim) -> usize {
        let should_read = kind.default_should_read();
        self.items.push(TextItem { id, text, kind, should_read, rect, start_ms: start, end_ms: end.min(self.dur), style, anim });
        self.item_genre.push(self.cur);
        self.items.len() - 1
    }

    fn rpg_dialogue(&mut self) {
        let (w, h) = (self.w, self.h);
        let boxes = [
            Rect::new(0.12 * w, 0.64 * h, 0.66 * w, 0.22 * h),
            Rect::new(0.17 * w, 0.56 * h, 0.66 * w, 0.22 * h),
            Rect::new(0.17 * w, 0.30 * h, 0.66 * w, 0.22 * h),
            // Narrower boxes that leave room for a toast stack on the left or right.
            Rect::new(0.34 * w, 0.56 * h, 0.62 * w, 0.22 * h),
            Rect::new(0.34 * w, 0.64 * h, 0.62 * w, 0.22 * h),
            Rect::new(0.34 * w, 0.64 * h, 0.46 * w, 0.22 * h),
            Rect::new(0.04 * w, 0.64 * h, 0.62 * w, 0.22 * h),
        ];
        let speaker_zone = |b: &Rect| Rect::new(b.x, b.y - 0.075 * h, 0.35 * w, 0.065 * h);
        let dur = self.dur;
        let Some(boxr) = boxes.iter().copied().find(|b| self.ok_read(b, 0.0, dur) && self.ok_read(&speaker_zone(b), 0.0, dur)) else {
            return;
        };
        let spk_zone = speaker_zone(&boxr);
        self.zone("dialogue", ZoneKind::Dialogue, boxr);
        self.zone("speaker", ZoneKind::Speaker, spk_zone);
        let mut st = self.style(Look::Boxed, 1.0);
        if let Some(p) = &mut st.panel {
            p.alpha = p.alpha.max(0.5);
        }
        let mut spk_st = st.clone();
        spk_st.bold = true;
        spk_st.color = *self.rng.pick(&[[255, 210, 90, 255], [140, 210, 255, 255], [255, 255, 255, 255]]);
        if let Some(p) = &mut spk_st.panel {
            p.padding = (spk_st.size_px * 0.3).round();
        }
        let typing = self.typewriter();
        let mut t = self.rng.range(300.0, 1500.0);
        while t < self.dur - 800.0 {
            let speaker = corpus::speaker(&mut self.rng);
            let line = corpus::dialogue_line(&mut self.rng);
            // Shrink the font until the wrapped line fits the box.
            let mut lst = st.clone();
            let text = loop {
                let wrapped = self.wrap(&lst, &line, boxr.w);
                let (_, th) = self.measure(&lst, &wrapped);
                if th <= boxr.h || lst.size_px <= 12.0 {
                    break wrapped;
                }
                lst.size_px -= 2.0;
            };
            let mut d = self.duration((2.5, 5.5), (1.0, 6.0), (0.4, 2.5));
            if let Some(cps) = typing {
                let typed_ms = text.chars().count() as f64 / cps as f64 * 1000.0;
                // A line the scene end would cut before it is typed out is not started.
                if t + typed_ms + 600.0 > self.dur {
                    break;
                }
                d = d.max(typed_ms + 600.0);
            }
            let end = t + d;
            let (fi, fo) = self.fades(250.0, 250.0);
            let anim = Anim { fade_in_ms: fi, fade_out_ms: fo, typewriter_cps: typing, ..Default::default() };
            let (sw, sh) = self.measure(&spk_st, &speaker);
            let spk_rect = Rect::new(spk_zone.x, spk_zone.bottom() - sh, sw, sh);
            if self.free_of_others(&spk_rect, t, end) && self.free_of_others(&boxr, t, end) {
                let sid = self.id("spk");
                self.push(sid, speaker, ItemKind::Speaker, spk_rect, t, end, spk_st.clone(), Anim { fade_in_ms: fi, fade_out_ms: fo, ..Default::default() });
                let did = self.id("dlg");
                self.push(did, text, ItemKind::Dialogue, boxr, t, end, lst, anim);
            }
            t = end + self.rng.range(150.0, 1000.0);
        }
    }

    fn pickup_stack(&mut self) {
        let (w, h) = (self.w, self.h);
        let dur = self.dur;
        let Some(zone) = self.choose(
            &[
                Rect::new(0.02 * w, 0.40 * h, 0.30 * w, 0.36 * h),
                Rect::new(0.68 * w, 0.40 * h, 0.30 * w, 0.36 * h),
                Rect::new(0.02 * w, 0.18 * h, 0.30 * w, 0.36 * h),
                Rect::new(0.68 * w, 0.18 * h, 0.30 * w, 0.36 * h),
            ],
            |r| self.ok_read(r, 0.0, dur),
        ) else {
            return;
        };
        self.zone("pickups", ZoneKind::Notification, zone);
        let st = self.style(Look::Boxed, 0.85);
        let with_total = self.rng.chance(0.6);
        let (_, row_h) = self.measure(&st, "+ 0 Xg (0)");
        let slot_h = row_h + 6.0;
        let slots = ((zone.h / slot_h).floor() as usize).max(1);
        // Live toasts: (item index, slot, name, count, total).
        let mut live: Vec<(usize, usize, String, i64, i64)> = Vec::new();
        let mut t = self.rng.range(300.0, 1200.0);
        while t < self.dur - 400.0 {
            live.retain(|(i, ..)| self.items[*i].end_ms > t);
            let d = self.duration((2.0, 4.5), (0.8, 5.0), (0.3, 1.8));
            let (fi, fo) = self.fades(200.0, 350.0);
            // Replace a toast in place when its counter changes.
            let replace = live
                .iter()
                .position(|(i, ..)| t - self.items[*i].start_ms > 400.0)
                .filter(|_| self.rng.chance(0.35));
            if let Some(k) = replace {
                let (old, slot, name, count, total) = live[k].clone();
                let add = self.rng.int(1, 12);
                let (n, tot) = (count + add, total + add);
                let text = corpus::pickup_text(&name, n, with_total.then_some(tot));
                let (tw, th) = self.measure(&st, &text);
                let rect = Rect::new(zone.x, zone.y + slot as f32 * slot_h, tw, th);
                if self.free_of_others(&rect, t, t + d) {
                    live.remove(k);
                    self.items[old].end_ms = t;
                    let old_id = self.items[old].id.clone();
                    let id = self.id("toast");
                    let anim = Anim { fade_out_ms: fo, replace_of: Some(old_id), ..Default::default() };
                    let idx = self.push(id, text, ItemKind::PickupToast, rect, t, t + d, st.clone(), anim);
                    live.push((idx, slot, name, n, tot));
                }
            } else if let Some(slot) = (0..slots).find(|s| !live.iter().any(|l| l.1 == *s)) {
                let name = self.rng.pick(corpus::PICKUP_ITEMS).to_string();
                let n = self.rng.int(1, 9);
                let total = n + self.rng.int(0, 40);
                let text = corpus::pickup_text(&name, n, with_total.then_some(total));
                let (tw, th) = self.measure(&st, &text);
                let rect = Rect::new(zone.x, zone.y + slot as f32 * slot_h, tw, th);
                if self.free_of_others(&rect, t, t + d) {
                    let id = self.id("toast");
                    let dx = if zone.x < w / 2.0 { -40.0 } else { 40.0 };
                    let anim = Anim { fade_in_ms: fi, fade_out_ms: fo, slide_in: [dx, 0.0], slide_ms: 180.0, ..Default::default() };
                    let idx = self.push(id, text, ItemKind::PickupToast, rect, t, t + d, st.clone(), anim);
                    live.push((idx, slot, name, n, total));
                }
            }
            t += self.duration((0.8, 2.5), (0.3, 2.5), (0.2, 1.2));
        }
    }

    fn subtitles(&mut self) {
        let (w, h) = (self.w, self.h);
        let dur = self.dur;
        let Some(zone) = self.choose(
            &[
                Rect::new(0.12 * w, 0.78 * h, 0.76 * w, 0.14 * h),
                Rect::new(0.12 * w, 0.66 * h, 0.76 * w, 0.14 * h),
                Rect::new(0.34 * w, 0.78 * h, 0.62 * w, 0.14 * h),
                Rect::new(0.04 * w, 0.78 * h, 0.62 * w, 0.14 * h),
            ],
            |r| self.ok_read(r, 0.0, dur),
        ) else {
            return;
        };
        self.zone("subtitles", ZoneKind::Dialogue, zone);
        let mut st = self.style(Look::Any, 1.0);
        st.align = Align::Center;
        let prefix = self.rng.chance(0.3);
        let mut t = self.rng.range(200.0, 1200.0);
        while t < self.dur - 300.0 {
            let mut line = corpus::dialogue_line(&mut self.rng);
            if prefix {
                line = format!("{}: {line}", corpus::speaker(&mut self.rng).to_uppercase());
            }
            let mut lst = st.clone();
            let text = loop {
                let wrapped = self.wrap(&lst, &line, zone.w);
                let (_, th) = self.measure(&lst, &wrapped);
                if th <= zone.h || lst.size_px <= 12.0 {
                    break wrapped;
                }
                lst.size_px -= 2.0;
            };
            let (tw, th) = self.measure(&lst, &text);
            let rect = Rect::new(zone.x + (zone.w - tw) / 2.0, zone.bottom() - th, tw, th);
            let d = self.duration((2.0, 5.0), (0.7, 5.0), (0.3, 2.0));
            let (fi, fo) = self.fades(150.0, 150.0);
            if self.free_of_others(&rect, t, t + d) {
                let id = self.id("sub");
                self.push(id, text, ItemKind::Subtitle, rect, t, t + d, lst, Anim { fade_in_ms: fi, fade_out_ms: fo, ..Default::default() });
            }
            t += d + self.rng.range(100.0, 900.0);
        }
    }

    fn objective_update(&mut self) {
        let (w, h) = (self.w, self.h);
        let mut st = self.style(Look::Any, 1.05);
        st.bold = self.rng.chance(0.6);
        // Objectives of a game always show at one place: pick it for the widest text, so every
        // objective lies inside the zone.
        let widest = corpus::all_objectives().iter().map(|o| self.measure(&st, o)).fold((0.0f32, 0.0f32), |a, b| (a.0.max(b.0), a.1.max(b.1)));
        let (mw, mh) = widest;
        // Anchors: (x of the left edge for text width tw, y). 0 = centred, 1 = left, 2 = right.
        let anchors: [(u8, f32); 4] = [(0, 0.11 * h), (1, 0.12 * h), (2, 0.18 * h), (0, 0.22 * h)];
        let x_for = |a: u8, tw: f32| match a {
            0 => (w - tw) / 2.0,
            1 => 0.03 * w,
            _ => w - tw - 0.03 * w,
        };
        let dur = self.dur;
        let zone_of = |a: u8, y: f32| Rect::new(x_for(a, mw), y, mw, mh).inflate(8.0);
        let Some(&(anchor, y)) = anchors.iter().find(|(a, y)| self.ok_read(&zone_of(*a, *y), 0.0, dur)) else {
            return;
        };
        self.zone("objective", ZoneKind::Notification, zone_of(anchor, y));
        let mut t = self.rng.range(500.0, 3000.0);
        while t < self.dur - 1000.0 {
            let text = corpus::objective(&mut self.rng);
            let (tw, th) = self.measure(&st, &text);
            let rect = Rect::new(x_for(anchor, tw), y, tw, th);
            let d = self.duration((3.0, 6.0), (1.5, 6.0), (0.5, 3.0));
            let (fi, fo) = self.fades(300.0, 400.0);
            if self.free_of_others(&rect, t, t + d) {
                let id = self.id("obj");
                let anim = Anim { fade_in_ms: fi, fade_out_ms: fo, slide_in: [0.0, -25.0], slide_ms: 250.0, ..Default::default() };
                self.push(id, text, ItemKind::Objective, rect, t, t + d, st.clone(), anim);
            }
            t += d + self.rng.range(1500.0, 5000.0);
        }
    }

    fn tooltip(&mut self) {
        let (w, h) = (self.w, self.h);
        let st = self.style(Look::Boxed, 0.8);
        let mut t = self.rng.range(300.0, 1500.0);
        while t < self.dur - 500.0 {
            let text = corpus::tooltip(&mut self.rng);
            let (tw, th) = self.measure(&st, &text);
            let mut cands = Vec::new();
            for _ in 0..12 {
                let x = self.rng.rangef(0.25 * w, 0.75 * w - tw).max(0.0);
                let y = self.rng.rangef(0.2 * h, 0.62 * h - th).max(0.0);
                cands.push(Rect::new(x.round(), y.round(), tw, th));
            }
            let d = self.duration((1.5, 4.0), (0.6, 4.0), (0.3, 1.5));
            let (fi, fo) = self.fades(120.0, 120.0);
            if let Some(rect) = self.choose(&cands, |r| self.ok_read(r, t, t + d)) {
                let id = self.id("tip");
                self.push(id, text, ItemKind::Tooltip, rect, t, t + d, st.clone(), Anim { fade_in_ms: fi, fade_out_ms: fo, ..Default::default() });
            }
            t += d + self.rng.range(300.0, 2500.0);
        }
    }

    fn prompt_flicker(&mut self) {
        let (w, h) = (self.w, self.h);
        let st = self.style(Look::Any, 0.85);
        // One place for all prompts, sized for the widest one, so every prompt is in the zone.
        let (mw, mh) = corpus::PROMPTS.iter().map(|p| self.measure(&st, p)).fold((0.0f32, 0.0f32), |a, b| (a.0.max(b.0), a.1.max(b.1)));
        let centres = [(w / 2.0 + 0.1 * w, 0.52 * h), (w / 2.0, 0.45 * h), (w / 2.0 - 0.15 * w, 0.40 * h)];
        let zone_of = |cx: f32, y: f32| Rect::new(cx - mw / 2.0, y, mw, mh).inflate(20.0);
        let Some(&(cx, y)) = centres.iter().find(|(cx, y)| {
            let z = zone_of(*cx, *y);
            self.clear(&z) && self.zone_free(&z)
        }) else {
            return;
        };
        self.zone("prompt", ZoneKind::Prompt, zone_of(cx, y));
        let mut t = self.rng.range(200.0, 1500.0);
        while t < self.dur - 300.0 {
            let text = self.rng.pick(corpus::PROMPTS).to_string();
            let (tw, th) = self.measure(&st, &text);
            let rect = Rect::new(cx - tw / 2.0, y, tw, th);
            // The same prompt flickers on and off a few times.
            for _ in 0..self.rng.int(1, 5) {
                let on = self.rng.range(300.0, 2000.0);
                if self.free_of_others(&rect, t, t + on) {
                    let id = self.id("prm");
                    self.push(id, text.clone(), ItemKind::Prompt, rect, t, t + on, st.clone(), Anim::default());
                }
                t += on + self.rng.range(80.0, 900.0);
            }
            t += self.rng.range(500.0, 3000.0);
        }
    }

    fn static_hud(&mut self) {
        let (w, h) = (self.w, self.h);
        let mut st = self.style(Look::Any, 0.8);
        st.font = "Consolas".to_string();
        if !self.fonts.contains(&st.font) {
            st.font = self.fonts[0].clone();
        }
        let corners = [
            (0.02 * w, 0.06 * h),
            (0.80 * w, 0.06 * h),
            (0.02 * w, 0.24 * h),
            (0.80 * w, 0.30 * h),
            (0.40 * w, 0.95 * h),
            (0.02 * w, 0.90 * h),
        ];
        let n = self.rng.int(2, 4) as usize;
        let mut used: Vec<Rect> = Vec::new();
        let first_label = self.rng.index(corpus::HUD_LABELS.len());
        for k in 0..n {
            // Consecutive labels, so no label shows twice.
            let label = corpus::HUD_LABELS[(first_label + k) % corpus::HUD_LABELS.len()];
            let mut value = self.rng.int(20, 120);
            let text_for = |v: i64| format!("{label} {v}/{}", 120);
            let text = text_for(value);
            let (tw, th) = self.measure(&st, &text);
            let cands: Vec<Rect> = corners
                .iter()
                .map(|(x, y)| Rect::new(x.min(w - tw - 4.0), y.min(h - th - 4.0), tw, th))
                .filter(|r| !used.iter().any(|u| u.intersects(r)))
                .collect();
            let dur = self.dur;
            // Never on background text or in a zone of another genre; skip the label if no
            // corner is free.
            let Some(rect) = self.choose(&cands, |r| self.ok_read(r, 0.0, dur)) else {
                continue;
            };
            used.push(rect);
            self.zone(&format!("hud{k}"), ZoneKind::Hud, rect);
            // Some HUD values change now and then (replaced in place).
            let changes = self.rng.chance(0.5);
            let mut t = 0.0;
            let mut prev: Option<String> = None;
            while t < self.dur {
                let until = if changes { (t + self.rng.range(2000.0, 8000.0)).min(self.dur) } else { self.dur };
                let id = self.id("hud");
                let anim = Anim { replace_of: prev.clone(), ..Default::default() };
                let (tw, th) = self.measure(&st, &text_for(value));
                let r = Rect::new(rect.x, rect.y, tw, th);
                self.push(id.clone(), text_for(value), ItemKind::HudStatic, r, t, until, st.clone(), anim);
                prev = Some(id);
                value = (value - self.rng.int(1, 15)).max(0);
                t = until;
            }
        }
    }

    fn damage_numbers(&mut self) {
        let (w, h) = (self.w, self.h);
        let mut t = self.rng.range(100.0, 800.0);
        while t < self.dur - 300.0 {
            let mut st = self.style(Look::Free, 1.1);
            st.bold = true;
            st.color = *self.rng.pick(&[[255, 60, 60, 255], [255, 230, 80, 255], [255, 255, 255, 255]]);
            st.outline = Some(Outline { color: [0, 0, 0, 255], px: 2.0 });
            let text = corpus::damage_number(&mut self.rng);
            let (tw, th) = self.measure(&st, &text);
            let x = self.rng.rangef(0.3 * w, 0.7 * w - tw);
            let y = self.rng.rangef(0.2 * h, 0.55 * h);
            let d = self.rng.range(400.0, 1000.0);
            let rect = Rect::new(x.round(), y.round(), tw, th);
            if self.free_of_others(&rect, t, t + d) {
                let id = self.id("dmg");
                let anim = Anim { fade_out_ms: (d * 0.4).round(), slide_in: [0.0, 30.0], slide_ms: d.round(), ..Default::default() };
                self.push(id, text, ItemKind::DamageNumber, rect, t, t + d, st, anim);
            }
            t += self.rng.range(200.0, 1500.0);
        }
    }

    fn menu_overlay(&mut self) {
        let (w, h) = (self.w, self.h);
        let mut st = self.style(Look::Boxed, 1.0);
        if st.panel.is_none() {
            st.panel = Some(Panel { color: [20, 20, 30, 255], alpha: 0.7, padding: 8.0, radius: 4.0 });
        }
        let mut t = self.rng.range(1500.0, 5000.0);
        while t < self.dur - 2000.0 {
            let d = self.rng.range(2500.0, 7000.0);
            let n = self.rng.int(3, 7) as usize;
            let start = self.rng.index(corpus::MENU_ENTRIES.len());
            let (_, row_h) = self.measure(&st, "Mg");
            let x = 0.40 * w;
            let mut y = (0.5 * h - n as f32 * (row_h + 6.0) / 2.0).max(0.05 * h);
            let mut rows = Vec::new();
            for k in 0..n {
                let text = corpus::MENU_ENTRIES[(start + k) % corpus::MENU_ENTRIES.len()].to_string();
                let (tw, th) = self.measure(&st, &text);
                rows.push((text, Rect::new(x, y, tw.max(0.2 * w), th)));
                y += row_h + 6.0;
            }
            // A menu that would cover text of another genre is not opened.
            if rows.iter().all(|(_, r)| self.free_of_others(r, t, t + d)) {
                for (text, rect) in rows {
                    let id = self.id("menu");
                    self.push(id, text, ItemKind::Menu, rect, t, t + d, st.clone(), Anim { fade_in_ms: 120.0, fade_out_ms: 120.0, ..Default::default() });
                }
            }
            t += d + self.rng.range(3000.0, 9000.0);
        }
    }
}

/// Writes a suite folder: `suite.json` plus `<id>.json` per scenario.
pub fn write_suite(dir: &Path, name: &str, seed: u64, scenarios: &[Scenario]) -> Result<()> {
    std::fs::create_dir_all(dir).with_context(|| format!("create {}", dir.display()))?;
    let mut files = Vec::new();
    for s in scenarios {
        let f = format!("{}.json", s.id);
        std::fs::write(dir.join(&f), serde_json::to_string_pretty(s)?)?;
        files.push(f);
    }
    let suite = Suite { name: name.to_string(), seed, scenarios: files };
    std::fs::write(dir.join("suite.json"), serde_json::to_string_pretty(&suite)?)?;
    Ok(())
}

pub fn load_scenario(path: &Path) -> Result<Scenario> {
    let s = std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    serde_json::from_str(&s).with_context(|| format!("parse {}", path.display()))
}

pub fn load_suite(dir: &Path) -> Result<(Suite, Vec<Scenario>)> {
    let p = dir.join("suite.json");
    let suite: Suite = serde_json::from_str(&std::fs::read_to_string(&p).with_context(|| format!("read {}", p.display()))?)
        .with_context(|| format!("parse {}", p.display()))?;
    let scenarios = suite.scenarios.iter().map(|f| load_scenario(&dir.join(f))).collect::<Result<Vec<_>>>()?;
    Ok((suite, scenarios))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_same_suite() {
        let opts = GenOptions::procedural_only();
        for kind in [SuiteKind::Smoke, SuiteKind::Hard] {
            let a = serde_json::to_string(&generate_suite(kind, 11, &opts)).unwrap();
            let b = serde_json::to_string(&generate_suite(kind, 11, &opts)).unwrap();
            assert_eq!(a, b);
            let c = serde_json::to_string(&generate_suite(kind, 12, &opts)).unwrap();
            assert_ne!(a, c);
        }
    }

    #[test]
    fn suites_have_expected_shape() {
        let opts = GenOptions::procedural_only();
        let smoke = generate_suite(SuiteKind::Smoke, 1, &opts);
        assert_eq!(smoke.len(), 8);
        let std = generate_suite(SuiteKind::Standard, 1, &opts);
        assert_eq!(std.len(), 40);
        for s in smoke.iter().chain(&std) {
            assert!(s.items.iter().any(|i| i.should_read), "{} has nothing to read", s.id);
            for it in &s.items {
                assert!(it.end_ms > it.start_ms, "{} {}", s.id, it.id);
                assert!(it.end_ms <= s.duration_ms);
                assert!(it.rect.w > 0.0 && it.rect.h > 0.0);
                if let Some(r) = &it.anim.replace_of {
                    assert!(s.items.iter().any(|o| &o.id == r));
                }
            }
        }
        // Duration buckets: hard must contain sub-second items.
        let hard = generate_suite(SuiteKind::Hard, 1, &opts);
        assert!(hard.iter().flat_map(|s| &s.items).any(|i| i.should_read && i.duration_ms() < 1000.0));
        assert!(std.iter().flat_map(|s| &s.items).any(|i| i.anim.replace_of.is_some() && i.kind == ItemKind::PickupToast));
    }

    /// Review fixes: no read item is covered by a concurrent item of another kind, every item is
    /// fully shown for at least half its time, and replaced toasts do not fade out.
    #[test]
    fn items_are_readable_and_uncovered() {
        let opts = GenOptions::procedural_only();
        for kind in [SuiteKind::Standard, SuiteKind::Hard] {
            for s in generate_suite(kind, 1, &opts) {
                for a in &s.items {
                    assert!(a.anim.fade_in_ms + a.anim.fade_out_ms <= 0.5 * a.duration_ms() + 1.0, "{} {}", s.id, a.id);
                    if s.items.iter().any(|o| o.anim.replace_of.as_deref() == Some(a.id.as_str())) {
                        assert_eq!(a.anim.fade_out_ms, 0.0, "{} {}", s.id, a.id);
                    }
                    if !a.should_read {
                        continue;
                    }
                    let same = |k: ItemKind| if k == ItemKind::Speaker { ItemKind::Dialogue } else { k };
                    for b in &s.items {
                        let concurrent = a.start_ms < b.end_ms && b.start_ms < a.end_ms;
                        if same(a.kind) != same(b.kind) && concurrent {
                            assert!(!a.rect.intersects(&b.rect), "{}: {} covered by {}", s.id, a.id, b.id);
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn round_trip_on_disk() {
        let dir = std::env::temp_dir().join(format!("arena-gen-test-{}", std::process::id()));
        let opts = GenOptions::procedural_only();
        let s = generate_suite(SuiteKind::Smoke, 5, &opts);
        write_suite(&dir, "smoke", 5, &s).unwrap();
        let (suite, back) = load_suite(&dir).unwrap();
        assert_eq!(suite.scenarios.len(), 8);
        assert_eq!(back, s);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
