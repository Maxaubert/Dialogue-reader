//! WinRT OCR reads the fixture PNGs in `tests/fixtures/` (Segoe UI 30 px, made with Pillow). The test skips with a
//! clear message when this PC has no en-US OCR language pack.

#![cfg(windows)]

use std::path::Path;

use dr_core::Ocr;
use dr_ocr::WinOcr;

fn engine() -> Option<WinOcr> {
    if !WinOcr::has_language("en-US") {
        eprintln!(
            "skipped: no en-US OCR language pack on this PC (Settings > Time & language > Language)"
        );
        return None;
    }
    Some(WinOcr::new("en-US").expect("an engine for an installed language"))
}

fn read(eng: &mut WinOcr, name: &str) -> (Vec<String>, Vec<dr_core::OcrLine>, (u32, u32)) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name);
    let img = image::open(&path)
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()))
        .to_rgba8();
    let (w, h) = img.dimensions();
    let lines = eng.recognize(w, h, img.as_raw()).expect("OCR runs");
    (
        lines.iter().map(|l| l.text.to_lowercase()).collect(),
        lines,
        (w, h),
    )
}

#[test]
fn reads_the_fixtures() {
    let Some(mut eng) = engine() else { return };
    assert_eq!(eng.language(), "en-US");
    assert!(eng.max_dim() >= 1000, "max_dim {}", eng.max_dim());

    let (text, lines, (w, h)) = read(&mut eng, "iron-ore.png");
    assert_eq!(text, ["iron ore"]);
    let r = lines[0].rect;
    assert!(r.w > 60.0 && r.h > 10.0, "{r:?}");
    assert!(
        r.x >= 0.0 && r.y >= 0.0 && r.right() <= w as f32 && r.bottom() <= h as f32,
        "{r:?}"
    );

    let (text, _, _) = read(&mut eng, "two-lines.png");
    assert_eq!(
        text,
        [
            "we need to get these horses moving",
            "before the snow buries us."
        ]
    );

    let (text, _, _) = read(&mut eng, "dark-on-light.png");
    assert_eq!(text, ["press e to talk"]);
}

#[test]
fn a_blank_image_gives_no_lines() {
    let Some(mut eng) = engine() else { return };
    let rgba = [10u8, 10, 10, 255].repeat(200 * 50);
    assert!(eng.recognize(200, 50, &rgba).unwrap().is_empty());
}

#[test]
fn a_bad_language_tag_is_an_error() {
    assert!(WinOcr::new("not a tag!").is_err());
}
