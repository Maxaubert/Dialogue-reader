//! The in-game overlays in the view model (M4, spec 2.7): which one shows, what it shows, and what its keys and
//! clicks do. The windows are in `crate::overlay`.
//!
//! - The hook keeps which overlay is open (`Runtime::overlay`): the quick menu key and Esc change it on the hook
//!   thread at once. [`Vm::overlay_sync`] makes the windows match it, on every key event (the runtime's waker
//!   wakes the UI thread at once) and on the reading timer. The app changes it with `Cmd::Overlay` (Edit zones,
//!   Settings, a click outside the menu, Done).
//! - The menu and zone edit open only over the game in front, and close when it is no longer in front (Alt+Tab,
//!   minimized, closed). While one is open, reading is held (`Cmd::Hold`, not the user's Pause) and the pill hides.
//! - The pill shows while the game is read (Reading or Paused) and is in front, at the place set in Settings
//!   (top right, bottom center, off). Its dot is green while reading; the speaker of the line being spoken shows
//!   (if "Show the speaker" is on) and the wave is lit while it is spoken.
//! - Zone edit freezes a screenshot of the game (it becomes the profile's picture) and edits the
//!   same zones as the Zones tab; each change is saved and sent to the reader at once.

use std::cell::RefCell;
use std::rc::{Rc, Weak};

use dr_runtime::keys::{Nav, Overlay};
use dr_runtime::runtime::{Cmd, Status};
use dr_runtime::speech;
use slint::{ComponentHandle, ModelRc, VecModel};

use super::{Page, Shared, Vm, sync, voices};
use crate::geom::Rect;
use crate::overlay::place::{CaptureMap, GameWin, PillAt};
use crate::overlay::quick_menu::{self, Out};
use crate::overlay::{Overlays, zone_edit};

thread_local! {
    /// The view model, for work posted to the UI thread by the hook thread's waker.
    static WAKE: RefCell<Weak<RefCell<Vm>>> = const { RefCell::new(Weak::new()) };
}

/// Hands the overlays to the view model and wires their callbacks. Only `main.rs` (and tests on the testing
/// backend) call it, after `attach_runtime`.
pub fn attach_overlays(vm: &Shared, o: Overlays) {
    wire(&o, vm);
    WAKE.with(|w| *w.borrow_mut() = Rc::downgrade(vm));
    let mut v = vm.borrow_mut();
    if let Some(rt) = v.reading.rt.as_mut() {
        // a key event wakes the UI thread at once, instead of waiting for the next tick
        rt.set_waker(Some(std::sync::Arc::new(|| {
            let _ = slint::invoke_from_event_loop(|| {
                let vm = WAKE.with(|w| w.borrow().upgrade());
                if let Some(vm) = vm
                    && let Ok(mut vm) = vm.try_borrow_mut()
                {
                    vm.reading_tick();
                }
            });
        })));
    }
    v.overlays = Some(o);
    v.overlay_sync();
}

