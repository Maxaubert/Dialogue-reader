//! Backgrounds: procedural kinds and looped image sequences, plus the background-text sidecar.

use crate::model::{Background, ProceduralKind, Rect};
use crate::rng::Rng;
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Fills an RGBA8 frame for time `t`.
pub trait BgSource {
    fn frame(&mut self, t_ms: f64, out: &mut [u8]) -> Result<()>;
}

pub fn open(bg: &Background, w: u32, h: u32) -> Result<Box<dyn BgSource>> {
    match bg {
        Background::Procedural { kind, seed } => Ok(Box::new(Procedural::new(*kind, *seed, w, h))),
        Background::Images { dir, fps, first_frame, count, offset_ms } => {
            let files = image_files(&crate::resolve_path(dir))?;
            let slice = slice_files(&files, *first_frame, *count);
            if slice.is_empty() {
                bail!("no frames in {dir} (first_frame {first_frame}, count {count:?})");
            }
            Ok(Box::new(ImageSeq {
                files: slice.to_vec(),
                fps: *fps,
                offset_ms: *offset_ms,
                w,
                h,
                cached: None,
            }))
        }
    }
}

/// Sorted `.jpg`/`.jpeg`/`.png` files of a folder.
pub fn image_files(dir: &Path) -> Result<Vec<PathBuf>> {
    let rd = std::fs::read_dir(dir).with_context(|| format!("read background folder {}", dir.display()))?;
    let mut files: Vec<PathBuf> = rd
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
            ext == "jpg" || ext == "jpeg" || ext == "png"
        })
        .collect();
    files.sort();
    Ok(files)
}

fn slice_files(files: &[PathBuf], first: usize, count: Option<usize>) -> &[PathBuf] {
    let first = first.min(files.len());
    let end = count.map(|c| (first + c).min(files.len())).unwrap_or(files.len());
    &files[first..end]
}

/// Index into the full sorted file list of the background frame shown at `t`.
pub fn image_index_at(t_ms: f64, fps: f64, offset_ms: f64, first: usize, slice_len: usize) -> usize {
    if slice_len == 0 {
        return first;
    }
    let k = (((t_ms + offset_ms).max(0.0) / 1000.0) * fps).floor() as usize;
    first + k % slice_len
}

struct ImageSeq {
    files: Vec<PathBuf>,
    fps: f64,
    offset_ms: f64,
    w: u32,
    h: u32,
    cached: Option<(usize, Vec<u8>)>,
}

impl BgSource for ImageSeq {
    fn frame(&mut self, t_ms: f64, out: &mut [u8]) -> Result<()> {
        let k = image_index_at(t_ms, self.fps, self.offset_ms, 0, self.files.len());
        if !matches!(&self.cached, Some((ck, _)) if *ck == k) {
            let path = &self.files[k];
            let img = image::open(path).with_context(|| format!("decode {}", path.display()))?.to_rgba8();
            let buf = resize_bilinear(img.as_raw(), img.width(), img.height(), self.w, self.h);
            self.cached = Some((k, buf));
        }
        out.copy_from_slice(&self.cached.as_ref().unwrap().1);
        Ok(())
    }
}

