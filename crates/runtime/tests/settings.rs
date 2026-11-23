//! settings.json (M5): round trip, defaults, a broken file kept as `.bad`, a newer file left alone. Every test
//! uses a temp folder, never the real `%APPDATA%\DialogReader`.

use dr_runtime::Paths;
use dr_runtime::settings::{self, PillAt, Problem, Settings};

fn temp() -> (tempfile::TempDir, Paths) {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::at(dir.path().join("DialogReader"));
    (dir, paths)
}

#[test]
fn no_file_gives_the_spec_defaults() {
    let (_d, paths) = temp();
    let l = settings::load(&paths);
    assert_eq!(l.problem, None);
    assert!(!l.read_only);
    let s = l.settings;
    assert!(
        !s.start_with_windows,
        "start with Windows is off by default (spec 2.9)"
    );
    assert!(s.open_minimized);
    assert!(s.show_in_taskbar);
    assert_eq!(s.pill_position, PillAt::TopRight);
    assert!(s.show_speaker);
}

#[test]
fn settings_round_trip() {
    let (_d, paths) = temp();
    let s = Settings {
        start_with_windows: true,
        open_minimized: false,
        show_in_taskbar: false,
        pill_position: PillAt::BottomCenter,
        show_speaker: false,
        ..Settings::default()
    };
    settings::save(&paths, &s).unwrap();
    let text = std::fs::read_to_string(paths.settings()).unwrap();
    assert!(
        text.contains("\"pill_position\": \"bottom_center\""),
        "{text}"
    );
    let l = settings::load(&paths);
    assert_eq!(l.problem, None);
    assert_eq!(l.settings, s);
    assert!(!paths.root().join("settings.json.tmp").exists());
}

#[test]
fn missing_fields_take_defaults_and_unknown_ones_are_ignored() {
    let s = Settings::from_json(r#"{"version":1,"show_speaker":false,"later_field":3}"#).unwrap();
    assert!(!s.show_speaker);
    assert!(s.open_minimized);
    assert_eq!(s.pill_position, PillAt::TopRight);
}

#[test]
fn a_broken_file_is_kept_as_bad_and_defaults_are_used() {
    let (_d, paths) = temp();
    std::fs::create_dir_all(paths.root()).unwrap();
    std::fs::write(paths.settings(), "{ not json").unwrap();
    let l = settings::load(&paths);
    assert_eq!(l.settings, Settings::default());
    assert!(
        matches!(l.problem, Some(Problem::Broken { kept: true, .. })),
        "{:?}",
        l.problem
    );
    assert!(!l.read_only);
    assert!(paths.root().join("settings.json.bad").exists());
    assert!(!paths.settings().exists());
    assert!(l.problem.unwrap().text().contains("settings.json.bad"));
}

#[test]
fn a_newer_file_is_not_touched() {
    let (_d, paths) = temp();
    std::fs::create_dir_all(paths.root()).unwrap();
    let newer = r#"{"version":9,"show_speaker":false}"#;
    std::fs::write(paths.settings(), newer).unwrap();
    let l = settings::load(&paths);
    assert_eq!(l.problem, Some(Problem::Newer(9)));
    assert!(l.read_only);
    assert_eq!(l.settings, Settings::default());
    assert_eq!(std::fs::read_to_string(paths.settings()).unwrap(), newer);
}

#[test]
fn pill_position_index_matches_the_segmented_control() {
    for p in [PillAt::TopRight, PillAt::BottomCenter, PillAt::Off] {
        assert_eq!(PillAt::of_index(p.index()), p);
    }
    assert_eq!(PillAt::of_index(-1), PillAt::TopRight);
}

#[test]
fn a_file_that_is_not_utf8_is_kept_as_bad() {
    let (_d, paths) = temp();
    std::fs::create_dir_all(paths.root()).unwrap();
    // UTF-16 with a byte order mark, as some editors save it
    let mut bytes = vec![0xFF, 0xFE];
    bytes.extend("{}".encode_utf16().flat_map(u16::to_le_bytes));
    std::fs::write(paths.settings(), &bytes).unwrap();
    let l = settings::load(&paths);
    assert!(
        matches!(l.problem, Some(Problem::Broken { kept: true, .. })),
        "{:?}",
        l.problem
    );
    assert!(!l.read_only, "the defaults can be saved again");
    assert_eq!(l.settings, Settings::default());
    assert!(!paths.settings().exists());
    assert_eq!(
        std::fs::read(paths.root().join("settings.json.bad")).unwrap(),
        bytes
    );
}

#[test]
fn a_utf8_byte_order_mark_is_fine() {
    let (_d, paths) = temp();
    std::fs::create_dir_all(paths.root()).unwrap();
    std::fs::write(paths.settings(), "\u{feff}{\"show_speaker\": false}").unwrap();
    let l = settings::load(&paths);
    assert_eq!(l.problem, None);
    assert!(!l.settings.show_speaker);
}
