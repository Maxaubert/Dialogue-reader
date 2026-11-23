//! Kokoro first, Windows voices as the backup (M3a; ships with the app, see
//! docs/app-spec.md section 6). [`Engines`] is one [`Tts`] over both, so the speech thread keeps one engine.
//!
//! Voice ids saved in a profile say which engine they belong to: `kokoro:af_heart`, `win:<Windows voice id>`.
//! An id with neither prefix was saved before Kokoro came (M3a) and is a Windows id. The prefixes are added and
//! taken off here only; [`crate::KokoroTts`], [`crate::WinTts`] and [`crate::FakeTts`] use their own ids.

use crate::{Pcm, Tts, Voice};

pub const KOKORO_PREFIX: &str = "kokoro:";
pub const WINDOWS_PREFIX: &str = "win:";

/// Which engine a saved id is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Which {
    Kokoro,
    Windows,
    /// An empty id: the main engine's default voice.
    Default,
}

/// The engine of a saved id and the id that engine knows.
pub fn split(id: &str) -> (Which, &str) {
    if id.is_empty() {
        (Which::Default, "")
    } else if let Some(k) = id.strip_prefix(KOKORO_PREFIX) {
        (Which::Kokoro, k)
    } else if let Some(w) = id.strip_prefix(WINDOWS_PREFIX) {
        (Which::Windows, w)
    } else {
        (Which::Windows, id)
    }
}

/// The saved form of an id: an old id without a prefix gets `win:`; other ids stay as they are.
pub fn canonical(id: &str) -> String {
    match split(id) {
        (Which::Default, _) => String::new(),
        (Which::Kokoro, k) => format!("{KOKORO_PREFIX}{k}"),
        (Which::Windows, w) => format!("{WINDOWS_PREFIX}{w}"),
    }
}

/// `v` with its id in the saved form for engine `which`.
fn tagged(mut v: Voice, which: Which) -> Voice {
    v.id = match which {
        Which::Kokoro => format!("{KOKORO_PREFIX}{}", v.id),
        _ => format!("{WINDOWS_PREFIX}{}", v.id),
    };
    v
}

/// Where [`Engines`] notes that it spoke with the other engine (the speech log in the app).
pub type Note = Box<dyn Fn(&str) + Send>;

/// Kokoro (when its files are there) and the Windows voices.
pub struct Engines {
    kokoro: Option<Box<dyn Tts>>,
    windows: Option<Box<dyn Tts>>,
    note: Option<Note>,
    /// Kokoro failed once and that was noted (noted once per run, not per line).
    kokoro_failed: bool,
}

impl Engines {
    pub fn new(kokoro: Option<Box<dyn Tts>>, windows: Option<Box<dyn Tts>>) -> Engines {
        Engines {
            kokoro,
            windows,
            note: None,
            kokoro_failed: false,
        }
    }

    /// Engines that write to `note` the first time Kokoro fails and the Windows voice speaks instead.
    pub fn with_note(mut self, note: Note) -> Engines {
        self.note = Some(note);
        self
    }

    pub fn has_kokoro(&self) -> bool {
        self.kokoro.is_some()
    }

    /// The voices the app offers: Kokoro's when Kokoro is there, else the Windows voices. Ids in saved form.
    pub fn shown_voices(&self) -> Vec<Voice> {
        match &self.kokoro {
            Some(k) => k
                .voices()
                .into_iter()
                .map(|v| tagged(v, Which::Kokoro))
                .collect(),
            None => self.windows_voices(),
        }
    }

    /// The Windows voices, ids in saved form.
    pub fn windows_voices(&self) -> Vec<Voice> {
        self.windows
            .as_ref()
            .map(|w| {
                w.voices()
                    .into_iter()
                    .map(|v| tagged(v, Which::Windows))
                    .collect()
            })
            .unwrap_or_default()
    }
}

impl Tts for Engines {
    /// Every voice of both engines (Kokoro first), ids in saved form.
    fn voices(&self) -> Vec<Voice> {
        let mut all: Vec<Voice> = self
            .kokoro
            .as_ref()
            .map(|k| {
                k.voices()
                    .into_iter()
                    .map(|v| tagged(v, Which::Kokoro))
                    .collect()
            })
            .unwrap_or_default();
        all.extend(self.windows_voices());
        all
    }

