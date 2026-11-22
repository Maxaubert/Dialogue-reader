//! `a1` reader: the dr-core engine (direction A1) wrapped for the arena.
//!
//! It maps the arena layout zones to dr-core zones (`hud` becomes "ignore") and wraps the arena OCR
//! engines (WinRT; PP-OCR with `--features ppocr`) behind the dr-core `Ocr` trait. It never sees
//! the truth. With `zones=off` it ignores the layout and runs the no-zone detector on the whole
//! frame. With `mode=manual` it runs the dr-core manual mode instead: no automatic reads, and a
//! simulated key press every `press_every_ms` (a fixed clock, never the truth) reads every zone.
//! Design and options: `docs/reader-a1.md`.

use crate::model::{Layout, Rect as ARect, ZoneKind as AKind};
use crate::ocr::{self, Engine, EngineOptions};
use crate::reader::{FrameRef, ReadEvent, Reader, ReaderContext};
use anyhow::{Context, Result, bail};
use dr_core::image::{Channel, Prep};
use dr_core::pipeline::KindTiming;
use dr_core::{Config, Frame, Manual, ManualConfig, Ocr, OcrLine, Pipeline, Rect, Zone, ZoneKind};
use std::collections::BTreeMap;
use std::path::PathBuf;

/// Every option `--reader-opt` accepts (anything else is an error, so typos never pass silently).
pub const OPTIONS: &[&str] = &[
    "engine", "lang", "models", "zones", "free", "scale", "stretch", "binarize", "dark", "pad", "chan", "refine", "r_scale",
    "r_stretch", "r_binarize", "r_dark", "r_pad", "r_margin", "r_fullw", "recheck", "small_alt", "small_h", "s_scale", "s_stretch", "s_binarize", "s_dark", "s_pad", "cell", "cell_thresh",
    "change_share", "same_px", "static_ms", "refresh_ms", "idle_ms", "ring_ms", "rescue", "emit_lost", "prompts", "gap_dialogue", "gap_speaker", "gap_notif", "gap_prompt",
    "stable_dialogue", "stable_speaker", "stable_notif", "stable_prompt", "confirm", "gone_reads", "jitter",
    "min_letters", "consensus", "typing_ms", "grow_ms", "change_ms", "lost_letters", "second_ms", "max_reads", "loose", "ref_h", "free_diff", "gone_dialogue", "gone_speaker", "gone_notif", "gone_prompt", "free_cell", "free_edge", "free_change", "free_recent", "free_gap", "free_max", "free_min_cells",
    "free_expire", "free_stable", "free_move", "trace", "mode", "press_every_ms", "look_ms", "repeats",
];

/// The arena OCR engine behind the dr-core trait.
pub struct EngineOcr(pub Box<dyn Engine>);

impl Ocr for EngineOcr {
    fn max_dim(&self) -> u32 {
        self.0.max_dim()
    }
    fn recognize(&mut self, w: u32, h: u32, rgba: &[u8]) -> Result<Vec<OcrLine>, String> {
        let lines = self.0.recognize(w, h, rgba).map_err(|e| format!("{e:#}"))?;
        Ok(lines.into_iter().map(|l| OcrLine { text: l.text, rect: Rect::new(l.rect.x, l.rect.y, l.rect.w, l.rect.h) }).collect())
    }
}

/// Manual mode: dr-core `Manual` and the simulated key.
struct ManualRun {
    m: Manual,
    every_ms: f64,
    next_ms: f64,
    /// Give the "repeat last" lines of an empty press (the arena scores them as hallucinations,
    /// because the line is no longer on screen).
    repeats: bool,
}

pub struct A1 {
    pipe: Pipeline,
    manual: Option<ManualRun>,
    ocr: EngineOcr,
    use_zones: bool,
    trace: Option<PathBuf>,
    warned: bool,
}

struct Opts<'a>(&'a BTreeMap<String, String>);

impl Opts<'_> {
    fn get<T: std::str::FromStr>(&self, key: &str, default: T) -> Result<T> {
        match self.0.get(key) {
            None => Ok(default),
            Some(v) => v.parse().ok().with_context(|| format!("a1: bad value {v:?} for {key}")),
        }
    }
    fn flag(&self, key: &str, default: bool) -> Result<bool> {
        match self.0.get(key).map(|s| s.as_str()) {
            None => Ok(default),
            Some("1" | "on" | "true" | "yes") => Ok(true),
            Some("0" | "off" | "false" | "no") => Ok(false),
            Some(v) => bail!("a1: {key} must be on or off, not {v:?}"),
        }
    }
}