/// Bilinear resize of an RGBA8 image (box-averaged when shrinking by 2x or more).
pub fn resize_bilinear(src: &[u8], sw: u32, sh: u32, dw: u32, dh: u32) -> Vec<u8> {
    if sw == dw && sh == dh {
        return src.to_vec();
    }
    let (sw, sh, dw, dh) = (sw as usize, sh as usize, dw as usize, dh as usize);
    let mut out = vec![0u8; dw * dh * 4];
    let sx = sw as f32 / dw as f32;
    let sy = sh as f32 / dh as f32;
    let xs: Vec<(usize, usize, f32)> = (0..dw)
        .map(|x| {
            let fx = ((x as f32 + 0.5) * sx - 0.5).clamp(0.0, (sw - 1) as f32);
            let x0 = fx.floor() as usize;
            (x0, (x0 + 1).min(sw - 1), fx - x0 as f32)
        })
        .collect();
    for y in 0..dh {
        let fy = ((y as f32 + 0.5) * sy - 0.5).clamp(0.0, (sh - 1) as f32);
        let y0 = fy.floor() as usize;
        let y1 = (y0 + 1).min(sh - 1);
        let wy = fy - y0 as f32;
        for (x, &(x0, x1, wx)) in xs.iter().enumerate() {
            for c in 0..4 {
                let p = |xx: usize, yy: usize| src[(yy * sw + xx) * 4 + c] as f32;
                let top = p(x0, y0) * (1.0 - wx) + p(x1, y0) * wx;
                let bot = p(x0, y1) * (1.0 - wx) + p(x1, y1) * wx;
                out[(y * dw + x) * 4 + c] = (top * (1.0 - wy) + bot * wy + 0.5) as u8;
            }
        }
    }
    out
}

/// Procedural backgrounds. Textures are built once; frames copy them with motion.
struct Procedural {
    kind: ProceduralKind,
    w: usize,
    h: usize,
    tex: Vec<u8>,
    tw: usize,
    th: usize,
    colors: [[f32; 3]; 2],
    shapes: Vec<ShapeDef>,
}

struct ShapeDef {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    r: f32,
    round: bool,
    color: [u8; 3],
}

fn hsv(h: f32, s: f32, v: f32) -> [f32; 3] {
    let h = (h.rem_euclid(1.0)) * 6.0;
    let i = h.floor() as i32;
    let f = h - i as f32;
    let (p, q, t) = (v * (1.0 - s), v * (1.0 - s * f), v * (1.0 - s * (1.0 - f)));
    match i {
        0 => [v, t, p],
        1 => [q, v, p],
        2 => [p, v, t],
        3 => [p, q, v],
        4 => [t, p, v],
        _ => [v, p, q],
    }
}

/// Smooth value noise in [0, 1] from a lattice of random values.
fn value_noise(lattice: &[f32], lw: usize, x: f32, y: f32) -> f32 {
    let (xi, yi) = (x.floor() as usize, y.floor() as usize);
    let (fx, fy) = (x - xi as f32, y - yi as f32);
    let (sx, sy) = (fx * fx * (3.0 - 2.0 * fx), fy * fy * (3.0 - 2.0 * fy));
    let g = |a: usize, b: usize| lattice[(b % lw) * lw + (a % lw)];
    let top = g(xi, yi) * (1.0 - sx) + g(xi + 1, yi) * sx;
    let bot = g(xi, yi + 1) * (1.0 - sx) + g(xi + 1, yi + 1) * sx;
    top * (1.0 - sy) + bot * sy
}

impl Procedural {
    fn new(kind: ProceduralKind, seed: u64, w: u32, h: u32) -> Self {
        let mut rng = Rng::new(seed);
        let (w, h) = (w as usize, h as usize);
        let hue = rng.f32();
        let colors = [hsv(hue, rng.rangef(0.3, 0.8), rng.rangef(0.15, 0.6)), hsv(hue + rng.rangef(-0.2, 0.2), rng.rangef(0.3, 0.9), rng.rangef(0.3, 0.9))];
        let mut p = Procedural { kind, w, h, tex: Vec::new(), tw: 0, th: 0, colors, shapes: Vec::new() };
        match kind {
            ProceduralKind::Gradient => {}
            ProceduralKind::Noise => p.build_noise(&mut rng),
            ProceduralKind::GrassLike => p.build_grass(&mut rng),
            ProceduralKind::MovingShapes => {
                for _ in 0..rng.int(6, 14) {
                    let c = hsv(rng.f32(), rng.rangef(0.3, 1.0), rng.rangef(0.3, 1.0));
                    p.shapes.push(ShapeDef {
                        x: rng.rangef(0.0, w as f32),
                        y: rng.rangef(0.0, h as f32),
                        vx: rng.rangef(-120.0, 120.0),
                        vy: rng.rangef(-80.0, 80.0),
                        r: rng.rangef(20.0, h as f32 * 0.2),
                        round: rng.chance(0.5),
                        color: [(c[0] * 255.0) as u8, (c[1] * 255.0) as u8, (c[2] * 255.0) as u8],
                    });
                }
            }
        }
        p
    }

