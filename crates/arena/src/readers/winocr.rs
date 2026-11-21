//! `winocr-naive` reader: WinRT OCR on each layout zone every 500 ms; a zone is read when its
//! text differs from the last read of that zone (a rough model of the old tool).
//!
//! - Polling uses frame time, not wall time.
//! - Each zone gives one event with all its lines joined by `\n`. A zone that holds several items
//!   at once (a stack of toasts) is therefore read as one block, as the old tool did.
//! - The last text is kept per zone, empty included, so the same toast shown twice is read twice.
//! - Without zones the whole frame is read and each line is its own event; a line is read when it
//!   was not on screen at the previous poll.
//! - The event rect is the union of the recognised word boxes, in frame pixels.
//!
//! Options (`--reader-opt k=v`): `interval_ms` (500), `scale` (1.0), `lang` (en-US),
//! `engine` (winocr; ppocr or ppocr-rec need `--features ppocr`), `models` (PP-OCR model folder).

use crate::model::{Layout, Rect, Zone};
use crate::ocr::{self, Engine, EngineOptions, OcrLine};
use crate::reader::{FrameRef, ReadEvent, Reader, ReaderContext};
use crate::text::normalize;
use anyhow::{Context, Result};

pub struct WinOcrNaive {
    engine: Box<dyn Engine>,
    zones: Vec<Zone>,
    interval_ms: f64,
    scale: f32,
    last_poll: Option<f64>,
    /// Normalised last text per zone.
    last_zone_text: Vec<String>,
    /// Normalised lines seen at the previous whole-frame poll.
    last_lines: Vec<String>,
}

fn opt<T: std::str::FromStr>(ctx: &ReaderContext, key: &str, default: T) -> Result<T> {
    match ctx.options.get(key) {
        None => Ok(default),
        Some(v) => v.parse().ok().with_context(|| format!("winocr-naive: bad value {v:?} for {key}")),
    }
}

pub fn create(ctx: &ReaderContext) -> Result<Box<dyn Reader>> {
    let mut opts = EngineOptions { lang: opt(ctx, "lang", "en-US".to_string())?, ..Default::default() };
    if let Some(m) = ctx.options.get("models") {
        opts.models = m.into();
    }
    let engine = ocr::create_engine(&opt(ctx, "engine", "winocr".to_string())?, &opts)?;
    let r = WinOcrNaive::new(engine, &ctx.layout, opt(ctx, "interval_ms", 500.0)?, opt(ctx, "scale", 1.0)?);
    Ok(Box::new(r))
}

impl WinOcrNaive {
    pub fn new(engine: Box<dyn Engine>, layout: &Layout, interval_ms: f64, scale: f32) -> Self {
        let mut r = WinOcrNaive {
            engine,
            zones: Vec::new(),
            interval_ms,
            scale,
            last_poll: None,
            last_zone_text: Vec::new(),
            last_lines: Vec::new(),
        };
        r.set_layout(layout);
        r
    }

    fn set_layout(&mut self, layout: &Layout) {
        self.zones = layout.zones.clone();
        self.last_zone_text = vec![String::new(); self.zones.len()];
        self.last_lines.clear();
    }

    fn ocr(&mut self, f: &FrameRef, region: Option<&Rect>) -> Vec<OcrLine> {
        // A failing engine must not look like an empty screen (recall 0 would be a silent lie).
        ocr::recognize_region(self.engine.as_mut(), f.rgba, f.width, f.height, region, self.scale)
            .unwrap_or_else(|e| panic!("winocr-naive: OCR failed at t={} ms: {e:#}", f.t_ms))
    }
}

/// Union of the line boxes (None when there are no lines).
fn union_rect(lines: &[OcrLine]) -> Option<Rect> {
    let mut it = lines.iter().map(|l| l.rect).filter(|r| r.area() > 0.0);
    let first = it.next()?;
    Some(it.fold(first, |u, r| {
        let x = u.x.min(r.x);
        let y = u.y.min(r.y);
        Rect::new(x, y, u.right().max(r.right()) - x, u.bottom().max(r.bottom()) - y)
    }))
}

/// Zone rule: returns the event text when the zone shows non-empty text that differs from the
/// last read, and updates `last` (empty included).
pub fn zone_change(last: &mut String, lines: &[OcrLine]) -> Option<String> {
    let text = lines.iter().map(|l| l.text.trim()).filter(|t| !t.is_empty()).collect::<Vec<_>>().join("\n");
    let norm = normalize(&text);
    let changed = !norm.is_empty() && norm != *last;
    *last = norm;
    changed.then_some(text)
}

/// Whole-frame rule: the lines not on screen at the previous poll; updates `last`.
pub fn new_lines<'a>(last: &mut Vec<String>, lines: &'a [OcrLine]) -> Vec<&'a OcrLine> {
    let norms: Vec<String> = lines.iter().map(|l| normalize(&l.text)).collect();
    let fresh = lines.iter().zip(&norms).filter(|(_, n)| !n.is_empty() && !last.contains(n)).map(|(l, _)| l).collect();
    *last = norms;
    fresh
}

