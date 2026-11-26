//! Cheap change detection: a zone is cut into cells and each cell keeps its mean grey level and
//! its mean horizontal step (edge strength). Comparing two signatures costs almost nothing.

use crate::image::Gray;

/// Cell means of one grey crop.
#[derive(Clone, Debug, PartialEq)]
pub struct Signature {
    pub cols: u32,
    pub rows: u32,
    /// Mean grey per cell.
    pub mean: Vec<u8>,
    /// Mean |left - right| step per cell (text strokes make this high).
    pub edge: Vec<u8>,
}

/// How different two signatures are.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Delta {
    /// Largest change of one cell (mean or edge), 0..=255.
    pub max: f32,
    /// Share of cells that changed by more than the cell threshold.
    pub share: f32,
}

impl Signature {
    /// `cell` is the cell size in pixels.
    pub fn of(g: &Gray, cell: u32) -> Signature {
        let cell = cell.max(2);
        let cols = g.w.div_ceil(cell).max(1);
        let rows = g.h.div_ceil(cell).max(1);
        let n = (cols * rows) as usize;
        let mut sum = vec![0u32; n];
        let mut esum = vec![0u32; n];
        let mut cnt = vec![0u32; n];
        let w = g.w as usize;
        for y in 0..g.h {
            let row = &g.px[y as usize * w..y as usize * w + w];
            let base = ((y / cell) * cols) as usize;
            for (x, &p) in row.iter().enumerate() {
                let c = base + x / cell as usize;
                sum[c] += p as u32;
                cnt[c] += 1;
                if x + 1 < w {
                    esum[c] += p.abs_diff(row[x + 1]) as u32;
                }
            }
        }
        let mean = sum.iter().zip(&cnt).map(|(&s, &c)| (s / c.max(1)) as u8).collect();
        let edge = esum.iter().zip(&cnt).map(|(&s, &c)| (s / c.max(1)).min(255) as u8).collect();
        Signature { cols, rows, mean, edge }
    }

    /// Compares with `o`. A cell counts as changed when its mean or edge moved by more than
    /// `cell_thresh`. Different shapes count as fully changed.
    pub fn delta(&self, o: &Signature, cell_thresh: f32) -> Delta {
        if (self.cols, self.rows) != (o.cols, o.rows) {
            return Delta { max: 255.0, share: 1.0 };
        }
        let mut max = 0u8;
        let mut changed = 0usize;
        for i in 0..self.mean.len() {
            let d = self.mean[i].abs_diff(o.mean[i]).max(self.edge[i].abs_diff(o.edge[i]));
            max = max.max(d);
            if d as f32 > cell_thresh {
                changed += 1;
            }
        }
        Delta { max: max as f32, share: changed as f32 / self.mean.len().max(1) as f32 }
    }

    /// Mean edge strength over all cells: high when the zone holds text (or busy texture).
    pub fn edge_level(&self) -> f32 {
        self.edge.iter().map(|&e| e as f32).sum::<f32>() / self.edge.len().max(1) as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn img(w: u32, h: u32, f: impl Fn(u32, u32) -> u8) -> Gray {
        Gray { w, h, px: (0..h).flat_map(|y| (0..w).map(move |x| (x, y))).map(|(x, y)| f(x, y)).collect() }
    }

    #[test]
    fn unchanged_zone_has_zero_delta() {
        let a = img(40, 16, |x, y| ((x * 7 + y * 3) % 255) as u8);
        let (sa, sb) = (Signature::of(&a, 8), Signature::of(&a.clone(), 8));
        assert_eq!(sa.delta(&sb, 4.0), Delta { max: 0.0, share: 0.0 });
    }

    #[test]
    fn small_glyph_change_is_seen() {
        // A "digit" (bright 4x6 block) changes shape in one cell only.
        let a = img(64, 16, |x, y| if (8..12).contains(&x) && (4..10).contains(&y) { 250 } else { 20 });
        let b = img(64, 16, |x, y| if (8..10).contains(&x) && (4..10).contains(&y) { 250 } else { 20 });
        let d = Signature::of(&a, 8).delta(&Signature::of(&b, 8), 6.0);
        assert!(d.max > 20.0, "{d:?}");
        assert!(d.share > 0.0 && d.share < 0.2, "{d:?}");
    }

    #[test]
    fn edges_are_high_on_text_like_stripes() {
        let flat = Signature::of(&img(32, 8, |_, _| 128), 8);
        let text = Signature::of(&img(32, 8, |x, _| if x % 3 == 0 { 255 } else { 0 }), 8);
        assert_eq!(flat.edge_level(), 0.0);
        assert!(text.edge_level() > 100.0);
        assert_eq!(flat.delta(&Signature::of(&img(40, 8, |_, _| 128), 8), 1.0).share, 1.0);
    }
}
