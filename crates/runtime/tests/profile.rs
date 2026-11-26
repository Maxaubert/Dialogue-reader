//! Profiles on disk: round trip, broken files kept as .bad, migration, and zone scaling into `dr_core::Zone`.
//! Every test uses a temp folder, never the real %APPDATA%.

use dr_runtime::profile::{self, LoadError};
use dr_runtime::{Kind, Mode, Paths, Profile, ZoneCfg};

fn temp() -> (tempfile::TempDir, Paths) {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::at(dir.path().join("DialogReader"));
    (dir, paths)
}

/// The example of spec 2.8, drawn on a 4K screenshot.
fn rdr2() -> Profile {
    let mut p = Profile::new("rdr2", "Red Dead Redemption 2", "RDR2.exe");
    p.mode = Mode::Manual;
    p.voice.id = "David".into();
    p.voice.speed = 1.1;
    p.zones = vec![
        ZoneCfg {
            name: "Subtitles".into(),
            kind: Kind::Dialogue,
            on: true,
            rect: [700.0, 1880.0, 2440.0, 160.0],
            base: [3840, 2160],
        },
        ZoneCfg {
            name: "Speaker name".into(),
            kind: Kind::Speaker,
            on: true,
            rect: [1424.0, 1808.0, 352.0, 88.0],
            base: [3840, 2160],
        },
        ZoneCfg {
            name: "Button prompts".into(),
            kind: Kind::Prompt,
            on: false,
            rect: [2456.0, 1380.0, 568.0, 136.0],
            base: [3840, 2160],
        },
    ];
    p
}

#[test]
fn round_trip_through_disk() {
    let (_d, paths) = temp();
    let p = rdr2();
    profile::save(&paths, &p).unwrap();
    assert!(paths.profile_json("rdr2").exists());
    assert!(!paths.profiles().join("rdr2.json.tmp").exists());
    let back = profile::load(&paths.profile_json("rdr2")).unwrap();
    assert_eq!(back, p);
    let (all, bad) = profile::load_all(&paths);
    assert_eq!(all, vec![p]);
    assert!(bad.is_empty());
}

#[test]
fn file_uses_the_spec_names() {
    let json = rdr2().to_json();
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(v["version"], 1);
    assert_eq!(v["match"]["exe"], "RDR2.exe");
    assert_eq!(v["mode"], "manual");
    assert_eq!(v["keys"]["read"], "Mouse 4");
    assert_eq!(v["keys"]["quick_menu"], "F7");
    assert_eq!(v["voice"]["engine"], "windows");
    assert_eq!(v["ocr_lang"], "en-US");
    assert_eq!(v["zones"][2]["kind"], "prompt");
    assert_eq!(v["zones"][0]["base"], serde_json::json!([3840, 2160]));
}

#[test]
fn broken_file_is_kept_as_bad_and_skipped() {
    let (_d, paths) = temp();
    profile::save(&paths, &rdr2()).unwrap();
    std::fs::write(paths.profile_json("broken"), "{ not json").unwrap();
    let (all, bad) = profile::load_all(&paths);
    assert_eq!(all.len(), 1);
    assert_eq!(bad.len(), 1);
    assert!(matches!(bad[0].error, LoadError::Broken(_)));
    let kept = paths.profiles().join("broken.json.bad");
    assert_eq!(bad[0].kept_as.as_deref(), Some(kept.as_path()));
    assert!(kept.exists());
    assert!(!paths.profile_json("broken").exists());
    // the next start finds only the good profile and no problem
    let (all, bad) = profile::load_all(&paths);
    assert_eq!((all.len(), bad.len()), (1, 0));
}

