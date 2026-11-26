//! The reader interface every reader (built-in or real) implements.

use crate::model::{Layout, Rect, TextItem};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// One frame. `rgba` is RGBA8, row-major, no padding (`width * height * 4` bytes).
pub struct FrameRef<'a> {
    pub t_ms: f64,
    pub width: u32,
    pub height: u32,
    pub rgba: &'a [u8],
}

/// "Speak this text now." The runner overwrites `t_ms` of events returned from `on_frame`
/// with the frame time (ideal clock) or frame time + processing time (simulated clock).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ReadEvent {
    pub t_ms: f64,
    pub text: String,
    #[serde(default)]
    pub rect: Option<Rect>,
    #[serde(default)]
    pub zone: Option<String>,
}

/// A reader under test.
///
/// Readers must do their work synchronously inside `on_frame` and return the events that work
/// produced. The simulated clock charges only the wall time of `on_frame`; a reader that hands
/// work to another thread and returns its result on a later call would be timed by how fast the
/// runner renders, not by the virtual clock. (The `winocr-naive` reader blocks on WinRT OCR inside
/// `on_frame`, so it follows this rule.) The simulated clock also delivers the next frame after
/// a busy period, not the newest one already waiting, so it can be up to one frame (33 ms) later
/// than a real one-slot mailbox.
pub trait Reader {
    fn name(&self) -> &str;
    fn setup(&mut self, _layout: &Layout) {}
    fn on_frame(&mut self, f: &FrameRef) -> Vec<ReadEvent>;
    /// Called once after the last frame; events keep the `t_ms` the reader gives them.
    fn finish(&mut self, _t_ms: f64) -> Vec<ReadEvent> {
        vec![]
    }
}

/// What a reader constructor gets. A fresh reader is built per scenario.
#[derive(Clone, Debug, Default)]
pub struct ReaderContext {
    pub scenario_id: String,
    pub width: u32,
    pub height: u32,
    pub layout: Layout,
    /// The answer key. Only the `oracle` reader may look at it.
    pub truth: Vec<TextItem>,
    /// Free-form options from `--reader-opt key=value`.
    pub options: BTreeMap<String, String>,
}
