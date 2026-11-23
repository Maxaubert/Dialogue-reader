//! Voices for the view model (M3a): the installed Windows voices, the speech thread for the voice test, and the
//! text the voice screens show.
//!
//! Only `Voices::windows()` (called by `main.rs`) can reach the speakers. Tests pass a fake engine and a null
//! sink, and render-screens uses the fake source, so no test plays sound.

use std::path::PathBuf;

use dr_runtime::speech::{ErrorKind, Speech, VoiceSettings};
use dr_tts::{Gender, Voice};

use crate::VoiceData;

/// The line the voice test speaks (page 05 shows it).
pub const SAMPLE: &str = "Arthur: We need to get these horses moving before the snow buries us.";

/// Bars of the wave on page 05.
pub const WAVE_BARS: i32 = 34;

/// The voice engine of v1, as the UI names it.
pub const ENGINE_LABEL: &str = "Windows";

pub struct Voices {
    pub list: Vec<Voice>,
    /// The error Windows gave when it could not list its voices (raw text, for the log). `None` with an empty
    /// list: no voice is installed.
    pub problem: Option<String>,
    /// The speech thread for the voice test. `None`: the test button only says that voices do not play here.
    pub speech: Option<Speech>,
    /// Where raw speech errors are written (`speech.log` in the logs folder). Only the app binary sets it.
    pub log: Option<PathBuf>,
}

impl Voices {
    /// No voices and no speech.
    pub fn none() -> Voices {
        Voices {
            list: Vec::new(),
            problem: None,
            speech: None,
            log: None,
        }
    }

    /// The Windows voices and a speech thread that plays on the speakers (the device opens at the first sound).
    /// Only the app binary calls this.
    #[cfg(windows)]
    pub fn windows() -> Voices {
        let (list, problem) = match dr_tts::WinTts::probe() {
            Ok(list) => (list, None),
            Err(why) => (Vec::new(), Some(why)),
        };
        let speech = Speech::start(
            Box::new(dr_tts::WinTts::new()),
            || Box::new(dr_runtime::speech::RodioOut::new()),
            VoiceSettings::default(),
        );
        let v = Voices {
            list,
            problem,
            speech: Some(speech),
            log: dr_runtime::paths::crash_logs().map(|d| d.join("speech.log")),
        };
        if let Some(p) = &v.problem {
            v.write_log(&format!("listing the Windows voices: {p}"));
        }
        v
    }

    /// Adds a line to the speech log (nothing when there is no log).
    pub fn write_log(&self, text: &str) {
        use std::io::Write;
        let Some(path) = &self.log else { return };
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
        {
            let _ = writeln!(f, "{secs} {text}");
        }
    }

    /// The index of voice `id`; the first voice when it is empty or not installed (that voice is then used).
    pub fn index_of(&self, id: &str) -> usize {
        self.list.iter().position(|v| v.id == id).unwrap_or(0)
    }

    /// The id to speak with for a saved id: itself when installed, else the first voice (empty when none).
    pub fn effective(&self, id: &str) -> String {
        self.list
            .get(self.index_of(id))
            .map(|v| v.id.clone())
            .unwrap_or_default()
    }

    /// "Windows · David" for a saved id (see [`Voices::index_of`]); "No voice" when none is installed.
    pub fn label(&self, id: &str) -> String {
        match self.list.get(self.index_of(id)) {
            Some(v) => format!("{ENGINE_LABEL} · {}", v.name),
            None => "No voice".into(),
        }
    }

    /// The Settings line under "Windows voices".
    pub fn desc(&self) -> String {
        match self.list.len() {
            0 => "Built into Windows · none installed".into(),
            1 => "Built into Windows · 1 voice".into(),
            n => format!("Built into Windows · {n} voices"),
        }
    }

    pub fn rows(&self) -> Vec<VoiceData> {
        self.list.iter().map(row).collect()
    }

    /// What page 05 says when the list is empty.
    pub fn problem_text(&self) -> String {
        if !self.list.is_empty() {
            String::new()
        } else if self.problem.is_some() {
            "Windows could not list its voices. Press Add voices to check the speech settings."
                .into()
        } else {
            NO_VOICE.into()
        }
    }
}

/// Page 05 with no Windows voice.
pub const NO_VOICE: &str = "No Windows voice is installed. Press Add voices to get one.";

/// The notice when the voice test did not play (the raw error goes to the log).
pub fn error_text(kind: ErrorKind) -> &'static str {
    match kind {
        ErrorKind::Engine => "The Windows voice could not speak the sample.",
        ErrorKind::NoDevice => "No sound device was found.",
        ErrorKind::DeviceLost => "The sound device stopped working. Try again.",
    }
}