fn wire(o: &Overlays, vm: &Shared) {
    let with = |vm: &Shared| {
        let w = Rc::downgrade(vm);
        move |f: &dyn Fn(&mut Vm)| {
            if let Some(v) = w.upgrade()
                && let Ok(mut vm) = v.try_borrow_mut()
            {
                f(&mut vm);
            }
        }
    };
    let m = &o.menu;
    let go = with(vm);
    m.on_activate(move |i| {
        go(&|vm| {
            let out = vm.menu_state(|s| s.activate(i.max(0) as usize));
            vm.menu_out(out);
        })
    });
    let go = with(vm);
    m.on_speed_moved(move |v| go(&|vm| vm.menu_speed(v, false)));
    let go = with(vm);
    m.on_speed_let_go(move |v| go(&|vm| vm.menu_speed(v, true)));
    let go = with(vm);
    m.on_volume_moved(move |v| go(&|vm| vm.menu_volume(v, false)));
    let go = with(vm);
    m.on_volume_let_go(move |v| go(&|vm| vm.menu_volume(v, true)));
    let go = with(vm);
    m.on_mode_picked(move |a| {
        go(&|vm| {
            let out = vm.menu_state(|s| s.set_mode(a));
            vm.menu_out(out);
        })
    });
    let go = with(vm);
    m.on_choose_voice(move |i| {
        go(&|vm| {
            let first = vm.menu_state(|s| s.voice_slice().0);
            let out = vm.menu_state(|s| s.choose_voice(first + i.max(0) as usize));
            vm.menu_out(out);
        })
    });
    let go = with(vm);
    m.on_outside(move || go(&|vm| vm.set_overlay(Overlay::None)));
    let z = &o.zones;
    let go = with(vm);
    z.on_done(move || go(&|vm| vm.set_overlay(Overlay::None)));
    let go = with(vm);
    z.on_remove(move |i| go(&|vm| vm.zone_remove(i)));
    let go = with(vm);
    z.on_commit(move |k, x, y, w, h| go(&|vm| vm.zone_add(k, Rect::new(x, y, w, h))));
    let go = with(vm);
    z.on_set_kind(move |k| go(&|vm| vm.zone_set_kind(k)));
    let wk = Rc::downgrade(vm);
    z.on_pick(move |x, y| {
        let Some(v) = wk.upgrade() else { return -1 };
        let Ok(mut vm) = v.try_borrow_mut() else {
            return -1;
        };
        vm.zone_pick(x, y)
    });
}

impl Vm {
    /// The overlays (tests).
    pub fn overlays(&self) -> Option<&Overlays> {
        self.overlays.as_ref()
    }

    pub fn overlays_mut(&mut self) -> Option<&mut Overlays> {
        self.overlays.as_mut()
    }

    /// Runs `f` on the quick menu's state (a default one when there are no overlays).
    fn menu_state<T>(&mut self, f: impl FnOnce(&mut quick_menu::Menu) -> T) -> T {
        match self.overlays.as_mut() {
            Some(o) => f(&mut o.state),
            None => f(&mut quick_menu::Menu::default()),
        }
    }

    /// The game window as the overlays see it, while it is being read.
    fn overlay_game(&self) -> Option<GameWin> {
        let o = self.overlays.as_ref()?;
        let hwnd = self.reading.rt.as_ref()?.target()?.hwnd;
        o.game(Some(hwnd))
    }

    /// Opens or closes an overlay from the app (the hook does it for the quick menu key and Esc).
    pub fn set_overlay(&mut self, open: Overlay) {
        let esc_closes = match open {
            Overlay::Menu => self.overlays.as_ref().is_none_or(|o| o.state.esc_closes()),
            // Esc in zone edit first drops a zone being drawn: the app decides
            Overlay::Zones => false,
            Overlay::None => true,
        };
        if let Some(rt) = self.reading.rt.as_mut() {
            rt.send(Cmd::Overlay { open, esc_closes });
        }
        self.overlay_sync();
    }

