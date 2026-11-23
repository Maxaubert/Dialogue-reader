//! Short questions. Kokoro's model card lists inputs under about 10 to 20
//! tokens as a weakness: "Should I?" comes out flat and held, as if the
//! sentence went on. Given a short sentence before it the model ends the
//! question properly, so a short question is synthesized after a lead-in
//! sentence that is cropped off again at the pause between the two.
//!
//! From Sonara (MIT, `crates/sonara-engine/src/kokoro/lead_in.rs` at commit 8fd2af7).
use super::vocab;

/// The sentence spoken before a short question (Kokoro phonemes: "Okay.").
pub const LEAD_IN: &str = "ˈOkA.";

/// A batch with fewer model tokens than this is "short".
const SHORT_TOKENS: usize = 20;

/// Whether a batch is a short question.
pub fn wants_lead_in(batch: &str) -> bool {
    let batch = batch.trim_end();
    batch.ends_with('?') && batch.chars().filter(|c| vocab::id(*c).is_some()).count() < SHORT_TOKENS
}

/// The batch after the lead-in.
pub fn with_lead_in(batch: &str) -> String {
    format!("{LEAD_IN} {batch}")
}

/// `audio` (trimmed, mono, of `with_lead_in`) without the lead-in: what
/// follows the longest quiet gap that starts in the first 70 % of it. `None`
/// if there is no such gap, so the lead-in cannot be told from the question.
pub fn crop_lead_in(audio: &[f32], sample_rate: u32) -> Option<Vec<f32>> {
    let frame = (sample_rate as usize / 100).max(1);
    let rms: Vec<f32> = audio
        .chunks(frame)
        .map(|c| (c.iter().map(|x| x * x).sum::<f32>() / c.len() as f32).sqrt())
        .collect();
    let peak = rms.iter().cloned().fold(0.0f32, f32::max);
    if peak <= 0.0 {
        return None;
    }
    let quiet: Vec<bool> = rms.iter().map(|r| *r < peak * 0.03).collect();
    let mut best = (0usize, 0usize);
    let mut i = 0;
    while i < quiet.len() {
        if !quiet[i] {
            i += 1;
            continue;
        }
        let start = i;
        while i < quiet.len() && quiet[i] {
            i += 1;
        }
        if i - start > best.1 - best.0 && start < quiet.len() * 7 / 10 {
            best = (start, i);
        }
    }
    // At least 40 ms, and some speech after it.
    if best.1 - best.0 < 4 || best.1 >= quiet.len() {
        return None;
    }
    Some(audio[(best.1 * frame).min(audio.len())..].to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(seconds: f32) -> Vec<f32> {
        (0..(seconds * 24_000.0) as usize)
            .map(|i| (i as f32 / 24_000.0 * 150.0 * std::f32::consts::TAU).sin() * 0.5)
            .collect()
    }

    #[test]
    fn only_short_questions_get_a_lead_in() {
        assert!(wants_lead_in("ʃˌʊd ˈI?"));
        assert!(!wants_lead_in("ʃˌʊd ˈI."));
        assert!(!wants_lead_in("hˈɛlO!"));
        // Twenty tokens or more is long enough for the model.
        assert!(!wants_lead_in(&format!("{}?", "a".repeat(30))));
        assert_eq!(with_lead_in("ʃˌʊd ˈI?"), "ˈOkA. ʃˌʊd ˈI?");
    }

    #[test]
    fn the_lead_in_is_cropped_at_the_pause() {
        let mut a = tone(0.3);
        a.extend(vec![0.0; 6_000]);
        let question = tone(0.5);
        a.extend(&question);
        let out = crop_lead_in(&a, 24_000).unwrap();
        // Within one 10 ms frame of the question itself.
        assert!(out.len() <= question.len() + 240, "{}", out.len());
        assert!(out.len() >= question.len() - 240, "{}", out.len());
    }

    #[test]
    fn without_a_pause_nothing_is_cropped() {
        assert_eq!(crop_lead_in(&tone(0.8), 24_000), None);
        assert_eq!(crop_lead_in(&vec![0.0; 24_000], 24_000), None);
        // A pause with nothing after it is no lead-in either.
        let mut a = tone(0.3);
        a.extend(vec![0.0; 6_000]);
        assert_eq!(crop_lead_in(&a, 24_000), None);
    }
}
