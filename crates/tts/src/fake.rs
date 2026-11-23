//! A fake engine for tests: silent PCM, 60 ms per character at speed 1, and a log of every call.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::{Gender, Pcm, Tts, Voice};

/// Sample rate of the fake PCM.
pub const RATE: u32 = 16_000;
/// Length of one character at speed 1.
pub const MS_PER_CHAR: f64 = 60.0;

/// One `synth` call: text, voice id as asked, speed.
#[derive(Clone, Debug, PartialEq)]
pub struct Call {
    pub text: String,
    pub voice: String,
    pub speed: f32,
}

#[derive(Clone, Default)]
pub struct FakeTts {
    voices: Vec<Voice>,
    calls: Arc<Mutex<Vec<Call>>>,
    fail: Arc<Mutex<Option<String>>>,
    delay: Arc<Mutex<Duration>>,
}

impl FakeTts {
    /// Two English voices: "fake-david" and "fake-zira".
    pub fn new() -> FakeTts {
        FakeTts::with_voices(vec![
            Voice {
                id: "fake-david".into(),
                name: "David".into(),
                lang: "en-US".into(),
                gender: Some(Gender::Male),
                engine: "windows",
            },
            Voice {
                id: "fake-zira".into(),
                name: "Zira".into(),
                lang: "en-US".into(),
                gender: Some(Gender::Female),
                engine: "windows",
            },
        ])
    }

    pub fn with_voices(voices: Vec<Voice>) -> FakeTts {
        FakeTts {
            voices,
            ..FakeTts::default()
        }
    }

    /// The calls so far. The log is shared by clones, so a test keeps a clone and moves the engine.
    pub fn calls(&self) -> Vec<Call> {
        self.calls.lock().map(|c| c.clone()).unwrap_or_default()
    }

    /// Every later `synth` takes `delay` before it returns, as a real engine takes time (shared by clones).
    pub fn set_delay(&self, delay: Duration) {
        if let Ok(mut d) = self.delay.lock() {
            *d = delay;
        }
    }

    /// Every later `synth` fails with `error` (`None`: works again).
    pub fn fail_with(&self, error: Option<&str>) {
        if let Ok(mut f) = self.fail.lock() {
            *f = error.map(str::to_string);
        }
    }
}

impl Tts for FakeTts {
    fn voices(&self) -> Vec<Voice> {
        self.voices.clone()
    }

    fn synth(&mut self, text: &str, voice: &str, speed: f32) -> Result<Pcm, String> {
        if let Ok(mut c) = self.calls.lock() {
            c.push(Call {
                text: text.into(),
                voice: voice.into(),
                speed,
            });
        }
        let delay = self.delay.lock().map(|d| *d).unwrap_or_default();
        if !delay.is_zero() {
            std::thread::sleep(delay);
        }
        if let Some(e) = self.fail.lock().ok().and_then(|f| f.clone()) {
            return Err(e);
        }
        let ms = text.chars().count() as f64 * MS_PER_CHAR / speed.max(0.1) as f64;
        let n = (ms / 1000.0 * RATE as f64).round() as usize;
        Ok(Pcm {
            rate: RATE,
            channels: 1,
            samples: vec![0; n],
        })
    }
}
