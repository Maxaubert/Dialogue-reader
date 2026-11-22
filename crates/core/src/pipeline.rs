//! The A1 pipeline: zones, change detection, OCR scheduling, frame ring and dedup.
//!
//! The work is split in two steps, so a host can bound the OCR cost of one frame:
//! - [`Pipeline::observe`] (cheap, every frame): cut each zone out as grey, build its cell
//!   signature, and decide whether the zone is due for OCR: when it changed since its last OCR,
//!   when a line still waits for confirmation, or when the refresh time ran out; never more often
//!   than the kind's minimum gap. A frame that is not read goes to the zone's ring.
//! - [`Pipeline::read`]: OCR the due zones (at most `max_reads` per call, the zone that waited
//!   longest first), split each read into units (one per row in notification zones, else one per
//!   zone) and feed the zone tracker, which speaks confirmed lines once.
//!
//! [`Pipeline::on_frame`] does both, plus the no-zone detector. When a line ends before it was
//! confirmed (a short pop-up), the pipeline reads the sharpest unread ring crop of its lifetime,
//! or reads its last crop again with another clean-up, and speaks the line when the reads agree.

use std::collections::VecDeque;

use crate::detect::{FreeDetector, FreeParams};
use crate::frame::Frame;
use crate::geom::{PxBox, Rect};
use crate::image::{Channel, Gray, Prep, prepare};
use crate::ocr::{Ocr, OcrLine};
use crate::ring::Ring;
use crate::signature::{Delta, Signature};
use crate::text::{Relation, is_junk, letters, relate};
use crate::track::{Lost, Params, Tracker, Unit};
use crate::zone::{Zone, ZoneKind};

/// Timing of one zone kind.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct KindTiming {
    /// Shortest time between two OCR runs of one zone.
    pub min_gap_ms: f64,
    /// A line must be the latest read this long before it is spoken.
    pub stable_ms: f64,
    /// A line is gone after this long without a read of it (and `gone_reads` fresh misses).
    pub gone_ms: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    pub prep: Prep,
    /// Second pass: each row found by the zone read is cut out tightly and read again with this
    /// clean-up (None turns the second pass off). Tight crops let a black and white threshold
    /// work even over busy backgrounds.
    pub refine: Option<Prep>,
    /// Margin around a row for the second pass, as a share of the row height.
    pub refine_margin: f32,
    /// Read a known line again when the zone read missed it: notification zones read the row at
    /// its last place; other zones read the whole zone with a second clean-up when the read was
    /// empty.
    pub recheck: bool,
    /// Zones (and regions) at most this tall are read in one pass with `small_prep`.
    pub small_h: u32,
    /// Prompt and speaker zones (one line by nature) up to `loose` times `small_h` tall are read
    /// like a small zone, so a loosely drawn prompt zone still gets the 2x read. 1 or less turns
    /// it off.
    pub loose: f32,
    pub small_prep: Prep,
    pub small_alt: Option<Prep>,
    /// The second pass takes the whole zone width (so a row the first pass read only in part is
    /// read whole).
    pub refine_full_width: bool,
    pub channel: Channel,
    /// Signature cell size, px.
    pub cell: u32,
    /// A cell changed when its mean or edge level moved more than this (0..=255).
    pub cell_thresh: f32,
    /// A zone changed when more than this share of its cells changed.
    pub change_share: f32,
    /// Pixels count as the same as the last read when no cell moved more than this. The read is
    /// then reused instead of running OCR, and it is not new evidence.
    pub same_px: f32,
    /// OCR a zone that holds tracks at least this often, even without a pixel change.
    pub refresh_ms: f64,
    /// OCR a zone without tracks at least this often.
    pub idle_ms: f64,
    /// Ring length.
    pub ring_ms: f64,
    /// Look in the ring for a second read of lines that ended unconfirmed.
    pub rescue: bool,
    /// Speak a line that ended with `confirm` agreeing reads but before `stable_ms` passed.
    pub emit_lost: bool,
    /// A line that ended unconfirmed needs this many letters to be spoken at all.
    pub lost_min_letters: usize,
    /// A line that ended unconfirmed after one fresh read that held on still pixels this long is
    /// read again with another clean-up (a second opinion). Negative turns it off.
    pub second_ms: f64,
    pub read_prompts: bool,
    pub dialogue: KindTiming,
    pub speaker: KindTiming,
    pub notification: KindTiming,
    pub prompt: KindTiming,
    /// Zone OCR reads per [`Pipeline::read`] call (0 = no limit).
    pub max_reads: usize,
    /// Frame height the pixel sizes were tuned at. Taller frames scale `small_h`, the paddings and
    /// the cell size up by their height over this (never down).
    pub ref_h: u32,
    /// Dedup rules (`stable_ms` and `gone_ms` are replaced per kind).
    pub track: Params,
    /// Record a text trace of every OCR run (for tuning).
    pub trace: bool,
    /// No-zone mode settings (used when there are no zones).
    pub free: FreeParams,
    /// Also watch the frame outside the zones with the no-zone detector (for tooltips).
    pub free_outside: bool,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            prep: Prep { scale: 1.0, pad: 16, ..Prep::default() },
            refine: Some(Prep { scale: 1.0, stretch: true, binarize: true, dark_text: true, pad: 8 }),
            refine_margin: 0.35,
            refine_full_width: true,
            recheck: true,
            small_h: 70,
            loose: 1.5,
            small_prep: Prep { scale: 2.0, pad: 16, ..Prep::default() },
            small_alt: Some(Prep { scale: 2.0, stretch: true, binarize: true, dark_text: true, pad: 16 }),
            channel: Channel::Luma,
            cell: 8,
            cell_thresh: 10.0,
            change_share: 0.0,
            same_px: 2.0,
            refresh_ms: 400.0,
            idle_ms: 1000.0,
            ring_ms: 2500.0,
            rescue: true,
            emit_lost: true,
            lost_min_letters: 3,
            second_ms: 60.0,
            read_prompts: true,
            // A 50 ms gap: about a fifth less OCR on busy screens, fewer repeats and junk, the
            // same recall on standard, and about 35 ms more latency (standard seed 1,
            // 2026-11-22). Speaker names and prompts are small and cheap, and short names need
            // every read on busy screens, so they keep no gap.
            dialogue: KindTiming { min_gap_ms: 50.0, stable_ms: 150.0, gone_ms: 100.0 },
            speaker: KindTiming { min_gap_ms: 0.0, stable_ms: 100.0, gone_ms: 100.0 },
            notification: KindTiming { min_gap_ms: 50.0, stable_ms: 60.0, gone_ms: 300.0 },
            prompt: KindTiming { min_gap_ms: 0.0, stable_ms: 300.0, gone_ms: 150.0 },
            max_reads: 0,
            ref_h: 1080,
            track: Params::default(),
            trace: false,
            free: FreeParams::default(),
            free_outside: false,
        }
    }
}

