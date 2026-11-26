//! Emits the answer key: each should_read item and each prompt once, on the first frame at or
//! after its start. A sanity check of the runner and scorer: it must score 100%.

use crate::model::{ItemKind, Layout, TextItem};
use crate::reader::{FrameRef, ReadEvent, Reader, ReaderContext};
use anyhow::Result;

/// Frames this late after an item ended still get its event (covers skipped frames).
const LATE_MS: f64 = 1000.0;

pub struct Oracle {
    items: Vec<TextItem>,
    done: Vec<bool>,
    layout: Layout,
}

pub fn create(ctx: &ReaderContext) -> Result<Box<dyn Reader>> {
    let items: Vec<TextItem> =
        ctx.truth.iter().filter(|i| i.should_read || i.kind == ItemKind::Prompt).cloned().collect();
    let done = vec![false; items.len()];
    Ok(Box::new(Oracle { items, done, layout: ctx.layout.clone() }))
}

impl Reader for Oracle {
    fn name(&self) -> &str {
        "oracle"
    }

    fn setup(&mut self, layout: &Layout) {
        self.layout = layout.clone();
    }

    fn on_frame(&mut self, f: &FrameRef) -> Vec<ReadEvent> {
        let mut out = Vec::new();
        for (i, it) in self.items.iter().enumerate() {
            if self.done[i] || f.t_ms < it.start_ms || f.t_ms > it.end_ms + LATE_MS {
                continue;
            }
            self.done[i] = true;
            let (cx, cy) = it.rect.center();
            let zone = self.layout.zones.iter().find(|z| z.rect.contains(cx, cy)).map(|z| z.name.clone());
            out.push(ReadEvent { t_ms: f.t_ms, text: it.text.clone(), rect: Some(it.rect), zone });
        }
        out
    }
}
