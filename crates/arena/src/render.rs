//! CPU renderer: draws text items (outline, blurred shadow, rounded panel, fades, slides,
//! typewriter) over a background into an RGBA8 frame.

use crate::background::{self, BgSource};
use crate::fonts;
use crate::model::{Align, Color, Scenario, TextItem};
use ab_glyph::{Font, PxScale, point};
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::Path;

/// Single-channel coverage mask.
#[derive(Clone, Debug)]
pub struct Mask {
    pub w: usize,
    pub h: usize,
    pub data: Vec<f32>,
}

impl Mask {
    pub fn new(w: usize, h: usize) -> Self {
        Mask { w, h, data: vec![0.0; w * h] }
    }
    fn at(&self, x: isize, y: isize) -> f32 {
        if x < 0 || y < 0 || x as usize >= self.w || y as usize >= self.h {
            0.0
        } else {
            self.data[y as usize * self.w + x as usize]
        }
    }
}

/// Max filter with a round kernel of radius `r` px (outline growth).
pub fn dilate(m: &Mask, r: f32) -> Mask {
    if r <= 0.0 {
        return m.clone();
    }
    let ri = r.ceil() as isize;
    let mut offsets = Vec::new();
    for dy in -ri..=ri {
        for dx in -ri..=ri {
            let d = ((dx * dx + dy * dy) as f32).sqrt();
            // Soft edge on the last ring keeps the outline anti-aliased.
            let wgt = (r + 0.5 - d).clamp(0.0, 1.0);
            if wgt > 0.0 {
                offsets.push((dx, dy, wgt));
            }
        }
    }
    let mut out = Mask::new(m.w, m.h);
    for y in 0..m.h as isize {
        for x in 0..m.w as isize {
            let mut best = 0.0f32;
            for &(dx, dy, wgt) in &offsets {
                let v = m.at(x + dx, y + dy) * wgt;
                if v > best {
                    best = v;
                    if best >= 1.0 {
                        break;
                    }
                }
            }
            out.data[y as usize * m.w + x as usize] = best;
        }
    }
    out
}

fn box_blur_pass(src: &[f32], dst: &mut [f32], w: usize, h: usize, r: usize, horizontal: bool) {
    let (n_lines, len) = if horizontal { (h, w) } else { (w, h) };
    let idx = |line: usize, i: usize| if horizontal { line * w + i } else { i * w + line };
    let norm = 1.0 / (2 * r + 1) as f32;
    for line in 0..n_lines {
        let mut acc = 0.0f32;
        for i in 0..=r.min(len.saturating_sub(1)) {
            acc += src[idx(line, i)];
        }
        for i in 0..len {
            dst[idx(line, i)] = acc * norm;
            let add = i + r + 1;
            if add < len {
                acc += src[idx(line, add)];
            }
            if i >= r {
                acc -= src[idx(line, i - r)];
            }
        }
    }
}

/// Approximate Gaussian blur (three box passes) with about `sigma` px spread.
pub fn blur(m: &Mask, sigma: f32) -> Mask {
    if sigma < 0.5 {
        return m.clone();
    }
    let r = ((sigma * 0.85).round() as usize).max(1);
    let mut a = m.data.clone();
    let mut b = vec![0.0; a.len()];
    for _ in 0..3 {
        box_blur_pass(&a, &mut b, m.w, m.h, r, true);
        box_blur_pass(&b, &mut a, m.w, m.h, r, false);
    }
    Mask { w: m.w, h: m.h, data: a }
}

/// Coverage of a rounded rectangle at pixel (px, py), anti-aliased.
fn rounded_rect_coverage(px: f32, py: f32, x: f32, y: f32, w: f32, h: f32, r: f32) -> f32 {
    let r = r.min(w / 2.0).min(h / 2.0).max(0.0);
    let cx = px.clamp(x + r, x + w - r);
    let cy = py.clamp(y + r, y + h - r);
    let (dx, dy) = (px - cx, py - cy);
    let d = (dx * dx + dy * dy).sqrt() - r;
    // d is distance outside the shape (negative inside). For r = 0 it is the box distance.
    let d = if r == 0.0 {
        let ox = (x - px).max(px - (x + w)).max(0.0);
        let oy = (y - py).max(py - (y + h)).max(0.0);
        if ox > 0.0 || oy > 0.0 { (ox * ox + oy * oy).sqrt() } else { -1.0 }
    } else {
        d
    };
    (0.5 - d).clamp(0.0, 1.0)
}