    fn build_noise(&mut self, rng: &mut Rng) {
        let (tw, th) = (self.w + 256, self.h + 256);
        let lw = 64;
        let lattice: Vec<f32> = (0..lw * lw).map(|_| rng.f32()).collect();
        let hue = rng.f32();
        let mut tex = vec![0u8; tw * th * 4];
        for y in 0..th {
            for x in 0..tw {
                let (fx, fy) = (x as f32, y as f32);
                let n = 0.55 * value_noise(&lattice, lw, fx / 90.0, fy / 90.0)
                    + 0.3 * value_noise(&lattice, lw, fx / 23.0 + 17.0, fy / 23.0 + 5.0)
                    + 0.15 * value_noise(&lattice, lw, fx / 5.0 + 3.0, fy / 5.0 + 11.0);
                let c = hsv(hue + n * 0.3, 0.5, 0.15 + 0.8 * n);
                let i = (y * tw + x) * 4;
                tex[i] = (c[0] * 255.0) as u8;
                tex[i + 1] = (c[1] * 255.0) as u8;
                tex[i + 2] = (c[2] * 255.0) as u8;
                tex[i + 3] = 255;
            }
        }
        self.tex = tex;
        self.tw = tw;
        self.th = th;
    }

    fn build_grass(&mut self, rng: &mut Rng) {
        let (tw, th) = (self.w + 64, self.h);
        let mut tex = vec![0u8; tw * th * 4];
        for y in 0..th {
            let k = y as f32 / th as f32;
            let base = [0.25 + 0.15 * k, 0.45 + 0.25 * k, 0.15 + 0.05 * k];
            for x in 0..tw {
                let i = (y * tw + x) * 4;
                tex[i] = (base[0] * 255.0) as u8;
                tex[i + 1] = (base[1] * 255.0) as u8;
                tex[i + 2] = (base[2] * 255.0) as u8;
                tex[i + 3] = 255;
            }
        }
        // Blades: thin tapered strokes, lighter and darker greens.
        let blades = tw * th / 220;
        for _ in 0..blades {
            let x0 = rng.rangef(0.0, tw as f32);
            let y0 = rng.rangef(th as f32 * 0.2, th as f32 * 1.1);
            let len = rng.rangef(20.0, 90.0);
            let lean = rng.rangef(-0.5, 0.5);
            let g = rng.rangef(0.35, 1.0);
            let col = [(70.0 * g + 20.0) as u8, (150.0 * g + 40.0) as u8, (60.0 * g) as u8];
            let steps = len as usize;
            for s in 0..steps {
                let y = y0 - s as f32;
                let x = x0 + lean * s as f32 * (s as f32 / len);
                let (xi, yi) = (x as isize, y as isize);
                if xi < 0 || yi < 0 || xi as usize >= tw || yi as usize >= th {
                    continue;
                }
                let i = (yi as usize * tw + xi as usize) * 4;
                tex[i..i + 3].copy_from_slice(&col);
            }
        }
        self.tex = tex;
        self.tw = tw;
        self.th = th;
    }
}

