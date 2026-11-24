//! Every page and overlay renders headless (software renderer, no window) at 1440 x 900, is not blank, and
//! matches its approved mockup shot within a tolerance.
//!
//! The mockup comparison is tolerant on purpose. Chromium draws text with ClearType and Slint draws grayscale
//! text, so single pixels never match. Both pictures are averaged over 10 x 10 px cells first; then a cell
//! differs when its color moves by more than 32 (of 255). That catches a moved, missing or recolored block, a
//! wrong background or a wrong size. It does not catch a font weight or a 1 to 2 px shift: check those in the
//! side-by-side review pictures.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use image::RgbaImage;

/// Renders all screens once per test process (the headless platform can be installed only once).
fn rendered() -> &'static Vec<PathBuf> {
    static FILES: OnceLock<Vec<PathBuf>> = OnceLock::new();
    FILES.get_or_init(|| {
        let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("screens-test");
        dr_app::screens::render_all(&out).expect("render")
    })
}

fn shots_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../design/mockups/d-black/shots")
}

#[test]
fn every_screen_renders_and_is_not_blank() {
    let files = rendered();
    assert_eq!(files.len(), dr_app::screens::all().len());
    for f in files {
        let img = image::open(f).expect("png").to_rgba8();
        assert_eq!(
            (img.width(), img.height()),
            (dr_app::screens::WIDTH, dr_app::screens::HEIGHT),
            "{}",
            f.display()
        );
        let colors: HashSet<[u8; 4]> = img.pixels().step_by(7).map(|p| p.0).collect();
        assert!(
            colors.len() > 50,
            "{} looks blank ({} colors)",
            f.display(),
            colors.len()
        );
        // the window background is fully opaque everywhere
        assert!(
            img.pixels().all(|p| p.0[3] == 255),
            "{} has see-through pixels",
            f.display()
        );
    }
}

#[test]
fn screen_names_match_the_mockups() {
    let names: Vec<_> = dr_app::screens::all().iter().map(|s| s.name).collect();
    for mockup in MOCKUPS {
        assert!(
            names.contains(&mockup.name),
            "no render for {}",
            mockup.name
        );
    }
}

/// How closely one screen must match its d-black mockup shot.
struct Mockup {
    /// the render (`src/screens.rs`)
    name: &'static str,
    /// the shot in design/mockups/d-black/shots
    shot: &'static str,
    /// highest allowed mean cell difference (0 to 255)
    mean: f64,
    /// highest allowed share of cells that differ by more than 32, in percent
    changed: f64,
    /// a setup step sits in the middle of the window, the shot has it at the top: the page between the title
    /// strip and the footer is `dy` px lower in the render than in the shot (0: no move)
    dy: u32,
    /// the height of the footer at the bottom, which does not move with the page
    foot: u32,
}

const fn m(name: &'static str, shot: &'static str, mean: f64, changed: f64) -> Mockup {
    Mockup {
        name,
        shot,
        mean,
        changed,
        dy: 0,
        foot: 0,
    }
}

/// A setup step: its page is `dy` px lower than in the shot, above a `foot` px footer (Welcome has none).
const fn step(
    name: &'static str,
    shot: &'static str,
    (dy, foot): (u32, u32),
    mean: f64,
    changed: f64,
) -> Mockup {
    Mockup {
        name,
        shot,
        mean,
        changed,
        dy,
        foot,
    }
}

/// Every screen with a d-black shot. The limits sit 0.4 above the values measured on 2026-11-24, so a real
/// change fails and text rendering noise does not. The stages draw the SVG scene, not the shot's bitmap, and the
/// fake read list has other lines than the mockup, so 03, 10 and 11 sit higher. Re-measured the same day after
/// Max's follow-ups: the setup steps (01 to 06, 18-voice-none) sit in the middle of the window now, not at the
/// top as in the shots, so their page is compared `dy` px lower (the title strip and the footer stay put). The
/// step 5 summary and the step 1 pictures are gone, and the Zones toolbar has the Overview's margins and no
/// camera, so those limits are a little higher. Later the same day Max asked for "Edit zones" (Overview) and
/// "Done" (Zones) as the white primary pill: the Zones shots match better, and the Overview and the switcher
/// over it (the mockups have a grey "Edit zones") were re-measured.
const MOCKUPS: [Mockup; 22] = [
    step("01-welcome", "01-setup-0-welcome", (243, 0), 0.8, 0.6),
    step("02-pick-window", "01-setup-1-game", (15, 72), 1.2, 1.0),
    step("03-draw-zones", "01-setup-2-zones", (14, 72), 1.9, 0.6),
    step("04-read-key-d", "01-setup-3-keys", (261, 72), 0.6, 0.4),
    step("05-voice", "01-setup-4-voice", (92, 72), 1.7, 0.8),
    step(
        "18-voice-none",
        "01-setup-4-voice-none",
        (250, 72),
        0.6,
        0.5,
    ),
    step("06-mode", "01-setup-5-mode", (262, 72), 1.0, 1.1),
    m("10-main-b", "10-overview", 2.9, 2.5),
    m("19-overview-not-running", "10-overview-off", 0.6, 0.5),
    m("11-zones", "11-zones", 3.2, 1.0),
    m("11-zones-top", "11-zones-top", 3.5, 0.9),
    m("11-zones-empty", "11-zones-empty", 1.6, 0.9),
    m("12-settings", "12-settings", 1.7, 1.3),
    m("13-overlay", "13-pill", 0.5, 0.4),
    m("14-game-switcher", "14-game-switcher", 1.8, 0.9),
    m("15-quick-menu-a", "15-quick-menu", 1.2, 1.1),
    m("15-quick-menu-voice", "15-quick-menu-voice", 1.6, 1.5),
    m("16-zone-edit", "16-zone-edit", 1.0, 0.5),
    m("16-zone-edit-new", "16-zone-edit-new", 1.3, 0.7),
    m("16-zone-edit-top", "16-zone-edit-top", 1.3, 0.7),
    m("17-activity", "17-activity", 1.6, 0.7),
    m("17-activity-empty", "17-activity-empty", 0.6, 0.6),
];

