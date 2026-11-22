//! Manual mode: the user presses a key, and every zone is read once.
//!
//! Nothing is read on its own. No OCR runs between presses, and the frame outside the zones is
//! never looked at (there is no no-zone detector here). The only per-frame work is the ring of
//! recent zone crops used to pick the clearest frame at a press (`look_ms`, 0 turns it off).
//!
//! A press ([`Manual::read_now`], or [`Manual::press`] and the next [`Manual::on_frame`]):
//! 1. Takes the zones that can be read (not Ignore; prompts only with `read_prompts`), top to
//!    bottom, then left to right. A speaker zone comes before the first dialogue zone.
//! 2. Reads each zone of the current frame with the A1 clean-up (`read_zone`: two-pass rows for
//!    tall zones, the 2x small-zone path with its second try), and with the black and white
//!    clean-up when that reads nothing.
//! 3. When the sharpest ring crop of the last `look_ms` is clearly sharper than the current one
//!    (a toast that is fading out), that crop is read too. A ring line replaces a weaker read of
//!    the same line, fills an empty zone, and in toast zones adds rows that already faded.
//! 4. Returns every line found, also lines spoken before. A line is given once per press.
//!
//! Empty press: when no zone holds text (and no zone failed), the lines of the last press that
//! found text are given again, flagged as a repeat ("say that again"). With no such press the
//! result is empty.

use crate::frame::Frame;
use crate::geom::PxBox;
use crate::image::Gray;
use crate::ocr::Ocr;
use crate::pipeline::{Config, Event, read_alt, read_zone, small_path, units_of};
use crate::ring::Ring;
use crate::text::{Relation, is_junk, relate};
use crate::track::Unit;
use crate::zone::{Zone, ZoneKind};

#[derive(Clone, Debug, PartialEq)]
pub struct ManualConfig {
    /// How far back the ring looks for a clearer crop at a press (ms). 0 turns the ring off, so
    /// frames between presses cost nothing at all.
    pub look_ms: f64,
    /// At most one ring crop per zone per this many ms.
    pub step_ms: f64,
    /// A ring crop is read too when its sharpness is at least this times the current crop's.
    pub fade_ratio: f32,
}

impl Default for ManualConfig {
    fn default() -> Self {
        ManualConfig { look_ms: 500.0, step_ms: 40.0, fade_ratio: 1.25 }
    }
}

/// What one press gives back.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Press {
    pub t_ms: f64,
    /// Lines in zone order (see the module text).
    pub events: Vec<Event>,
    /// OCR errors, one per failed zone read ("zone name: error").
    pub errors: Vec<String>,
    /// The zones were empty, and `events` are the lines of the last press that found text.
    pub repeat: bool,
}

struct MZone {
    zone: Zone,
    px: Option<PxBox>,
    ring: Ring<Gray>,
    last_push: f64,
}

pub struct Manual {
    base: Config,
    cfg: Config,
    mcfg: ManualConfig,
    zones: Vec<MZone>,
    frame_size: Option<(u32, u32)>,
    empty: Vec<String>,
    pending: bool,
    last: Vec<Event>,
    /// OCR runs so far.
    pub ocr_runs: u64,
}

impl Manual {
    pub fn new(cfg: Config, mcfg: ManualConfig, zones: &[Zone]) -> Manual {
        let mut m = Manual { cfg: cfg.clone(), base: cfg, mcfg, zones: Vec::new(), frame_size: None, empty: Vec::new(), pending: false, last: Vec::new(), ocr_runs: 0 };
        m.set_zones(zones);
        m
    }

    /// Replaces the zones. Ignore zones (and prompt zones without `read_prompts`) are dropped:
    /// they are never read. The press history is kept.
    pub fn set_zones(&mut self, zones: &[Zone]) {
        let (look, step) = (self.mcfg.look_ms.max(0.0), self.mcfg.step_ms.max(1.0));
        let cap = (look / step).ceil() as usize + 2;
        self.zones = zones
            .iter()
            .filter(|z| z.kind != ZoneKind::Ignore && (z.kind != ZoneKind::Prompt || self.base.read_prompts))
            .map(|z| MZone { zone: z.clone(), px: None, ring: Ring::new(look + step, cap), last_push: f64::NEG_INFINITY })
            .collect();
        self.frame_size = None;
    }

    /// Zones with no pixels in the current frame. They are never read.
    pub fn empty_zones(&self) -> &[String] {
        &self.empty
    }

    /// The lines of the last press that found text (what an empty press repeats).
    pub fn last(&self) -> &[Event] {
        &self.last
    }

    /// Asks for a read on the next [`Manual::on_frame`] (for a hotkey thread).
    pub fn press(&mut self) {
        self.pending = true;
    }

