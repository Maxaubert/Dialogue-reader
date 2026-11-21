//! Plays frames to a reader under the ideal or simulated clock.

use crate::model::{Layout, RecordedScenario, Scenario, TextItem};
use crate::reader::{FrameRef, ReadEvent, Reader, ReaderContext};
use crate::render::SceneRenderer;
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Clock {
    /// Every frame delivered; processing time ignored.
    Ideal,
    /// Frames that arrive while the reader is busy are skipped; events are late by the busy time.
    Simulated,
    /// Real window and capture (`arena live`). Only set by live mode, never accepted by `run`.
    Live,
}

impl Clock {
    pub fn parse(s: &str) -> Result<Self> {
        Ok(match s {
            "ideal" => Clock::Ideal,
            "simulated" => Clock::Simulated,
            _ => bail!("unknown clock {s:?} (ideal, simulated)"),
        })
    }
    pub fn name(self) -> &'static str {
        match self {
            Clock::Ideal => "ideal",
            Clock::Simulated => "simulated",
            Clock::Live => "live",
        }
    }
}

/// Frames of one scene, rendered or loaded on demand (skipped frames are never produced).
pub trait FrameSource {
    fn frame_count(&self) -> usize;
    fn time_ms(&self, i: usize) -> f64;
    fn size(&self) -> (u32, u32);
    fn produce(&mut self, i: usize, out: &mut Vec<u8>) -> Result<()>;
}

pub struct SceneSource {
    pub renderer: SceneRenderer,
}

impl FrameSource for SceneSource {
    fn frame_count(&self) -> usize {
        self.renderer.scenario.frame_count()
    }
    fn time_ms(&self, i: usize) -> f64 {
        self.renderer.scenario.frame_time_ms(i)
    }
    fn size(&self) -> (u32, u32) {
        (self.renderer.scenario.width, self.renderer.scenario.height)
    }
    fn produce(&mut self, i: usize, out: &mut Vec<u8>) -> Result<()> {
        let t = self.time_ms(i);
        self.renderer.render(t, out)
    }
}

/// Recorded frames: `(t_ms, path)` sorted by time.
pub struct RecordedSource {
    pub frames: Vec<(f64, PathBuf)>,
    pub width: u32,
    pub height: u32,
}

/// File extensions a recorded frame may have (the importer writes `.jpg`, the recorder `.png`).
pub const RECORDED_FRAME_EXTS: [&str; 3] = [".png", ".jpg", ".jpeg"];

/// Parses the signed ms offset of a recorder frame name, e.g. `f012_-00450ms.png` gives -450.
pub fn parse_offset_ms(name: &str) -> Option<f64> {
    let stem = RECORDED_FRAME_EXTS
        .iter()
        .chain([".qoi"].iter())
        .find_map(|ext| name.strip_suffix(ext))
        .unwrap_or(name);
    let body = stem.strip_suffix("ms")?;
    let (_, off) = body.rsplit_once('_')?;
    off.parse::<i64>().ok().map(|v| v as f64)
}

impl RecordedSource {
    pub fn open(dir: &Path) -> Result<Self> {
        let mut frames: Vec<(f64, PathBuf)> = std::fs::read_dir(dir)
            .with_context(|| format!("read frames {}", dir.display()))?
            .flatten()
            .filter_map(|e| {
                let name = e.file_name().to_string_lossy().to_string();
                if !RECORDED_FRAME_EXTS.iter().any(|ext| name.ends_with(ext)) {
                    return None;
                }
                parse_offset_ms(&name).map(|t| (t, e.path()))
            })
            .collect();
        frames.sort_by(|a, b| a.0.total_cmp(&b.0));
        let first = frames.first().with_context(|| format!("no fNNN_+OFFSETms.png/.jpg frames in {}", dir.display()))?;
        let (width, height) = image::image_dimensions(&first.1)?;
        Ok(RecordedSource { frames, width, height })
    }
}

impl FrameSource for RecordedSource {
    fn frame_count(&self) -> usize {
        self.frames.len()
    }
    fn time_ms(&self, i: usize) -> f64 {
        self.frames[i].0
    }
    fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }
    fn produce(&mut self, i: usize, out: &mut Vec<u8>) -> Result<()> {
        let img = image::open(&self.frames[i].1)?.to_rgba8();
        let (w, h) = img.dimensions();
        let raw = img.into_raw();
        *out = if (w, h) == (self.width, self.height) {
            raw
        } else {
            crate::background::resize_bilinear(&raw, w, h, self.width, self.height)
        };
        Ok(())
    }
}

/// How processing time is charged. Tests use a fixed cost so results do not depend on the PC.
pub enum Cost<'a> {
    Wall,
    Custom(&'a mut dyn FnMut(usize, f64) -> f64),
}