    /// Makes the overlay windows match the overlay the hook has open, the game window and the reading state.
    pub fn overlay_sync(&mut self) {
        let Some(rt) = self.reading.rt.as_ref() else {
            return;
        };
        let mut want = rt.overlay();
        let status = rt.status();
        if self.overlays.as_ref().is_some_and(|o| o.closed) {
            return;
        }
        if self.overlays.is_none() {
            // no overlay windows (the renderer could not be set up): the quick menu key does nothing
            if want != Overlay::None {
                self.set_overlay_quiet(Overlay::None);
            }
            return;
        }
        let g = self.overlay_game();
        let usable = g.is_some_and(|g| g.front && !g.minimized && !g.client.is_empty());
        if want != Overlay::None && !usable {
            want = Overlay::None;
            self.set_overlay_quiet(Overlay::None);
        }
        let shown = self.overlays.as_ref().map_or(Overlay::None, |o| o.shown);
        if want != shown {
            let mut now = want;
            match shown {
                Overlay::Menu => self.with_overlays(|o| o.menu_off()),
                Overlay::Zones => self.with_overlays(|o| {
                    o.zones_off();
                    o.map = None;
                }),
                Overlay::None => {}
            }
            match want {
                Overlay::Menu => {
                    self.menu_fill();
                    let ok = g.is_some_and(|g| self.with_overlays(|o| o.menu_on(&g)));
                    if !ok {
                        now = Overlay::None;
                    }
                }
                // zone edit opens through `open_zone_edit`, which shows it first
                Overlay::Zones => {
                    if self.overlays.as_ref().is_none_or(|o| o.map.is_none()) {
                        now = Overlay::None;
                    }
                }
                Overlay::None => {}
            }
            self.with_overlays(|o| o.shown = now);
            if now != want {
                self.set_overlay_quiet(now);
            }
            self.overlay_error();
        } else if want == Overlay::Menu
            && let Some(g) = g
        {
            // follow the game when it moves
            self.with_overlays(|o| o.menu_on(&g));
        }
        let open = self.overlays.as_ref().map_or(Overlay::None, |o| o.shown);
        // reading waits while an overlay is open
        let hold = open != Overlay::None;
        if self.overlays.as_ref().is_some_and(|o| o.held != hold) {
            self.with_overlays(|o| o.held = hold);
            if let Some(rt) = self.reading.rt.as_mut() {
                rt.send(Cmd::Hold(hold));
            }
        }
        // the pill: over the game being read, in front, while no other overlay is open
        let reading = matches!(status, Status::Reading | Status::Paused);
        let at = self
            .win()
            .map_or(PillAt::TopRight, |w| PillAt::of(w.get_pill_position()));
        let pill = open == Overlay::None && reading && usable;
        self.push_pill(status);
        self.with_overlays(|o| o.pill_on(pill, g.as_ref(), at));
        self.overlay_error();
    }

    fn with_overlays<T: Default>(&mut self, f: impl FnOnce(&mut Overlays) -> T) -> T {
        self.overlays.as_mut().map(f).unwrap_or_default()
    }

    /// Sets the hook's overlay without a new sync (inside one).
    fn set_overlay_quiet(&mut self, open: Overlay) {
        if let Some(rt) = self.reading.rt.as_mut() {
            rt.send(Cmd::Overlay {
                open,
                esc_closes: open != Overlay::Zones,
            });
        }
    }

    /// An overlay that could not be shown: said once in `reading.log`.
    fn overlay_error(&mut self) {
        if let Some(e) = self.overlays.as_mut().and_then(|o| o.last_error.take()) {
            self.err(&format!("overlay: {e}"));
        }
    }

    /// The pill's dot, speaker and wave.
    fn push_pill(&self, status: Status) {
        let (Some(o), Some(w)) = (self.overlays.as_ref(), self.win()) else {
            return;
        };
        let speaking = self
            .voices
            .speech
            .as_ref()
            .map(speech::Speech::status)
            .is_some_and(|s| s.speaking.is_some() && s.test.is_none() && !s.muted);
        let who = w.get_now_who();
        let who = who.trim().trim_end_matches(':').trim_end();
        let speaker = if speaking && w.get_show_speaker() {
            who
        } else {
            ""
        };
        let p = &o.pill;
        if p.get_speaker().as_str() != speaker {
            p.set_speaker(speaker.into());
        }
        p.set_ok(status == Status::Reading);
        p.set_speaking(speaking);
    }

