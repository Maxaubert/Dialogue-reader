//! PP-OCR engine (PaddleOCR ONNX models run with ONNX Runtime through the `ort` crate).
//!
//! A model folder (never committed) holds:
//! - `det.onnx`: DB text detector.
//! - `rec.onnx`: CTC line recogniser (input 3 x 48 x W).
//! - `dict.txt`: one character per line; class 0 is the CTC blank and the last class is a space.
//!
//! Folders used on 2026-11-21 (under `arena-data/models/`):
//! - `ppocr-v3` (default): det = PP-OCRv3 mobile det (huggingface.co/monkt/paddleocr-onnx,
//!   `detection/v3/det.onnx`); rec = en_PP-OCRv3 mobile rec (huggingface.co/SWHL/RapidOCR,
//!   `PP-OCRv3/en_PP-OCRv3_rec_infer.onnx`); dict = PaddleOCR `ppocr/utils/en_dict.txt` (release/2.7).
//! - `ppocr-v5`: same det; rec and dict = monkt/paddleocr-onnx `languages/english/` (en_PP-OCRv5
//!   mobile rec). This export reads lines up to 320 px wide (after scaling to 48 px high) but
//!   returns junk for wider lines, so long dialogue lines fail unless the detector splits them.
//!   The v5 det in that repo is the 88 MB server model, so it was not used.
//!
//! Two engines: `ppocr` (detect lines, then read each) and `ppocr-rec` (read the whole image as
//! one line, for crops that are already single lines).

use super::{Engine, OcrLine};
use crate::model::Rect;
use anyhow::{Context, Result, anyhow, bail};
use ort::session::Session;
use ort::value::Tensor;
use std::path::Path;
use std::sync::OnceLock;

/// Recogniser input height (PP-OCRv4/v5 use 3 x 48 x W).
const REC_H: usize = 48;
/// Narrowest recogniser input; shorter lines are padded (as PaddleOCR does).
const REC_MIN_W: usize = 320;
const REC_MAX_W: usize = 3200;
/// Detector: largest side after resizing, and DB post-processing constants (PaddleOCR defaults).
const DET_MAX_SIDE: f32 = 960.0;
const DET_THRESH: f32 = 0.3;
const DET_BOX_THRESH: f32 = 0.6;
const DET_UNCLIP: f32 = 1.5;
const DET_MIN_SIZE: f32 = 3.0;

fn ort_err<E: std::fmt::Display>(e: E) -> anyhow::Error {
    anyhow!("onnxruntime: {e}")
}

pub struct PpOcr {
    /// Model folder name, shown in the engine name.
    tag: String,
    det: Option<Session>,
    rec: Session,
    /// Index = class id; 0 is the blank.
    classes: Vec<String>,
}

fn session(env: &ort::environment::Environment, path: &Path) -> Result<Session> {
    if !path.is_file() {
        bail!("PP-OCR model missing: {} (see src/ocr/ppocr.rs for where to get it)", path.display());
    }
    Session::builder(env)
        .map_err(ort_err)?
        .with_intra_threads(1)
        .map_err(ort_err)?
        .commit_from_file(path)
        .map_err(ort_err)
        .with_context(|| format!("load {}", path.display()))
}

/// ONNX Runtime allows one environment per process; every engine shares it.
fn environment() -> Result<&'static ort::environment::Environment> {
    static ENV: OnceLock<ort::environment::Environment> = OnceLock::new();
    if let Some(e) = ENV.get() {
        return Ok(e);
    }
    let env = ort::init().build().map_err(ort_err)?;
    Ok(ENV.get_or_init(|| env))
}

impl PpOcr {
    /// `detect`: false reads the whole image as one line (no detector).
    pub fn new(models: &Path, detect: bool) -> Result<Self> {
        let env = environment()?;
        let dict = std::fs::read_to_string(models.join("dict.txt"))
            .with_context(|| format!("read {}", models.join("dict.txt").display()))?;
        let mut classes = vec![String::new()];
        classes.extend(dict.split('\n').map(|l| l.trim_end_matches('\r').to_string()).filter(|l| !l.is_empty()));
        classes.push(" ".to_string());
        let rec = session(env, &models.join("rec.onnx"))?;
        let det = if detect { Some(session(env, &models.join("det.onnx"))?) } else { None };
        let tag = models.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        let mut me = PpOcr { tag, det, rec, classes };
        // The class count must match the model, else every read is garbage.
        let probe = vec![0u8; 64 * 32 * 4];
        let n = me.rec_logits(64, 32, &probe)?.1;
        if n != me.classes.len() {
            bail!("PP-OCR: rec model has {n} classes but dict.txt gives {} (blank + chars + space)", me.classes.len());
        }
        Ok(me)
    }

