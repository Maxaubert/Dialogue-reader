//! In-game zone edit (spec 2.7, mockup 16-zone-edit): the current game's zones over a frozen screenshot. It edits
//! the same zone editor as the Zones tab (`vm::zones::Editor`, capture px of the screenshot), so a change is saved
//! to the profile and sent to the reader at once, as a change on page 11 is.
//!
//! The window draws the box being drawn and its kind picker itself; these functions turn its logical px into
//! capture px ([`CaptureMap`]) and back: the zones it shows, a click (which zone), a new zone. [`bar_clear`] says
//! where the bar (Done) can go without covering a zone.

use dr_runtime::Kind;

use super::place::CaptureMap;
use crate::EditZone;
use crate::geom::{Rect, hit_zone};
use crate::vm::zones::Editor;

/// The zones as the window shows them (logical px).
pub fn view_zones(e: &Editor, m: &CaptureMap) -> Vec<EditZone> {
    e.zones
        .iter()
        .map(|z| {
            let r = view_rect(z.rect, m);
            EditZone {
                kind: crate::vm::ui_kind(z.kind),
                x: r.x,
                y: r.y,
                w: r.w,
                h: r.h,
            }
        })
        .collect()
}

/// A rect in capture px, in the window (logical px).
pub fn view_rect(r: Rect, m: &CaptureMap) -> Rect {
    let (x0, y0) = m.to_view(r.x, r.y);
    let (x1, y1) = m.to_view(r.x + r.w, r.y + r.h);
    Rect::new(x0, y0, x1 - x0, y1 - y0)
}

/// A rect of the window (logical px) in capture px.
pub fn capture_rect(r: Rect, m: &CaptureMap) -> Rect {
    let (x0, y0) = m.to_capture(r.x, r.y);
    let (x1, y1) = m.to_capture(r.x + r.w, r.y + r.h);
    Rect::new(x0, y0, x1 - x0, y1 - y0)
}

/// The bar of zone edit (logical px, as in ui/overlays/zone_edit.slint): about as wide as "Drag to add · Esc to
/// finish" and Done, 48 high, 24 from the window's edge. Its places in order: top centre (home), bottom centre,
/// top right, top left.
pub const BAR_W: f32 = 280.0;
pub const BAR_H: f32 = 48.0;
pub const BAR_EDGE: f32 = 24.0;
/// The room kept around the bar.
pub const BAR_GAP: f32 = 8.0;
/// A zone's tag: 24 high, 4 from the box; about this wide.
pub const TAG_H: f32 = 24.0;
pub const TAG_GAP: f32 = 4.0;
pub const TAG_W: f32 = 130.0;

/// Where the bar sits at each of its places in a window of `w` x `h`, with the room around it.
pub fn bar_places(w: f32, h: f32) -> [Rect; 4] {
    let g = BAR_GAP;
    let at = |x: f32, y: f32| Rect::new(x - g, y - g, BAR_W + 2.0 * g, BAR_H + 2.0 * g);
    let (top, bottom) = (BAR_EDGE, h - BAR_EDGE - BAR_H);
    [
        at((w - BAR_W) / 2.0, top),
        at((w - BAR_W) / 2.0, bottom),
        at(w - BAR_EDGE - BAR_W, top),
        at(BAR_EDGE, top),
    ]
}

/// A zone's tag: above the box; under it when there is no room above; inside it at the top left when there is no
/// room under it either (the same test as the window).
pub fn tag_rect(z: Rect, h: f32) -> Rect {
    let room = TAG_H + TAG_GAP;
    if z.y >= room {
        Rect::new(z.x - 1.0, z.y - room, TAG_W, TAG_H)
    } else if z.y + z.h + room <= h {
        Rect::new(z.x - 1.0, z.y + z.h + TAG_GAP, TAG_W, TAG_H)
    } else {
        Rect::new(z.x + 4.0, z.y + 4.0, TAG_W, TAG_H)
    }
}

