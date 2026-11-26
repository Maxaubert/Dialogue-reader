//! dr-tts: text to speech for DialogReader (docs/app-spec.md 2.5).
//!
//! - [`Tts`]: the engine interface. An engine lists its voices and turns text into PCM. It never plays sound:
//!   playback, volume, stop and mute belong to the audio sink in `dr-runtime` (`speech.rs`).
//! - [`KokoroTts`]: Kokoro voices, the main engine when its files are on the PC (ships with the app: docs/app-spec.md section 6). See [`kokoro`].
//! - [`WinTts`]: the voices built into Windows (`Windows.Media.SpeechSynthesis.SpeechSynthesizer`), the backup
//!   when Kokoro's files are missing.
//! - [`Engines`]: one engine over both; profile voice ids carry the engine (`kokoro:af_heart`, `win:<id>`).
//! - [`FakeTts`]: a test engine that makes silent PCM whose length follows the text and the speed.
//! - [`wav::parse`]: WAV bytes to [`Pcm`].

pub mod engines;
pub mod fake;
pub mod kokoro;
pub mod wav;
#[cfg(windows)]
mod win;

pub use engines::Engines;
pub use fake::FakeTts;
pub use kokoro::{KokoroFiles, KokoroTts};
#[cfg(windows)]
pub use win::WinTts;

/// Voice gender, when the engine says.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gender {
    Male,
    Female,
}

/// One voice of an engine.
#[derive(Clone, Debug, PartialEq)]
pub struct Voice {
    /// Engine-specific id, saved in the profile (`voice.id`).
    pub id: String,
    /// Short name for the UI, for example "David".
    pub name: String,
    /// BCP 47 language tag, for example "en-US".
    pub lang: String,
    pub gender: Option<Gender>,
    /// Engine name as saved in the profile (`voice.engine`): "kokoro" for [`KokoroTts`], "windows" for [`WinTts`].
    pub engine: &'static str,
}

/// 16-bit PCM, samples interleaved by channel.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Pcm {
    pub rate: u32,
    pub channels: u16,
    pub samples: Vec<i16>,
}

impl Pcm {
    /// Length in milliseconds.
    pub fn duration_ms(&self) -> f64 {
        if self.rate == 0 || self.channels == 0 {
            return 0.0;
        }
        self.samples.len() as f64 / self.channels as f64 / self.rate as f64 * 1000.0
    }
}

/// Slowest and fastest speed the app offers (spec 2.8: 0.5 to 2.0).
pub const SPEED_MIN: f32 = 0.5;
pub const SPEED_MAX: f32 = 2.0;

/// A text-to-speech engine.
pub trait Tts: Send {
    /// The voices this engine can use now.
    fn voices(&self) -> Vec<Voice>;
    /// Speaks `text` with the voice `voice` (an id from [`Tts::voices`]; empty or unknown means the engine's
    /// default voice) at `speed` (1.0 is normal) into PCM.
    fn synth(&mut self, text: &str, voice: &str, speed: f32) -> Result<Pcm, String>;
}

impl<T: Tts + ?Sized> Tts for Box<T> {
    fn voices(&self) -> Vec<Voice> {
        (**self).voices()
    }
    fn synth(&mut self, text: &str, voice: &str, speed: f32) -> Result<Pcm, String> {
        (**self).synth(text, voice, speed)
    }
}