impl BgSource for Procedural {
    fn frame(&mut self, t_ms: f64, out: &mut [u8]) -> Result<()> {
        let t = (t_ms / 1000.0) as f32;
        let (w, h) = (self.w, self.h);
        match self.kind {
            ProceduralKind::Gradient | ProceduralKind::MovingShapes => {
                let shift = (t * 0.4).sin() * 0.15;
                for y in 0..h {
                    let k = (y as f32 / h as f32 + shift).clamp(0.0, 1.0);
                    let c: Vec<u8> = (0..3)
                        .map(|i| ((self.colors[0][i] * (1.0 - k) + self.colors[1][i] * k) * 255.0) as u8)
                        .collect();
                    for x in 0..w {
                        let i = (y * w + x) * 4;
                        out[i..i + 3].copy_from_slice(&c);
                        out[i + 3] = 255;
                    }
                }
                if self.kind == ProceduralKind::MovingShapes {
                    for s in &self.shapes {
                        let cx = (s.x + s.vx * t).rem_euclid(w as f32 + 2.0 * s.r) - s.r;
                        let cy = (s.y + s.vy * t).rem_euclid(h as f32 + 2.0 * s.r) - s.r;
                        let (x0, x1) = ((cx - s.r).max(0.0) as usize, ((cx + s.r).max(0.0) as usize).min(w));
                        let (y0, y1) = ((cy - s.r).max(0.0) as usize, ((cy + s.r).max(0.0) as usize).min(h));
                        for y in y0..y1 {
                            for x in x0..x1 {
                                let (dx, dy) = (x as f32 - cx, y as f32 - cy);
                                if s.round && dx * dx + dy * dy > s.r * s.r {
                                    continue;
                                }
                                let i = (y * w + x) * 4;
                                out[i..i + 3].copy_from_slice(&s.color);
                            }
                        }
                    }
                }
            }
            ProceduralKind::Noise => {
                let ox = ((t * 37.0).rem_euclid(256.0)) as usize;
                let oy = ((t * 23.0).rem_euclid(256.0)) as usize;
                for y in 0..h {
                    let src = ((y + oy) * self.tw + ox) * 4;
                    out[y * w * 4..(y + 1) * w * 4].copy_from_slice(&self.tex[src..src + w * 4]);
                }
            }
            ProceduralKind::GrassLike => {
                for y in 0..h {
                    let k = y as f32 / h as f32;
                    let sway = ((t * 1.7 + k * 3.0).sin() * 10.0 * (1.0 - k) + 32.0) as usize;
                    let src = (y * self.tw + sway.min(63)) * 4;
                    out[y * w * 4..(y + 1) * w * 4].copy_from_slice(&self.tex[src..src + w * 4]);
                }
            }
        }
        Ok(())
    }
}

/// `background_text.json` in an image background folder: where the frames contain real text.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct BgTextFile {
    /// Pixel size the region rects are given in (the source frame size).
    pub source_width: u32,
    pub source_height: u32,
    /// Inclusive range of frame indices the generator uses (for example gameplay only, no menus).
    #[serde(default)]
    pub clean_frames: Option<[usize; 2]>,
    pub regions: Vec<BgTextRegion>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct BgTextRegion {
    pub name: String,
    pub rect: Rect,
    /// Inclusive range of 0-based indices into the sorted frame list; `None` means every frame.
    #[serde(default)]
    pub frames: Option<[usize; 2]>,
    /// Known strings shown in this region (used to classify reads that carry no rect).
    #[serde(default)]
    pub texts: Vec<String>,
}

pub const BG_TEXT_FILE: &str = "background_text.json";

/// Loads `<dir>/background_text.json`, else the committed copy `crates/arena/bgtext/<folder name>.json`.
pub fn load_bg_text(dir: &Path) -> Option<BgTextFile> {
    let local = dir.join(BG_TEXT_FILE);
    let committed = dir
        .file_name()
        .map(|n| Path::new(env!("CARGO_MANIFEST_DIR")).join("bgtext").join(n).with_extension("json"));
    let path = if local.is_file() { local } else { committed? };
    let s = std::fs::read_to_string(&path).ok()?;
    match serde_json::from_str(&s) {
        Ok(f) => Some(f),
        Err(e) => {
            eprintln!("warning: ignoring {}: {e}", path.display());
            None
        }
    }
}

/// Background text regions of a scenario, scaled to scenario pixels, with a time lookup.
#[derive(Clone, Debug, Default)]
pub struct BgTextMap {
    regions: Vec<(Rect, Option<[usize; 2]>, Vec<String>)>,
    fps: f64,
    offset_ms: f64,
    first: usize,
    slice_len: usize,
}

