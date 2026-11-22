//! One screenshot of a window with Windows Graphics Capture (spec 2.3): the picture zones are drawn on.
//!
//! The size is the capture size, so zone rects drawn on it are in the pixels the reader gets later (M3b uses
//! the same capture). Settings as the spec says: no cursor, no yellow border where Windows allows it.
//!
//! WGC captures the whole window: for a windowed game that includes its title bar and frame, so the picture
//! can be larger than the client size page 02 shows. Zones are in these capture pixels.

use std::sync::mpsc;
use std::time::Duration;

use windows_capture::capture::{Context, GraphicsCaptureApiError, GraphicsCaptureApiHandler};
use windows_capture::frame::Frame;
use windows_capture::graphics_capture_api::{Error as ApiError, InternalCaptureControl};
use windows_capture::settings::{
    ColorFormat, CursorCaptureSettings, DirtyRegionSettings, DrawBorderSettings,
    MinimumUpdateIntervalSettings, SecondaryWindowSettings, Settings,
};
use windows_capture::window::Window;

use crate::shot::Shot;

/// How long to wait for the first frame.
pub const TIMEOUT: Duration = Duration::from_secs(3);

type Tx = mpsc::SyncSender<Result<Shot, String>>;

struct One {
    tx: Option<Tx>,
}

impl GraphicsCaptureApiHandler for One {
    type Flags = Tx;
    type Error = Box<dyn std::error::Error + Send + Sync>;

    fn new(ctx: Context<Self::Flags>) -> Result<Self, Self::Error> {
        Ok(One {
            tx: Some(ctx.flags),
        })
    }

    fn on_frame_arrived(
        &mut self,
        frame: &mut Frame,
        control: InternalCaptureControl,
    ) -> Result<(), Self::Error> {
        let (w, h) = (frame.width(), frame.height());
        let shot = match frame.buffer() {
            Ok(fb) => {
                let mut tmp = Vec::new();
                let rgba = fb.as_nopadding_buffer(&mut tmp).to_vec();
                Ok(Shot {
                    width: w,
                    height: h,
                    rgba,
                })
            }
            Err(e) => Err(format!("could not read the captured frame: {e}")),
        };
        if let Some(tx) = self.tx.take() {
            let _ = tx.send(shot);
        }
        control.stop();
        Ok(())
    }

    fn on_closed(&mut self) -> Result<(), Self::Error> {
        if let Some(tx) = self.tx.take() {
            let _ = tx.send(Err("the window closed".into()));
        }
        Ok(())
    }
}

/// Takes one frame of the window `hwnd` (`HWND.0 as isize`). A minimized window gives no frames, so it times
/// out with a plain message (the app checks for a minimized window first).
pub fn capture(hwnd: isize) -> Result<Shot, String> {
    capture_with(hwnd, TIMEOUT)
}

pub fn capture_with(hwnd: isize, timeout: Duration) -> Result<Shot, String> {
    let mut cursor = CursorCaptureSettings::WithoutCursor;
    let mut border = DrawBorderSettings::WithoutBorder;
    loop {
        let (tx, rx) = mpsc::sync_channel(1);
        let settings = Settings::new(
            Window::from_raw_hwnd(hwnd as *mut std::ffi::c_void),
            cursor,
            border,
            SecondaryWindowSettings::Default,
            MinimumUpdateIntervalSettings::Default,
            DirtyRegionSettings::Default,
            ColorFormat::Rgba8,
            tx,
        );
        let control = match One::start_free_threaded(settings) {
            Ok(c) => c,
            Err(e) if relax(&e, &mut cursor, &mut border) => continue,
            Err(e) => return Err(format!("Windows could not capture this window ({e})")),
        };
        let got = rx.recv_timeout(timeout);
        let _ = control.stop();
        return match got {
            Ok(r) => r,
            Err(_) => Err(format!(
                "no picture came from the window within {} seconds",
                timeout.as_secs()
            )),
        };
    }
}

/// Older Windows 10 builds cannot hide the yellow border or the cursor in a capture. That is cosmetic, not an
/// error (spec 2.3): switch that setting back to the Windows default and try again. False when nothing is left
/// to relax.
fn relax<E>(
    e: &GraphicsCaptureApiError<E>,
    cursor: &mut CursorCaptureSettings,
    border: &mut DrawBorderSettings,
) -> bool {
    match e {
        GraphicsCaptureApiError::GraphicsCaptureApiError(ApiError::BorderConfigUnsupported)
            if *border != DrawBorderSettings::Default =>
        {
            *border = DrawBorderSettings::Default;
            true
        }
        GraphicsCaptureApiError::GraphicsCaptureApiError(ApiError::CursorConfigUnsupported)
            if *cursor != CursorCaptureSettings::Default =>
        {
            *cursor = CursorCaptureSettings::Default;
            true
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsupported_border_or_cursor_falls_back_to_the_default() {
        type E = GraphicsCaptureApiError<String>;
        let (mut c, mut b) = (
            CursorCaptureSettings::WithoutCursor,
            DrawBorderSettings::WithoutBorder,
        );
        let border = E::GraphicsCaptureApiError(ApiError::BorderConfigUnsupported);
        let cursor = E::GraphicsCaptureApiError(ApiError::CursorConfigUnsupported);
        assert!(relax(&border, &mut c, &mut b));
        assert_eq!(b, DrawBorderSettings::Default);
        // already at the default: a real error now
        assert!(!relax(&border, &mut c, &mut b));
        assert!(relax(&cursor, &mut c, &mut b));
        assert_eq!(c, CursorCaptureSettings::Default);
        assert!(!relax(&cursor, &mut c, &mut b));
        assert!(!relax(&E::ItemConvertFailed, &mut c, &mut b));
    }
}
