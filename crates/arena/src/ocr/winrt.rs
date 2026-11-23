//! Windows.Media.Ocr (WinRT) engine: a thin adapter over `dr_ocr::WinOcr` (moved there in M3a).
//! It adds the engine name and maps the `dr-core` types to the arena types. The reading code is in `dr-ocr`.

use super::{Engine, OcrLine};
use crate::model::Rect;
use anyhow::{Result, anyhow};
use dr_core::Ocr;

pub struct WinOcr {
    inner: dr_ocr::WinOcr,
}

impl WinOcr {
    /// Engine for the language tag `lang` (for example "en-US"). Falls back to the user profile
    /// languages with a warning when that language pack is not installed.
    pub fn new(lang: &str) -> Result<Self> {
        Ok(WinOcr { inner: dr_ocr::WinOcr::new(lang).map_err(|e| anyhow!(e))? })
    }

    /// Language tag the engine actually uses.
    pub fn language(&self) -> &str {
        self.inner.language()
    }
}

impl Engine for WinOcr {
    fn name(&self) -> String {
        format!("winocr({})", self.language())
    }

    fn max_dim(&self) -> u32 {
        Ocr::max_dim(&self.inner)
    }

    fn recognize(&mut self, w: u32, h: u32, rgba: &[u8]) -> Result<Vec<OcrLine>> {
        let lines = Ocr::recognize(&mut self.inner, w, h, rgba).map_err(|e| anyhow!(e))?;
        Ok(lines
            .into_iter()
            .map(|l| OcrLine { text: l.text, rect: Rect::new(l.rect.x, l.rect.y, l.rect.w, l.rect.h) })
            .collect())
    }
}
