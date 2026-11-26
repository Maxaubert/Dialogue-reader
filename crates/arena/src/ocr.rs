//! OCR engines and `arena ocr-bench`.
//!
//! The bench cuts single-line crops with a known answer out of generated scenes, runs one engine on
//! each crop and reports CER, exact-match rate and ms per crop, overall and per style bucket.
//! The `winocr-naive` reader uses the same engines through `recognize_region`.

#[cfg(feature = "ppocr")]
pub mod ppocr;
#[cfg(windows)]
pub mod winrt;

use crate::background;
use crate::generator::{self, GenOptions, SuiteKind};
use crate::model::{Align, Background, Rect, Scenario, TextItem};
use crate::render::{blit, build_sprite, save_png};
use crate::runner;
use crate::score::percentile;
use crate::text::{levenshtein, normalize};
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

/// One recognised line. `rect` is in the pixels of the image given to the engine.
#[derive(Clone, Debug, PartialEq)]
pub struct OcrLine {
    pub text: String,
    pub rect: Rect,
}

pub trait Engine {
    fn name(&self) -> String;
    /// Largest width or height the engine accepts; bigger images are scaled down first.
    fn max_dim(&self) -> u32 {
        u32::MAX
    }
    /// Recognises text in an RGBA8 image (no row padding).
    fn recognize(&mut self, w: u32, h: u32, rgba: &[u8]) -> Result<Vec<OcrLine>>;
}

/// Settings shared by every engine.
#[derive(Clone, Debug)]
pub struct EngineOptions {
    /// WinRT language tag.
    pub lang: String,
    /// Folder with the PP-OCR ONNX models and dictionary.
    pub models: PathBuf,
}

impl Default for EngineOptions {
    fn default() -> Self {
        EngineOptions { lang: "en-US".to_string(), models: crate::data_dir().join("models").join("ppocr-v3") }
    }
}

pub const ENGINES: &[&str] = &["winocr", "ppocr", "ppocr-rec"];

pub fn create_engine(name: &str, opts: &EngineOptions) -> Result<Box<dyn Engine>> {
    match name {
        #[cfg(windows)]
        "winocr" => Ok(Box::new(winrt::WinOcr::new(&opts.lang)?)),
        #[cfg(feature = "ppocr")]
        "ppocr" => Ok(Box::new(ppocr::PpOcr::new(&opts.models, true)?)),
        #[cfg(feature = "ppocr")]
        "ppocr-rec" => Ok(Box::new(ppocr::PpOcr::new(&opts.models, false)?)),
        #[cfg(not(feature = "ppocr"))]
        "ppocr" | "ppocr-rec" => {
            let _ = &opts.models;
            bail!("ppocr: built without the `ppocr` cargo feature (cargo build -p arena --features ppocr)")
        }
        _ => bail!("unknown OCR engine {name:?}; known: {}", ENGINES.join(", ")),
    }
}

/// Copies the pixel box [x0, x1) x [y0, y1) out of an RGBA8 frame.
pub fn crop_rgba(rgba: &[u8], fw: u32, x0: u32, y0: u32, x1: u32, y1: u32) -> Vec<u8> {
    let (cw, ch) = ((x1 - x0) as usize, (y1 - y0) as usize);
    let mut out = Vec::with_capacity(cw * ch * 4);
    for y in y0 as usize..y0 as usize + ch {
        let start = (y * fw as usize + x0 as usize) * 4;
        out.extend_from_slice(&rgba[start..start + cw * 4]);
    }
    out
}

/// Pixel box of `r` clamped to the frame: (x0, y0, x1, y1), or `None` when empty.
pub fn clamp_rect(r: &Rect, fw: u32, fh: u32) -> Option<(u32, u32, u32, u32)> {
    let x0 = r.x.floor().clamp(0.0, fw as f32) as u32;
    let y0 = r.y.floor().clamp(0.0, fh as f32) as u32;
    let x1 = r.right().ceil().clamp(0.0, fw as f32) as u32;
    let y1 = r.bottom().ceil().clamp(0.0, fh as f32) as u32;
    (x1 > x0 && y1 > y0).then_some((x0, y0, x1, y1))
}