impl BgTextMap {
    /// Empty for procedural backgrounds or folders without a sidecar.
    pub fn for_background(bg: &Background, w: u32, h: u32) -> BgTextMap {
        let Background::Images { dir, fps, first_frame, count, offset_ms } = bg else {
            return BgTextMap::default();
        };
        let dir = crate::resolve_path(dir);
        let Some(file) = load_bg_text(&dir) else { return BgTextMap::default() };
        let n_files = image_files(&dir).map(|f| f.len()).unwrap_or(0);
        let first = (*first_frame).min(n_files);
        let slice_len = count.map(|c| c.min(n_files - first)).unwrap_or(n_files - first);
        let sx = w as f32 / file.source_width.max(1) as f32;
        let sy = h as f32 / file.source_height.max(1) as f32;
        BgTextMap {
            regions: file.regions.iter().map(|r| (r.rect.scale(sx, sy), r.frames, r.texts.clone())).collect(),
            fps: *fps,
            offset_ms: *offset_ms,
            first,
            slice_len,
        }
    }

    /// A map from regions already in scenario pixels: (rect, frame range, known texts).
    pub fn new(regions: Vec<(Rect, Option<[usize; 2]>, Vec<String>)>, fps: f64, offset_ms: f64, first: usize, slice_len: usize) -> Self {
        BgTextMap { regions, fps, offset_ms, first, slice_len }
    }

    pub fn is_empty(&self) -> bool {
        self.regions.is_empty()
    }

    /// Time between two background frames, ms (250 when unknown).
    pub fn frame_ms(&self) -> f64 {
        if self.fps > 0.0 { 1000.0 / self.fps } else { 250.0 }
    }

    /// Regions shown at time `t`: (rect, known texts).
    pub fn active_at(&self, t_ms: f64) -> Vec<(&Rect, &[String])> {
        let idx = image_index_at(t_ms, self.fps, self.offset_ms, self.first, self.slice_len);
        self.regions
            .iter()
            .filter(|(_, fr, _)| fr.is_none_or(|[a, b]| idx >= a && idx <= b))
            .map(|(r, _, t)| (r, t.as_slice()))
            .collect()
    }

    /// Every region shown at any time of the scenario's frame slice (for keep-out zones).
    pub fn ever_active(&self) -> Vec<Rect> {
        let (lo, hi) = (self.first, self.first + self.slice_len.saturating_sub(1));
        self.regions
            .iter()
            .filter(|(_, fr, _)| fr.is_none_or(|[a, b]| b >= lo && a <= hi))
            .map(|(r, _, _)| *r)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn procedural_kinds_fill_frames() {
        for kind in [ProceduralKind::Gradient, ProceduralKind::Noise, ProceduralKind::MovingShapes, ProceduralKind::GrassLike] {
            let mut bg = open(&Background::Procedural { kind, seed: 3 }, 64, 36).unwrap();
            let mut a = vec![0u8; 64 * 36 * 4];
            let mut b = a.clone();
            bg.frame(0.0, &mut a).unwrap();
            bg.frame(1500.0, &mut b).unwrap();
            assert!(a.chunks(4).all(|p| p[3] == 255), "{kind:?} opaque");
            assert_ne!(a, b, "{kind:?} moves");
        }
    }

    #[test]
    fn image_index_loops() {
        assert_eq!(image_index_at(0.0, 10.0, 0.0, 5, 3), 5);
        assert_eq!(image_index_at(250.0, 10.0, 0.0, 5, 3), 7);
        assert_eq!(image_index_at(300.0, 10.0, 0.0, 5, 3), 5);
    }

    #[test]
    fn resize_keeps_flat_colour() {
        let src: Vec<u8> = [10u8, 20, 30, 255].repeat(40 * 20);
        let out = resize_bilinear(&src, 40, 20, 17, 9);
        assert!(out.chunks(4).all(|p| p == [10, 20, 30, 255]));
    }
}
