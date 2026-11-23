//! Kokoro voices (built 2026-11-23; ships with the app, docs/app-spec.md section 6).
//! Kokoro-82M v1.0 (Apache-2.0 weights) on Microsoft's CPU ONNX Runtime, with phonemes from Sonara's vendored
//! misaki (no espeak). 28 English voices, 24 kHz mono.
//!
//! - [`KokoroFiles`]: where the model, the voice styles and `onnxruntime.dll` are. [`KokoroFiles::locate`] looks in
//!   the app folder first, then `%LOCALAPPDATA%\DialogReader\models\kokoro\v1.0`, then (debug builds, or with
//!   `DIALOGREADER_SONARA_KOKORO=1`) Sonara's folders as a developer fallback. Nothing is downloaded yet.
//! - [`KokoroTts`]: the engine. The voice list needs no model. The model is loaded once (about a second, a few
//!   hundred MB) at the first `synth` or by [`KokoroTts::preload`] on its own thread, then reused by every clone.
//!   [`KokoroTts::watch`] tells another thread (the UI) whether the load worked, without waiting for it.
//!
//! The model, voice and phoneme code is copied from Sonara (MIT, commit 8fd2af7, see
//! `LICENSE-Sonara`), moved to the `ort` version this workspace pins. Its model manager (download, hashing) is
//! not copied: DialogReader only reads files that are already there.

mod fallback;
pub mod lead_in;
pub mod model;
pub mod phonemes;
pub mod text;
pub mod vocab;
pub mod voices;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use crate::{Gender, Pcm, SPEED_MAX, SPEED_MIN, Tts, Voice};
use model::{Acoustic, SAMPLE_RATE};
use phonemes::Phonemizer;
use voices::{DEFAULT_VOICE, ENGLISH_VOICES, Styles};

/// Engine name saved in the profile (`voice.engine`).
pub const ENGINE: &str = "kokoro";

/// The model file and the voice styles file (Kokoro v1.0, from the kokoro-onnx release `model-files-v1.0`).
pub const MODEL_FILE: &str = "kokoro-v1.0.onnx";
pub const VOICES_FILE: &str = "voices-v1.0.bin";
/// Their sizes in bytes (Sonara `download::pinned_files`): a file of another size is a broken copy.
pub const MODEL_SIZE: u64 = 325_532_387;
pub const VOICES_SIZE: u64 = 28_214_398;
/// ONNX Runtime (Microsoft, MIT), loaded from a full path.
pub const RUNTIME_FILE: &str = "onnxruntime.dll";

/// Silence between two sentences. Each sentence is made on its own and its silent edges are cut, so without
/// this the sentences would run into each other.
pub const SENTENCE_GAP_MS: u32 = 250;

