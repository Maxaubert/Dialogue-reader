//! Manual mode (`dr_core::Manual`): nothing is read without a key press, a press reads every
//! zone once, Ignore zones and the frame outside the zones are never read, an empty press repeats
//! the last lines, and a toast that faded just before the press is read from the ring.
//!
//! The fake OCR reads a bar code (a start mark and 7 bit cells, white on grey) in the middle row
//! of the image it gets, like the one in `scenarios.rs`, and logs the size of every image.

use dr_core::{Config, Frame, Manual, ManualConfig, Ocr, OcrLine, Rect, Zone, ZoneKind};

const TEXTS: [&str; 8] = ["", "Quest updated", "Kael", "Stay close to the wall.", "Picked up Iron Ore", "Secret outside text", "Press E to open", "Map ready"];

#[derive(Default)]
struct Coded {
    sizes: Vec<(u32, u32)>,
}

impl Ocr for Coded {
    fn recognize(&mut self, w: u32, h: u32, rgba: &[u8]) -> Result<Vec<OcrLine>, String> {
        self.sizes.push((w, h));
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
        Ok(match TEXTS.get(id) {
            Some(t) if id > 0 => vec![OcrLine { text: t.to_string(), rect: Rect::new(0.0, 0.0, w as f32, h as f32) }],
            _ => vec![],
        })
    }
}

const FW: u32 = 256;
const FH: u32 = 128;
const BG: u8 = 40;

/// A 96x16 zone rect with its corner at (x, y).
fn r(x: f32, y: f32) -> Rect {
    Rect::new(x, y, 96.0, 16.0)
}

/// A frame: grey background with the code of each (corner, text id, alpha) drawn in a 96x16 box.
fn frame(marks: &[((u32, u32), usize, f32)]) -> Vec<u8> {
    let mut v = vec![0u8; (FW * FH * 4) as usize];
    for p in v.chunks_mut(4) {
        p.copy_from_slice(&[BG, BG, BG, 255]);
    }
    for &((x0, y0), id, alpha) in marks {
        let ink = (BG as f32 + (255.0 - BG as f32) * alpha).round() as u8;
        for y in y0 + 4..y0 + 12 {
            for x in x0..x0 + 96 {
                let cell = (x - x0).checked_sub(8).map(|d| d / 6);
                let white = id > 0
                    && match cell {
                        Some(0) => true,
                        Some(k) if (2..9).contains(&k) => id >> (k - 2) & 1 == 1,
                        _ => false,
                    };
                if white {
                    let i = ((y * FW + x) * 4) as usize;
                    v[i..i + 3].copy_from_slice(&[ink, ink, ink]);
                }
            }
        }
    }
    v
}

fn texts(p: &dr_core::Press) -> Vec<&str> {
    p.events.iter().map(|e| e.text.as_str()).collect()
}

fn manual(zones: &[Zone]) -> Manual {
    Manual::new(Config::default(), ManualConfig::default(), zones)
}

#[test]
fn nothing_is_read_without_a_press() {
    let mut m = manual(&[Zone::new("d", ZoneKind::Dialogue, r(16.0, 40.0))]);
    let mut ocr = Coded::default();
    let px = frame(&[((16, 40), 3, 1.0)]);
    for i in 0..90 {
        assert!(m.on_frame(&Frame::new(i as f64 * 33.3, FW, FH, &px), &mut ocr).is_none());
    }
    assert!(ocr.sizes.is_empty());
    assert_eq!(m.ocr_runs, 0);
}

