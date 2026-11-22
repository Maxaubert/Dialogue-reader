//! Probe: OCR one crop of a PNG frame with several dr-core clean-up settings.
//! Usage: cargo run -p arena --release --example a1_probe -- <png> <x> <y> <w> <h>
use arena::ocr;
use arena::readers::a1::EngineOcr;
use dr_core::geom::Rect;
use dr_core::image::{Channel, Gray, Prep};
use dr_core::pipeline::ocr_gray;

fn main() -> anyhow::Result<()> {
    let a: Vec<String> = std::env::args().collect();
    let img = image::open(&a[1])?.to_rgba8();
    let (fw, fh) = img.dimensions();
    let n = |i: usize| a[i].parse::<f32>().unwrap();
    let r = Rect::new(n(2), n(3), n(4), n(5));
    let px = r.to_px(fw, fh).unwrap();
    let mut eng = EngineOcr(ocr::create_engine("winocr", &Default::default())?);
    for ch in [Channel::Luma, Channel::Max] {
        let g = Gray::crop(img.as_raw(), fw, px, ch);
        for scale in [1.0f32, 1.5, 2.0, 3.0] {
            for (stretch, bin, dark, pad) in [(false, false, false, 0), (true, false, false, 0), (false, false, false, 16), (true, true, true, 0), (false, false, true, 0)] {
                let prep = Prep { scale, stretch, binarize: bin, dark_text: dark, pad };
                let t = std::time::Instant::now();
                let lines = ocr_gray(&mut eng, &g, &prep, (0.0, 0.0)).map_err(anyhow::Error::msg)?;
                let ms = t.elapsed().as_secs_f64() * 1000.0;
                let txt: Vec<String> = lines.iter().map(|l| l.text.clone()).collect();
                println!("{ch:?} s{scale} st{} bin{} dark{} pad{pad}: {ms:.1} ms {txt:?}", stretch as u8, bin as u8, dark as u8);
            }
        }
    }
    Ok(())
}
