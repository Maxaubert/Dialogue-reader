//! Test arena: plays fake game scenes with a known answer key to a reader and scores the result.
//! See docs/arena-spec.md.

pub mod background;
pub mod cli;
pub mod corpus;
pub mod fonts;
pub mod generator;
pub mod live;
pub mod model;
pub mod ocr;
pub mod reader;
pub mod readers;
pub mod realworld;
pub mod render;
pub mod rng;
pub mod runner;
pub mod score;
pub mod text;

use std::path::{Path, PathBuf};

/// Repo root (two levels above this crate).
pub fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

/// Local data folder: `$ARENA_DATA`, else `./arena-data` when present, else `<repo>/arena-data`.
pub fn data_dir() -> PathBuf {
    if let Ok(d) = std::env::var("ARENA_DATA") {
        return PathBuf::from(d);
    }
    let local = PathBuf::from("arena-data");
    if local.is_dir() { local } else { repo_root().join("arena-data") }
}

/// Resolves a path from a scenario file: absolute as is; relative against the current folder,
/// then against the repo root.
pub fn resolve_path(p: &str) -> PathBuf {
    let path = PathBuf::from(p);
    if path.is_absolute() || path.exists() {
        return path;
    }
    let from_root = repo_root().join(&path);
    if from_root.exists() { from_root } else { path }
}

/// A path as stored in scenario files: relative to the repo root when inside it, `/` separated.
pub fn portable_path(p: &Path) -> String {
    let abs = |x: &Path| std::fs::canonicalize(x).ok();
    if let (Some(a), Some(root)) = (abs(p), abs(&repo_root()))
        && let Ok(rel) = a.strip_prefix(&root)
    {
        return rel.to_string_lossy().replace('\\', "/");
    }
    p.to_string_lossy().replace('\\', "/")
}

// Shared Windows bindings declared for live mode and the winocr reader. Leave these lines in
// place (they are harmless once the crates are used), so parallel agents never edit this file.
#[cfg(windows)]
use windows as _;
#[cfg(windows)]
use windows_capture as _;