#[test]
fn a_press_reads_every_zone_once_in_reading_order() {
    // Toast on top, dialogue in the middle, the speaker name below it: speaker comes first.
    let zones = [
        Zone::new("dlg", ZoneKind::Dialogue, r(16.0, 40.0)),
        Zone::new("spk", ZoneKind::Speaker, r(16.0, 70.0)),
        Zone::new("toast", ZoneKind::Notification, r(140.0, 4.0)),
        Zone::new("prompt", ZoneKind::Prompt, r(140.0, 100.0)),
    ];
    let mut m = manual(&zones);
    let mut ocr = Coded::default();
    let px = frame(&[((16, 40), 3, 1.0), ((16, 70), 2, 1.0), ((140, 4), 4, 1.0), ((140, 100), 6, 1.0)]);
    // The press flag is handled on the next frame.
    let f = Frame::new(0.0, FW, FH, &px);
    assert!(m.on_frame(&f, &mut ocr).is_none());
    m.press();
    let p = m.on_frame(&Frame::new(33.0, FW, FH, &px), &mut ocr).expect("a press");
    assert!(!m.pending());
    assert_eq!(texts(&p), vec!["Picked up Iron Ore", "Kael", "Stay close to the wall.", "Press E to open"]);
    assert!(!p.repeat && p.errors.is_empty());
    assert_eq!(p.events[1].kind, Some(ZoneKind::Speaker));
    assert!(p.events.iter().all(|e| e.t_ms == 33.0));
    // Nothing after the press until the next one.
    assert!(m.on_frame(&Frame::new(66.0, FW, FH, &px), &mut ocr).is_none());
}

#[test]
fn ignore_zones_and_the_frame_outside_the_zones_are_never_read() {
    let px = frame(&[((16, 40), 3, 1.0), ((140, 4), 5, 1.0), ((140, 100), 7, 1.0)]);
    // Only an Ignore zone, or no zone at all: a press reads nothing (no whole-frame fallback).
    for zones in [vec![Zone::new("hud", ZoneKind::Ignore, r(140.0, 4.0))], vec![]] {
        let mut m = manual(&zones);
        let mut ocr = Coded::default();
        let p = m.read_now(&Frame::new(0.0, FW, FH, &px), &mut ocr);
        assert!(p.events.is_empty() && !p.repeat);
        assert!(ocr.sizes.is_empty());
    }
    // A dialogue zone and an Ignore zone: only the dialogue zone is read, and every OCR image is
    // a crop of it (96 + 2 * 16 padding, at 2x = 256 wide), never the whole frame.
    let zones = [Zone::new("d", ZoneKind::Dialogue, r(16.0, 40.0)), Zone::new("hud", ZoneKind::Ignore, r(140.0, 4.0))];
    let mut m = manual(&zones);
    let mut ocr = Coded::default();
    let p = m.read_now(&Frame::new(0.0, FW, FH, &px), &mut ocr);
    assert_eq!(texts(&p), vec!["Stay close to the wall."]);
    assert!(!ocr.sizes.is_empty());
    assert!(ocr.sizes.iter().all(|&(w, h)| w <= 256 && h <= 96), "{:?}", ocr.sizes);
}

#[test]
fn a_repeated_press_reads_the_lines_again() {
    let mut m = manual(&[Zone::new("d", ZoneKind::Dialogue, r(16.0, 40.0))]);
    let mut ocr = Coded::default();
    let px = frame(&[((16, 40), 3, 1.0)]);
    for i in 0..3 {
        let p = m.read_now(&Frame::new(i as f64 * 500.0, FW, FH, &px), &mut ocr);
        assert_eq!(texts(&p), vec!["Stay close to the wall."], "press {i}");
        assert!(!p.repeat);
    }
}

#[test]
fn one_line_is_given_once_per_press() {
    // Two overlapping zones that both hold the same line.
    let zones = [Zone::new("a", ZoneKind::Dialogue, r(16.0, 40.0)), Zone::new("b", ZoneKind::Notification, r(16.0, 40.0))];
    let mut m = manual(&zones);
    let p = m.read_now(&Frame::new(0.0, FW, FH, &frame(&[((16, 40), 3, 1.0)])), &mut Coded::default());
    assert_eq!(texts(&p), vec!["Stay close to the wall."]);
}

