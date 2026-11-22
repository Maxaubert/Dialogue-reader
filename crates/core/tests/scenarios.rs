//! Whole-pipeline cases the arena generator does not make: back-to-back similar lines, slow and
//! fast typing at several frame rates, still screens, short pop-ups on still pixels, number
//! changes on still pixels, OCR errors and odd zones. Ported from the A1 review (2026-11-22).
//!
//! The fake OCR reads a bar code drawn in the zone (a start mark and 7 bit cells, white on a grey
//! background), so it answers for the pixels it is given, also after scaling, padding and black
//! and white clean-up. The background grey changes whenever the text changes (or on every frame
//! when "busy").

use dr_core::pipeline::{Config, Pipeline, trim_edge_marks};
use dr_core::text::normalize;
use dr_core::track::{Params, Tracker, Unit};
use dr_core::{Frame, Ocr, OcrLine, Rect, Zone, ZoneKind};
use std::cell::RefCell;
use std::rc::Rc;

/// Reads the bar code; id 0 (no code) is an empty read.
struct Coded {
    texts: Rc<RefCell<Vec<String>>>,
}

impl Ocr for Coded {
    fn recognize(&mut self, w: u32, h: u32, rgba: &[u8]) -> Result<Vec<OcrLine>, String> {
        let y = h / 2;
        let px = |x: u32| rgba[((y * w + x) * 4) as usize] as i32;
        let bg = px(0);
        let on = |x: u32| (px(x) - bg).abs() > 60;
        let Some(ms) = (0..w).find(|&x| on(x)) else { return Ok(vec![]) };
        let me = (ms..w).find(|&x| !on(x)).unwrap_or(w);
        let c = (me - ms) as f32;
        let mut id = 0usize;
        for k in 0..7 {
            let x = ms as f32 + (2.0 + k as f32) * c + c / 2.0;
            if (x as u32) < w && on(x as u32) {
                id |= 1 << k;
            }
        }
        let texts = self.texts.borrow();
        Ok(match texts.get(id) {
            Some(t) if id > 0 => vec![OcrLine { text: t.clone(), rect: Rect::new(0.0, 0.0, w as f32, h as f32) }],
            _ => vec![],
        })
    }
}

/// Always reads `text`; fails on images `fail_w` wide.
struct Fixed {
    text: String,
    calls: usize,
    fail_w: Option<u32>,
}

impl Ocr for Fixed {
    fn recognize(&mut self, w: u32, h: u32, _rgba: &[u8]) -> Result<Vec<OcrLine>, String> {
        self.calls += 1;
        if self.fail_w == Some(w) {
            return Err("engine busy".into());
        }
        Ok(vec![OcrLine { text: self.text.clone(), rect: Rect::new(0.0, 0.0, w as f32, h as f32) }])
    }
}

const FW: u32 = 128;
const FH: u32 = 32;

/// A frame whose zone [16, 8, 96, 16] has background `level` and the code of `id`.
fn frame_with(level: u8, id: usize) -> Vec<u8> {
    let mut v = vec![0u8; (FW * FH * 4) as usize];
    for y in 8..24u32 {
        for x in 16..112u32 {
            let lx = x - 16;
            let cell = lx.checked_sub(8).map(|d| d / 6);
            let white = id > 0
                && (12..20).contains(&y)
                && match cell {
                    Some(0) => true,
                    Some(k) if (2..9).contains(&k) => id >> (k - 2) & 1 == 1,
                    _ => false,
                };
            let p = if white { 255 } else { level };
            let i = ((y * FW + x) * 4) as usize;
            v[i..i + 4].copy_from_slice(&[p, p, p, 255]);
        }
    }
    v
}

fn zone(kind: ZoneKind) -> Zone {
    Zone::new("z", kind, Rect::new(16.0, 8.0, 96.0, 16.0))
}

