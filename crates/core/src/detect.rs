//! No-zone mode: finds new text anywhere in the frame by change and contrast.
//!
//! The frame is read at half size and cut into cells. A cell is a text candidate when it has
//! strong edges (text strokes) and changed recently. Touching candidate cells form boxes; each box
//! becomes a "region" with its own tracker (lines in a region are joined, so a multi-line tooltip
//! is one event). Regions are OCR'd when they change, like zones: a region whose pixels did not
//! change since its last read reuses that read, and the reuse is no new evidence. Regions that
//! stay quiet expire.
//! This is weaker than zones: busy, moving backgrounds also make edges that change.

use crate::frame::Frame;
use crate::geom::{PxBox, Rect};
use crate::image::{Channel, Gray};
use crate::ocr::Ocr;
use crate::pipeline::{Config, Event, read_zone, units_of};
use crate::signature::Signature;
use crate::text::{is_junk, normalize};
use crate::track::{Params, Tracker, Unit};
use crate::zone::ZoneKind;

#[derive(Clone, Debug, PartialEq)]
pub struct FreeParams {
    /// Cell size in half-size pixels (8 means 16 frame pixels).
    pub cell: u32,
    /// Smallest mean edge step for a text cell.
    pub edge_min: f32,
    /// A cell changed when its mean or edge moved more than this.
    pub change: f32,
    /// A cell stays a candidate this long after it changed.
    pub recent_ms: f64,
    /// Shortest time between two OCR runs of one region.
    pub min_gap_ms: f64,
    /// At most this many region OCR runs per frame.
    pub max_ocr: usize,
    /// Boxes smaller than this many cells are ignored.
    pub min_cells: usize,
    /// A region without candidate cells for this long is dropped.
    pub expire_ms: f64,
    /// Stable time before speaking.
    pub stable_ms: f64,
    /// A region whose box moved more than this many pixels is moving text and is not read.
    pub move_px: f32,
    /// Cell changes are measured against the frame at least this old, so a slow fade counts the
    /// same at 30 and at 60 frames per second.
    pub diff_ms: f64,
}

impl Default for FreeParams {
    fn default() -> Self {
        FreeParams {
            cell: 8,
            edge_min: 14.0,
            change: 12.0,
            recent_ms: 600.0,
            min_gap_ms: 60.0,
            max_ocr: 4,
            min_cells: 3,
            expire_ms: 400.0,
            stable_ms: 120.0,
            move_px: 1000.0,
            diff_ms: 30.0,
        }
    }
}

struct Region {
    rect: Rect,
    tracker: Tracker,
    last_ocr: f64,
    last_active: f64,
    changed: bool,
    /// Centre of the first box, and the largest distance the box centre moved from it.
    start: (f32, f32),
    moved: f32,
    /// Signature and units of the last OCR read.
    sig: Option<Signature>,
    last_units: Vec<Unit>,
}

pub struct FreeDetector {
    p: FreeParams,
    track: Params,
    prev: Option<Signature>,
    prev_t: f64,
    /// Last change time per cell.
    changed_at: Vec<f64>,
    regions: Vec<Region>,
    /// Frame areas never looked at (zones the user drew, in zone-plus-free mode).
    pub mask: Vec<Rect>,
    /// Recently spoken lines (normalised) with time, so a region that is rebuilt does not repeat.
    spoken: Vec<(String, f64)>,
}

/// Half-size grey of the whole frame (every second pixel).
pub fn half_gray(f: &Frame, ch: Channel) -> Gray {
    let (w, h) = (f.w / 2, f.h / 2);
    let mut px = Vec::with_capacity((w * h) as usize);
    for y in 0..h {
        let row = (y * 2 * f.w) as usize * 4;
        for x in 0..w {
            let i = row + (x * 2) as usize * 4;
            let p = &f.rgba[i..i + 3];
            px.push(match ch {
                Channel::Luma => ((p[0] as u32 * 77 + p[1] as u32 * 150 + p[2] as u32 * 29) >> 8) as u8,
                Channel::Max => p[0].max(p[1]).max(p[2]),
            });
        }
    }
    Gray { w, h, px }
}

/// Connected boxes of `on` cells (8-neighbour, with a one-cell horizontal bridge so words join).
pub fn boxes(on: &[bool], cols: usize, rows: usize, min_cells: usize) -> Vec<(usize, usize, usize, usize)> {
    let mut seen = vec![false; on.len()];
    let mut out = Vec::new();
    for start in 0..on.len() {
        if !on[start] || seen[start] {
            continue;
        }
        let mut stack = vec![start];
        seen[start] = true;
        let (mut x0, mut y0, mut x1, mut y1, mut n) = (usize::MAX, usize::MAX, 0, 0, 0);
        while let Some(i) = stack.pop() {
            let (x, y) = (i % cols, i / cols);
            n += 1;
            x0 = x0.min(x);
            y0 = y0.min(y);
            x1 = x1.max(x);
            y1 = y1.max(y);
            for dy in -2i64..=2 {
                for dx in -2i64..=2 {
                    // Bridge one empty cell along a row (between words), or between the lines
                    // of one block, also one cell to the side (lines of a tooltip do not start
                    // and end at the same cell). No longer diagonal jumps.
                    if dx.abs() == 2 && dy != 0 || dy.abs() == 2 && dx.abs() > 1 {
                        continue;
                    }
                    let (nx, ny) = (x as i64 + dx, y as i64 + dy);
                    if nx < 0 || ny < 0 || nx >= cols as i64 || ny >= rows as i64 {
                        continue;
                    }
                    let j = ny as usize * cols + nx as usize;
                    if on[j] && !seen[j] {
                        seen[j] = true;
                        stack.push(j);
                    }
                }
            }
        }
        if n >= min_cells {
            out.push((x0, y0, x1 + 1, y1 + 1));
        }
    }
    out
}