    /// The quick menu's state from the current game, when it opens.
    fn menu_fill(&mut self) {
        let Some(g) = self.current.and_then(|i| self.games.get(i)) else {
            return;
        };
        let paused = self.reading.rt.as_ref().is_some_and(|r| r.is_paused());
        let v = &g.profile.voice;
        let n = self.voices.list.len();
        let voice = (n > 0).then(|| self.voices.index_of(&v.id));
        let (speed, volume) = (v.speed, v.volume);
        let automatic = g.profile.mode == dr_runtime::Mode::Auto;
        let muted = self.muted;
        self.with_overlays(|o| {
            let s = &mut o.state;
            s.open();
            s.stopped = paused;
            s.muted = muted;
            s.speed = (speed * 10.0).round() / 10.0;
            s.volume = volume.clamp(0.0, 1.0);
            s.automatic = automatic;
            s.voices = n;
            s.voice = voice;
        });
        self.push_menu();
    }

    /// The quick menu's state to its window.
    fn push_menu(&self) {
        let Some(o) = self.overlays.as_ref() else {
            return;
        };
        let s = &o.state;
        let m = &o.menu;
        let game = self
            .current
            .and_then(|i| self.games.get(i))
            .map(|g| g.profile.name.clone())
            .unwrap_or_default();
        m.set_game(game.into());
        m.set_highlight(s.highlight as i32);
        m.set_stopped(s.stopped);
        m.set_muted(s.muted);
        m.set_voice(
            s.voice
                .and_then(|i| self.voices.list.get(i))
                .map(|v| v.name.clone())
                .unwrap_or_default()
                .into(),
        );
        m.set_speed(s.speed);
        m.set_speed_pos(
            (s.speed - quick_menu::SPEED_MIN) / (quick_menu::SPEED_MAX - quick_menu::SPEED_MIN),
        );
        m.set_volume(s.volume);
        m.set_automatic(s.automatic);
        m.set_quit_armed(s.quit_armed);
        m.set_voice_open(s.voice_open);
        let (first, len) = s.voice_slice();
        let rows = self.voices.list[first.min(self.voices.list.len())..]
            .iter()
            .take(len)
            .map(voices::row)
            .collect();
        sync(&o.voices, rows);
        m.set_voice_hi(s.voice_hi as i32 - first as i32);
        m.set_voice_chosen(s.voice.map_or(-1, |c| c as i32 - first as i32));
    }

    /// A key the open overlay took (from the hook).
    pub fn overlay_nav(&mut self, n: Nav) {
        let shown = self.overlays.as_ref().map_or(Overlay::None, |o| o.shown);
        match shown {
            Overlay::Menu => {
                let out = self.menu_state(|s| s.key(n));
                self.menu_out(out);
            }
            Overlay::Zones => match n {
                Nav::Delete => {
                    let sel = self.main.selected;
                    if let Some(i) = sel {
                        self.zone_remove(i as i32);
                    }
                }
                Nav::Esc => {
                    let draft = self.overlays.as_ref().is_some_and(|o| o.zones.get_draft());
                    if draft {
                        self.with_overlays(|o| o.zones.set_draft(false));
                    } else {
                        self.set_overlay(Overlay::None);
                    }
                }
                _ => {}
            },
            Overlay::None => {}
        }
    }

    /// Does what the quick menu asked for.
    pub fn menu_out(&mut self, out: Out) {
        match out {
            Out::None => {}
            Out::Close => self.set_overlay(Overlay::None),
            Out::ManageZones => {
                if let Err(e) = self.open_zone_edit() {
                    self.err(&format!("zone edit: {e}"));
                }
            }
            Out::Pause(p) => {
                if self.reading.rt.as_ref().is_some_and(|r| r.is_paused() != p) {
                    self.toggle_pause();
                }
            }
            Out::Mute(m) => {
                self.muted = m;
                self.speech_cmd(speech::Cmd::Mute(m));
            }
            Out::Voice(i) => self.pick_game_voice(i as i32),
            Out::Preview(i) => self.preview_voice(i),
            Out::Speed(s) => self.set_game_voice(Some(s), None, true),
            Out::Volume(v) => self.set_game_voice(None, Some(v), true),
            Out::Mode(a) => {
                self.set_mode(a);
                self.push_profile_settings();
            }
            Out::Settings => {
                self.set_overlay(Overlay::None);
                self.open_main(Page::Settings);
            }
            Out::Quit => {
                self.set_overlay_quiet(Overlay::None);
                self.with_overlays(|o| o.hide_all());
                crate::shell::quit();
            }
        }
        // Esc goes back from the voice list rather than closing. Only the Esc rule is sent: the hook may have
        // closed the menu meanwhile (the quick menu key while the profile was saved), and that must stay closed.
        let shown = self.overlays.as_ref().map_or(Overlay::None, |o| o.shown);
        if shown == Overlay::Menu {
            let esc = self.overlays.as_ref().is_none_or(|o| o.state.esc_closes());
            if let Some(rt) = self.reading.rt.as_mut() {
                rt.send(Cmd::EscCloses(esc));
            }
        }
        self.push_menu();
    }