impl Config {
    pub fn timing(&self, k: ZoneKind) -> KindTiming {
        match k {
            ZoneKind::Dialogue => self.dialogue,
            ZoneKind::Speaker => self.speaker,
            ZoneKind::Notification => self.notification,
            ZoneKind::Prompt | ZoneKind::Ignore => self.prompt,
        }
    }

    /// The pixel sizes for a frame `h` pixels tall (scaled up from `ref_h`, never down).
    pub fn for_height(&self, h: u32) -> Config {
        let k = (h as f32 / self.ref_h.max(1) as f32).max(1.0);
        if k <= 1.0 {
            return self.clone();
        }
        let px = |v: u32| (v as f32 * k).round() as u32;
        let pad = |p: Prep| Prep { pad: px(p.pad), ..p };
        Config {
            prep: pad(self.prep),
            refine: self.refine.map(pad),
            small_prep: pad(self.small_prep),
            small_alt: self.small_alt.map(pad),
            small_h: px(self.small_h),
            cell: px(self.cell),
            ..self.clone()
        }
    }
}

/// "Speak this now."
#[derive(Clone, Debug, PartialEq)]
pub struct Event {
    pub t_ms: f64,
    pub text: String,
    pub rect: Option<Rect>,
    /// Zone name (None in no-zone mode).
    pub zone: Option<String>,
    /// Zone kind (None in no-zone mode).
    pub kind: Option<ZoneKind>,
}

/// What one call gives back. An OCR error in one zone does not drop the events of the others.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Output {
    pub events: Vec<Event>,
    /// OCR errors, one per failed zone read ("zone name: error").
    pub errors: Vec<String>,
}

struct Snap {
    gray: Gray,
    sharp: f32,
}

/// A frame of a zone that is due for OCR but not read yet.
struct Todo {
    t: f64,
    gray: Gray,
    sig: Signature,
    delta: Option<Delta>,
    same: bool,
}

/// Crops of the last fresh OCR reads kept per zone (for second opinions).
const HIST: usize = 6;
/// The trace stops growing at this many lines.
const TRACE_MAX: usize = 200_000;

struct ZoneState {
    zone: Zone,
    px: Option<PxBox>,
    sig_ocr: Option<Signature>,
    last_ocr: f64,
    /// Recent frames that were not read (and differ from the last read).
    ring: Ring<Snap>,
    /// Crops of the last fresh OCR reads, with their time.
    hist: VecDeque<(f64, Gray)>,
    tracker: Tracker,
    /// Units of the last OCR read, fed again when the pixels did not change.
    last_units: Vec<Unit>,
    todo: Option<Todo>,
    /// The last fresh read found nothing: when the pixels stay still for `SETTLE_MS`, read once
    /// more with the other clean-up (text that WinRT misses with one clean-up is often read by
    /// the other).
    settle: bool,
    /// Time of the last fresh OCR read.
    last_fresh: f64,
}

/// Still time before the settle read. Busy and video backgrounds never stay still this long, so
/// the extra read happens only on calm screens.
const SETTLE_MS: f64 = 150.0;

pub struct Pipeline {
    /// Settings as given.
    base: Config,
    /// Settings for the current frame size.
    cfg: Config,
    zones: Vec<ZoneState>,
    /// Every zone given, ignore zones too (the no-zone detector never looks inside them).
    all_zones: Vec<Zone>,
    free: Option<FreeDetector>,
    frame_size: Option<(u32, u32)>,
    /// Zones with no pixels in the current frame (empty, off the frame or not a number).
    empty: Vec<String>,
    /// OCR runs so far.
    pub ocr_runs: u64,
    pub trace: Vec<String>,
}