/// Builds the dr-core config from `--reader-opt` values (defaults are the tuned A1 settings).
pub fn config_from(options: &BTreeMap<String, String>) -> Result<Config> {
    for k in options.keys() {
        if !OPTIONS.contains(&k.as_str()) {
            bail!("a1: unknown option {k:?}; known: {}", OPTIONS.join(", "));
        }
    }
    let o = Opts(options);
    let d = Config::default();
    let timing = |name: &str, base: KindTiming| -> Result<KindTiming> {
        Ok(KindTiming {
            min_gap_ms: o.get(&format!("gap_{name}"), base.min_gap_ms)?,
            stable_ms: o.get(&format!("stable_{name}"), base.stable_ms)?,
            gone_ms: o.get(&format!("gone_{name}"), base.gone_ms)?,
        })
    };
    let mut c = Config {
        prep: Prep {
            scale: o.get("scale", d.prep.scale)?,
            stretch: o.flag("stretch", d.prep.stretch)?,
            binarize: o.flag("binarize", d.prep.binarize)?,
            dark_text: o.flag("dark", d.prep.dark_text)?,
            pad: o.get("pad", d.prep.pad)?,
        },
        refine: if o.flag("refine", d.refine.is_some())? {
            let r = d.refine.unwrap_or_default();
            Some(Prep {
                scale: o.get("r_scale", r.scale)?,
                stretch: o.flag("r_stretch", r.stretch)?,
                binarize: o.flag("r_binarize", r.binarize)?,
                dark_text: o.flag("r_dark", r.dark_text)?,
                pad: o.get("r_pad", r.pad)?,
            })
        } else {
            None
        },
        refine_margin: o.get("r_margin", d.refine_margin)?,
        refine_full_width: o.flag("r_fullw", d.refine_full_width)?,
        recheck: o.flag("recheck", d.recheck)?,
        loose: o.get("loose", d.loose)?,
        lost_min_letters: o.get("lost_letters", d.lost_min_letters)?,
        second_ms: o.get("second_ms", d.second_ms)?,
        max_reads: o.get("max_reads", d.max_reads)?,
        ref_h: o.get("ref_h", d.ref_h)?,
        small_alt: if o.flag("small_alt", d.small_alt.is_some())? { d.small_alt } else { None },
        small_h: o.get("small_h", d.small_h)?,
        small_prep: Prep {
            scale: o.get("s_scale", d.small_prep.scale)?,
            stretch: o.flag("s_stretch", d.small_prep.stretch)?,
            binarize: o.flag("s_binarize", d.small_prep.binarize)?,
            dark_text: o.flag("s_dark", d.small_prep.dark_text)?,
            pad: o.get("s_pad", d.small_prep.pad)?,
        },
        channel: match o.get("chan", "luma".to_string())?.as_str() {
            "luma" => Channel::Luma,
            "max" => Channel::Max,
            v => bail!("a1: chan must be luma or max, not {v:?}"),
        },
        cell: o.get("cell", d.cell)?,
        cell_thresh: o.get("cell_thresh", d.cell_thresh)?,
        change_share: o.get("change_share", d.change_share)?,
        same_px: o.get("same_px", d.same_px)?,
        refresh_ms: o.get("refresh_ms", d.refresh_ms)?,
        idle_ms: o.get("idle_ms", d.idle_ms)?,
        ring_ms: o.get("ring_ms", d.ring_ms)?,
        rescue: o.flag("rescue", d.rescue)?,
        emit_lost: o.flag("emit_lost", d.emit_lost)?,
        read_prompts: o.flag("prompts", d.read_prompts)?,
        dialogue: timing("dialogue", d.dialogue)?,
        speaker: timing("speaker", d.speaker)?,
        notification: timing("notif", d.notification)?,
        prompt: timing("prompt", d.prompt)?,
        trace: options.contains_key("trace"),
        free_outside: o.flag("free", d.free_outside)?,
        ..d.clone()
    };
    c.track.confirm = o.get("confirm", d.track.confirm)?;
    c.track.gone_reads = o.get("gone_reads", d.track.gone_reads)?;
    c.track.jitter = o.get("jitter", d.track.jitter)?;
    c.track.min_letters = o.get("min_letters", d.track.min_letters)?;
    c.track.consensus = o.get("consensus", d.track.consensus)?;
    c.track.static_ms = o.get("static_ms", d.track.static_ms)?;
    c.track.typing_ms = o.get("typing_ms", d.track.typing_ms)?;
    c.track.grow_ms = o.get("grow_ms", d.track.grow_ms)?;
    c.track.change_ms = o.get("change_ms", d.track.change_ms)?;
    let f = &mut c.free;
    f.cell = o.get("free_cell", f.cell)?;
    f.edge_min = o.get("free_edge", f.edge_min)?;
    f.change = o.get("free_change", f.change)?;
    f.recent_ms = o.get("free_recent", f.recent_ms)?;
    f.min_gap_ms = o.get("free_gap", f.min_gap_ms)?;
    f.max_ocr = o.get("free_max", f.max_ocr)?;
    f.min_cells = o.get("free_min_cells", f.min_cells)?;
    f.expire_ms = o.get("free_expire", f.expire_ms)?;
    f.stable_ms = o.get("free_stable", f.stable_ms)?;
    f.move_px = o.get("free_move", f.move_px)?;
    f.diff_ms = o.get("free_diff", f.diff_ms)?;
    Ok(c)
}