/// Runs `engine` on the part of a frame inside `region` (whole frame when `None`), scaled by
/// `scale` first. Returned line rects are in frame pixels.
pub fn recognize_region(engine: &mut dyn Engine, rgba: &[u8], fw: u32, fh: u32, region: Option<&Rect>, scale: f32) -> Result<Vec<OcrLine>> {
    let full = Rect::new(0.0, 0.0, fw as f32, fh as f32);
    let Some((x0, y0, x1, y1)) = clamp_rect(region.unwrap_or(&full), fw, fh) else { return Ok(Vec::new()) };
    let (cw, ch) = (x1 - x0, y1 - y0);
    let crop = if (x0, y0, cw, ch) == (0, 0, fw, fh) { rgba.to_vec() } else { crop_rgba(rgba, fw, x0, y0, x1, y1) };
    let limit = engine.max_dim() as f32 / cw.max(ch) as f32;
    let s = scale.max(0.05).min(limit);
    let (sw, sh) = (((cw as f32 * s).round() as u32).max(1), ((ch as f32 * s).round() as u32).max(1));
    let img = if (sw, sh) == (cw, ch) { crop } else { background::resize_bilinear(&crop, cw, ch, sw, sh) };
    let (kx, ky) = (cw as f32 / sw as f32, ch as f32 / sh as f32);
    let lines = engine.recognize(sw, sh, &img)?;
    Ok(lines
        .into_iter()
        .map(|l| OcrLine {
            text: l.text,
            rect: Rect::new(x0 as f32 + l.rect.x * kx, y0 as f32 + l.rect.y * ky, l.rect.w * kx, l.rect.h * ky),
        })
        .collect())
}

// ---------------------------------------------------------------------------------------------
// ocr-bench
// ---------------------------------------------------------------------------------------------

#[derive(clap::Args, Debug, Clone)]
pub struct Args {
    /// OCR engine: winocr, ppocr (det + rec) or ppocr-rec (rec only; needs --features ppocr)
    #[arg(long, default_value = "winocr")]
    pub engine: String,
    /// Suite folder (from `arena gen`)
    #[arg(long, conflicts_with = "gen")]
    pub suite: Option<PathBuf>,
    /// Generate the suite in memory instead: smoke, standard or hard
    #[arg(long)]
    pub r#gen: Option<String>,
    /// Seed for --gen
    #[arg(long, default_value_t = 1)]
    pub seed: u64,
    /// Folder for the bench report (default arena-data/reports/ocr-bench-<suite>-<engine>)
    #[arg(long)]
    pub report: Option<PathBuf>,
    /// Scale crops by this factor before OCR (e.g. 2 for a 2x upscale)
    #[arg(long, default_value_t = 1.0)]
    pub scale: f32,
    /// WinRT OCR language tag
    #[arg(long, default_value = "en-US")]
    pub lang: String,
    /// Folder with the PP-OCR models (default arena-data/models/ppocr-v3)
    #[arg(long)]
    pub models: Option<PathBuf>,
    /// Threads used to render the crops (OCR itself runs on one thread so ms per crop is clean)
    #[arg(long, default_value_t = 8)]
    pub jobs: usize,
    /// Only the first N crops (quick checks)
    #[arg(long)]
    pub limit: Option<usize>,
    /// Write the crops that were not read exactly as PNGs into this folder
    #[arg(long)]
    pub save_crops: Option<PathBuf>,
}

