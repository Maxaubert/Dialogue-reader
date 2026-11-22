//! View-model layer (spec 3.2): plain Rust state for profiles, windows, screenshots and zones, and the Slint
//! models made from it. Slint callbacks come here; the window only shows what this layer pushes.
//!
//! Two sources:
//! - [`Source::Fake`]: the mockup data of `fake.rs` (render-screens, the screen tests, the nav tests).
//! - [`Source::Real`]: profiles in a data folder and a [`Desktop`] for windows and screenshots. The app passes
//!   `%APPDATA%\DialogReader` and [`RealDesktop`]; tests pass a temp folder and a fake desktop.
//!
//! Memory: only the current game's screenshot is decoded at full size (for pages 10 and 11). The other games
//! keep a small cover, made the first time the game switcher opens, and the screenshot size from the PNG header.

mod live;
pub mod zones;

pub use live::{LiveRun, NOTE_CLOSED, NOTE_FAILED, NOTE_LIVE, NOTE_MINIMIZED};

use std::cell::RefCell;
use std::rc::Rc;

use dr_runtime::profile::{self, Problem};
use dr_runtime::windows_list::WinInfo;
use dr_runtime::{Paths, Profile, Shot, ZoneCfg};
use slint::{
    ComponentHandle, Image, Model, ModelRc, Rgba8Pixel, SharedPixelBuffer, SharedString, VecModel,
};

use crate::geom::Rect;
use crate::{GameData, Kind, MainWindow, Page, WindowData, ZoneData};
use zones::{EdZone, Editor};

/// Where windows and screenshots come from.
pub trait Desktop {
    /// The windows to offer on page 02, games first. `profile_exes` count as games.
    fn windows(&self, profile_exes: &[String]) -> Vec<WinInfo>;
    /// One screenshot of the window.
    fn screenshot(&self, win: &WinInfo) -> Result<Shot, String>;
    /// The window is minimized now (it gives no picture then).
    fn is_minimized(&self, win: &WinInfo) -> bool {
        win.minimized
    }
    /// The window still exists.
    fn is_open(&self, _win: &WinInfo) -> bool {
        true
    }
    /// The windows without their icons (cheaper), to find a running game for the live view.
    fn windows_quick(&self, profile_exes: &[String]) -> Vec<WinInfo> {
        self.windows(profile_exes)
    }
    /// `windows_quick`, handed to `done` (from another thread when the list is slow to read: the real desktop).
    fn windows_quick_then(
        &self,
        profile_exes: Vec<String>,
        done: Box<dyn FnOnce(Vec<WinInfo>) + Send>,
    ) {
        done(self.windows_quick(&profile_exes));
    }
    /// A live view of the window: frames fit inside `max_w` x `max_h` and go to `sink` on another thread until
    /// the returned handle is dropped (src/vm/live.rs).
    fn live(
        &self,
        _win: &WinInfo,
        _max_w: u32,
        _max_h: u32,
        _sink: Box<dyn dr_runtime::live::LiveSink>,
    ) -> Result<Box<dyn LiveRun>, String> {
        Err("no live view here".into())
    }
}

/// The real desktop: `EnumWindows` and Windows Graphics Capture.
pub struct RealDesktop;

impl Desktop for RealDesktop {
    fn windows(&self, profile_exes: &[String]) -> Vec<WinInfo> {
        dr_runtime::windows_list::list(&dr_runtime::windows_list::ListOpts::for_app(
            profile_exes.to_vec(),
        ))
    }

    fn screenshot(&self, win: &WinInfo) -> Result<Shot, String> {
        dr_runtime::screenshot::capture(win.hwnd)
    }

    fn is_minimized(&self, win: &WinInfo) -> bool {
        dr_runtime::windows_list::is_minimized(win.hwnd)
    }

    fn is_open(&self, win: &WinInfo) -> bool {
        dr_runtime::windows_list::is_open(win.hwnd)
    }

    fn windows_quick(&self, profile_exes: &[String]) -> Vec<WinInfo> {
        let mut o = dr_runtime::windows_list::ListOpts::for_app(profile_exes.to_vec());
        o.icons = false;
        dr_runtime::windows_list::list(&o)
    }

    fn windows_quick_then(
        &self,
        profile_exes: Vec<String>,
        done: Box<dyn FnOnce(Vec<WinInfo>) + Send>,
    ) {
        // about 10 ms: not on the UI thread. A thread that cannot start (out of resources) never answers; the
        // next page change starts a new search.
        let _ = std::thread::Builder::new()
            .name("window-list".into())
            .spawn(move || {
                let mut o = dr_runtime::windows_list::ListOpts::for_app(profile_exes);
                o.icons = false;
                done(dr_runtime::windows_list::list(&o));
            });
    }

    fn live(
        &self,
        win: &WinInfo,
        max_w: u32,
        max_h: u32,
        sink: Box<dyn dr_runtime::live::LiveSink>,
    ) -> Result<Box<dyn LiveRun>, String> {
        dr_runtime::live::Live::start(win.hwnd, max_w, max_h, dr_runtime::live::INTERVAL, sink)
            .map(|l| Box::new(l) as Box<dyn LiveRun>)
    }
}

