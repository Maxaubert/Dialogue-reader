//! Zone drawing on a screenshot (pages 03 and 11): draw, select, move, resize, rename, re-kind, switch on or
//! off, delete, and undo of the last change. Plain Rust, no Slint: the stage sends pointer events in image
//! pixels and reads the zones back.

use dr_runtime::Kind;

use crate::geom::{Rect, hit_handle, hit_zone};

/// Smallest zone, in view pixels (as `geom::MIN_W` / `MIN_H`). Divided by the stage scale for image pixels.
const MIN_W: f32 = crate::geom::MIN_W;
const MIN_H: f32 = crate::geom::MIN_H;
/// How close to a handle a press must be, in view pixels.
const HANDLE_RADIUS: f32 = 6.0;
/// A press that moves less than this (view pixels) is a click.
const CLICK_SLOP: f32 = 3.0;

#[derive(Clone, Debug, PartialEq)]
pub struct EdZone {
    pub name: String,
    pub kind: Kind,
    pub on: bool,
    /// In image pixels of the screenshot.
    pub rect: Rect,
}

#[derive(Clone, Debug, PartialEq)]
enum Drag {
    None,
    /// A press on empty space (or anywhere in draw mode): a drag draws a zone, a click selects.
    Draw {
        x0: f32,
        y0: f32,
        x1: f32,
        y1: f32,
    },
    /// A press on a zone in select mode: a drag moves it.
    Move {
        i: usize,
        x0: f32,
        y0: f32,
        orig: Rect,
        before: Vec<EdZone>,
    },
    /// A press on a handle of the selected zone.
    Resize {
        i: usize,
        handle: usize,
        orig: Rect,
        before: Vec<EdZone>,
    },
}

/// One snapshot for undo.
#[derive(Clone, Debug, PartialEq)]
struct Undo {
    zones: Vec<EdZone>,
    selected: Option<usize>,
}

#[derive(Clone, Debug)]
pub struct Editor {
    pub zones: Vec<EdZone>,
    pub selected: Option<usize>,
    /// Draw tool (true) or select tool (false).
    pub draw: bool,
    /// The kind a new zone gets.
    pub kind: Kind,
    /// Screenshot size in pixels.
    pub img_w: f32,
    pub img_h: f32,
    drag: Drag,
    undo: Option<Undo>,
}

/// What a pointer event changed, so the caller knows what to push and whether to save.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Changed {
    /// The zone list changed (save it).
    pub zones: bool,
    /// Only the selection or the draft box changed.
    pub view: bool,
}

impl Editor {
    pub fn new(zones: Vec<EdZone>, img_w: f32, img_h: f32) -> Editor {
        Editor {
            zones,
            selected: None,
            draw: true,
            kind: Kind::Dialogue,
            img_w,
            img_h,
            drag: Drag::None,
            undo: None,
        }
    }

    fn bounds(&self) -> Rect {
        Rect::new(0.0, 0.0, self.img_w, self.img_h)
    }

    fn rects(&self) -> Vec<Rect> {
        self.zones.iter().map(|z| z.rect).collect()
    }

    fn snapshot(&self) -> Undo {
        Undo {
            zones: self.zones.clone(),
            selected: self.selected,
        }
    }

    /// The box being drawn, if any (image pixels, normalized and clamped).
    pub fn draft(&self) -> Option<Rect> {
        match self.drag {
            Drag::Draw { x0, y0, x1, y1 } if (x1 - x0).abs() > 0.0 || (y1 - y0).abs() > 0.0 => {
                let b = self.bounds();
                let (ax, bx) = (x0.min(x1).clamp(0.0, b.w), x0.max(x1).clamp(0.0, b.w));
                let (ay, by) = (y0.min(y1).clamp(0.0, b.h), y0.max(y1).clamp(0.0, b.h));
                Some(Rect::new(ax, ay, bx - ax, by - ay))
            }
            _ => None,
        }
    }

