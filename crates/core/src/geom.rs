//! Rectangles in frame pixels.

/// Axis-aligned rectangle, frame pixels.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Rect { x, y, w, h }
    }
    pub fn right(&self) -> f32 {
        self.x + self.w
    }
    pub fn bottom(&self) -> f32 {
        self.y + self.h
    }
    pub fn area(&self) -> f32 {
        self.w.max(0.0) * self.h.max(0.0)
    }
    /// Grows the rect by `m` pixels on every side.
    pub fn inflate(&self, m: f32) -> Rect {
        Rect::new(self.x - m, self.y - m, self.w + 2.0 * m, self.h + 2.0 * m)
    }
    /// Smallest rect holding both.
    pub fn union(&self, o: &Rect) -> Rect {
        let x = self.x.min(o.x);
        let y = self.y.min(o.y);
        Rect::new(x, y, self.right().max(o.right()) - x, self.bottom().max(o.bottom()) - y)
    }
    /// Union of many rects (None when empty or all have no area).
    pub fn union_all<'a>(rects: impl IntoIterator<Item = &'a Rect>) -> Option<Rect> {
        rects.into_iter().filter(|r| r.area() > 0.0).fold(None, |u: Option<Rect>, r| Some(u.map_or(*r, |u| u.union(r))))
    }
    /// Vertical overlap of two rects as a share of the smaller height (0..=1).
    pub fn v_overlap(&self, o: &Rect) -> f32 {
        let h = self.bottom().min(o.bottom()) - self.y.max(o.y);
        let m = self.h.min(o.h);
        if h <= 0.0 || m <= 0.0 { 0.0 } else { h / m }
    }
    /// Pixel box clamped to a `fw` x `fh` frame: (x0, y0, x1, y1), or None when empty.
    pub fn to_px(&self, fw: u32, fh: u32) -> Option<PxBox> {
        let x0 = self.x.floor().clamp(0.0, fw as f32) as u32;
        let y0 = self.y.floor().clamp(0.0, fh as f32) as u32;
        let x1 = self.right().ceil().clamp(0.0, fw as f32) as u32;
        let y1 = self.bottom().ceil().clamp(0.0, fh as f32) as u32;
        (x1 > x0 && y1 > y0).then_some(PxBox { x0, y0, x1, y1 })
    }
}

/// Integer pixel box [x0, x1) x [y0, y1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PxBox {
    pub x0: u32,
    pub y0: u32,
    pub x1: u32,
    pub y1: u32,
}

impl PxBox {
    pub fn w(&self) -> u32 {
        self.x1 - self.x0
    }
    pub fn h(&self) -> u32 {
        self.y1 - self.y0
    }
    pub fn rect(&self) -> Rect {
        Rect::new(self.x0 as f32, self.y0 as f32, self.w() as f32, self.h() as f32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamps_and_unions() {
        let r = Rect::new(-5.0, 2.5, 20.0, 10.0);
        assert_eq!(r.to_px(10, 10), Some(PxBox { x0: 0, y0: 2, x1: 10, y1: 10 }));
        assert_eq!(Rect::new(20.0, 0.0, 5.0, 5.0).to_px(10, 10), None);
        let u = Rect::union_all(&[Rect::new(0.0, 0.0, 2.0, 2.0), Rect::new(5.0, 5.0, 1.0, 1.0), Rect::default()]);
        assert_eq!(u, Some(Rect::new(0.0, 0.0, 6.0, 6.0)));
        assert!((Rect::new(0.0, 0.0, 10.0, 10.0).v_overlap(&Rect::new(50.0, 5.0, 10.0, 20.0)) - 0.5).abs() < 1e-6);
    }
}
