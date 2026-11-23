//! Text to Kokoro tokens: Kokoro's text rules, misaki G2P with Sonara's own
//! words first and the permissive fallback, misaki-rs's symbols mapped to
//! Kokoro's, then batches the model can take.
//!
//! From Sonara (MIT, `crates/sonara-engine/src/kokoro/phonemes.rs` at commit 8fd2af7).
use super::{fallback::WordFallback, text, vocab};
use misaki::{G2P, Language, Lexicon};
use std::sync::Arc;

/// The most tokens one inference takes (Kokoro's context is 512 with the
/// two pad tokens).
pub const MAX_TOKENS: usize = 510;

/// Words misaki does not know or says wrong, as they come out of the text
/// rules (lower case answers its capitalized form too). misaki's US symbols:
/// `A` = eɪ, `I` = aɪ, `O` = oʊ, `ˈ` primary and `ˌ` secondary stress.
pub const CUSTOM_LEXICON: &[(&str, &str)] = &[
    ("sonara", "sənˈɑɹə"),
    ("sonarad", "sənˈɑɹə dˈi"),
    ("kokoro", "kˈOkəɹˌO"),
    ("onnx", "ˈɑnɪks"),
    ("onyx", "ˈɑnɪks"),
    ("claude", "klˈɔd"),
    ("codex", "kˈOdɛks"),
    ("misaki", "misˈɑki"),
    ("wasapi", "wəsˈɑpi"),
    ("cpal", "sˈipˌæl"),
    ("npm", "ˌɛnpˌiˈɛm"),
    ("config", "kˈɑnfɪɡ"),
    ("enum", "ˈinəm"),
    ("onecore", "wˈʌnkˌɔɹ"),
    ("toml", "tˈɑməl"),
    // Owner's report 2026-11-14: `url` in code and "Url" were said "earl",
    // `async` "uh-sink", and the prose rules' "ay sink" "eye sink".
    ("url", "jˌuˌɑɹˈɛl"),
    ("urls", "jˌuˌɑɹˈɛlz"),
    ("async", "ˈAsɪŋk"),
    ("ay", "ˈA"),
];

/// misaki-rs writes diphthongs and affricates with a zero-width joiner;
/// Kokoro's vocabulary has misaki's single symbols for them.
const JOINED: &[(&str, &str)] = &[
    ("a\u{200d}ɪ", "I"),
    ("e\u{200d}ɪ", "A"),
    ("o\u{200d}ʊ", "O"),
    ("a\u{200d}ʊ", "W"),
    ("ɔ\u{200d}ɪ", "Y"),
    ("t\u{200d}ʃ", "ʧ"),
    ("d\u{200d}ʒ", "ʤ"),
];

/// Map misaki-rs output to Kokoro symbols and tidy spaces around
/// punctuation.
pub fn to_kokoro_symbols(ps: &str) -> String {
    let mut ps = ps.to_string();
    for (joined, single) in JOINED {
        ps = ps.replace(joined, single);
    }
    let ps = ps.replace('\u{200d}', "");
    let mut out = String::with_capacity(ps.len());
    for c in ps.chars() {
        if c == ' ' && out.ends_with(' ') {
            continue;
        }
        if matches!(c, ',' | '.' | ':' | ';' | '?' | '!') && out.ends_with(' ') {
            out.pop();
        }
        out.push(c);
    }
    out.trim().to_string()
}

/// misaki leaves a weak word (a pronoun, an auxiliary) with only a secondary
/// stress, which is right mid-sentence. As the last word before `.`, `!` or
/// `?` it makes Kokoro sound as if more were coming ("should I?" like the
/// start of "should I try that?"), so its first secondary stress becomes
/// primary.
pub fn stress_sentence_end(ps: &str) -> String {
    let chars: Vec<char> = ps.chars().collect();
    let mut out = chars.clone();
    for (i, c) in chars.iter().enumerate() {
        if !matches!(c, '.' | '!' | '?') || chars.get(i + 1).is_some_and(|n| *n != ' ') {
            continue;
        }
        let start = chars[..i]
            .iter()
            .rposition(|c| *c == ' ')
            .map_or(0, |p| p + 1);
        let word = &chars[start..i];
        if word.contains(&'ˈ') {
            continue;
        }
        if let Some(p) = word.iter().position(|c| *c == 'ˌ') {
            out[start + p] = 'ˈ';
        }
    }
    out.into_iter().collect()
}