/// Overlap area of two rects.
fn overlap(a: &Rect, b: &Rect) -> f32 {
    let w = a.right().min(b.right()) - a.x.max(b.x);
    let h = a.bottom().min(b.bottom()) - a.y.max(b.y);
    if w <= 0.0 || h <= 0.0 { 0.0 } else { w * h }
}

fn iou(a: &Rect, b: &Rect) -> f32 {
    let w = a.right().min(b.right()) - a.x.max(b.x);
    let h = a.bottom().min(b.bottom()) - a.y.max(b.y);
    if w <= 0.0 || h <= 0.0 {
        return 0.0;
    }
    let i = w * h;
    i / (a.area() + b.area() - i)
}

impl FreeDetector {
    pub fn new(p: FreeParams, track: Params) -> FreeDetector {
        FreeDetector { p, track, prev: None, prev_t: f64::NEG_INFINITY, changed_at: Vec::new(), regions: Vec::new(), mask: Vec::new(), spoken: Vec::new() }
    }

    /// Returns the events, the number of OCR runs and the first OCR error (the regions after a
    /// failed read are still handled).
    pub fn on_frame(&mut self, f: &Frame, ocr: &mut dyn Ocr, cfg: &Config, mut trace: Option<&mut Vec<String>>) -> (Vec<Event>, u64, Option<String>) {
        let t = f.t_ms;
        let p = self.p.clone();
        let half = half_gray(f, cfg.channel);
        let sig = Signature::of(&half, p.cell);
        let (cols, rows) = (sig.cols as usize, sig.rows as usize);
        if self.changed_at.len() != cols * rows {
            self.changed_at = vec![f64::NEG_INFINITY; cols * rows];
            self.prev = None;
        }
        if let Some(prev) = &self.prev {
            for i in 0..cols * rows {
                let d = sig.mean[i].abs_diff(prev.mean[i]).max(sig.edge[i].abs_diff(prev.edge[i]));
                if d as f32 > p.change {
                    self.changed_at[i] = t;
                }
            }
        }
        let cell_px = (p.cell * 2) as f32;
        let masked = |cx: usize, cy: usize| {
            let (x, y) = ((cx as f32 + 0.5) * cell_px, (cy as f32 + 0.5) * cell_px);
            self.mask.iter().any(|m| x >= m.x && x < m.right() && y >= m.y && y < m.bottom())
        };
        let on: Vec<bool> = (0..cols * rows)
            .map(|i| sig.edge[i] as f32 >= p.edge_min && t - self.changed_at[i] <= p.recent_ms && !masked(i % cols, i / cols))
            .collect();
        if self.prev.is_none() || t - self.prev_t >= p.diff_ms - 1e-6 {
            self.prev = Some(sig);
            self.prev_t = t;
        }
        let found = boxes(&on, cols, rows, p.min_cells);
        // Match boxes to regions.
        for r in &mut self.regions {
            r.changed = false;
        }
        for (x0, y0, x1, y1) in found {
            let rect = Rect::new(x0 as f32 * cell_px, y0 as f32 * cell_px, (x1 - x0) as f32 * cell_px, (y1 - y0) as f32 * cell_px).inflate(6.0);
            if rect.w > f.w as f32 * 0.8 || rect.h > f.h as f32 * 0.6 {
                continue;
            }
            // A box that reaches into a masked zone (or its border) is that zone's text.
            if self.mask.iter().any(|m| overlap(&m.inflate(16.0), &rect) > 0.15 * rect.area()) {
                continue;
            }
            match self.regions.iter_mut().max_by(|a, b| iou(&a.rect, &rect).total_cmp(&iou(&b.rect, &rect))) {
                Some(r) if iou(&r.rect, &rect) > 0.3 => {
                    r.rect = rect;
                    let c = (rect.x + rect.w / 2.0, rect.y + rect.h / 2.0);
                    r.moved = r.moved.max(((c.0 - r.start.0).powi(2) + (c.1 - r.start.1).powi(2)).sqrt());
                    r.last_active = t;
                    r.changed = true;
                }
                _ => self.regions.push(Region {
                    rect,
                    tracker: Tracker::new(Params { stable_ms: p.stable_ms, ..self.track.clone() }),
                    last_ocr: f64::NEG_INFINITY,
                    last_active: t,
                    changed: true,
                    start: (rect.x + rect.w / 2.0, rect.y + rect.h / 2.0),
                    moved: 0.0,
                    sig: None,
                    last_units: Vec::new(),
                }),
            }
        }
        // OCR the regions that need it, oldest read first.
        let mut order: Vec<usize> = (0..self.regions.len())
            .filter(|&i| {
                let r = &self.regions[i];
                t - r.last_ocr >= p.min_gap_ms - 1e-6 && (r.changed || r.tracker.pending())
            })
            .collect();
        order.sort_by(|&a, &b| self.regions[a].last_ocr.total_cmp(&self.regions[b].last_ocr));
        order.truncate(p.max_ocr.max(1));
        let mut out = Vec::new();
        let mut runs = 0;
        let mut err = None;
        for i in order {
            let r = &mut self.regions[i];
            let Some(px) = r.rect.to_px(f.w, f.h) else { continue };
            let g = Gray::crop(f.rgba, f.w, px, cfg.channel);
            let sig = Signature::of(&g, cfg.cell);
            // Same pixels as the last read: reuse it, and it is no new evidence.
            let same = r.sig.as_ref().is_some_and(|s| sig.delta(s, cfg.cell_thresh).max <= cfg.same_px);
            let units = if same {
                r.last_units.clone()
            } else {
                match read_zone(ocr, &g, cfg, (px.x0 as f32, px.y0 as f32), false) {
                    Ok((lines, n)) => {
                        runs += n;
                        units_of(ZoneKind::Dialogue, &lines)
                    }
                    Err(e) => {
                        err.get_or_insert(e);
                        continue;
                    }
                }
            };
            r.last_ocr = t;
            if !same {
                r.sig = Some(sig);
                r.last_units = units.clone();
            }
            let (speak, lost) = r.tracker.update(t, &units, !same);
            if let Some(tr) = trace.as_deref_mut()
                && tr.len() < 200_000
            {
                let u: Vec<&str> = units.iter().map(|u| u.raw.as_str()).collect();
                tr.push(format!("{t:.0} free {:?} {} {u:?}", PxBox::rect(&px), if same { "same" } else { "ocr" }));
            }
            let mut texts: Vec<(String, Option<Rect>)> = speak.into_iter().map(|s| (s.raw, s.rect)).collect();
            // Text that travels (damage numbers, floating labels) is not read.
            let moving = r.moved > p.move_px;
            if cfg.emit_lost {
                for l in lost {
                    if let Some(b) = l.agreed(&r.tracker.params)
                        && !is_junk(&b.raw, self.track.min_letters)
                    {
                        texts.push((b.raw.clone(), b.rect));
                    }
                }
            }
            if moving {
                texts.clear();
            }
            for (text, rect) in texts {
                let n = normalize(&text);
                if self.spoken.iter().any(|(s, ts)| *s == n && t - ts < 2000.0) {
                    continue;
                }
                self.spoken.push((n, t));
                out.push(Event { t_ms: t, text, rect, zone: None, kind: None });
            }
        }
        // Expire quiet regions.
        let exp = p.expire_ms;
        self.regions.retain(|r| t - r.last_active < exp || r.tracker.pending());
        self.spoken.retain(|(_, ts)| t - ts < 5000.0);
        (out, runs, err)
    }