/// Groups OCR lines into rows (lines that share most of their height), top to bottom, each row
/// left to right.
pub fn rows(lines: &[OcrLine]) -> Vec<Vec<OcrLine>> {
    let mut sorted: Vec<OcrLine> = lines.iter().filter(|l| !l.text.trim().is_empty()).cloned().collect();
    sorted.sort_by(|a, b| a.rect.y.total_cmp(&b.rect.y));
    let mut rows: Vec<Vec<OcrLine>> = Vec::new();
    for l in sorted {
        match rows.iter_mut().find(|r| r.iter().any(|o| o.rect.v_overlap(&l.rect) > 0.5)) {
            Some(r) => r.push(l),
            None => rows.push(vec![l]),
        }
    }
    for r in &mut rows {
        r.sort_by(|a, b| a.rect.x.total_cmp(&b.rect.x));
    }
    rows
}

/// Turns one OCR read of a zone into units.
pub fn units_of(kind: ZoneKind, lines: &[OcrLine]) -> Vec<Unit> {
    let rws = rows(lines);
    let row_text = |r: &Vec<OcrLine>| r.iter().map(|l| l.text.trim()).collect::<Vec<_>>().join(" ");
    let row_rect = |r: &Vec<OcrLine>| Rect::union_all(r.iter().map(|l| &l.rect));
    if kind.joins_lines() {
        if rws.is_empty() {
            return Vec::new();
        }
        let mut text = rws.iter().map(row_text).collect::<Vec<_>>().join("\n");
        if kind == ZoneKind::Speaker {
            text = trim_edge_marks(&text);
        }
        let rect = Rect::union_all(lines.iter().map(|l| &l.rect));
        vec![Unit::new(&text, rect)]
    } else {
        rws.iter().map(|r| Unit::new(&row_text(r), row_rect(r))).collect()
    }
}

/// Drops one-character tokens at the start or end that look like a panel border read as a mark
/// ("I Juno", "Juno |"), when a real word stays. Digits are never dropped, and a letter only at
/// the start, so "Player 1" and "Henry I" stay whole.
pub fn trim_edge_marks(s: &str) -> String {
    let mut toks: Vec<&str> = s.split_whitespace().collect();
    let marks = "|![]()/\\-_.,'\"`~:;";
    let start = |t: &str| t.chars().count() == 1 && (marks.contains(t) || t == "I" || t == "l");
    let end = |t: &str| t.chars().count() == 1 && marks.contains(t);
    while toks.len() > 1 && start(toks[0]) {
        toks.remove(0);
    }
    while toks.len() > 1 && end(toks[toks.len() - 1]) {
        toks.pop();
    }
    toks.join(" ")
}

/// Runs OCR on a grey crop whose top-left corner sits at `origin` in the frame. Line rects come
/// back in frame pixels.
pub fn ocr_gray(ocr: &mut dyn Ocr, g: &Gray, prep: &Prep, origin: (f32, f32)) -> Result<Vec<OcrLine>, String> {
    let (img, back, pad) = prepare(g, prep, ocr.max_dim());
    let lines = ocr.recognize(img.w, img.h, &img.to_rgba())?;
    Ok(lines
        .into_iter()
        .map(|l| OcrLine {
            text: l.text,
            rect: Rect::new(origin.0 + l.rect.x * back - pad, origin.1 + l.rect.y * back - pad, l.rect.w * back, l.rect.h * back),
        })
        .collect())
}

/// The crop is read in one pass as one line (`small_h`, or `loose` times that for one-line kinds).
fn small_path(g: &Gray, cfg: &Config, one_line: bool) -> bool {
    let limit = if one_line { cfg.small_h as f32 * cfg.loose.max(1.0) } else { cfg.small_h as f32 };
    g.h as f32 <= limit
}

fn useful(lines: &[OcrLine], cfg: &Config) -> bool {
    lines.iter().any(|l| !is_junk(&l.text, cfg.track.min_letters))
}

/// One pass with the small-zone clean-up, and a second one with `small_alt` when the first reads
/// nothing useful.
fn read_small(ocr: &mut dyn Ocr, g: &Gray, cfg: &Config, origin: (f32, f32)) -> Result<(Vec<OcrLine>, u64), String> {
    let lines = ocr_gray(ocr, g, &cfg.small_prep, origin)?;
    match &cfg.small_alt {
        Some(alt) if !useful(&lines, cfg) => Ok((ocr_gray(ocr, g, alt, origin)?, 2)),
        _ => Ok((lines, 1)),
    }
}

/// Reads a zone crop: one OCR pass over the whole crop, then (with `cfg.refine`) a second pass
/// per row on a tight crop. A row keeps its first-pass text when the second pass reads nothing
/// useful. `one_line` zones (prompts, speaker names) up to `loose` times `small_h` are read like
/// small zones. Returns the lines (one per row after refining) and the number of OCR runs.
pub fn read_zone(ocr: &mut dyn Ocr, g: &Gray, cfg: &Config, origin: (f32, f32), one_line: bool) -> Result<(Vec<OcrLine>, u64), String> {
    if small_path(g, cfg, one_line) {
        // A one-line zone is already a tight crop.
        return read_small(ocr, g, cfg, origin);
    }
    let lines = ocr_gray(ocr, g, &cfg.prep, origin)?;
    let mut runs = 1;
    let Some(rp) = &cfg.refine else { return Ok((lines, runs)) };
    let mut out = Vec::new();
    for row in rows(&lines) {
        let Some(r) = Rect::union_all(row.iter().map(|l| &l.rect)) else { continue };
        let first = row.iter().map(|l| l.text.trim()).collect::<Vec<_>>().join(" ");
        let again = read_row(ocr, g, cfg, rp, origin, &r)?;
        runs += u64::from(again.is_some());
        let text = again.filter(|t| !is_junk(t, cfg.track.min_letters)).unwrap_or(first);
        out.push(OcrLine { text, rect: r });
    }
    Ok((out, runs))
}