/// Kokoro drops an unstressed schwa that starts a sentence at Sonara's
/// default rate ("Agreed" is heard as "greed", "A real" as "real"), so one
/// there gets a primary stress. A schwa inside a sentence is left alone.
pub fn stress_sentence_start(ps: &str) -> String {
    let chars: Vec<char> = ps.chars().collect();
    let mut out = String::with_capacity(ps.len() + 4);
    for (i, c) in chars.iter().enumerate() {
        let starts =
            i == 0 || (i >= 2 && chars[i - 1] == ' ' && matches!(chars[i - 2], '.' | '!' | '?'));
        if starts && matches!(c, 'ɐ' | 'ə') {
            out.push('ˈ');
        }
        out.push(*c);
    }
    out
}

/// The G2P with Sonara's words and fallback. Building it decodes the
/// embedded lexicons (about a second); it is then shared.
pub struct Phonemizer {
    g2p: G2P,
}

impl Phonemizer {
    pub fn new() -> Self {
        let mut lexicon = Lexicon::new(Language::EnglishUS);
        for (word, phonemes) in CUSTOM_LEXICON {
            lexicon.insert_gold(word, phonemes);
        }
        let lexicon = Arc::new(lexicon);
        let fallback = Box::new(WordFallback::new(lexicon.clone()));
        Phonemizer {
            g2p: G2P::with_lexicon(lexicon, Some(fallback)),
        }
    }

    /// The Kokoro phoneme string for `text`.
    pub fn phonemize(&self, text: &str) -> String {
        let t = text::normalize(text);
        if t.is_empty() {
            return String::new();
        }
        match self.g2p.g2p(&t) {
            Ok((ps, _)) => stress_sentence_start(&stress_sentence_end(&to_kokoro_symbols(&ps))),
            // Our fallback never fails; keep the words rather than nothing.
            Err(_) => String::new(),
        }
    }
}

impl Default for Phonemizer {
    fn default() -> Self {
        Self::new()
    }
}

/// The token ids of a phoneme string (symbols the model lacks dropped).
pub fn tokens(ps: &str) -> Vec<i64> {
    ps.chars().filter_map(vocab::id).collect()
}

fn token_len(s: &str) -> usize {
    s.chars().filter(|c| vocab::id(*c).is_some()).count()
}

/// Split `s` after any char in `ends` that is followed by a space or the
/// end, keeping the char with the piece before it.
fn split_after(s: &str, ends: &[char]) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        cur.push(c);
        if ends.contains(&c) && chars.peek().is_none_or(|n| *n == ' ') {
            out.push(std::mem::take(&mut cur));
        }
    }
    if !cur.trim().is_empty() {
        out.push(cur);
    }
    out.into_iter()
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty())
        .collect()
}

/// Pack `pieces` (each at most `max`) into as few batches as fit.
fn pack(pieces: Vec<String>, max: usize) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for p in pieces {
        match out.last_mut() {
            Some(last) if token_len(last) + 1 + token_len(&p) <= max => {
                last.push(' ');
                last.push_str(&p);
            }
            _ => out.push(p),
        }
    }
    out
}

/// Cut one piece longer than `max` tokens: at clause marks, then at
/// spaces, then anywhere.
fn cut(piece: &str, max: usize) -> Vec<String> {
    if token_len(piece) <= max {
        return vec![piece.to_string()];
    }
    let mut parts = Vec::new();
    for clause in split_after(piece, &[',', ';', ':']) {
        if token_len(&clause) <= max {
            parts.push(clause);
            continue;
        }
        for word in clause.split(' ') {
            if token_len(word) <= max {
                parts.push(word.to_string());
                continue;
            }
            let mut cur = String::new();
            for c in word.chars() {
                if vocab::id(c).is_some() && token_len(&cur) == max {
                    parts.push(std::mem::take(&mut cur));
                }
                cur.push(c);
            }
            if !cur.is_empty() {
                parts.push(cur);
            }
        }
    }
    pack(parts, max)
}

/// The batches one synthesis runs, in order: one per sentence (so the first
/// sentence plays while the next is synthesized), a long sentence cut at
/// clause marks, then at spaces, so none exceeds `MAX_TOKENS`.
pub fn batches(ps: &str) -> Vec<String> {
    batches_of(ps, MAX_TOKENS)
}