pub enum Source {
    Fake,
    Real {
        paths: Paths,
        desktop: Box<dyn Desktop>,
    },
}

/// Slint kind from a profile kind, and back.
pub fn ui_kind(k: dr_runtime::Kind) -> Kind {
    match k {
        dr_runtime::Kind::Dialogue => Kind::Dialogue,
        dr_runtime::Kind::Speaker => Kind::Speaker,
        dr_runtime::Kind::Notification => Kind::Pickups,
        dr_runtime::Kind::Prompt => Kind::Prompts,
        dr_runtime::Kind::Ignore => Kind::Ignore,
    }
}

pub fn disk_kind(k: Kind) -> dr_runtime::Kind {
    match k {
        Kind::Dialogue => dr_runtime::Kind::Dialogue,
        Kind::Speaker => dr_runtime::Kind::Speaker,
        Kind::Pickups => dr_runtime::Kind::Notification,
        Kind::Prompts => dr_runtime::Kind::Prompt,
        Kind::Ignore => dr_runtime::Kind::Ignore,
    }
}

pub fn image_of(width: u32, height: u32, rgba: &[u8]) -> Image {
    Image::from_rgba8(SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(
        rgba, width, height,
    ))
}

/// Width of a cover (game switcher, title bar, page 06): enough for those tiles, a few hundred KB each.
pub const COVER_W: u32 = 480;

/// A small copy of a screenshot for covers.
pub fn cover_of(s: &Shot) -> Image {
    let Some(view) =
        image::ImageBuffer::<image::Rgba<u8>, &[u8]>::from_raw(s.width, s.height, &s.rgba[..])
    else {
        return Image::default();
    };
    if s.width <= COVER_W {
        return image_of(s.width, s.height, &s.rgba);
    }
    let h = ((s.height as u64 * COVER_W as u64) / s.width as u64).max(1) as u32;
    let small = image::imageops::thumbnail(&view, COVER_W, h);
    image_of(small.width(), small.height(), small.as_raw())
}

/// The profile is for this window: same exe (case does not matter), and the title holds `title_contains` when it
/// is set. A profile without an exe (the game's process could not be read) matches by title only, and an
/// unreadable window never matches by its empty exe.
pub fn profile_matches(p: &Profile, w: &WinInfo) -> bool {
    let title_ok = |t: &str| w.title.to_lowercase().contains(&t.to_lowercase());
    let title = p
        .matches
        .title_contains
        .as_deref()
        .filter(|t| !t.trim().is_empty());
    if p.matches.exe.is_empty() {
        return title.is_some_and(title_ok);
    }
    !w.exe.is_empty() && w.exe.eq_ignore_ascii_case(&p.matches.exe) && title.is_none_or(title_ok)
}

fn zone_row(z: &EdZone) -> ZoneData {
    ZoneData {
        name: z.name.as_str().into(),
        kind: ui_kind(z.kind),
        on: z.on,
        x: z.rect.x,
        y: z.rect.y,
        w: z.rect.w,
        h: z.rect.h,
    }
}

/// Zones from a profile, in pixels of a `w` x `h` screenshot.
pub fn ed_zones(p: &Profile, w: u32, h: u32) -> Vec<EdZone> {
    p.zones
        .iter()
        .map(|z| {
            let [x, y, zw, zh] = z.rect_in(w, h);
            EdZone {
                name: z.name.clone(),
                kind: z.kind,
                on: z.on,
                rect: Rect::new(x, y, zw, zh),
            }
        })
        .collect()
}

/// Zones for a profile: in pixels of the screenshot they were drawn on, with that size as `base`.
pub fn zone_cfgs(e: &Editor) -> Vec<ZoneCfg> {
    let base = [e.img_w.round() as u32, e.img_h.round() as u32];
    e.zones
        .iter()
        .map(|z| ZoneCfg {
            name: z.name.clone(),
            kind: z.kind,
            on: z.on,
            rect: [z.rect.x, z.rect.y, z.rect.w, z.rect.h],
            base,
        })
        .collect()
}

/// Updates `model` to `rows`, row by row when the count is the same (keeps the list items alive).
fn sync<T: Clone + PartialEq + 'static>(model: &VecModel<T>, rows: Vec<T>) {
    if model.row_count() == rows.len() {
        for (i, r) in rows.into_iter().enumerate() {
            if model.row_data(i).as_ref() != Some(&r) {
                model.set_row_data(i, r);
            }
        }
    } else {
        model.set_vec(rows);
    }
}

struct Real {
    paths: Paths,
    desktop: Box<dyn Desktop>,
}

/// One profile with what the UI needs of its screenshot.
struct Game {
    profile: Profile,
    /// Width and height of the saved screenshot (from the PNG header), `None` when there is none.
    shot_size: Option<(u32, u32)>,
    /// A small copy for the switcher and the title bar. `None` until it is made.
    cover: Option<Image>,
}

/// A window on page 02, to find it again after a refresh or a search.
#[derive(PartialEq)]
struct WinKey {
    hwnd: isize,
    pid: u32,
    title: String,
    exe: String,
}

