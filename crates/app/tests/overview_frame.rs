//! The Overview frame at Max's maximized window (1707 x 912, headless, no window): a 16:9 picture fills the
//! frame to its edge (a thin band of the frame color read as a second frame, Max 2026-11-24), and with no zone
//! chips the picture frame and the Now panel run down to the page margin.

use std::path::Path;

use image::RgbaImage;

const W: u32 = 1707;
const H: u32 = 912;

fn near(a: [u8; 4], b: [u8; 3], d: u8) -> bool {
    a.iter().zip(b).all(|(x, y)| x.abs_diff(y) <= d)
}

fn shot(files: &[std::path::PathBuf], name: &str) -> RgbaImage {
    let f = files
        .iter()
        .find(|f| f.file_stem().is_some_and(|s| s == name))
        .unwrap_or_else(|| panic!("no render {name}"));
    image::open(f).expect("png").to_rgba8()
}

#[test]
fn the_frame_at_1707_has_no_thin_band_and_runs_down_without_chips() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("overview-frame-1707");
    let files = dr_app::screens::render_all_at(&out, W, H).expect("render");
    // the frame band color (surface-1): the band above a picture that is wider than 16:9
    let band = shot(&files, "10-overview-real-shot").get_pixel(500, 110).0;
    assert!(near(band, [10, 10, 10], 3), "band {band:?}");

    // a 16:9 picture: just inside the frame's left edge (x 24) is the picture, not the band
    let img = shot(&files, "10-overview-real-shot-16x9");
    for x in 26..32 {
        let p = img.get_pixel(x, 450).0;
        assert!(!near(p, [10, 10, 10], 6), "x {x}: {p:?} is the frame band");
        assert!(!near(p, [0, 0, 0], 6), "x {x}: {p:?} is the page");
    }

    // no chips: the picture frame and the Now panel both reach 24 px above the bottom
    let img = shot(&files, "10-overview-no-chips");
    for x in [500, 1500] {
        let inside = img.get_pixel(x, H - 24 - 4).0;
        let under = img.get_pixel(x, H - 24 + 4).0;
        assert!(!near(inside, [0, 0, 0], 2), "x {x}: {inside:?} ends early");
        assert!(
            near(under, [0, 0, 0], 2),
            "x {x}: {under:?} runs into the margin"
        );
    }
}