/// One single-line crop with its answer and style buckets.
pub struct Crop {
    pub id: String,
    pub truth: String,
    pub w: u32,
    pub h: u32,
    pub rgba: Vec<u8>,
    pub buckets: Vec<(&'static str, String)>,
}

/// Bucket a style falls into, per dimension.
pub fn style_buckets(item: &TextItem, bg: &Background) -> Vec<(&'static str, String)> {
    let st = &item.style;
    let panel = match &st.panel {
        None => "none".to_string(),
        Some(p) if p.alpha < 0.35 => "alpha <0.35".to_string(),
        Some(p) if p.alpha < 0.6 => "alpha 0.35-0.6".to_string(),
        Some(p) if p.alpha < 0.8 => "alpha 0.6-0.8".to_string(),
        Some(_) => "alpha >=0.8".to_string(),
    };
    let size = match st.size_px {
        s if s < 16.0 => "<16 px",
        s if s < 20.0 => "16-19 px",
        s if s < 24.0 => "20-23 px",
        s if s < 30.0 => "24-29 px",
        _ => ">=30 px",
    };
    let bgk = match bg {
        Background::Procedural { kind, .. } => serde_json::to_value(kind).ok().and_then(|v| v.as_str().map(String::from)).unwrap_or_default(),
        Background::Images { dir, .. } => {
            format!("images:{}", Path::new(dir).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default())
        }
    };
    let kind = serde_json::to_value(item.kind).ok().and_then(|v| v.as_str().map(String::from)).unwrap_or_default();
    vec![
        ("outline", if st.outline.is_some() { "outline" } else { "no outline" }.to_string()),
        ("shadow", if st.shadow.is_some() { "shadow" } else { "no shadow" }.to_string()),
        ("panel", panel),
        ("font", format!("{}{}", st.font, if st.bold { " bold" } else { "" })),
        ("size", size.to_string()),
        ("background", bgk),
        ("kind", kind),
    ]
}

/// The item showing only line `li` of its text, with the box that line would get on its own.
fn line_item(item: &TextItem, li: usize, line: &str) -> TextItem {
    let st = &item.style;
    let pad = st.panel.as_ref().map(|p| p.padding).unwrap_or(0.0);
    let (tw, th) = crate::fonts::measure(&st.font, st.bold, st.italic, st.size_px, st.line_spacing, line);
    let line_h = st.size_px * st.line_spacing;
    let w = match st.align {
        Align::Left => (tw + 2.0 * pad).ceil(),
        Align::Center => item.rect.w,
    };
    let mut it = item.clone();
    it.text = line.to_string();
    it.rect = Rect::new(item.rect.x, item.rect.y + li as f32 * line_h, w, (th + 2.0 * pad).ceil());
    it.anim = Default::default();
    it.start_ms = 0.0;
    it.end_ms = f64::MAX;
    it
}

/// Margin around a line crop: the outline and shadow plus a few pixels of background.
fn crop_margin(item: &TextItem) -> f32 {
    let st = &item.style;
    let o = st.outline.as_ref().map(|o| o.px).unwrap_or(0.0);
    let s = st.shadow.as_ref().map(|s| s.dx.abs().max(s.dy.abs()) + s.blur).unwrap_or(0.0);
    (6.0 + o + s).ceil()
}

/// Single-line crops of every text item of one scenario, drawn fully visible on the background
/// frame at the middle of the item's time.
pub fn scenario_crops(s: &Scenario) -> Result<Vec<Crop>> {
    let (fw, fh) = (s.width, s.height);
    let mut bg = background::open(&s.background, fw, fh)?;
    let mut frame = vec![0u8; (fw * fh * 4) as usize];
    let mut out = Vec::new();
    for item in &s.items {
        let mid = (item.start_ms + item.end_ms.min(s.duration_ms)) / 2.0;
        for (li, line) in item.text.split('\n').enumerate() {
            let line = line.trim();
            if normalize(line).is_empty() {
                continue;
            }
            let it = line_item(item, li, line);
            bg.frame(mid, &mut frame)?;
            let sprite = build_sprite(&it, it.text.chars().count())?;
            let x = it.rect.x.round() as i32 + sprite.off_x;
            let y = it.rect.y.round() as i32 + sprite.off_y;
            blit(&mut frame, fw as usize, fh as usize, &sprite, x, y, 1.0);
            let Some((x0, y0, x1, y1)) = clamp_rect(&it.rect.inflate(crop_margin(&it)), fw, fh) else { continue };
            out.push(Crop {
                id: format!("{}/{}/{}", s.id, item.id, li),
                truth: line.to_string(),
                w: x1 - x0,
                h: y1 - y0,
                rgba: crop_rgba(&frame, fw, x0, y0, x1, y1),
                buckets: style_buckets(item, &s.background),
            });
        }
    }
    Ok(out)
}

/// One crop's result.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct CropResult {
    pub id: String,
    pub truth: String,
    pub read: String,
    pub edits: usize,
    pub truth_chars: usize,
    pub cer: f64,
    pub ms: f64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct BenchStats {
    pub crops: usize,
    /// Mean of the per-crop CER (each capped at 1).
    pub mean_cer: f64,
    /// Total edits over total truth characters (long lines weigh more).
    pub micro_cer: f64,
    pub exact_rate: f64,
    /// Share of crops where the engine returned nothing.
    pub empty_rate: f64,
    pub ms_mean: f64,
    pub ms_p50: f64,
    pub ms_p95: f64,
}

pub fn stats(rs: &[&CropResult]) -> BenchStats {
    if rs.is_empty() {
        return BenchStats::default();
    }
    let n = rs.len() as f64;
    let edits: usize = rs.iter().map(|r| r.edits).sum();
    let chars: usize = rs.iter().map(|r| r.truth_chars).sum();
    let ms: Vec<f64> = rs.iter().map(|r| r.ms).collect();
    BenchStats {
        crops: rs.len(),
        mean_cer: rs.iter().map(|r| r.cer.min(1.0)).sum::<f64>() / n,
        micro_cer: if chars == 0 { 0.0 } else { edits as f64 / chars as f64 },
        exact_rate: rs.iter().filter(|r| r.edits == 0).count() as f64 / n,
        empty_rate: rs.iter().filter(|r| normalize(&r.read).is_empty()).count() as f64 / n,
        ms_mean: ms.iter().sum::<f64>() / n,
        ms_p50: percentile(&ms, 0.5),
        ms_p95: percentile(&ms, 0.95),
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct BenchReport {
    pub suite: String,
    pub engine: String,
    pub scale: f32,
    pub overall: BenchStats,
    /// dimension -> bucket -> stats
    pub buckets: BTreeMap<String, BTreeMap<String, BenchStats>>,
    /// Worst reads (highest CER first), for a quick look.
    pub worst: Vec<CropResult>,
}

/// Scores one read against its truth (both normalised).
pub fn score_read(id: &str, truth: &str, read: &str, ms: f64) -> CropResult {
    let t: Vec<char> = normalize(truth).chars().collect();
    let h: Vec<char> = normalize(read).chars().collect();
    let edits = levenshtein(&h, &t);
    let cer = if t.is_empty() { if h.is_empty() { 0.0 } else { 1.0 } } else { edits as f64 / t.len() as f64 };
    CropResult { id: id.to_string(), truth: truth.to_string(), read: read.to_string(), edits, truth_chars: t.len(), cer, ms }
}

pub fn build_bench_report(suite: &str, engine: &str, scale: f32, crops: &[Crop], results: &[CropResult]) -> BenchReport {
    let all: Vec<&CropResult> = results.iter().collect();
    let mut groups: BTreeMap<String, BTreeMap<String, Vec<&CropResult>>> = BTreeMap::new();
    for (c, r) in crops.iter().zip(results) {
        for (dim, b) in &c.buckets {
            groups.entry(dim.to_string()).or_default().entry(b.clone()).or_default().push(r);
        }
    }
    let buckets = groups.into_iter().map(|(d, m)| (d, m.into_iter().map(|(b, v)| (b, stats(&v))).collect())).collect();
    let mut worst: Vec<CropResult> = results.iter().filter(|r| r.edits > 0).cloned().collect();
    worst.sort_by(|a, b| b.cer.total_cmp(&a.cer).then(a.id.cmp(&b.id)));
    worst.truncate(30);
    BenchReport { suite: suite.to_string(), engine: engine.to_string(), scale, overall: stats(&all), buckets, worst }
}

fn stats_row(name: &str, s: &BenchStats) -> String {
    format!(
        "| {name} | {} | {:.3} | {:.3} | {:.3} | {:.3} | {:.1} | {:.1} |\n",
        s.crops, s.mean_cer, s.micro_cer, s.exact_rate, s.empty_rate, s.ms_p50, s.ms_p95
    )
}

pub fn bench_markdown(r: &BenchReport) -> String {
    let head = "| Bucket | Crops | Mean CER | Micro CER | Exact | Empty | ms p50 | ms p95 |\n|---|---|---|---|---|---|---|---|\n";
    let mut s = format!("# OCR bench: {} on {} (scale {})\n\n## Overall\n\n{head}", r.engine, r.suite, r.scale);
    s += &stats_row("all", &r.overall);
    for (dim, m) in &r.buckets {
        s += &format!("\n## By {dim}\n\n{head}");
        for (b, st) in m {
            s += &stats_row(b, st);
        }
    }
    s += "\n## Worst reads\n\n| Crop | Truth | Read | CER |\n|---|---|---|---|\n";
    for w in &r.worst {
        let esc = |t: &str| t.replace('|', "\\|").replace('\n', " / ");
        s += &format!("| {} | {} | {} | {:.2} |\n", w.id, esc(&w.truth), esc(&w.read), w.cer);
    }
    s
}

pub fn run(args: &Args) -> Result<()> {
    let (suite_name, scenarios) = match (&args.suite, &args.r#gen) {
        (Some(dir), _) => {
            let (suite, sc) = generator::load_suite(dir)?;
            (suite.name, sc)
        }
        (None, Some(kind)) => {
            let k = SuiteKind::parse(kind)?;
            (k.name().to_string(), generator::generate_suite(k, args.seed, &GenOptions::from_pc()))
        }
        (None, None) => bail!("ocr-bench: give --suite <dir> or --gen smoke|standard|hard"),
    };
    let opts = EngineOptions {
        lang: args.lang.clone(),
        models: args.models.clone().unwrap_or_else(|| EngineOptions::default().models),
    };
    let mut engine = create_engine(&args.engine, &opts)?;

    let t0 = Instant::now();
    let per_scene = runner::parallel(scenarios.len(), args.jobs, |i| scenario_crops(&scenarios[i]))?;
    let mut crops: Vec<Crop> = per_scene.into_iter().flatten().collect();
    if let Some(n) = args.limit {
        crops.truncate(n);
    }
    eprintln!("{} crops from {} scenes in {:.1} s", crops.len(), scenarios.len(), t0.elapsed().as_secs_f64());
    if let Some(c) = crops.first() {
        recognize_region(engine.as_mut(), &c.rgba, c.w, c.h, None, args.scale)?; // warm-up, not timed
    }

    let mut results = Vec::with_capacity(crops.len());
    for c in &crops {
        let start = Instant::now();
        let lines = recognize_region(engine.as_mut(), &c.rgba, c.w, c.h, None, args.scale)
            .with_context(|| format!("OCR of crop {}", c.id))?;
        let ms = start.elapsed().as_secs_f64() * 1000.0;
        let read = lines.iter().map(|l| l.text.as_str()).collect::<Vec<_>>().join(" ");
        results.push(score_read(&c.id, &c.truth, &read, ms));
    }

    let report = build_bench_report(&suite_name, &engine.name(), args.scale, &crops, &results);
    let mut scale_tag = if (args.scale - 1.0).abs() > 1e-3 { format!("-x{}", args.scale) } else { String::new() };
    if args.engine.starts_with("ppocr")
        && let Some(n) = opts.models.file_name()
    {
        scale_tag = format!("-{}{scale_tag}", n.to_string_lossy());
    }
    let dir = args
        .report
        .clone()
        .unwrap_or_else(|| crate::data_dir().join("reports").join(format!("ocr-bench-{suite_name}-{}{scale_tag}", args.engine)));
    std::fs::create_dir_all(&dir).with_context(|| format!("create {}", dir.display()))?;
    std::fs::write(dir.join("report.json"), serde_json::to_string_pretty(&report)?)?;
    std::fs::write(dir.join("results.json"), serde_json::to_string_pretty(&results)?)?;
    std::fs::write(dir.join("report.md"), bench_markdown(&report))?;
    if let Some(out) = &args.save_crops {
        std::fs::create_dir_all(out)?;
        for (c, r) in crops.iter().zip(&results) {
            if r.edits > 0 {
                save_png(&out.join(format!("{}.png", c.id.replace('/', "_"))), c.w, c.h, &c.rgba)?;
            }
        }
    }
    let o = &report.overall;
    println!(
        "{}: {} crops, mean CER {:.3}, micro CER {:.3}, exact {:.3}, empty {:.3}, ms p50 {:.1} p95 {:.1}",
        report.engine, o.crops, o.mean_cer, o.micro_cer, o.exact_rate, o.empty_rate, o.ms_p50, o.ms_p95
    );
    println!("report: {}", dir.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crop_and_clamp() {
        let (w, h) = (4u32, 3u32);
        let rgba: Vec<u8> = (0..w * h * 4).map(|i| i as u8).collect();
        let (x0, y0, x1, y1) = clamp_rect(&Rect::new(-2.0, 1.0, 4.5, 10.0), w, h).unwrap();
        assert_eq!((x0, y0, x1, y1), (0, 1, 3, 3));
        let c = crop_rgba(&rgba, w, x0, y0, x1, y1);
        assert_eq!(c.len(), 3 * 2 * 4);
        assert_eq!(&c[0..4], &rgba[16..20]);
        assert!(clamp_rect(&Rect::new(10.0, 0.0, 5.0, 5.0), w, h).is_none());
    }

    #[test]
    fn scoring_reads() {
        let r = score_read("a", "+ 6 Iron Ore (27)", "+6 lron Ore 27", 1.0);
        assert_eq!(r.truth_chars, "6 iron ore 27".chars().count());
        assert_eq!(r.edits, 1);
        let e = score_read("b", "Hello", "", 1.0);
        assert_eq!((e.edits, e.cer), (5, 1.0));
        let s = stats(&[&r, &e]);
        assert_eq!(s.crops, 2);
        assert!((s.micro_cer - 6.0 / 18.0).abs() < 1e-9);
        assert!((s.empty_rate - 0.5).abs() < 1e-9);
        assert_eq!(s.exact_rate, 0.0);
    }

    /// Engine that reports the image size it got, to check region offsets and scaling.
    struct SizeEngine;
    impl Engine for SizeEngine {
        fn name(&self) -> String {
            "size".into()
        }
        fn recognize(&mut self, w: u32, h: u32, _rgba: &[u8]) -> Result<Vec<OcrLine>> {
            Ok(vec![OcrLine { text: format!("{w}x{h}"), rect: Rect::new(0.0, 0.0, w as f32, h as f32) }])
        }
    }

    #[test]
    fn region_rects_map_back_to_the_frame() {
        let (fw, fh) = (100u32, 50u32);
        let frame = vec![0u8; (fw * fh * 4) as usize];
        let lines = recognize_region(&mut SizeEngine, &frame, fw, fh, Some(&Rect::new(10.0, 5.0, 20.0, 10.0)), 2.0).unwrap();
        assert_eq!(lines[0].text, "40x20");
        assert_eq!(lines[0].rect, Rect::new(10.0, 5.0, 20.0, 10.0));
    }

    #[test]
    fn crops_have_one_line_each() {
        let sc = generator::generate_suite(SuiteKind::Smoke, 3, &GenOptions::procedural_only());
        let crops = scenario_crops(&sc[0]).unwrap();
        assert!(!crops.is_empty());
        for c in &crops {
            assert!(!c.truth.contains('\n'));
            assert_eq!(c.rgba.len(), (c.w * c.h * 4) as usize);
            assert_eq!(c.buckets.len(), 7);
        }
    }

    #[cfg(windows)]
    #[test]
    fn winrt_reads_a_clean_line() {
        let Ok(mut eng) = winrt::WinOcr::new("en-US") else {
            eprintln!("skipped: no WinRT OCR engine on this PC");
            return;
        };
        let item = TextItem {
            id: "t".into(),
            text: "Iron Ore".into(),
            kind: crate::model::ItemKind::PickupToast,
            should_read: true,
            rect: Rect::new(20.0, 20.0, 200.0, 40.0),
            start_ms: 0.0,
            end_ms: 1000.0,
            style: crate::model::Style {
                font: "Segoe UI".into(),
                size_px: 28.0,
                color: [255, 255, 255, 255],
                outline: None,
                shadow: None,
                panel: None,
                bold: false,
                italic: false,
                align: Align::Left,
                line_spacing: 1.25,
            },
            anim: Default::default(),
        };
        let (fw, fh) = (260u32, 80u32);
        let mut frame: Vec<u8> = [20u8, 20, 30, 255].repeat((fw * fh) as usize);
        let sprite = build_sprite(&item, 8).unwrap();
        blit(&mut frame, fw as usize, fh as usize, &sprite, 20 + sprite.off_x, 20 + sprite.off_y, 1.0);
        let lines = recognize_region(&mut eng, &frame, fw, fh, None, 1.0).unwrap();
        let text: Vec<String> = lines.iter().map(|l| l.text.clone()).collect();
        assert_eq!(normalize(&text.join(" ")), "iron ore", "read {text:?}");
        assert!(lines[0].rect.w > 20.0);
    }
}