/// Premultiplied RGBA float image of one item at one typewriter state.
#[derive(Clone, Debug)]
pub struct Sprite {
    pub w: usize,
    pub h: usize,
    /// Sprite top-left relative to the item's rect top-left.
    pub off_x: i32,
    pub off_y: i32,
    pub px: Vec<[f32; 4]>,
}

fn over(dst: &mut [f32; 4], c: Color, cov: f32) {
    let a = (c[3] as f32 / 255.0) * cov;
    if a <= 0.0 {
        return;
    }
    let inv = 1.0 - a;
    dst[0] = c[0] as f32 / 255.0 * a + dst[0] * inv;
    dst[1] = c[1] as f32 / 255.0 * a + dst[1] * inv;
    dst[2] = c[2] as f32 / 255.0 * a + dst[2] * inv;
    dst[3] = a + dst[3] * inv;
}

/// Number of characters shown at time `t` (typewriter), or all of them.
pub fn visible_chars(item: &TextItem, t: f64) -> usize {
    let total = item.text.chars().count();
    match item.anim.typewriter_cps {
        Some(cps) if cps > 0.0 => {
            let n = ((t - item.start_ms).max(0.0) / 1000.0 * cps as f64).floor() as usize + 1;
            n.min(total)
        }
        _ => total,
    }
}

/// Builds the sprite of `item` showing its first `n_chars` characters.
pub fn build_sprite(item: &TextItem, n_chars: usize) -> Result<Sprite> {
    let st = &item.style;
    let font = fonts::font(&st.font, st.bold, st.italic)
        .with_context(|| format!("no font file found for {:?} or any fallback", st.font))?;
    let lay = fonts::layout(&font, st.size_px, st.line_spacing, &item.text);
    let outline_px = st.outline.as_ref().map(|o| o.px).unwrap_or(0.0);
    let (sh_dx, sh_dy, sh_blur) =
        st.shadow.as_ref().map(|s| (s.dx, s.dy, s.blur)).unwrap_or((0.0, 0.0, 0.0));
    let margin = (outline_px + sh_dx.abs().max(sh_dy.abs()) + 3.0 * sh_blur + 2.0).ceil() as i32;
    let rect = item.rect;
    let w = (rect.w.ceil() as i32 + 2 * margin).max(1) as usize;
    let h = (rect.h.ceil() as i32 + 2 * margin).max(1) as usize;

    // Where the text block starts, relative to the sprite.
    let pad = st.panel.as_ref().map(|p| p.padding).unwrap_or(0.0);
    let inner_w = rect.w - 2.0 * pad;
    let inner_h = rect.h - 2.0 * pad;
    let text_y = margin as f32 + pad + if st.panel.is_some() { 0.0 } else { ((inner_h - lay.height) / 2.0).max(0.0) };

    let mut text = Mask::new(w, h);
    let scale = PxScale::from(st.size_px);
    for (gi, g) in lay.glyphs.iter().enumerate() {
        if g.char_index >= n_chars {
            break;
        }
        let line_w = lay.line_widths[lay.line_of_glyph[gi]];
        let line_x = match st.align {
            Align::Left => 0.0,
            Align::Center => ((inner_w - line_w) / 2.0).max(0.0),
        };
        let gx = margin as f32 + pad + line_x + g.x;
        let gy = text_y + g.baseline;
        let glyph = g.id.with_scale_and_position(scale, point(gx, gy));
        if let Some(outlined) = font.outline_glyph(glyph) {
            let b = outlined.px_bounds();
            outlined.draw(|x, y, c| {
                let px = b.min.x as i32 + x as i32;
                let py = b.min.y as i32 + y as i32;
                if px >= 0 && py >= 0 && (px as usize) < w && (py as usize) < h {
                    let i = py as usize * w + px as usize;
                    text.data[i] = (text.data[i] + c).min(1.0);
                }
            });
        }
    }

    let outline = st.outline.as_ref().map(|o| dilate(&text, o.px));
    let mut px = vec![[0.0f32; 4]; w * h];

    if let Some(p) = &st.panel {
        let col = [p.color[0], p.color[1], p.color[2], (p.alpha.clamp(0.0, 1.0) * 255.0) as u8];
        let (x0, y0) = (margin as f32, margin as f32);
        for y in 0..h {
            for x in 0..w {
                let cov = rounded_rect_coverage(x as f32 + 0.5, y as f32 + 0.5, x0, y0, rect.w, rect.h, p.radius);
                if cov > 0.0 {
                    over(&mut px[y * w + x], col, cov);
                }
            }
        }
    }

    if let Some(s) = &st.shadow {
        let shape = match &outline {
            Some(o) => o,
            None => &text,
        };
        let mut shifted = Mask::new(w, h);
        let (dx, dy) = (s.dx.round() as isize, s.dy.round() as isize);
        for y in 0..h as isize {
            for x in 0..w as isize {
                shifted.data[y as usize * w + x as usize] = shape.at(x - dx, y - dy);
            }
        }
        let sm = blur(&shifted, s.blur);
        for (d, &c) in px.iter_mut().zip(&sm.data) {
            over(d, s.color, c);
        }
    }

    if let (Some(o), Some(om)) = (&st.outline, &outline) {
        for (d, &c) in px.iter_mut().zip(&om.data) {
            over(d, o.color, c);
        }
    }
    for (d, &c) in px.iter_mut().zip(&text.data) {
        over(d, st.color, c);
    }

    Ok(Sprite { w, h, off_x: -margin, off_y: -margin, px })
}