/// An error of the copied Sonara code.
#[derive(Clone, Debug, PartialEq)]
pub enum Error {
    Engine(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Engine(e) => f.write_str(e),
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;

/// Where Kokoro's files are.
#[derive(Clone, Debug, PartialEq)]
pub struct KokoroFiles {
    pub model: PathBuf,
    pub voices: PathBuf,
    pub runtime: PathBuf,
}

/// The folders [`KokoroFiles::locate`] looks in, in order.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SearchDirs {
    /// Folders that hold `kokoro-v1.0.onnx` and `voices-v1.0.bin`.
    pub model: Vec<PathBuf>,
    /// Folders that hold `onnxruntime.dll`.
    pub runtime: Vec<PathBuf>,
}

impl SearchDirs {
    /// The folders of this PC:
    /// - model: `<exe dir>\kokoro`, `%LOCALAPPDATA%\DialogReader\models\kokoro\v1.0`, then Sonara's
    ///   `%LOCALAPPDATA%\Sonara\models\kokoro\v1.0` and `%USERPROFILE%\.sonara\kokoro`;
    /// - runtime: `<exe dir>`, then Sonara's `%LOCALAPPDATA%\Sonara\runtime\<version>` folders (newest first).
    ///
    /// Sonara's folders are a developer fallback ([`sonara_fallback`]): a shipped build must not load another
    /// app's DLL and model, which change when that app updates and would hide a broken install.
    pub fn of_this_pc() -> SearchDirs {
        let exe_dir = std::env::current_exe()
            .ok()
            .and_then(|e| e.parent().map(Path::to_path_buf));
        let local = std::env::var_os("LOCALAPPDATA").map(PathBuf::from);
        let home = std::env::var_os("USERPROFILE").map(PathBuf::from);
        let mut d = SearchDirs::default();
        if let Some(e) = &exe_dir {
            d.model.push(e.join("kokoro"));
            d.runtime.push(e.clone());
        }
        if let Some(l) = &local {
            d.model.push(l.join(r"DialogReader\models\kokoro\v1.0"));
        }
        let env = std::env::var("DIALOGREADER_SONARA_KOKORO").ok();
        if !sonara_fallback(cfg!(debug_assertions), env.as_deref()) {
            return d;
        }
        if let Some(l) = &local {
            d.model.push(l.join(r"Sonara\models\kokoro\v1.0"));
        }
        if let Some(h) = &home {
            d.model.push(h.join(r".sonara\kokoro"));
        }
        if let Some(l) = &local {
            d.runtime.extend(newest_first(&l.join(r"Sonara\runtime")));
        }
        d
    }
}

/// Sonara's folders are searched in a debug build, or when `DIALOGREADER_SONARA_KOKORO` is `1` (a release build
/// on a developer PC).
pub fn sonara_fallback(debug: bool, env: Option<&str>) -> bool {
    debug || env.is_some_and(|v| v.trim() == "1")
}

/// The subfolders of `dir`, newest version first (names compared as dotted numbers).
fn newest_first(dir: &Path) -> Vec<PathBuf> {
    let Ok(read) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut dirs: Vec<PathBuf> = read
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_dir())
        .collect();
    let key = |p: &PathBuf| -> Vec<u64> {
        p.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .split('.')
            .map(|s| s.parse().unwrap_or(0))
            .collect()
    };
    dirs.sort_by_key(|p| std::cmp::Reverse(key(p)));
    dirs
}

fn has_size(p: &Path, size: u64) -> bool {
    std::fs::metadata(p).is_ok_and(|m| m.is_file() && m.len() == size)
}

impl KokoroFiles {
    /// The first folder in `dirs.model` with both model files at their sizes, and the first `onnxruntime.dll` in
    /// `dirs.runtime`. The error says what is missing (for the log, not the UI).
    pub fn find(dirs: &SearchDirs) -> std::result::Result<KokoroFiles, String> {
        Self::find_with_sizes(dirs, MODEL_SIZE, VOICES_SIZE)
    }

    /// [`KokoroFiles::find`] with other sizes (tests use small files).
    pub fn find_with_sizes(
        dirs: &SearchDirs,
        model_size: u64,
        voices_size: u64,
    ) -> std::result::Result<KokoroFiles, String> {
        let model_dir = dirs.model.iter().find(|d| {
            has_size(&d.join(MODEL_FILE), model_size) && has_size(&d.join(VOICES_FILE), voices_size)
        });
        let runtime = dirs
            .runtime
            .iter()
            .map(|d| d.join(RUNTIME_FILE))
            .find(|p| p.is_file());
        let list = |v: &[PathBuf]| {
            v.iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>()
                .join("; ")
        };
        match (model_dir, runtime) {
            (Some(d), Some(runtime)) => Ok(KokoroFiles {
                model: d.join(MODEL_FILE),
                voices: d.join(VOICES_FILE),
                runtime,
            }),
            (None, _) => Err(format!(
                "Kokoro: no folder has {MODEL_FILE} and {VOICES_FILE} (looked in {})",
                list(&dirs.model)
            )),
            (_, None) => Err(format!(
                "Kokoro: {RUNTIME_FILE} not found (looked in {})",
                list(&dirs.runtime)
            )),
        }
    }

