//! The activity log of a game (spec 2.8): `log\<id>.jsonl`, one JSON line per line read, spoken or skipped (and
//! why). Overview shows the newest five (Recent), "Show all" the list. Capped at [`MAX_BYTES`]: when a line would
//! pass it, the older half is cut.

use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// The log is cut back to about half this size when it grows past it.
pub const MAX_BYTES: u64 = 5 * 1024 * 1024;

/// Why a line read was not spoken.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Skip {
    /// A prompt zone with prompts off.
    PromptsOff,
    /// The zone already spoke this line.
    AlreadyRead,
    /// It waited in the queue too long (more than 10 s).
    TooOld,
    /// The voice could not speak it.
    VoiceError,
    /// Stop came before it was spoken.
    Stopped,
}

impl Skip {
    /// The text the activity list shows.
    pub fn text(self) -> &'static str {
        match self {
            Skip::PromptsOff => "Prompts off",
            Skip::AlreadyRead => "Already read",
            Skip::TooOld => "Waited too long",
            Skip::VoiceError => "Voice error",
            Skip::Stopped => "Stopped",
        }
    }
}

/// One line of the log.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Entry {
    /// Unix time in ms.
    pub t: u64,
    /// The zone name (empty when unknown).
    #[serde(default)]
    pub zone: String,
    /// The zone kind, by its name on disk (`dialogue`, `speaker`, ...); empty when unknown.
    #[serde(default)]
    pub kind: String,
    /// The speaker, when a speaker line was spoken with it ("Arthur:").
    #[serde(default)]
    pub who: String,
    pub text: String,
    /// None: spoken.
    #[serde(default)]
    pub skip: Option<Skip>,
}

/// Unix time now in ms.
pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

/// The log file of one game.
pub struct Log {
    path: PathBuf,
    max: u64,
}

impl Log {
    pub fn new(path: impl Into<PathBuf>) -> Log {
        Log {
            path: path.into(),
            max: MAX_BYTES,
        }
    }

    /// A log cut at `max` bytes (tests).
    pub fn with_max(path: impl Into<PathBuf>, max: u64) -> Log {
        Log {
            path: path.into(),
            max,
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Adds lines at the end. A file that would grow past the cap keeps its newer half first.
    pub fn append(&self, entries: &[Entry]) -> std::io::Result<()> {
        if entries.is_empty() {
            return Ok(());
        }
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let mut text = String::new();
        for e in entries {
            text.push_str(&serde_json::to_string(e).map_err(std::io::Error::other)?);
            text.push('\n');
        }
        let size = std::fs::metadata(&self.path).map_or(0, |m| m.len());
        if size + text.len() as u64 > self.max {
            self.cut(text.len() as u64)?;
        }
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        f.write_all(text.as_bytes())
    }

    /// Keeps the newest lines that fit in half the cap (minus `room`).
    fn cut(&self, room: u64) -> std::io::Result<()> {
        let old = std::fs::read_to_string(&self.path).unwrap_or_default();
        let keep = (self.max / 2).saturating_sub(room) as usize;
        let mut start = old.len().saturating_sub(keep);
        // start at a line
        while start > 0 && start < old.len() && !old.is_char_boundary(start) {
            start += 1;
        }
        let rest = &old[start.min(old.len())..];
        let rest = match rest.find('\n') {
            Some(i) if start > 0 => &rest[i + 1..],
            _ => rest,
        };
        crate::paths::write_atomic(&self.path, rest.as_bytes())
    }

    /// The newest `n` lines, oldest first. Lines that do not parse are left out.
    pub fn last(&self, n: usize) -> Vec<Entry> {
        let Ok(text) = std::fs::read_to_string(&self.path) else {
            return Vec::new();
        };
        let mut out: Vec<Entry> = text
            .lines()
            .rev()
            .filter_map(|l| serde_json::from_str(l).ok())
            .take(n)
            .collect();
        out.reverse();
        out
    }
}

/// "21:41:50" in the PC's time zone for a unix time in ms.
#[cfg(windows)]
pub fn local_time(t_ms: u64) -> String {
    use windows::Win32::Foundation::{FILETIME, SYSTEMTIME};
    use windows::Win32::System::Time::{FileTimeToSystemTime, SystemTimeToTzSpecificLocalTime};
    // FILETIME counts 100 ns steps since 1601
    let ft = t_ms * 10_000 + 116_444_736_000_000_000;
    let ft = FILETIME {
        dwLowDateTime: ft as u32,
        dwHighDateTime: (ft >> 32) as u32,
    };
    let mut utc = SYSTEMTIME::default();
    let mut local = SYSTEMTIME::default();
    unsafe {
        if FileTimeToSystemTime(&ft, &mut utc).is_err()
            || SystemTimeToTzSpecificLocalTime(None, &utc, &mut local).is_err()
        {
            return utc_time(t_ms);
        }
    }
    format!(
        "{:02}:{:02}:{:02}",
        local.wHour, local.wMinute, local.wSecond
    )
}

#[cfg(not(windows))]
pub fn local_time(t_ms: u64) -> String {
    utc_time(t_ms)
}

fn utc_time(t_ms: u64) -> String {
    let s = t_ms / 1000;
    format!("{:02}:{:02}:{:02}", s / 3600 % 24, s / 60 % 60, s % 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn e(text: &str, skip: Option<Skip>) -> Entry {
        Entry {
            t: 1_700_000_000_000,
            zone: "Subtitles".into(),
            kind: "dialogue".into(),
            who: String::new(),
            text: text.into(),
            skip,
        }
    }

    #[test]
    fn append_and_read_back_the_newest() {
        let dir = tempfile::tempdir().unwrap();
        let log = Log::new(dir.path().join("log").join("g.jsonl"));
        assert!(log.last(5).is_empty());
        log.append(&[e("one", None), e("two", Some(Skip::PromptsOff))])
            .unwrap();
        log.append(&[e("three", Some(Skip::AlreadyRead))]).unwrap();
        let last = log.last(2);
        assert_eq!(
            last.iter().map(|x| x.text.as_str()).collect::<Vec<_>>(),
            ["two", "three"]
        );
        assert_eq!(last[0].skip, Some(Skip::PromptsOff));
        let raw = std::fs::read_to_string(log.path()).unwrap();
        assert!(raw.contains("\"skip\":\"prompts_off\""), "{raw}");
        // a damaged line is left out
        std::fs::write(log.path(), format!("{raw}not json\n")).unwrap();
        assert_eq!(log.last(10).len(), 3);
    }

    #[test]
    fn a_full_log_keeps_its_newer_half() {
        let dir = tempfile::tempdir().unwrap();
        let log = Log::with_max(dir.path().join("g.jsonl"), 2_000);
        for i in 0..100 {
            log.append(&[e(&format!("line {i}"), None)]).unwrap();
        }
        let size = std::fs::metadata(log.path()).unwrap().len();
        assert!(size <= 2_000, "{size}");
        let all = log.last(1000);
        assert_eq!(all.last().unwrap().text, "line 99");
        assert!(all.len() > 5 && all.first().unwrap().text != "line 0");
    }

    #[test]
    fn times_and_texts() {
        assert_eq!(utc_time(((21 * 60 + 41) * 60 + 50) * 1000), "21:41:50");
        assert_eq!(local_time(now_ms()).len(), 8);
        assert_eq!(Skip::TooOld.text(), "Waited too long");
    }
}
