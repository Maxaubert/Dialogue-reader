//! Grey crops and the clean-up done before OCR.

use crate::geom::PxBox;

/// An 8-bit grey image, row-major.
#[derive(Clone, Debug, PartialEq)]
pub struct Gray {
    pub w: u32,
    pub h: u32,
    pub px: Vec<u8>,
}

/// How a colour pixel becomes grey.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Channel {
    /// Rec. 601 luma.
    Luma,
    /// Brightest channel: keeps coloured text (orange, cyan) bright on dark panels.
    Max,
}

impl Gray {
    /// Cuts `b` out of an RGBA8 frame of width `fw`.
    pub fn crop(rgba: &[u8], fw: u32, b: PxBox, ch: Channel) -> Gray {
        let (w, h) = (b.w(), b.h());
        let mut px = Vec::with_capacity((w * h) as usize);
        for y in b.y0..b.y1 {
            let row = &rgba[((y * fw + b.x0) * 4) as usize..((y * fw + b.x1) * 4) as usize];
            match ch {
                Channel::Luma => {
                    px.extend(row.as_chunks::<4>().0.iter().map(|p| ((p[0] as u32 * 77 + p[1] as u32 * 150 + p[2] as u32 * 29) >> 8) as u8))
                }
                Channel::Max => px.extend(row.as_chunks::<4>().0.iter().map(|p| p[0].max(p[1]).max(p[2]))),
            }
        }
        Gray { w, h, px }
    }

    /// Cuts `b` (in this image's pixels) out of a grey image.
    pub fn sub(&self, b: PxBox) -> Gray {
        let mut px = Vec::with_capacity((b.w() * b.h()) as usize);
        for y in b.y0..b.y1 {
            let s = (y * self.w + b.x0) as usize;
            px.extend_from_slice(&self.px[s..s + b.w() as usize]);
        }
        Gray { w: b.w(), h: b.h(), px }
    }

    pub fn get(&self, x: u32, y: u32) -> u8 {
        self.px[(y * self.w + x) as usize]
    }

    /// Bilinear resize.
    pub fn resize(&self, nw: u32, nh: u32) -> Gray {
        if (nw, nh) == (self.w, self.h) {
            return self.clone();
        }
        let (nw, nh) = (nw.max(1), nh.max(1));
        let sx = self.w as f32 / nw as f32;
        let sy = self.h as f32 / nh as f32;
        let mut px = Vec::with_capacity((nw * nh) as usize);
        // Precompute the x taps once per column.
        let xt: Vec<(usize, usize, f32)> = (0..nw)
            .map(|x| {
                let fx = ((x as f32 + 0.5) * sx - 0.5).max(0.0);
                let x0 = (fx as u32).min(self.w - 1);
                let x1 = (x0 + 1).min(self.w - 1);
                (x0 as usize, x1 as usize, fx - x0 as f32)
            })
            .collect();
        let w = self.w as usize;
        for y in 0..nh {
            let fy = ((y as f32 + 0.5) * sy - 0.5).max(0.0);
            let y0 = (fy as u32).min(self.h - 1) as usize;
            let y1 = (y0 + 1).min(self.h as usize - 1);
            let ty = fy - y0 as f32;
            let (r0, r1) = (&self.px[y0 * w..y0 * w + w], &self.px[y1 * w..y1 * w + w]);
            for &(x0, x1, tx) in &xt {
                let a = r0[x0] as f32 * (1.0 - tx) + r0[x1] as f32 * tx;
                let b = r1[x0] as f32 * (1.0 - tx) + r1[x1] as f32 * tx;
                px.push((a * (1.0 - ty) + b * ty + 0.5) as u8);
            }
        }
        Gray { w: nw, h: nh, px }
    }

    /// Mean absolute horizontal step between neighbours. Text is sharp, so a fully faded-in line
    /// scores higher than the same line half faded.
    pub fn sharpness(&self) -> f32 {
        if self.w < 2 || self.h == 0 {
            return 0.0;
        }
        let mut sum = 0u64;
        for row in self.px.chunks_exact(self.w as usize).step_by(2) {
            sum += row.windows(2).map(|p| p[0].abs_diff(p[1]) as u64).sum::<u64>();
        }
        let rows = self.h.div_ceil(2) as u64;
        sum as f32 / (rows * (self.w as u64 - 1)) as f32
    }