/// Opacity and slide offset of an item at time `t` (opacity 0 when hidden).
pub fn anim_state(item: &TextItem, t: f64) -> (f32, f32, f32) {
    if !item.visible_at(t) {
        return (0.0, 0.0, 0.0);
    }
    let a = &item.anim;
    let since = t - item.start_ms;
    let until = item.end_ms - t;
    let mut alpha = 1.0f64;
    if a.fade_in_ms > 0.0 && since < a.fade_in_ms {
        alpha = alpha.min(since / a.fade_in_ms);
    }
    if a.fade_out_ms > 0.0 && until < a.fade_out_ms {
        alpha = alpha.min(until / a.fade_out_ms);
    }
    let (mut dx, mut dy) = (0.0f32, 0.0f32);
    if a.slide_ms > 0.0 {
        if since < a.slide_ms {
            let k = 1.0 - ease(since / a.slide_ms) as f32;
            dx += a.slide_in[0] * k;
            dy += a.slide_in[1] * k;
        }
        if until < a.slide_ms {
            let k = 1.0 - ease(until / a.slide_ms) as f32;
            dx += a.slide_out[0] * k;
            dy += a.slide_out[1] * k;
        }
    }
    (alpha.clamp(0.0, 1.0) as f32, dx, dy)
}

fn ease(p: f64) -> f64 {
    let p = p.clamp(0.0, 1.0);
    1.0 - (1.0 - p) * (1.0 - p)
}

