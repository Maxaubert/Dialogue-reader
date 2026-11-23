//! Kokoro's own text rules, run before G2P: code identifiers, paths,
//! versions, numbers and keys, read the way a developer says them. Sonara's shared text rules
//! (`sonara_core::text`) come first in the hosts that use them; these only
//! add what the misaki lexicon cannot know. OneCore does not need them.
//!
//! From Sonara (MIT, `crates/sonara-engine/src/kokoro/text.rs` at commit 8fd2af7).
use regex::{Captures, Regex};
use std::sync::OnceLock;

fn re(cell: &'static OnceLock<Regex>, pat: &str) -> &'static Regex {
    cell.get_or_init(|| Regex::new(pat).expect("valid text rule"))
}

/// How a file extension is read after "dot".
fn ext_word(ext: &str) -> Option<&'static str> {
    Some(match ext.to_ascii_lowercase().as_str() {
        "py" => "P Y",
        "rs" => "R S",
        "ts" => "T S",
        "js" => "J S",
        "json" => "Jason",
        "md" => "M D",
        "exe" => "E X E",
        "dll" => "D L L",
        "toml" => "toml",
        "txt" => "text",
        "wav" => "wave",
        "onnx" => "onyx",
        "bin" => "bin",
        "html" => "H T M L",
        "yml" | "yaml" => "yammel",
        "cs" => "C sharp",
        "lock" => "lock",
        _ => return None,
    })
}

/// Digits after a decimal point are read one by one: "14" is "one four".
fn digits_spoken(d: &str) -> String {
    const NAMES: [&str; 10] = [
        "zero", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine",
    ];
    d.chars()
        .filter_map(|c| c.to_digit(10))
        .map(|n| NAMES[n as usize])
        .collect::<Vec<_>>()
        .join(" ")
}

fn ordinal(n: u32) -> String {
    const ONES: [&str; 20] = [
        "zeroth",
        "first",
        "second",
        "third",
        "fourth",
        "fifth",
        "sixth",
        "seventh",
        "eighth",
        "ninth",
        "tenth",
        "eleventh",
        "twelfth",
        "thirteenth",
        "fourteenth",
        "fifteenth",
        "sixteenth",
        "seventeenth",
        "eighteenth",
        "nineteenth",
    ];
    const TENS: [&str; 10] = [
        "", "", "twenty", "thirty", "forty", "fifty", "sixty", "seventy", "eighty", "ninety",
    ];
    const TENS_TH: [&str; 10] = [
        "",
        "",
        "twentieth",
        "thirtieth",
        "fortieth",
        "fiftieth",
        "sixtieth",
        "seventieth",
        "eightieth",
        "ninetieth",
    ];
    let n = n as usize;
    if n < 20 {
        ONES[n].to_string()
    } else if n.is_multiple_of(10) {
        TENS_TH[n / 10].to_string()
    } else {
        format!("{} {}", TENS[n / 10], ONES[n % 10])
    }
}