    /// Speaks with the engine of the id. Empty: Kokoro's default voice, or the Windows default without Kokoro.
    /// - A Kokoro id with no Kokoro, or when Kokoro fails (its model did not load), speaks with the default
    ///   Windows voice. When that fails too, the error is Kokoro's.
    /// - A Windows id that Windows cannot speak (no Windows voice) speaks with Kokoro's default voice. When that
    ///   fails too, the error is the Windows one.
    ///
    /// No voice list is asked for here: listing the Windows voices costs a pass over every voice, per line.
    fn synth(&mut self, text: &str, voice: &str, speed: f32) -> Result<Pcm, String> {
        let (which, id) = split(voice);
        match which {
            Which::Kokoro | Which::Default => {
                let Some(k) = &mut self.kokoro else {
                    return match &mut self.windows {
                        Some(w) => w.synth(text, "", speed),
                        None => Err("no voice engine is available".into()),
                    };
                };
                let error = match k.synth(text, id, speed) {
                    Ok(pcm) => return Ok(pcm),
                    Err(e) => e,
                };
                let Some(w) = &mut self.windows else {
                    return Err(error);
                };
                match w.synth(text, "", speed) {
                    Ok(pcm) => {
                        if !self.kokoro_failed {
                            self.kokoro_failed = true;
                            if let Some(note) = &self.note {
                                note(&format!(
                                    "Kokoro could not speak, the Windows default voice speaks instead: {error}"
                                ));
                            }
                        }
                        Ok(pcm)
                    }
                    Err(_) => Err(error),
                }
            }
            Which::Windows => {
                let Some(w) = &mut self.windows else {
                    return match &mut self.kokoro {
                        Some(k) => k.synth(text, "", speed),
                        None => Err("no voice engine is available".into()),
                    };
                };
                let error = match w.synth(text, id, speed) {
                    Ok(pcm) => return Ok(pcm),
                    Err(e) => e,
                };
                match &mut self.kokoro {
                    Some(k) => k.synth(text, "", speed).map_err(|_| error),
                    None => Err(error),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fake::FakeTts;
    use crate::{Gender, Voice};

    fn kokoro_fake() -> FakeTts {
        FakeTts::with_voices(vec![Voice {
            id: "af_heart".into(),
            name: "Heart".into(),
            lang: "en-US".into(),
            gender: Some(Gender::Female),
            engine: "kokoro",
        }])
    }

    #[test]
    fn ids_say_their_engine_and_old_ids_are_windows() {
        assert_eq!(split("kokoro:af_heart"), (Which::Kokoro, "af_heart"));
        assert_eq!(split("win:HKEY\\David"), (Which::Windows, "HKEY\\David"));
        assert_eq!(split("HKEY\\David"), (Which::Windows, "HKEY\\David"));
        assert_eq!(split(""), (Which::Default, ""));
        assert_eq!(canonical("HKEY\\David"), "win:HKEY\\David");
        assert_eq!(canonical("win:x"), "win:x");
        assert_eq!(canonical("kokoro:af_heart"), "kokoro:af_heart");
        assert_eq!(canonical(""), "");
    }

    #[test]
    fn routes_each_id_to_its_engine() {
        let (k, w) = (kokoro_fake(), FakeTts::new());
        let mut e = Engines::new(Some(Box::new(k.clone())), Some(Box::new(w.clone())));
        let shown: Vec<String> = e.shown_voices().into_iter().map(|v| v.id).collect();
        assert_eq!(shown, ["kokoro:af_heart"]);
        let all: Vec<String> = e.voices().into_iter().map(|v| v.id).collect();
        assert_eq!(all, ["kokoro:af_heart", "win:fake-david", "win:fake-zira"]);

        e.synth("a", "kokoro:af_heart", 1.0).unwrap();
        e.synth("b", "", 1.0).unwrap();
        e.synth("c", "win:fake-zira", 1.5).unwrap();
        e.synth("d", "fake-david", 1.0).unwrap();
        let kc: Vec<(String, String)> = k.calls().into_iter().map(|c| (c.text, c.voice)).collect();
        assert_eq!(
            kc,
            [("a".into(), "af_heart".into()), ("b".into(), "".into())]
        );
        let wc: Vec<(String, String, f32)> = w
            .calls()
            .into_iter()
            .map(|c| (c.text, c.voice, c.speed))
            .collect();
        assert_eq!(
            wc,
            [
                ("c".into(), "fake-zira".into(), 1.5),
                ("d".into(), "fake-david".into(), 1.0)
            ]
        );
    }

    #[test]
    fn falls_back_to_the_other_engine() {
        // no Kokoro: Windows voices are shown, and a Kokoro id speaks with the Windows default
        let w = FakeTts::new();
        let mut e = Engines::new(None, Some(Box::new(w.clone())));
        assert!(!e.has_kokoro());
        assert_eq!(e.shown_voices()[0].id, "win:fake-david");
        e.synth("a", "kokoro:af_heart", 1.0).unwrap();
        assert_eq!(w.calls()[0].voice, "");

        // Kokoro and no Windows voice (Windows fails to speak, as `DefaultVoice` does on Max's PC): an old
        // Windows id speaks with Kokoro's default
        let k = kokoro_fake();
        let no_win = FakeTts::with_voices(Vec::new());
        no_win.fail_with(Some("Windows voices: 0x80070002"));
        let mut e = Engines::new(Some(Box::new(k.clone())), Some(Box::new(no_win)));
        e.synth("b", "HKEY\\David", 1.0).unwrap();
        assert_eq!(k.calls()[0].voice, "");

        let mut none = Engines::new(None, None);
        assert!(none.voices().is_empty());
        assert!(none.synth("c", "", 1.0).is_err());
    }

    #[test]
    fn a_kokoro_that_fails_speaks_with_windows_and_notes_it_once() {
        let k = kokoro_fake();
        k.fail_with(Some("Kokoro: onnxruntime.dll is too old"));
        let w = FakeTts::new();
        let notes = std::sync::Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
        let n = notes.clone();
        let mut e = Engines::new(Some(Box::new(k.clone())), Some(Box::new(w.clone())))
            .with_note(Box::new(move |s| n.lock().unwrap().push(s.into())));
        e.synth("a", "kokoro:af_heart", 1.0).unwrap();
        e.synth("b", "", 1.0).unwrap();
        let wc: Vec<(String, String)> = w.calls().into_iter().map(|c| (c.text, c.voice)).collect();
        assert_eq!(wc, [("a".into(), "".into()), ("b".into(), "".into())]);
        let notes = notes.lock().unwrap();
        assert_eq!(notes.len(), 1, "{notes:?}");
        assert!(notes[0].contains("too old"), "{notes:?}");

        // Kokoro fails and Windows has no voice either: the error is Kokoro's, not the Windows one
        let no_win = FakeTts::with_voices(Vec::new());
        no_win.fail_with(Some("Windows voices: no voice installed"));
        let mut e = Engines::new(Some(Box::new(k.clone())), Some(Box::new(no_win)));
        assert_eq!(
            e.synth("c", "", 1.0).unwrap_err(),
            "Kokoro: onnxruntime.dll is too old"
        );
        let mut e = Engines::new(Some(Box::new(k)), None);
        assert_eq!(
            e.synth("d", "kokoro:af_heart", 1.0).unwrap_err(),
            "Kokoro: onnxruntime.dll is too old"
        );
    }

    #[test]
    fn a_windows_voice_that_fails_speaks_with_kokoro() {
        let k = kokoro_fake();
        let w = FakeTts::new();
        w.fail_with(Some("Windows voices: speak failed"));
        let mut e = Engines::new(Some(Box::new(k.clone())), Some(Box::new(w.clone())));
        e.synth("a", "win:fake-zira", 1.0).unwrap();
        assert_eq!(k.calls()[0].voice, "");
        // both fail: the Windows error
        k.fail_with(Some("no model"));
        assert_eq!(
            e.synth("b", "win:fake-zira", 1.0).unwrap_err(),
            "Windows voices: speak failed"
        );
    }
}
