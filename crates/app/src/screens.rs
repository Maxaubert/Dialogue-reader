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
    w.set_sheet(false);
    w.set_overlay(OverlayPreview::None);
    w.set_switcher_open(false);
    w.set_voice_menu(false);
    w.set_preview_voice(-1);
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
            // No mockup: page 02 with an "Other windows" row picked (selection must be as clear as a card).
            name: "02-pick-window-other",
            setup: |w| {
                page(w, Page::PickWindow);
                use slint::Model;
                w.set_picked_window(w.get_games_running().row_count() as i32);
            },
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
        // No mockup (Max, 2026-11-24): the voice picker of the Overview toolbar, open
        Screen {
            name: "10-overview-voice-menu",
            setup: |w| {
                page(w, Page::Overview);
                w.set_voice_menu(true);
            },
        },
        // No mockup (Max, 2026-11-24): the third voice's sample plays from the picker (its button is Stop)
        Screen {
            name: "10-overview-voice-preview",
            setup: |w| {
                page(w, Page::Overview);
                w.set_voice_menu(true);
                w.set_voice_testing(true);
                w.set_preview_voice(2);
            },
        },
        // a real screenshot that is not 16:9 (shown whole): one frame, as tall as the Now panel (Max, 2026-11-24)
        Screen {
            name: "10-overview-real-shot",
            setup: |w| {
                page(w, Page::Overview);
                w.set_real_shots(true);
                w.set_shot_w(1880.0);
                w.set_shot_h(1000.0);
            },
        },
        // a 16:9 real screenshot: at 1707 x 912 the frame hugs the picture (no thin band, no second frame)
        Screen {
            name: "10-overview-real-shot-16x9",
            setup: |w| {
                page(w, Page::Overview);
                w.set_real_shots(true);
                w.set_shot_w(1920.0);
                w.set_shot_h(1080.0);
            },
        },
        Screen {
            name: "11-zones",
            setup: |w| {
                w.set_real_shots(false);
                w.set_shot_w(1600.0);
                w.set_shot_h(900.0);
                page(w, Page::Zones);
            },
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
        // No mockup (Max, 2026-11-24): Delete in the switcher asks first, on the highlighted row
        Screen {
            name: "14-game-switcher-delete",
            setup: |w| {
                page(w, Page::Overview);
                w.set_switcher_open(true);
                w.invoke_switcher_ask_delete();
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
        // zone edit as it opens: no zone selected, nothing drawn
        Screen {
            name: "16-zone-edit",
            setup: |w| {
                page(w, Page::Overview);
                w.set_edit_selected(-1);
                w.set_edit_draft(false);
                w.set_overlay(OverlayPreview::ZoneEdit);
            },
        },
        Screen {
            name: "17-activity",
            setup: |w| page(w, Page::Activity),
        },
        // every shared part in its states (ui/components/sheet.slint), to compare with the d-black token sheet
        Screen {
            name: "00-components",
            setup: |w| {
                page(w, Page::Overview);
                w.set_sheet(true);
            },
        },
        // review aids (M3a, no mockup): the Settings voice picker open (sample voices, the list scrolls), what pages
        // 05 and Settings > Voices show when Kokoro's files are missing, and what 05, 06 and Settings > Voices show
        // with no voice at all
        Screen {
            name: "18-settings-voice-list",
            setup: |w| {
                page(w, Page::Settings);
                w.invoke_settings_jump(3);
                w.invoke_settings_open_voice_list();
            },
        },
        // Kokoro's files missing (2026-11-23): the Windows voices are the backup
        Screen {
            name: "18-voice-windows",
            setup: |w| {
                windows_voices(w);
                page(w, Page::Voice);
            },
        },
        Screen {
            name: "18-settings-voices-windows",
            setup: |w| {
                windows_voices(w);
                page(w, Page::Settings);
                w.invoke_settings_jump(3);
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
        // M3b: key capture waits for the new read key; Overview of a game that is not running, nothing read yet
        Screen {
            name: "19-keys-capture",
            setup: |w| {
                page(w, Page::Keys);
                w.set_capturing_key(0);
            },
        },
        Screen {
            name: "19-overview-not-running",
            setup: |w| {
                w.set_capturing_key(-1);
                w.set_status_text(crate::vm::reading::STATUS_NO_WINDOW.into());
                w.set_status_ok(false);
                w.set_game_running(false);
                w.set_voice_label("Adam".into());
                w.set_now_who("".into());
                w.set_now_line("".into());
                w.set_recent(slint::ModelRc::default());
                page(w, Page::Overview);
            },
        },
        // d-black states (after the others: they change the data): Activity with nothing read, Zones with a box
        // at the top edge (its tag goes under it), Zones with no zone
        Screen {
            name: "17-activity-empty",
            setup: |w| {
                // reading again (screen 19 left the game not running): the lime dot stays on an empty Activity
                w.set_status_text(crate::vm::reading::STATUS_READING.into());
                w.set_status_ok(true);
                w.set_game_running(true);
                w.set_log(slint::ModelRc::default());
                page(w, Page::Activity);
            },
        },
        Screen {
            name: "11-zones-top",
            setup: |w| {
                use slint::Model;
                let mut zones: Vec<crate::ZoneData> = w.get_zones().iter().collect();
                zones.push(crate::ZoneData {
                    name: "Pickups".into(),
                    kind: crate::Kind::Pickups,
                    on: true,
                    x: 640.0,
                    y: 4.0,
                    w: 340.0,
                    h: 44.0,
                });
                w.set_zones_selected(zones.len() as i32 - 1);
                w.set_zone_kind(crate::Kind::Pickups);
                w.set_zones(slint::ModelRc::new(slint::VecModel::from(zones)));
                page(w, Page::Zones);
            },
        },
        Screen {
            name: "11-zones-empty",
            setup: |w| {
                w.set_zones_selected(-1);
                w.set_zones(slint::ModelRc::default());
                page(w, Page::Zones);
            },
        },
        // zone edit with a box just drawn: its tag, and the kind picker under it (16-zone-edit-new)
        Screen {
            name: "16-zone-edit-new",
            setup: |w| {
                page(w, Page::Overview);
                w.set_edit_selected(-1);
                w.set_edit_draft_x(930.0);
                w.set_edit_draft_y(150.0);
                w.set_edit_draft_w(360.0);
                w.set_edit_draft_h(112.0);
                w.set_edit_draft(true);
                w.set_overlay(OverlayPreview::ZoneEdit);
            },
        },
        // a zone at the top edge, selected: its tag and the picker go under it, and the bar moves to the top
        // right (16-zone-edit-top). Last, as it changes the zone list.
        Screen {
            name: "16-zone-edit-top",
            setup: |w| {
                use slint::Model;
                page(w, Page::Overview);
                let mut zones: Vec<crate::EditZone> = w.get_edit_zones().iter().collect();
                zones.push(crate::EditZone {
                    kind: crate::Kind::Pickups,
                    x: 560.0,
                    y: 4.0,
                    w: 340.0,
                    h: 46.0,
                });
                let rects: Vec<crate::geom::Rect> = zones
                    .iter()
                    .map(|z| crate::geom::Rect::new(z.x, z.y, z.w, z.h))
                    .collect();
                let clear =
                    crate::overlay::zone_edit::bar_clear(&rects, WIDTH as f32, HEIGHT as f32);
                w.set_edit_bar_clear(slint::ModelRc::new(slint::VecModel::from(clear.to_vec())));
                w.set_edit_selected(zones.len() as i32 - 1);
                w.set_edit_zones(slint::ModelRc::new(slint::VecModel::from(zones)));
                w.set_edit_draft(false);
                w.set_overlay(OverlayPreview::ZoneEdit);
            },
        },
        // a game with no zone chips: the picture and the Now panel run to the bottom (Max, 2026-11-24). Last, as it
        // empties the chips.
        Screen {
            name: "10-overview-no-chips",
            setup: |w| {
                w.set_zone_kinds(slint::ModelRc::default());
                page(w, Page::Overview);
            },
        },
    ]
}

/// Kokoro's files are missing; four Windows voices (after the screens with the sample voices).
fn windows_voices(w: &MainWindow) {
    use dr_tts::{Gender, Voice};
    let voice = |id: &str, name: &str, lang: &str, g| Voice {
        id: id.into(),
        name: name.into(),
        lang: lang.into(),
        gender: Some(g),
        engine: "windows",
    };
    let tts = dr_tts::FakeTts::with_voices(vec![
        voice("david", "David", "en-US", Gender::Male),
        voice("zira", "Zira", "en-US", Gender::Female),
        voice("hazel", "Hazel", "en-GB", Gender::Female),
        voice("george", "George", "en-GB", Gender::Male),
    ]);
    let v =
        crate::vm::voices::Voices::of(&dr_tts::Engines::new(None, Some(Box::new(tts))), 4, None);
    w.set_voices(slint::ModelRc::new(slint::VecModel::from(v.rows())));
    w.set_picked_voice(0);
    w.set_game_voice(0);
    w.set_voices_problem(v.problem_text().into());
    w.set_voices_desc(v.desc().into());
    w.set_voice_label(v.label("").into());
    w.set_voice_testing(false);
    w.set_test_time("0:00".into());
}

/// The state of a PC with no voice at all (the last screens, so the others keep the sample voices).
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
    render_all_at(out, WIDTH, HEIGHT)
}

/// [`render_all`] at another window size (logical px, scale 1), for example 1707 x 912 (Max's maximized window).
pub fn render_all_at(out: &Path, width: u32, height: u32) -> Result<Vec<PathBuf>, String> {
    std::fs::create_dir_all(out).map_err(|e| format!("cannot create {}: {e}", out.display()))?;
    let r = HeadlessRenderer::install(width, height)?;
    // no clock runs between the renders: every page and control shows its end state at once
    crate::fonts::set_instant_motion(true);
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
