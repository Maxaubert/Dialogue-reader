//! Zones the user draws per game.

use crate::geom::Rect;

/// What a zone holds. It sets how text there is split, timed and deduplicated.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ZoneKind {
    /// Dialogue box or subtitles: the whole zone is one line of speech (wrapped lines joined).
    Dialogue,
    /// Speaker name: one short line.
    Speaker,
    /// Toasts, pickups, objective updates: each text row is its own event.
    Notification,
    /// Interaction prompts ("Press E to ..."): flicker, so they need a longer debounce.
    Prompt,
    /// Static HUD or anything the user does not want read.
    Ignore,
}

impl ZoneKind {
    /// Rows of one zone read become one event (true) or one event per row (false).
    pub fn joins_lines(self) -> bool {
        !matches!(self, ZoneKind::Notification)
    }
    /// One line of text by nature (a prompt, a speaker name).
    pub fn one_line(self) -> bool {
        matches!(self, ZoneKind::Prompt | ZoneKind::Speaker)
    }
    pub fn name(self) -> &'static str {
        match self {
            ZoneKind::Dialogue => "dialogue",
            ZoneKind::Speaker => "speaker",
            ZoneKind::Notification => "notification",
            ZoneKind::Prompt => "prompt",
            ZoneKind::Ignore => "ignore",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Zone {
    pub name: String,
    pub kind: ZoneKind,
    /// Frame pixels of a frame `base` in size (or of any frame when `base` is None).
    pub rect: Rect,
    /// Width and height of the frame the zone was drawn on. When the game runs at another size
    /// the rect is scaled to it. None: `rect` is in the pixels of every frame.
    pub base: Option<(u32, u32)>,
}

impl Zone {
    /// A zone in frame pixels (no base size).
    pub fn new(name: &str, kind: ZoneKind, rect: Rect) -> Zone {
        Zone { name: name.to_string(), kind, rect, base: None }
    }

    /// The rect in the pixels of a `fw` x `fh` frame.
    pub fn rect_in(&self, fw: u32, fh: u32) -> Rect {
        match self.base {
            Some((bw, bh)) if bw > 0 && bh > 0 && (bw, bh) != (fw, fh) => {
                let (sx, sy) = (fw as f32 / bw as f32, fh as f32 / bh as f32);
                Rect::new(self.rect.x * sx, self.rect.y * sy, self.rect.w * sx, self.rect.h * sy)
            }
            _ => self.rect,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zones_scale_with_the_frame() {
        let z = Zone { base: Some((1280, 720)), ..Zone::new("d", ZoneKind::Dialogue, Rect::new(100.0, 600.0, 400.0, 60.0)) };
        assert_eq!(z.rect_in(1280, 720), z.rect);
        assert_eq!(z.rect_in(2560, 1440), Rect::new(200.0, 1200.0, 800.0, 120.0));
        assert_eq!(Zone::new("d", ZoneKind::Dialogue, z.rect).rect_in(2560, 1440), z.rect);
    }
}
