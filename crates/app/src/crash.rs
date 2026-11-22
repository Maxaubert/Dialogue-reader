//! Crash log. The shipped exe is built with `panic = "abort"` (`[profile.ship]`), so a panic ends the app at
//! once with no unwinding. The panic hook runs before that: it writes what happened to
//! `%LOCALAPPDATA%\DialogReader\logs\crash-<unix seconds>-<pid>.log`, then the default hook runs.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Installs the hook (once, from `main`). Logs go to `dir`, or to the default folder when `None`.
pub fn install(dir: Option<PathBuf>) {
    let Some(dir) = dir.or_else(dr_runtime::paths::crash_logs) else {
        return;
    };
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let thread = std::thread::current()
            .name()
            .unwrap_or("unnamed")
            .to_string();
        let msg = if let Some(s) = info.payload().downcast_ref::<&str>() {
            (*s).to_string()
        } else if let Some(s) = info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "(no message)".to_string()
        };
        let place = info
            .location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_default();
        let trace = std::backtrace::Backtrace::force_capture();
        let _ = write_report(&dir, &report(&thread, &msg, &place, &trace.to_string()));
        default(info);
    }));
}

/// The text of a crash log.
pub fn report(thread: &str, msg: &str, place: &str, trace: &str) -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!(
        "DialogReader {} crashed\nunix time: {secs}\nthread: {thread}\nwhere: {place}\nmessage: {msg}\n\nbacktrace:\n{trace}\n",
        env!("CARGO_PKG_VERSION")
    )
}

/// Writes one crash log into `dir` and returns its path.
pub fn write_report(dir: &Path, text: &str) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let path = dir.join(format!("crash-{secs}-{}.log", std::process::id()));
    std::fs::write(&path, text)?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_is_written_to_the_folder() {
        let dir = std::env::temp_dir().join(format!("dr-crash-test-{}", std::process::id()));
        let p = write_report(&dir, &report("main", "boom", "src/x.rs:1:2", "trace")).unwrap();
        let text = std::fs::read_to_string(&p).unwrap();
        assert!(text.contains("message: boom"));
        assert!(text.contains("where: src/x.rs:1:2"));
        assert!(text.starts_with(&format!(
            "DialogReader {} crashed",
            env!("CARGO_PKG_VERSION")
        )));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
