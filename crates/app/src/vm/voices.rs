//! Voices for the view model: Kokoro voices (the main engine; they ship with the app, docs/app-spec.md
//! section 6), the Windows voices as the backup when Kokoro's files are missing
//! or its model does not load, the speech thread for the voice test, and the text the voice screens show.
//!
//! The list holds Kokoro's voices when Kokoro is there, else the Windows voices. When Kokoro's model fails to load
//! (it loads on its own thread), [`Voices::check_kokoro`] switches the list to the Windows voices. Ids are in the saved form of
//! `dr_tts::engines` (`kokoro:af_heart`, `win:<id>`); an old id without a prefix is a Windows id.
//!
//! Only `Voices::real()` (called by `main.rs`) looks for Kokoro's files and can reach the speakers. Tests pass a
//! fake engine and a null sink, and render-screens uses the fake source, so no test plays sound or loads a model.

use std::path::PathBuf;

use dr_runtime::speech::{ErrorKind, Speech, VoiceSettings};
use dr_tts::{Gender, Voice};

use crate::VoiceData;

/// The line the voice test speaks (page 05 shows it).
pub const SAMPLE: &str = "Arthur: We need to get these horses moving before the snow buries us.";

/// Bars of the wave on page 05.
pub const WAVE_BARS: i32 = 34;

/// An engine name as the UI shows it: "Kokoro", "Windows".
pub fn engine_label(engine: &str) -> &'static str {
    match engine {
        dr_tts::kokoro::ENGINE => "Kokoro",
        _ => "Windows",
    }
}

