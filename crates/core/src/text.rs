//! Text helpers: normalising, edit distance and the "same line?" rules used by dedup.

/// Lowercase; anything that is not a letter or digit becomes a space; spaces collapsed.
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

/// Edit distance of two already normalised strings, divided by the longer length (0..=1).
pub fn distance(a: &str, b: &str) -> f32 {
    if a == b {
        return 0.0;
    }
    let (ca, cb): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    let n = ca.len().max(cb.len());
    if n == 0 { 0.0 } else { levenshtein(&ca, &cb) as f32 / n as f32 }
}

/// The numbers of a string in order, joined by commas ("+ 6 Iron Ore (27)" gives "6,27").
pub fn digits(s: &str) -> String {
    let mut out = String::new();
    let mut prev_digit = false;
    for c in s.chars() {
        if c.is_ascii_digit() {
            if !prev_digit && !out.is_empty() {
                out.push(',');
            }
            out.push(c);
        }
        prev_digit = c.is_ascii_digit();
    }
    out
}

/// Two number lists (from [`digits`]) fit together: equal, or one is a run inside the other, as
/// when OCR missed a part of the line ("20" and "20,23"; "" fits everything).
pub fn numbers_fit(a: &str, b: &str) -> bool {
    if a == b {
        return true;
    }
    let la: Vec<&str> = a.split(',').filter(|x| !x.is_empty()).collect();
    let lb: Vec<&str> = b.split(',').filter(|x| !x.is_empty()).collect();
    let (short, long) = if la.len() <= lb.len() { (la, lb) } else { (lb, la) };
    short.is_empty() || long.windows(short.len()).any(|w| w == short.as_slice())
}

/// Number of letters (any script).
pub fn letters(s: &str) -> usize {
    s.chars().filter(|c| c.is_alphabetic()).count()
}

/// `short` is the start of `long` (both normalised), allowing the last word of `short` to be cut
/// or misread: a typewriter line that is still being typed.
pub fn is_growing_prefix(short: &str, long: &str) -> bool {
    let (s, l) = (short.chars().count(), long.chars().count());
    if s < 2 || s >= l {
        return false;
    }
    // A two-letter start counts only as a whole word ("go" of "go to", not of "gold").
    if long.starts_with(short) && (s >= 3 || long.chars().nth(s) == Some(' ')) {
        return true;
    }
    if s < 3 {
        return false;
    }
    // Drop the last (maybe half drawn) word of `short` and compare again.
    if let Some((head, _)) = short.rsplit_once(' ')
        && head.chars().count() >= 3
        && long.starts_with(head)
    {
        return true;
    }
    // OCR errors: `short` is close to the start of `long` ("sa you arc" and "so you are the").
    if s >= 5 {
        let start: String = long.chars().take(s).collect();
        return distance(short, &start) <= 0.25 && digits(short) == digits(&start);
    }
    false
}

/// How two reads of one zone relate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Relation {
    /// The same text (equal or OCR jitter).
    Same,
    /// One is a typed-out continuation of the other.
    Grows,
    /// Looks the same but a number changed: a new event ("+ 6 Iron Ore (27)" to "+ 12 Iron Ore (33)").
    NumberChanged,
    Different,
}

/// The letters of a normalised string, digits dropped and spaces collapsed
/// ("6 iron ore 27" gives "iron ore").
pub fn letters_key(norm: &str) -> String {
    norm.split(' ').filter(|w| w.chars().any(|c| !c.is_ascii_digit())).map(|w| w.chars().filter(|c| !c.is_ascii_digit()).collect::<String>()).collect::<Vec<_>>().join(" ")
}

/// Compares two normalised reads. Letters decide whether it is the same line (`jitter` is the
/// largest distance between the letter keys); digits decide whether a number changed.
pub fn relate(a: &str, b: &str, jitter: f32) -> Relation {
    if a == b {
        return Relation::Same;
    }
    if is_growing_prefix(a, b) || is_growing_prefix(b, a) {
        return Relation::Grows;
    }
    let (ka, kb) = (letters_key(a), letters_key(b));
    if ka.is_empty() || kb.is_empty() {
        return Relation::Different;
    }
    // Short words must match exactly ("Nyx" vs "Nyr" are different names), or be the last
    // words of the longer read (OCR lost the start: "Talk" and "E Talk", "Ore" and "Iron Ore").
    let (short, long) = if ka.chars().count() <= kb.chars().count() { (&ka, &kb) } else { (&kb, &ka) };
    let same_letters = if short.chars().count() <= 4 {
        short == long || long.ends_with(&format!(" {short}"))
    } else {
        distance(&ka, &kb) <= jitter.max(0.0)
    };
    if !same_letters {
        return Relation::Different;
    }
    if numbers_fit(&digits(a), &digits(b)) { Relation::Same } else { Relation::NumberChanged }
}

/// `new` holds a word that `old` does not: a new line that looks like the old one ("Iron Ore" to
/// "Iron Rod", "Kael" to "Kaelen", "I can do that." to "I can not do that."). OCR noise must not
/// count, so the new word needs 3 or more letters, the texts must differ by 2 or more letters
/// (spaces ignored, so a split or joined word is no change), and a cut read of `old` is no change.
pub fn new_words(old: &str, new: &str) -> bool {
    if old == new || is_growing_prefix(new, old) {
        return false;
    }
    let (ko, kn) = (letters_key(old), letters_key(new));
    let flat = |s: &str| s.chars().filter(|c| *c != ' ').collect::<Vec<char>>();
    if levenshtein(&flat(&ko), &flat(&kn)) < 2 {
        return false;
    }
    let ow: Vec<&str> = ko.split(' ').filter(|w| !w.is_empty()).collect();
    kn.split(' ').filter(|w| w.chars().count() >= 3).any(|w| !ow.iter().any(|o| word_match(o, w)))
}

