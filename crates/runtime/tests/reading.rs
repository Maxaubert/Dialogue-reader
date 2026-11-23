//! The reading loop headless (app-plan.md M3b): an arena scene drawn frame by frame -> the runtime (reader thread,
//! real WinRT OCR) -> the speech thread with a fake voice and a null sink. Nothing shows, nothing plays, no key
//! hook is installed, no window is captured. Skips (with a message) when the en-US OCR pack is missing.
//!
//! The source is scripted and runs in lockstep with the reader on scene time (40 ms steps), so the test does not
//! depend on how fast OCR is. A gate holds it at a scene time while the test checks or presses.

#![cfg(windows)]

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use arena::model::{
    Anim, Background, ItemKind, Layout, Panel, ProceduralKind, Rect, Scenario, Style, TextItem,
    Zone as AZone, ZoneKind as AKind,
};
use arena::render::SceneRenderer;
use dr_core::{Ocr, OcrLine, SkipReason};
use dr_runtime::capture::{FrameSource, Gate, ScriptSource};
use dr_runtime::profile::Mode;
use dr_runtime::reader::{ReadCfg, ReaderEv};
use dr_runtime::runtime::{Cmd, Ev, Parts, RunCfg, Runtime, Status, Target};
use dr_runtime::speech::{self, NullSink, Speech, VoiceSettings};
use dr_tts::FakeTts;

const STEP: f64 = 40.0;
const W: u32 = 1280;
const H: u32 = 720;
const SPEAKER: &str = "Arthur";
const LINE_1: &str = "We need to get these horses moving.";
const LINE_2: &str = "The snow is coming down fast tonight.";
const PROMPT: &str = "Press E to open the door";

fn ocr_ready() -> bool {
    if dr_ocr::WinOcr::has_language("en-US") {
        return true;
    }
    eprintln!(
        "SKIPPED: no en-US OCR language pack on this PC (Settings > Time & language > Language)"
    );
    false
}

fn style() -> Style {
    Style {
        font: "Segoe UI".into(),
        size_px: 30.0,
        color: [240, 240, 240, 255],
        outline: None,
        shadow: None,
        panel: Some(Panel {
            color: [10, 10, 14, 255],
            alpha: 0.85,
            padding: 10.0,
            radius: 4.0,
        }),
        bold: false,
        italic: false,
        align: Default::default(),
        line_spacing: 1.25,
    }
}

fn item(id: &str, text: &str, kind: ItemKind, rect: Rect, start_ms: f64, end_ms: f64) -> TextItem {
    TextItem {
        id: id.into(),
        text: text.into(),
        kind,
        should_read: kind != ItemKind::Prompt,
        rect,
        start_ms,
        end_ms,
        style: style(),
        anim: Anim::default(),
    }
}

/// A speaker and two dialogue lines one after the other, and a prompt that stays.
fn scene() -> Scenario {
    let speaker = Rect::new(200.0, 470.0, 300.0, 56.0);
    let dialogue = Rect::new(200.0, 540.0, 880.0, 64.0);
    let prompt = Rect::new(440.0, 300.0, 400.0, 56.0);
    Scenario {
        id: "m3b-reading".into(),
        seed: 1,
        genres: vec![],
        width: W,
        height: H,
        fps: 25.0,
        duration_ms: 9_000.0,
        background: Background::Procedural {
            kind: ProceduralKind::Gradient,
            seed: 7,
        },
        layout: Layout {
            zones: vec![
                AZone {
                    name: "Speaker".into(),
                    kind: AKind::Speaker,
                    rect: speaker.inflate(8.0),
                },
                AZone {
                    name: "Subtitles".into(),
                    kind: AKind::Dialogue,
                    rect: dialogue.inflate(8.0),
                },
                AZone {
                    name: "Prompt".into(),
                    kind: AKind::Prompt,
                    rect: prompt.inflate(8.0),
                },
            ],
        },
        items: vec![
            item("s1", SPEAKER, ItemKind::Speaker, speaker, 500.0, 4_000.0),
            item("d1", LINE_1, ItemKind::Dialogue, dialogue, 500.0, 4_000.0),
            item("d2", LINE_2, ItemKind::Dialogue, dialogue, 5_000.0, 8_500.0),
            item("p1", PROMPT, ItemKind::Prompt, prompt, 0.0, 9_000.0),
        ],
    }
}

fn zones(s: &Scenario) -> Vec<dr_core::Zone> {
    arena::readers::a1::map_zones(&s.layout)
}

/// Frames of the scene; `still_after`: from this time on nothing new comes (the screen does not change).
fn source(s: Scenario, still_after: f64, gate: Option<Arc<Gate>>) -> Box<dyn FrameSource> {
    let end = s.duration_ms;
    // the renderer is not Send: it draws on its own thread, one frame per asked time
    let (ask, asked) = std::sync::mpsc::channel::<f64>();
    let (give, given) = std::sync::mpsc::channel::<Vec<u8>>();
    std::thread::spawn(move || {
        let mut r = SceneRenderer::new(s).expect("scene renders");
        for t in asked {
            let mut buf = Vec::new();
            r.render(t, &mut buf).expect("frame");
            if give.send(buf).is_err() {
                return;
            }
        }
    });
    let mut sent = false;
    let render: dr_runtime::capture::Render = Box::new(move |t| {
        if sent && t > still_after {
            return None;
        }
        sent = true;
        ask.send(t).ok()?;
        Some((W, H, given.recv().ok()?))
    });
    let src = ScriptSource::new(render, STEP, end);
    Box::new(match gate {
        Some(g) => src.with_gate(g),
        None => src,
    })
}