    /// Stretches the 2nd to 98th percentile to 0..=255.
    pub fn stretch(&mut self) {
        let mut hist = [0u32; 256];
        for &p in &self.px {
            hist[p as usize] += 1;
        }
        let n = self.px.len() as u32;
        let pick = |q: u32| {
            let mut acc = 0;
            for (v, &c) in hist.iter().enumerate() {
                acc += c;
                if acc > q {
                    return v as u8;
                }
            }
            255
        };
        let (lo, hi) = (pick(n / 50), pick(n - n / 50 - 1));
        if hi <= lo + 8 {
            return;
        }
        let k = 255.0 / (hi - lo) as f32;
        for p in &mut self.px {
            *p = ((p.saturating_sub(lo)) as f32 * k).min(255.0) as u8;
        }
    }

    /// Otsu threshold to black and white.
    pub fn binarize(&mut self) {
        let mut hist = [0u64; 256];
        for &p in &self.px {
            hist[p as usize] += 1;
        }
        let total = self.px.len() as f64;
        let sum: f64 = hist.iter().enumerate().map(|(v, &c)| v as f64 * c as f64).sum();
        let (mut wb, mut sb, mut best, mut thr) = (0.0, 0.0, -1.0, 128u8);
        for (v, &c) in hist.iter().enumerate() {
            wb += c as f64;
            if wb == 0.0 {
                continue;
            }
            let wf = total - wb;
            if wf == 0.0 {
                break;
            }
            sb += v as f64 * c as f64;
            let (mb, mf) = (sb / wb, (sum - sb) / wf);
            let between = wb * wf * (mb - mf) * (mb - mf);
            if between > best {
                best = between;
                thr = v as u8;
            }
        }
        for p in &mut self.px {
            *p = if *p > thr { 255 } else { 0 };
        }
    }

    pub fn invert(&mut self) {
        for p in &mut self.px {
            *p = 255 - *p;
        }
    }

    pub fn mean(&self) -> f32 {
        if self.px.is_empty() {
            return 0.0;
        }
        self.px.iter().map(|&p| p as u64).sum::<u64>() as f32 / self.px.len() as f32
    }

    /// Grey to RGBA8 (opaque), for the OCR engine.
    pub fn to_rgba(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.px.len() * 4);
        for &p in &self.px {
            out.extend_from_slice(&[p, p, p, 255]);
        }
        out
    }
}

/// Clean-up before OCR.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Prep {
    /// Upscale factor (WinRT OCR reads small text better at 2x).
    pub scale: f32,
    /// Stretch contrast first.
    pub stretch: bool,
    /// Otsu black and white.
    pub binarize: bool,
    /// Make the text dark on light: invert when the crop is mostly dark.
    pub dark_text: bool,
    /// Blank border added on every side (in source pixels) so text at the zone edge is not cut.
    pub pad: u32,
}

impl Default for Prep {
    fn default() -> Self {
        Prep { scale: 2.0, stretch: false, binarize: false, dark_text: false, pad: 0 }
    }
}

/// Applies `prep` to a grey crop. `max_dim` caps the output size. Returns the image and the
/// factor from output pixels back to crop pixels, plus the padding offset in output pixels.
pub fn prepare(src: &Gray, prep: &Prep, max_dim: u32) -> (Gray, f32, f32) {
    let mut g = src.clone();
    if prep.stretch {
        g.stretch();
    }
    if prep.pad > 0 {
        g = g.padded(prep.pad);
    }
    let limit = max_dim as f32 / g.w.max(g.h) as f32;
    let s = prep.scale.max(0.1).min(limit);
    let (nw, nh) = (((g.w as f32 * s).round() as u32).max(1), ((g.h as f32 * s).round() as u32).max(1));
    let back = g.w as f32 / nw as f32;
    let mut out = g.resize(nw, nh);
    if prep.binarize {
        out.binarize();
    }
    if prep.dark_text && out.mean() < 128.0 {
        out.invert();
    }
    (out, back, prep.pad as f32)
}