/// script: (text, busy). Returns (time, text) of every event (time -1 for `finish`).
fn run(kind: ZoneKind, dt: f64, script: &[(&str, bool)]) -> Vec<(f64, String)> {
    let texts = Rc::new(RefCell::new(vec![String::new()]));
    let mut ocr = Coded { texts: texts.clone() };
    let mut p = Pipeline::new(Config::default(), &[zone(kind)]);
    let mut out = Vec::new();
    let mut lvl = 40u8;
    let mut prev = String::from("#none#");
    let mut last = 0.0;
    for (i, (s, busy)) in script.iter().enumerate() {
        if s.trim_end() != prev.trim_end() || *busy {
            lvl = if s.is_empty() {
                if lvl == 0 { 20 } else { 0 }
            } else {
                match lvl {
                    60 => 100,
                    100 => 140,
                    140 => 180,
                    _ => 60,
                }
            };
        }
        prev = s.to_string();
        let id = if s.is_empty() {
            0
        } else {
            let mut t = texts.borrow_mut();
            t.iter().position(|x| x == s).unwrap_or_else(|| {
                t.push(s.to_string());
                t.len() - 1
            })
        };
        assert!(id < 128, "too many texts for the code");
        let px = frame_with(lvl, id);
        let t = i as f64 * dt;
        last = t;
        let o = p.on_frame(&Frame::new(t, FW, FH, &px), &mut ocr);
        assert!(o.errors.is_empty());
        out.extend(o.events.into_iter().map(|e| (e.t_ms.round(), e.text)));
    }
    out.extend(p.finish(last, &mut ocr).events.into_iter().map(|e| (-1.0, e.text)));
    out
}

fn rep(s: &str, n: usize, busy: bool) -> Vec<(&str, bool)> {
    vec![(s, busy); n]
}

fn texts(ev: &[(f64, String)]) -> Vec<String> {
    ev.iter().map(|e| e.1.clone()).collect()
}

#[test]
fn a_similar_line_after_a_spoken_one_is_a_new_event() {
    for kind in [ZoneKind::Notification, ZoneKind::Dialogue] {
        for busy in [false, true] {
            for (a, b) in [
                ("Picked up Iron Ore", "Picked up Iron Rod"),
                ("North gate opened", "South gate opened"),
                ("I can do that.", "I can not do that."),
                ("Stay close.", "Stay close to the wall and keep quiet."),
                ("Kael", "Kaelen"),
            ] {
                // In toast zones a line that only gains words is one line (OCR first caught a
                // part of the row), unless the gap was long enough to end the first line.
                let grows = dr_core::text::adds_only(&normalize(a), &normalize(b));
                for gap in [0usize, 2, 4, 6, 12] {
                    let mut s = rep(a, 30, busy);
                    s.extend(rep("", gap, busy));
                    s.extend(rep(b, 60, busy));
                    let ev = run(kind, 1000.0 / 30.0, &s);
                    let want = if kind == ZoneKind::Notification && grows && gap < 10 { vec![a] } else { vec![a, b] };
                    assert_eq!(texts(&ev), want, "{kind:?} busy={busy} gap={gap}");
                }
            }
        }
    }
}

fn typed(line: &str, cps: f64, fps: f64, hold_frames: usize) -> Vec<String> {
    let n = line.chars().count();
    let frames = (n as f64 / cps * fps).ceil() as usize + hold_frames;
    (0..frames)
        .map(|i| {
            let k = ((i as f64 / fps * cps) as usize + 1).min(n);
            line.chars().take(k).collect()
        })
        .collect()
}

#[test]
fn typing_is_spoken_once_when_done_at_any_speed_and_frame_rate() {
    let line = "Go to the lighthouse and wait for me there until dawn.";
    for (cps, fps) in [(40.0, 30.0), (25.0, 30.0), (20.0, 30.0), (15.0, 30.0), (10.0, 30.0), (40.0, 60.0), (25.0, 60.0), (30.0, 120.0), (60.0, 144.0)] {
        let frames = typed(line, cps, fps, (fps * 2.0) as usize);
        for busy in [false, true] {
            let s: Vec<(&str, bool)> = frames.iter().map(|f| (f.as_str(), busy)).collect();
            let ev = run(ZoneKind::Dialogue, 1000.0 / fps, &s);
            assert_eq!(ev.len(), 1, "cps={cps} fps={fps} busy={busy}: {ev:?}");
            assert_eq!(normalize(&ev[0].1), normalize(line), "cps={cps} fps={fps} busy={busy}");
        }
    }
}