/// A second opinion on a crop with another clean-up: tall zones are read whole with the row
/// clean-up (`refine`), small zones with `small_alt`.
pub fn read_alt(ocr: &mut dyn Ocr, g: &Gray, cfg: &Config, origin: (f32, f32), one_line: bool) -> Result<Vec<OcrLine>, String> {
    let bw = |p: Prep| Prep { stretch: true, binarize: true, dark_text: true, ..p };
    let p = if small_path(g, cfg, one_line) { cfg.small_alt.unwrap_or(bw(cfg.small_prep)) } else { cfg.refine.unwrap_or(bw(cfg.prep)) };
    ocr_gray(ocr, g, &p, origin)
}

/// Second-pass read of one row `r` (frame pixels) of a zone crop `g` whose corner is at
/// `origin`. None when the row box is empty.
pub fn read_row(ocr: &mut dyn Ocr, g: &Gray, cfg: &Config, rp: &Prep, origin: (f32, f32), r: &Rect) -> Result<Option<String>, String> {
    let m = r.h * cfg.refine_margin;
    let local = if cfg.refine_full_width {
        Rect::new(0.0, r.y - origin.1 - m, g.w as f32, r.h + m * 2.0)
    } else {
        Rect::new(r.x - origin.0 - m * 1.5, r.y - origin.1 - m, r.w + m * 3.0, r.h + m * 2.0)
    };
    match local.to_px(g.w, g.h) {
        Some(b) if b.w() >= 4 && b.h() >= 4 => {
            let sub = g.sub(b);
            let again = ocr_gray(ocr, &sub, rp, (origin.0 + b.x0 as f32, origin.1 + b.y0 as f32))?;
            Ok(Some(rows(&again).iter().map(|r| r.iter().map(|l| l.text.trim()).collect::<Vec<_>>().join(" ")).collect::<Vec<_>>().join(" ")))
        }
        _ => Ok(None),
    }
}

fn log(trace: &mut Vec<String>, on: bool, line: impl FnOnce() -> String) {
    if on && trace.len() < TRACE_MAX {
        trace.push(line());
    }
}

impl Pipeline {
    pub fn new(cfg: Config, zones: &[Zone]) -> Pipeline {
        let mut p = Pipeline { cfg: cfg.clone(), base: cfg, zones: Vec::new(), all_zones: Vec::new(), free: None, frame_size: None, empty: Vec::new(), ocr_runs: 0, trace: Vec::new() };
        p.set_zones(zones);
        p
    }

    /// The settings as given (before scaling to the frame size).
    pub fn config(&self) -> &Config {
        &self.base
    }

    /// Zones that have no pixels in the current frame (empty, off the frame or not a number).
    /// They are never read.
    pub fn empty_zones(&self) -> &[String] {
        &self.empty
    }

    /// Replaces the zones (state is reset). With no zones the whole frame is watched (no-zone mode).
    pub fn set_zones(&mut self, zones: &[Zone]) {
        self.all_zones = zones.to_vec();
        let cfg = &self.base;
        self.zones = zones
            .iter()
            .filter(|z| z.kind != ZoneKind::Ignore && (z.kind != ZoneKind::Prompt || cfg.read_prompts))
            .map(|z| ZoneState {
                zone: z.clone(),
                px: None,
                sig_ocr: None,
                last_ocr: f64::NEG_INFINITY,
                ring: Ring::new(cfg.ring_ms, 200),
                hist: VecDeque::new(),
                tracker: Tracker::new(Params {
                    stable_ms: cfg.timing(z.kind).stable_ms,
                    gone_ms: cfg.timing(z.kind).gone_ms,
                    grow_events: cfg.track.grow_events && z.kind.joins_lines() && z.kind != ZoneKind::Prompt,
                    ..cfg.track.clone()
                }),
                last_units: Vec::new(),
                todo: None,
                settle: false,
                last_fresh: f64::NEG_INFINITY,
            })
            .collect();
        // No zone to read (none drawn, or only "ignore" ones): watch the whole frame instead.
        let no_read_zones = self.zones.is_empty();
        self.free = (no_read_zones || cfg.free_outside).then(|| {
            let mut d = FreeDetector::new(cfg.free.clone(), cfg.track.clone());
            d.mask = zones.iter().map(|z| z.rect).collect();
            d
        });
        self.frame_size = None;
    }

    fn fit_to(&mut self, f: &Frame) {
        if self.frame_size == Some((f.w, f.h)) {
            return;
        }
        self.frame_size = Some((f.w, f.h));
        self.cfg = self.base.for_height(f.h);
        self.empty.clear();
        for z in &mut self.zones {
            z.px = z.zone.rect_in(f.w, f.h).to_px(f.w, f.h);
            if z.px.is_none() {
                self.empty.push(z.zone.name.clone());
            }
            z.sig_ocr = None;
            z.ring.clear();
            z.hist.clear();
            z.todo = None;
        }
        if let Some(free) = &mut self.free {
            free.mask = self.all_zones.iter().map(|z| z.rect_in(f.w, f.h)).collect();
        }
    }