impl Gray {
    /// Adds `p` pixels of edge colour (the crop's median) on every side.
    pub fn padded(&self, p: u32) -> Gray {
        // Median from a histogram (sorting the crop cost milliseconds on big zones).
        let mut hist = [0usize; 256];
        for &v in &self.px {
            hist[v as usize] += 1;
        }
        let half = self.px.len() / 2;
        let mut seen = 0;
        let mut fill = 0u8;
        for (v, &n) in hist.iter().enumerate() {
            seen += n;
            if seen > half {
                fill = v as u8;
                break;
            }
        }
        let (nw, nh) = (self.w + 2 * p, self.h + 2 * p);
        let mut px = vec![fill; (nw * nh) as usize];
        for y in 0..self.h {
            let d = ((y + p) * nw + p) as usize;
            let s = (y * self.w) as usize;
            px[d..d + self.w as usize].copy_from_slice(&self.px[s..s + self.w as usize]);
        }
        Gray { w: nw, h: nh, px }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(w: u32, h: u32, f: impl Fn(u32, u32) -> [u8; 4]) -> Vec<u8> {
        let mut v = Vec::new();
        for y in 0..h {
            for x in 0..w {
                v.extend_from_slice(&f(x, y));
            }
        }
        v
    }

    #[test]
    fn crop_and_channels() {
        let f = frame(4, 3, |x, y| [(x * 10) as u8, 200, (y * 50) as u8, 255]);
        let g = Gray::crop(&f, 4, PxBox { x0: 1, y0: 1, x1: 3, y1: 3 }, Channel::Max);
        assert_eq!((g.w, g.h), (2, 2));
        assert_eq!(g.px, vec![200; 4]);
        let l = Gray::crop(&f, 4, PxBox { x0: 0, y0: 0, x1: 1, y1: 1 }, Channel::Luma);
        assert_eq!(l.px[0], ((200 * 150) >> 8) as u8);
    }

    #[test]
    fn resize_keeps_flat_images_flat() {
        let g = Gray { w: 3, h: 2, px: vec![90; 6] };
        let r = g.resize(7, 5);
        assert_eq!((r.w, r.h), (7, 5));
        assert!(r.px.iter().all(|&p| p == 90));
    }

    #[test]
    fn sharpness_rises_with_contrast() {
        let stripes = |a: u8, b: u8| Gray { w: 8, h: 2, px: (0..16).map(|i| if i % 2 == 0 { a } else { b }).collect() };
        assert!(stripes(0, 255).sharpness() > stripes(100, 150).sharpness());
        assert_eq!(stripes(7, 7).sharpness(), 0.0);
    }

    #[test]
    fn binarize_and_dark_text() {
        // Light text (200) on a dark panel (30): after prep the text must be dark on light.
        let mut px = vec![30u8; 100];
        for p in px.iter_mut().skip(40).take(10) {
            *p = 200;
        }
        let g = Gray { w: 10, h: 10, px };
        let (out, back, pad) = prepare(&g, &Prep { scale: 1.0, stretch: true, binarize: true, dark_text: true, pad: 0 }, 1000);
        assert_eq!((back, pad), (1.0, 0.0));
        assert_eq!(out.px[45], 0);
        assert_eq!(out.px[0], 255);
    }

    #[test]
    fn prepare_scales_and_caps() {
        let g = Gray { w: 100, h: 10, px: vec![0; 1000] };
        let (out, back, _) = prepare(&g, &Prep { scale: 2.0, ..Default::default() }, 150);
        assert_eq!(out.w, 150);
        assert!((back - 100.0 / 150.0).abs() < 1e-6);
        let (p, _, off) = prepare(&g, &Prep { scale: 1.0, pad: 4, ..Default::default() }, 10_000);
        assert_eq!((p.w, p.h, off), (108, 18, 4.0));
    }
}