#[test]
fn newer_file_is_left_alone() {
    let (_d, paths) = temp();
    std::fs::create_dir_all(paths.profiles()).unwrap();
    let f = paths.profile_json("future");
    std::fs::write(&f, r#"{"version": 7, "id": "future"}"#).unwrap();
    let e = profile::load(&f).unwrap_err();
    assert_eq!(e.error, LoadError::Newer(7));
    assert!(e.kept_as.is_none());
    assert!(f.exists());
}

#[test]
fn the_file_name_is_the_id() {
    let (_d, paths) = temp();
    // a copied file whose id points somewhere else, even out of the folder
    let mut p = rdr2();
    p.id = "..\\x".into();
    std::fs::create_dir_all(paths.profiles()).unwrap();
    std::fs::write(paths.profiles().join("copy.JSON"), p.to_json()).unwrap();
    let (all, bad) = profile::load_all(&paths);
    assert!(bad.is_empty());
    assert_eq!(all.len(), 1, "the extension case does not matter");
    assert_eq!(all[0].id, "copy");
    // saved again, it goes back to the same file
    profile::save(&paths, &all[0]).unwrap();
    let names: Vec<String> = std::fs::read_dir(paths.profiles())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().to_lowercase())
        .collect();
    assert_eq!(names, ["copy.json"]);
}

#[test]
fn ids_on_disk_count_files_that_were_not_loaded() {
    let (_d, paths) = temp();
    std::fs::create_dir_all(paths.profiles()).unwrap();
    std::fs::write(paths.profile_json("Future"), r#"{"version": 7}"#).unwrap();
    std::fs::write(paths.profiles().join("old.json.bad"), "x").unwrap();
    std::fs::write(paths.profiles().join("pic.png"), "x").unwrap();
    std::fs::write(paths.profiles().join("notes.txt"), "x").unwrap();
    let mut ids = profile::ids_on_disk(&paths);
    ids.sort();
    assert_eq!(ids, ["future", "old", "pic"]);
    assert_eq!(profile::new_id("Future", &ids), "future-2");
}

#[test]
fn version_0_migrates() {
    // no version field, UI kind names, optional fields left out
    let old = r#"{
        "id": "satis", "name": "Satisfactory", "match": { "exe": "FactoryGame.exe", "title_contains": null },
        "mode": "auto",
        "zones": [
            { "name": "Pickups", "kind": "pickups", "on": true, "rect": [10, 20, 300, 40], "base": [2560, 1440] },
            { "name": "Hints", "kind": "prompts", "on": true, "rect": [10, 80, 300, 40], "base": [2560, 1440] }
        ]
    }"#;
    let p = Profile::from_json(old).unwrap();
    assert_eq!(p.version, 1);
    assert_eq!(p.zones[0].kind, Kind::Notification);
    assert_eq!(p.zones[1].kind, Kind::Prompt);
    assert_eq!(p.keys.read, "Mouse 4");
    assert_eq!(p.ocr_lang, "en-US");
    // saved again, it is a version 1 file
    assert!(p.to_json().contains("\"version\": 1"));
}

#[test]
fn zones_scale_to_any_frame_size() {
    let p = rdr2();
    let zones = p.core_zones();
    // the switched-off prompt zone is not passed to dr-core
    assert_eq!(zones.len(), 2);
    let sub = &zones[0];
    assert_eq!(sub.kind, dr_core::ZoneKind::Dialogue);
    assert_eq!(sub.base, Some((3840, 2160)));
    // 4K: the rect as drawn
    assert_eq!(
        sub.rect_in(3840, 2160),
        dr_core::Rect::new(700.0, 1880.0, 2440.0, 160.0)
    );
    // 1440p: two thirds
    let r = sub.rect_in(2560, 1440);
    for (a, b) in [
        (r.x, 466.6667),
        (r.y, 1253.3334),
        (r.w, 1626.6667),
        (r.h, 106.66667),
    ] {
        assert!((a - b).abs() < 1e-3, "{a} vs {b}");
    }
    // 1080p: half
    assert_eq!(
        sub.rect_in(1920, 1080),
        dr_core::Rect::new(350.0, 940.0, 1220.0, 80.0)
    );
    // ZoneCfg::rect_in gives the same numbers
    assert_eq!(p.zones[0].rect_in(1920, 1080), [350.0, 940.0, 1220.0, 80.0]);
}

#[test]
fn zones_drawn_at_1080p_scale_up() {
    let z = ZoneCfg {
        name: "d".into(),
        kind: Kind::Dialogue,
        on: true,
        rect: [100.0, 900.0, 800.0, 60.0],
        base: [1920, 1080],
    };
    assert_eq!(
        z.core().rect_in(3840, 2160),
        dr_core::Rect::new(200.0, 1800.0, 1600.0, 120.0)
    );
    assert!((z.core().rect_in(2560, 1440).w - 1066.6666).abs() < 1e-3);
}
