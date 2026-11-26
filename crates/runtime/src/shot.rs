//! A screenshot in memory: tight RGBA rows, as `dr_core::Frame` takes them.

use std::path::Path;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Shot {
    pub width: u32,
    pub height: u32,
    /// `width * height * 4` bytes, RGBA, no row padding.
    pub rgba: Vec<u8>,
}

impl Shot {
    /// One flat color (tests and placeholders).
    pub fn filled(width: u32, height: u32, rgba: [u8; 4]) -> Shot {
        Shot {
            width,
            height,
            rgba: rgba.repeat((width * height) as usize),
        }
    }

    pub fn pixel(&self, x: u32, y: u32) -> [u8; 4] {
        let i = ((y * self.width + x) * 4) as usize;
        [
            self.rgba[i],
            self.rgba[i + 1],
            self.rgba[i + 2],
            self.rgba[i + 3],
        ]
    }

    /// Saves it as PNG (atomic).
    pub fn save_png(&self, path: &Path) -> Result<(), String> {
        let img = image::RgbaImage::from_raw(self.width, self.height, self.rgba.clone())
            .ok_or_else(|| "screenshot buffer has the wrong size".to_string())?;
        let mut bytes = Vec::new();
        img.write_to(
            &mut std::io::Cursor::new(&mut bytes),
            image::ImageFormat::Png,
        )
        .map_err(|e| format!("could not encode the screenshot: {e}"))?;
        crate::paths::write_atomic(path, &bytes)
            .map_err(|e| format!("could not save {}: {e}", path.display()))
    }

    pub fn load_png(path: &Path) -> Result<Shot, String> {
        let img = image::open(path)
            .map_err(|e| format!("could not read {}: {e}", path.display()))?
            .to_rgba8();
        Ok(Shot {
            width: img.width(),
            height: img.height(),
            rgba: img.into_raw(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn png_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = Shot::filled(4, 3, [10, 20, 30, 255]);
        s.rgba[0] = 200;
        let p = dir.path().join("x.png");
        s.save_png(&p).unwrap();
        assert_eq!(Shot::load_png(&p).unwrap(), s);
    }
}