/// Two words are the same up to OCR noise: equal, or 4 letters or more and at most 1 edit in 4.
fn word_match(a: &str, b: &str) -> bool {
    a == b || (a.chars().count() >= 4 && b.chars().count() >= 4 && distance(a, b) <= 0.25)
}

/// Every word of `old` is still in `new`: `new` only adds words ("Glass" to "Glass Vial").
pub fn adds_only(old: &str, new: &str) -> bool {
    let (ko, kn) = (letters_key(old), letters_key(new));
    let nw: Vec<&str> = kn.split(' ').filter(|w| !w.is_empty()).collect();
    ko.split(' ').filter(|w| !w.is_empty()).all(|o| nw.iter().any(|w| word_match(o, w)))
}

/// A read too weak to speak: too few letters, or mostly symbols.
pub fn is_junk(raw: &str, min_letters: usize) -> bool {
    let n = normalize(raw);
    if n.is_empty() {
        return true;
    }
    let l = letters(&n);
    if l < min_letters {
        return true;
    }
    // Mostly symbols: letters and digits are under half of the visible characters.
    let visible = raw.chars().filter(|c| !c.is_whitespace()).count().max(1);
    let alnum = raw.chars().filter(|c| c.is_alphanumeric()).count();
    alnum * 2 < visible
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_like_the_scorer() {
        assert_eq!(normalize("  + 6 Iron Ore (27)!"), "6 iron ore 27");
        assert_eq!(normalize("Press [E]"), "press e");
        assert_eq!(normalize("--"), "");
    }

    #[test]
    fn number_change_is_a_new_line_but_jitter_is_not() {
        let a = normalize("+ 6 Iron Ore (27)");
        let b = normalize("+ 12 Iron Ore (33)");
        assert_eq!(relate(&a, &b, 0.3), Relation::NumberChanged);
        assert_eq!(relate(&normalize("Restores 40 heaIth"), &normalize("Restores 40 health"), 0.3), Relation::Same);
        assert_eq!(relate("nyx", "nyr", 0.4), Relation::Different);
        assert_eq!(relate("talk", "e talk", 0.3), Relation::Same);
        assert_eq!(relate("9 ore", "9 iron ore", 0.3), Relation::Same);
        assert_eq!(relate("ash", "flash", 0.3), Relation::Different);
        assert_eq!(relate("go to the light", "go to the lighthouse and", 0.3), Relation::Grows);
        assert_eq!(relate("go to th", "go to the lighthouse", 0.3), Relation::Grows);
        assert_eq!(relate("so you a", "sa you arc t", 0.3), Relation::Grows);
        assert_eq!(relate("open the gate", "close the door", 0.3), Relation::Different);
        assert_eq!(relate("46 iron ore 27", "6 iron ore 27", 0.3), Relation::NumberChanged);
        assert_eq!(relate("645", "646", 0.3), Relation::Different);
        assert_eq!(letters_key("6 iron ore 27"), "iron ore");
        assert_eq!(digits("+ 6 Iron Ore (27)"), "6,27");
        assert!(numbers_fit("20", "20,23"));
        assert!(numbers_fit("", "5,22"));
        assert!(numbers_fit("22", "5,22"));
        assert!(!numbers_fit("1", "12"));
        assert!(!numbers_fit("6,27", "12,33"));
        assert_eq!(relate("i copper ore 22", "5 copper ore 22", 0.3), Relation::Same);
        assert_eq!(letters_key("h3llo 12 w0rld"), "hllo wrld");
    }

    #[test]
    fn new_words_tell_a_new_line_from_noise() {
        let n = |a: &str, b: &str| new_words(&normalize(a), &normalize(b));
        assert!(n("Picked up Iron Ore", "Picked up Iron Rod"));
        assert!(n("North gate opened", "South gate opened"));
        assert!(n("I can do that.", "I can not do that."));
        assert!(n("Stay close.", "Stay close to the wall and keep quiet."));
        assert!(n("Kael", "Kaelen"));
        // Noise and cut reads.
        assert!(!n("Restores 40 heaIth", "Restores 40 health"));
        assert!(!n("Iron Ore", "IronOre"));
        assert!(!n("Press to talk", "Press E to talk"));
        assert!(!n("Stay close to the wall", "Stay close to the wal"));
        assert!(!n("I can not do that.", "I can do that."));
        assert!(!n("+ 6 Iron Ore (27)", "+ 12 Iron Ore (33)"));
        assert!(adds_only("glass 26", "t 7 glass vial 26"));
        assert!(!adds_only("iron ore", "iron rod"));
    }

    #[test]
    fn junk_filter() {
        assert!(is_junk("-", 2));
        assert!(is_junk("645", 2));
        assert!(is_junk("~ ' I", 2));
        assert!(!is_junk("Nyx", 2));
        assert!(!is_junk("+ 9 Mana Crystal", 2));
        assert!(is_junk("|| -- // a b", 2));
    }

    #[test]
    fn distance_bounds() {
        assert_eq!(distance("", ""), 0.0);
        assert_eq!(distance("abc", ""), 1.0);
        assert!((distance("abcd", "abce") - 0.25).abs() < 1e-6);
    }
}
