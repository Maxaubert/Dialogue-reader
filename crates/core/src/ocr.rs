//! The OCR engine interface. The product and the arena plug their engines in here.

use crate::geom::Rect;

/// One recognised line. `rect` is in the pixels of the image given to the engine.
#[derive(Clone, Debug, PartialEq)]
pub struct OcrLine {
    pub text: String,
    pub rect: Rect,
}

pub trait Ocr {
    /// Largest width or height the engine accepts; bigger crops are scaled down first.
    fn max_dim(&self) -> u32 {
        u32::MAX
    }
    /// Recognises text in an RGBA8 image (no row padding). Errors are passed up unchanged.
    fn recognize(&mut self, w: u32, h: u32, rgba: &[u8]) -> Result<Vec<OcrLine>, String>;
}
