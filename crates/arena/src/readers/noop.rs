//! Reads nothing. Must score recall 0 and zero false reads.

use crate::reader::{FrameRef, ReadEvent, Reader, ReaderContext};
use anyhow::Result;

pub struct Noop;

pub fn create(_ctx: &ReaderContext) -> Result<Box<dyn Reader>> {
    Ok(Box::new(Noop))
}

impl Reader for Noop {
    fn name(&self) -> &str {
        "noop"
    }
    fn on_frame(&mut self, _f: &FrameRef) -> Vec<ReadEvent> {
        Vec::new()
    }
}
