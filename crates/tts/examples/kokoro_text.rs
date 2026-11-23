//! Prints what Kokoro's text rules and phonemes make of game lines (no model, no sound).
//! `cargo run -p dr-tts --example kokoro_text`
fn main() {
    use dr_tts::kokoro::{game_text, phonemes::Phonemizer, text};
    let p = Phonemizer::new();
    for l in [
        "WE NEED TO GO",
        "PRESS E TO MOUNT",
        "A well-known, half-dead outlaw.",
        "Mr. Morgan owes me $50.",
        "Wait... the O'Driscoll boys?",
        "Arthur: We need to get these horses moving before the snow buries us.",
        "DUTCH: We need to go.",
        "Dutch: We need to go.",
        "JOHN:",
        "JOHN: Arthur, wait.",
        "John: Arthur, wait.",
        "SADIE: Move!",
        "CIRI: Go.",
        "ZOE: Go.",
        "NPC: Go.",
        "Press LB to run.",
        "Chapter 2: Colter. Press [E] / LB to continue.",
        "Driscoll",
        "driscoll",
        "O'Driscoll",
        "Dutch van der Linde",
        "Valentine",
        "Strawberry",
        "Saint Denis",
        "Kvothe",
        "Geralt of Rivia",
    ] {
        // what KokoroTts::synth does: game_text first
        let g = game_text(l);
        println!(
            "{l}\n  text: {}\n  ps:   {}",
            text::normalize(&g),
            p.phonemize(&g)
        );
    }
}
