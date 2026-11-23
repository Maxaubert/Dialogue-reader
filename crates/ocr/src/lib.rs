//! dr-ocr: the OCR engines of DialogReader (docs/app-spec.md 2.4).
//!
//! v1 has one engine: [`WinOcr`], the OCR built into Windows (`Windows.Media.Ocr`). It implements
//! [`dr_core::Ocr`], so the reader (`dr_core::Pipeline`, `dr_core::Manual`) and the arena use the same code.
//! The arena wraps it in a thin adapter (`crates/arena/src/ocr/winrt.rs`) that adds a name.

#[cfg(windows)]
mod winrt;

#[cfg(windows)]
pub use winrt::WinOcr;