    /// Processes one frame: the no-zone detector, [`Pipeline::observe`], then [`Pipeline::read`]
    /// with the `max_reads` budget.
    pub fn on_frame(&mut self, f: &Frame, ocr: &mut dyn Ocr) -> Output {
        self.fit_to(f);
        let mut out = Output::default();
        if let Some(free) = &mut self.free {
            let (ev, runs, err) = free.on_frame(f, ocr, &self.cfg, if self.cfg.trace { Some(&mut self.trace) } else { None });
            self.ocr_runs += runs;
            out.events.extend(ev);
            if let Some(e) = err {
                out.errors.push(format!("no-zone mode: {e}"));
            }
        }
        self.observe(f);
        let r = self.read(ocr, self.cfg.max_reads);
        out.events.extend(r.events);
        out.errors.extend(r.errors);
        out
    }

    /// The cheap step for every frame: crops, signatures and the ring. It runs no OCR. Zones that
    /// are due keep this frame until [`Pipeline::read`]; a newer frame replaces it.
    pub fn observe(&mut self, f: &Frame) {
        self.fit_to(f);
        let t = f.t_ms;
        let cfg = &self.cfg;
        for z in &mut self.zones {
            let Some(px) = z.px else { continue };
            let timing = cfg.timing(z.zone.kind);
            let g = Gray::crop(f.rgba, f.w, px, cfg.channel);
            let sig = Signature::of(&g, cfg.cell);
            let delta = z.sig_ocr.as_ref().map(|s| sig.delta(s, cfg.cell_thresh));
            let changed = delta.is_none_or(|d| d.share > cfg.change_share);
            let same = delta.is_some_and(|d| d.max <= cfg.same_px);
            if same {
                // The lines of the last read are still on screen.
                z.tracker.touch(t);
            }
            let since = t - z.last_ocr;
            let wait = if z.tracker.has_tracks() { cfg.refresh_ms } else { cfg.idle_ms };
            let due = since >= timing.min_gap_ms - 1e-6 && (changed || z.tracker.pending() || since >= wait - 1e-6 || (same && z.settle && t - z.last_fresh >= SETTLE_MS - 1e-6));
            if due {
                let old = z.todo.replace(Todo { t, gray: g, sig, delta, same });
                if let Some(old) = old
                    && cfg.rescue
                    && !old.same
                {
                    let sharp = old.gray.sharpness();
                    z.ring.push(old.t, Snap { gray: old.gray, sharp });
                }
            } else if cfg.rescue && !same {
                let sharp = g.sharpness();
                z.ring.push(t, Snap { gray: g, sharp });
            }
        }
    }

    /// OCRs the zones that are due, at most `max_reads` OCR reads (0 = all), the zone that waited
    /// longest first. Reuses of the last read (same pixels) cost nothing and are not counted.
    pub fn read(&mut self, ocr: &mut dyn Ocr, max_reads: usize) -> Output {
        let mut order: Vec<usize> = (0..self.zones.len()).filter(|&i| self.zones[i].todo.is_some()).collect();
        order.sort_by(|&a, &b| {
            let (za, zb) = (&self.zones[a], &self.zones[b]);
            let sa = za.todo.as_ref().is_some_and(|t| t.same);
            let sb = zb.todo.as_ref().is_some_and(|t| t.same);
            sb.cmp(&sa).then(za.last_ocr.total_cmp(&zb.last_ocr))
        });
        let mut out = Output::default();
        let mut reads = 0;
        for zi in order {
            let same = self.zones[zi].todo.as_ref().is_some_and(|t| t.same);
            if !same {
                if max_reads > 0 && reads >= max_reads {
                    continue;
                }
                reads += 1;
            }
            let Some(todo) = self.zones[zi].todo.take() else { continue };
            let (ev, err) = self.read_one(zi, todo, ocr);
            out.events.extend(ev);
            if let Some(e) = err {
                out.errors.push(format!("{}: {e}", self.zones[zi].zone.name));
            }
        }
        out
    }

    /// OCR of the zone crop (unless the pixels are the same), the rechecks, and the tracker.
    fn zone_units(&mut self, zi: usize, g: &Gray, ocr: &mut dyn Ocr) -> Result<Vec<Unit>, String> {
        let cfg = &self.cfg;
        let z = &self.zones[zi];
        let Some(px) = z.px else { return Ok(Vec::new()) };
        let origin = (px.x0 as f32, px.y0 as f32);
        let (lines, runs) = read_zone(ocr, g, cfg, origin, z.zone.kind.one_line())?;
        self.ocr_runs += runs;
        let mut units = units_of(z.zone.kind, &lines);
        let jit = cfg.track.jitter;
        if !cfg.recheck {
            return Ok(units);
        }
        if z.zone.kind == ZoneKind::Notification
            && let Some(rp) = &cfg.refine
        {
            // Rows the first pass missed: read each known line again at its last place.
            let missing: Vec<(Rect, String)> = z
                .tracker
                .tracks
                .iter()
                .filter_map(|tr| tr.rect.map(|r| (r, tr.current().norm.clone())))
                .filter(|(_, n)| units.iter().all(|u| relate(&u.norm, n, jit) == Relation::Different))
                .collect();
            for (r, norm) in missing.into_iter().take(3) {
                if let Some(text) = read_row(ocr, g, cfg, rp, origin, &r)? {
                    self.ocr_runs += 1;
                    let u = Unit::new(&text, Some(r));
                    if !is_junk(&text, cfg.track.min_letters) && relate(&u.norm, &norm, jit) != Relation::Different {
                        units.push(u);
                    }
                }
            }
        } else if !small_path(g, cfg, z.zone.kind.one_line())
            && units.iter().all(|u| is_junk(&u.raw, cfg.track.min_letters))
            && z.tracker.tracks.iter().any(|tr| tr.streak > 0)
        {
            // A line that was read a moment ago and now reads as nothing: a second clean-up
            // before calling it gone (a blinking arrow or a flash can spoil one read). Small
            // zones already tried their second clean-up.
            let lines = read_alt(ocr, g, cfg, origin, z.zone.kind.one_line())?;
            self.ocr_runs += 1;
            let alt = units_of(z.zone.kind, &lines);
            let known = alt.iter().any(|u| !is_junk(&u.raw, cfg.track.min_letters) && z.tracker.tracks.iter().any(|tr| relate(&u.norm, &tr.current().norm, jit) != Relation::Different));
            if known {
                units = alt;
            }
        }
        Ok(units)
    }

