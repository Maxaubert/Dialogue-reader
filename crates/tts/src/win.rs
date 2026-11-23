//! The voices built into Windows (`Windows.Media.SpeechSynthesis`). They need no download and run on the PC.
//!
//! `SynthesizeTextToStreamAsync` gives a WAV stream in memory; it is read into bytes and parsed to PCM. Nothing is
//! played here. Speed goes to the engine (`SpeakingRate`), which sounds better than resampling.

use windows::Media::SpeechSynthesis::{SpeechSynthesizer, VoiceGender, VoiceInformation};
use windows::Storage::Streams::DataReader;
use windows::core::HSTRING;

use crate::{Gender, Pcm, SPEED_MAX, SPEED_MIN, Tts, Voice};

/// Engine name saved in the profile.
pub const ENGINE: &str = "windows";

/// The synthesizer is made at the first `synth` call, so listing voices works even where Windows cannot start
/// one (no voice installed). The voice picked last is kept, so a line with the same voice does not list the
/// voices again.
#[derive(Default)]
pub struct WinTts {
    synth: Option<SpeechSynthesizer>,
    /// The voice id asked for last, and the voice set on `synth` for it.
    voice: Option<(String, VoiceInformation)>,
}

fn err(what: &str) -> impl Fn(windows::core::Error) -> String + '_ {
    move |e| format!("Windows voices: {what}: {}", e.message())
}

/// "Microsoft David" -> "David" (the engine name shows in front of it in the UI).
pub fn short_name(display: &str) -> String {
    let s = display.trim();
    let s = s.strip_prefix("Microsoft ").unwrap_or(s);
    let s = s.strip_suffix(" Desktop").unwrap_or(s);
    s.trim().to_string()
}

fn voice_of(v: &VoiceInformation) -> Option<Voice> {
    Some(Voice {
        id: v.Id().ok()?.to_string(),
        name: short_name(&v.DisplayName().ok()?.to_string()),
        lang: v.Language().ok()?.to_string(),
        gender: match v.Gender().ok()? {
            VoiceGender::Male => Some(Gender::Male),
            VoiceGender::Female => Some(Gender::Female),
            _ => None,
        },
        engine: ENGINE,
    })
}

fn all_voices() -> windows::core::Result<Vec<VoiceInformation>> {
    let list = SpeechSynthesizer::AllVoices()?;
    Ok((0..list.Size()?)
        .filter_map(|i| list.GetAt(i).ok())
        .collect())
}

/// Error 0x80070002 (file not found) from `DefaultVoice` and `SpeechSynthesizer::new`: Windows has no voice
/// installed (`AllVoices` then gives an empty list, not this error).
const NOT_FOUND: windows::core::HRESULT = windows::core::HRESULT(0x8007_0002_u32 as i32);

fn plain(e: &windows::core::Error) -> String {
    if e.code() == NOT_FOUND {
        "Windows voices: no voice is installed".into()
    } else {
        format!("Windows voices: {}", e.message())
    }
}

impl WinTts {
    pub fn new() -> WinTts {
        WinTts::default()
    }

    /// The installed voices (an empty list when none is installed), or the error Windows gave when it could not
    /// list them (`Tts::voices` gives an empty list then).
    pub fn probe() -> Result<Vec<Voice>, String> {
        Ok(all_voices()
            .map_err(|e| plain(&e))?
            .iter()
            .filter_map(voice_of)
            .collect())
    }

    /// The voice Windows uses when none is picked (`DefaultVoice`), if any.
    pub fn default_voice() -> Option<Voice> {
        voice_of(&SpeechSynthesizer::DefaultVoice().ok()?)
    }
}

impl Tts for WinTts {
    fn voices(&self) -> Vec<Voice> {
        all_voices()
            .unwrap_or_default()
            .iter()
            .filter_map(voice_of)
            .collect()
    }

    fn synth(&mut self, text: &str, voice: &str, speed: f32) -> Result<Pcm, String> {
        if self.synth.is_none() {
            self.synth = Some(SpeechSynthesizer::new().map_err(|e| plain(&e))?);
        }
        let synth = self.synth.as_ref().expect("made above");
        if self.voice.as_ref().is_none_or(|(id, _)| id != voice) {
            // an empty or unknown id means the default voice, also after another voice was picked before
            let picked = all_voices()
                .map_err(|e| plain(&e))?
                .into_iter()
                .find(|v| !voice.is_empty() && v.Id().is_ok_and(|id| id == voice));
            let v = match picked {
                Some(v) => v,
                None => SpeechSynthesizer::DefaultVoice().map_err(|e| plain(&e))?,
            };
            self.voice = None;
            synth.SetVoice(&v).map_err(err("pick the voice"))?;
            self.voice = Some((voice.to_string(), v));
        }
        let rate = speed.clamp(SPEED_MIN, SPEED_MAX) as f64;
        synth
            .Options()
            .and_then(|o| o.SetSpeakingRate(rate))
            .map_err(err("set the speed"))?;
        let stream = synth
            .SynthesizeTextToStreamAsync(&HSTRING::from(text))
            .and_then(|op| op.join())
            .map_err(err("speak"))?;
        let size = stream.Size().map_err(err("read the sound"))?;
        let size =
            u32::try_from(size).map_err(|_| "Windows voices: the sound is too long".to_string())?;
        let input = stream.GetInputStreamAt(0).map_err(err("read the sound"))?;
        let reader = DataReader::CreateDataReader(&input).map_err(err("read the sound"))?;
        let got = reader
            .LoadAsync(size)
            .and_then(|op| op.join())
            .map_err(err("read the sound"))?;
        let mut bytes = vec![0u8; got as usize];
        reader
            .ReadBytes(&mut bytes)
            .map_err(err("read the sound"))?;
        crate::wav::parse(&bytes).map_err(|e| format!("Windows voices: {e}"))
    }
}

#[cfg(test)]
mod tests {
    use super::short_name;

    #[test]
    fn short_names() {
        assert_eq!(short_name("Microsoft David"), "David");
        assert_eq!(short_name("Microsoft Zira Desktop"), "Zira");
        assert_eq!(short_name("Some Voice"), "Some Voice");
    }
}