/// Arena zones to dr-core zones. Only rect and kind are used for behaviour; the name is kept for
/// the event's `zone` field.
pub fn map_zones(layout: &Layout) -> Vec<Zone> {
    let r = |r: &ARect| Rect::new(r.x, r.y, r.w, r.h);
    layout
        .zones
        .iter()
        .map(|z| {
            let kind = match z.kind {
                AKind::Dialogue => ZoneKind::Dialogue,
                AKind::Speaker => ZoneKind::Speaker,
                AKind::Notification => ZoneKind::Notification,
                AKind::Prompt => ZoneKind::Prompt,
                AKind::Hud => ZoneKind::Ignore,
            };
            Zone::new(&z.name, kind, r(&z.rect))
        })
        .collect()
}

pub fn create(ctx: &ReaderContext) -> Result<Box<dyn Reader>> {
    let cfg = config_from(&ctx.options)?;
    let o = Opts(&ctx.options);
    let mut eo = EngineOptions { lang: o.get("lang", "en-US".to_string())?, ..Default::default() };
    if let Some(m) = ctx.options.get("models") {
        eo.models = m.into();
    }
    let engine = ocr::create_engine(&o.get("engine", "winocr".to_string())?, &eo)?;
    let use_zones = o.flag("zones", true)?;
    let trace = ctx.options.get("trace").map(|d| PathBuf::from(d).join(format!("{}.txt", ctx.scenario_id)));
    let zones = if use_zones { map_zones(&ctx.layout) } else { Vec::new() };
    let manual = match o.get("mode", "auto".to_string())?.as_str() {
        "auto" => None,
        "manual" => {
            let every_ms: f64 = o.get("press_every_ms", 3000.0)?;
            if every_ms.is_nan() || every_ms <= 0.0 {
                bail!("a1: press_every_ms must be above 0");
            }
            let mcfg = ManualConfig { look_ms: o.get("look_ms", ManualConfig::default().look_ms)?, ..ManualConfig::default() };
            Some(ManualRun { m: Manual::new(cfg.clone(), mcfg, &zones), every_ms, next_ms: every_ms, repeats: o.flag("repeats", true)? })
        }
        v => bail!("a1: mode must be auto or manual, not {v:?}"),
    };
    Ok(Box::new(A1 { pipe: Pipeline::new(cfg, &zones), manual, ocr: EngineOcr(engine), use_zones, trace, warned: false }))
}

fn to_event(e: dr_core::Event) -> ReadEvent {
    ReadEvent { t_ms: e.t_ms, text: e.text, rect: e.rect.map(|r| ARect::new(r.x, r.y, r.w, r.h)), zone: e.zone }
}

impl Reader for A1 {
    fn name(&self) -> &str {
        "a1"
    }

    fn setup(&mut self, layout: &Layout) {
        let zones = if self.use_zones { map_zones(layout) } else { Vec::new() };
        self.pipe.set_zones(&zones);
        if let Some(mr) = &mut self.manual {
            mr.m.set_zones(&zones);
        }
    }

