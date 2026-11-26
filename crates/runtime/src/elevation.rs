//! Games DialogReader cannot fully serve (spec section 5, Risks), so the app can say why.
//!
//! - **Elevated games.** A game that runs as administrator: the low-level hooks of a normal app do not see input
//!   that goes to it (UIPI), and overlays may not stay on top. [`needs_admin`] checks the game's process token
//!   (`TokenElevation`); a process that cannot be opened counts as elevated (spec). [`restart_as_admin`]
//!   starts DialogReader again with "runas"; the app has no button for it yet (open question for Max).
//! - **Exclusive fullscreen.** WGC gives black frames or none, and overlays do not show over it.
//!   [`exclusive_fullscreen`] is true when the game's window is in front and Windows says a Direct3D game runs
//!   in exclusive fullscreen (`SHQueryUserNotificationState` = `QUNS_RUNNING_D3D_FULL_SCREEN`). Black frames
//!   alone are not used: loading screens are black too.

use windows::Win32::Foundation::{CloseHandle, HANDLE, HWND};
use windows::Win32::Security::{GetTokenInformation, TOKEN_ELEVATION, TOKEN_QUERY, TokenElevation};
use windows::Win32::System::Threading::{
    GetCurrentProcess, OpenProcess, OpenProcessToken, PROCESS_QUERY_LIMITED_INFORMATION,
};

/// The token of `process` is elevated. None: the token could not be read.
fn token_elevated(process: HANDLE) -> Option<bool> {
    unsafe {
        let mut token = HANDLE::default();
        OpenProcessToken(process, TOKEN_QUERY, &mut token).ok()?;
        let mut e = TOKEN_ELEVATION::default();
        let mut len = 0u32;
        let r = GetTokenInformation(
            token,
            TokenElevation,
            Some(&mut e as *mut TOKEN_ELEVATION as *mut _),
            std::mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut len,
        );
        let _ = CloseHandle(token);
        r.ok()?;
        Some(e.TokenIsElevated != 0)
    }
}

/// DialogReader itself runs as administrator.
pub fn self_elevated() -> bool {
    token_elevated(unsafe { GetCurrentProcess() }).unwrap_or(false)
}

/// The process `pid` runs as administrator. A process that cannot be opened or read counts as elevated (an
/// elevated or protected process refuses a normal one).
pub fn process_elevated(pid: u32) -> bool {
    if pid == 0 {
        return false;
    }
    unsafe {
        let Ok(p) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else {
            return true;
        };
        let e = token_elevated(p).unwrap_or(true);
        let _ = CloseHandle(p);
        e
    }
}

/// The game runs as administrator and DialogReader does not: keys and overlays will not reach it.
pub fn needs_admin(pid: u32) -> bool {
    process_elevated(pid) && !self_elevated()
}

/// Starts DialogReader again as administrator (Windows asks the user first). The caller then quits.
pub fn restart_as_admin(args: &str) -> Result<(), String> {
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
    use windows::core::HSTRING;
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let r = unsafe {
        ShellExecuteW(
            None,
            &HSTRING::from("runas"),
            &HSTRING::from(exe.as_os_str()),
            &HSTRING::from(args),
            None,
            SW_SHOWNORMAL,
        )
    };
    // values above 32 mean success
    if r.0 as isize > 32 {
        Ok(())
    } else {
        Err(format!("Windows did not start it (code {})", r.0 as isize))
    }
}

/// The game window `hwnd` is in front, in exclusive fullscreen.
pub fn exclusive_fullscreen(hwnd: isize) -> bool {
    use windows::Win32::UI::Shell::{QUNS_RUNNING_D3D_FULL_SCREEN, SHQueryUserNotificationState};
    use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;
    let front = unsafe { GetForegroundWindow() };
    if front != HWND(hwnd as *mut _) {
        return false;
    }
    matches!(
        unsafe { SHQueryUserNotificationState() },
        Ok(s) if s == QUNS_RUNNING_D3D_FULL_SCREEN
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn this_process_is_read_and_a_gone_one_counts_as_elevated() {
        let me = std::process::id();
        // the test runs as this process: never "needs admin" for itself
        assert_eq!(process_elevated(me), self_elevated());
        assert!(!needs_admin(me));
        assert!(!process_elevated(0));
        // no process has this id (ids are multiples of 4)
        assert!(process_elevated(4_294_967_291));
        assert!(!exclusive_fullscreen(0));
    }
}
