//! Folders on disk (spec 2.8). The root is a value, so tests point it at a temp folder and never touch the
//! real `%APPDATA%`.

use std::path::{Path, PathBuf};

/// The data folder: `%APPDATA%\DialogReader` in the app, any folder in tests.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Paths {
    root: PathBuf,
}

impl Paths {
    /// `%APPDATA%\DialogReader`. `None` when `APPDATA` is not set.
    pub fn from_env() -> Option<Paths> {
        let appdata = std::env::var_os("APPDATA")?;
        Some(Paths::at(Path::new(&appdata).join("DialogReader")))
    }

    /// A data folder at `root` (tests use a temp folder).
    pub fn at(root: impl Into<PathBuf>) -> Paths {
        Paths { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// `profiles\`: one `<id>.json` and one `<id>.png` per game.
    pub fn profiles(&self) -> PathBuf {
        self.root.join("profiles")
    }

    pub fn profile_json(&self, id: &str) -> PathBuf {
        self.profiles().join(format!("{id}.json"))
    }

    /// The last screenshot of that game (zone editor and cover).
    pub fn profile_png(&self, id: &str) -> PathBuf {
        self.profiles().join(format!("{id}.png"))
    }

    /// `settings.json` (M5).
    pub fn settings(&self) -> PathBuf {
        self.root.join("settings.json")
    }

    /// `log\<id>.jsonl`: the lines read for that game (spec 2.8).
    pub fn activity_log(&self, id: &str) -> PathBuf {
        self.root.join("log").join(format!("{id}.jsonl"))
    }
}

/// Crash logs: `%LOCALAPPDATA%\DialogReader\logs`. `None` when `LOCALAPPDATA` is not set.
pub fn crash_logs() -> Option<PathBuf> {
    let local = std::env::var_os("LOCALAPPDATA")?;
    Some(Path::new(&local).join("DialogReader").join("logs"))
}

/// Writes `bytes` to `path` atomically: a `.tmp` file next to it, then a rename over the old file.
/// A crash in the middle leaves the old file or the new one, never half a file.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
    }
    // std::fs::rename replaces an existing file on Windows (MoveFileExW with MOVEFILE_REPLACE_EXISTING).
    std::fs::rename(&tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_under_the_root() {
        let p = Paths::at("C:/x/DialogReader");
        assert_eq!(
            p.profile_json("rdr2"),
            Path::new("C:/x/DialogReader/profiles/rdr2.json")
        );
        assert_eq!(
            p.profile_png("rdr2"),
            Path::new("C:/x/DialogReader/profiles/rdr2.png")
        );
        assert_eq!(p.settings(), Path::new("C:/x/DialogReader/settings.json"));
        assert_eq!(
            p.activity_log("rdr2"),
            Path::new("C:/x/DialogReader/log/rdr2.jsonl")
        );
    }

    #[test]
    fn atomic_write_replaces_and_leaves_no_tmp() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("a").join("b.json");
        write_atomic(&f, b"one").unwrap();
        write_atomic(&f, b"two").unwrap();
        assert_eq!(std::fs::read(&f).unwrap(), b"two");
        assert!(!dir.path().join("a").join("b.json.tmp").exists());
    }
}