const CELL: u32 = 10;

/// Mean color of each CELL x CELL block.
fn cells(img: &RgbaImage, (x0, y0, w, h): (u32, u32, u32, u32)) -> Vec<[f64; 3]> {
    let mut out = Vec::new();
    for cy in 0..h / CELL {
        for cx in 0..w / CELL {
            let mut sum = [0f64; 3];
            for y in 0..CELL {
                for x in 0..CELL {
                    let p = img.get_pixel(x0 + cx * CELL + x, y0 + cy * CELL + y).0;
                    for c in 0..3 {
                        sum[c] += p[c] as f64;
                    }
                }
            }
            out.push(sum.map(|v| v / (CELL * CELL) as f64));
        }
    }
    out
}

/// The title strip height: it never moves with the page.
const STRIP: u32 = 40;

/// The shot cut to `h` px, with its page (under the title strip, above a `foot` px footer) moved `dy` px down.
/// The rows the move uncovers take the page background (the shot's last page row).
fn moved(shot: &RgbaImage, h: u32, dy: u32, foot: u32) -> RgbaImage {
    let mut out = image::imageops::crop_imm(shot, 0, 0, shot.width(), h).to_image();
    if dy == 0 {
        return out;
    }
    let bottom = h - foot;
    for y in STRIP..bottom {
        let src = if y >= STRIP + dy { y - dy } else { bottom - 1 };
        for x in 0..shot.width() {
            out.put_pixel(x, y, *shot.get_pixel(x, src));
        }
    }
    out
}

#[test]
fn screens_match_the_mockup_shots() {
    let files = rendered();
    let mut failures = Vec::new();
    for mk in &MOCKUPS {
        let shot = shots_dir().join(format!("{}.png", mk.shot));
        let mock = image::open(&shot)
            .unwrap_or_else(|e| panic!("mockup shot {}: {e}", shot.display()))
            .to_rgba8();
        let path = files
            .iter()
            .find(|f| f.file_stem().is_some_and(|s| s == mk.name))
            .expect("render");
        let ours = image::open(path).expect("png").to_rgba8();
        // a few shots are taller than the window (a page scrolled in the mockup): compare the window part
        assert_eq!(mock.width(), ours.width(), "{}", mk.name);
        assert!(mock.height() >= ours.height(), "{}", mk.name);
        let mock = moved(&mock, ours.height(), mk.dy, mk.foot);
        let region = (0, 0, ours.width(), ours.height());
        let (a, b) = (cells(&mock, region), cells(&ours, region));
        let diffs: Vec<f64> = a
            .iter()
            .zip(&b)
            .map(|(p, q)| (0..3).map(|c| (p[c] - q[c]).abs()).fold(0.0, f64::max))
            .collect();
        let mean = diffs.iter().sum::<f64>() / diffs.len() as f64;
        let changed =
            100.0 * diffs.iter().filter(|&&d| d > 32.0).count() as f64 / diffs.len() as f64;
        println!(
            "{:22} mean {mean:5.2} (max {:4.1})  changed {changed:5.2}% (max {:4.1}%)",
            mk.name, mk.mean, mk.changed
        );
        if mean > mk.mean || changed > mk.changed {
            failures.push(format!(
                "{}: mean {mean:.2} > {} or changed {changed:.2}% > {}%",
                mk.name, mk.mean, mk.changed
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "screens differ from the mockups:\n{}",
        failures.join("\n")
    );
}
