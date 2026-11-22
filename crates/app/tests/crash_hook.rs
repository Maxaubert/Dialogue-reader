//! The panic hook writes a crash log into the folder it is given (a temp folder here). The abort of the ship
//! profile is not exercised by tests: they build with unwinding.

#[test]
fn a_panic_writes_a_crash_log() {
    let dir = tempfile::tempdir().unwrap();
    dr_app::crash::install(Some(dir.path().to_path_buf()));
    let r = std::thread::spawn(|| panic!("test panic for the crash log")).join();
    assert!(r.is_err());
    let logs: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .filter_map(Result::ok)
        .collect();
    assert_eq!(logs.len(), 1);
    let name = logs[0].file_name().to_string_lossy().into_owned();
    assert!(
        name.starts_with("crash-") && name.ends_with(".log"),
        "{name}"
    );
    let text = std::fs::read_to_string(logs[0].path()).unwrap();
    assert!(
        text.contains("message: test panic for the crash log"),
        "{text}"
    );
    assert!(text.contains("crash_hook.rs"), "{text}");
}