/// "en-US" -> "American English"; other tags as they are.
pub fn lang_name(tag: &str) -> String {
    match tag.to_ascii_lowercase().as_str() {
        "en-us" => "American English",
        "en-gb" => "British English",
        "en-au" => "Australian English",
        "en-ca" => "Canadian English",
        "en-in" => "Indian English",
        "en-ie" => "Irish English",
        "en-nz" => "New Zealand English",
        _ => return tag.to_string(),
    }
    .into()
}

fn gender_name(g: Option<Gender>) -> Option<&'static str> {
    g.map(|g| match g {
        Gender::Male => "Male",
        Gender::Female => "Female",
    })
}

/// A list row: "David", "American English · Male", "US · Male".
pub fn row(v: &Voice) -> VoiceData {
    let region = v.lang.split('-').nth(1).unwrap_or(&v.lang).to_uppercase();
    let region = if region == "GB" {
        "UK".to_string()
    } else {
        region
    };
    let join = |a: String| match gender_name(v.gender) {
        Some(g) => format!("{a} · {g}"),
        None => a,
    };
    VoiceData {
        name: v.name.as_str().into(),
        desc: join(lang_name(&v.lang)).into(),
        short: join(region).into(),
    }
}

/// "0:02 / 0:04" (whole seconds, rounded up for the length).
pub fn time_text(pos_ms: f64, len_ms: f64) -> String {
    let m = |s: u64| format!("{}:{:02}", s / 60, s % 60);
    m((pos_ms.max(0.0) / 1000.0) as u64) + " / " + &m((len_ms.max(0.0) / 1000.0).ceil() as u64)
}

/// Wave bars played after `pos_ms` of `len_ms`.
pub fn bars_played(pos_ms: f64, len_ms: f64) -> i32 {
    if len_ms <= 0.0 {
        return 0;
    }
    ((pos_ms / len_ms).clamp(0.0, 1.0) * WAVE_BARS as f64).round() as i32
}

/// Profile speed (0.5 to 2) from a page-05 slider (0 to 1 for 0x to 2x), and back.
pub fn speed_of_slider(v: f32) -> f32 {
    (v * 2.0).clamp(dr_tts::SPEED_MIN, dr_tts::SPEED_MAX)
}

pub fn slider_of_speed(s: f32) -> f32 {
    (s / 2.0).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(id: &str, name: &str, lang: &str, g: Option<Gender>) -> Voice {
        Voice {
            id: id.into(),
            name: name.into(),
            lang: lang.into(),
            gender: g,
            engine: "windows",
        }
    }

    #[test]
    fn rows_labels_and_fallbacks() {
        let vs = Voices {
            list: vec![
                v("a", "David", "en-US", Some(Gender::Male)),
                v("b", "Hazel", "en-GB", Some(Gender::Female)),
            ],
            problem: None,
            speech: None,
            log: None,
        };
        let r = row(&vs.list[1]);
        assert_eq!(
            (r.desc.as_str(), r.short.as_str()),
            ("British English · Female", "UK · Female")
        );
        assert_eq!(row(&v("c", "X", "de-DE", None)).desc.as_str(), "de-DE");
        assert_eq!(vs.label("b"), "Windows · Hazel");
        // empty or uninstalled id: the first voice, also for speaking
        assert_eq!(vs.label(""), "Windows · David");
        assert_eq!(vs.effective("gone"), "a");
        assert_eq!(vs.desc(), "Built into Windows · 2 voices");
        assert_eq!(vs.problem_text(), "");
        let none = Voices::none();
        assert_eq!(none.label("a"), "No voice");
        assert_eq!(none.effective("a"), "");
        assert_eq!(
            none.problem_text(),
            "No Windows voice is installed. Press Add voices to get one."
        );
        // a raw Windows error stays out of the page
        let failed = Voices {
            problem: Some("Windows voices: Class not registered".into()),
            ..Voices::none()
        };
        assert_eq!(
            failed.problem_text(),
            "Windows could not list its voices. Press Add voices to check the speech settings."
        );
    }

    #[test]
    fn time_bars_and_speed() {
        assert_eq!(time_text(2100.0, 3500.0), "0:02 / 0:04");
        assert_eq!(time_text(0.0, 61_000.0), "0:00 / 1:01");
        assert_eq!(bars_played(500.0, 1000.0), 17);
        assert_eq!(bars_played(5.0, 0.0), 0);
        assert_eq!(speed_of_slider(0.55), 1.1);
        assert_eq!(speed_of_slider(0.1), 0.5);
        assert_eq!(slider_of_speed(1.1), 0.55);
    }
}
