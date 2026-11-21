//! Generate -> render -> run -> score, with the built-in readers. Procedural backgrounds only,
//! so it passes without arena-data.

use arena::background::BgTextMap;
use arena::generator::{GenOptions, SuiteKind, generate_suite};
use arena::runner::{Clock, run_scenario};
use arena::score::{build_report, score_scenario};
use std::collections::BTreeMap;

fn report(reader: &str, clock: Clock, n: usize) -> arena::score::Metrics {
    let suite = generate_suite(SuiteKind::Smoke, 3, &GenOptions::procedural_only());
    let opts = BTreeMap::new();
    let scored = suite
        .iter()
        .take(n)
        .map(|s| {
            let run = run_scenario(s, reader, clock, &opts).unwrap();
            score_scenario(&s.id, &s.genres, &s.items, &run, &BgTextMap::for_background(&s.background, s.width, s.height))
        })
        .collect();
    build_report("smoke", reader, "x", scored).totals
}

#[test]
fn oracle_scores_perfectly() {
    if arena::fonts::font("Segoe UI", false, false).is_none() {
        return; // no fonts on this machine
    }
    for clock in [Clock::Ideal, Clock::Simulated] {
        let m = report("oracle", clock, 2);
        assert!(m.items_should_read > 0);
        assert_eq!(m.recall, 1.0, "{clock:?}");
        assert_eq!(m.mean_cer, 0.0);
        assert_eq!(m.noise_reads + m.hallucinations + m.duplicates + m.background_reads, 0);
    }
}

#[test]
fn noop_scores_zero() {
    if arena::fonts::font("Segoe UI", false, false).is_none() {
        return;
    }
    let m = report("noop", Clock::Simulated, 2);
    assert!(m.items_should_read > 0);
    assert_eq!(m.recall, 0.0);
    assert_eq!(m.events_total, 0);
    assert_eq!(m.noise_reads + m.hallucinations, 0);
}

#[test]
fn recorded_set_runs_with_negative_times() {
    use arena::model::{Anim, Align, ItemKind, RecordedScenario, Rect, Style, TextItem};
    let dir = std::env::temp_dir().join(format!("arena-recorded-test-{}", std::process::id()));
    let frames = dir.join("frames");
    std::fs::create_dir_all(&frames).unwrap();
    let px = vec![40u8; 8 * 6 * 4];
    for name in ["f000_-00100ms.png", "f001_+00000ms.png", "f002_+00100ms.png"] {
        arena::render::save_png(&frames.join(name), 8, 6, &px).unwrap();
    }
    let style = Style {
        font: "Arial".into(),
        size_px: 20.0,
        color: [255; 4],
        outline: None,
        shadow: None,
        panel: None,
        bold: false,
        italic: false,
        align: Align::Left,
        line_spacing: 1.25,
    };
    let truth = vec![TextItem {
        id: "toast".into(),
        text: "+ 9 Leaves (9)".into(),
        kind: ItemKind::PickupToast,
        should_read: true,
        rect: Rect::new(0.0, 0.0, 8.0, 6.0),
        start_ms: -100.0,
        end_ms: 50.0,
        style,
        anim: Anim::default(),
    }];
    let rec = RecordedScenario {
        id: "mark-001".into(),
        frames_dir: "frames".into(),
        truth,
        layout: Default::default(),
        start_uncertain: vec![],
        end_uncertain: vec![],
    };
    std::fs::write(dir.join("mark-001.recorded.json"), serde_json::to_string(&rec).unwrap()).unwrap();

    let loaded = arena::runner::load_recorded(&dir).unwrap();
    assert_eq!(loaded.len(), 1);
    let (path, r) = &loaded[0];
    let opts = BTreeMap::new();
    for (reader, recall) in [("oracle", 1.0), ("noop", 0.0)] {
        let run = arena::runner::run_recorded(path, r, reader, Clock::Simulated, &opts).unwrap();
        assert_eq!(run.frames_total, 3);
        let s = score_scenario(&r.id, &[], &r.truth, &run, &BgTextMap::default());
        assert_eq!(s.score.metrics.recall, recall, "{reader}");
        assert_eq!(s.score.metrics.hallucinations, 0);
        if reader == "oracle" {
            assert_eq!(run.events[0].t_ms.floor(), -100.0);
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
}
