//! Command line: gen, render, run, score, plus the stub commands of the parallel agents.

use crate::background::BgTextMap;
use crate::generator::{self, GenOptions, SuiteKind};
use crate::model::TextItem;
use crate::render::{SceneRenderer, save_png};
use crate::runner::{self, Clock, ScenarioRun};
use crate::score::{self, Report, TimingFlags, Tolerance};
use crate::{live, ocr, readers, realworld};
use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

#[derive(Parser, Debug)]
#[command(name = "arena", about = "Plays fake game scenes to a reader and scores it")]
pub struct Cli {
    #[command(subcommand)]
    pub cmd: Cmd,
}

#[derive(Subcommand, Debug)]
pub enum Cmd {
    /// Generate a suite of scenarios
    Gen(GenArgs),
    /// Render a scenario to PNG frames for inspection
    Render(RenderArgs),
    /// Play a suite (or recorded set) to a reader and score it
    Run(RunArgs),
    /// Score saved events again (no reader run)
    Score(ScoreArgs),
    /// OCR engine bench on single-line crops (stub)
    OcrBench(ocr::Args),
    /// Play a scene in a real (off-screen) window, capture it with WGC and score the reader live
    Live(live::Args),
    /// Build the real-world recorded set from a recorder session (stub)
    RealworldImport(realworld::Args),
}

#[derive(clap::Args, Debug)]
pub struct GenArgs {
    /// smoke (8 scenes), standard (40) or hard (40)
    #[arg(long)]
    pub suite: String,
    #[arg(long, default_value_t = 1)]
    pub seed: u64,
    /// Output folder (default arena-data/suites/<suite>)
    #[arg(long)]
    pub out: Option<PathBuf>,
    /// Use procedural backgrounds only, even when arena-data/backgrounds exists
    #[arg(long)]
    pub procedural_only: bool,
}

#[derive(clap::Args, Debug)]
pub struct RenderArgs {
    #[arg(long)]
    pub scenario: PathBuf,
    #[arg(long)]
    pub out: PathBuf,
    /// Write every Nth frame
    #[arg(long, default_value_t = 1)]
    pub every: usize,
    #[arg(long)]
    pub from_ms: Option<f64>,
    #[arg(long)]
    pub to_ms: Option<f64>,
    /// Exact times to render instead, comma separated ms (e.g. 1200,3400)
    #[arg(long, value_delimiter = ',')]
    pub times: Vec<f64>,
}

#[derive(clap::Args, Debug)]
pub struct RunArgs {
    /// Suite folder (from `arena gen`)
    #[arg(long, conflicts_with = "recorded")]
    pub suite: Option<PathBuf>,
    /// Folder of *.recorded.json files (real-world set), or one such file
    #[arg(long)]
    pub recorded: Option<PathBuf>,
    #[arg(long, default_value = "oracle")]
    pub reader: String,
    /// simulated (default) or ideal
    #[arg(long, default_value = "simulated")]
    pub clock: String,
    /// Report folder (default arena-data/reports/<suite>-<reader>-<clock>)
    #[arg(long)]
    pub report: Option<PathBuf>,
    /// Earlier report.json; exit code 2 when recall, CER, noise or hallucinations got worse
    #[arg(long)]
    pub baseline: Option<PathBuf>,
    /// Scenes run in parallel (default: 1 for the simulated clock, 8 for ideal)
    #[arg(long)]
    pub jobs: Option<usize>,
    /// Reader option key=value (repeatable)
    #[arg(long = "reader-opt")]
    pub reader_opts: Vec<String>,
}

#[derive(clap::Args, Debug)]
pub struct ScoreArgs {
    #[arg(long, conflicts_with = "recorded")]
    pub suite: Option<PathBuf>,
    #[arg(long)]
    pub recorded: Option<PathBuf>,
    /// Report folder of an earlier run (holds events/)
    #[arg(long)]
    pub events: PathBuf,
    /// Where to write report.json/.md (default: the events report folder)
    #[arg(long)]
    pub out: Option<PathBuf>,
    #[arg(long)]
    pub baseline: Option<PathBuf>,
}

pub fn main() -> ExitCode {
    let cli = Cli::parse();
    match dispatch(cli.cmd) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("error: {e:#}");
            ExitCode::from(1)
        }
    }
}

