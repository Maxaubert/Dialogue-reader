//! Prints what Windows says about its voices (WinRT `AllVoices`, `DefaultVoice`). Plays nothing.
#[cfg(windows)]
fn main() {
    use windows::Media::SpeechSynthesis::SpeechSynthesizer;
    match SpeechSynthesizer::AllVoices() {
        Ok(list) => println!("AllVoices: ok, {} voices", list.Size().unwrap_or(0)),
        Err(e) => println!("AllVoices: error {e:?}"),
    }
    match SpeechSynthesizer::DefaultVoice() {
        Ok(v) => println!("DefaultVoice: {:?}", v.DisplayName()),
        Err(e) => println!("DefaultVoice: error {e:?}"),
    }
    match SpeechSynthesizer::new() {
        Ok(_) => println!("new: ok"),
        Err(e) => println!("new: error {e:?}"),
    }
}
#[cfg(not(windows))]
fn main() {}