    /// Runs the recogniser; returns (logits [T x C], C).
    fn rec_logits(&mut self, w: u32, h: u32, rgba: &[u8]) -> Result<(Vec<f32>, usize)> {
        let (w, h) = (w as usize, h as usize);
        let tw = ((REC_H as f32 * w as f32 / h.max(1) as f32).ceil() as usize).clamp(1, REC_MAX_W);
        let pw = tw.max(REC_MIN_W);
        let scaled = crate::background::resize_bilinear(rgba, w as u32, h as u32, tw as u32, REC_H as u32);
        // BGR, (x / 255 - 0.5) / 0.5, CHW; padding stays 0.
        let mut data = vec![0f32; 3 * REC_H * pw];
        for y in 0..REC_H {
            for x in 0..tw {
                let i = (y * tw + x) * 4;
                for (c, src) in [2usize, 1, 0].into_iter().enumerate() {
                    data[c * REC_H * pw + y * pw + x] = scaled[i + src] as f32 / 127.5 - 1.0;
                }
            }
        }
        let input = Tensor::from_array(([1usize, 3, REC_H, pw], data)).map_err(ort_err)?;
        let out = self.rec.run(ort::inputs![input]).map_err(ort_err)?;
        let (shape, logits) = out[0].try_extract_tensor::<f32>().map_err(ort_err)?;
        let c = *shape.last().context("rec output has no shape")? as usize;
        Ok((logits.to_vec(), c))
    }

    /// Greedy CTC decoding of one line image.
    fn read_line(&mut self, w: u32, h: u32, rgba: &[u8]) -> Result<String> {
        let (logits, c) = self.rec_logits(w, h, rgba)?;
        let mut out = String::new();
        let mut prev = 0usize;
        for step in logits.chunks_exact(c) {
            let best = step.iter().enumerate().max_by(|a, b| a.1.total_cmp(b.1)).map(|(i, _)| i).unwrap_or(0);
            if best != 0 && best != prev {
                out += self.classes.get(best).map(String::as_str).unwrap_or("");
            }
            prev = best;
        }
        Ok(out.trim().to_string())
    }

    /// DB detection: axis-aligned line boxes in image pixels.
    fn detect(&mut self, w: u32, h: u32, rgba: &[u8]) -> Result<Vec<Rect>> {
        let det = self.det.as_mut().context("no detector loaded")?;
        let r = (DET_MAX_SIDE / w.max(h) as f32).min(1.0);
        let round32 = |v: f32| (((v / 32.0).round() as usize) * 32).max(32);
        let (dw, dh) = (round32(w as f32 * r), round32(h as f32 * r));
        let scaled = crate::background::resize_bilinear(rgba, w, h, dw as u32, dh as u32);
        const MEAN: [f32; 3] = [0.485, 0.456, 0.406];
        const STD: [f32; 3] = [0.229, 0.224, 0.225];
        let mut data = vec![0f32; 3 * dw * dh];
        for i in 0..dw * dh {
            for (c, src) in [2usize, 1, 0].into_iter().enumerate() {
                data[c * dw * dh + i] = (scaled[i * 4 + src] as f32 / 255.0 - MEAN[c]) / STD[c];
            }
        }
        let input = Tensor::from_array(([1usize, 3, dh, dw], data)).map_err(ort_err)?;
        let out = det.run(ort::inputs![input]).map_err(ort_err)?;
        let (_, prob) = out[0].try_extract_tensor::<f32>().map_err(ort_err)?;
        let boxes = db_boxes(prob, dw, dh);
        let (sx, sy) = (w as f32 / dw as f32, h as f32 / dh as f32);
        Ok(boxes
            .into_iter()
            .filter_map(|b| {
                let x0 = (b.x * sx).max(0.0);
                let y0 = (b.y * sy).max(0.0);
                let x1 = (b.right() * sx).min(w as f32);
                let y1 = (b.bottom() * sy).min(h as f32);
                (x1 - x0 >= 2.0 && y1 - y0 >= 2.0).then(|| Rect::new(x0, y0, x1 - x0, y1 - y0))
            })
            .collect())
    }
}