fn overlap(a: Rect, b: Rect) -> bool {
    a.x < b.x + b.w && a.x + a.w > b.x && a.y < b.y + b.h && a.y + a.h > b.y
}

/// For each place of the bar ([`bar_places`]): no zone and no tag sits under it. `zones` in window px.
pub fn bar_clear(zones: &[Rect], w: f32, h: f32) -> [bool; 4] {
    bar_places(w, h).map(|p| {
        !zones
            .iter()
            .any(|&z| overlap(p, z) || overlap(p, tag_rect(z, h)))
    })
}

/// [`bar_clear`] for the editor's zones in the zone edit window.
pub fn bar_clear_of(e: &Editor, m: &CaptureMap) -> [bool; 4] {
    let (w, h) = m.view_size();
    let rects: Vec<Rect> = e.zones.iter().map(|z| view_rect(z.rect, m)).collect();
    bar_clear(&rects, w, h)
}

/// The zone under a click of the window (logical px).
pub fn pick(e: &Editor, m: &CaptureMap, x: f32, y: f32) -> Option<usize> {
    let (cx, cy) = m.to_capture(x, y);
    let rects: Vec<Rect> = e.zones.iter().map(|z| z.rect).collect();
    hit_zone(&rects, cx, cy)
}

/// Adds the box the window drew (logical px) as a zone of `kind`. Returns its index.
pub fn add(e: &mut Editor, m: &CaptureMap, r: Rect, kind: Kind) -> Option<usize> {
    e.add(capture_rect(r, m), kind)
}

/// Starts a resize of the selected zone by one of its handles (0 top left, clockwise to 7 left).
pub fn resize_start(e: &mut Editor, handle: i32) -> bool {
    usize::try_from(handle).is_ok_and(|h| e.begin_resize(h))
}

/// The pointer during a resize, at (x, y) in the window (logical px). The zone keeps its smallest size and stays
/// on the picture. True on the release when the zone changed (save it).
pub fn resize_to(e: &mut Editor, m: &CaptureMap, x: f32, y: f32, release: bool) -> bool {
    let (cx, cy) = m.to_capture(x, y);
    let scale = m.view_per_capture();
    if release {
        e.pointer_up(cx, cy, scale).zones
    } else {
        e.pointer_move(cx, cy, scale);
        false
    }
}

/// Removes zone `i`.
pub fn remove(e: &mut Editor, i: usize) -> bool {
    if i >= e.zones.len() {
        return false;
    }
    e.select(Some(i));
    e.delete_selected()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bar_avoids_zones_and_their_tags() {
        let (w, h) = (1440.0, 900.0);
        // nothing on the picture: every place is clear
        assert_eq!(bar_clear(&[], w, h), [true; 4]);
        // a box at the top centre covers home; a box at the bottom centre the bottom place
        let top = Rect::new(640.0, 4.0, 340.0, 46.0);
        assert_eq!(bar_clear(&[top], w, h), [false, true, true, true]);
        let bottom = Rect::new(280.0, 800.0, 880.0, 50.0);
        assert_eq!(bar_clear(&[bottom], w, h), [true, false, true, true]);
        // only the tag of this box reaches the top right place (it sits above the box)
        let under = Rect::new(1200.0, 100.0, 200.0, 60.0);
        assert_eq!(bar_clear(&[under], w, h), [true, true, false, true]);
        assert_eq!(
            bar_clear(&[Rect::new(1200.0, 120.0, 200.0, 60.0)], w, h),
            [true; 4]
        );
    }

    #[test]
    fn a_tag_flips_under_the_box_then_inside_it() {
        let h = 900.0;
        assert_eq!(tag_rect(Rect::new(100.0, 300.0, 50.0, 50.0), h).y, 272.0);
        assert_eq!(tag_rect(Rect::new(100.0, 10.0, 50.0, 50.0), h).y, 64.0);
        assert_eq!(tag_rect(Rect::new(100.0, 10.0, 50.0, 880.0), h).y, 14.0);
    }
}