    /// [`KokoroFiles::find`] in the folders of this PC ([`SearchDirs::of_this_pc`]).
    pub fn locate() -> std::result::Result<KokoroFiles, String> {
        Self::find(&SearchDirs::of_this_pc())
    }
}

/// What a load gives: the model and the voice styles.
pub type Model = (Box<dyn Acoustic>, Styles);

/// Builds the model from the files (tests pass a fake).
pub type Loader = Arc<dyn Fn(&KokoroFiles) -> std::result::Result<Model, String> + Send + Sync>;

/// The real loader: ONNX Runtime from `files.runtime`, the model, and the styles of the English voices.
pub fn load_onnx(files: &KokoroFiles) -> std::result::Result<Model, String> {
    let env = model::init_runtime(&files.runtime)?;
    let model = model::OrtModel::load(env, &files.model).map_err(|e| e.to_string())?;
    let bytes =
        std::fs::read(&files.voices).map_err(|e| format!("cannot read the Kokoro voices: {e}"))?;
    let styles = Styles::parse(&bytes, ENGLISH_VOICES).map_err(|e| e.to_string())?;
    Ok((Box::new(model), styles))
}

struct Loaded {
    model: Mutex<Box<dyn Acoustic>>,
    styles: Styles,
}

/// Where the model load is.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum LoadState {
    /// Not tried yet, or still loading.
    #[default]
    Pending,
    Ready,
    /// The load failed (the raw error, for the log). It is not tried again.
    Failed(String),
}

/// The load state of one engine and its clones. Its lock is never held during a load, so the UI thread can read
/// it at any time ([`KokoroTts::is_loaded`] waits for a running load).
#[derive(Clone, Debug, Default)]
pub struct LoadWatch(Arc<Mutex<LoadState>>);

impl LoadWatch {
    pub fn get(&self) -> LoadState {
        self.0.lock().map(|s| s.clone()).unwrap_or_default()
    }

    pub fn set(&self, state: LoadState) {
        if let Ok(mut s) = self.0.lock() {
            *s = state;
        }
    }
}

struct Shared {
    files: KokoroFiles,
    loader: Loader,
    watch: LoadWatch,
    /// `None` until the first load; then the model or why it could not load (a failed load is not tried again:
    /// the files do not change while the app runs).
    loaded: Mutex<Option<std::result::Result<Arc<Loaded>, String>>>,
}

/// The phonemizer, built once per process (it decodes the lexicons, about a second).
fn phonemizer() -> &'static Phonemizer {
    static P: OnceLock<Phonemizer> = OnceLock::new();
    P.get_or_init(Phonemizer::new)
}

/// The Kokoro engine. Clones share one model.
#[derive(Clone)]
pub struct KokoroTts {
    shared: Arc<Shared>,
}

impl KokoroTts {
    /// The engine for `files`, with the real model. Nothing is loaded yet.
    pub fn new(files: KokoroFiles) -> KokoroTts {
        KokoroTts::with_loader(files, Arc::new(load_onnx))
    }

    pub fn with_loader(files: KokoroFiles, loader: Loader) -> KokoroTts {
        KokoroTts {
            shared: Arc::new(Shared {
                files,
                loader,
                watch: LoadWatch::default(),
                loaded: Mutex::new(None),
            }),
        }
    }

    pub fn files(&self) -> &KokoroFiles {
        &self.shared.files
    }

    /// Loads the model now if it is not loaded (blocks; a `synth` meanwhile waits for it). Also builds the
    /// phonemizer.
    pub fn load(&self) -> std::result::Result<(), String> {
        self.loaded().map(|_| ())
    }

    /// Loads the model on its own thread, so the first line does not wait for it.
    pub fn preload(&self) {
        let me = self.clone();
        let _ = std::thread::Builder::new()
            .name("kokoro-load".into())
            .spawn(move || {
                let _ = me.load();
            });
    }

    /// The load state, readable from any thread without waiting for a running load.
    pub fn watch(&self) -> LoadWatch {
        self.shared.watch.clone()
    }

