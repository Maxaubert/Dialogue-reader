//! Settings that are not per game (M5): Startup and In-game overlay in `settings.json`, Start with Windows through
//! the desktop (the registry), Check now, and Export and Import of a game.

use std::path::Path;

use dr_runtime::Profile;
use dr_runtime::profile;
use dr_runtime::settings::{self, PillAt};

use slint::ComponentHandle;

use super::{Game, Vm};
use crate::Page;

/// Where DialogReader's releases are (Settings > Updates > Check now). The repo is private today; see
/// docs/app-plan.md, M5 open questions.
pub const RELEASES_URL: &str = "https://github.com/Maxaubert/dialogue-reader-rs/releases";

/// The line under the version in the app. It says what Check now does; the app cannot know yet whether it is the
/// latest version (no update check is built in, spec 6 item 2).
pub const UPDATE_NOTE: &str = "Check now opens the releases page in your browser.";

/// Said when a setting changes but settings.json must not be written (it is from a newer DialogReader, or it could
/// not be read or moved aside).
pub const NOT_SAVED: &str = "This change is not saved: DialogReader does not write over a settings file it could not use. It lasts until \
     DialogReader closes.";

/// The largest file Import reads: an exported game is a few kilobytes.
const IMPORT_MAX: u64 = 1024 * 1024;

impl Vm {
    /// Reads settings.json and Start with Windows, and shows them in Settings. Real source only.
    pub(super) fn load_prefs(&mut self) {
        let Some(real) = &self.real else { return };
        let loaded = settings::load(&real.paths);
        let autostart = real.desktop.autostart();
        self.prefs = loaded.settings;
        self.prefs_read_only = loaded.read_only;
        // the registry is the truth: the toggle never says Windows will start the app when it will not
        if let Some(on) = autostart {
            self.prefs.start_with_windows = on;
        }
        if let Some(w) = self.win() {
            w.set_start_with_windows(self.prefs.start_with_windows);
            w.set_open_minimized(self.prefs.open_minimized);
            w.set_show_in_taskbar(self.prefs.show_in_taskbar);
            w.set_pill_position(self.prefs.pill_position.index());
            w.set_show_speaker(self.prefs.show_speaker);
            w.set_update_note(UPDATE_NOTE.into());
            w.set_show_auto_update(false);
        }
        if let Some(p) = &loaded.problem {
            self.notice(&p.text());
        }
    }

    /// The saved settings (the defaults with the fake source).
    pub fn prefs(&self) -> &settings::Settings {
        &self.prefs
    }

    /// A Startup or In-game overlay row changed: Start with Windows goes to the registry, the taskbar button follows,
    /// and settings.json is saved.
    pub fn settings_changed(&mut self) {
        let Some(w) = self.win() else { return };
        let Some(real) = &self.real else { return };
        let want = w.get_start_with_windows();
        if want != self.prefs.start_with_windows {
            match real.desktop.set_autostart(want) {
                Ok(()) => self.prefs.start_with_windows = want,
                Err(e) => {
                    w.set_start_with_windows(self.prefs.start_with_windows);
                    self.notice(&format!("Start with Windows could not be changed: {e}"));
                }
            }
        }
        let taskbar = w.get_show_in_taskbar();
        if taskbar != self.prefs.show_in_taskbar && w.window().is_visible() {
            crate::shell::set_taskbar(&w, taskbar);
        }
        self.prefs.open_minimized = w.get_open_minimized();
        self.prefs.show_in_taskbar = taskbar;
        self.prefs.pill_position = PillAt::of_index(w.get_pill_position());
        self.prefs.show_speaker = w.get_show_speaker();
        self.save_prefs();
    }

    fn save_prefs(&self) {
        let Some(real) = &self.real else { return };
        if self.prefs_read_only {
            self.notice(NOT_SAVED);
            return;
        }
        if let Err(e) = settings::save(&real.paths, &self.prefs) {
            self.notice(&format!("The settings could not be saved: {e}"));
        }
    }

    /// Settings > Updates > Check now: the releases page in the browser.
    pub fn check_updates(&self) {
        if let Some(real) = &self.real {
            real.desktop.open_url(RELEASES_URL);
        }
    }

    /// The main window's HWND, to own a file dialog.
    fn owner(&self) -> Option<isize> {
        #[cfg(windows)]
        {
            let w = self.win()?;
            crate::overlay::win::hwnd_of(w.window()).map(|h| h.0 as isize)
        }
        #[cfg(not(windows))]
        None
    }