    /// The speed slider of the menu moved to `pos` (0 to 1): the speed snaps to 0.1 as with the keys, and is
    /// saved when the slider is let go.
    fn menu_speed(&mut self, pos: f32, save: bool) {
        let speed = quick_menu::SPEED_MIN
            + pos.clamp(0.0, 1.0) * (quick_menu::SPEED_MAX - quick_menu::SPEED_MIN);
        self.with_overlays(|o| {
            o.state.set_speed(speed);
        });
        let s = self.menu_state(|s| s.speed);
        self.set_game_voice(Some(s), None, save);
        self.push_menu();
    }

    /// The volume slider of the menu moved (saved when it is let go).
    fn menu_volume(&mut self, v: f32, save: bool) {
        let v = v.clamp(0.0, 1.0);
        self.with_overlays(|o| o.state.volume = v);
        self.set_game_voice(None, Some(v), save);
    }

    /// Speed or volume of the current game from the quick menu: the speech thread gets it at once.
    fn set_game_voice(&mut self, speed: Option<f32>, volume: Option<f32>, save: bool) {
        let Some(g) = self.current else { return };
        let voice = &mut self.games[g].profile.voice;
        if let Some(s) = speed {
            voice.speed = s;
        }
        if let Some(v) = volume {
            voice.volume = v;
        }
        self.send_voice();
        if save {
            self.save_current();
            self.push_profile_settings();
        }
    }

    /// Space in the voice list: a sample of voice `i` at the game's speed and volume.
    fn preview_voice(&mut self, i: usize) {
        let (Some(v), Some(g)) = (
            self.voices.list.get(i),
            self.current.and_then(|i| self.games.get(i)),
        ) else {
            return;
        };
        let (id, speed, volume) = (v.id.clone(), g.profile.voice.speed, g.profile.voice.volume);
        self.test_id += 1;
        let tid = self.test_id;
        self.speech_cmd(speech::Cmd::Test {
            id: tid,
            text: voices::SAMPLE.into(),
            voice: id,
            speed,
            volume: volume.clamp(0.0, 1.0),
        });
    }

    /// Settings in the quick menu: the main window comes to the front on that page.
    fn open_main(&mut self, page: Page) {
        let Some(w) = self.win() else { return };
        // the main window may be hidden in the tray (M5)
        crate::shell::show_main(&w);
        w.set_switcher_open(false);
        w.set_page(page);
        self.page_changed();
        #[cfg(windows)]
        if self.overlays.as_ref().is_some_and(Overlays::is_win32) {
            crate::overlay::win::bring_to_front(w.window());
        }
    }

