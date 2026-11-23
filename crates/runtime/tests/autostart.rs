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
    a.enable(exe).unwrap();
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
