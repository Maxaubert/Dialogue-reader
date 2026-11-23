//! Start with Windows (spec 2.9, M5).
//!
//! Normally a value under `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`: `DialogReader` =
//! `"<path>\DialogReader.exe" --minimized`. No admin rights are needed and the installer removes it on uninstall.
//!
//! Always the Run value, also when DialogReader runs as administrator. An elevated sign-in task (highest
//! privileges) pointing at an exe in a folder the user can write (the per-user install) would let any program of
//! that user replace the exe and get admin rights at the next sign-in, so DialogReader never makes one. An old
//! task of that name (from a 0.2.0 test build) is still removed when Start with Windows is turned off. Starting
//! with Windows as administrator is an open question (docs/app-plan.md, M5).
//!
//! The Run value follows the exe ([`Autostart::follow`]): after a move, or a portable copy replaced by the
//! installed one, the app writes it again at start.
//!
//! The value name is a field, so tests use their own name and never touch the app's value.

use std::path::Path;

use windows::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS};
use windows::Win32::System::Registry::{
    HKEY_CURRENT_USER, REG_SZ, RRF_RT_REG_SZ, RegDeleteKeyValueW, RegGetValueW, RegSetKeyValueW,
};
use windows::core::HSTRING;

/// The Run key under HKCU.
pub const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
/// The app's value name (and the name of an old task, see the module docs).
pub const APP_VALUE: &str = "DialogReader";
/// The argument Windows starts DialogReader with: only the tray icon shows when "Open minimized" is on.
pub const MINIMIZED_ARG: &str = "--minimized";

/// One "start with Windows" entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Autostart {
    name: String,
}

impl Autostart {
    /// The app's own entry (`DialogReader`).
    pub fn app() -> Autostart {
        Autostart::named(APP_VALUE)
    }

    /// An entry with another value name (tests).
    pub fn named(name: &str) -> Autostart {
        Autostart { name: name.into() }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    /// The command line Windows runs: the exe in quotes, then `--minimized`.
    pub fn command(exe: &Path) -> String {
        format!("\"{}\" {MINIMIZED_ARG}", exe.display())
    }

    /// The Run value, `None` when there is none.
    pub fn run_value(&self) -> Option<String> {
        let (key, name) = (HSTRING::from(RUN_KEY), HSTRING::from(self.name.as_str()));
        let mut len = 0u32;
        let r = unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                &key,
                &name,
                RRF_RT_REG_SZ,
                None,
                None,
                Some(&mut len),
            )
        };
        if r != ERROR_SUCCESS || len == 0 {
            return None;
        }
        let mut buf = vec![0u16; (len as usize).div_ceil(2)];
        let r = unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                &key,
                &name,
                RRF_RT_REG_SZ,
                None,
                Some(buf.as_mut_ptr().cast()),
                Some(&mut len),
            )
        };
        if r != ERROR_SUCCESS {
            return None;
        }
        let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
        Some(String::from_utf16_lossy(&buf[..end]))
    }

    /// Writes the Run value for `exe`.
    pub fn set_run_value(&self, exe: &Path) -> Result<(), String> {
        let cmd = Autostart::command(exe);
        let mut data: Vec<u16> = cmd.encode_utf16().collect();
        data.push(0);
        let r = unsafe {
            RegSetKeyValueW(
                HKEY_CURRENT_USER,
                &HSTRING::from(RUN_KEY),
                &HSTRING::from(self.name.as_str()),
                REG_SZ.0,
                Some(data.as_ptr().cast()),
                (data.len() * 2) as u32,
            )
        };
        if r == ERROR_SUCCESS {
            Ok(())
        } else {
            Err(format!("writing the Run value failed ({})", r.0))
        }
    }

    /// Removes the Run value. No value is not an error.
    pub fn remove_run_value(&self) -> Result<(), String> {
        let r = unsafe {
            RegDeleteKeyValueW(
                HKEY_CURRENT_USER,
                &HSTRING::from(RUN_KEY),
                &HSTRING::from(self.name.as_str()),
            )
        };
        if r == ERROR_SUCCESS || r == ERROR_FILE_NOT_FOUND {
            Ok(())
        } else {
            Err(format!("removing the Run value failed ({})", r.0))
        }
    }

    /// An old scheduled task of this name exists (DialogReader no longer makes one).
    pub fn has_task(&self) -> bool {
        schtasks(&["/Query", "/TN", &self.name]).is_ok()
    }

    /// Start with Windows is on: the Run value or the task is there.
    pub fn is_on(&self) -> bool {
        self.run_value().is_some() || self.has_task()
    }

    /// The Run value points at `exe` (after a move or a new install it points elsewhere and is written again).
    pub fn points_at(&self, exe: &Path) -> bool {
        self.run_value()
            .is_some_and(|v| v.eq_ignore_ascii_case(&Autostart::command(exe)))
    }

    /// The Run value is there but points at another exe (the app moved, or a portable copy was replaced by the
    /// installed one): it is written again for `exe`. True when it was written. Nothing happens when there is no
    /// Run value (Start with Windows is off, or a task does it).
    pub fn follow(&self, exe: &Path) -> Result<bool, String> {
        if self.run_value().is_none() || self.points_at(exe) {
            return Ok(false);
        }
        self.set_run_value(exe).map(|()| true)
    }

    /// Turns it on for `exe`: the Run value, also when this process runs as administrator (see the module docs).
    pub fn enable(&self, exe: &Path) -> Result<(), String> {
        self.set_run_value(exe)
    }

    /// Turns it off: removes an old task (only an elevated DialogReader can), then the Run value. When the task cannot
    /// be removed, nothing is changed.
    pub fn disable(&self) -> Result<(), String> {
        if self.has_task() {
            schtasks(&["/Delete", "/F", "/TN", &self.name]).map_err(|e| {
                format!(
                    "it was turned on by DialogReader running as administrator, so it can only be turned off \
                     that way ({e})"
                )
            })?;
        }
        self.remove_run_value()
    }
}

/// Runs `schtasks.exe` without a console window. `Err` with its output when it fails.
fn schtasks(args: &[&str]) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let out = std::process::Command::new("schtasks.exe")
        .args(args)
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("schtasks could not start: {e}"))?;
    if out.status.success() {
        Ok(())
    } else {
        Err(format!(
            "schtasks {}: {}",
            args.first().copied().unwrap_or(""),
            String::from_utf8_lossy(&out.stderr).trim()
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_quotes_the_path_and_starts_minimized() {
        assert_eq!(
            Autostart::command(Path::new(
                r"C:\Users\me\AppData\Local\Programs\DialogReader\DialogReader.exe"
            )),
            r#""C:\Users\me\AppData\Local\Programs\DialogReader\DialogReader.exe" --minimized"#
        );
    }
}