impl WinKey {
    fn of(w: &WinInfo) -> WinKey {
        WinKey {
            hwnd: w.hwnd,
            pid: w.pid,
            title: w.title.clone(),
            exe: w.exe.clone(),
        }
    }
}

/// A rename being typed: which editor (true: setup), which zone, the text so far.
struct PendingRename {
    setup: bool,
    zone: usize,
    name: String,
}

pub struct Vm {
    w: slint::Weak<MainWindow>,
    real: Option<Real>,
    games: Vec<Game>,
    current: Option<usize>,
    /// Every window of the last refresh, and the ones page 02 shows (games, then others).
    windows: Vec<WinInfo>,
    shown_games: Vec<usize>,
    shown_others: Vec<usize>,
    search: String,
    /// Page 03: the window being set up, its screenshot, its zones.
    setup_win: Option<WinInfo>,
    setup_shot: Option<Shot>,
    setup: Editor,
    /// Pages 10 and 11: the current profile's zones.
    main: Editor,
    rename: Option<PendingRename>,
    /// The live view on the stages (live.rs).
    live: live::LiveState,
    setup_zones: Rc<VecModel<ZoneData>>,
    zones: Rc<VecModel<ZoneData>>,
    games_running: Rc<VecModel<WindowData>>,
    other_windows: Rc<VecModel<WindowData>>,
    game_rows: Rc<VecModel<GameData>>,
}

pub type Shared = Rc<RefCell<Vm>>;

/// Fills the window from `source` and wires every view-model callback.
pub fn install(w: &MainWindow, source: Source) -> Shared {
    let fake_setup = || Editor::new(crate::fake::ed_zones(true), 1600.0, 900.0);
    let mut vm = Vm {
        w: w.as_weak(),
        real: None,
        games: Vec::new(),
        current: None,
        windows: Vec::new(),
        shown_games: Vec::new(),
        shown_others: Vec::new(),
        search: String::new(),
        setup_win: None,
        setup_shot: None,
        setup: fake_setup(),
        main: Editor::new(crate::fake::ed_zones(false), 1600.0, 900.0),
        rename: None,
        live: live::LiveState::default(),
        setup_zones: Rc::new(VecModel::default()),
        zones: Rc::new(VecModel::default()),
        games_running: Rc::new(VecModel::default()),
        other_windows: Rc::new(VecModel::default()),
        game_rows: Rc::new(VecModel::default()),
    };
    w.set_setup_zones(ModelRc::from(vm.setup_zones.clone()));
    w.set_zones(ModelRc::from(vm.zones.clone()));
    w.set_games_running(ModelRc::from(vm.games_running.clone()));
    w.set_other_windows(ModelRc::from(vm.other_windows.clone()));
    w.set_games(ModelRc::from(vm.game_rows.clone()));
    // the fake (mockup) state: zone 1 selected on 03 and 11, as the mockups show
    vm.setup.select(Some(1));
    vm.setup.kind = dr_runtime::Kind::Dialogue;
    vm.main.select(Some(1));
    vm.main.draw = false;
    vm.main.kind = dr_runtime::Kind::Dialogue;
    match source {
        Source::Fake => {
            crate::fake::fill(w);
            vm.games_running.set_vec(crate::fake::windows().0);
            vm.other_windows.set_vec(crate::fake::windows().1);
            vm.game_rows.set_vec(crate::fake::games());
        }
        Source::Real { paths, desktop } => {
            // voices, the activity log and zone edit stay fake until M3 and M4
            crate::fake::fill(w);
            w.set_real_shots(true);
            w.set_extra_zones(ModelRc::default());
            w.set_recent(ModelRc::default());
            w.set_log(ModelRc::default());
            vm.real = Some(Real { paths, desktop });
            vm.setup = Editor::new(Vec::new(), 1600.0, 900.0);
            vm.main = Editor::new(Vec::new(), 1600.0, 900.0);
            vm.main.draw = false;
            vm.load_profiles();
            vm.refresh_windows();
            if vm.games.is_empty() {
                w.set_page(Page::Welcome);
            } else {
                vm.open_game(0, None);
                w.set_page(Page::Overview);
            }
        }
    }
    vm.push_setup();
    vm.push_main();
    let vm = Rc::new(RefCell::new(vm));
    wire(w, &vm);
    live::wire(w, &vm);
    vm
}

impl Vm {
    fn win(&self) -> Option<MainWindow> {
        self.w.upgrade()
    }

    fn notice(&self, text: &str) {
        if let Some(w) = self.win() {
            w.set_notice(text.into());
        }
    }

    fn load_profiles(&mut self) {
        let Some(real) = &self.real else { return };
        let (profiles, problems) = profile::load_all(&real.paths);
        self.games = profiles
            .into_iter()
            .map(|p| Game {
                shot_size: image::image_dimensions(real.paths.profile_png(&p.id)).ok(),
                profile: p,
                cover: None,
            })
            .collect();
        if let Some(msg) = problems_text(&problems) {
            self.notice(&msg);
        }
        self.push_games();
    }