    fn on_frame(&mut self, f: &FrameRef) -> Vec<ReadEvent> {
        let frame = Frame::new(f.t_ms, f.width, f.height, f.rgba);
        if let Some(mr) = &mut self.manual {
            // A key press at every multiple of `every_ms` (the first frame at or after it).
            if f.t_ms >= mr.next_ms {
                mr.m.press();
                mr.next_ms = ((f.t_ms / mr.every_ms).floor() + 1.0) * mr.every_ms;
            }
            let Some(p) = mr.m.on_frame(&frame, &mut self.ocr) else { return Vec::new() };
            if let Some(e) = p.errors.first() {
                panic!("a1: OCR failed at t={} ms: {e}", f.t_ms);
            }
            if p.repeat && !mr.repeats {
                return Vec::new();
            }
            return p.events.into_iter().map(to_event).collect();
        }
        let out = self.pipe.on_frame(&frame, &mut self.ocr);
        if !self.warned && !self.pipe.empty_zones().is_empty() {
            self.warned = true;
            eprintln!("a1: zones with no pixels in a {}x{} frame: {:?}", f.width, f.height, self.pipe.empty_zones());
        }
        // A failing engine must not look like an empty screen. (dr-core keeps the other zones'
        // events; the arena stops, because its scores would be wrong.)
        if let Some(e) = out.errors.first() {
            panic!("a1: OCR failed at t={} ms: {e}", f.t_ms);
        }
        out.events.into_iter().map(to_event).collect()
    }

    fn finish(&mut self, t_ms: f64) -> Vec<ReadEvent> {
        if let Some(mr) = &self.manual {
            if let Some(p) = &self.trace {
                if let Some(dir) = p.parent() {
                    let _ = std::fs::create_dir_all(dir);
                }
                if let Err(e) = std::fs::write(p, format!("manual mode\nocr runs: {}", mr.m.ocr_runs)) {
                    eprintln!("a1: cannot write trace {}: {e}", p.display());
                }
            }
            // Manual mode reads only on a press.
            return Vec::new();
        }
        let out = self.pipe.finish(t_ms, &mut self.ocr);
        if let Some(e) = out.errors.first() {
            panic!("a1: OCR failed at finish: {e}");
        }
        let ev = out.events;
        if let Some(p) = &self.trace {
            if let Some(dir) = p.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let mut lines = std::mem::take(&mut self.pipe.trace);
            lines.push(format!("ocr runs: {}", self.pipe.ocr_runs));
            if let Err(e) = std::fs::write(p, lines.join("\n")) {
                eprintln!("a1: cannot write trace {}: {e}", p.display());
            }
        }
        ev.into_iter().map(to_event).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Zone as AZone;

    #[test]
    fn options_parse_and_reject_typos() {
        let mut m = BTreeMap::new();
        m.insert("scale".to_string(), "3".to_string());
        m.insert("stable_notif".to_string(), "90".to_string());
        m.insert("free".to_string(), "on".to_string());
        let c = config_from(&m).unwrap();
        assert_eq!(c.prep.scale, 3.0);
        assert_eq!(c.notification.stable_ms, 90.0);
        assert!(c.free_outside);
        for (k, v) in [("mode", "manual"), ("press_every_ms", "2000"), ("look_ms", "0")] {
            m.insert(k.to_string(), v.to_string());
        }
        assert!(config_from(&m).is_ok());
        m.insert("scael".to_string(), "2".to_string());
        assert!(config_from(&m).is_err());
    }

    #[test]
    fn hud_zones_become_ignore() {
        let layout = Layout {
            zones: vec![
                AZone { name: "hud0".into(), kind: AKind::Hud, rect: ARect::new(0.0, 0.0, 10.0, 10.0) },
                AZone { name: "pickups".into(), kind: AKind::Notification, rect: ARect::new(1.0, 2.0, 3.0, 4.0) },
            ],
        };
        let z = map_zones(&layout);
        assert_eq!(z[0].kind, ZoneKind::Ignore);
        assert_eq!(z[1].kind, ZoneKind::Notification);
        assert_eq!(z[1].rect, Rect::new(1.0, 2.0, 3.0, 4.0));
    }
}