    pub fn can_undo(&self) -> bool {
        self.undo.is_some()
    }

    /// Press at (x, y) image pixels; `scale` is view pixels per image pixel.
    pub fn pointer_down(&mut self, x: f32, y: f32, scale: f32) -> Changed {
        let scale = scale.max(1e-3);
        if let Some(i) = self.selected
            && let Some(handle) = hit_handle(self.zones[i].rect, x, y, HANDLE_RADIUS / scale)
        {
            self.drag = Drag::Resize {
                i,
                handle,
                orig: self.zones[i].rect,
                before: self.zones.clone(),
            };
            return Changed::default();
        }
        if !self.draw
            && let Some(i) = hit_zone(&self.rects(), x, y)
        {
            let was = self.selected;
            self.selected = Some(i);
            self.drag = Drag::Move {
                i,
                x0: x,
                y0: y,
                orig: self.zones[i].rect,
                before: self.zones.clone(),
            };
            return Changed {
                zones: false,
                view: was != self.selected,
            };
        }
        self.drag = Drag::Draw {
            x0: x,
            y0: y,
            x1: x,
            y1: y,
        };
        Changed::default()
    }

    pub fn pointer_move(&mut self, x: f32, y: f32, scale: f32) -> Changed {
        let scale = scale.max(1e-3);
        let b = self.bounds();
        match &mut self.drag {
            Drag::None => Changed::default(),
            Drag::Draw { x1, y1, .. } => {
                (*x1, *y1) = (x, y);
                Changed {
                    zones: false,
                    view: true,
                }
            }
            Drag::Move {
                i, x0, y0, orig, ..
            } => {
                let (i, o) = (*i, *orig);
                let nx = (o.x + x - *x0).clamp(0.0, (b.w - o.w).max(0.0));
                let ny = (o.y + y - *y0).clamp(0.0, (b.h - o.h).max(0.0));
                if let Some(z) = self.zones.get_mut(i) {
                    z.rect = Rect::new(nx, ny, o.w, o.h);
                }
                Changed {
                    zones: false,
                    view: true,
                }
            }
            Drag::Resize {
                i, handle, orig, ..
            } => {
                let (i, h, o) = (*i, *handle, *orig);
                if let Some(z) = self.zones.get_mut(i) {
                    z.rect = resize(o, h, x, y, b, MIN_W / scale, MIN_H / scale);
                }
                Changed {
                    zones: false,
                    view: true,
                }
            }
        }
    }

    pub fn pointer_up(&mut self, x: f32, y: f32, scale: f32) -> Changed {
        let scale = scale.max(1e-3);
        self.pointer_move(x, y, scale);
        match std::mem::replace(&mut self.drag, Drag::None) {
            Drag::None => Changed::default(),
            Drag::Draw { x0, y0, x1, y1 } => {
                let moved = (x1 - x0).abs().max((y1 - y0).abs()) * scale;
                let r = crate::geom::drag_to_rect(x0, y0, x1, y1, self.bounds());
                let big = r.is_some_and(|r| r.w * scale >= MIN_W && r.h * scale >= MIN_H);
                if moved >= CLICK_SLOP && big {
                    let r = r.expect("checked");
                    self.undo = Some(self.snapshot());
                    let name = self.new_name(self.kind);
                    self.zones.push(EdZone {
                        name,
                        kind: self.kind,
                        on: true,
                        rect: r,
                    });
                    self.selected = Some(self.zones.len() - 1);
                    return Changed {
                        zones: true,
                        view: true,
                    };
                }
                if moved < CLICK_SLOP {
                    // a click: select the zone under it, or nothing
                    self.selected = hit_zone(&self.rects(), x0, y0);
                    return Changed {
                        zones: false,
                        view: true,
                    };
                }
                // a drag too small for a zone
                Changed {
                    zones: false,
                    view: true,
                }
            }
            Drag::Move {
                i, orig, before, ..
            }
            | Drag::Resize {
                i, orig, before, ..
            } => {
                if self.zones.get(i).is_some_and(|z| z.rect != orig) {
                    self.undo = Some(Undo {
                        zones: before,
                        selected: Some(i),
                    });
                    Changed {
                        zones: true,
                        view: true,
                    }
                } else {
                    Changed {
                        zones: false,
                        view: true,
                    }
                }
            }
        }
    }