    /// Makes the covers that are still missing (the first time the switcher opens).
    fn load_covers(&mut self) {
        let Some(real) = &self.real else { return };
        let mut changed = false;
        for g in self.games.iter_mut().filter(|g| g.cover.is_none()) {
            let shot = Shot::load_png(&real.paths.profile_png(&g.profile.id)).ok();
            g.cover = Some(shot.as_ref().map(cover_of).unwrap_or_default());
            changed = true;
        }
        if changed {
            self.push_games();
        }
    }

    /// The exes of the profiles (for the game group of page 02). A profile without an exe has none.
    fn profile_exes(&self) -> Vec<String> {
        self.games
            .iter()
            .map(|g| g.profile.matches.exe.clone())
            .filter(|e| !e.is_empty())
            .collect()
    }

    fn push_games(&self) {
        let rows = self
            .games
            .iter()
            .map(|g| {
                let n = g.profile.zones.len();
                let is_running = self.windows.iter().any(|w| profile_matches(&g.profile, w));
                GameData {
                    name: g.profile.name.as_str().into(),
                    meta: format!(
                        "{} · {n} zone{}",
                        if is_running { "Running" } else { "Not running" },
                        if n == 1 { "" } else { "s" }
                    )
                    .into(),
                    art: SharedString::default(),
                    running: is_running,
                    cover: g.cover.clone().unwrap_or_default(),
                }
            })
            .collect();
        sync(&self.game_rows, rows);
        if let (Some(w), Some(i)) = (self.win(), self.current) {
            w.set_current_game(i as i32);
        }
    }

    /// Makes game `i` the current profile: its zones and screenshot on 10 and 11. `full` is its screenshot when
    /// the caller has it already (Finish); otherwise it is read from disk.
    fn open_game(&mut self, i: usize, full: Option<&Shot>) {
        let Some(real) = &self.real else { return };
        if i >= self.games.len() {
            return;
        }
        let loaded;
        let shot = match full {
            Some(s) => Some(s),
            None => {
                loaded = Shot::load_png(&real.paths.profile_png(&self.games[i].profile.id)).ok();
                loaded.as_ref()
            }
        };
        let g = &mut self.games[i];
        if let Some(s) = shot {
            g.shot_size = Some((s.width, s.height));
            if g.cover.is_none() {
                g.cover = Some(cover_of(s));
            }
        }
        self.current = Some(i);
        let g = &self.games[i];
        let (sw, sh) = match g.shot_size {
            Some(s) => s,
            // no screenshot on disk: use the size the zones were drawn on
            None => g
                .profile
                .zones
                .first()
                .map(|z| (z.base[0], z.base[1]))
                .unwrap_or((1920, 1080)),
        };
        let mut e = Editor::new(ed_zones(&g.profile, sw, sh), sw as f32, sh as f32);
        e.draw = false;
        // the live view of the game before is for its zones: stop it before they go (the tick starts the new one)
        self.live_stop();
        self.main = e;
        if let Some(w) = self.win() {
            w.set_current_game(i as i32);
            w.set_shot(
                shot.map(|s| image_of(s.width, s.height, &s.rgba))
                    .unwrap_or_default(),
            );
            w.set_shot_w(sw as f32);
            w.set_shot_h(sh as f32);
            w.set_shot_info(format!("Screenshot · {sw} × {sh} · Fit to window").into());
        }
        self.push_profile_settings();
        self.push_games();
        self.push_main();
    }

    /// The current game's mode and keys (Overview and Settings show them; setup pages share them).
    fn push_profile_settings(&self) {
        let (Some(w), Some(g)) = (self.win(), self.current.and_then(|i| self.games.get(i))) else {
            return;
        };
        w.set_automatic(g.profile.mode == dr_runtime::Mode::Auto);
        w.set_read_key(g.profile.keys.read.as_str().into());
        w.set_menu_key(g.profile.keys.quick_menu.as_str().into());
    }

    /// The window picked on page 02 now, to find it again in a new list.
    fn picked_key(&self) -> Option<WinKey> {
        let w = self.win()?;
        self.picked(w.get_picked_window()).map(WinKey::of)
    }

    fn refresh_windows(&mut self) {
        let Some(real) = &self.real else { return };
        // read the pick before the list changes: its index points into the old list
        let before = self.picked_key();
        self.windows = real.desktop.windows(&self.profile_exes());
        self.apply_search(before);
        self.push_games();
    }