    /// Settings > Games > Export (`save`), or Import there and "Import settings" on the welcome page: asks for a
    /// file. The answer comes back through the window's `file-picked` callback ([`Vm::file_picked`]).
    pub fn pick_file(&self, save: bool) {
        let Some(real) = &self.real else { return };
        let suggested = if save {
            let Some(i) = self.current else { return };
            Some(format!("{}.json", file_safe(&self.games[i].profile.name)))
        } else {
            None
        };
        let weak = self.win().map(|w| w.as_weak());
        real.desktop.pick_file_then(
            self.owner(),
            suggested,
            Box::new(move |path| {
                if let (Some(weak), Some(path)) = (weak, path) {
                    let path = path.to_string_lossy().into_owned();
                    let _ = weak
                        .upgrade_in_event_loop(move |w| w.invoke_file_picked(save, path.into()));
                }
            }),
        );
    }

    /// The file dialog answered: export to `path`, or import from it.
    pub fn file_picked(&mut self, save: bool, path: &str) {
        let path = Path::new(path);
        if save {
            let Some(i) = self.current else { return };
            match self.export_to(path) {
                Ok(()) => self.notice(&format!(
                    "{} was exported to {}.",
                    self.games[i].profile.name,
                    path.display()
                )),
                Err(e) => self.notice(&format!("The game could not be exported: {e}")),
            }
        } else if let Err(e) = self.import_from(path) {
            self.notice(&e);
        }
    }

    /// Writes the current game's profile to `path`.
    pub fn export_to(&self, path: &Path) -> Result<(), String> {
        let i = self.current.ok_or("no game is set up")?;
        dr_runtime::paths::write_atomic(path, self.games[i].profile.to_json().as_bytes())
            .map_err(|e| e.to_string())
    }

    /// Adds the game in `path` (a profile exported by DialogReader) under a new id and makes it the current game.
    /// The error is the notice text.
    pub fn import_from(&mut self, path: &Path) -> Result<(), String> {
        let Some(real) = &self.real else {
            return Err("Nothing can be saved here.".into());
        };
        let size = std::fs::metadata(path)
            .map_err(|e| format!("The file could not be read: {e}"))?
            .len();
        if size > IMPORT_MAX {
            return Err(
                "This file is too big to be a DialogReader game, so nothing was imported.".into(),
            );
        }
        let text = std::fs::read_to_string(path)
            .map_err(|e| format!("The file could not be read: {e}"))?;
        let mut p: Profile = Profile::from_json(&text).map_err(|e| {
            format!("This file is not a DialogReader game ({e}), so nothing was imported.")
        })?;
        if p.name.trim().is_empty() {
            p.name = path
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| "Imported game".into());
        }
        // a new id, so no profile on disk is overwritten (also files that were not loaded)
        let mut taken = profile::ids_on_disk(&real.paths);
        taken.extend(self.games.iter().map(|g| g.profile.id.clone()));
        p.id = profile::new_id(&p.name, &taken);
        profile::save(&real.paths, &p).map_err(|e| format!("The game could not be saved: {e}"))?;
        self.games.insert(
            0,
            Game {
                profile: p,
                shot_size: None,
                cover: None,
            },
        );
        self.open_game(0, None);
        if let Some(w) = self.win() {
            w.set_adding_game(false);
            w.set_switcher_open(false);
            w.set_page(Page::Overview);
        }
        self.notice("");
        Ok(())
    }
}

/// A file name from a game name: characters Windows does not allow become spaces.
fn file_safe(name: &str) -> String {
    let s: String = name
        .chars()
        .map(|c| {
            if c.is_control() || r#"<>:"/\|?*"#.contains(c) {
                ' '
            } else {
                c
            }
        })
        .collect();
    let s = s.trim().trim_end_matches('.').trim().to_string();
    if s.is_empty() { "game".into() } else { s }
}

#[cfg(test)]
mod tests {
    use super::file_safe;

    #[test]
    fn file_names_from_game_names() {
        assert_eq!(file_safe("Red Dead Redemption 2"), "Red Dead Redemption 2");
        assert_eq!(
            file_safe("Clair Obscur: Expedition 33"),
            "Clair Obscur  Expedition 33"
        );
        assert_eq!(file_safe("a/b?"), "a b");
        assert_eq!(file_safe("..."), "game");
    }
}