#[test]
fn an_empty_press_repeats_the_last_lines() {
    let zones = [Zone::new("spk", ZoneKind::Speaker, r(16.0, 20.0)), Zone::new("d", ZoneKind::Dialogue, r(16.0, 40.0))];
    let mut m = manual(&zones);
    let mut ocr = Coded::default();
    let empty = frame(&[]);
    // No history yet: nothing.
    let p = m.read_now(&Frame::new(0.0, FW, FH, &empty), &mut ocr);
    assert!(p.events.is_empty() && !p.repeat);
    let full = frame(&[((16, 20), 2, 1.0), ((16, 40), 3, 1.0)]);
    let p = m.read_now(&Frame::new(1000.0, FW, FH, &full), &mut ocr);
    assert_eq!(texts(&p), vec!["Kael", "Stay close to the wall."]);
    // The box closed: the last lines again, flagged, at the new press time.
    let p = m.read_now(&Frame::new(5000.0, FW, FH, &empty), &mut ocr);
    assert!(p.repeat);
    assert_eq!(texts(&p), vec!["Kael", "Stay close to the wall."]);
    assert!(p.events.iter().all(|e| e.t_ms == 5000.0));
    // A new line replaces the history.
    let p = m.read_now(&Frame::new(6000.0, FW, FH, &frame(&[((16, 40), 7, 1.0)])), &mut ocr);
    assert_eq!(texts(&p), vec!["Map ready"]);
    let p = m.read_now(&Frame::new(7000.0, FW, FH, &empty), &mut ocr);
    assert!(p.repeat);
    assert_eq!(texts(&p), vec!["Map ready"]);
}

#[test]
fn a_toast_that_faded_just_before_the_press_is_read_from_the_ring() {
    // The toast is shown for 300 ms, fades out over 200 ms, and the key is pressed as it ends.
    let fade = |alpha: f32| frame(&[((140, 4), 1, alpha)]);
    let run = |mcfg: ManualConfig| {
        let mut m = Manual::new(Config::default(), mcfg, &[Zone::new("toast", ZoneKind::Notification, r(140.0, 4.0))]);
        let mut ocr = Coded::default();
        let mut t = 0.0;
        for i in 0..15 {
            let a = if i < 9 { 1.0 } else { 1.0 - (i - 8) as f32 / 6.0 };
            assert!(m.on_frame(&Frame::new(t, FW, FH, &fade(a)), &mut ocr).is_none());
            t += 33.3;
        }
        assert!(ocr.sizes.is_empty(), "no OCR before the press");
        m.press();
        let p = m.on_frame(&Frame::new(t, FW, FH, &fade(0.0)), &mut ocr).expect("a press");
        texts(&p).iter().map(|s| s.to_string()).collect::<Vec<_>>()
    };
    assert_eq!(run(ManualConfig::default()), vec!["Quest updated"]);
    // Without the ring the faded toast is lost.
    assert!(run(ManualConfig { look_ms: 0.0, ..ManualConfig::default() }).is_empty());
}

#[test]
fn a_clear_current_line_is_not_mixed_with_an_older_ring_line() {
    // Dialogue changed just before the press: the old (sharper) line is not read out.
    let mut m = manual(&[Zone::new("d", ZoneKind::Dialogue, r(16.0, 40.0))]);
    let mut ocr = Coded::default();
    for i in 0..10 {
        m.observe(&Frame::new(i as f64 * 33.3, FW, FH, &frame(&[((16, 40), 3, 1.0)])));
    }
    let p = m.read_now(&Frame::new(340.0, FW, FH, &frame(&[((16, 40), 7, 0.6)])), &mut ocr);
    assert_eq!(texts(&p), vec!["Map ready"]);
}

#[test]
fn with_prompts_off_a_prompt_zone_is_read_for_the_log_only() {
    let zones = [Zone::new("dlg", ZoneKind::Dialogue, r(16.0, 40.0)), Zone::new("prompt", ZoneKind::Prompt, r(140.0, 100.0))];
    let cfg = Config { read_prompts: false, ..Config::default() };
    let mut m = Manual::new(cfg, ManualConfig::default(), &zones);
    let mut ocr = Coded::default();
    let px = frame(&[((16, 40), 3, 1.0), ((140, 100), 6, 1.0)]);
    let p = m.read_now(&Frame::new(0.0, FW, FH, &px), &mut ocr);
    assert_eq!(texts(&p), vec!["Stay close to the wall."]);
    assert_eq!(p.skipped.len(), 1);
    assert_eq!(p.skipped[0].line.text, "Press E to open");
    assert_eq!(p.skipped[0].reason, dr_core::SkipReason::PromptsOff);
    // only the prompt in view: nothing to speak, and no repeat of a skipped line later
    let px = frame(&[((140, 100), 6, 1.0)]);
    let p = m.read_now(&Frame::new(100.0, FW, FH, &px), &mut ocr);
    assert_eq!(texts(&p), vec!["Stay close to the wall."], "the last spoken lines are repeated");
    assert!(p.repeat);
    assert_eq!(p.skipped[0].line.text, "Press E to open");
}