    /// Fills the page 02 lists. `before` (the window picked before) stays picked when it is still listed.
    fn apply_search(&mut self, before: Option<WinKey>) {
        let q = self.search.trim().to_lowercase();
        let hit = |w: &WinInfo| {
            q.is_empty() || w.title.to_lowercase().contains(&q) || w.exe.to_lowercase().contains(&q)
        };
        self.shown_games = (0..self.windows.len())
            .filter(|&i| self.windows[i].game && hit(&self.windows[i]))
            .collect();
        self.shown_others = (0..self.windows.len())
            .filter(|&i| !self.windows[i].game && hit(&self.windows[i]))
            .collect();
        let row = |w: &WinInfo| WindowData {
            name: w.title.as_str().into(),
            exe: w.exe.as_str().into(),
            meta: w.meta().into(),
            art: SharedString::default(),
            icon: w
                .icon
                .as_ref()
                .map(|i| image_of(i.width, i.height, &i.rgba))
                .unwrap_or_default(),
        };
        self.games_running.set_vec(
            self.shown_games
                .iter()
                .map(|&i| row(&self.windows[i]))
                .collect::<Vec<_>>(),
        );
        self.other_windows.set_vec(
            self.shown_others
                .iter()
                .map(|&i| row(&self.windows[i]))
                .collect::<Vec<_>>(),
        );
        let again = before.and_then(|k| {
            self.shown_games
                .iter()
                .chain(&self.shown_others)
                .position(|&i| WinKey::of(&self.windows[i]) == k)
        });
        // nothing picked before: the first game, but never an other window on its own
        let pick = again.or((!self.shown_games.is_empty()).then_some(0));
        if let Some(w) = self.win() {
            w.set_picked_window(pick.map_or(-1, |i| i as i32));
        }
    }

    fn picked(&self, i: i32) -> Option<&WinInfo> {
        let i = usize::try_from(i).ok()?;
        let idx = if i < self.shown_games.len() {
            self.shown_games[i]
        } else {
            *self.shown_others.get(i - self.shown_games.len())?
        };
        self.windows.get(idx)
    }

    /// One screenshot of `win`; a minimized window is refused at once (it would only time out).
    fn shoot(&self, win: &WinInfo) -> Result<Shot, String> {
        let Some(real) = &self.real else {
            return Err("no desktop".into());
        };
        if real.desktop.is_minimized(win) {
            return Err("the window is minimized. Open it, then try again".into());
        }
        real.desktop.screenshot(win)
    }

    /// Continue on page 02: one screenshot of the picked window.
    fn take_setup_shot(&mut self) -> bool {
        if self.real.is_none() {
            return true;
        }
        let Some(w) = self.win() else { return false };
        let Some(win) = self.picked(w.get_picked_window()).cloned() else {
            self.notice("Pick a window first. Start your game, then press Refresh.");
            return false;
        };
        // spec 2.8: the profile follows the game by its exe, so one game has one profile
        if let Some(g) = self
            .games
            .iter()
            .find(|g| profile_matches(&g.profile, &win))
        {
            self.notice(&format!(
                "{} already has a profile. Open it with the game switcher (Ctrl+K) to change its zones.",
                g.profile.name
            ));
            return false;
        }
        match self.shoot(&win) {
            Ok(shot) => {
                let same_window = self
                    .setup_win
                    .as_ref()
                    .is_some_and(|o| o.exe == win.exe && o.title == win.title);
                if same_window {
                    self.setup.rescale(shot.width as f32, shot.height as f32);
                } else {
                    self.setup = Editor::new(Vec::new(), shot.width as f32, shot.height as f32);
                }
                let img = image_of(shot.width, shot.height, &shot.rgba);
                w.set_setup_shot(img.clone());
                w.set_setup_shot_w(shot.width as f32);
                w.set_setup_shot_h(shot.height as f32);
                w.set_setup_game(win.title.as_str().into());
                w.set_setup_cover(img);
                self.setup_shot = Some(shot);
                self.setup_win = Some(win);
                self.notice("");
                self.push_setup();
                // page 03 comes next: its kind picker shows the setup editor's kind
                self.push_tools(&w, &self.setup);
                true
            }
            Err(e) => {
                self.notice(&format!("No screenshot of {}: {e}.", win.title));
                false
            }
        }
    }

    /// Finish on page 06: saves the new profile and its screenshot, and makes it the current game.
    fn finish_setup(&mut self) -> bool {
        let Some(w) = self.win() else { return false };
        let Some(real) = &self.real else { return true };
        let (Some(win), Some(shot)) = (&self.setup_win, &self.setup_shot) else {
            self.notice("No game window is picked yet. Go back to step 1 and pick your game.");
            return false;
        };
        // every id with a file on disk, also files that were not loaded (newer format, kept as .bad)
        let mut taken = profile::ids_on_disk(&real.paths);
        taken.extend(self.games.iter().map(|g| g.profile.id.clone()));
        let id = profile::new_id(&win.title, &taken);
        let mut p = Profile::new(&id, &win.title, &win.exe);
        if win.exe.is_empty() {
            // the game's process could not be read (it runs as administrator, or anti-cheat hides it)
            p.matches.title_contains = Some(win.title.clone());
        }
        p.mode = if w.get_automatic() {
            dr_runtime::Mode::Auto
        } else {
            dr_runtime::Mode::Manual
        };
        p.keys.read = w.get_read_key().into();
        p.keys.quick_menu = w.get_menu_key().into();
        // the voice list is sample data until M3a, so no voice id is saved yet; speed and volume are
        p.voice.speed = (w.get_voice_speed() * 2.0).clamp(0.5, 2.0);
        p.voice.volume = w.get_voice_volume().clamp(0.0, 1.0);
        p.zones = zone_cfgs(&self.setup);
        if let Err(e) = shot
            .save_png(&real.paths.profile_png(&id))
            .and_then(|_| profile::save(&real.paths, &p))
        {
            self.notice(&format!("The game could not be saved: {e}"));
            return false;
        }
        let shot = self.setup_shot.take().expect("checked above");
        self.games.insert(
            0,
            Game {
                profile: p,
                shot_size: Some((shot.width, shot.height)),
                cover: None,
            },
        );
        self.setup_win = None;
        self.setup = Editor::new(Vec::new(), 1600.0, 900.0);
        self.push_setup();
        self.notice("");
        w.set_adding_game(false);
        self.open_game(0, Some(&shot));
        true
    }