    pub fn pending(&self) -> bool {
        self.pending
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
            z.ring.clear();
            z.last_push = f64::NEG_INFINITY;
        }
    }

    /// The per-frame upkeep: one grey crop per zone every `step_ms` into the ring. No OCR.
    pub fn observe(&mut self, f: &Frame) {
        if self.mcfg.look_ms <= 0.0 {
            return;
        }
        self.fit_to(f);
        let step = self.mcfg.step_ms;
        for z in &mut self.zones {
            let Some(px) = z.px else { continue };
            if f.t_ms - z.last_push >= step - 1e-6 {
                z.ring.push(f.t_ms, Gray::crop(f.rgba, f.w, px, self.cfg.channel));
                z.last_push = f.t_ms;
            }
        }
    }

    /// Every frame: reads the zones when a press waits, then keeps the ring. None without a press.
    pub fn on_frame(&mut self, f: &Frame, ocr: &mut dyn Ocr) -> Option<Press> {
        let out = self.pending.then(|| self.read_now(f, ocr));
        self.observe(f);
        out
    }

    /// Zone indices in reading order: rows top to bottom (zones that share most of their height
    /// are one row), each row left to right; speaker zones move before the first dialogue zone.
    fn order(&self) -> Vec<usize> {
        let mut idx: Vec<usize> = (0..self.zones.len()).filter(|&i| self.zones[i].px.is_some()).collect();
        let r = |i: usize| self.zones[i].px.map(|p| p.rect()).unwrap_or_default();
        idx.sort_by(|&a, &b| r(a).y.total_cmp(&r(b).y));
        let mut rows: Vec<Vec<usize>> = Vec::new();
        for i in idx {
            match rows.iter_mut().find(|row| row.iter().any(|&o| r(o).v_overlap(&r(i)) > 0.5)) {
                Some(row) => row.push(i),
                None => rows.push(vec![i]),
            }
        }
        let mut order: Vec<usize> = rows
            .into_iter()
            .flat_map(|mut row| {
                row.sort_by(|&a, &b| r(a).x.total_cmp(&r(b).x));
                row
            })
            .collect();
        let kind = |i: usize| self.zones[i].zone.kind;
        if let Some(d) = order.iter().position(|&i| kind(i) == ZoneKind::Dialogue) {
            let late: Vec<usize> = order[d..].iter().copied().filter(|&i| kind(i) == ZoneKind::Speaker).collect();
            order.retain(|i| !late.contains(i));
            order.splice(d..d, late);
        }
        order
    }

    /// One crop read the A1 way, then with the other clean-up when that found nothing useful.
    fn read_crop(&mut self, zi: usize, g: &Gray, ocr: &mut dyn Ocr) -> Result<Vec<Unit>, String> {
        let z = &self.zones[zi];
        let Some(px) = z.px else { return Ok(Vec::new()) };
        let (kind, cfg) = (z.zone.kind, &self.cfg);
        let origin = (px.x0 as f32, px.y0 as f32);
        let (lines, runs) = read_zone(ocr, g, cfg, origin, kind.one_line())?;
        self.ocr_runs += runs;
        let mut units = units_of(kind, &lines);
        let min = cfg.track.min_letters;
        if units.iter().all(|u| is_junk(&u.raw, min)) && !small_path(g, cfg, kind.one_line()) {
            // Small zones tried both clean-ups already.
            let lines = read_alt(ocr, g, cfg, origin, kind.one_line())?;
            self.ocr_runs += 1;
            units = units_of(kind, &lines);
        }
        units.retain(|u| !is_junk(&u.raw, min));
        Ok(units)
    }

    /// The lines of one zone at time `t`: the current crop, and the sharpest ring crop when the
    /// current one looks faded.
    fn read_zone_now(&mut self, zi: usize, g: &Gray, t: f64, ocr: &mut dyn Ocr) -> Result<Vec<Unit>, String> {
        let now = self.read_crop(zi, g, ocr)?;
        let z = &self.zones[zi];
        let sharp = g.sharpness();
        let pick = if self.mcfg.look_ms > 0.0 { z.ring.best_in(t - self.mcfg.look_ms, t - 1e-6, |c| c.sharpness()) } else { None };
        let Some((_, ring_g)) = pick.filter(|(_, c)| c.sharpness() > sharp * self.mcfg.fade_ratio.max(1.0)) else { return Ok(now) };
        let ring_g = ring_g.clone();
        let joins = z.zone.kind.joins_lines();
        let old = self.read_crop(zi, &ring_g, ocr)?;
        Ok(merge(now, old, joins, self.cfg.track.jitter))
    }

    /// Reads every zone of `f` now (see the module text). A press that was waiting is used up.
    pub fn read_now(&mut self, f: &Frame, ocr: &mut dyn Ocr) -> Press {
        self.pending = false;
        self.fit_to(f);
        let t = f.t_ms;
        let jit = self.cfg.track.jitter;
        let mut out = Press { t_ms: t, ..Press::default() };
        let mut said: Vec<String> = Vec::new();
        for zi in self.order() {
            let Some(px) = self.zones[zi].px else { continue };
            let g = Gray::crop(f.rgba, f.w, px, self.cfg.channel);
            match self.read_zone_now(zi, &g, t, ocr) {
                Ok(units) => {
                    let z = &self.zones[zi].zone;
                    for u in units {
                        // Once per press (overlapping zones, or a row read twice).
                        if said.iter().any(|s| relate(&u.norm, s, jit) == Relation::Same) {
                            continue;
                        }
                        said.push(u.norm.clone());
                        out.events.push(Event { t_ms: t, text: u.raw, rect: u.rect, zone: Some(z.name.clone()), kind: Some(z.kind) });
                    }
                }
                Err(e) => out.errors.push(format!("{}: {e}", self.zones[zi].zone.name)),
            }
        }
        if !out.events.is_empty() {
            self.last = out.events.clone();
        } else if out.errors.is_empty() && !self.last.is_empty() {
            out.repeat = true;
            out.events = self.last.iter().map(|e| Event { t_ms: t, ..e.clone() }).collect();
        }
        out
    }
}