/// Result of playing one scene to one reader. Saved as `events/<id>.json`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ScenarioRun {
    pub scenario_id: String,
    pub reader: String,
    pub clock: Clock,
    pub events: Vec<ReadEvent>,
    /// Charged processing time of each delivered frame, ms.
    pub frame_costs_ms: Vec<f64>,
    pub frames_total: usize,
    pub frames_processed: usize,
    /// `model::truth_fingerprint` of the answer key the run was made for (empty in old runs).
    #[serde(default)]
    pub truth_fingerprint: String,
    /// The `--reader-opt` options of the run.
    #[serde(default)]
    pub reader_options: BTreeMap<String, String>,
}

/// Plays all frames of `src` to `reader`.
///
/// The simulated clock charges only the wall time spent inside `on_frame`, so it is correct only
/// for readers that finish their work there (see the `Reader` trait).
pub fn play(src: &mut dyn FrameSource, reader: &mut dyn Reader, layout: &Layout, clock: Clock, mut cost: Cost) -> Result<(Vec<ReadEvent>, Vec<f64>, usize)> {
    if clock == Clock::Live {
        bail!("the live clock is only for `arena live`");
    }
    reader.setup(layout);
    let (w, h) = src.size();
    let mut buf = Vec::new();
    let mut events = Vec::new();
    let mut costs = Vec::new();
    let mut busy_until = f64::NEG_INFINITY;
    let mut processed = 0usize;
    let mut last_t = 0.0;
    for i in 0..src.frame_count() {
        let t = src.time_ms(i);
        last_t = t;
        if clock == Clock::Simulated && t < busy_until {
            continue;
        }
        src.produce(i, &mut buf)?;
        let frame = FrameRef { t_ms: t, width: w, height: h, rgba: &buf };
        let start = Instant::now();
        let mut evs = reader.on_frame(&frame);
        let wall = start.elapsed().as_secs_f64() * 1000.0;
        let c = match &mut cost {
            Cost::Wall => wall,
            Cost::Custom(f) => f(processed, wall),
        };
        processed += 1;
        costs.push(c);
        let ev_t = match clock {
            Clock::Ideal | Clock::Live => t,
            Clock::Simulated => {
                busy_until = t + c;
                t + c
            }
        };
        for e in &mut evs {
            e.t_ms = ev_t;
        }
        events.extend(evs);
    }
    events.extend(reader.finish(last_t));
    Ok((events, costs, processed))
}

pub fn reader_context(id: &str, w: u32, h: u32, layout: &Layout, truth: &[TextItem], options: &BTreeMap<String, String>) -> ReaderContext {
    ReaderContext {
        scenario_id: id.to_string(),
        width: w,
        height: h,
        layout: layout.clone(),
        truth: truth.to_vec(),
        options: options.clone(),
    }
}

pub fn run_scenario(s: &Scenario, reader_name: &str, clock: Clock, options: &BTreeMap<String, String>) -> Result<ScenarioRun> {
    let ctx = reader_context(&s.id, s.width, s.height, &s.layout, &s.items, options);
    let mut reader = crate::readers::create(reader_name, &ctx)?;
    let mut src = SceneSource { renderer: SceneRenderer::new(s.clone())? };
    let frames_total = src.frame_count();
    let (events, frame_costs_ms, frames_processed) = play(&mut src, reader.as_mut(), &s.layout, clock, Cost::Wall)
        .with_context(|| format!("scenario {}", s.id))?;
    Ok(ScenarioRun {
        scenario_id: s.id.clone(),
        reader: reader_name.to_string(),
        clock,
        events,
        frame_costs_ms,
        frames_total,
        frames_processed,
        truth_fingerprint: crate::model::truth_fingerprint(&s.items),
        reader_options: options.clone(),
    })
}

pub fn run_recorded(path: &Path, r: &RecordedScenario, reader_name: &str, clock: Clock, options: &BTreeMap<String, String>) -> Result<ScenarioRun> {
    let base = path.parent().unwrap_or(Path::new("."));
    let mut src = RecordedSource::open(&base.join(&r.frames_dir))?;
    let ctx = reader_context(&r.id, src.width, src.height, &r.layout, &r.truth, options);
    let mut reader = crate::readers::create(reader_name, &ctx)?;
    let frames_total = src.frame_count();
    let (events, frame_costs_ms, frames_processed) = play(&mut src, reader.as_mut(), &r.layout, clock, Cost::Wall)?;
    Ok(ScenarioRun {
        scenario_id: r.id.clone(),
        reader: reader_name.to_string(),
        clock,
        events,
        frame_costs_ms,
        frames_total,
        frames_processed,
        truth_fingerprint: crate::model::truth_fingerprint(&r.truth),
        reader_options: options.clone(),
    })
}

