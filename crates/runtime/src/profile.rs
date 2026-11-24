//! Game profiles on disk (spec 2.8): `profiles\<id>.json`, one per game.
//!
//! Zone rects are in capture pixels of the screenshot they were drawn on (`base`), exactly what
//! `dr_core::Zone` takes. Writes are atomic. A file that does not parse is renamed to `<id>.json.bad` and
//! skipped, so the app starts anyway and can say so.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::paths::{Paths, write_atomic};

/// The profile format this build writes.
pub const VERSION: u32 = 1;

/// What a zone holds. The names on disk are the `dr-core` names; the UI names differ for two of them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Dialogue,
    Speaker,
    /// "Pickups" in the UI.
    Notification,
    /// "Prompts" in the UI.
    Prompt,
    Ignore,
}

impl Kind {
    pub const ALL: [Kind; 5] = [
        Kind::Dialogue,
        Kind::Speaker,
        Kind::Notification,
        Kind::Prompt,
        Kind::Ignore,
    ];

    /// The name on disk.
    pub fn disk_name(self) -> &'static str {
        self.core().name()
    }

    /// The name in the UI.
    pub fn ui_name(self) -> &'static str {
        match self {
            Kind::Dialogue => "Dialogue",
            Kind::Speaker => "Speaker",
            Kind::Notification => "Pickups",
            Kind::Prompt => "Prompts",
            Kind::Ignore => "Ignore",
        }
    }

    pub fn core(self) -> dr_core::ZoneKind {
        match self {
            Kind::Dialogue => dr_core::ZoneKind::Dialogue,
            Kind::Speaker => dr_core::ZoneKind::Speaker,
            Kind::Notification => dr_core::ZoneKind::Notification,
            Kind::Prompt => dr_core::ZoneKind::Prompt,
            Kind::Ignore => dr_core::ZoneKind::Ignore,
        }
    }
}

