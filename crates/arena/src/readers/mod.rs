//! Reader registry. To add a reader: one line in `registry()` plus its module.

pub mod noop;
pub mod oracle;
pub mod winocr;

use crate::reader::{Reader, ReaderContext};
use anyhow::{Result, bail};

/// Builds a reader for one scenario.
pub type Ctor = fn(&ReaderContext) -> Result<Box<dyn Reader>>;

/// Every reader `arena run --reader <name>` knows.
pub fn registry() -> Vec<(&'static str, Ctor)> {
    vec![
        ("oracle", oracle::create),
        ("noop", noop::create),
        ("winocr-naive", winocr::create),
    ]
}

pub fn names() -> Vec<&'static str> {
    registry().into_iter().map(|(n, _)| n).collect()
}

pub fn create(name: &str, ctx: &ReaderContext) -> Result<Box<dyn Reader>> {
    match registry().into_iter().find(|(n, _)| *n == name) {
        Some((_, ctor)) => ctor(ctx),
        None => bail!("unknown reader {name:?}; known: {}", names().join(", ")),
    }
}