/// WinRT OCR that counts its calls.
struct Counting(Box<dyn Ocr>, Arc<AtomicU64>);

impl Ocr for Counting {
    fn max_dim(&self) -> u32 {
        self.0.max_dim()
    }
    fn recognize(&mut self, w: u32, h: u32, rgba: &[u8]) -> Result<Vec<OcrLine>, String> {
        self.1.fetch_add(1, Ordering::Relaxed);
        self.0.recognize(w, h, rgba)
    }
}

struct Rig {
    rt: Runtime,
    speech: Speech,
    ocr_calls: Arc<AtomicU64>,
    /// Every speech event so far.
    heard: Vec<speech::Ev>,
    /// Every runtime event so far.
    seen: Vec<Ev>,
}

fn rig(src: Box<dyn FrameSource>, mode: Mode, zones: Vec<dr_core::Zone>, sink: NullSink) -> Rig {
    let speech = Speech::start(
        Box::new(FakeTts::new()),
        move || Box::new(sink),
        VoiceSettings::default(),
    );
    let calls = Arc::new(AtomicU64::new(0));
    let c = calls.clone();
    let src = Arc::new(Mutex::new(Some(src)));
    let parts = Parts {
        ocr: Arc::new(move |lang: &str| {
            let o = dr_ocr::WinOcr::new(lang)?;
            Ok(Box::new(Counting(Box::new(o), c.clone())) as Box<dyn Ocr>)
        }),
        source: Arc::new(move |_hwnd| {
            let s = src.lock().unwrap().take().ok_or("one source only")?;
            Ok((s, None))
        }),
        hooks: false,
        checks: false,
        speech: speech.sender(),
    };
    let mut rt = Runtime::new(parts);
    rt.send(Cmd::Start(Box::new(RunCfg {
        read: ReadCfg {
            zones,
            mode,
            read_prompts: false,
        },
        read_key: "Mouse 4".into(),
        menu_key: "F7".into(),
        ocr_lang: "en-US".into(),
    })));
    rt.send(Cmd::Window(Some(Target { hwnd: 1, pid: 0 })));
    Rig {
        rt,
        speech,
        ocr_calls: calls,
        heard: Vec::new(),
        seen: Vec::new(),
    }
}

impl Rig {
    fn pump(&mut self) {
        self.heard.extend(self.speech.events());
        self.seen.extend(self.rt.events());
    }

