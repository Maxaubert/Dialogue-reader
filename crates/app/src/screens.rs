//! Headless screen renders: every page and overlay drawn with Slint's software renderer into a buffer.
//! No window is created and nothing touches the desktop.

use std::path::{Path, PathBuf};
use std::rc::Rc;

use slint::platform::software_renderer::{
    MinimalSoftwareWindow, PremultipliedRgbaColor, RepaintBufferType,
};
use slint::platform::{Platform, WindowAdapter};
use slint::{ComponentHandle, PhysicalSize, PlatformError};

use crate::{MainWindow, OverlayPreview, Page, Render};

pub const WIDTH: u32 = 1440;
pub const HEIGHT: u32 = 900;

/// One screen: the file name (as in `design/mockups/c-midnight/shots`) and the state that shows it.
pub struct Screen {
    pub name: &'static str,
    pub setup: fn(&MainWindow),
}

fn page(w: &MainWindow, p: Page) {
    w.set_overlay(OverlayPreview::None);
    w.set_switcher_open(false);
    w.invoke_settings_close_voice_list();
    w.set_page(p);
}

/// Every approved mockup, plus the activity list ("Show all"), Settings scrolled to the keys, the Settings voice
/// picker open, and the voice screens with no Windows voice installed.
pub fn all() -> Vec<Screen> {
    vec![
        Screen {
            name: "01-welcome",
            setup: |w| page(w, Page::Welcome),
        },
        Screen {
            name: "02-pick-window",
            setup: |w| page(w, Page::PickWindow),
        },
        Screen {
            name: "03-draw-zones",
            setup: |w| page(w, Page::DrawZones),
        },
        Screen {
            name: "04-read-key-d",
            setup: |w| page(w, Page::Keys),
        },
        Screen {
            name: "05-voice",
            setup: |w| page(w, Page::Voice),
        },
        Screen {
            name: "06-mode",
            setup: |w| page(w, Page::Mode),
        },
        Screen {
            name: "10-main-b",
            setup: |w| page(w, Page::Overview),
        },
        Screen {
            name: "11-zones",
            setup: |w| page(w, Page::Zones),
        },
        Screen {
            name: "12-settings",
            setup: |w| page(w, Page::Settings),
        },
        Screen {
            name: "12-settings-keys",
            setup: |w| {
                page(w, Page::Settings);
                w.invoke_settings_jump(2);
            },
        },
        Screen {
            name: "13-overlay",
            setup: |w| {
                page(w, Page::Overview);
                w.set_overlay(OverlayPreview::Pill);
            },
        },
        Screen {
            name: "14-game-switcher",
            setup: |w| {
                page(w, Page::Overview);
                w.set_switcher_open(true);
            },
        },
        Screen {
            name: "15-quick-menu-a",
            setup: |w| {
                page(w, Page::Overview);
                w.set_overlay(OverlayPreview::QuickMenu);
            },
        },
        Screen {
            name: "15-quick-menu-voice",
            setup: |w| {
                page(w, Page::Overview);
                w.set_overlay(OverlayPreview::QuickMenuVoice);
            },
        },
        Screen {
            name: "16-zone-edit",
            setup: |w| {
                page(w, Page::Overview);
                w.set_overlay(OverlayPreview::ZoneEdit);
            },
        },
        Screen {
            name: "17-activity",
            setup: |w| page(w, Page::Activity),
        },
        // review aids (M3a, no mockup): the Settings voice picker open (sample voices, the list scrolls), and what
        // pages 05, 06 and Settings > Voices show with no Windows voice installed
        Screen {
            name: "18-settings-voice-list",
            setup: |w| {
                page(w, Page::Settings);
                w.invoke_settings_jump(3);
                w.invoke_settings_open_voice_list();
            },
        },
        Screen {
            name: "18-voice-none",
            setup: |w| {
                no_voices(w);
                page(w, Page::Voice);
            },
        },
        Screen {
            name: "18-mode-voice-none",
            setup: |w| {
                no_voices(w);
                page(w, Page::Mode);
            },
        },
        Screen {
            name: "18-settings-voices-none",
            setup: |w| {
                no_voices(w);
                page(w, Page::Settings);
                w.invoke_settings_jump(3);
            },
        },
    ]
}

/// The state of a PC with no Windows voice (the last screens, so the others keep the sample voices).
fn no_voices(w: &MainWindow) {
    let none = crate::vm::voices::Voices::none();
    w.set_voices(slint::ModelRc::default());
    w.set_voices_problem(none.problem_text().into());
    w.set_voices_desc(none.desc().into());
    w.set_voice_label(none.label("").into());
    w.set_voice_testing(false);
    w.set_test_time("0:00".into());
}

struct Headless {
    window: Rc<MinimalSoftwareWindow>,
}

impl Platform for Headless {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, PlatformError> {
        Ok(self.window.clone())
    }
}

/// A software-rendered window that is never shown on screen. One per thread.
pub struct HeadlessRenderer {
    window: Rc<MinimalSoftwareWindow>,
    width: u32,
    height: u32,
}

impl HeadlessRenderer {
    /// Installs the headless platform on this thread. Call before creating any Slint component.
    pub fn install(width: u32, height: u32) -> Result<Self, String> {
        let window = MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
        window.set_size(PhysicalSize::new(width, height));
        slint::platform::set_platform(Box::new(Headless {
            window: window.clone(),
        }))
        .map_err(|e| format!("could not set the headless platform: {e}"))?;
        Ok(Self {
            window,
            width,
            height,
        })
    }

    /// Draws the current state of the window into an RGBA image.
    pub fn render(&self) -> image::RgbaImage {
        slint::platform::update_timers_and_animations();
        let (w, h) = (self.width as usize, self.height as usize);
        let mut buf = vec![PremultipliedRgbaColor::default(); w * h];
        self.window.request_redraw();
        self.window.draw_if_needed(|r| {
            r.render(&mut buf, w);
        });
        let mut img = image::RgbaImage::new(self.width, self.height);
        for (px, c) in img.pixels_mut().zip(buf.iter()) {
            let a = c.alpha;
            let un = |v: u8| {
                if a == 0 {
                    0
                } else {
                    ((v as u32 * 255 + a as u32 / 2) / a as u32).min(255) as u8
                }
            };
            *px = image::Rgba([un(c.red), un(c.green), un(c.blue), a]);
        }
        img
    }
}

/// Renders every screen to `<out>/<name>.png`. Returns the written paths.
pub fn render_all(out: &Path) -> Result<Vec<PathBuf>, String> {
    std::fs::create_dir_all(out).map_err(|e| format!("cannot create {}: {e}", out.display()))?;
    let r = HeadlessRenderer::install(WIDTH, HEIGHT)?;
    let w = crate::build_main_window().map_err(|e| e.to_string())?;
    // the software renderer cannot clip to rounded corners: paint them with CornerMask
    w.global::<Render>().set_software(true);
    w.show().map_err(|e| e.to_string())?;
    let mut written = Vec::new();
    for s in all() {
        (s.setup)(&w);
        let img = r.render();
        let path = out.join(format!("{}.png", s.name));
        img.save(&path)
            .map_err(|e| format!("cannot write {}: {e}", path.display()))?;
        written.push(path);
    }
    w.hide().map_err(|e| e.to_string())?;
    Ok(written)
}
