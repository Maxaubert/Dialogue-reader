//! The quick menu as a state machine (spec 2.7, mockups 15-quick-menu-a and 15-quick-menu-voice). Plain Rust,
//! unit-tested (`tests/quick_menu.rs`). Keys come from the hook ([`Menu::key`]), clicks from the window
//! ([`Menu::activate`]); each returns what the app must do ([`Out`]).
//!
//! Order (design rule): Edit zones, Stop (play/pause), Mute, Voice, Speed, Volume, Mode, Settings, Quit.
//! Up and Down move and wrap around. Left and Right change Speed, Volume and Mode, and Right opens the voice
//! list. Enter or Space selects. Esc closes. Quit needs two presses: the first arms it ("Press again to quit"),
//! moving away disarms it. In the voice list Up and Down move (no wrap), Space plays a sample, Enter chooses,
//! Left or Esc goes back to the menu.

use dr_runtime::keys::Nav;

pub const ZONES: usize = 0;
pub const STOP: usize = 1;
pub const MUTE: usize = 2;
pub const VOICE: usize = 3;
pub const SPEED: usize = 4;
pub const VOLUME: usize = 5;
pub const MODE: usize = 6;
pub const SETTINGS: usize = 7;
pub const QUIT: usize = 8;
pub const ROWS: usize = 9;

pub const SPEED_MIN: f32 = 0.5;
pub const SPEED_MAX: f32 = 2.0;
pub const SPEED_STEP: f32 = 0.1;
pub const VOLUME_STEP: f32 = 0.05;
/// Rows of the voice list on screen at once; a longer list scrolls with the highlight.
pub const VOICE_ROWS: usize = 8;

/// What the app does after a key or click.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Out {
    /// Only the menu itself changed (highlight, Quit armed, the voice list opened or closed).
    None,
    Close,
    ManageZones,
    /// Stop or Resume reading (the user's Pause).
    Pause(bool),
    Mute(bool),
    /// A voice of the list was chosen (its index in the full list).
    Voice(usize),
    /// Play a sample of this voice (Space in the voice list).
    Preview(usize),
    Speed(f32),
    Volume(f32),
    /// Automatic (true) or On key press (false).
    Mode(bool),
    Settings,
    Quit,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Menu {
    pub highlight: usize,
    pub quit_armed: bool,
    pub stopped: bool,
    pub muted: bool,
    pub speed: f32,
    pub volume: f32,
    pub automatic: bool,
    /// How many voices the list has, and which one the game uses.
    pub voices: usize,
    pub voice: Option<usize>,
    pub voice_open: bool,
    pub voice_hi: usize,
    /// The first voice row on screen.
    pub voice_first: usize,
}

impl Default for Menu {
    fn default() -> Self {
        Menu {
            highlight: 0,
            quit_armed: false,
            stopped: false,
            muted: false,
            speed: 1.0,
            volume: 0.8,
            automatic: true,
            voices: 0,
            voice: None,
            voice_open: false,
            voice_hi: 0,
            voice_first: 0,
        }
    }
}

/// `v` on the 0.1 grid (no drift after many steps).
fn tenth(v: f32) -> f32 {
    (v * 10.0).round() / 10.0
}

/// `v` on the 0.05 grid.
fn twentieth(v: f32) -> f32 {
    (v * 20.0).round() / 20.0
}

impl Menu {
    /// Opens the menu fresh: the highlight on the first row, Quit not armed, the voice list closed.
    pub fn open(&mut self) {
        self.highlight = 0;
        self.quit_armed = false;
        self.voice_open = false;
    }

    /// Esc closes the menu (false while the voice list is open: Esc goes back to the menu).
    pub fn esc_closes(&self) -> bool {
        !self.voice_open
    }

    /// The voice rows on screen: (first, how many).
    pub fn voice_slice(&self) -> (usize, usize) {
        (
            self.voice_first,
            VOICE_ROWS.min(self.voices - self.voice_first.min(self.voices)),
        )
    }

    fn show_voice_hi(&mut self) {
        if self.voice_hi < self.voice_first {
            self.voice_first = self.voice_hi;
        } else if self.voice_hi >= self.voice_first + VOICE_ROWS {
            self.voice_first = self.voice_hi + 1 - VOICE_ROWS;
        }
        self.voice_first = self.voice_first.min(self.voices.saturating_sub(VOICE_ROWS));
    }

