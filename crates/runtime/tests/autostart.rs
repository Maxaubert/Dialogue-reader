//! Start with Windows (M5): the Run value is written and removed. The test uses its own value name and removes
//! it at the end, also when an assert fails; the app's `DialogReader` value is never touched.

#![cfg(windows)]

use std::path::Path;

use dr_runtime::autostart::{APP_VALUE, Autostart};

/// Removes the test value when the test ends, pass or fail.
struct Cleanup(Autostart);

impl Drop for Cleanup {
    fn drop(&mut self) {
        let _ = self.0.remove_run_value();
    }
}

#[test]
fn run_value_is_written_and_removed() {
    let name = format!("DialogReader.test-{}", std::process::id());
    assert_ne!(name, APP_VALUE);
    let a = Autostart::named(&name);
    let _cleanup = Cleanup(a.clone());
    let app_before = Autostart::app().run_value();

    assert_eq!(a.run_value(), None);
    let exe = Path::new(r"C:\Program Files\Test Folder\DialogReader.exe");
    a.enable(exe, false).unwrap();
    assert_eq!(
        a.run_value().as_deref(),
        Some(r#""C:\Program Files\Test Folder\DialogReader.exe" --minimized"#)
    );
    assert!(a.is_on());
    assert!(a.points_at(exe));
    assert!(!a.points_at(Path::new(r"C:\Elsewhere\DialogReader.exe")));

    a.disable().unwrap();
    assert_eq!(a.run_value(), None);
    assert!(!a.is_on());
    // removing twice is fine
    a.remove_run_value().unwrap();

    // the app's own value is as it was
    assert_eq!(Autostart::app().run_value(), app_before);
}

#[test]
fn the_run_value_follows_the_exe() {
    let name = format!("DialogReader.test-follow-{}", std::process::id());
    let a = Autostart::named(&name);
    let _cleanup = Cleanup(a.clone());
    let app_before = Autostart::app().run_value();
    let old = Path::new(r"C:\Portable\DialogReader.exe");
    let new = Path::new(r"C:\Users\me\AppData\Local\Programs\DialogReader\DialogReader.exe");

    // off: nothing is written
    assert_eq!(a.follow(new), Ok(false));
    assert_eq!(a.run_value(), None);

    // on for the old place: written again for the new one
    a.set_run_value(old).unwrap();
    assert_eq!(a.follow(new), Ok(true));
    assert!(a.points_at(new));
    // already right: left alone
    assert_eq!(a.follow(new), Ok(false));

    a.disable().unwrap();
    assert_eq!(a.run_value(), None);
    assert_eq!(Autostart::app().run_value(), app_before);
}

/// Makes a real sign-in task (normal rights, a test name, a path that does not exist) and removes it. Ignored in
/// the gate because it changes Task Scheduler; run it by hand: `cargo test -p dr-runtime --test autostart --
/// --ignored`.
#[test]
#[ignore]
fn a_sign_in_task_is_made_and_removed() {
    let name = format!("DialogReader.test-task-{}", std::process::id());
    let a = Autostart::named(&name);
    struct Gone(String);
    impl Drop for Gone {
        fn drop(&mut self) {
            let _ = std::process::Command::new("schtasks.exe")
                .args(["/Delete", "/F", "/TN", &self.0])
                .output();
        }
    }
    let _gone = Gone(name.clone());
    let exe = Path::new(r"C:\Does Not Exist\Test Folder\DialogReader.exe");
    a.make_task(exe, false).unwrap();
    assert!(a.has_task());
    assert!(a.is_on());
    let out = std::process::Command::new("schtasks.exe")
        .args(["/Query", "/TN", &name, "/XML"])
        .output()
        .unwrap();
    let xml = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(
        xml.contains(r"<Command>C:\Does Not Exist\Test Folder\DialogReader.exe</Command>"),
        "{xml}"
    );
    assert!(xml.contains("<Arguments>--minimized</Arguments>"), "{xml}");
    assert!(
        xml.contains("<ExecutionTimeLimit>PT0S</ExecutionTimeLimit>"),
        "{xml}"
    );
    a.disable().unwrap();
    assert!(!a.has_task());
}
