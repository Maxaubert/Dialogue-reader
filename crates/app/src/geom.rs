//! Stage mapping and zone drawing math. Pure functions, unit-tested; the Slint `Stage` uses the same mapping.

/// A rectangle in pixels (image pixels or view pixels, as the caller says).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub const fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self { x, y, w, h }
    }

    pub fn contains(&self, px: f32, py: f32) -> bool {
        px >= self.x && px <= self.x + self.w && py >= self.y && py <= self.y + self.h
    }
}

/// How an image of `src` size is shown in a view of `view` size with "cover" (fill, crop the rest, centered).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StageMap {
    pub scale: f32,
    pub off_x: f32,
    pub off_y: f32,
}

impl StageMap {
    pub fn cover(src_w: f32, src_h: f32, view_w: f32, view_h: f32) -> Self {
        let scale = (view_w / src_w).max(view_h / src_h);
        Self {
            scale,
            off_x: (view_w - src_w * scale) / 2.0,
            off_y: (view_h - src_h * scale) / 2.0,
        }
    }

    /// Image px to view px.
    pub fn to_view(&self, r: Rect) -> Rect {
        Rect::new(
            self.off_x + r.x * self.scale,
            self.off_y + r.y * self.scale,
            r.w * self.scale,
            r.h * self.scale,
        )
    }

    /// View px to image px.
    pub fn to_image(&self, r: Rect) -> Rect {
        Rect::new(
            (r.x - self.off_x) / self.scale,
            (r.y - self.off_y) / self.scale,
            r.w / self.scale,
            r.h / self.scale,
        )
    }
}

/// Smallest zone a drag may make, in view px.
pub const MIN_W: f32 = 16.0;
pub const MIN_H: f32 = 12.0;

/// Turns a drag from (x0, y0) to (x1, y1) into a zone inside `bounds`, or `None` when it is too small.
/// The drag may go in any direction; the result is clamped to the bounds.
pub fn drag_to_rect(x0: f32, y0: f32, x1: f32, y1: f32, bounds: Rect) -> Option<Rect> {
    let clamp_x = |v: f32| v.clamp(bounds.x, bounds.x + bounds.w);
    let clamp_y = |v: f32| v.clamp(bounds.y, bounds.y + bounds.h);
    let (ax, bx) = (clamp_x(x0.min(x1)), clamp_x(x0.max(x1)));
    let (ay, by) = (clamp_y(y0.min(y1)), clamp_y(y0.max(y1)));
    let r = Rect::new(ax, ay, bx - ax, by - ay);
    (r.w >= MIN_W && r.h >= MIN_H).then_some(r)
}

/// The zone under a point: the smallest one that holds it, so a small zone inside a big one can be picked.
pub fn hit_zone(zones: &[Rect], px: f32, py: f32) -> Option<usize> {
    zones
        .iter()
        .enumerate()
        .filter(|(_, r)| r.contains(px, py))
        .min_by(|(_, a), (_, b)| (a.w * a.h).total_cmp(&(b.w * b.h)))
        .map(|(i, _)| i)
}

/// The eight resize handles of a zone: corners and edge middles, clockwise from the top left.
pub fn handles(r: Rect) -> [(f32, f32); 8] {
    let (x0, y0, x1, y1) = (r.x, r.y, r.x + r.w, r.y + r.h);
    let (mx, my) = (r.x + r.w / 2.0, r.y + r.h / 2.0);
    [
        (x0, y0),
        (mx, y0),
        (x1, y0),
        (x1, my),
        (x1, y1),
        (mx, y1),
        (x0, y1),
        (x0, my),
    ]
}

/// Index of the handle within `radius` px of the point, if any.
pub fn hit_handle(r: Rect, px: f32, py: f32, radius: f32) -> Option<usize> {
    handles(r)
        .iter()
        .position(|&(hx, hy)| (hx - px).abs() <= radius && (hy - py).abs() <= radius)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cover_16_9_stage_scales_evenly() {
        let m = StageMap::cover(1600.0, 900.0, 872.0, 490.5);
        assert!((m.scale - 0.545).abs() < 1e-4);
        assert!(m.off_x.abs() < 1e-3 && m.off_y.abs() < 1e-3);
    }

    #[test]
    fn cover_crops_the_sides_on_a_wider_source() {
        // the zone edit frame: 1600 x 900 scene in a 1440 x 900 window
        let m = StageMap::cover(1600.0, 900.0, 1440.0, 900.0);
        assert_eq!(m.scale, 1.0);
        assert_eq!(m.off_x, -80.0);
        assert_eq!(m.off_y, 0.0);
    }

    #[test]
    fn view_and_image_mapping_round_trip() {
        let m = StageMap::cover(3840.0, 2160.0, 1016.0, 571.5);
        let r = Rect::new(700.0, 1880.0, 2440.0, 160.0);
        let back = m.to_image(m.to_view(r));
        for (a, b) in [(r.x, back.x), (r.y, back.y), (r.w, back.w), (r.h, back.h)] {
            assert!((a - b).abs() < 1e-2, "{a} vs {b}");
        }
    }

    #[test]
    fn drag_in_any_direction_gives_the_same_rect() {
        let b = Rect::new(0.0, 0.0, 1440.0, 900.0);
        let a = drag_to_rect(930.0, 150.0, 1290.0, 262.0, b).unwrap();
        let c = drag_to_rect(1290.0, 262.0, 930.0, 150.0, b).unwrap();
        assert_eq!(a, c);
        assert_eq!(a, Rect::new(930.0, 150.0, 360.0, 112.0));
    }

    #[test]
    fn drag_too_small_is_refused() {
        let b = Rect::new(0.0, 0.0, 1440.0, 900.0);
        assert_eq!(drag_to_rect(10.0, 10.0, 20.0, 40.0, b), None);
        assert_eq!(drag_to_rect(10.0, 10.0, 60.0, 15.0, b), None);
        assert!(drag_to_rect(10.0, 10.0, 26.0, 22.0, b).is_some());
    }

    #[test]
    fn drag_is_clamped_to_the_image() {
        let b = Rect::new(0.0, 0.0, 1440.0, 900.0);
        let r = drag_to_rect(-50.0, 800.0, 200.0, 1000.0, b).unwrap();
        assert_eq!(r, Rect::new(0.0, 800.0, 200.0, 100.0));
    }

    #[test]
    fn hit_picks_the_smallest_zone() {
        let zones = [
            Rect::new(0.0, 0.0, 500.0, 500.0),
            Rect::new(100.0, 100.0, 50.0, 50.0),
        ];
        assert_eq!(hit_zone(&zones, 120.0, 120.0), Some(1));
        assert_eq!(hit_zone(&zones, 300.0, 300.0), Some(0));
        assert_eq!(hit_zone(&zones, 600.0, 10.0), None);
    }

    #[test]
    fn handles_hit_test() {
        let r = Rect::new(100.0, 100.0, 200.0, 50.0);
        assert_eq!(hit_handle(r, 101.0, 99.0, 5.0), Some(0));
        assert_eq!(hit_handle(r, 300.0, 125.0, 5.0), Some(3));
        assert_eq!(hit_handle(r, 200.0, 150.0, 5.0), Some(5));
        assert_eq!(hit_handle(r, 200.0, 125.0, 5.0), None);
    }
}