    /// The model has loaded (waits while a load runs: not for the UI thread, see [`KokoroTts::watch`]).
    pub fn is_loaded(&self) -> bool {
        self.shared
            .loaded
            .lock()
            .is_ok_and(|l| matches!(*l, Some(Ok(_))))
    }

    fn loaded(&self) -> std::result::Result<Arc<Loaded>, String> {
        let mut slot = self.shared.loaded.lock().unwrap_or_else(|e| e.into_inner());
        if slot.is_none() {
            let _ = phonemizer();
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                (self.shared.loader)(&self.shared.files)
            }))
            .unwrap_or_else(|_| Err("loading the Kokoro model panicked".into()));
            self.shared.watch.set(match &result {
                Ok(_) => LoadState::Ready,
                Err(e) => LoadState::Failed(e.clone()),
            });
            *slot = Some(result.map(|(model, styles)| {
                Arc::new(Loaded {
                    model: Mutex::new(model),
                    styles,
                })
            }));
        }
        slot.clone().expect("set above")
    }

    /// The Kokoro voice for an id ("af_heart", also "kokoro:af_heart"); empty means [`DEFAULT_VOICE`].
    pub fn voice_id(wanted: &str) -> std::result::Result<&'static str, String> {
        if wanted.is_empty() {
            return Ok(DEFAULT_VOICE);
        }
        let w = wanted.strip_prefix("kokoro:").unwrap_or(wanted);
        ENGLISH_VOICES
            .iter()
            .find(|v| v.eq_ignore_ascii_case(w))
            .copied()
            .ok_or_else(|| format!("Kokoro: unknown voice '{wanted}'"))
    }
}

/// Game text before the phonemes (Sonara's text rules are made for developer text):
/// - a line with no lower-case letter ("WE NEED TO GO", a common game style) is read as words, not spelled
///   letter by letter: it is lower-cased;
/// - an Irish "O'" name ("O'Driscoll") is read as "O Driscoll", not spelled;
/// - a capital speaker name before a colon ("CIRI: Go.") is read as a name ("Ciri: Go.").
pub fn game_text(text: &str) -> String {
    // "O'" + a capital letter, at the start of a word ("o'clock" stays)
    let mut t = text.to_string();
    for apostrophe in ["'", "\u{2019}"] {
        let mut out = String::with_capacity(t.len());
        let mut rest = t.as_str();
        while let Some(i) = rest.find(apostrophe) {
            let (head, tail) = rest.split_at(i);
            let after = &tail[apostrophe.len()..];
            let mut back = head.chars().rev();
            let o_name = back.next() == Some('O')
                && back.next().is_none_or(|c| !c.is_alphanumeric())
                && after.chars().next().is_some_and(char::is_uppercase);
            out.push_str(head);
            out.push_str(if o_name { " " } else { apostrophe });
            rest = after;
        }
        out.push_str(rest);
        t = out;
    }
    let words = t
        .split(|c: char| !c.is_alphabetic())
        .filter(|w| w.chars().count() >= 2)
        .count();
    if words >= 2 && !t.chars().any(char::is_lowercase) {
        t = t.to_lowercase();
    }
    speaker_as_name(&t)
}

/// A capital speaker name before a colon at the start of a line ("CIRI: Go.", "JOHN:") is written as a name
/// ("Ciri: Go."), so a short name the lexicon does not know is read as a word, not spelled letter by letter. A
/// word of fewer than 3 letters or with no vowel ("AI:", "NPC:") stays as it is: it is likely an acronym.
fn speaker_as_name(text: &str) -> String {
    let Some((head, rest)) = text.split_once(':') else {
        return text.to_string();
    };
    let is_label = head.chars().any(char::is_alphabetic)
        && head.chars().count() <= 40
        && head
            .chars()
            .all(|c| c.is_uppercase() || matches!(c, ' ' | '.' | '\'' | '\u{2019}' | '-'));
    if !is_label {
        return text.to_string();
    }
    let mut out = String::with_capacity(text.len());
    let mut word = String::new();
    let flush = |word: &mut String, out: &mut String| {
        let vowel = word.chars().any(|c| "AEIOUY".contains(c));
        let mut c = word.chars();
        match c.next() {
            Some(first) if vowel && word.chars().count() >= 3 => {
                out.push(first);
                out.push_str(&c.as_str().to_lowercase());
            }
            _ => out.push_str(word),
        }
        word.clear();
    };
    for c in head.chars() {
        if c.is_alphabetic() {
            word.push(c);
        } else {
            flush(&mut word, &mut out);
            out.push(c);
        }
    }
    flush(&mut word, &mut out);
    out.push(':');
    out.push_str(rest);
    out
}