    /// "Dialogue", then "Dialogue 2", ...
    fn new_name(&self, kind: Kind) -> String {
        let base = kind.ui_name();
        if !self.zones.iter().any(|z| z.name == base) {
            return base.to_string();
        }
        (2..)
            .map(|n| format!("{base} {n}"))
            .find(|c| !self.zones.iter().any(|z| &z.name == c))
            .expect("a free name")
    }

    pub fn select(&mut self, i: Option<usize>) {
        self.selected = i.filter(|&i| i < self.zones.len());
        if let Some(i) = self.selected {
            self.kind = self.zones[i].kind;
        }
    }

    pub fn delete_selected(&mut self) -> bool {
        let Some(i) = self.selected.filter(|&i| i < self.zones.len()) else {
            return false;
        };
        self.undo = Some(self.snapshot());
        self.zones.remove(i);
        self.selected = None;
        // a Delete key press while the mouse still holds a zone ends that drag (its index is gone)
        self.drag = Drag::None;
        true
    }

    /// Picks the kind: new zones get it, and the selected zone changes to it.
    pub fn set_kind(&mut self, kind: Kind) -> bool {
        self.kind = kind;
        match self.selected {
            Some(i) if i < self.zones.len() && self.zones[i].kind != kind => {
                self.undo = Some(self.snapshot());
                self.zones[i].kind = kind;
                true
            }
            _ => false,
        }
    }

    pub fn rename(&mut self, i: usize, name: &str) -> bool {
        let name = name.trim();
        if i >= self.zones.len() || name.is_empty() || self.zones[i].name == name {
            return false;
        }
        self.undo = Some(self.snapshot());
        self.zones[i].name = name.to_string();
        true
    }

    pub fn set_on(&mut self, i: usize, on: bool) -> bool {
        if i >= self.zones.len() || self.zones[i].on == on {
            return false;
        }
        self.undo = Some(self.snapshot());
        self.zones[i].on = on;
        true
    }

    /// Undoes the last change (one step).
    pub fn undo(&mut self) -> bool {
        let Some(u) = self.undo.take() else {
            return false;
        };
        self.zones = u.zones;
        self.selected = u.selected.filter(|&i| i < self.zones.len());
        self.drag = Drag::None;
        true
    }

    /// A new screenshot of another size: zones keep their place on the picture.
    pub fn rescale(&mut self, img_w: f32, img_h: f32) {
        if self.img_w > 0.0 && self.img_h > 0.0 && (img_w, img_h) != (self.img_w, self.img_h) {
            let (sx, sy) = (img_w / self.img_w, img_h / self.img_h);
            for z in &mut self.zones {
                z.rect = Rect::new(z.rect.x * sx, z.rect.y * sy, z.rect.w * sx, z.rect.h * sy);
            }
        }
        self.img_w = img_w;
        self.img_h = img_h;
    }
}

