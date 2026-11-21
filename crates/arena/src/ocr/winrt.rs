//! Windows.Media.Ocr (WinRT) engine.

use super::{Engine, OcrLine};
use crate::model::Rect;
use anyhow::{Context, Result};
use windows::Globalization::Language;
use windows::Graphics::Imaging::{BitmapPixelFormat, SoftwareBitmap};
use windows::Media::Ocr::OcrEngine;
use windows::Storage::Streams::DataWriter;
use windows::core::HSTRING;

pub struct WinOcr {
    engine: OcrEngine,
    lang: String,
    max_dim: u32,
}

impl WinOcr {
    /// Engine for the language tag `lang` (for example "en-US"). Falls back to the user profile
    /// languages with a warning when that language pack is not installed.
    pub fn new(lang: &str) -> Result<Self> {
        let want = Language::CreateLanguage(&HSTRING::from(lang)).with_context(|| format!("bad language tag {lang:?}"))?;
        let engine = if OcrEngine::IsLanguageSupported(&want)? {
            OcrEngine::TryCreateFromLanguage(&want).with_context(|| format!("WinRT OCR: no engine for {lang}"))?
        } else {
            eprintln!("warning: WinRT OCR has no {lang} language pack; using the user profile languages");
            OcrEngine::TryCreateFromUserProfileLanguages()
                .context("WinRT OCR: no engine for the user profile languages (install an OCR language pack)")?
        };
        let used = engine.RecognizerLanguage()?.LanguageTag()?.to_string();
        let max_dim = OcrEngine::MaxImageDimension().unwrap_or(2600);
        Ok(WinOcr { engine, lang: used, max_dim })
    }

    /// Language tag the engine actually uses.
    pub fn language(&self) -> &str {
        &self.lang
    }
}

impl Engine for WinOcr {
    fn name(&self) -> String {
        format!("winocr({})", self.lang)
    }

    fn max_dim(&self) -> u32 {
        self.max_dim
    }

    fn recognize(&mut self, w: u32, h: u32, rgba: &[u8]) -> Result<Vec<OcrLine>> {
        let mut bgra = rgba.to_vec();
        for px in bgra.as_chunks_mut::<4>().0 {
            px.swap(0, 2);
        }
        let writer = DataWriter::new()?;
        writer.WriteBytes(&bgra)?;
        let buffer = writer.DetachBuffer()?;
        let bitmap = SoftwareBitmap::CreateCopyFromBuffer(&buffer, BitmapPixelFormat::Bgra8, w as i32, h as i32)
            .context("WinRT OCR: SoftwareBitmap from buffer")?;
        let result = self.engine.RecognizeAsync(&bitmap)?.join().context("WinRT OCR: RecognizeAsync")?;
        let lines = result.Lines()?;
        let mut out = Vec::new();
        for i in 0..lines.Size()? {
            let line = lines.GetAt(i)?;
            let text = line.Text()?.to_string();
            let words = line.Words()?;
            let mut rect: Option<Rect> = None;
            for j in 0..words.Size()? {
                let b = words.GetAt(j)?.BoundingRect()?;
                let r = Rect::new(b.X, b.Y, b.Width, b.Height);
                rect = Some(match rect {
                    None => r,
                    Some(u) => union(&u, &r),
                });
            }
            out.push(OcrLine { text, rect: rect.unwrap_or_default() });
        }
        Ok(out)
    }
}

fn union(a: &Rect, b: &Rect) -> Rect {
    let x = a.x.min(b.x);
    let y = a.y.min(b.y);
    Rect::new(x, y, a.right().max(b.right()) - x, a.bottom().max(b.bottom()) - y)
}