/// DB post-processing on a probability map: connected regions above the threshold, kept when
/// their mean probability is high enough, grown by the unclip distance.
pub fn db_boxes(prob: &[f32], w: usize, h: usize) -> Vec<Rect> {
    let mut seen = vec![false; w * h];
    let mut boxes = Vec::new();
    let mut stack = Vec::new();
    for start in 0..w * h {
        if seen[start] || prob[start] <= DET_THRESH {
            continue;
        }
        seen[start] = true;
        stack.push(start);
        let (mut x0, mut y0, mut x1, mut y1) = (w, h, 0usize, 0usize);
        let (mut sum, mut n) = (0f32, 0usize);
        while let Some(i) = stack.pop() {
            let (x, y) = (i % w, i / w);
            x0 = x0.min(x);
            y0 = y0.min(y);
            x1 = x1.max(x);
            y1 = y1.max(y);
            sum += prob[i];
            n += 1;
            let mut push = |j: usize| {
                if !seen[j] && prob[j] > DET_THRESH {
                    seen[j] = true;
                    stack.push(j);
                }
            };
            if x > 0 {
                push(i - 1);
            }
            if x + 1 < w {
                push(i + 1);
            }
            if y > 0 {
                push(i - w);
            }
            if y + 1 < h {
                push(i + w);
            }
        }
        let (bw, bh) = ((x1 - x0 + 1) as f32, (y1 - y0 + 1) as f32);
        if bw.min(bh) < DET_MIN_SIZE || sum / (n as f32) < DET_BOX_THRESH {
            continue;
        }
        let d = bw * bh * DET_UNCLIP / (2.0 * (bw + bh));
        boxes.push(Rect::new(x0 as f32 - d, y0 as f32 - d, bw + 2.0 * d, bh + 2.0 * d));
    }
    // Reading order: top to bottom, then left to right.
    boxes.sort_by(|a, b| {
        let same_row = (a.center().1 - b.center().1).abs() < a.h.min(b.h) / 2.0;
        if same_row { a.x.total_cmp(&b.x) } else { a.y.total_cmp(&b.y) }
    });
    boxes
}

impl Engine for PpOcr {
    fn name(&self) -> String {
        if self.det.is_some() { format!("ppocr({})", self.tag) } else { format!("ppocr-rec({})", self.tag) }
    }

    fn recognize(&mut self, w: u32, h: u32, rgba: &[u8]) -> Result<Vec<OcrLine>> {
        if self.det.is_none() {
            let text = self.read_line(w, h, rgba)?;
            let rect = Rect::new(0.0, 0.0, w as f32, h as f32);
            return Ok(if text.is_empty() { vec![] } else { vec![OcrLine { text, rect }] });
        }
        let mut out = Vec::new();
        for b in self.detect(w, h, rgba)? {
            let Some((x0, y0, x1, y1)) = super::clamp_rect(&b, w, h) else { continue };
            let crop = super::crop_rgba(rgba, w, x0, y0, x1, y1);
            let text = self.read_line(x1 - x0, y1 - y0, &crop)?;
            if !text.is_empty() {
                out.push(OcrLine { text, rect: b });
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn db_boxes_find_blobs_and_grow_them() {
        let (w, h) = (40usize, 20usize);
        let mut prob = vec![0f32; w * h];
        for y in 5..10 {
            for x in 4..30 {
                prob[y * w + x] = 0.9;
            }
        }
        prob[18 * w + 38] = 0.9; // a 1 px speck: dropped (min size)
        let b = db_boxes(&prob, w, h);
        assert_eq!(b.len(), 1);
        assert!(b[0].x < 4.0 && b[0].right() > 30.0 && b[0].y < 5.0 && b[0].bottom() > 10.0);
    }
}