    /// Edit zones: a frozen screenshot of the game, and zone edit over it.
    pub fn open_zone_edit(&mut self) -> Result<(), String> {
        let i = self.current.ok_or("no current game")?;
        let hwnd = self
            .reading
            .rt
            .as_ref()
            .and_then(|r| r.target())
            .ok_or("the game is not being read")?
            .hwnd;
        let g = self.overlay_game().ok_or("the game window is gone")?;
        let win = self
            .target_win(hwnd)
            .ok_or("the game window is not listed")?;
        let shot = self.shoot(&win)?;
        self.adopt_shot(i, &shot)
            .map_err(|_| "the screenshot could not be saved".to_string())?;
        self.main.select(None);
        self.main.draw = false;
        // the menu goes first, then zone edit shows over what the capture covers
        self.with_overlays(|o| {
            o.menu_off();
            o.shown = Overlay::None;
        });
        let shown = self.with_overlays(|o| {
            o.zones
                .set_shot(super::image_of(shot.width, shot.height, &shot.rgba));
            o.zones.set_selected(-1);
            o.zones.set_draft(false);
            if !o.zones_on(&g) {
                return false;
            }
            o.map = Some(CaptureMap::new(
                g.frame,
                o.zones_scale(),
                shot.width,
                shot.height,
            ));
            o.shown = Overlay::Zones;
            true
        });
        if !shown {
            self.overlay_error();
            self.set_overlay(Overlay::None);
            return Err("zone edit could not be shown".into());
        }
        if let Some(rt) = self.reading.rt.as_mut() {
            rt.send(Cmd::Overlay {
                open: Overlay::Zones,
                esc_closes: false,
            });
        }
        self.push_zone_edit();
        self.overlay_sync();
        Ok(())
    }

    /// The game window in the window list (for its screenshot).
    fn target_win(&self, hwnd: isize) -> Option<dr_runtime::windows_list::WinInfo> {
        if let Some(w) = self.windows.iter().find(|w| w.hwnd == hwnd) {
            return Some(w.clone());
        }
        let real = self.real.as_ref()?;
        real.desktop
            .windows_quick(&self.profile_exes())
            .into_iter()
            .find(|w| w.hwnd == hwnd)
    }

    /// The current game's zones to zone edit.
    fn push_zone_edit(&self) {
        let Some(o) = self.overlays.as_ref() else {
            return;
        };
        let Some(m) = o.map.as_ref() else { return };
        sync(&o.edit_zones, zone_edit::view_zones(&self.main, m));
        o.zones.set_bar_clear(ModelRc::new(VecModel::from(
            zone_edit::bar_clear_of(&self.main, m).to_vec(),
        )));
        o.zones
            .set_selected(self.main.selected.map_or(-1, |i| i as i32));
    }

    fn zone_map(&self) -> Option<CaptureMap> {
        self.overlays.as_ref()?.map
    }

    /// A new zone drawn in zone edit (its kind picked): saved, and read from now on.
    fn zone_add(&mut self, kind: crate::Kind, r: Rect) {
        let Some(m) = self.zone_map() else { return };
        if zone_edit::add(&mut self.main, &m, r, super::disk_kind(kind)).is_some() {
            self.save_main();
            self.push_main();
        }
        self.with_overlays(|o| o.zones.set_draft(false));
        self.push_zone_edit();
    }

    /// The kind picker under the selected zone: it reads as that kind from now on, saved.
    fn zone_set_kind(&mut self, kind: crate::Kind) {
        if self.main.selected.is_some() && self.main.set_kind(super::disk_kind(kind)) {
            self.save_main();
            self.push_main();
        }
        self.push_zone_edit();
    }

    /// The x of the selected zone, or Delete: the zone goes, saved.
    fn zone_remove(&mut self, i: i32) {
        let Ok(i) = usize::try_from(i) else { return };
        if zone_edit::remove(&mut self.main, i) {
            self.save_main();
            self.push_main();
        }
        self.push_zone_edit();
    }

    /// A click in zone edit: the zone under it is selected (or none).
    fn zone_pick(&mut self, x: f32, y: f32) -> i32 {
        let Some(m) = self.zone_map() else { return -1 };
        let hit = zone_edit::pick(&self.main, &m, x, y);
        self.main.select(hit);
        self.push_main();
        hit.map_or(-1, |i| i as i32)
    }
}
