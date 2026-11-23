//! In-game zone edit (spec 2.7, mockup 16-zone-edit): the current game's zones over a frozen screenshot. It edits
//! the same zone editor as the Zones tab (`vm::zones::Editor`, capture px of the screenshot), so a change is saved
//! to the profile and sent to the reader at once, as a change on page 11 is.
//!
//! The window draws the box being drawn and its kind picker itself; these functions turn its logical px into
//! capture px ([`CaptureMap`]) and back: the zones it shows, the dim layer, a click (which zone), a new zone.

use dr_runtime::Kind;

use super::place::CaptureMap;
use crate::EditZone;
use crate::geom::{Rect, dim_path, hit_zone};
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

/// The dim layer: the window minus every zone.
pub fn dim(e: &Editor, m: &CaptureMap) -> String {
    let (w, h) = m.view_size();
    let rects: Vec<Rect> = e.zones.iter().map(|z| view_rect(z.rect, m)).collect();
    dim_path(w, h, &rects)
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

/// Removes zone `i`.
pub fn remove(e: &mut Editor, i: usize) -> bool {
    if i >= e.zones.len() {
        return false;
    }
    e.select(Some(i));
    e.delete_selected()
}