/// Automatic (reads new text as it appears) or on key press.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    Auto,
    Manual,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Match {
    /// Process file name, for example `RDR2.exe`. The profile follows the game by it.
    pub exe: String,
    pub title_contains: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Keys {
    /// Read key, as text (`"Mouse 4"`, `"F7"`, `"Ctrl+Shift+R"`).
    pub read: String,
    pub quick_menu: String,
}

impl Default for Keys {
    fn default() -> Self {
        Keys {
            read: "Mouse 4".into(),
            quick_menu: "F7".into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VoiceCfg {
    /// The engine of `id`: `kokoro` or `windows` (the id prefix, `kokoro:` or `win:`, decides when they differ).
    pub engine: String,
    pub id: String,
    pub speed: f32,
    pub volume: f32,
}

impl Default for VoiceCfg {
    fn default() -> Self {
        VoiceCfg {
            engine: "windows".into(),
            id: String::new(),
            speed: 1.0,
            volume: 0.8,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ZoneCfg {
    pub name: String,
    pub kind: Kind,
    pub on: bool,
    /// x, y, w, h in pixels of a `base` sized capture.
    pub rect: [f32; 4],
    /// Width and height of the screenshot the zone was drawn on.
    pub base: [u32; 2],
}

impl ZoneCfg {
    pub fn core(&self) -> dr_core::Zone {
        let [x, y, w, h] = self.rect;
        dr_core::Zone {
            name: self.name.clone(),
            kind: self.kind.core(),
            rect: dr_core::Rect::new(x, y, w, h),
            base: Some((self.base[0], self.base[1])),
        }
    }

    /// The rect in pixels of a `w` x `h` capture (same scaling as `dr_core::Zone::rect_in`).
    pub fn rect_in(&self, w: u32, h: u32) -> [f32; 4] {
        let r = self.core().rect_in(w, h);
        [r.x, r.y, r.w, r.h]
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Profile {
    pub version: u32,
    pub id: String,
    pub name: String,
    #[serde(rename = "match")]
    pub matches: Match,
    pub mode: Mode,
    #[serde(default)]
    pub keys: Keys,
    #[serde(default)]
    pub voice: VoiceCfg,
    #[serde(default)]
    pub read_prompts: bool,
    #[serde(default = "default_lang")]
    pub ocr_lang: String,
    #[serde(default)]
    pub zones: Vec<ZoneCfg>,
}

fn default_lang() -> String {
    "en-US".into()
}

impl Profile {
    /// A new profile for a game, with the defaults of the spec.
    pub fn new(id: &str, name: &str, exe: &str) -> Profile {
        Profile {
            version: VERSION,
            id: id.into(),
            name: name.into(),
            matches: Match {
                exe: exe.into(),
                title_contains: None,
            },
            mode: Mode::Auto,
            keys: Keys::default(),
            voice: VoiceCfg::default(),
            read_prompts: false,
            ocr_lang: default_lang(),
            zones: Vec::new(),
        }
    }

    /// The zones `dr-core` reads: the ones switched on.
    pub fn core_zones(&self) -> Vec<dr_core::Zone> {
        self.zones
            .iter()
            .filter(|z| z.on)
            .map(ZoneCfg::core)
            .collect()
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("a profile always serializes")
    }

    /// Parses a profile file, migrating older versions. Does not touch the disk.
    pub fn from_json(text: &str) -> Result<Profile, LoadError> {
        let mut v: serde_json::Value =
            serde_json::from_str(text).map_err(|e| LoadError::Broken(e.to_string()))?;
        let version = match v.get("version") {
            None => 0,
            Some(n) => n
                .as_u64()
                .and_then(|n| u32::try_from(n).ok())
                .ok_or_else(|| LoadError::Broken("\"version\" is not a number".into()))?,
        };
        if version > VERSION {
            return Err(LoadError::Newer(version));
        }
        migrate(&mut v, version);
        serde_json::from_value(v).map_err(|e| LoadError::Broken(e.to_string()))
    }
}

/// Brings an older profile up to [`VERSION`].
///
/// Version 0 is a file without a `version` field (drafts made before M2). It may use the UI kind names
/// (`pickups`, `prompts`); version 1 uses the `dr-core` names.
fn migrate(v: &mut serde_json::Value, from: u32) {
    if from < 1 {
        if let Some(zones) = v.get_mut("zones").and_then(|z| z.as_array_mut()) {
            for z in zones {
                if let Some(k) = z.get_mut("kind") {
                    let new = match k.as_str() {
                        Some("pickups") => Some("notification"),
                        Some("prompts") => Some("prompt"),
                        _ => None,
                    };
                    if let Some(n) = new {
                        *k = n.into();
                    }
                }
            }
        }
        if let Some(o) = v.as_object_mut() {
            o.insert("version".into(), 1.into());
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum LoadError {
    /// The file is not a profile. It was renamed to `.bad` when loaded from disk.
    Broken(String),
    /// Made by a newer DialogReader. Left as it is.
    Newer(u32),
    Io(String),
}

impl std::fmt::Display for LoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LoadError::Broken(e) => write!(f, "the file is damaged ({e})"),
            LoadError::Newer(v) => write!(f, "it was saved by a newer DialogReader (format {v})"),
            LoadError::Io(e) => write!(f, "it could not be read ({e})"),
        }
    }
}

/// A profile file that could not be loaded, and why.
#[derive(Clone, Debug, PartialEq)]
pub struct Problem {
    pub path: PathBuf,
    pub error: LoadError,
    /// Where the broken file was moved (`<id>.json.bad`).
    pub kept_as: Option<PathBuf>,
}

/// Loads one profile file. A broken file is renamed to `<file>.bad` (an older `.bad` is replaced).
pub fn load(path: &Path) -> Result<Profile, Problem> {
    let text = std::fs::read_to_string(path).map_err(|e| Problem {
        path: path.to_path_buf(),
        error: LoadError::Io(e.to_string()),
        kept_as: None,
    })?;
    let mut p = Profile::from_json(&text).map_err(|error| {
        let kept_as = matches!(error, LoadError::Broken(_)).then(|| {
            let mut bad = path.as_os_str().to_owned();
            bad.push(".bad");
            let bad = PathBuf::from(bad);
            std::fs::rename(path, &bad).ok().map(|_| bad)
        });
        Problem {
            path: path.to_path_buf(),
            error,
            kept_as: kept_as.flatten(),
        }
    })?;
    // The file name is the id: a copied or hand-edited file with another id inside (or one like `..\x`)
    // still saves back to this file, and two files can never share an id.
    if let Some(stem) = path.file_stem().map(|s| s.to_string_lossy().into_owned())
        && p.id != stem
    {
        p.id = stem;
    }
    Ok(p)
}

/// Saves a profile as `profiles\<id>.json` (atomic).
pub fn save(paths: &Paths, p: &Profile) -> Result<(), String> {
    let path = paths.profile_json(&p.id);
    write_atomic(&path, p.to_json().as_bytes())
        .map_err(|e| format!("could not save {}: {e}", path.display()))
}

/// Every profile in `profiles\`, newest file first, and the files that could not be loaded.
pub fn load_all(paths: &Paths) -> (Vec<Profile>, Vec<Problem>) {
    let mut found: Vec<(std::time::SystemTime, PathBuf)> = match std::fs::read_dir(paths.profiles())
    {
        Ok(rd) => rd
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| {
                p.extension()
                    .is_some_and(|x| x.eq_ignore_ascii_case("json"))
            })
            .map(|p| {
                (
                    std::fs::metadata(&p)
                        .and_then(|m| m.modified())
                        .unwrap_or(std::time::UNIX_EPOCH),
                    p,
                )
            })
            .collect(),
        Err(_) => Vec::new(),
    };
    found.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    let (mut ok, mut bad) = (Vec::new(), Vec::new());
    for (_, p) in found {
        match load(&p) {
            Ok(v) => ok.push(v),
            Err(e) => bad.push(e),
        }
    }
    (ok, bad)
}

/// The ids that have any file in `profiles\` (`<id>.json`, `<id>.json.bad`, `<id>.json.tmp`, `<id>.png`), loaded
/// or not, in lower case. A new profile must not take one of them: it would overwrite a file made by a newer
/// DialogReader, or mix the new game up with a file kept as `.bad`.
pub fn ids_on_disk(paths: &Paths) -> Vec<String> {
    let Ok(rd) = std::fs::read_dir(paths.profiles()) else {
        return Vec::new();
    };
    rd.filter_map(Result::ok)
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_lowercase();
            [".json", ".png"]
                .iter()
                .find_map(|ext| name.find(ext).map(|i| name[..i].to_string()))
        })
        .collect()
}

/// Deletes a game: `profiles\<id>.json` (and its `.bad` and `.tmp` copies), `profiles\<id>.png` and its activity
/// log `log\<id>.jsonl`. Only files in the data folder, named from the id; an id that could point outside it (a
/// path separator, `..`) deletes nothing. A file that is not there is fine.
pub fn delete(paths: &Paths, id: &str) -> Result<(), String> {
    if id.is_empty() || id.contains(['/', '\\', ':']) || id.contains("..") {
        return Err(format!("not a profile id: {id:?}"));
    }
    let json = paths.profile_json(id);
    let with = |ext: &str| {
        let mut p = json.as_os_str().to_owned();
        p.push(ext);
        PathBuf::from(p)
    };
    let files = [
        json.clone(),
        with(".bad"),
        with(".tmp"),
        paths.profile_png(id),
        paths.activity_log(id),
    ];
    for f in files {
        match std::fs::remove_file(&f) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(format!("could not delete {}: {e}", f.display())),
        }
    }
    Ok(())
}

/// A file-safe id from a game name, unique among `taken` (case does not matter, as on a Windows disk):
/// "Red Dead Redemption 2" -> "red-dead-redemption-2".
pub fn new_id(name: &str, taken: &[String]) -> String {
    let mut base = String::new();
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            base.push(c.to_ascii_lowercase());
        } else if !base.ends_with('-') && !base.is_empty() {
            base.push('-');
        }
    }
    let base = base.trim_end_matches('-');
    let base: String = if base.is_empty() {
        "game".into()
    } else {
        base.chars().take(48).collect()
    };
    let base = base.trim_end_matches('-').to_string();
    let free = |c: &str| !taken.iter().any(|t| t.eq_ignore_ascii_case(c));
    if free(&base) {
        return base;
    }
    (2..)
        .map(|n| format!("{base}-{n}"))
        .find(|c| free(c))
        .expect("some id is free")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delete_removes_the_files_of_one_game_only() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::at(dir.path().join("DialogReader"));
        for id in ["gone", "kept"] {
            save(&paths, &Profile::new(id, id, "game.exe")).unwrap();
            std::fs::write(paths.profile_png(id), b"png").unwrap();
            std::fs::create_dir_all(paths.activity_log(id).parent().unwrap()).unwrap();
            std::fs::write(paths.activity_log(id), b"{}").unwrap();
        }
        std::fs::write(paths.profiles().join("gone.json.bad"), b"x").unwrap();
        delete(&paths, "gone").unwrap();
        assert!(!paths.profile_json("gone").exists());
        assert!(!paths.profile_png("gone").exists());
        assert!(!paths.activity_log("gone").exists());
        assert!(!paths.profiles().join("gone.json.bad").exists());
        assert!(paths.profile_json("kept").exists());
        assert!(paths.profile_png("kept").exists());
        assert!(paths.activity_log("kept").exists());
        assert_eq!(ids_on_disk(&paths), ["kept", "kept"]);
        // again: nothing left to delete is not an error
        delete(&paths, "gone").unwrap();
        // an id that could leave the folder deletes nothing
        for bad in ["", "..\\kept", "../kept", "c:kept"] {
            assert!(delete(&paths, bad).is_err(), "{bad:?}");
        }
        assert!(paths.profile_json("kept").exists());
    }

    #[test]
    fn ids_are_file_safe_and_unique() {
        assert_eq!(
            new_id("Red Dead Redemption 2", &[]),
            "red-dead-redemption-2"
        );
        assert_eq!(
            new_id("Clair Obscur: Expedition 33", &[]),
            "clair-obscur-expedition-33"
        );
        assert_eq!(new_id("  ***  ", &[]), "game");
        assert_eq!(
            new_id("Satisfactory", &["satisfactory".into()]),
            "satisfactory-2"
        );
        assert_eq!(
            new_id(
                "Satisfactory",
                &["satisfactory".into(), "satisfactory-2".into()]
            ),
            "satisfactory-3"
        );
        // case does not matter on a Windows disk
        assert_eq!(
            new_id("Satisfactory", &["Satisfactory".into()]),
            "satisfactory-2"
        );
    }

    #[test]
    fn kind_names_on_disk_and_in_the_ui() {
        let names: Vec<_> = Kind::ALL
            .iter()
            .map(|k| (k.disk_name(), k.ui_name()))
            .collect();
        assert_eq!(
            names,
            [
                ("dialogue", "Dialogue"),
                ("speaker", "Speaker"),
                ("notification", "Pickups"),
                ("prompt", "Prompts"),
                ("ignore", "Ignore")
            ]
        );
        for k in Kind::ALL {
            assert_eq!(
                serde_json::to_string(&k).unwrap(),
                format!("\"{}\"", k.disk_name())
            );
        }
    }
}