fn dispatch(cmd: Cmd) -> Result<ExitCode> {
    match cmd {
        Cmd::Gen(a) => cmd_gen(&a).map(|_| ExitCode::SUCCESS),
        Cmd::Render(a) => cmd_render(&a).map(|_| ExitCode::SUCCESS),
        Cmd::Run(a) => cmd_run(&a),
        Cmd::Score(a) => cmd_score(&a),
        Cmd::OcrBench(a) => ocr::run(&a).map(|_| ExitCode::SUCCESS),
        Cmd::Live(a) => live::run(&a).map(|_| ExitCode::SUCCESS),
        Cmd::RealworldImport(a) => realworld::run(&a).map(|_| ExitCode::SUCCESS),
    }
}

fn cmd_gen(a: &GenArgs) -> Result<()> {
    let kind = SuiteKind::parse(&a.suite)?;
    let opts = if a.procedural_only { GenOptions::procedural_only() } else { GenOptions::from_pc() };
    let out = a.out.clone().unwrap_or_else(|| crate::data_dir().join("suites").join(kind.name()));
    let scenarios = generator::generate_suite(kind, a.seed, &opts);
    generator::write_suite(&out, kind.name(), a.seed, &scenarios)?;
    let items: usize = scenarios.iter().map(|s| s.items.len()).sum();
    let read: usize = scenarios.iter().flat_map(|s| &s.items).filter(|i| i.should_read).count();
    let images = scenarios.iter().filter(|s| matches!(s.background, crate::model::Background::Images { .. })).count();
    let secs: f64 = scenarios.iter().map(|s| s.duration_ms / 1000.0).sum();
    println!(
        "wrote {} scenarios ({secs:.0} s, {items} items, {read} to read, {images} on image backgrounds) to {}",
        scenarios.len(),
        out.display()
    );
    Ok(())
}

fn cmd_render(a: &RenderArgs) -> Result<()> {
    let s = generator::load_scenario(&a.scenario)?;
    std::fs::create_dir_all(&a.out)?;
    let times: Vec<f64> = if a.times.is_empty() {
        (0..s.frame_count())
            .step_by(a.every.max(1))
            .map(|i| s.frame_time_ms(i))
            .filter(|t| a.from_ms.is_none_or(|f| *t >= f) && a.to_ms.is_none_or(|e| *t <= e))
            .collect()
    } else {
        a.times.clone()
    };
    let (w, h) = (s.width, s.height);
    let mut r = SceneRenderer::new(s)?;
    let mut buf = Vec::new();
    for t in &times {
        r.render(*t, &mut buf)?;
        save_png(&a.out.join(format!("t{:07.0}.png", t)), w, h, &buf)?;
    }
    println!("wrote {} frames to {}", times.len(), a.out.display());
    Ok(())
}

/// One scored unit: the answer key and background text of a scene.
struct Case {
    id: String,
    genres: Vec<String>,
    items: Vec<TextItem>,
    bg: BgTextMap,
    timing: TimingFlags,
}

impl Case {
    fn from_scenario(s: &crate::model::Scenario) -> Case {
        Case {
            id: s.id.clone(),
            genres: s.genres.clone(),
            items: s.items.clone(),
            bg: BgTextMap::for_background(&s.background, s.width, s.height),
            timing: TimingFlags::default(),
        }
    }
    fn from_recorded(r: &crate::model::RecordedScenario) -> Case {
        Case {
            id: r.id.clone(),
            genres: vec!["recorded".into()],
            items: r.truth.clone(),
            bg: BgTextMap::default(),
            timing: TimingFlags::new(&r.start_uncertain, &r.end_uncertain),
        }
    }
}

/// What the scored scenes are: suite name and seed (none for the recorded set).
struct SetInfo {
    name: String,
    seed: Option<u64>,
}

/// Folder-name tag for reader options, e.g. `-engine-ppocr` (empty without options).
fn opts_tag(opts: &BTreeMap<String, String>) -> String {
    opts.iter()
        .map(|(k, v)| {
            let clean = |s: &str| s.chars().map(|c| if c.is_ascii_alphanumeric() || c == '.' { c } else { '-' }).collect::<String>();
            format!("-{}-{}", clean(k), clean(v))
        })
        .collect()
}