    /// Lines with enough agreeing reads that were never spoken.
    pub fn finish(&mut self, t: f64, cfg: &Config) -> Vec<Event> {
        let mut out = Vec::new();
        if !cfg.emit_lost {
            return out;
        }
        for r in &mut self.regions {
            let params = r.tracker.params.clone();
            for l in r.tracker.drain() {
                if let Some(b) = l.agreed(&params)
                    && !is_junk(&b.raw, self.track.min_letters)
                {
                    out.push(Event { t_ms: t, text: b.raw.clone(), rect: b.rect, zone: None, kind: None });
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boxes_join_words_on_a_row_and_split_rows_apart() {
        // 8x5 grid: a "word" at (0..2, 1), another at (3..5, 1) one cell apart, and a row 4
        // two rows below (too far to join).
        let mut on = vec![false; 40];
        for x in [0, 1, 3, 4] {
            on[8 + x] = true;
        }
        for x in 0..3 {
            on[32 + x] = true;
        }
        let b = boxes(&on, 8, 5, 2);
        assert_eq!(b, vec![(0, 1, 5, 2), (0, 4, 3, 5)]);
        assert!(boxes(&on, 8, 5, 6).is_empty());
    }

    #[test]
    fn half_gray_takes_every_second_pixel() {
        let mut px = vec![0u8; 4 * 2 * 4];
        px[(2 * 4)..(2 * 4 + 4)].copy_from_slice(&[255, 255, 255, 255]);
        let g = half_gray(&Frame::new(0.0, 4, 2, &px), Channel::Max);
        assert_eq!((g.w, g.h), (2, 1));
        assert_eq!(g.px, vec![0, 255]);
    }
}
