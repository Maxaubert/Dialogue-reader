//! One captured frame.

/// RGBA8 pixels, row-major, no row padding (`w * h * 4` bytes). `t_ms` is the capture time.
#[derive(Clone, Copy)]
pub struct Frame<'a> {
    pub t_ms: f64,
    pub w: u32,
    pub h: u32,
    pub rgba: &'a [u8],
}

impl<'a> Frame<'a> {
    /// Panics when `rgba` is not exactly `w * h * 4` bytes (a frame with row padding or another
    /// pixel order must be copied to tight RGBA first).
    pub fn new(t_ms: f64, w: u32, h: u32, rgba: &'a [u8]) -> Self {
        assert_eq!(rgba.len(), w as usize * h as usize * 4, "frame buffer is not tight RGBA8 of {w}x{h}");
        Frame { t_ms, w, h, rgba }
    }
}