/// Joins the read of the current crop (`now`) with the read of a clearer ring crop (`old`).
/// A ring line replaces a shorter read of the same line; an empty `now` takes the ring lines; in
/// toast zones (`joins` false) ring rows the current read lacks are added, top to bottom.
fn merge(now: Vec<Unit>, old: Vec<Unit>, joins: bool, jit: f32) -> Vec<Unit> {
    if now.is_empty() {
        return old;
    }
    let close = |a: &Unit, b: &Unit| matches!(relate(&a.norm, &b.norm, jit), Relation::Same | Relation::Grows);
    let mut out: Vec<Unit> = now
        .into_iter()
        .map(|u| match old.iter().find(|o| close(&u, o)) {
            Some(o) if o.norm.chars().count() > u.norm.chars().count() => o.clone(),
            _ => u,
        })
        .collect();
    if !joins {
        let extra: Vec<Unit> = old.into_iter().filter(|o| out.iter().all(|u| relate(&u.norm, &o.norm, jit) == Relation::Different)).collect();
        out.extend(extra);
        out.sort_by(|a, b| a.rect.map_or(0.0, |r| r.y).total_cmp(&b.rect.map_or(0.0, |r| r.y)));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geom::Rect;

    fn u(s: &str, y: f32) -> Unit {
        Unit::new(s, Some(Rect::new(0.0, y, 10.0, 5.0)))
    }

    #[test]
    fn merge_prefers_the_fuller_ring_read_and_adds_faded_toast_rows() {
        let m = merge(vec![u("Quest upd", 10.0)], vec![u("Quest updated", 10.0), u("Picked up Iron Ore", 0.0)], false, 0.3);
        assert_eq!(m.iter().map(|u| u.raw.as_str()).collect::<Vec<_>>(), vec!["Picked up Iron Ore", "Quest updated"]);
        // Dialogue: a different ring line is an older line, not added.
        let m = merge(vec![u("New words here", 0.0)], vec![u("Old line before", 0.0)], true, 0.3);
        assert_eq!(m.iter().map(|u| u.raw.as_str()).collect::<Vec<_>>(), vec!["New words here"]);
        let m = merge(vec![], vec![u("Faded away", 0.0)], true, 0.3);
        assert_eq!(m[0].raw, "Faded away");
    }

    #[test]
    fn order_is_rows_then_columns_with_speaker_first() {
        let r = |x: f32, y: f32| Rect::new(x, y, 20.0, 10.0);
        let zones = [
            Zone::new("dlg", ZoneKind::Dialogue, r(0.0, 50.0)),
            Zone::new("spk", ZoneKind::Speaker, r(30.0, 52.0)),
            Zone::new("right", ZoneKind::Notification, r(60.0, 0.0)),
            Zone::new("left", ZoneKind::Notification, r(10.0, 1.0)),
            Zone::new("hud", ZoneKind::Ignore, r(0.0, 0.0)),
            Zone::new("low", ZoneKind::Prompt, r(0.0, 80.0)),
        ];
        let mut m = Manual::new(Config::default(), ManualConfig::default(), &zones);
        let px = vec![0u8; 100 * 100 * 4];
        m.fit_to(&Frame::new(0.0, 100, 100, &px));
        let names: Vec<&str> = m.order().into_iter().map(|i| m.zones[i].zone.name.as_str()).collect();
        assert_eq!(names, vec!["left", "right", "spk", "dlg", "low"]);
    }
}
