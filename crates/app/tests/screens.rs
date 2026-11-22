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
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../design/mockups/c-midnight/shots")
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

/// How closely one screen must match its mockup shot.
struct Mockup {
    name: &'static str,
    /// highest allowed mean cell difference (0 to 255)
    mean: f64,
    /// highest allowed share of cells that differ by more than 32, in percent
    changed: f64,
    /// part of the picture to compare (x, y, w, h); None = all of it
    region: Option<(u32, u32, u32, u32)>,
}

const fn m(name: &'static str, mean: f64, changed: f64) -> Mockup {
    Mockup {
        name,
        mean,
        changed,
        region: None,
    }
}

/// Every approved mockup. The limits sit a little above today's values (measured 2026-11-22), so a real change
/// fails and text rendering noise does not. Wider limits have a reason next to them.
const MOCKUPS: [Mockup; 14] = [
    m("01-welcome", 1.6, 0.6),
    m("02-pick-window", 1.3, 0.5),
    m("03-draw-zones", 1.6, 0.6),
    m("04-read-key-d", 1.0, 0.4),
    m("05-voice", 1.3, 0.6),
    // the summary has one more row than the mockup (Quick menu key): compare down to the Read key row
    Mockup {
        name: "06-mode",
        mean: 1.6,
        changed: 0.7,
        region: Some((0, 0, 1440, 590)),
    },
    m("10-main-b", 1.5, 0.6),
    m("11-zones", 1.4, 0.5),
    // Keys (two rows) replaces the mockup's Shortcuts section (four rows): compare above that section.
    // The side list also says "Keys" where the mockup says "Shortcuts".
    Mockup {
        name: "12-settings",
        mean: 1.6,
        changed: 1.0,
        region: Some((0, 0, 1440, 530)),
    },
    m("13-overlay", 0.6, 0.2),
    // the page behind the switcher is 10-main-b, not the older main page of the mockup: compare the panel only
    Mockup {
        name: "14-game-switcher",
        mean: 1.8,
        changed: 0.8,
        region: Some((150, 44, 520, 482)),
    },
    // the software renderer draws no shadows; the panels' wide soft shadows show in the mockups
    m("15-quick-menu-a", 3.0, 2.6),
    m("15-quick-menu-voice", 2.8, 2.5),
    m("16-zone-edit", 2.0, 0.4),
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

#[test]
fn screens_match_the_mockup_shots() {
    let files = rendered();
    let mut failures = Vec::new();
    for mk in &MOCKUPS {
        let shot = shots_dir().join(format!("{}.png", mk.name));
        let mock = image::open(&shot)
            .unwrap_or_else(|e| panic!("mockup shot {}: {e}", shot.display()))
            .to_rgba8();
        let path = files
            .iter()
            .find(|f| f.file_stem().is_some_and(|s| s == mk.name))
            .expect("render");
        let ours = image::open(path).expect("png").to_rgba8();
        assert_eq!(mock.dimensions(), ours.dimensions(), "{}", mk.name);
        let region = mk.region.unwrap_or((0, 0, mock.width(), mock.height()));
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