    /// Back on page 02 of "Add game": drop the setup, show the current game again.
    fn cancel_setup(&mut self) {
        if self.rename.as_ref().is_some_and(|r| r.setup) {
            self.rename = None;
        }
        self.setup_win = None;
        self.setup_shot = None;
        self.setup = Editor::new(Vec::new(), 1600.0, 900.0);
        self.push_setup();
        self.push_profile_settings();
        self.push_main();
    }

    /// Saves the current profile.
    fn save_current(&mut self) {
        let Some(i) = self.current else { return };
        let Some(real) = &self.real else { return };
        if let Err(e) = profile::save(&real.paths, &self.games[i].profile) {
            self.notice(&format!("Not saved: {e}"));
        }
    }

    /// Saves the current profile after a zone change on 10 or 11.
    fn save_main(&mut self) {
        let Some(i) = self.current else { return };
        self.games[i].profile.zones = zone_cfgs(&self.main);
        self.save_current();
        self.push_games();
    }

    /// The mode button on Overview.
    fn set_mode(&mut self, automatic: bool) {
        let Some(i) = self.current else { return };
        let mode = if automatic {
            dr_runtime::Mode::Auto
        } else {
            dr_runtime::Mode::Manual
        };
        if self.games[i].profile.mode != mode {
            self.games[i].profile.mode = mode;
            self.save_current();
        }
    }

    fn retake(&mut self, setup: bool) {
        if self.real.is_none() {
            return;
        }
        if setup {
            let Some(win) = self.setup_win.clone() else {
                return;
            };
            match self.shoot(&win) {
                Ok(shot) => {
                    self.setup.rescale(shot.width as f32, shot.height as f32);
                    if let Some(w) = self.win() {
                        let img = image_of(shot.width, shot.height, &shot.rgba);
                        w.set_setup_shot(img.clone());
                        w.set_setup_shot_w(shot.width as f32);
                        w.set_setup_shot_h(shot.height as f32);
                        w.set_setup_cover(img);
                    }
                    self.setup_shot = Some(shot);
                    self.notice("");
                    self.push_setup();
                }
                Err(e) => self.notice(&format!("No new screenshot: {e}.")),
            }
            return;
        }
        let Some(i) = self.current else { return };
        self.refresh_windows();
        let Some(win) = self
            .windows
            .iter()
            .find(|w| profile_matches(&self.games[i].profile, w))
            .cloned()
        else {
            let name = self.games[i].profile.name.clone();
            self.notice(&format!(
                "{name} is not running. Start it to take a new screenshot."
            ));
            return;
        };
        match self.shoot(&win) {
            Ok(shot) => {
                let Some(real) = &self.real else { return };
                let id = self.games[i].profile.id.clone();
                if let Err(e) = shot.save_png(&real.paths.profile_png(&id)) {
                    self.notice(&format!("The screenshot could not be saved: {e}"));
                    return;
                }
                self.main.rescale(shot.width as f32, shot.height as f32);
                if let Some(w) = self.win() {
                    w.set_shot(image_of(shot.width, shot.height, &shot.rgba));
                    w.set_shot_w(shot.width as f32);
                    w.set_shot_h(shot.height as f32);
                }
                self.games[i].cover = Some(cover_of(&shot));
                self.games[i].shot_size = Some((shot.width, shot.height));
                // the line under the stage on 11 keeps the live view note ("Live", or why the saved
                // screenshot shows)
                self.push_shot_info();
                self.notice("");
                self.save_main();
                self.push_main();
            }
            Err(e) => self.notice(&format!("No new screenshot: {e}.")),
        }
    }

    /// Esc on 03 or 11: cancels a drag on the stage (the zone goes back, a new one is dropped). False when no
    /// drag was going on.
    pub fn cancel_zone_drag(&mut self) -> bool {
        let Some(w) = self.win() else { return false };
        let page = w.get_page();
        let cancelled = self.editor(page).cancel_drag();
        if cancelled {
            self.after(page, false);
            self.live_flush();
        }
        cancelled
    }

    /// Stores the text typed into a zone name; [`Vm::commit_rename`] saves it.
    fn type_rename(&mut self, page: Page, zone: usize, name: &str) {
        let setup = page == Page::DrawZones;
        if self
            .rename
            .as_ref()
            .is_some_and(|r| r.setup != setup || r.zone != zone)
        {
            self.commit_rename();
        }
        self.rename = Some(PendingRename {
            setup,
            zone,
            name: name.to_string(),
        });
    }