pub(crate) fn batches_of(ps: &str, max: usize) -> Vec<String> {
    split_after(ps, &['.', '!', '?'])
        .into_iter()
        .flat_map(|sentence| cut(&sentence, max))
        .filter(|b| token_len(b) > 0)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_is_spelled_and_async_starts_like_ace_in_any_case() {
        // Owner's report 2026-11-14: "url" was said "earl", "async"
        // "uh-sink" and the prose rules' "ay sink" "eye sink".
        let p = Phonemizer::new();
        for text in [
            "Check the url field.",
            "Url parsing failed.",
            "The URL is wrong.",
        ] {
            assert!(
                p.phonemize(text).contains("jˌuˌɑɹˈɛl"),
                "{text}: {}",
                p.phonemize(text)
            );
        }
        assert!(p.phonemize("Two urls.").contains("jˌuˌɑɹˈɛlz"));
        for text in ["The async keyword.", "Async code.", "It is ay sink now."] {
            let ps = p.phonemize(text);
            assert!(
                ps.contains("ˈAs") || ps.contains("ˈA sˈɪŋk"),
                "{text}: {ps}"
            );
            assert!(
                !ps.contains("ɐsˈɪŋk") && !ps.contains("ˈI sˈɪŋk"),
                "{text}: {ps}"
            );
        }
    }

    #[test]
    fn joined_symbols_become_kokoro_symbols() {
        assert_eq!(
            to_kokoro_symbols("hˈa\u{200d}ʊs ,  t\u{200d}ʃˈe\u{200d}ɪn ."),
            "hˈWs, ʧˈAn."
        );
    }

    #[test]
    fn a_weak_last_word_is_stressed_so_a_short_question_sounds_finished() {
        // "should I?" has the same weak "I" as the start of "should I try that?".
        assert_eq!(stress_sentence_end("ʃˌʊd ˌI?"), "ʃˌʊd ˈI?");
        assert_eq!(
            stress_sentence_end("kˌæn ˌI. ʃˌʊd ˌI?"),
            "kˌæn ˈI. ʃˌʊd ˈI?"
        );
        // Mid-sentence words, and last words that already carry a primary
        // stress, are left alone.
        assert_eq!(
            stress_sentence_end("ʃˌʊd ˌI tɹˈI ðˈæt?"),
            "ʃˌʊd ˌI tɹˈI ðˈæt?"
        );
        assert_eq!(stress_sentence_end("ɪz ɪɾ?"), "ɪz ɪɾ?");
        assert_eq!(stress_sentence_end("ˈe.ɡ. ˌI"), "ˈe.ɡ. ˌI");
    }

    #[test]
    fn a_sentence_starting_with_a_weak_vowel_keeps_it_at_a_fast_rate() {
        // At Sonara's default rate (Kokoro speed 1.25) the model drops a
        // leading unstressed schwa: "Agreed" came out as "greed", "A real"
        // as "real". Stressed, it stays.
        assert_eq!(stress_sentence_start("ɐɡɹˈiːd, ðˈæt."), "ˈɐɡɹˈiːd, ðˈæt.");
        assert_eq!(
            stress_sentence_start("ɐ ɹˈiːəl pɹˈɑʤˌɛkt. ɐɡɹˈi! ə?"),
            "ˈɐ ɹˈiːəl pɹˈɑʤˌɛkt. ˈɐɡɹˈi! ˈə?"
        );
        // Other starts, a schwa inside a sentence and a stressed one stay.
        assert_eq!(stress_sentence_start("hˈɛlO ɐ kˈæt."), "hˈɛlO ɐ kˈæt.");
        assert_eq!(stress_sentence_start("ˈɐbaʊt. kˈæt."), "ˈɐbaʊt. kˈæt.");
        assert_eq!(stress_sentence_start(""), "");
    }

    #[test]
    fn one_batch_per_sentence() {
        assert_eq!(
            batches("hˈɛlO. wˈʌn, tˈu! θɹˈi? fˈɔɹ"),
            vec!["hˈɛlO.", "wˈʌn, tˈu!", "θɹˈi?", "fˈɔɹ"]
        );
        // A dot inside a word (a leftover abbreviation) does not split.
        assert_eq!(batches("ˈe.ɡ. sˈʌm"), vec!["ˈe.ɡ.", "sˈʌm"]);
        assert!(batches("").is_empty());
        assert!(batches(" . ").iter().all(|b| token_len(b) > 0));
    }

    #[test]
    fn a_long_sentence_is_cut_at_clauses_then_spaces() {
        let ps = "ab cd, ef gh, ij kl.";
        assert_eq!(batches_of(ps, 6), vec!["ab cd,", "ef gh,", "ij kl."]);
        assert_eq!(batches_of(ps, 13), vec!["ab cd, ef gh,", "ij kl."]);
        // ASCII g is not a Kokoro symbol (IPA ɡ is), so it does not count.
        assert_eq!(batches_of("abcdefhk ij", 3), vec!["abc", "def", "hk", "ij"]);
        assert_eq!(batches_of("abcgdef", 3), vec!["abcg", "def"]);
        for b in batches_of(&"wˈʌn tˈu θɹˈi ".repeat(200), MAX_TOKENS) {
            assert!(token_len(&b) <= MAX_TOKENS, "{}", token_len(&b));
        }
    }

    #[test]
    fn tokens_drop_unknown_symbols() {
        assert_eq!(tokens("a\u{2753}b"), vec![43, 44]);
    }
}