    fn read_one(&mut self, zi: usize, todo: Todo, ocr: &mut dyn Ocr) -> (Vec<Event>, Option<String>) {
        let Todo { t, gray, sig, delta, same } = todo;
        let units = if same && self.zones[zi].settle && t - self.zones[zi].last_fresh >= SETTLE_MS - 1e-6 {
            // Still pixels after a read that found nothing: one read with the other clean-up.
            // Tall zones only (small zones tried both clean-ups already). The same pixels are no
            // new evidence, so a line found here is trusted after the still-screen time.
            let z = &mut self.zones[zi];
            z.settle = false;
            let one_line = z.zone.kind.one_line();
            if small_path(&gray, &self.cfg, one_line) {
                z.last_units.clone()
            } else {
                let origin = z.px.map_or((0.0, 0.0), |p| (p.x0 as f32, p.y0 as f32));
                match read_alt(ocr, &gray, &self.cfg, origin, one_line) {
                    Ok(lines) => {
                        self.ocr_runs += 1;
                        let u = units_of(z.zone.kind, &lines);
                        z.last_units = u.clone();
                        u
                    }
                    Err(e) => return (Vec::new(), Some(e)),
                }
            }
        } else if same {
            self.zones[zi].last_units.clone()
        } else {
            match self.zone_units(zi, &gray, ocr) {
                Ok(u) => u,
                // The zone stays unread: it is due again on the next frame.
                Err(e) => return (Vec::new(), Some(e)),
            }
        };
        let on = self.cfg.trace;
        let z = &mut self.zones[zi];
        let (speak, lost) = z.tracker.update(t, &units, !same);
        log(&mut self.trace, on, || {
            let d = delta.map_or("new".to_string(), |d| format!("max {:.0} share {:.2}", d.max, d.share));
            let u: Vec<&str> = units.iter().map(|u| u.raw.as_str()).collect();
            // Row heights, for replaying a trace into the tracker.
            let y: Vec<String> = units.iter().map(|u| u.rect.map_or("-".into(), |r| format!("{:.0}+{:.0}", r.y, r.h))).collect();
            format!("{t:.0} {} {} [{d}] {u:?} {y:?}", z.zone.name, if same { "same" } else { "ocr" })
        });
        if !same {
            z.settle = units.iter().all(|u| is_junk(&u.raw, self.cfg.track.min_letters));
            z.last_fresh = t;
            z.sig_ocr = Some(sig);
            z.last_units = units;
            z.hist.push_back((t, gray));
            while z.hist.len() > HIST {
                z.hist.pop_front();
            }
        }
        z.last_ocr = t;
        let (name, kind) = (z.zone.name.clone(), z.zone.kind);
        let mut out = Vec::new();
        for s in speak {
            log(&mut self.trace, on, || format!("{t:.0} {name} SPEAK {:?}", s.raw));
            out.push(Event { t_ms: t, text: s.raw, rect: s.rect, zone: Some(name.clone()), kind: Some(kind) });
        }
        let mut err = None;
        for l in lost {
            match self.recover(zi, t, l, ocr) {
                Ok(Some(e)) => out.push(e),
                Ok(None) => {}
                Err(e) => err = Some(e),
            }
        }
        (out, err)
    }

