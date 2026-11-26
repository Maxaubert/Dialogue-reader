//! The version (M5): the app's version is the workspace version in the root Cargo.toml, `--version` prints it, and
//! DialogReader.exe carries it in its version block (the installer and `release.yml` read the same file).

use std::path::Path;

/// `version` under `[workspace.package]` in the root Cargo.toml.
fn workspace_version() -> String {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../Cargo.toml");
    let text = std::fs::read_to_string(root).unwrap();
    let mut in_pkg = false;
    for line in text.lines() {
        let l = line.trim();
        if l.starts_with('[') {
            in_pkg = l == "[workspace.package]";
        } else if in_pkg && let Some(v) = l.strip_prefix("version") {
            return v
                .trim()
                .trim_start_matches('=')
                .trim()
                .trim_matches('"')
                .to_string();
        }
    }
    panic!("no [workspace.package] version in Cargo.toml");
}

#[test]
fn the_version_is_the_workspace_version() {
    assert_eq!(dr_app::VERSION, workspace_version());
}

#[test]
fn dash_dash_version_prints_it() {
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_DialogReader"))
        .arg("--version")
        .output()
        .expect("DialogReader --version runs");
    assert!(out.status.success());
    assert_eq!(
        String::from_utf8_lossy(&out.stdout).trim(),
        format!("DialogReader {}", workspace_version())
    );
}