/// Runs `f` over `n` jobs on `threads` threads, keeping input order in the output.
pub fn parallel<T: Send>(n: usize, threads: usize, f: impl Fn(usize) -> Result<T> + Sync) -> Result<Vec<T>> {
    let next = AtomicUsize::new(0);
    let out: Mutex<Vec<Option<Result<T>>>> = Mutex::new((0..n).map(|_| None).collect());
    std::thread::scope(|sc| {
        for _ in 0..threads.clamp(1, n.max(1)) {
            sc.spawn(|| {
                loop {
                    let i = next.fetch_add(1, Ordering::SeqCst);
                    if i >= n {
                        break;
                    }
                    let r = f(i);
                    out.lock().unwrap()[i] = Some(r);
                }
            });
        }
    });
    out.into_inner().unwrap().into_iter().map(|r| r.expect("job ran")).collect()
}

/// Loads every `*.recorded.json` under `path` (or the file itself).
pub fn load_recorded(path: &Path) -> Result<Vec<(PathBuf, RecordedScenario)>> {
    let files: Vec<PathBuf> = if path.is_file() {
        vec![path.to_path_buf()]
    } else {
        let mut v: Vec<PathBuf> = std::fs::read_dir(path)
            .with_context(|| format!("read {}", path.display()))?
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.to_string_lossy().ends_with(".recorded.json"))
            .collect();
        v.sort();
        v
    };
    if files.is_empty() {
        bail!("no *.recorded.json in {}", path.display());
    }
    files
        .into_iter()
        .map(|p| {
            let s = std::fs::read_to_string(&p)?;
            let r: RecordedScenario = serde_json::from_str(&s).with_context(|| format!("parse {}", p.display()))?;
            Ok((p, r))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 10 frames, 100 ms apart, 2x2 black.
    struct Fake;
    impl FrameSource for Fake {
        fn frame_count(&self) -> usize {
            10
        }
        fn time_ms(&self, i: usize) -> f64 {
            i as f64 * 100.0
        }
        fn size(&self) -> (u32, u32) {
            (2, 2)
        }
        fn produce(&mut self, _i: usize, out: &mut Vec<u8>) -> Result<()> {
            out.clear();
            out.resize(16, 0);
            Ok(())
        }
    }

    /// Records the frame times it saw and emits one event per frame.
    struct Echo(Vec<f64>);
    impl Reader for Echo {
        fn name(&self) -> &str {
            "echo"
        }
        fn on_frame(&mut self, f: &FrameRef) -> Vec<ReadEvent> {
            self.0.push(f.t_ms);
            vec![ReadEvent { t_ms: -1.0, text: format!("{}", f.t_ms), rect: None, zone: None }]
        }
    }

    #[test]
    fn ideal_clock_delivers_every_frame() {
        let mut r = Echo(vec![]);
        let mut fixed = |_: usize, _: f64| 250.0;
        let (ev, costs, n) = play(&mut Fake, &mut r, &Layout::default(), Clock::Ideal, Cost::Custom(&mut fixed)).unwrap();
        assert_eq!(n, 10);
        assert_eq!(costs.len(), 10);
        assert_eq!(r.0.len(), 10);
        assert_eq!(ev[3].t_ms, 300.0);
    }

    #[test]
    fn simulated_clock_skips_busy_frames() {
        let mut r = Echo(vec![]);
        // 250 ms per frame: frame 0 busy until 250, so 100 and 200 skipped; next is 300, etc.
        let mut fixed = |_: usize, _: f64| 250.0;
        let (ev, _, n) = play(&mut Fake, &mut r, &Layout::default(), Clock::Simulated, Cost::Custom(&mut fixed)).unwrap();
        assert_eq!(r.0, vec![0.0, 300.0, 600.0, 900.0]);
        assert_eq!(n, 4);
        assert_eq!(ev.iter().map(|e| e.t_ms).collect::<Vec<_>>(), vec![250.0, 550.0, 850.0, 1150.0]);
    }

    #[test]
    fn simulated_clock_with_cheap_reader_sees_all() {
        let mut r = Echo(vec![]);
        let mut fixed = |i: usize, _: f64| if i == 2 { 150.0 } else { 5.0 };
        play(&mut Fake, &mut r, &Layout::default(), Clock::Simulated, Cost::Custom(&mut fixed)).unwrap();
        // Frame 200 costs 150 ms, so frame 300 is skipped.
        assert_eq!(r.0, vec![0.0, 100.0, 200.0, 400.0, 500.0, 600.0, 700.0, 800.0, 900.0]);
    }

    #[test]
    fn parses_recorder_names() {
        assert_eq!(parse_offset_ms("f000_-01500ms.png"), Some(-1500.0));
        assert_eq!(parse_offset_ms("f017_+00250ms.png"), Some(250.0));
        assert_eq!(parse_offset_ms("f003_+01234ms.jpg"), Some(1234.0));
        assert_eq!(parse_offset_ms("frame.png"), None);
    }
}
