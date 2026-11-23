//! App settings on disk: `%APPDATA%\DialogReader\settings.json` (spec 2.8, M5).
//!
//! What is not per game: Startup and the in-game overlay of Settings. Keys and voices stay in each game's profile.
//! "Start with Windows" is saved here too, but the Run value in the registry is the truth (src/autostart.rs): the
//! app reads it at start, so the toggle never claims something Windows will not do.
//!
//! A file that cannot be read is kept as `settings.json.bad` and the app starts with the defaults and says so, as
//! profiles do. A file from a newer DialogReader is left alone (not renamed) and the defaults are used.

use serde::{Deserialize, Serialize};

use crate::paths::{Paths, write_atomic};

/// The settings file format. A newer file is not overwritten by mistake: see [`load`].
pub const VERSION: u32 = 1;

/// Where the small status pill sits over the game (Settings > In-game overlay > Position).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PillAt {
    #[default]
    TopRight,
    BottomCenter,
    Off,
}

impl PillAt {
    /// The index of the Segmented control: 0 top right, 1 bottom center, 2 off.
    pub fn index(self) -> i32 {
        match self {
            PillAt::TopRight => 0,
            PillAt::BottomCenter => 1,
            PillAt::Off => 2,
        }
    }

    pub fn of_index(i: i32) -> PillAt {
        match i {
            1 => PillAt::BottomCenter,
            2 => PillAt::Off,
            _ => PillAt::TopRight,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub version: u32,
    /// Off by default (spec 2.9). The registry decides at start (see the module docs).
    pub start_with_windows: bool,
    /// When Windows starts DialogReader at sign-in (`--minimized`), only the tray icon shows.
    pub open_minimized: bool,
    /// The main window has a taskbar button. Off: only the tray icon (and Alt+Tab) reach it.
    pub show_in_taskbar: bool,
    pub pill_position: PillAt,
    pub show_speaker: bool,
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            version: VERSION,
            start_with_windows: false,
            open_minimized: true,
            show_in_taskbar: true,
            pill_position: PillAt::TopRight,
            show_speaker: true,
        }
    }
}

/// Why the settings file was not used.
#[derive(Clone, Debug, PartialEq)]
pub enum Problem {
    /// Not a settings file (broken JSON, wrong types). It was renamed to `settings.json.bad`.
    Broken { error: String, kept: bool },
    /// Written by a newer DialogReader. Left as it is; the defaults are used and not saved over it.
    Newer(u32),
}

impl Problem {
    /// The notice under the title bar.
    pub fn text(&self) -> String {
        match self {
            Problem::Broken { kept: true, .. } => {
                "The settings file could not be read, so the default settings are used. It was kept as \
                 settings.json.bad."
                    .into()
            }
            Problem::Broken { kept: false, .. } => {
                "The settings file could not be read, so the default settings are used.".into()
            }
            Problem::Newer(_) => {
                "The settings file is from a newer DialogReader, so the default settings are used and it is not \
                 changed."
                    .into()
            }
        }
    }
}

impl Settings {
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("settings always serialize")
    }

    /// Parses a settings file. Missing fields take their defaults; unknown fields are ignored.
    pub fn from_json(text: &str) -> Result<Settings, Problem> {
        let v: serde_json::Value = serde_json::from_str(text).map_err(|e| Problem::Broken {
            error: e.to_string(),
            kept: false,
        })?;
        if let Some(n) = v.get("version").and_then(|n| n.as_u64())
            && n > VERSION as u64
        {
            return Err(Problem::Newer(n as u32));
        }
        let mut s: Settings = serde_json::from_value(v).map_err(|e| Problem::Broken {
            error: e.to_string(),
            kept: false,
        })?;
        s.version = VERSION;
        Ok(s)
    }
}

/// The settings in `paths`, or the defaults when there is no file yet. A broken file is renamed to `.bad`.
/// `Some(problem)` when the file was there but not used. `newer` in the result means: do not save over it.
pub fn load(paths: &Paths) -> Loaded {
    let path = paths.settings();
    let bytes = match std::fs::read(&path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Loaded::default(),
        Err(e) => {
            return Loaded {
                settings: Settings::default(),
                problem: Some(Problem::Broken {
                    error: e.to_string(),
                    kept: false,
                }),
                read_only: true,
            };
        }
    };
    // text that is not UTF-8 is a broken file too (renamed to .bad below); a byte order mark is fine
    let parsed = match String::from_utf8(bytes) {
        Ok(t) => Settings::from_json(t.strip_prefix('\u{feff}').unwrap_or(&t)),
        Err(e) => Err(Problem::Broken {
            error: format!("not UTF-8 text: {e}"),
            kept: false,
        }),
    };
    match parsed {
        Ok(settings) => Loaded {
            settings,
            problem: None,
            read_only: false,
        },
        Err(Problem::Broken { error, .. }) => {
            let mut bad = path.as_os_str().to_owned();
            bad.push(".bad");
            let kept = std::fs::rename(&path, &bad).is_ok();
            Loaded {
                settings: Settings::default(),
                problem: Some(Problem::Broken { error, kept }),
                read_only: !kept,
            }
        }
        Err(p) => Loaded {
            settings: Settings::default(),
            problem: Some(p),
            read_only: true,
        },
    }
}

/// What [`load`] found.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Loaded {
    pub settings: Settings,
    pub problem: Option<Problem>,
    /// The file must not be written (it is from a newer DialogReader, or it could not be moved aside).
    pub read_only: bool,
}

/// Writes `settings.json` atomically.
pub fn save(paths: &Paths, s: &Settings) -> Result<(), String> {
    write_atomic(&paths.settings(), s.to_json().as_bytes()).map_err(|e| e.to_string())
}