/// Draws a sprite onto an opaque RGBA8 frame with extra opacity `alpha`.
pub fn blit(frame: &mut [u8], fw: usize, fh: usize, s: &Sprite, x: i32, y: i32, alpha: f32) {
    if alpha <= 0.0 {
        return;
    }
    for sy in 0..s.h {
        let fy = y + sy as i32;
        if fy < 0 || fy as usize >= fh {
            continue;
        }
        for sx in 0..s.w {
            let fx = x + sx as i32;
            if fx < 0 || fx as usize >= fw {
                continue;
            }
            let p = s.px[sy * s.w + sx];
            let a = p[3] * alpha;
            if a <= 0.0 {
                continue;
            }
            let i = (fy as usize * fw + fx as usize) * 4;
            for c in 0..3 {
                let d = frame[i + c] as f32 / 255.0;
                let v = p[c] * alpha + d * (1.0 - a);
                frame[i + c] = (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
            }
            frame[i + 3] = 255;
        }
    }
}

/// Renders the frames of one scenario. Sprites are cached per item and typewriter state.
pub struct SceneRenderer {
    pub scenario: Scenario,
    bg: Box<dyn BgSource>,
    sprites: HashMap<usize, (usize, Sprite)>,
}

impl SceneRenderer {
    pub fn new(scenario: Scenario) -> Result<Self> {
        let bg = background::open(&scenario.background, scenario.width, scenario.height)?;
        Ok(SceneRenderer { scenario, bg, sprites: HashMap::new() })
    }

    /// Draws the frame at `t` into `out` (RGBA8, width * height * 4 bytes).
    pub fn render(&mut self, t: f64, out: &mut Vec<u8>) -> Result<()> {
        let (w, h) = (self.scenario.width as usize, self.scenario.height as usize);
        out.resize(w * h * 4, 0);
        self.bg.frame(t, out)?;
        for (i, item) in self.scenario.items.iter().enumerate() {
            let (alpha, dx, dy) = anim_state(item, t);
            if alpha <= 0.0 {
                self.sprites.remove(&i);
                continue;
            }
            let n = visible_chars(item, t);
            let fresh = !matches!(self.sprites.get(&i), Some((cn, _)) if *cn == n);
            if fresh {
                self.sprites.insert(i, (n, build_sprite(item, n)?));
            }
            let s = &self.sprites[&i].1;
            let x = (item.rect.x + dx).round() as i32 + s.off_x;
            let y = (item.rect.y + dy).round() as i32 + s.off_y;
            blit(out, w, h, s, x, y, alpha);
        }
        Ok(())
    }
}

/// Writes an RGBA8 frame as PNG.
pub fn save_png(path: &Path, w: u32, h: u32, rgba: &[u8]) -> Result<()> {
    let img = image::RgbaImage::from_raw(w, h, rgba.to_vec()).context("frame buffer size")?;
    img.save(path).with_context(|| format!("write {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::*;

    fn item(style: Style) -> TextItem {
        TextItem {
            id: "a".into(),
            text: "Hello".into(),
            kind: ItemKind::Subtitle,
            should_read: true,
            rect: Rect::new(10.0, 10.0, 120.0, 40.0),
            start_ms: 0.0,
            end_ms: 1000.0,
            style,
            anim: Anim { fade_in_ms: 200.0, fade_out_ms: 200.0, ..Default::default() },
        }
    }

    fn style() -> Style {
        Style {
            font: "Segoe UI".into(),
            size_px: 28.0,
            color: [255, 255, 255, 255],
            outline: Some(Outline { color: [0, 0, 0, 255], px: 2.0 }),
            shadow: Some(Shadow { color: [0, 0, 0, 200], dx: 2.0, dy: 2.0, blur: 3.0 }),
            panel: None,
            bold: false,
            italic: false,
            align: Align::Left,
            line_spacing: 1.25,
        }
    }

    #[test]
    fn fades_are_partial() {
        let it = item(style());
        assert_eq!(anim_state(&it, -1.0).0, 0.0);
        let (a, _, _) = anim_state(&it, 100.0);
        assert!((a - 0.5).abs() < 1e-3);
        assert_eq!(anim_state(&it, 500.0).0, 1.0);
        let (a, _, _) = anim_state(&it, 900.0);
        assert!((a - 0.5).abs() < 1e-3);
        assert_eq!(anim_state(&it, 1000.0).0, 0.0);
    }

    #[test]
    fn typewriter_counts_chars() {
        let mut it = item(style());
        it.anim.typewriter_cps = Some(10.0);
        assert_eq!(visible_chars(&it, 0.0), 1);
        assert_eq!(visible_chars(&it, 250.0), 3);
        assert_eq!(visible_chars(&it, 5000.0), 5);
    }

    #[test]
    fn sprite_has_text_and_outline() {
        if fonts::font("Segoe UI", false, false).is_none() {
            return; // no fonts on this machine
        }
        let s = build_sprite(&item(style()), 5).unwrap();
        let opaque = s.px.iter().filter(|p| p[3] > 0.9).count();
        let white = s.px.iter().filter(|p| p[3] > 0.9 && p[0] > 0.9).count();
        let dark = s.px.iter().filter(|p| p[3] > 0.9 && p[0] < 0.1).count();
        assert!(white > 50, "text pixels {white}");
        assert!(dark > 50, "outline pixels {dark}");
        assert!(opaque > white);
    }

    #[test]
    fn rounded_panel_corners_are_transparent() {
        let mut st = style();
        st.outline = None;
        st.shadow = None;
        st.panel = Some(Panel { color: [20, 20, 40, 255], alpha: 0.8, padding: 6.0, radius: 10.0 });
        if fonts::font("Segoe UI", false, false).is_none() {
            return;
        }
        let s = build_sprite(&item(st), 0).unwrap();
        let m = (-s.off_x) as usize;
        let corner = s.px[m * s.w + m];
        let middle = s.px[(m + 20) * s.w + m + 60];
        assert!(corner[3] < 0.3, "corner alpha {}", corner[3]);
        assert!((middle[3] - 0.8).abs() < 0.02, "panel alpha {}", middle[3]);
    }
}