/// Apply the rules to one chunk of text.
pub fn normalize(text: &str) -> String {
    static ACRO_PL: OnceLock<Regex> = OnceLock::new();
    static ORD: OnceLock<Regex> = OnceLock::new();
    static LONGCAPS: OnceLock<Regex> = OnceLock::new();
    static VERSION: OnceLock<Regex> = OnceLock::new();
    static DOTTED: OnceLock<Regex> = OnceLock::new();
    static DECIMAL: OnceLock<Regex> = OnceLock::new();
    static ENVVAR: OnceLock<Regex> = OnceLock::new();
    static SCOPE: OnceLock<Regex> = OnceLock::new();
    static CALL: OnceLock<Regex> = OnceLock::new();
    static FLAG: OnceLock<Regex> = OnceLock::new();
    static KEYS: OnceLock<Regex> = OnceLock::new();
    static FILE: OnceLock<Regex> = OnceLock::new();
    static PATHSEP: OnceLock<Regex> = OnceLock::new();
    static DRIVE: OnceLock<Regex> = OnceLock::new();
    static CAMEL1: OnceLock<Regex> = OnceLock::new();
    static CAMEL2: OnceLock<Regex> = OnceLock::new();
    static HYPHEN: OnceLock<Regex> = OnceLock::new();
    /// English words of one or two letters that end a hyphenated word.
    const SHORT_WORDS: &[&str] = &[
        "a", "i", "up", "in", "on", "of", "to", "by", "at", "go", "do", "no", "so", "be", "me",
        "we", "he", "it", "is", "an", "as", "or", "if", "my", "us", "ok", "oh", "hi", "ex", "ox",
        "ax",
    ];
    static WS: OnceLock<Regex> = OnceLock::new();

    let mut t = text.to_string();
    // IDs, GUIDs, PRs -> ID's (the plural of an acronym, not CamelCase).
    t = re(&ACRO_PL, r"\b([A-Z]{2,})s\b")
        .replace_all(&t, "$1's")
        .into_owned();
    // 28th -> twenty eighth.
    t = re(&ORD, r"\b(\d{1,2})(st|nd|rd|th)\b")
        .replace_all(&t, |c: &Captures| {
            ordinal(c[1].parse().expect("one or two digits"))
        })
        .into_owned();
    // Long all-caps words are names, not acronyms: SONARA, LOCALAPPDATA.
    t = re(&LONGCAPS, r"\b[A-Z]{6,}\b")
        .replace_all(&t, |c: &Captures| c[0].to_lowercase())
        .into_owned();
    // onnx inside identifiers: onnxruntime -> onyx runtime.
    t = t
        .replace("onnxruntime", "onyx runtime")
        .replace("onnx", "onyx");
    // v0.8.5 -> version 0 point 8 point 5.
    t = re(&VERSION, r"\bv(\d+(?:\.\d+)+)\b")
        .replace_all(&t, |c: &Captures| {
            format!("version {}", c[1].replace('.', " point "))
        })
        .into_owned();
    // 1.2.3 (two or more dots) -> 1 point 2 point 3.
    t = re(&DOTTED, r"\b\d+(?:\.\d+){2,}\b")
        .replace_all(&t, |c: &Captures| c[0].replace('.', " point "))
        .into_owned();
    // 3.14 -> 3 point one four.
    t = re(&DECIMAL, r"\b(\d+)\.(\d+)\b")
        .replace_all(&t, |c: &Captures| {
            format!("{} point {}", &c[1], digits_spoken(&c[2]))
        })
        .into_owned();
    // %LOCALAPPDATA% -> LOCALAPPDATA (the fallback splits it into words).
    t = re(&ENVVAR, r"%([A-Za-z_]+)%")
        .replace_all(&t, "$1")
        .into_owned();
    // @sonara/client -> at sonara slash client.
    t = re(&SCOPE, r"@([a-z][\w-]*)/")
        .replace_all(&t, "at $1/")
        .into_owned();
    // connect() -> connect.
    t = re(&CALL, r"\b(\w+)\(\)").replace_all(&t, "$1").into_owned();
    // --reset -> dash dash reset.
    t = re(&FLAG, r"(^|\s)--([a-z])")
        .replace_all(&t, "${1}dash dash $2")
        .into_owned();
    // Ctrl+Alt+M -> Control Alt M; macOS: Ctrl+Option+Cmd+M -> Control
    // Option Command M.
    t = re(&KEYS, r"\b(Ctrl|Alt|Option|Shift|Win|Cmd)\+")
        .replace_all(&t, |c: &Captures| {
            let k = match &c[1] {
                "Ctrl" => "Control",
                "Cmd" => "Command",
                k => k,
            };
            format!("{k} ")
        })
        .into_owned();
    // name.ext -> name dot <ext>.
    t = re(&FILE, r"\b([\w-]+)\.([A-Za-z]{2,4})\b")
        .replace_all(&t, |c: &Captures| match ext_word(&c[2]) {
            Some(w) => format!("{} dot {}", &c[1], w),
            None => c[0].to_string(),
        })
        .into_owned();
    // C:\Users -> C drive slash Users.
    t = re(&DRIVE, r"\b([A-Za-z]):[/\\](\w)")
        .replace_all(&t, "$1 drive slash $2")
        .into_owned();
    // Path separators (twice: matches overlap at one-letter segments).
    for _ in 0..2 {
        t = re(&PATHSEP, r"(\w)[/\\](\w)")
            .replace_all(&t, "$1 slash $2")
            .into_owned();
    }
    // CamelCase: SessionChannel -> Session Channel, HTTPServer -> HTTP Server.
    t = re(&CAMEL1, r"([a-z0-9])([A-Z])")
        .replace_all(&t, "$1 $2")
        .into_owned();
    t = re(&CAMEL2, r"([A-Z]+)([A-Z][a-z])")
        .replace_all(&t, "$1 $2")
        .into_owned();
    // Code-ish hyphens: cargo-deny stays a compound; a short tail becomes
    // an acronym that misaki spells: espeak-ng -> espeak NG. A short English
    // word stays a word: follow-up, check-in, add-on (#314).
    t = re(&HYPHEN, r"\b([a-z]+)-([a-z]+)\b")
        .replace_all(&t, |c: &Captures| {
            let b = &c[2];
            if b.len() <= 2 && !SHORT_WORDS.contains(&b) {
                format!("{} {}", &c[1], b.to_uppercase())
            } else {
                format!("{}-{}", &c[1], b)
            }
        })
        .into_owned();
    re(&WS, r"\s+").replace_all(&t, " ").trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_numbers_and_ordinals() {
        assert_eq!(
            normalize("Pi is 3.14 at v0.8.5 on the 28th."),
            "Pi is 3 point one four at version 0 point 8 point 5 on the twenty eighth."
        );
        assert_eq!(normalize("1.2.3"), "1 point 2 point 3");
        assert_eq!(normalize("the 3rd and 40th"), "the third and fortieth");
    }

    #[test]
    fn paths_files_and_identifiers() {
        assert_eq!(
            normalize("src/sonara/daemon/ingest.py"),
            "src slash sonara slash daemon slash ingest dot P Y"
        );
        assert_eq!(
            normalize("Set %LOCALAPPDATA%\\Sonara"),
            "Set localappdata slash Sonara"
        );
        assert_eq!(normalize("SessionChannel"), "Session Channel");
        assert_eq!(normalize("HTTPServer"), "HTTP Server");
        assert_eq!(
            normalize("npm i @sonara/client, then connect()"),
            "npm i at sonara slash client, then connect"
        );
        assert_eq!(normalize("onnxruntime.dll"), "onyx runtime dot D L L");
    }

    #[test]
    fn drive_letters_and_toml() {
        assert_eq!(
            normalize(r"C:\Users\me and D:/data"),
            "C drive slash Users slash me and D drive slash data"
        );
        assert_eq!(normalize("Cargo.toml"), "Cargo dot toml");
    }

    #[test]
    fn a_macos_chord_is_read_as_words() {
        // The macOS labels (keymap::Labels::Mac) in the step hint.
        assert_eq!(
            normalize("Use Ctrl+Option+Down, or Option+Cmd+P"),
            "Use Control Option Down, or Option Command P"
        );
    }

    #[test]
    fn keys_flags_and_hyphens() {
        assert_eq!(
            normalize("Run keymap --reset, then Ctrl+Alt+M"),
            "Run keymap dash dash reset, then Control Alt M"
        );
        assert_eq!(
            normalize("espeak-ng and cargo-deny"),
            "espeak NG and cargo-deny"
        );
    }

    #[test]
    fn a_short_english_word_after_a_hyphen_is_not_spelled() {
        // sonarad.log 2026-10-15 13:07:36: "follow-up" was read "follow U P".
        assert_eq!(
            normalize("a follow-up, a check-in, an add-on and a set-up"),
            "a follow-up, a check-in, an add-on and a set-up"
        );
        assert_eq!(normalize("GUIDs and IDs"), "GUID's and ID's");
    }

    #[test]
    fn non_ascii_text_passes_through() {
        assert_eq!(
            normalize("Caf\u{e9} \u{2192} \u{1f600}  ok"),
            "Caf\u{e9} \u{2192} \u{1f600} ok"
        );
        assert_eq!(normalize(""), "");
    }
}