fn parse_opts(v: &[String]) -> Result<BTreeMap<String, String>> {
    v.iter()
        .map(|kv| kv.split_once('=').map(|(k, v)| (k.to_string(), v.to_string())).with_context(|| format!("--reader-opt {kv:?} is not key=value")))
        .collect()
}

fn cmd_run(a: &RunArgs) -> Result<ExitCode> {
    let clock = Clock::parse(&a.clock)?;
    // Check the reader name before any work.
    if !readers::names().contains(&a.reader.as_str()) {
        bail!("unknown reader {:?}; known: {}", a.reader, readers::names().join(", "));
    }
    let opts = parse_opts(&a.reader_opts)?;
    let jobs = a.jobs.unwrap_or(if clock == Clock::Simulated { 1 } else { 8 });
    let started = std::time::Instant::now();

    let (set, cases, runs) = if let Some(dir) = &a.suite {
        let (suite, scenarios) = generator::load_suite(dir)?;
        let runs = runner::parallel(scenarios.len(), jobs, |i| {
            let r = runner::run_scenario(&scenarios[i], &a.reader, clock, &opts);
            if let Ok(run) = &r {
                eprintln!("  {} : {} events, {}/{} frames", run.scenario_id, run.events.len(), run.frames_processed, run.frames_total);
            }
            r
        })?;
        let cases = scenarios.iter().map(Case::from_scenario).collect::<Vec<_>>();
        (SetInfo { name: suite.name, seed: Some(suite.seed) }, cases, runs)
    } else if let Some(path) = &a.recorded {
        let recs = runner::load_recorded(path)?;
        let runs = runner::parallel(recs.len(), jobs, |i| runner::run_recorded(&recs[i].0, &recs[i].1, &a.reader, clock, &opts))?;
        let cases = recs.iter().map(|(_, r)| Case::from_recorded(r)).collect::<Vec<_>>();
        (SetInfo { name: "recorded".to_string(), seed: None }, cases, runs)
    } else {
        bail!("give --suite <dir> or --recorded <dir>");
    };

    let report_dir = a
        .report
        .clone()
        .unwrap_or_else(|| crate::data_dir().join("reports").join(format!("{}-{}{}-{}", set.name, a.reader, opts_tag(&opts), a.clock)));
    let ev_dir = report_dir.join("events");
    std::fs::create_dir_all(&ev_dir)?;
    for r in &runs {
        std::fs::write(ev_dir.join(format!("{}.json", r.scenario_id)), serde_json::to_string_pretty(r)?)?;
    }
    eprintln!("ran {} scenes in {:.1} s", runs.len(), started.elapsed().as_secs_f64());
    finish_report(&set, &a.reader, &opts, &a.clock, &cases, &runs, &report_dir, a.baseline.as_deref())
}

fn cmd_score(a: &ScoreArgs) -> Result<ExitCode> {
    let (set, cases) = if let Some(dir) = &a.suite {
        let (suite, scenarios) = generator::load_suite(dir)?;
        let cases = scenarios.iter().map(Case::from_scenario).collect();
        (SetInfo { name: suite.name, seed: Some(suite.seed) }, cases)
    } else if let Some(path) = &a.recorded {
        let cases = runner::load_recorded(path)?.iter().map(|(_, r)| Case::from_recorded(r)).collect();
        (SetInfo { name: "recorded".to_string(), seed: None }, cases)
    } else {
        bail!("give --suite <dir> or --recorded <dir>");
    };
    let cases: Vec<Case> = cases;
    let runs = cases
        .iter()
        .map(|c| {
            let p = a.events.join("events").join(format!("{}.json", c.id));
            let s = std::fs::read_to_string(&p).with_context(|| format!("read {}", p.display()))?;
            let run = serde_json::from_str::<ScenarioRun>(&s).with_context(|| format!("parse {}", p.display()))?;
            check_run_matches(c, &run, &p)?;
            Ok(run)
        })
        .collect::<Result<Vec<_>>>()?;
    let reader = runs.first().map(|r| r.reader.clone()).unwrap_or_default();
    let opts = runs.first().map(|r| r.reader_options.clone()).unwrap_or_default();
    let clock = runs.first().map(|r| r.clock.name().to_string()).unwrap_or_default();
    let out = a.out.clone().unwrap_or_else(|| a.events.clone());
    finish_report(&set, &reader, &opts, &clock, &cases, &runs, &out, a.baseline.as_deref())
}