pub struct Voices {
    pub list: Vec<Voice>,
    /// Kokoro's files were found (and its model has not failed to load): `list` holds Kokoro's voices.
    pub kokoro: bool,
    /// The Windows voices (ids in saved form): the list when Kokoro is missing or fails to load.
    pub windows_list: Vec<Voice>,
    /// Kokoro's load state (read without waiting for the load). `None`: no Kokoro, or a test without a model.
    pub kokoro_load: Option<dr_tts::kokoro::LoadWatch>,
    /// How many Windows voices are installed (Settings > Voices shows it also when the list is Kokoro's).
    pub windows_count: usize,
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
            kokoro: false,
            windows_list: Vec::new(),
            kokoro_load: None,
            windows_count: 0,
            problem: None,
            speech: None,
            log: None,
        }
    }

    /// The list for `engines` ([`dr_tts::Engines::shown_voices`]), no speech thread.
    pub fn of(engines: &dr_tts::Engines, windows_count: usize, problem: Option<String>) -> Voices {
        Voices {
            list: engines.shown_voices(),
            kokoro: engines.has_kokoro(),
            windows_list: engines.windows_voices(),
            windows_count,
            problem,
            ..Voices::none()
        }
    }

    /// Kokoro (when its files are found; the model starts loading on its own thread now), the Windows voices, and
    /// a speech thread that plays on the speakers (the device opens at the first sound). Only the app binary calls
    /// this.
    #[cfg(windows)]
    pub fn real() -> Voices {
        let (win_list, problem) = match dr_tts::WinTts::probe() {
            Ok(list) => (list, None),
            Err(why) => (Vec::new(), Some(why)),
        };
        let log = dr_runtime::paths::crash_logs().map(|d| d.join("speech.log"));
        let located = dr_tts::KokoroFiles::locate();
        let found = located.as_ref().ok().cloned();
        let (kokoro, watch, missing) = match located {
            Ok(files) => {
                let k = dr_tts::KokoroTts::new(files);
                k.preload();
                let watch = k.watch();
                (Some(Box::new(k) as Box<dyn dr_tts::Tts>), Some(watch), None)
            }
            Err(why) => (None, None, Some(why)),
        };
        let note_log = log.clone();
        let engines = dr_tts::Engines::new(kokoro, Some(Box::new(dr_tts::WinTts::new())))
            .with_note(Box::new(move |text| {
                if let Some(p) = &note_log {
                    append_log(p, text);
                }
            }));
        let mut v = Voices::of(&engines, win_list.len(), problem);
        v.kokoro_load = watch;
        v.speech = Some(Speech::start(
            Box::new(engines),
            || Box::new(dr_runtime::speech::RodioOut::new()),
            VoiceSettings::default(),
        ));
        v.log = log;
        if let Some(m) = &missing {
            v.write_log(m);
        }
        if let Some(f) = &found {
            // which files this start uses: an installed app must name its own folder, not Sonara's (M5 check)
            v.write_log(&format!(
                "Kokoro files: model {}, voices {}, runtime {}",
                f.model.display(),
                f.voices.display(),
                f.runtime.display()
            ));
        }
        if let Some(p) = &v.problem {
            v.write_log(&format!("listing the Windows voices: {p}"));
        }
        v
    }

    /// Adds a line to the speech log (nothing when there is no log).
    pub fn write_log(&self, text: &str) {
        if let Some(path) = &self.log {
            append_log(path, text);
        }
    }

    /// When Kokoro's model failed to load: the list becomes the Windows voices, the failure goes to the log, and
    /// true is returned (the screens must show the new list). Reads the load state without waiting for a load.
    pub fn check_kokoro(&mut self) -> bool {
        if !self.kokoro {
            return false;
        }
        let Some(dr_tts::kokoro::LoadState::Failed(why)) =
            self.kokoro_load.as_ref().map(|w| w.get())
        else {
            return false;
        };
        self.kokoro = false;
        self.list = self.windows_list.clone();
        self.write_log(&format!(
            "Kokoro could not load, the Windows voices are listed instead: {why}"
        ));
        true
    }

    /// The index of voice `id` (an old id without a prefix is a Windows id); the first voice when it is empty or
    /// not in the list (that voice is then used: Kokoro's Heart when Kokoro is there).
    pub fn index_of(&self, id: &str) -> usize {
        let id = dr_tts::engines::canonical(id);
        self.list.iter().position(|v| v.id == id).unwrap_or(0)
    }

    /// The id to speak with for a saved id: itself when installed, else the first voice (empty when none).
    pub fn effective(&self, id: &str) -> String {
        self.list
            .get(self.index_of(id))
            .map(|v| v.id.clone())
            .unwrap_or_default()
    }

    /// The voice name for the Overview ("Heart", "David") of a saved id (see [`Voices::index_of`]); "No voice" when
    /// there is none.
    pub fn label(&self, id: &str) -> String {
        match self.list.get(self.index_of(id)) {
            Some(v) => v.name.clone(),
            None => "No voice".into(),
        }
    }

    /// The Settings line under "Kokoro", in the approved mockup's form ("Installed · 11 voices · 312 MB"): the
    /// voice count and the size of the model and voice files (rounded to whole MB).
    pub fn kokoro_desc(&self) -> String {
        if self.kokoro {
            let mb =
                (dr_tts::kokoro::MODEL_SIZE + dr_tts::kokoro::VOICES_SIZE + 500_000) / 1_000_000;
            format!("Installed · {} voices · {mb} MB", self.list.len())
        } else {
            "Not found · Windows voices speak instead".into()
        }
    }

    /// The Settings "Voices" row: how many voices each engine has ("Kokoro 11 · Windows 11"). Kokoro is left out
    /// when its files are missing.
    pub fn desc(&self) -> String {
        match (self.kokoro, self.windows_count) {
            (true, n) => format!("Kokoro {} · Windows {n}", self.list.len()),
            (false, 0) => "No voices found".into(),
            (false, n) => format!("Windows {n}"),
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

/// Page 05 with no voice at all: no Kokoro files and no Windows voice.
pub const NO_VOICE: &str = "The Kokoro voices are missing and no Windows voice is installed. Press Add voices to get a Windows voice.";

/// The notice when the voice test did not play (the raw error goes to the log).
pub fn error_text(kind: ErrorKind) -> &'static str {
    match kind {
        ErrorKind::Engine => "The voice could not speak the sample.",
        ErrorKind::NoDevice => "No sound device was found.",
        ErrorKind::DeviceLost => "The sound device stopped working. Try again.",
    }
}

/// Adds a line to the log at `path`, with the time in seconds.
pub(crate) fn append_log(path: &std::path::Path, text: &str) {
    use std::io::Write;
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
        engine: engine_label(v.engine).into(),
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

    fn windows_fake() -> dr_tts::FakeTts {
        dr_tts::FakeTts::with_voices(vec![
            v("a", "David", "en-US", Some(Gender::Male)),
            v("b", "Hazel", "en-GB", Some(Gender::Female)),
        ])
    }

    #[test]
    fn rows_labels_and_fallbacks() {
        let engines = dr_tts::Engines::new(None, Some(Box::new(windows_fake())));
        let vs = Voices::of(&engines, 2, None);
        let r = row(&vs.list[1]);
        assert_eq!(
            (r.desc.as_str(), r.short.as_str(), r.engine.as_str()),
            ("British English · Female", "UK · Female", "Windows")
        );
        assert_eq!(row(&v("c", "X", "de-DE", None)).desc.as_str(), "de-DE");
        // ids carry the engine; an old id without it is a Windows id
        assert_eq!(vs.list[1].id, "win:b");
        assert_eq!(vs.label("win:b"), "Hazel");
        assert_eq!(vs.index_of("b"), 1);
        // empty or uninstalled id: the first voice, also for speaking
        assert_eq!(vs.label(""), "David");
        assert_eq!(vs.effective("gone"), "win:a");
        assert_eq!(vs.desc(), "Windows 2");
        assert_eq!(vs.kokoro_desc(), "Not found · Windows voices speak instead");
        assert_eq!(vs.problem_text(), "");
        let none = Voices::none();
        assert_eq!(none.label("a"), "No voice");
        assert_eq!(none.effective("a"), "");
        assert_eq!(
            none.problem_text(),
            "The Kokoro voices are missing and no Windows voice is installed. Press Add voices to get a Windows voice."
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
    fn kokoro_first_and_windows_counted_apart() {
        let kokoro = dr_tts::FakeTts::with_voices(vec![
            dr_tts::kokoro::voice_of("af_heart"),
            dr_tts::kokoro::voice_of("am_adam"),
            dr_tts::kokoro::voice_of("bm_george"),
        ]);
        let engines = dr_tts::Engines::new(Some(Box::new(kokoro)), Some(Box::new(windows_fake())));
        let vs = Voices::of(&engines, 2, None);
        assert!(vs.kokoro);
        let ids: Vec<&str> = vs.list.iter().map(|v| v.id.as_str()).collect();
        assert_eq!(
            ids,
            ["kokoro:af_heart", "kokoro:am_adam", "kokoro:bm_george"]
        );
        // a new game speaks with Heart; an old Windows id that is not in the list too
        assert_eq!(vs.label(""), "Heart");
        assert_eq!(vs.effective(""), "kokoro:af_heart");
        assert_eq!(vs.effective("a"), "kokoro:af_heart");
        assert_eq!(vs.label("kokoro:am_adam"), "Adam");
        let r = row(&vs.list[2]);
        assert_eq!(
            (
                r.name.as_str(),
                r.desc.as_str(),
                r.short.as_str(),
                r.engine.as_str()
            ),
            ("George", "British English · Male", "UK · Male", "Kokoro")
        );
        assert_eq!(vs.kokoro_desc(), "Installed · 3 voices · 354 MB");
        assert_eq!(vs.desc(), "Kokoro 3 · Windows 2");
        assert_eq!(
            error_text(ErrorKind::Engine),
            "The voice could not speak the sample."
        );
    }

    #[test]
    fn a_kokoro_that_fails_to_load_gives_the_windows_list() {
        use dr_tts::kokoro::{LoadState, LoadWatch};
        let kokoro = dr_tts::FakeTts::with_voices(vec![dr_tts::kokoro::voice_of("af_heart")]);
        let engines = dr_tts::Engines::new(Some(Box::new(kokoro)), Some(Box::new(windows_fake())));
        let dir = tempfile::tempdir().unwrap();
        let watch = LoadWatch::default();
        let mut vs = Voices {
            kokoro_load: Some(watch.clone()),
            log: Some(dir.path().join("speech.log")),
            ..Voices::of(&engines, 2, None)
        };
        // still loading, then loaded: nothing changes
        assert!(!vs.check_kokoro());
        watch.set(LoadState::Ready);
        assert!(!vs.check_kokoro());
        assert_eq!(vs.label(""), "Heart");
        // failed: the Windows voices, once
        watch.set(LoadState::Failed("onnxruntime.dll is too old".into()));
        assert!(vs.check_kokoro());
        assert!(!vs.check_kokoro());
        assert!(!vs.kokoro);
        assert_eq!(vs.label(""), "David");
        assert_eq!(vs.effective("kokoro:af_heart"), "win:a");
        assert_eq!(vs.kokoro_desc(), "Not found · Windows voices speak instead");
        let log = std::fs::read_to_string(dir.path().join("speech.log")).unwrap();
        assert!(log.contains("too old"), "{log}");
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