#[test]
fn one_bad_read_on_a_still_screen_does_not_repeat_the_line() {
    // A spoiled read that holds for 2 frames (66 ms) or less. Longer than the kind's gone time
    // (dialogue 100 ms) it is the line leaving and coming back, which is a new event by design
    // (prompts flicker like that).
    for kind in [ZoneKind::Dialogue, ZoneKind::Notification] {
        for lead in 12usize..34 {
            for glitch in [1usize, 2] {
                let mut s = rep("Hello there my friend", lead, false);
                s.extend(rep("", glitch, false));
                s.extend(rep("Hello there my friend", 30, false));
                let ev = run(kind, 1000.0 / 30.0, &s);
                assert_eq!(ev.len(), 1, "{kind:?} lead={lead} glitch={glitch}: {ev:?}");
            }
        }
    }
}

#[test]
fn a_short_popup_on_a_still_screen_is_spoken() {
    // Shown for 3 frames (66 ms on still pixels) or more: the second clean-up agrees.
    for busy in [false, true] {
        for n in 3usize..=10 {
            let mut s = rep("", 10, busy);
            s.extend(rep("Quest updated", n, busy));
            s.extend(rep("", 30, busy));
            let ev = run(ZoneKind::Notification, 1000.0 / 30.0, &s);
            assert_eq!(texts(&ev), vec!["Quest updated"], "busy={busy} frames={n}");
        }
    }
}

#[test]
fn a_number_change_on_a_still_screen_is_spoken_soon() {
    for busy in [false, true] {
        for lead in [20usize, 26, 31] {
            let mut s = rep("+ 6 Iron Ore (27)", lead, busy);
            s.extend(rep("+ 12 Iron Ore (33)", 40, busy));
            let ev = run(ZoneKind::Notification, 1000.0 / 30.0, &s);
            assert_eq!(texts(&ev), vec!["+ 6 Iron Ore (27)", "+ 12 Iron Ore (33)"], "busy={busy} lead={lead}");
            let change = lead as f64 * 1000.0 / 30.0;
            assert!(ev[1].0 - change <= 300.0, "busy={busy} lead={lead}: spoken {:.0} ms late", ev[1].0 - change);
        }
    }
}

#[test]
fn speaker_trim_keeps_name_parts_and_best_prefers_the_latest() {
    assert_eq!(trim_edge_marks("Player 1"), "Player 1");
    assert_eq!(trim_edge_marks("Henry I"), "Henry I");
    assert_eq!(trim_edge_marks("Unit 1 !"), "Unit 1");
    assert_eq!(trim_edge_marks("I Juno"), "Juno");
    let mut tr = Tracker::new(Params { stable_ms: 5000.0, ..Config::default().track });
    for (i, s) in ["Quick toa", "Quick toast", "Quick toast now"].iter().enumerate() {
        tr.update(i as f64 * 33.0, &[Unit::new(s, None)], true);
    }
    assert_eq!(tr.tracks[0].best().raw, "Quick toast now");
}

#[test]
fn an_ocr_error_in_one_zone_keeps_the_events_of_the_others() {
    // Zone "b" is 20 px wide and always fails (its OCR image is (20 + 2 * 16 padding) * 2 = 104
    // px wide).
    let mut ocr = Fixed { text: "Hello there my friend".into(), calls: 0, fail_w: Some(104) };
    let zones = [zone(ZoneKind::Dialogue), Zone::new("b", ZoneKind::Speaker, Rect::new(0.0, 0.0, 20.0, 6.0))];
    let mut p = Pipeline::new(Config::default(), &zones);
    let mut said = Vec::new();
    let mut errors = 0;
    for i in 0..20 {
        let px = frame_with(if i % 2 == 0 { 100 } else { 140 }, 0);
        let o = p.on_frame(&Frame::new(i as f64 * 33.0, FW, FH, &px), &mut ocr);
        errors += o.errors.len();
        said.extend(o.events.into_iter().map(|e| (e.zone, e.kind, e.text)));
    }
    assert!(errors > 0);
    assert_eq!(said, vec![(Some("z".to_string()), Some(ZoneKind::Dialogue), "Hello there my friend".to_string())]);
}