impl Reader for WinOcrNaive {
    fn name(&self) -> &str {
        "winocr-naive"
    }

    fn setup(&mut self, layout: &Layout) {
        self.set_layout(layout);
    }

    fn on_frame(&mut self, f: &FrameRef) -> Vec<ReadEvent> {
        if self.last_poll.is_some_and(|p| f.t_ms - p < self.interval_ms - 1e-6) {
            return Vec::new();
        }
        self.last_poll = Some(f.t_ms);
        let mut out = Vec::new();
        if self.zones.is_empty() {
            let lines = self.ocr(f, None);
            let mut last = std::mem::take(&mut self.last_lines);
            for l in new_lines(&mut last, &lines) {
                out.push(ReadEvent { t_ms: f.t_ms, text: l.text.clone(), rect: Some(l.rect), zone: None });
            }
            self.last_lines = last;
            return out;
        }
        for zi in 0..self.zones.len() {
            let rect = self.zones[zi].rect;
            let lines = self.ocr(f, Some(&rect));
            if let Some(text) = zone_change(&mut self.last_zone_text[zi], &lines) {
                out.push(ReadEvent { t_ms: f.t_ms, text, rect: union_rect(&lines), zone: Some(self.zones[zi].name.clone()) });
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ZoneKind;
    use std::cell::RefCell;
    use std::rc::Rc;

    fn line(t: &str) -> OcrLine {
        OcrLine { text: t.into(), rect: Rect::new(1.0, 2.0, 30.0, 10.0) }
    }

    #[test]
    fn zone_rule_reads_changes_and_repeats_after_a_gap() {
        let mut last = String::new();
        assert_eq!(zone_change(&mut last, &[line("Hello"), line("there")]), Some("Hello\nthere".into()));
        assert_eq!(zone_change(&mut last, &[line("hello  there!")]), None);
        assert_eq!(zone_change(&mut last, &[]), None);
        assert_eq!(zone_change(&mut last, &[line("Hello there")]), Some("Hello there".into()));
    }

    #[test]
    fn frame_rule_reads_new_lines_only() {
        let mut last = Vec::new();
        let a = [line("Objective"), line("+ 6 Iron Ore")];
        assert_eq!(new_lines(&mut last, &a).len(), 2);
        let b = [line("Objective"), line("+ 12 Iron Ore")];
        let fresh = new_lines(&mut last, &b);
        assert_eq!(fresh.len(), 1);
        assert_eq!(fresh[0].text, "+ 12 Iron Ore");
    }

    /// Engine returning scripted lines and counting calls.
    struct Scripted {
        calls: Rc<RefCell<Vec<(u32, u32)>>>,
        text: Rc<RefCell<String>>,
    }
    impl Engine for Scripted {
        fn name(&self) -> String {
            "scripted".into()
        }
        fn recognize(&mut self, w: u32, h: u32, _rgba: &[u8]) -> Result<Vec<OcrLine>> {
            self.calls.borrow_mut().push((w, h));
            let t = self.text.borrow().clone();
            Ok(if t.is_empty() { vec![] } else { vec![OcrLine { text: t, rect: Rect::new(2.0, 3.0, 10.0, 5.0) }] })
        }
    }

    #[test]
    fn polls_zones_every_interval_and_maps_rects() {
        let calls = Rc::new(RefCell::new(Vec::new()));
        let text = Rc::new(RefCell::new("Hi".to_string()));
        let eng = Scripted { calls: calls.clone(), text: text.clone() };
        let layout = Layout { zones: vec![Zone { name: "dialogue".into(), kind: ZoneKind::Dialogue, rect: Rect::new(10.0, 20.0, 40.0, 30.0) }] };
        let mut r = WinOcrNaive::new(Box::new(eng), &layout, 500.0, 1.0);
        let px = vec![0u8; 100 * 100 * 4];
        let frame = |t| FrameRef { t_ms: t, width: 100, height: 100, rgba: &px };
        let mut events = Vec::new();
        for i in 0..31 {
            if i == 20 {
                *text.borrow_mut() = "Bye".into();
            }
            events.extend(r.on_frame(&frame(i as f64 * 100.0 / 3.0)));
        }
        // t = 0, 500, 1000 ms: three polls of one zone, each on a 40x30 crop.
        assert_eq!(calls.borrow().len(), 3);
        assert_eq!(calls.borrow()[0], (40, 30));
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].text, "Hi");
        assert_eq!(events[0].rect, Some(Rect::new(12.0, 23.0, 10.0, 5.0)));
        assert_eq!(events[0].zone.as_deref(), Some("dialogue"));
        assert_eq!(events[1].text, "Bye");
    }
}