    /// A line ended unconfirmed. Speak it when it already had enough agreeing reads
    /// (`emit_lost`), or when a second read agrees: an unread ring frame from its lifetime, or
    /// (one fresh read on still pixels) its last crop read with another clean-up.
    fn recover(&mut self, zi: usize, t: f64, l: Lost, ocr: &mut dyn Ocr) -> Result<Option<Event>, String> {
        let cfg = &self.cfg;
        let on = cfg.trace;
        let z = &mut self.zones[zi];
        let tr = &l.track;
        // Prompts flicker: only the debounce in the tracker may speak them.
        if z.zone.kind == ZoneKind::Prompt {
            return Ok(None);
        }
        let agreed = l.agreed(&z.tracker.params).cloned();
        let best = agreed.clone().unwrap_or_else(|| tr.best().clone());
        // Too short to trust, or the start of a line that is still on screen ("Go" of "Go home").
        if is_junk(&best.raw, cfg.track.min_letters) || letters(&best.norm) < cfg.lost_min_letters || z.tracker.grows_from(&best.norm) {
            return Ok(None);
        }
        let Some(px) = z.px else { return Ok(None) };
        let origin = (px.x0 as f32, px.y0 as f32);
        // A second read must agree closely (half the usual jitter): nothing else checks it, and
        // garbled reads of a busy screen agree roughly all the time.
        let agrees = |units: &[Unit]| -> Option<String> {
            units.iter().find(|u| relate(&u.norm, &best.norm, cfg.track.jitter * 0.5) == Relation::Same).map(|u| {
                if u.norm.chars().count() >= best.norm.chars().count() { u.raw.clone() } else { best.raw.clone() }
            })
        };
        let mut speak: Option<String> = None;
        let mut why = "";
        if cfg.emit_lost && agreed.is_some() {
            speak = Some(best.raw.clone());
            why = "agreeing reads";
        } else {
            let ring_pick = if cfg.rescue { z.ring.best_in(tr.first_seen - 150.0, tr.last_seen + 150.0, |s| s.sharp) } else { None };
            if let Some((ts, snap)) = ring_pick {
                let ts = *ts;
                let (lines, runs) = read_zone(ocr, &snap.gray, cfg, origin, z.zone.kind.one_line())?;
                self.ocr_runs += runs;
                let units = units_of(z.zone.kind, &lines);
                if let Some(s) = agrees(&units) {
                    speak = Some(s);
                    why = "ring read agrees";
                }
                log(&mut self.trace, on, || {
                    let u: Vec<&str> = units.iter().map(|u| u.raw.as_str()).collect();
                    format!("{t:.0} {} RING read of {ts:.0} for {:?}: {u:?}", z.zone.name, best.raw)
                });
            }
            if speak.is_none()
                && cfg.second_ms >= 0.0
                && let Some(tf) = tr.last_fresh()
                && tr.last_seen - tf >= cfg.second_ms - 1e-6
                && let Some((_, g)) = z.hist.iter().find(|(ht, _)| (*ht - tf).abs() < 0.5)
            {
                // One read of still pixels: read the same crop again with the other clean-up.
                let lines = read_alt(ocr, g, cfg, origin, z.zone.kind.one_line())?;
                self.ocr_runs += 1;
                let units = units_of(z.zone.kind, &lines);
                if let Some(s) = agrees(&units) {
                    speak = Some(s);
                    why = "second clean-up agrees";
                }
                log(&mut self.trace, on, || {
                    let u: Vec<&str> = units.iter().map(|u| u.raw.as_str()).collect();
                    format!("{t:.0} {} SECOND read of {tf:.0} for {:?}: {u:?}", z.zone.name, best.raw)
                });
            }
        }
        let Some(text) = speak else { return Ok(None) };
        // Never repeat a line this zone already speaks or shows.
        if z.tracker.knows(&crate::text::normalize(&text)) {
            return Ok(None);
        }
        log(&mut self.trace, on, || format!("{t:.0} {} SPEAK-LOST ({why}) {text:?}", z.zone.name));
        Ok(Some(Event { t_ms: t, text, rect: best.rect, zone: Some(z.zone.name.clone()), kind: Some(z.zone.kind) }))
    }