/// "af_heart" -> "Heart".
pub fn short_name(id: &str) -> String {
    let name = id.split_once('_').map_or(id, |(_, n)| n);
    let mut c = name.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

/// The voice of a Kokoro id: the second letter says female or male, the first US or UK English.
pub fn voice_of(id: &str) -> Voice {
    Voice {
        id: id.to_string(),
        name: short_name(id),
        lang: voices::language(id).to_string(),
        gender: match id.as_bytes().get(1) {
            Some(b'f') => Some(Gender::Female),
            Some(b'm') => Some(Gender::Male),
            _ => None,
        },
        engine: ENGINE,
    }
}

impl Tts for KokoroTts {
    /// Every English voice, in Kokoro's own order (best rated first). No model is needed for the list.
    fn voices(&self) -> Vec<Voice> {
        ENGLISH_VOICES.iter().map(|id| voice_of(id)).collect()
    }

    fn synth(&mut self, text: &str, voice: &str, speed: f32) -> std::result::Result<Pcm, String> {
        let voice = Self::voice_id(voice)?;
        let speed = speed.clamp(SPEED_MIN, SPEED_MAX);
        let loaded = self.loaded()?;
        let ps = phonemizer().phonemize(&game_text(text));
        let gap = vec![0i16; (SAMPLE_RATE * SENTENCE_GAP_MS / 1000) as usize];
        let mut samples: Vec<i16> = Vec::new();
        for batch in phonemes::batches(&ps) {
            let infer = |ps: &str| -> std::result::Result<Vec<f32>, String> {
                let tokens = phonemes::tokens(ps);
                let style = loaded
                    .styles
                    .style(voice, tokens.len())
                    .ok_or_else(|| format!("Kokoro: no style for voice '{voice}'"))?;
                let mut model = loaded.model.lock().unwrap_or_else(|e| e.into_inner());
                model
                    .infer(&tokens, style, speed)
                    .map_err(|e| e.to_string())
            };
            // A short question is heard after a lead-in sentence that is cut off again (Sonara's lead_in);
            // without a clean cut it is spoken on its own.
            let cropped = if lead_in::wants_lead_in(&batch) {
                infer(&lead_in::with_lead_in(&batch))
                    .ok()
                    .and_then(|a| lead_in::crop_lead_in(trim(&a), SAMPLE_RATE))
            } else {
                None
            };
            let audio = match cropped {
                Some(a) => a,
                None => trim(&infer(&batch)?).to_vec(),
            };
            if !samples.is_empty() {
                samples.extend_from_slice(&gap);
            }
            samples.extend(to_i16(&normalize_rms(&audio)));
        }
        Ok(Pcm {
            rate: SAMPLE_RATE,
            channels: 1,
            samples,
        })
    }
}

/// Trim leading and trailing silence (60 dB below the loudest 2048-sample frame, hop 512), as kokoro-onnx.
/// From Sonara (`kokoro/mod.rs`).
pub fn trim(a: &[f32]) -> &[f32] {
    let (fl, hop) = (2048usize, 512usize);
    if a.len() < fl {
        return a;
    }
    let frames: Vec<f32> = (0..=(a.len() - fl) / hop)
        .map(|i| {
            let f = &a[i * hop..i * hop + fl];
            (f.iter().map(|x| x * x).sum::<f32>() / fl as f32).sqrt()
        })
        .collect();
    let peak = frames.iter().cloned().fold(0.0f32, f32::max);
    if peak <= 0.0 {
        return a;
    }
    let thr = peak * 10f32.powf(-60.0 / 20.0);
    let first = frames.iter().position(|&r| r > thr).unwrap_or(0);
    let last = frames
        .iter()
        .rposition(|&r| r > thr)
        .unwrap_or(frames.len() - 1);
    let s = first * hop;
    let e = ((last + 1) * hop + fl).min(a.len());
    &a[s..e]
}

/// Voiced RMS to 0.08, peak at most 0.97, so every voice plays at about the same loudness. From Sonara
/// (`kokoro/mod.rs`).
pub fn normalize_rms(x: &[f32]) -> Vec<f32> {
    let frame = 480;
    if x.len() < frame {
        return x.to_vec();
    }
    let rms: Vec<f32> = x
        .chunks_exact(frame)
        .map(|f| (f.iter().map(|v| v * v).sum::<f32>() / frame as f32).sqrt())
        .collect();
    let gate = (rms.iter().cloned().fold(0.0f32, f32::max) * 0.1).max(1e-4);
    let voiced: Vec<f32> = rms.into_iter().filter(|r| *r > gate).collect();
    if voiced.is_empty() {
        return x.to_vec();
    }
    let level = voiced.iter().sum::<f32>() / voiced.len() as f32;
    let mut g = 0.08 / level;
    let pk = x.iter().fold(0.0f32, |m, v| m.max(v.abs()));
    if pk * g > 0.97 {
        g = 0.97 / pk;
    }
    x.iter().map(|v| v * g).collect()
}

fn to_i16(x: &[f32]) -> Vec<i16> {
    x.iter()
        .map(|s| (s.clamp(-1.0, 1.0) * 32767.0) as i16)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A model that makes a tone of 1000 samples per token, shorter when faster, and logs the speeds.
    struct Tone(Arc<Mutex<Vec<f32>>>);

    impl Acoustic for Tone {
        fn infer(&mut self, tokens: &[i64], style: &[f32], speed: f32) -> Result<Vec<f32>> {
            assert_eq!(style.len(), voices::STYLE_DIM);
            self.0.lock().unwrap().push(speed);
            let n = (tokens.len() as f32 * 1000.0 / speed) as usize;
            Ok((0..n).map(|i| (i as f32 * 0.07).sin() * 0.3).collect())
        }
    }

    fn files() -> KokoroFiles {
        KokoroFiles {
            model: "m.onnx".into(),
            voices: "v.bin".into(),
            runtime: "ort.dll".into(),
        }
    }

    /// An engine with the tone model and styles for af_heart and bm_george only.
    fn engine(speeds: Arc<Mutex<Vec<f32>>>, loads: Arc<Mutex<u32>>) -> KokoroTts {
        let loader: Loader = Arc::new(move |_| {
            *loads.lock().unwrap() += 1;
            let style = voices::tests::npy(&vec![0.1; voices::STYLE_ROWS * voices::STYLE_DIM]);
            let npz =
                voices::tests::zip(&[("af_heart.npy", style.clone()), ("bm_george.npy", style)]);
            let styles =
                Styles::parse(&npz, &["af_heart", "bm_george"]).map_err(|e| e.to_string())?;
            Ok((Box::new(Tone(speeds.clone())) as Box<dyn Acoustic>, styles))
        });
        KokoroTts::with_loader(files(), loader)
    }

    #[test]
    fn voices_have_names_genders_and_accents() {
        let k = engine(Default::default(), Default::default());
        let v = k.voices();
        assert_eq!(v.len(), 28);
        assert_eq!(v[0], voice_of("af_heart"));
        assert_eq!(
            (
                v[0].name.as_str(),
                v[0].lang.as_str(),
                v[0].gender,
                v[0].engine
            ),
            ("Heart", "en-US", Some(Gender::Female), "kokoro")
        );
        let george = voice_of("bm_george");
        assert_eq!(
            (george.name.as_str(), george.lang.as_str(), george.gender),
            ("George", "en-GB", Some(Gender::Male))
        );
        assert!(v.iter().any(|v| v.id == "am_adam" && v.name == "Adam"));
        // the list needs no model
        assert!(!k.is_loaded());
    }

    #[test]
    fn voice_ids_with_or_without_the_engine_prefix() {
        assert_eq!(KokoroTts::voice_id(""), Ok("af_heart"));
        assert_eq!(KokoroTts::voice_id("kokoro:bm_george"), Ok("bm_george"));
        assert_eq!(KokoroTts::voice_id("AF_Sarah"), Ok("af_sarah"));
        assert!(KokoroTts::voice_id("zf_xiaobei").is_err());
    }

    #[test]
    fn game_text_reads_capitals_and_o_names_as_words() {
        assert_eq!(game_text("WE NEED TO GO!"), "we need to go!");
        assert_eq!(game_text("PRESS E TO MOUNT"), "press e to mount");
        // one capital word or a mixed line stays as it is (an acronym is still spelled)
        assert_eq!(game_text("OK"), "OK");
        assert_eq!(game_text("Press LB to run."), "Press LB to run.");
        // a capital speaker name is read as a name; an acronym speaker is still spelled
        assert_eq!(game_text("DUTCH: We need to go."), "Dutch: We need to go.");
        assert_eq!(game_text("JOHN:"), "John:");
        assert_eq!(game_text("CIRI: Go."), "Ciri: Go.");
        assert_eq!(game_text("NPC: Go."), "NPC: Go.");
        assert_eq!(game_text("AI: Hello."), "AI: Hello.");
        assert_eq!(game_text("Chapter 2: Colter."), "Chapter 2: Colter.");
        assert_eq!(game_text("Note: PRESS LB"), "Note: PRESS LB");
        assert_eq!(
            game_text("Wait, the O'Driscoll boys?"),
            "Wait, the O Driscoll boys?"
        );
        assert_eq!(game_text("THE O\u{2019}DRISCOLLS"), "the o driscolls");
        assert_eq!(
            game_text("Who'd say don't at six o'clock?"),
            "Who'd say don't at six o'clock?"
        );
        // the phonemes of a capital line are words, not letters; so are those of a capital speaker name
        let p = phonemizer();
        assert_eq!(
            p.phonemize(&game_text("ZOE: We need to go.")),
            p.phonemize("Zoe: We need to go.")
        );
        assert_ne!(
            p.phonemize("ZOE: We need to go."),
            p.phonemize("Zoe: We need to go."),
            "without the rule ZOE is spelled"
        );
        assert_eq!(
            p.phonemize(&game_text("WE NEED TO GO")),
            p.phonemize("we need to go")
        );
    }

    #[test]
    fn loads_once_and_synth_follows_text_and_speed() {
        let speeds = Arc::new(Mutex::new(Vec::new()));
        let loads = Arc::new(Mutex::new(0));
        let mut k = engine(speeds.clone(), loads.clone());
        let mut other = k.clone();
        let short = k.synth("Hello there.", "kokoro:af_heart", 1.0).unwrap();
        assert_eq!((short.rate, short.channels), (24_000, 1));
        assert!(short.samples.iter().any(|&s| s != 0));
        let long = other
            .synth(
                "Hello there. We need to get these horses moving.",
                "bm_george",
                1.0,
            )
            .unwrap();
        assert!(long.duration_ms() > short.duration_ms() * 2.0);
        // two sentences: one gap of silence between them (the tone model never makes 6000 zeros in a row)
        let gap = (SAMPLE_RATE * SENTENCE_GAP_MS / 1000) as usize;
        let zeros = |p: &Pcm| {
            p.samples
                .split(|&s| s != 0)
                .map(<[i16]>::len)
                .max()
                .unwrap_or(0)
        };
        assert!((gap..gap + 3).contains(&zeros(&long)), "{}", zeros(&long));
        assert!(zeros(&short) < gap);
        let fast = k.synth("Hello there.", "", 9.0).unwrap();
        assert!(fast.duration_ms() < short.duration_ms() * 0.6);
        assert_eq!(*loads.lock().unwrap(), 1, "clones share one model");
        assert_eq!(other.watch().get(), LoadState::Ready);
        assert!(
            speeds.lock().unwrap().contains(&SPEED_MAX),
            "speed is clamped"
        );
        // a voice with no style is an error, not a crash
        assert!(k.synth("Hello.", "af_bella", 1.0).is_err());
        assert!(k.synth("Hello.", "nobody", 1.0).is_err());
    }

    #[test]
    fn a_failed_load_is_an_error_every_time() {
        let mut k = KokoroTts::with_loader(files(), Arc::new(|_| Err("no model".to_string())));
        let watch = k.clone().watch();
        assert_eq!(watch.get(), LoadState::Pending);
        assert_eq!(k.load(), Err("no model".to_string()));
        assert_eq!(watch.get(), LoadState::Failed("no model".into()));
        assert_eq!(k.synth("Hi.", "", 1.0).unwrap_err(), "no model");
        assert!(!k.is_loaded());
    }

    #[test]
    fn find_takes_the_first_folder_with_both_files() {
        let root = std::env::temp_dir().join(format!("dr-tts-kokoro-find-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let (a, b, c, rt) = (
            root.join("a"),
            root.join("b"),
            root.join("c"),
            root.join("rt"),
        );
        for d in [&a, &b, &c, &rt] {
            std::fs::create_dir_all(d).unwrap();
        }
        // a: model only; b: both, voices broken (wrong size); c: both
        std::fs::write(a.join(MODEL_FILE), [0u8; 3]).unwrap();
        std::fs::write(b.join(MODEL_FILE), [0u8; 3]).unwrap();
        std::fs::write(b.join(VOICES_FILE), [0u8; 1]).unwrap();
        std::fs::write(c.join(MODEL_FILE), [0u8; 3]).unwrap();
        std::fs::write(c.join(VOICES_FILE), [0u8; 2]).unwrap();
        let mut dirs = SearchDirs {
            model: vec![root.join("missing"), a, b, c.clone()],
            runtime: vec![root.join("missing"), rt.clone()],
        };
        let err = KokoroFiles::find_with_sizes(&dirs, 3, 2).unwrap_err();
        assert!(err.contains(RUNTIME_FILE), "{err}");
        std::fs::write(rt.join(RUNTIME_FILE), b"x").unwrap();
        let f = KokoroFiles::find_with_sizes(&dirs, 3, 2).unwrap();
        assert_eq!(
            f,
            KokoroFiles {
                model: c.join(MODEL_FILE),
                voices: c.join(VOICES_FILE),
                runtime: rt.join(RUNTIME_FILE),
            }
        );
        dirs.model.pop();
        assert!(
            KokoroFiles::find_with_sizes(&dirs, 3, 2)
                .unwrap_err()
                .contains(MODEL_FILE)
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn sonara_folders_only_in_debug_or_when_asked() {
        assert!(sonara_fallback(true, None));
        assert!(!sonara_fallback(false, None));
        assert!(!sonara_fallback(false, Some("0")));
        assert!(!sonara_fallback(false, Some("")));
        assert!(sonara_fallback(false, Some("1")));
    }

    #[test]
    fn runtime_folders_newest_version_first() {
        let root = std::env::temp_dir().join(format!("dr-tts-kokoro-rt-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        for v in ["0.9.1", "0.38.3", "0.10.0"] {
            std::fs::create_dir_all(root.join(v)).unwrap();
        }
        let names: Vec<String> = newest_first(&root)
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, ["0.38.3", "0.10.0", "0.9.1"]);
        assert!(newest_first(&root.join("none")).is_empty());
        let _ = std::fs::remove_dir_all(&root);
    }
}