    /// Saves a typed zone name as one change (one undo step, one write). Runs on Enter, when the field loses
    /// focus, before any other zone action (so the zone index is still the one the text was typed for), on a
    /// tab change and when the app closes.
    pub fn commit_rename(&mut self) {
        let Some(r) = self.rename.take() else { return };
        let page = if r.setup {
            Page::DrawZones
        } else {
            Page::Zones
        };
        let changed = self.editor(page).rename(r.zone, &r.name);
        self.after(page, changed);
    }

    fn push_setup(&self) {
        sync(
            &self.setup_zones,
            self.setup.zones.iter().map(zone_row).collect(),
        );
        if let Some(w) = self.win() {
            w.set_setup_selected(self.setup.selected.map_or(-1, |i| i as i32));
            if w.get_page() == Page::DrawZones {
                self.push_tools(&w, &self.setup);
            }
        }
    }

    fn push_main(&self) {
        sync(&self.zones, self.main.zones.iter().map(zone_row).collect());
        if let Some(w) = self.win() {
            w.set_zones_selected(self.main.selected.map_or(-1, |i| i as i32));
            if w.get_page() != Page::DrawZones {
                self.push_tools(&w, &self.main);
            }
        }
    }

    fn push_tools(&self, w: &MainWindow, e: &Editor) {
        w.set_zone_kind(ui_kind(e.kind));
        match e.draft() {
            Some(r) => {
                w.set_draft(ZoneData {
                    name: SharedString::default(),
                    kind: ui_kind(e.kind),
                    on: true,
                    x: r.x,
                    y: r.y,
                    w: r.w,
                    h: r.h,
                });
                w.set_drafting(true);
            }
            None => w.set_drafting(false),
        }
    }

    /// The editor of the page on screen: 03 edits the setup zones, the rest the current profile.
    fn editor(&mut self, page: Page) -> &mut Editor {
        if page == Page::DrawZones {
            &mut self.setup
        } else {
            &mut self.main
        }
    }

    /// After an editor change: push it, and save the profile when its zones changed.
    fn after(&mut self, page: Page, zones_changed: bool) {
        if page == Page::DrawZones {
            self.push_setup();
        } else {
            if zones_changed {
                self.save_main();
            }
            self.push_main();
        }
    }
}

fn problems_text(problems: &[Problem]) -> Option<String> {
    let p = problems.first()?;
    let name = p
        .path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let kept = match &p.kept_as {
        Some(k) => format!(
            " It was kept as {}.",
            k.file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default()
        ),
        None => String::new(),
    };
    let more = if problems.len() > 1 {
        format!(" ({} more files have problems.)", problems.len() - 1)
    } else {
        String::new()
    };
    Some(format!(
        "The game profile {name} was not loaded: {}.{kept}{more}",
        p.error
    ))
}