/// Moves the edges of `o` that `handle` holds to (x, y), keeping a minimum size and the image bounds.
/// Handles: 0 top left, 1 top, 2 top right, 3 right, 4 bottom right, 5 bottom, 6 bottom left, 7 left.
pub fn resize(o: Rect, handle: usize, x: f32, y: f32, b: Rect, min_w: f32, min_h: f32) -> Rect {
    let (mut l, mut t, mut r, mut bt) = (o.x, o.y, o.x + o.w, o.y + o.h);
    let x = x.clamp(b.x, b.x + b.w);
    let y = y.clamp(b.y, b.y + b.h);
    if matches!(handle, 0 | 6 | 7) {
        l = x.min(r - min_w);
    }
    if matches!(handle, 2..=4) {
        r = x.max(l + min_w);
    }
    if matches!(handle, 0..=2) {
        t = y.min(bt - min_h);
    }
    if matches!(handle, 4..=6) {
        bt = y.max(t + min_h);
    }
    let (l, r) = (l.max(b.x), r.min(b.x + b.w));
    let (t, bt) = (t.max(b.y), bt.min(b.y + b.h));
    Rect::new(l, t, (r - l).max(0.0), (bt - t).max(0.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ed() -> Editor {
        let z = |name: &str, kind, x, y, w, h| EdZone {
            name: name.into(),
            kind,
            on: true,
            rect: Rect::new(x, y, w, h),
        };
        Editor::new(
            vec![
                z("Subtitles", Kind::Dialogue, 100.0, 800.0, 800.0, 60.0),
                z("Speaker name", Kind::Speaker, 400.0, 740.0, 200.0, 40.0),
            ],
            1920.0,
            1080.0,
        )
    }

    #[test]
    fn drag_draws_a_zone_and_selects_it() {
        let mut e = ed();
        e.pointer_down(1000.0, 100.0, 0.5);
        e.pointer_move(1200.0, 160.0, 0.5);
        assert_eq!(e.draft(), Some(Rect::new(1000.0, 100.0, 200.0, 60.0)));
        let c = e.pointer_up(1200.0, 160.0, 0.5);
        assert!(c.zones);
        assert_eq!(e.zones.len(), 3);
        assert_eq!(e.zones[2].rect, Rect::new(1000.0, 100.0, 200.0, 60.0));
        assert_eq!(e.zones[2].name, "Dialogue", "named after its kind");
        assert_eq!(e.selected, Some(2));
        assert_eq!(e.draft(), None);
    }

    #[test]
    fn names_count_up() {
        let mut e = ed();
        for i in 0..2 {
            let y = 100.0 + 100.0 * i as f32;
            e.pointer_down(1000.0, y, 1.0);
            e.pointer_up(1100.0, y + 40.0, 1.0);
        }
        assert_eq!(e.zones[2].name, "Dialogue");
        assert_eq!(e.zones[3].name, "Dialogue 2");
    }

    #[test]
    fn tiny_drag_or_click_adds_nothing() {
        let mut e = ed();
        e.pointer_down(1000.0, 100.0, 1.0);
        assert!(!e.pointer_up(1008.0, 104.0, 1.0).zones);
        assert_eq!(e.zones.len(), 2);
        // a click on a zone selects it, also with the draw tool
        e.pointer_down(500.0, 830.0, 1.0);
        e.pointer_up(500.0, 830.0, 1.0);
        assert_eq!(e.selected, Some(0));
        e.pointer_down(1500.0, 300.0, 1.0);
        e.pointer_up(1500.0, 300.0, 1.0);
        assert_eq!(e.selected, None);
    }

    #[test]
    fn select_tool_moves_a_zone_inside_the_image() {
        let mut e = ed();
        e.draw = false;
        e.pointer_down(500.0, 760.0, 1.0); // speaker name (smaller one wins)
        assert_eq!(e.selected, Some(1));
        e.pointer_move(520.0, 700.0, 1.0);
        assert_eq!(e.zones[1].rect, Rect::new(420.0, 680.0, 200.0, 40.0));
        // past the bottom edge: clamped
        let c = e.pointer_up(520.0, 2000.0, 1.0);
        assert!(c.zones);
        assert_eq!(e.zones[1].rect, Rect::new(420.0, 1040.0, 200.0, 40.0));
        assert!(e.undo());
        assert_eq!(e.zones[1].rect, Rect::new(400.0, 740.0, 200.0, 40.0));
    }

    #[test]
    fn corner_drag_resizes_the_selected_zone() {
        let mut e = ed();
        e.select(Some(0));
        // bottom right handle of Subtitles is at (900, 860); within 6 view px at scale 0.5 = 12 image px
        e.pointer_down(908.0, 866.0, 0.5);
        e.pointer_move(1000.0, 900.0, 0.5);
        let c = e.pointer_up(1000.0, 900.0, 0.5);
        assert!(c.zones);
        assert_eq!(e.zones[0].rect, Rect::new(100.0, 800.0, 900.0, 100.0));
        assert_eq!(e.zones.len(), 2, "a handle drag never draws");
    }

    #[test]
    fn resize_keeps_a_minimum_and_the_bounds() {
        let b = Rect::new(0.0, 0.0, 100.0, 100.0);
        let o = Rect::new(10.0, 10.0, 50.0, 50.0);
        // top left past the bottom right: stops at the minimum
        assert_eq!(
            resize(o, 0, 90.0, 90.0, b, 16.0, 12.0),
            Rect::new(44.0, 48.0, 16.0, 12.0)
        );
        // right edge past the image
        assert_eq!(
            resize(o, 3, 500.0, 0.0, b, 16.0, 12.0),
            Rect::new(10.0, 10.0, 90.0, 50.0)
        );
        // the top handle moves only the top
        assert_eq!(
            resize(o, 1, 999.0, 0.0, b, 16.0, 12.0),
            Rect::new(10.0, 0.0, 50.0, 60.0)
        );
    }

    #[test]
    fn delete_kind_rename_toggle_and_undo() {
        let mut e = ed();
        e.select(Some(1));
        assert_eq!(e.kind, Kind::Speaker);
        assert!(e.set_kind(Kind::Notification));
        assert_eq!(e.zones[1].kind, Kind::Notification);
        assert!(e.undo());
        assert_eq!(e.zones[1].kind, Kind::Speaker);
        assert!(!e.undo(), "one step only");

        assert!(e.rename(0, "  Lines  "));
        assert_eq!(e.zones[0].name, "Lines");
        assert!(!e.rename(0, "   "));
        assert!(e.set_on(0, false));
        assert!(!e.zones[0].on);

        e.select(Some(0));
        assert!(e.delete_selected());
        assert_eq!(e.zones.len(), 1);
        assert_eq!(e.selected, None);
        assert!(e.undo());
        assert_eq!(e.zones.len(), 2);
        assert_eq!(e.zones[0].name, "Lines");
        assert_eq!(e.selected, Some(0));
    }

    #[test]
    fn delete_during_a_drag_ends_the_drag() {
        // the last zone held by the mouse (move), then Delete: the next move and release must not panic
        let mut e = ed();
        e.draw = false;
        e.pointer_down(500.0, 760.0, 1.0);
        assert_eq!(e.selected, Some(1));
        assert!(e.delete_selected());
        assert!(!e.pointer_move(600.0, 700.0, 1.0).zones);
        assert!(!e.pointer_up(600.0, 700.0, 1.0).zones);
        assert_eq!(e.zones.len(), 1);
        assert_eq!(e.zones[0].rect, Rect::new(100.0, 800.0, 800.0, 60.0));
        // the same with a resize of the first zone: the other zone is not moved instead
        let mut e = ed();
        e.select(Some(0));
        e.pointer_down(900.0, 860.0, 1.0);
        assert!(e.delete_selected());
        e.pointer_move(1000.0, 900.0, 1.0);
        assert!(!e.pointer_up(1000.0, 900.0, 1.0).zones);
        assert_eq!(e.zones[0].rect, Rect::new(400.0, 740.0, 200.0, 40.0));
        assert!(e.undo());
        assert_eq!(e.zones.len(), 2);
    }

    #[test]
    fn rescale_follows_a_new_screenshot() {
        let mut e = ed();
        e.rescale(3840.0, 2160.0);
        assert_eq!(e.zones[0].rect, Rect::new(200.0, 1600.0, 1600.0, 120.0));
    }
}