    /// Pumps until `done` or the limit.
    fn until(&mut self, limit: Duration, done: impl Fn(&Rig) -> bool) -> bool {
        let end = Instant::now() + limit;
        while Instant::now() < end {
            self.pump();
            if done(self) {
                return true;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        self.pump();
        done(self)
    }

    fn ended(&self) -> bool {
        self.seen
            .iter()
            .any(|e| matches!(e, Ev::Reader(ReaderEv::Ended(_))))
    }

    /// The texts that started to play, in order.
    fn spoken(&self) -> Vec<String> {
        self.heard
            .iter()
            .filter_map(|e| match e {
                speech::Ev::Started { item, .. } => Some(item.text.clone()),
                _ => None,
            })
            .collect()
    }

    fn skipped(&self) -> Vec<(String, SkipReason)> {
        self.seen
            .iter()
            .filter_map(|e| match e {
                Ev::Reader(ReaderEv::Skipped(v)) => Some(v.clone()),
                _ => None,
            })
            .flatten()
            .map(|(l, r)| (l.text, r))
            .collect()
    }
}

/// Lowercase letters and digits only, so OCR spacing and punctuation do not matter.
fn norm(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

/// `want` appear in `got` in this order (each one inside one spoken item, or across the joined ones).
fn in_order(got: &[String], want: &[&str]) -> bool {
    let all = norm(&got.join(" "));
    let mut at = 0;
    for w in want {
        match all[at..].find(&norm(w)) {
            Some(i) => at += i + norm(w).len(),
            None => return false,
        }
    }
    true
}

const LIMIT: Duration = Duration::from_secs(90);

#[test]
fn automatic_speaks_the_lines_in_order_and_logs_the_prompt() {
    if !ocr_ready() {
        return;
    }
    let s = scene();
    let z = zones(&s);
    let mut r = rig(
        source(s, f64::MAX, None),
        Mode::Auto,
        z,
        NullSink::instant(),
    );
    assert!(r.until(LIMIT, Rig::ended), "the scene did not end");
    assert!(r.until(Duration::from_secs(5), |r| r.spoken().len() >= 2));
    let spoken = r.spoken();
    assert!(
        in_order(&spoken, &[SPEAKER, LINE_1, LINE_2]),
        "spoken: {spoken:?}"
    );
    // the speaker and its line came in one read: one item
    assert!(
        spoken.iter().any(|t| norm(t).starts_with(&norm(SPEAKER))),
        "{spoken:?}"
    );
    // prompts off: the prompt is never spoken, but it is in the log
    assert!(!in_order(&spoken, &[PROMPT]), "{spoken:?}");
    let skipped = r.skipped();
    assert!(
        skipped
            .iter()
            .any(|(t, why)| *why == SkipReason::PromptsOff && norm(t).contains("openthedoor")),
        "{skipped:?}"
    );
    // the window "closed": the runtime lets the reader go and waits for the window
    r.rt.tick();
    assert_eq!(r.rt.status(), Status::NoWindow);
}

#[test]
fn a_line_that_appears_once_on_a_still_screen_is_spoken() {
    if !ocr_ready() {
        return;
    }
    // the speaker and line 1 show at 500 ms; one frame at 520 ms has them, then nothing new comes (WGC sends no
    // frame for a window that does not change)
    let s = scene();
    let z = zones(&s);
    let mut src_scene = s.clone();
    src_scene.duration_ms = 4_000.0;
    let mut r = rig(
        source(src_scene, 520.0, None),
        Mode::Auto,
        z,
        NullSink::instant(),
    );
    assert!(r.until(LIMIT, Rig::ended));
    assert!(r.until(Duration::from_secs(5), |r| !r.spoken().is_empty()));
    assert!(in_order(&r.spoken(), &[LINE_1]), "{:?}", r.spoken());
}

#[test]
fn manual_mode_reads_nothing_until_a_press() {
    if !ocr_ready() {
        return;
    }
    let s = scene();
    let z = zones(&s);
    let gate = Gate::new(2_000.0);
    let mut r = rig(
        source(s, f64::MAX, Some(gate.clone())),
        Mode::Manual,
        z,
        NullSink::instant(),
    );
    assert!(gate.wait_at(2_000.0, LIMIT));
    // let the reader go round a few times on the held frame
    std::thread::sleep(Duration::from_millis(300));
    r.pump();
    assert!(r.spoken().is_empty(), "{:?}", r.spoken());
    assert_eq!(
        r.ocr_calls.load(Ordering::Relaxed),
        0,
        "no OCR between presses"
    );
    // a press reads the screen as it is (the source is held: no new frame comes)
    r.rt.send(Cmd::ReadNow);
    assert!(
        r.until(LIMIT, |r| in_order(&r.spoken(), &[SPEAKER, LINE_1])),
        "{:?} {:?}",
        r.spoken(),
        r.seen
    );
    assert!(r.seen.iter().any(|e| matches!(
        e,
        Ev::Reader(ReaderEv::Pressed { lines, repeat: false }) if *lines >= 1
    )));
    // the prompt zone is read after the spoken ones, for the log
    assert!(r.until(Duration::from_secs(10), |r| {
        r.skipped()
            .iter()
            .any(|(_, why)| *why == SkipReason::PromptsOff)
    }));
    // line 2 shows later; it is not read without a press
    gate.open_until(7_000.0);
    assert!(gate.wait_at(7_000.0, LIMIT));
    std::thread::sleep(Duration::from_millis(200));
    r.pump();
    assert!(!in_order(&r.spoken(), &[LINE_2]));
    gate.open_until(f64::MAX);
}

#[test]
fn pause_stops_ocr_and_stop_clears_the_queue() {
    if !ocr_ready() {
        return;
    }
    let s = scene();
    let z = zones(&s);
    let gate = Gate::new(3_000.0);
    // sounds last their real length (fake voice: 60 ms per character), so lines wait in the queue
    let sink = NullSink::timed(1.0);
    let mut r = rig(source(s, f64::MAX, Some(gate.clone())), Mode::Auto, z, sink);
    assert!(gate.wait_at(3_000.0, LIMIT));
    assert!(r.until(LIMIT, |r| !r.spoken().is_empty()));
    // Stop: what plays is cut and the queue is empty
    r.speech.send(speech::Cmd::Stop);
    assert!(r.until(Duration::from_secs(5), |r| {
        r.heard.iter().any(|e| matches!(e, speech::Ev::Cut { .. }))
    }));
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(r.speech.status().queued, 0);
    assert_eq!(r.speech.status().speaking, None);
    // Pause: the reader goes on taking frames, but no OCR runs
    r.rt.send(Cmd::Pause(true));
    assert_eq!(r.rt.status(), Status::Paused);
    gate.open_until(3_400.0);
    assert!(gate.wait_at(3_400.0, LIMIT));
    std::thread::sleep(Duration::from_millis(100));
    let before = r.ocr_calls.load(Ordering::Relaxed);
    let heard = r.spoken().len();
    // line 2 comes and goes while paused
    gate.open_until(f64::MAX);
    assert!(r.until(LIMIT, Rig::ended));
    assert_eq!(
        r.ocr_calls.load(Ordering::Relaxed),
        before,
        "OCR ran while paused"
    );
    assert_eq!(r.spoken().len(), heard);
    assert!(!in_order(&r.spoken(), &[LINE_2]));
}