    /// Call after the last frame: frames still waiting are read, and lines still waiting are
    /// handled like lines that ended.
    pub fn finish(&mut self, t: f64, ocr: &mut dyn Ocr) -> Output {
        let mut out = self.read(ocr, 0);
        if let Some(free) = &mut self.free {
            out.events.extend(free.finish(t, &self.cfg));
        }
        for zi in 0..self.zones.len() {
            let lost = self.zones[zi].tracker.drain();
            for l in lost {
                match self.recover(zi, t, l, ocr) {
                    Ok(Some(e)) => out.events.push(e),
                    Ok(None) => {}
                    Err(e) => out.errors.push(format!("{}: {e}", self.zones[zi].zone.name)),
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// Fake OCR: the frame encodes a "text id" in the grey level of the crop's first pixel.
    /// 0 means no text. Counts calls.
    struct Fake {
        texts: HashMap<u8, &'static str>,
        calls: usize,
    }
    impl Ocr for Fake {
        fn recognize(&mut self, w: u32, h: u32, rgba: &[u8]) -> Result<Vec<OcrLine>, String> {
            self.calls += 1;
            // Centre pixel holds the id (prep may upscale, so read the middle).
            let c = ((h / 2 * w + w / 2) * 4) as usize;
            let id = rgba[c];
            Ok(match self.texts.get(&id) {
                Some(t) => vec![OcrLine { text: t.to_string(), rect: Rect::new(0.0, 0.0, w as f32, h as f32) }],
                None => vec![],
            })
        }
    }

    fn frame_with(level: u8) -> Vec<u8> {
        // 64x32 frame; the zone is [16, 8, 32, 16]; it holds `level` with white "strokes" in
        // every 4th column (not the centre one), the rest is 0.
        let mut v = vec![0u8; 64 * 32 * 4];
        for y in 8..24 {
            for x in 16..48 {
                let i = (y * 64 + x) * 4;
                let p = if level > 0 && (x - 16) % 4 == 1 { 255 } else { level };
                v[i..i + 4].copy_from_slice(&[p, p, p, 255]);
            }
        }
        v
    }

    fn cfg() -> Config {
        Config { prep: Prep { scale: 1.0, ..Default::default() }, ..Default::default() }
    }

    fn zone(kind: ZoneKind) -> Zone {
        Zone::new("z", kind, Rect::new(16.0, 8.0, 32.0, 16.0))
    }

    fn run(p: &mut Pipeline, ocr: &mut Fake, script: &[u8]) -> Vec<(f64, String)> {
        let mut out = Vec::new();
        let mut last = 0.0;
        for (i, &lvl) in script.iter().enumerate() {
            let px = frame_with(lvl);
            let t = i as f64 * 1000.0 / 30.0;
            last = t;
            for e in p.on_frame(&Frame::new(t, 64, 32, &px), ocr).events {
                out.push((e.t_ms, e.text));
            }
        }
        for e in p.finish(last, ocr).events {
            out.push((e.t_ms, e.text));
        }
        out
    }

    fn fake() -> Fake {
        Fake { texts: HashMap::from([(100, "Hello there"), (150, "Hello there"), (200, "Good bye now")]), calls: 0 }
    }

    #[test]
    fn unchanged_zone_is_not_read_again() {
        let mut p = Pipeline::new(Config { rescue: false, ..cfg() }, &[zone(ZoneKind::Dialogue)]);
        let mut o = fake();
        let said = run(&mut p, &mut o, &[100; 60]);
        assert_eq!(said.len(), 1);
        assert_eq!(said[0].1, "Hello there");
        // 2 s of a still zone: reads until confirmed and stable, then one refresh per 400 ms.
        assert!(o.calls <= 12, "{} calls", o.calls);
    }

    #[test]
    fn change_triggers_a_read_and_a_new_line() {
        let mut p = Pipeline::new(cfg(), &[zone(ZoneKind::Dialogue)]);
        let mut o = fake();
        let mut script = vec![100u8; 20];
        script.extend([0u8; 10]);
        script.extend([200u8; 20]);
        let said = run(&mut p, &mut o, &script);
        assert_eq!(said.iter().map(|s| s.1.as_str()).collect::<Vec<_>>(), vec!["Hello there", "Good bye now"]);
        // The second line is spoken a few frames after it appeared (frame 30 = 1000 ms).
        assert!(said[1].0 >= 1000.0 && said[1].0 < 1300.0, "{said:?}");
    }

    #[test]
    fn short_popup_is_rescued_from_the_ring() {
        // One frame of text only: the tracker sees one read, the ring has no unread frame, so
        // nothing can confirm it. Two frames with the OCR forced to skip one (min gap) give the
        // ring a second frame to agree with.
        let cfg = Config { notification: KindTiming { min_gap_ms: 60.0, stable_ms: 60.0, gone_ms: 60.0 }, emit_lost: false, ..cfg() };
        let mut p = Pipeline::new(cfg, &[zone(ZoneKind::Notification)]);
        let mut o = fake();
        let mut script = vec![0u8; 5];
        script.extend([100u8, 150]);
        script.extend([0u8; 10]);
        let said = run(&mut p, &mut o, &script);
        assert_eq!(said.iter().map(|s| s.1.as_str()).collect::<Vec<_>>(), vec!["Hello there"]);
    }

    #[test]
    fn speaker_border_marks_are_trimmed() {
        assert_eq!(trim_edge_marks("I Juno"), "Juno");
        assert_eq!(trim_edge_marks("Elder Wynn |"), "Elder Wynn");
        assert_eq!(trim_edge_marks("I"), "I");
        let l = OcrLine { text: "I Kael".into(), rect: Rect::new(0.0, 0.0, 5.0, 5.0) };
        assert_eq!(units_of(ZoneKind::Speaker, std::slice::from_ref(&l))[0].raw, "Kael");
        assert_eq!(units_of(ZoneKind::Dialogue, &[l])[0].raw, "I Kael");
    }

    #[test]
    fn the_no_zone_mode_never_reads_inside_ignore_zones() {
        // Only an ignore zone: the whole frame is watched, but not the ignore zone.
        let mut p = Pipeline::new(cfg(), &[zone(ZoneKind::Ignore)]);
        let mut o = fake();
        let mut script = vec![0u8; 5];
        script.extend([100u8; 30]);
        assert!(run(&mut p, &mut o, &script).is_empty());
        assert_eq!(o.calls, 0);
        assert_eq!(p.free.as_ref().map(|f| f.mask.len()), Some(1));
    }

    #[test]
    fn ignore_zones_are_never_read() {
        let mut p = Pipeline::new(cfg(), &[zone(ZoneKind::Ignore)]);
        let mut o = fake();
        assert!(run(&mut p, &mut o, &[100; 20]).is_empty());
        assert_eq!(o.calls, 0);
    }

    #[test]
    fn rows_split_in_notification_zones_and_join_in_dialogue_zones() {
        let l = |t: &str, y: f32, x: f32| OcrLine { text: t.into(), rect: Rect::new(x, y, 50.0, 10.0) };
        let lines = [l("second", 20.0, 0.0), l("(9)", 1.0, 60.0), l("+ 9 Leaves", 0.0, 0.0)];
        let n = units_of(ZoneKind::Notification, &lines);
        assert_eq!(n.iter().map(|u| u.raw.as_str()).collect::<Vec<_>>(), vec!["+ 9 Leaves (9)", "second"]);
        let d = units_of(ZoneKind::Dialogue, &lines);
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].raw, "+ 9 Leaves (9)\nsecond");
    }
}