/// Refuses saved events that were made for another scene or another answer key (for example a
/// suite generated again with a different seed or generator). Events without a fingerprint come
/// from an older arena and only get a warning.
fn check_run_matches(c: &Case, run: &ScenarioRun, path: &Path) -> Result<()> {
    if run.scenario_id != c.id {
        bail!("{} holds events of scene {:?}, not {:?}", path.display(), run.scenario_id, c.id);
    }
    let want = crate::model::truth_fingerprint(&c.items);
    if run.truth_fingerprint.is_empty() {
        eprintln!("warning: {} has no truth fingerprint (older arena); cannot check it matches the answer key", path.display());
    } else if run.truth_fingerprint != want {
        bail!(
            "{} was made for another answer key of {} (fingerprint {} vs {}); run the reader again",
            path.display(),
            c.id,
            run.truth_fingerprint,
            want
        );
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn finish_report(
    set: &SetInfo,
    reader: &str,
    opts: &BTreeMap<String, String>,
    clock: &str,
    cases: &[Case],
    runs: &[ScenarioRun],
    dir: &Path,
    baseline: Option<&Path>,
) -> Result<ExitCode> {
    let scored = cases.iter().zip(runs).map(|(c, r)| score::score_scenario_with(&c.id, &c.genres, &c.items, r, &c.bg, &c.timing)).collect();
    let mut report = score::build_report(&set.name, reader, clock, scored);
    report.seed = set.seed;
    report.reader_options = opts.clone();
    let prints: Vec<(String, String)> = cases.iter().map(|c| (c.id.clone(), crate::model::truth_fingerprint(&c.items))).collect();
    report.truth_fingerprint = score::suite_fingerprint(&prints);
    let suite = set.name.as_str();
    let reader = score::reader_label(reader, opts);
    std::fs::create_dir_all(dir)?;
    std::fs::write(dir.join("report.json"), serde_json::to_string_pretty(&report)?)?;
    std::fs::write(dir.join("report.md"), score::report_markdown(&report))?;
    let m = &report.totals;
    println!(
        "{suite} / {reader} / {clock}: recall {:.3} ({}/{}), CER {:.3}, exact {:.3}, prompts {:.3}, dup {}, noise {}, background {}, halluc {}, latency p50/p95 {:.0}/{:.0} ms, cpu p50/p95 {:.2}/{:.2} ms",
        m.recall, m.items_read, m.items_should_read, m.mean_cer, m.exact_match_rate, m.prompt_recall, m.duplicates, m.noise_reads, m.background_reads, m.hallucinations, m.latency_p50_ms, m.latency_p95_ms, m.cpu_ms_p50, m.cpu_ms_p95
    );
    println!("report: {}", dir.join("report.md").display());
    if let Some(b) = baseline {
        let base: Report = serde_json::from_str(&std::fs::read_to_string(b).with_context(|| format!("read baseline {}", b.display()))?)?;
        let mismatch = score::baseline_mismatch(&report, &base);
        if !mismatch.is_empty() {
            bail!("baseline {} was made on other scenes, so it cannot be compared: {}", b.display(), mismatch.join("; "));
        }
        if base.reader != report.reader || base.reader_options != report.reader_options {
            println!(
                "note: baseline reader is {}, this run is {}",
                score::reader_label(&base.reader, &base.reader_options),
                score::reader_label(&report.reader, &report.reader_options)
            );
        }
        let c = score::compare(&report.totals, &base.totals, Tolerance::default());
        println!("vs baseline {}:", b.display());
        for l in &c.lines {
            println!("  {l}");
        }
        if !c.regressions.is_empty() {
            for r in &c.regressions {
                println!("  REGRESSION: {r}");
            }
            return Ok(ExitCode::from(2));
        }
        println!("  no regressions");
    }
    Ok(ExitCode::SUCCESS)
}
