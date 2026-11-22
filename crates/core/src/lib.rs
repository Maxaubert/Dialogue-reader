//! dr-core: the dialogue reader engine (reader "A1").
//!
//! It takes captured frames, watches user zones, runs OCR only when a zone changed, and decides
//! what to speak. It knows nothing about the test arena; the arena wraps it in
//! `crates/arena/src/readers/a1.rs`. Design: `docs/reader-a1.md`.
//!
//! Main entry: [`pipeline::Pipeline`]. Feed it every frame with [`pipeline::Pipeline::on_frame`].
//! Manual mode (read every zone on a key press, nothing else): [`manual::Manual`].

pub mod detect;
pub mod frame;
pub mod geom;
pub mod image;
pub mod manual;
pub mod ocr;
pub mod pipeline;
pub mod ring;
pub mod signature;
pub mod text;
pub mod track;
pub mod zone;

pub use frame::Frame;
pub use geom::Rect;
pub use manual::{Manual, ManualConfig, Press};
pub use ocr::{Ocr, OcrLine};
pub use pipeline::{Config, Event, Pipeline};
pub use zone::{Zone, ZoneKind};