#[test]
fn odd_zones_are_reported_and_never_panic() {
    let mut ocr = Fixed { text: "Hello there my friend".into(), calls: 0, fail_w: None };
    let zones = [
        Zone::new("zero", ZoneKind::Dialogue, Rect::new(5.0, 5.0, 0.0, 0.0)),
        Zone::new("out", ZoneKind::Notification, Rect::new(500.0, 500.0, 50.0, 20.0)),
        Zone::new("nan", ZoneKind::Notification, Rect::new(f32::NAN, 0.0, 20.0, 20.0)),
        Zone::new("z", ZoneKind::Dialogue, Rect::new(16.0, 8.0, 32.0, 16.0)),
    ];
    let mut p = Pipeline::new(Config::default(), &zones);
    let mut n = 0;
    for i in 0..20 {
        let px = frame_with(100, 0);
        n += p.on_frame(&Frame::new(i as f64 * 33.0, FW, FH, &px), &mut ocr).events.len();
    }
    assert_eq!(n, 1);
    assert_eq!(p.empty_zones(), ["zero", "out", "nan"]);
    // A zone drawn on a 64x32 frame follows the game to 128x64.
    let z = Zone { base: Some((FW, FH)), ..zone(ZoneKind::Dialogue) };
    let mut p = Pipeline::new(Config::default(), &[z]);
    let big = vec![90u8; (FW * 2 * FH * 2 * 4) as usize];
    p.on_frame(&Frame::new(0.0, FW * 2, FH * 2, &big), &mut ocr);
    assert!(p.empty_zones().is_empty());
    let tiny = vec![90u8; 4];
    p.on_frame(&Frame::new(33.0, 1, 1, &tiny), &mut ocr);
    let mut p = Pipeline::new(Config::default(), &[]);
    for (w, h) in [(1u32, 1u32), (3, 3), (64, 32), (65, 33)] {
        let px = vec![120u8; (w * h * 4) as usize];
        p.on_frame(&Frame::new(0.0, w, h, &px), &mut ocr);
    }
}

#[test]
fn a_read_budget_limits_ocr_per_frame() {
    let mut ocr = Fixed { text: "Hello there my friend".into(), calls: 0, fail_w: None };
    let zones = [Zone::new("a", ZoneKind::Dialogue, Rect::new(0.0, 0.0, 32.0, 16.0)), Zone::new("b", ZoneKind::Dialogue, Rect::new(32.0, 16.0, 32.0, 16.0))];
    let mut p = Pipeline::new(Config { max_reads: 1, ..Config::default() }, &zones);
    let mut by_zone = std::collections::BTreeSet::new();
    for i in 0..30u32 {
        let before = ocr.calls;
        // Every frame changes both zones.
        let px = vec![(i * 7 % 200) as u8 + 20; 64 * 32 * 4];
        let o = p.on_frame(&Frame::new(i as f64 * 33.0, 64, 32, &px), &mut ocr);
        // One zone read (one small-zone OCR pass) per frame.
        assert!(ocr.calls - before <= 1, "frame {i}: {} calls", ocr.calls - before);
        by_zone.extend(o.events.into_iter().filter_map(|e| e.zone));
    }
    // Both zones still get their turn.
    assert_eq!(by_zone.len(), 2);
}




/// Reads text only from black and white images (as if the plain clean-up failed).
struct OnlyBw;

impl Ocr for OnlyBw {
    fn recognize(&mut self, w: u32, h: u32, rgba: &[u8]) -> Result<Vec<OcrLine>, String> {
        let bw = rgba.chunks(4).all(|p| p[0] == 0 || p[0] == 255);
        let ink = rgba.chunks(4).any(|p| p[0] != rgba[0]);
        Ok(if bw && ink { vec![OcrLine { text: "Hold the line!".into(), rect: Rect::new(0.0, h as f32 * 0.4, w as f32, h as f32 * 0.2) }] } else { vec![] })
    }
}

#[test]
fn a_still_line_the_first_clean_up_misses_is_read_by_the_second() {
    // A tall zone (100 px) on a still screen: the first pass reads nothing, so once the pixels
    // stay still the zone is read again with the black and white clean-up.
    let (w, h) = (200u32, 140u32);
    let mut px = vec![60u8; (w * h * 4) as usize];
    for y in 60..80 {
        for x in (40..160).step_by(3) {
            let i = ((y * w + x) * 4) as usize;
            px[i..i + 3].copy_from_slice(&[230, 230, 230]);
        }
    }
    let mut p = Pipeline::new(Config::default(), &[Zone::new("d", ZoneKind::Dialogue, Rect::new(10.0, 20.0, 180.0, 100.0))]);
    let mut said = Vec::new();
    for i in 0..30 {
        said.extend(p.on_frame(&Frame::new(i as f64 * 33.3, w, h, &px), &mut OnlyBw).events.into_iter().map(|e| e.text));
    }
    assert_eq!(said, vec!["Hold the line!"]);
}