fn wire(w: &MainWindow, vm: &Shared) {
    let v = vm.clone();
    let weak = w.as_weak();
    w.on_zone_pointer(move |kind, x, y, scale| {
        let Some(w) = weak.upgrade() else { return };
        let page = w.get_page();
        let draw = if page == Page::DrawZones {
            w.get_setup_draw()
        } else {
            w.get_zones_draw()
        };
        let mut vm = v.borrow_mut();
        vm.commit_rename();
        let e = vm.editor(page);
        e.draw = draw;
        let c = match kind {
            0 => e.pointer_down(x, y, scale),
            1 => e.pointer_move(x, y, scale),
            2 => e.pointer_up(x, y, scale),
            _ => {
                e.cancel_drag();
                zones::Changed::default()
            }
        };
        vm.after(page, c.zones);
        if kind >= 2 {
            // the drag ended: a live frame of a new size that waited for it shows now
            vm.live_flush();
        }
    });
    let v = vm.clone();
    w.on_zone_cancel(move || v.borrow_mut().cancel_zone_drag());
    let v = vm.clone();
    let weak = w.as_weak();
    w.on_zone_select(move |i| {
        let Some(w) = weak.upgrade() else { return };
        let page = w.get_page();
        let mut vm = v.borrow_mut();
        vm.commit_rename();
        vm.editor(page).select(usize::try_from(i).ok());
        vm.after(page, false);
    });
    let v = vm.clone();
    let weak = w.as_weak();
    w.on_zone_toggle(move |i, on| {
        let Some(w) = weak.upgrade() else { return };
        let page = w.get_page();
        let mut vm = v.borrow_mut();
        vm.commit_rename();
        let changed = usize::try_from(i).is_ok_and(|i| vm.editor(page).set_on(i, on));
        vm.after(page, changed);
    });
    let v = vm.clone();
    let weak = w.as_weak();
    w.on_zone_rename(move |i, name| {
        let Some(w) = weak.upgrade() else { return };
        // the text field keeps its own text while typing; Enter or leaving the field saves it
        if let Ok(i) = usize::try_from(i) {
            v.borrow_mut().type_rename(w.get_page(), i, &name);
        }
    });
    let v = vm.clone();
    w.on_zone_rename_done(move || v.borrow_mut().commit_rename());
    let v = vm.clone();
    let weak = w.as_weak();
    w.on_zone_kind_picked(move |k| {
        let Some(w) = weak.upgrade() else { return };
        let page = w.get_page();
        let mut vm = v.borrow_mut();
        vm.commit_rename();
        let changed = vm.editor(page).set_kind(disk_kind(k));
        vm.after(page, changed);
    });
    let v = vm.clone();
    let weak = w.as_weak();
    w.on_zone_delete(move || {
        let Some(w) = weak.upgrade() else { return };
        let page = w.get_page();
        let mut vm = v.borrow_mut();
        vm.commit_rename();
        let changed = vm.editor(page).delete_selected();
        vm.after(page, changed);
    });
    let v = vm.clone();
    let weak = w.as_weak();
    w.on_zone_undo(move || {
        let Some(w) = weak.upgrade() else { return };
        let page = w.get_page();
        let mut vm = v.borrow_mut();
        vm.commit_rename();
        let changed = vm.editor(page).undo();
        vm.after(page, changed);
    });
    let v = vm.clone();
    let weak = w.as_weak();
    w.on_retake_shot(move || {
        let Some(w) = weak.upgrade() else { return };
        let mut vm = v.borrow_mut();
        vm.commit_rename();
        vm.retake(w.get_page() == Page::DrawZones);
    });
    let v = vm.clone();
    w.on_windows_refresh(move || v.borrow_mut().refresh_windows());
    let v = vm.clone();
    w.on_windows_search(move |q| {
        let mut vm = v.borrow_mut();
        if vm.real.is_some() {
            let before = vm.picked_key();
            vm.search = q.into();
            vm.apply_search(before);
        }
    });
    let v = vm.clone();
    w.on_game_chosen(move |i| {
        let mut vm = v.borrow_mut();
        vm.commit_rename();
        if vm.real.is_some()
            && let Ok(i) = usize::try_from(i)
        {
            vm.open_game(i, None);
        }
    });
    let v = vm.clone();
    w.on_switcher_opened(move || v.borrow_mut().load_covers());
    let v = vm.clone();
    w.on_mode_changed(move |a| v.borrow_mut().set_mode(a));
    let v = vm.clone();
    w.on_setup_cancelled(move || v.borrow_mut().cancel_setup());
    let v = vm.clone();
    let weak = w.as_weak();
    w.on_add_game(move || {
        let Some(w) = weak.upgrade() else { return };
        let mut vm = v.borrow_mut();
        vm.commit_rename();
        vm.setup_win = None;
        vm.setup_shot = None;
        vm.refresh_windows();
        w.set_setup_draw(true);
        w.set_notice("".into());
        if vm.real.is_some() {
            w.set_adding_game(true);
        }
        w.set_page(Page::PickWindow);
    });
    let v = vm.clone();
    w.on_leave(move |page| {
        let mut vm = v.borrow_mut();
        vm.commit_rename();
        match page {
            Page::Welcome => {
                vm.refresh_windows();
                true
            }
            Page::PickWindow => vm.take_setup_shot(),
            Page::Mode => vm.finish_setup(),
            _ => {
                vm.notice("");
                true
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use dr_runtime::windows_list::WinMode;

    fn win(title: &str, exe: &str) -> WinInfo {
        WinInfo {
            hwnd: 1,
            pid: 1,
            title: title.into(),
            exe: exe.into(),
            exe_path: None,
            width: 10,
            height: 10,
            mode: WinMode::Windowed,
            minimized: false,
            game: false,
            icon: None,
        }
    }

    #[test]
    fn an_empty_exe_never_matches_by_exe() {
        let p = Profile::new("g", "Game", "Game.exe");
        assert!(profile_matches(&p, &win("Game", "game.EXE")));
        assert!(!profile_matches(&p, &win("Game", "")));
        // a profile saved for an unreadable process: by title only
        let mut q = Profile::new("h", "Hidden Game", "");
        assert!(
            !profile_matches(&q, &win("Hidden Game", "")),
            "no title set"
        );
        q.matches.title_contains = Some("Hidden Game".into());
        assert!(profile_matches(&q, &win("hidden game v1.2", "")));
        assert!(!profile_matches(&q, &win("Notes", "")));
        assert!(!profile_matches(&q, &win("Notes", "notepad.exe")));
        // with an exe and a title, both must fit
        let mut r = Profile::new("r", "R", "R.exe");
        r.matches.title_contains = Some("Main".into());
        assert!(profile_matches(&r, &win("Main window", "R.exe")));
        assert!(!profile_matches(&r, &win("Launcher", "R.exe")));
    }

    #[test]
    fn cover_is_small() {
        let s = Shot::filled(3840, 2160, [1, 2, 3, 255]);
        let c = cover_of(&s);
        assert_eq!((c.size().width, c.size().height), (COVER_W, 270));
        let s = Shot::filled(100, 50, [1, 2, 3, 255]);
        assert_eq!(cover_of(&s).size().width, 100);
    }
}
