//! Start with Windows (spec 2.9, M5).
//!
//! Normally a value under `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`: `DialogReader` =
//! `"<path>\DialogReader.exe" --minimized`. No admin rights are needed and the installer removes it on uninstall.
//!
//! When DialogReader itself runs as administrator (for a game that runs as administrator, spec 4), Windows would
//! start it from the Run key without admin rights at the next sign-in. So an elevated DialogReader uses a Task
//! Scheduler task with the highest privileges instead (task name = the value name), and removes the Run value. The
//! task is made from an XML definition ([`task_xml`]): no time limit (the schtasks default stops it after 72
//! hours) and it also starts on battery. Turning it off removes the task first, then the Run value, so a failure
//! changes nothing.
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
/// The app's value name (and task name when elevated).
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

    /// The scheduled task exists (only an elevated DialogReader makes one).
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

    /// Turns it on for `exe`: the Run value, or the task when this process runs as administrator.
    pub fn enable(&self, exe: &Path, elevated: bool) -> Result<(), String> {
        if elevated {
            self.make_task(exe, true)?;
            self.remove_run_value()
        } else {
            self.set_run_value(exe)
        }
    }

    /// Makes (or replaces) the sign-in task for `exe`, with the highest privileges when `highest`.
    pub fn make_task(&self, exe: &Path, highest: bool) -> Result<(), String> {
        let xml = task_xml(exe, current_user().as_deref(), highest);
        let file = std::env::temp_dir().join(format!(
            "dialogreader-task-{}-{}.xml",
            std::process::id(),
            self.name.replace(|c: char| !c.is_ascii_alphanumeric(), "-")
        ));
        // Task Scheduler reads UTF-16 with a byte order mark
        let mut bytes = vec![0xFF, 0xFE];
        bytes.extend(xml.encode_utf16().flat_map(u16::to_le_bytes));
        std::fs::write(&file, bytes)
            .map_err(|e| format!("the task file could not be written: {e}"))?;
        let r = schtasks(&[
            "/Create",
            "/F",
            "/TN",
            &self.name,
            "/XML",
            &file.to_string_lossy(),
        ]);
        let _ = std::fs::remove_file(&file);
        r
    }

    /// Turns it off: removes the task (only an elevated DialogReader can), then the Run value. When the task cannot
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

/// `DOMAIN\user` of this process, from the environment. `None` when it is not set.
fn current_user() -> Option<String> {
    let user = std::env::var("USERNAME").ok().filter(|u| !u.is_empty())?;
    Some(
        match std::env::var("USERDOMAIN").ok().filter(|d| !d.is_empty()) {
            Some(d) => format!("{d}\\{user}"),
            None => user,
        },
    )
}

/// Escapes text for an XML element.
fn xml_text(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// The Task Scheduler definition: at sign-in of `user` (any user when `None`), run `exe --minimized`, with the
/// highest privileges when `highest`. No time limit, starts and keeps running on battery, one copy at a time.
pub fn task_xml(exe: &Path, user: Option<&str>, highest: bool) -> String {
    let user_id = user
        .map(|u| format!("<UserId>{}</UserId>", xml_text(u)))
        .unwrap_or_default();
    let level = if highest {
        "HighestAvailable"
    } else {
        "LeastPrivilege"
    };
    format!(
        r#"<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.2" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <RegistrationInfo>
    <Description>Starts DialogReader when you sign in.</Description>
  </RegistrationInfo>
  <Triggers>
    <LogonTrigger>
      <Enabled>true</Enabled>
      {user_id}
    </LogonTrigger>
  </Triggers>
  <Principals>
    <Principal id="Author">
      {user_id}
      <LogonType>InteractiveToken</LogonType>
      <RunLevel>{level}</RunLevel>
    </Principal>
  </Principals>
  <Settings>
    <MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>
    <DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>
    <StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>
    <ExecutionTimeLimit>PT0S</ExecutionTimeLimit>
    <Enabled>true</Enabled>
  </Settings>
  <Actions Context="Author">
    <Exec>
      <Command>{exe}</Command>
      <Arguments>{MINIMIZED_ARG}</Arguments>
    </Exec>
  </Actions>
</Task>
"#,
        exe = xml_text(&exe.display().to_string()),
    )
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
    fn the_task_has_no_time_limit_and_runs_on_battery() {
        let x = task_xml(
            Path::new(r"C:\Program Files\A & B\DialogReader.exe"),
            Some(r"PC\Max"),
            true,
        );
        assert!(x.contains(r"<Command>C:\Program Files\A &amp; B\DialogReader.exe</Command>"));
        assert!(x.contains("<Arguments>--minimized</Arguments>"));
        assert!(x.contains("<ExecutionTimeLimit>PT0S</ExecutionTimeLimit>"));
        assert!(x.contains("<DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>"));
        assert!(x.contains("<StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>"));
        assert!(x.contains("<RunLevel>HighestAvailable</RunLevel>"));
        assert_eq!(x.matches(r"<UserId>PC\Max</UserId>").count(), 2);
        let any = task_xml(Path::new(r"C:\x.exe"), None, false);
        assert!(!any.contains("<UserId>"));
        assert!(any.contains("<RunLevel>LeastPrivilege</RunLevel>"));
    }

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
