//! Windows.Media.Ocr (WinRT) engine. Moved from `crates/arena/src/ocr/winrt.rs` in M3a; the reading code is the
//! same, so the arena scores do not change.

use dr_core::{Ocr, OcrLine, Rect};
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

/// Adds a short note in front of a WinRT error.
fn ctx<T>(r: windows::core::Result<T>, what: &str) -> Result<T, String> {
    r.map_err(|e| format!("{what}: {e}"))
}

impl WinOcr {
    /// Engine for the language tag `lang` (for example "en-US"). Falls back to the user profile
    /// languages with a warning when that language pack is not installed.
    pub fn new(lang: &str) -> Result<Self, String> {
        let want = ctx(
            Language::CreateLanguage(&HSTRING::from(lang)),
            &format!("bad language tag {lang:?}"),
        )?;
        let engine = if ctx(OcrEngine::IsLanguageSupported(&want), "WinRT OCR")? {
            ctx(
                OcrEngine::TryCreateFromLanguage(&want),
                &format!("WinRT OCR: no engine for {lang}"),
            )?
        } else {
            eprintln!(
                "warning: WinRT OCR has no {lang} language pack; using the user profile languages"
            );
            ctx(
                OcrEngine::TryCreateFromUserProfileLanguages(),
                "WinRT OCR: no engine for the user profile languages (install an OCR language pack)",
            )?
        };
        let used = ctx(
            engine.RecognizerLanguage().and_then(|l| l.LanguageTag()),
            "WinRT OCR",
        )?
        .to_string();
        let max_dim = OcrEngine::MaxImageDimension().unwrap_or(2600);
        Ok(WinOcr {
            engine,
            lang: used,
            max_dim,
        })
    }

    /// Language tag the engine actually uses.
    pub fn language(&self) -> &str {
        &self.lang
    }

    /// The OCR language pack for `lang` is installed.
    pub fn has_language(lang: &str) -> bool {
        Language::CreateLanguage(&HSTRING::from(lang))
            .and_then(|l| OcrEngine::IsLanguageSupported(&l))
            .unwrap_or(false)
    }
}

impl Ocr for WinOcr {
    fn max_dim(&self) -> u32 {
        self.max_dim
    }

    fn recognize(&mut self, w: u32, h: u32, rgba: &[u8]) -> Result<Vec<OcrLine>, String> {
        let mut bgra = rgba.to_vec();
        for px in bgra.as_chunks_mut::<4>().0 {
            px.swap(0, 2);
        }
        let writer = ctx(DataWriter::new(), "WinRT OCR")?;
        ctx(writer.WriteBytes(&bgra), "WinRT OCR")?;
        let buffer = ctx(writer.DetachBuffer(), "WinRT OCR")?;
        let bitmap = ctx(
            SoftwareBitmap::CreateCopyFromBuffer(
                &buffer,
                BitmapPixelFormat::Bgra8,
                w as i32,
                h as i32,
            ),
            "WinRT OCR: SoftwareBitmap from buffer",
        )?;
        let result = ctx(
            self.engine.RecognizeAsync(&bitmap).and_then(|op| op.join()),
            "WinRT OCR: RecognizeAsync",
        )?;
        let lines = ctx(result.Lines(), "WinRT OCR")?;
        let mut out = Vec::new();
        for i in 0..ctx(lines.Size(), "WinRT OCR")? {
            let line = ctx(lines.GetAt(i), "WinRT OCR")?;
            let text = ctx(line.Text(), "WinRT OCR")?.to_string();
            let words = ctx(line.Words(), "WinRT OCR")?;
            let mut rect: Option<Rect> = None;
            for j in 0..ctx(words.Size(), "WinRT OCR")? {
                let b = ctx(words.GetAt(j).and_then(|w| w.BoundingRect()), "WinRT OCR")?;
                let r = Rect::new(b.X, b.Y, b.Width, b.Height);
                rect = Some(match rect {
                    None => r,
                    Some(u) => u.union(&r),
                });
            }
            out.push(OcrLine {
                text,
                rect: rect.unwrap_or_default(),
            });
        }
        Ok(out)
    }
}
