//! Text normalising and character error rate.

/// Lowercase; punctuation, brackets and symbols become spaces; whitespace collapsed.
pub fn normalize(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut space = false;
    for ch in s.chars().flat_map(|c| c.to_lowercase()) {
        if ch.is_alphanumeric() {
            if space && !out.is_empty() {
                out.push(' ');
            }
            space = false;
            out.push(ch);
        } else {
            space = true;
        }
    }
    out
}

/// Levenshtein distance over chars.
pub fn levenshtein(a: &[char], b: &[char]) -> usize {
    if a.is_empty() {
        return b.len();
    }
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut cur = vec![0; b.len() + 1];
    for (i, ca) in a.iter().enumerate() {
        cur[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let sub = prev[j] + usize::from(ca != cb);
            cur[j + 1] = sub.min(prev[j + 1] + 1).min(cur[j] + 1);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[b.len()]
}

/// CER of `hyp` against `truth`, both normalised first: edits / truth length.
/// Empty truth gives 0 for an empty hypothesis and 1 otherwise (never divides by zero).
pub fn cer(hyp: &str, truth: &str) -> f64 {
    cer_normalized(&normalize(hyp), &normalize(truth))
}

/// CER of two already normalised strings.
pub fn cer_normalized(hyp: &str, truth: &str) -> f64 {
    let h: Vec<char> = hyp.chars().collect();
    let t: Vec<char> = truth.chars().collect();
    if t.is_empty() {
        return if h.is_empty() { 0.0 } else { 1.0 };
    }
    levenshtein(&h, &t) as f64 / t.len() as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes() {
        assert_eq!(normalize("  + 6 Iron Ore (27)  "), "6 iron ore 27");
        assert_eq!(normalize("Press [E] to\nopen!"), "press e to open");
        assert_eq!(normalize("[E]"), "e");
        assert_eq!(normalize("!!"), "");
    }

    #[test]
    fn cer_values() {
        assert_eq!(cer("Hello", "hello!"), 0.0);
        assert!((cer("helo", "hello") - 0.2).abs() < 1e-9);
        assert_eq!(cer("", "abc"), 1.0);
        assert_eq!(cer("", "!!"), 0.0);
        assert_eq!(cer("x", "!!"), 1.0);
        assert!(cer("+ 6 Iron Ore (27)", "+ 12 Iron Ore (33)") < 0.35);
        assert_eq!(levenshtein(&['a', 'b'], &['b', 'a']), 2);
    }
}