    fn open_voices(&mut self) -> Out {
        if self.voices == 0 {
            return Out::None;
        }
        self.highlight = VOICE;
        self.voice_open = true;
        self.voice_hi = self.voice.filter(|&v| v < self.voices).unwrap_or(0);
        self.voice_first = 0;
        self.show_voice_hi();
        Out::None
    }

    /// A key from the hook.
    pub fn key(&mut self, k: Nav) -> Out {
        if self.voice_open {
            return self.voice_key(k);
        }
        match k {
            Nav::Up | Nav::Down => {
                self.highlight = if k == Nav::Up {
                    (self.highlight + ROWS - 1) % ROWS
                } else {
                    (self.highlight + 1) % ROWS
                };
                self.quit_armed = false;
                Out::None
            }
            Nav::Enter | Nav::Space => self.activate(self.highlight),
            Nav::Right => match self.highlight {
                VOICE => self.open_voices(),
                SPEED => self.set_speed(self.speed + SPEED_STEP),
                VOLUME => self.set_volume(self.volume + VOLUME_STEP),
                MODE => self.set_mode(false),
                _ => Out::None,
            },
            Nav::Left => match self.highlight {
                SPEED => self.set_speed(self.speed - SPEED_STEP),
                VOLUME => self.set_volume(self.volume - VOLUME_STEP),
                MODE => self.set_mode(true),
                _ => Out::None,
            },
            Nav::Esc => Out::Close,
            Nav::Delete => Out::None,
        }
    }

    fn voice_key(&mut self, k: Nav) -> Out {
        match k {
            Nav::Up => {
                self.voice_hi = self.voice_hi.saturating_sub(1);
                self.show_voice_hi();
                Out::None
            }
            Nav::Down => {
                self.voice_hi = (self.voice_hi + 1).min(self.voices.saturating_sub(1));
                self.show_voice_hi();
                Out::None
            }
            Nav::Space => Out::Preview(self.voice_hi),
            Nav::Enter => self.choose_voice(self.voice_hi),
            Nav::Left | Nav::Esc => {
                self.voice_open = false;
                Out::None
            }
            Nav::Right | Nav::Delete => Out::None,
        }
    }

    /// A voice row chosen (Enter, or a click; index in the full list).
    pub fn choose_voice(&mut self, i: usize) -> Out {
        if i >= self.voices {
            return Out::None;
        }
        self.voice = Some(i);
        self.voice_open = false;
        Out::Voice(i)
    }

    /// Enter or Space on a row, or a click on it.
    pub fn activate(&mut self, row: usize) -> Out {
        if row >= ROWS {
            return Out::None;
        }
        let was_armed = self.quit_armed && row == QUIT;
        self.highlight = row;
        self.quit_armed = false;
        self.voice_open = false;
        match row {
            ZONES => Out::ManageZones,
            STOP => {
                self.stopped = !self.stopped;
                Out::Pause(self.stopped)
            }
            MUTE => {
                self.muted = !self.muted;
                Out::Mute(self.muted)
            }
            VOICE => self.open_voices(),
            MODE => self.set_mode(!self.automatic),
            SETTINGS => Out::Settings,
            QUIT if was_armed => Out::Quit,
            QUIT => {
                self.quit_armed = true;
                Out::None
            }
            _ => Out::None,
        }
    }

    pub fn set_speed(&mut self, v: f32) -> Out {
        let v = tenth(v.clamp(SPEED_MIN, SPEED_MAX));
        if v == self.speed {
            return Out::None;
        }
        self.speed = v;
        Out::Speed(v)
    }

    pub fn set_volume(&mut self, v: f32) -> Out {
        let v = twentieth(v.clamp(0.0, 1.0));
        if v == self.volume {
            return Out::None;
        }
        self.volume = v;
        Out::Volume(v)
    }

    pub fn set_mode(&mut self, automatic: bool) -> Out {
        if automatic == self.automatic {
            return Out::None;
        }
        self.automatic = automatic;
        Out::Mode(automatic)
    }
}
