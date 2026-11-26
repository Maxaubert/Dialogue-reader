//! The permissive fallback for words misaki does not know (no espeak, spec
//! R2): split into lexicon words, then a small letter-to-sound rule set,
//! then spell the letters (M0, candidate A).
//!
//! From Sonara (MIT, `crates/sonara-engine/src/kokoro/fallback.rs` at commit 8fd2af7).
use misaki::Lexicon;
use misaki::fallback::{Fallback, FallbackError};
use std::sync::Arc;

pub struct WordFallback {
    lex: Arc<Lexicon>,
}

/// Two-letter parts a split may use; other two-letter "words" in the
/// lexicon (abbreviations) make bad splits.
const SHORT: &[&str] = &[
    "ad", "on", "up", "in", "an", "at", "it", "is", "go", "do", "no", "so", "to", "me", "my", "we",
    "id",
];

/// Letter-to-sound rules, longest graphemes first (misaki's US symbols).
const RULES: &[(&str, &str)] = &[
    ("tion", "ʃən"),
    ("sion", "ʒən"),
    ("ough", "O"),
    ("igh", "I"),
    ("sh", "ʃ"),
    ("ch", "ʧ"),
    ("th", "θ"),
    ("ph", "f"),
    ("ng", "ŋ"),
    ("ck", "k"),
    ("qu", "kw"),
    ("wh", "w"),
    ("ee", "i"),
    ("ea", "i"),
    ("oo", "u"),
    ("ou", "W"),
    ("ow", "O"),
    ("ai", "A"),
    ("ay", "A"),
    ("oi", "Y"),
    ("oy", "Y"),
    ("au", "ɔ"),
    ("aw", "ɔ"),
    ("ar", "ɑɹ"),
    ("er", "ɜɹ"),
    ("ir", "ɜɹ"),
    ("ur", "ɜɹ"),
    ("or", "ɔɹ"),
    ("a", "æ"),
    ("e", "ɛ"),
    ("i", "ɪ"),
    ("o", "ɑ"),
    ("u", "ʌ"),
    ("y", "i"),
    ("b", "b"),
    ("c", "k"),
    ("d", "d"),
    ("f", "f"),
    ("g", "ɡ"),
    ("h", "h"),
    ("j", "ʤ"),
    ("k", "k"),
    ("l", "l"),
    ("m", "m"),
    ("n", "n"),
    ("p", "p"),
    ("q", "k"),
    ("r", "ɹ"),
    ("s", "s"),
    ("t", "t"),
    ("v", "v"),
    ("w", "w"),
    ("x", "ks"),
    ("z", "z"),
];

impl WordFallback {
    pub fn new(lex: Arc<Lexicon>) -> Self {
        WordFallback { lex }
    }

    fn known(&self, w: &str) -> Option<String> {
        self.lex.get_word(w, "NN", None, None).map(|(p, _)| p)
    }

    /// Letter names, one after the other ("SQL" is "S Q L").
    fn spell(&self, word: &str) -> String {
        word.chars()
            .filter(|c| c.is_ascii_alphanumeric())
            .map(|c| {
                let u = c.to_ascii_uppercase().to_string();
                self.known(&u).unwrap_or_default()
            })
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Split into 2 or 3 lexicon words: fewest parts, then the most common
    /// (gold) words, then no plural first part, then the longest first part.
    fn segment(&self, word: &str) -> Option<String> {
        let w: Vec<char> = word.to_lowercase().chars().collect();
        let n = w.len();
        if n < 4 {
            return None;
        }
        let part = |a: usize, b: usize| -> String { w[a..b].iter().collect() };
        let ok = |a: usize, b: usize| -> Option<String> {
            let p = part(a, b);
            if p.chars().count() < 2 || (p.chars().count() == 2 && !SHORT.contains(&p.as_str())) {
                return None;
            }
            self.known(&p)
        };
        let gold = |a: usize, b: usize| -> usize { self.lex.in_gold(&part(a, b)) as usize };
        let plural_first = |i: usize| (w[i - 1] == 's') as usize;
        type Key = (usize, usize, usize, usize);
        let mut best: Option<(Key, String)> = None;
        let mut offer = |key: Key, ps: String| {
            if best.as_ref().is_none_or(|(k, _)| key < *k) {
                best = Some((key, ps));
            }
        };
        for i in 2..n {
            let Some(p1) = ok(0, i) else { continue };
            if let Some(p2) = ok(i, n) {
                let key = (2, 2 - gold(0, i) - gold(i, n), plural_first(i), n - i);
                offer(key, format!("{p1}{p2}"));
            }
            for j in (i + 2)..n {
                if let (Some(p2), Some(p3)) = (ok(i, j), ok(j, n)) {
                    let key = (
                        3,
                        3 - gold(0, i) - gold(i, j) - gold(j, n),
                        plural_first(i),
                        n - i,
                    );
                    offer(key, format!("{p1}{p2}{p3}"));
                }
            }
        }
        best.map(|(_, p)| p)
    }

    /// Naive English letter-to-sound for pronounceable words; `None` for
    /// long consonant runs (nvlddmkm, dxgkrnl), which are spelled instead.
    fn lts(&self, word: &str) -> Option<String> {
        let w = word.to_lowercase();
        if !w.chars().all(|c| c.is_ascii_lowercase()) {
            return None;
        }
        let vowel = |c: char| "aeiouy".contains(c);
        if !w.chars().any(vowel) {
            return None;
        }
        let mut run = 0;
        for c in w.chars() {
            run = if vowel(c) { 0 } else { run + 1 };
            if run >= 4 {
                return None;
            }
        }
        let mut out = String::new();
        let mut i = 0;
        let b = w.as_bytes();
        let mut stressed = false;
        while i < b.len() {
            // A final e after a consonant is silent.
            if i == b.len() - 1 && b[i] == b'e' && i >= 2 {
                break;
            }
            let rest = &w[i..];
            let (g, p) = RULES.iter().find(|(g, _)| rest.starts_with(g))?;
            let is_vowel = p
                .chars()
                .next()
                .map(|c| "æɛɪɑʌiuAIOWYɔɜ".contains(c))
                .unwrap_or(false);
            if is_vowel && !stressed {
                out.push('ˈ');
                stressed = true;
            }
            // A doubled consonant is read once.
            if !(out.ends_with(p) && !is_vowel) {
                out.push_str(p);
            }
            i += g.len();
        }
        Some(out)
    }
}

impl Fallback for WordFallback {
    fn phonemize(&self, word: &str) -> Result<String, FallbackError> {
        let letters: String = word.chars().filter(|c| c.is_alphabetic()).collect();
        let all_caps = !letters.is_empty() && letters.chars().all(|c| c.is_uppercase());
        if all_caps && letters.chars().count() <= 5 {
            return Ok(self.spell(word));
        }
        if let Some(p) = self.segment(word) {
            return Ok(p);
        }
        if let Some(p) = self.lts(word) {
            return Ok(p);
        }
        Ok(self.spell(word))
    }
}
