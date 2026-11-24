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
pub mod overlays;
pub mod prefs;
pub mod reading;
pub mod voices;
pub mod zones;

pub use live::{LiveRun, NOTE_CLOSED, NOTE_FAILED, NOTE_LIVE, NOTE_MINIMIZED};
pub use overlays::attach_overlays;
pub use reading::attach_runtime;

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
use voices::Voices;
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
    /// Opens the Windows speech settings, where voices are added ("Add voices"). Nothing by default.
    fn open_speech_settings(&self) {}
    /// Start with Windows is on (the Run value or the task, src/autostart.rs). `None`: not known here.
    fn autostart(&self) -> Option<bool> {
        None
    }
    /// Turns Start with Windows on or off.
    fn set_autostart(&self, _on: bool) -> Result<(), String> {
        Err("Start with Windows cannot be changed here.".into())
    }
    /// Opens a web page in the browser (Settings > Updates > Check now).
    fn open_url(&self, _url: &str) {}
    /// Asks for a file and hands it to `done` (`None`: cancelled). `save`: the suggested name of a file to save
    /// (Export), or `None` to open one (Import). `owner` is the main window's HWND. The real desktop shows the dialog
    /// on another thread, so the UI thread is never inside a modal loop; `done` may run on that thread.
    fn pick_file_then(
        &self,
        _owner: Option<isize>,
        _save: Option<String>,
        done: Box<dyn FnOnce(Option<std::path::PathBuf>) + Send>,
    ) {
        done(None);
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

    fn open_speech_settings(&self) {
        // the Settings app opens the page of the ms-settings link
        let _ = std::process::Command::new("explorer.exe")
            .arg("ms-settings:speech")
            .spawn();
    }

    fn autostart(&self) -> Option<bool> {
        Some(dr_runtime::autostart::Autostart::app().is_on())
    }

    fn set_autostart(&self, on: bool) -> Result<(), String> {
        let a = dr_runtime::autostart::Autostart::app();
        if on {
            let exe = std::env::current_exe().map_err(|e| e.to_string())?;
            a.enable(&exe)
        } else {
            a.disable()
        }
    }

    fn open_url(&self, url: &str) {
        use windows::Win32::UI::Shell::ShellExecuteW;
        use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
        use windows::core::{HSTRING, w};
        unsafe {
            ShellExecuteW(
                None,
                w!("open"),
                &HSTRING::from(url),
                None,
                None,
                SW_SHOWNORMAL,
            );
        }
    }

    fn pick_file_then(
        &self,
        owner: Option<isize>,
        save: Option<String>,
        done: Box<dyn FnOnce(Option<std::path::PathBuf>) + Send>,
    ) {
        let _ = std::thread::Builder::new()
            .name("file-dialog".into())
            .spawn(move || {
                done(match save {
                    Some(name) => crate::dialogs::save_path(owner, &name),
                    None => crate::dialogs::open_path(owner),
                })
            });
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
        /// The voices (Kokoro, or the Windows voices) and the speech thread for the voice test.
        voices: Box<Voices>,
    },
}

/// The voice list and the voice texts of `v` to the window (page 05 and Settings > Voices).
fn push_voice_list(w: &MainWindow, v: &Voices) {
    w.set_voices(ModelRc::new(VecModel::from(v.rows())));
    w.set_voices_problem(v.problem_text().into());
    w.set_voices_desc(v.desc().into());
    w.set_kokoro_desc(v.kokoro_desc().into());
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

/// The profile can be for this window: same exe (case does not matter), and the title holds `title_contains` when
/// it is set. A profile without an exe (the game's process could not be read) matches by title only, and an
/// unreadable window never matches by its empty exe. Several profiles can match one window (games that share
/// an exe, such as emulators); [`best_profile`] picks the one it belongs to.
pub fn profile_matches(p: &Profile, w: &WinInfo) -> bool {
    match_score(p, w).is_some()
}

/// A window title without the parts that change while a game runs, so a saved title still fits: numbers with a
/// point, comma or colon ("59.94", "1,234", "12:34", "0.0.30"), numbers with a unit ("60 fps", "16 ms",
/// "100%", "60 Hz"), and numbers after a label ("FPS: 60", "VPS 60"). Other numbers stay, so "Persona 4" and
/// "Persona 5" stay apart (but "HD 1.5" and "HD 2.5" do not). Lowercase, with single spaces.
pub fn stable_title(title: &str) -> String {
    const UNITS: [&str; 5] = ["fps", "vps", "ms", "hz", "%"];
    const LABELS: [&str; 2] = ["fps", "vps"];
    let c: Vec<char> = title.to_lowercase().chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < c.len() {
        // a number that starts here (not inside a word such as "doom3")
        let starts = c[i].is_ascii_digit() && (i == 0 || !c[i - 1].is_alphanumeric());
        if !starts {
            out.push(c[i]);
            i += 1;
            continue;
        }
        let mut j = i;
        let mut sep = false;
        while j < c.len() {
            if c[j].is_ascii_digit() {
                j += 1;
            } else if matches!(c[j], '.' | ',' | ':')
                && c.get(j + 1).is_some_and(char::is_ascii_digit)
            {
                sep = true;
                j += 1;
            } else {
                break;
            }
        }
        let rest: String = c[j..].iter().collect();
        let gap = rest.len() - rest.trim_start().len();
        let unit = UNITS.iter().find(|u| {
            let r = rest.trim_start();
            r.starts_with(**u) && !r[u.len()..].starts_with(char::is_alphanumeric)
        });
        let before = out.trim_end().trim_end_matches(':').trim_end();
        let labelled = LABELS.iter().any(|l| before.ends_with(l));
        if sep || unit.is_some() || labelled {
            // the unit is ASCII: as many chars as bytes
            i = j + unit.map_or(0, |u| gap + u.len());
            out.push(' ');
        } else {
            out.extend(&c[i..j]);
            i = j;
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The window title `live` holds the saved title `saved`, with [`stable_title`] applied to both. The number is the
/// length of the saved part that fits (longer is a better fit).
fn title_fits(saved: &str, live: &str) -> Option<usize> {
    let key = stable_title(saved);
    let (key, live) = if key.is_empty() {
        // a title made only of counters: as it is
        (saved.trim().to_lowercase(), live.to_lowercase())
    } else {
        (key, stable_title(live))
    };
    (!key.is_empty() && live.contains(&key)).then(|| key.chars().count())
}

/// How well a profile fits a window: `None` when it does not match; a profile with a title beats one with the
/// exe only, and a longer title beats a shorter one. Titles are compared with [`stable_title`], so a frame
/// counter in the title does not break the match.
pub fn match_score(p: &Profile, w: &WinInfo) -> Option<usize> {
    let title = p
        .matches
        .title_contains
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty());
    if p.matches.exe.is_empty() {
        return title.and_then(|t| title_fits(t, &w.title)).map(|n| 1 + n);
    }
    if w.exe.is_empty() || !w.exe.eq_ignore_ascii_case(&p.matches.exe) {
        return None;
    }
    match title {
        None => Some(0),
        Some(t) => title_fits(t, &w.title).map(|n| 1 + n),
    }
}

/// The profile a window belongs to: the best [`match_score`], the first of `profiles` on a tie.
pub fn best_profile<'a>(
    profiles: impl IntoIterator<Item = &'a Profile>,
    w: &WinInfo,
) -> Option<usize> {
    let mut best: Option<(usize, usize)> = None;
    for (i, p) in profiles.into_iter().enumerate() {
        if let Some(sc) = match_score(p, w)
            && best.is_none_or(|(_, b)| sc > b)
        {
            best = Some((i, sc));
        }
    }
    best.map(|(i, _)| i)
}

/// For each profile: a window it is the best match of runs now. One window makes one profile "Running".
pub fn running(profiles: &[&Profile], windows: &[WinInfo]) -> Vec<bool> {
    let mut on = vec![false; profiles.len()];
    for w in windows {
        if let Some(i) = best_profile(profiles.iter().copied(), w) {
            on[i] = true;
        }
    }
    on
}

/// The profile already set up for this window: the window's best profile, when it matched by its title (the
/// same rule as [`match_score`]), or, for a profile with the exe only, when its name (the window title at setup)
/// is this title, apart from counters ([`stable_title`]). Another window of the same exe with another title gets
/// its own profile.
pub fn same_game<'a>(
    profiles: impl IntoIterator<Item = &'a Profile> + Clone,
    w: &WinInfo,
) -> Option<usize> {
    let i = best_profile(profiles.clone(), w)?;
    let p = profiles.into_iter().nth(i)?;
    let titled = p
        .matches
        .title_contains
        .as_deref()
        .is_some_and(|t| !t.trim().is_empty());
    (titled || stable_title(&p.name) == stable_title(&w.title)).then_some(i)
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

/// The Overview zone chips: one per kind that has a zone, in the order Speaker, Dialogue, Pickups, Prompts
/// (Ignore zones are not listed). A chip is on while any zone of its kind is on.
fn kind_chips(zones: &[EdZone]) -> Vec<ZoneData> {
    use dr_runtime::Kind as K;
    [K::Speaker, K::Dialogue, K::Notification, K::Prompt]
        .into_iter()
        .filter(|&k| zones.iter().any(|z| z.kind == k))
        .map(|k| ZoneData {
            kind: ui_kind(k),
            on: zones.iter().any(|z| z.kind == k && z.on),
            ..ZoneData::default()
        })
        .collect()
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
    /// Page 02 has no Refresh button: while it shows it asks once a second, the quick list (no icons) is read on
    /// another thread, and only a list that changed is read again in full ([`Vm::auto_refresh_windows`]).
    pick_looking: bool,
    pick_found: std::sync::Arc<std::sync::Mutex<Option<Vec<WinInfo>>>>,
    /// Page 03: the window being set up, its screenshot, its zones.
    setup_win: Option<WinInfo>,
    setup_shot: Option<Shot>,
    setup: Editor,
    /// Pages 10 and 11: the current profile's zones.
    main: Editor,
    rename: Option<PendingRename>,
    /// The live view on the stages (live.rs).
    live: live::LiveState,
    /// Reading: the runtime, the activity log, key capture (reading.rs).
    reading: reading::ReadState,
    setup_zones: Rc<VecModel<ZoneData>>,
    zones: Rc<VecModel<ZoneData>>,
    games_running: Rc<VecModel<WindowData>>,
    other_windows: Rc<VecModel<WindowData>>,
    game_rows: Rc<VecModel<GameData>>,
    /// Installed voices and the speech thread (none with the fake source).
    voices: Voices,
    /// Polls the voice test while it plays (real app only; tests call [`Vm::poll_test`]).
    test_timer: slint::Timer,
    /// The id of the last voice test sent, and the one that plays now (or is being made).
    test_id: u64,
    test_running: Option<u64>,
    /// The Settings sliders moved and the profile is not saved yet (it is saved when they are let go).
    voice_dirty: bool,
    /// The in-game overlays (M4): only the app makes them (`attach_overlays`).
    overlays: Option<crate::overlay::Overlays>,
    /// Mute from the quick menu (for this session; not saved).
    muted: bool,
    /// settings.json (M5, prefs.rs): the defaults with the fake source.
    prefs: dr_runtime::settings::Settings,
    /// settings.json must not be written (newer format, or a broken file that could not be moved aside).
    prefs_read_only: bool,
    /// What the game switcher searches for (its rows are the games whose name holds it).
    switcher_query: String,
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
        pick_looking: false,
        pick_found: Default::default(),
        setup_win: None,
        setup_shot: None,
        setup: fake_setup(),
        main: Editor::new(crate::fake::ed_zones(false), 1600.0, 900.0),
        rename: None,
        live: live::LiveState::default(),
        reading: reading::ReadState::default(),
        setup_zones: Rc::new(VecModel::default()),
        zones: Rc::new(VecModel::default()),
        games_running: Rc::new(VecModel::default()),
        other_windows: Rc::new(VecModel::default()),
        game_rows: Rc::new(VecModel::default()),
        voices: Voices::none(),
        test_timer: slint::Timer::default(),
        test_id: 0,
        test_running: None,
        voice_dirty: false,
        overlays: None,
        muted: false,
        prefs: dr_runtime::settings::Settings::default(),
        prefs_read_only: false,
        switcher_query: String::new(),
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
            vm.push_switcher_hits();
        }
        Source::Real {
            paths,
            desktop,
            voices,
        } => {
            // zone edit and the quick menu stay fake until M4
            crate::fake::fill(w);
            vm.voices = *voices;
            vm.voices.check_kokoro();
            push_voice_list(w, &vm.voices);
            w.set_voice_testing(false);
            w.set_test_played(0);
            w.set_test_time("0:00".into());
            w.set_picked_voice(0);
            w.set_voice_speed(0.5);
            w.set_voice_label(vm.voices.label("").into());
            w.set_real_shots(true);
            w.set_extra_zones(ModelRc::default());
            vm.reading_models(w);
            w.set_now_who("".into());
            w.set_now_line("".into());
            // nothing reads until main.rs attaches the runtime
            w.set_status_text(reading::STATUS_STOPPED.into());
            w.set_status_ok(false);
            vm.real = Some(Real { paths, desktop });
            vm.load_prefs();
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

    fn profiles(&self) -> Vec<&Profile> {
        self.games.iter().map(|g| &g.profile).collect()
    }

    /// The switcher's rows: the games whose name holds what was typed (any case), all of them for an empty search.
    fn push_switcher_hits(&self) {
        let Some(w) = self.win() else { return };
        let names: Vec<SharedString> = (0..self.game_rows.row_count())
            .filter_map(|i| self.game_rows.row_data(i).map(|g| g.name))
            .collect();
        let hits = switcher_hits(&names, &self.switcher_query);
        w.set_switcher_hits(ModelRc::new(VecModel::from(hits)));
    }

    fn push_games(&self) {
        let on = running(&self.profiles(), &self.windows);
        let rows = self
            .games
            .iter()
            .zip(on)
            .enumerate()
            .map(|(i, (g, is_running))| {
                // TEXT.md: "Reading" or when it was last played. Profiles do not keep when a game was last
                // played, so only the game being read says something.
                let reading = is_running && self.current == Some(i);
                GameData {
                    name: g.profile.name.as_str().into(),
                    meta: if reading { "Reading" } else { "" }.into(),
                    art: SharedString::default(),
                    running: is_running,
                    cover: g.cover.clone().unwrap_or_default(),
                }
            })
            .collect();
        sync(&self.game_rows, rows);
        self.push_switcher_hits();
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
        self.reading_start();
    }

    /// The current game's mode and keys (Overview and Settings show them; setup pages share them).
    fn push_profile_settings(&self) {
        let (Some(w), Some(g)) = (self.win(), self.current.and_then(|i| self.games.get(i))) else {
            return;
        };
        w.set_automatic(g.profile.mode == dr_runtime::Mode::Auto);
        w.set_read_key(g.profile.keys.read.as_str().into());
        w.set_menu_key(g.profile.keys.quick_menu.as_str().into());
        let v = &g.profile.voice;
        w.set_game_voice(self.voices.index_of(&v.id) as i32);
        w.set_game_speed(voices::slider_of_speed(v.speed));
        w.set_game_volume(v.volume.clamp(0.0, 1.0));
        w.set_voice_label(self.voices.label(&v.id).into());
        self.send_voice();
    }

    /// The current game's voice, speed and volume to the speech thread (what M3b reads with).
    fn send_voice(&self) {
        let (Some(sp), Some(g)) = (
            &self.voices.speech,
            self.current.and_then(|i| self.games.get(i)),
        ) else {
            return;
        };
        let v = &g.profile.voice;
        sp.send(dr_runtime::speech::Cmd::SetVoice(
            self.voices.effective(&v.id),
        ));
        sp.send(dr_runtime::speech::Cmd::SetSpeed(v.speed));
        sp.send(dr_runtime::speech::Cmd::SetVolume(v.volume));
    }

    /// Settings > Voices: a voice picked for the current game.
    fn pick_game_voice(&mut self, i: i32) {
        let Some(g) = self.current else { return };
        let Some(v) = usize::try_from(i)
            .ok()
            .and_then(|i| self.voices.list.get(i))
        else {
            return;
        };
        let voice = &mut self.games[g].profile.voice;
        voice.engine = v.engine.to_string();
        voice.id = v.id.clone();
        self.save_current();
        self.push_profile_settings();
    }

    /// Settings > Voices: the speed or volume slider of the current game moved. The speech thread gets it at
    /// once (also a sample that plays); the profile is saved when the slider is let go ([`Vm::game_voice_saved`]).
    pub fn game_voice_changed(&mut self) {
        let (Some(w), Some(g)) = (self.win(), self.current) else {
            return;
        };
        let speed = voices::speed_of_slider(w.get_game_speed());
        let volume = w.get_game_volume().clamp(0.0, 1.0);
        let voice = &mut self.games[g].profile.voice;
        if (voice.speed, voice.volume) == (speed, volume) {
            return;
        }
        voice.speed = speed;
        voice.volume = volume;
        self.voice_dirty = true;
        self.send_voice();
        if self.test_running.is_some()
            && w.get_test_in_settings()
            && let Some(sp) = &self.voices.speech
        {
            sp.send(dr_runtime::speech::Cmd::SetTestVolume(volume));
        }
    }

    /// Settings > Voices: a slider was let go. Saves the current profile when a slider moved.
    pub fn game_voice_saved(&mut self) {
        if std::mem::take(&mut self.voice_dirty) {
            self.save_current();
        }
    }

    /// Page 05: the Volume slider moved. A sample that plays follows it.
    fn setup_volume_moved(&mut self) {
        let Some(w) = self.win() else { return };
        if self.test_running.is_some()
            && !w.get_test_in_settings()
            && let Some(sp) = &self.voices.speech
        {
            sp.send(dr_runtime::speech::Cmd::SetTestVolume(
                w.get_voice_volume().clamp(0.0, 1.0),
            ));
        }
    }

    /// Any page change: a voice test stops, and slider moves not saved yet are saved.
    fn page_changed(&mut self) {
        if self.test_running.is_some() {
            self.stop_test();
        }
        self.end_capture();
        self.game_voice_saved();
        self.check_kokoro();
    }

    /// When Kokoro's model failed to load (it loads on its own thread), the voice screens switch to the Windows
    /// voices. Called at points that exist anyway: a page change, a voice test.
    pub fn check_kokoro(&mut self) {
        if !self.voices.check_kokoro() {
            return;
        }
        let Some(w) = self.win() else { return };
        push_voice_list(&w, &self.voices);
        if usize::try_from(w.get_picked_voice()).map_or(true, |i| i >= self.voices.list.len()) {
            w.set_picked_voice(0);
        }
        w.set_voice_label(self.voices.label("").into());
        self.push_profile_settings();
    }

    /// Play sample (page 05, `setup`) or Test (Settings, the current game).
    pub fn test_voice(&mut self, setup: bool) -> bool {
        self.check_kokoro();
        let Some(w) = self.win() else { return false };
        if self.voices.list.is_empty() {
            self.notice(&self.voices.problem_text());
            return false;
        }
        if self.voices.speech.is_none() {
            self.notice("Sample data: voices do not play here.");
            return false;
        }
        // events of an earlier sample are old news; line events still go to the log
        let _ = self.drain_speech();
        let (voice, speed, volume) = if setup {
            let i = usize::try_from(w.get_picked_voice()).unwrap_or(0);
            let id = self
                .voices
                .list
                .get(i)
                .map(|v| v.id.clone())
                .unwrap_or_default();
            (
                id,
                voices::speed_of_slider(w.get_voice_speed()),
                w.get_voice_volume(),
            )
        } else {
            let Some(g) = self.current.and_then(|i| self.games.get(i)) else {
                return false;
            };
            let v = &g.profile.voice;
            (self.voices.effective(&v.id), v.speed, v.volume)
        };
        let Some(sp) = &self.voices.speech else {
            return false;
        };
        self.test_id += 1;
        sp.send(dr_runtime::speech::Cmd::Test {
            id: self.test_id,
            text: voices::SAMPLE.into(),
            voice,
            speed,
            volume: volume.clamp(0.0, 1.0),
        });
        self.test_running = Some(self.test_id);
        w.set_voice_testing(true);
        w.set_test_in_settings(!setup);
        w.set_test_played(0);
        w.set_test_time("0:00".into());
        true
    }

    /// Stop on the voice test.
    pub fn stop_test(&mut self) {
        if let Some(sp) = &self.voices.speech {
            sp.send(dr_runtime::speech::Cmd::Stop);
        }
        self.test_running = None;
        self.test_timer.stop();
        if let Some(w) = self.win() {
            w.set_voice_testing(false);
            w.set_test_played(0);
        }
    }

    /// Updates the voice test from the speech thread. Returns true while the sample still plays (or is still
    /// being made). Only the events of the sample sent last count: an earlier sample's Cut does not end it.
    pub fn poll_test(&mut self) -> bool {
        let events = self.drain_speech();
        self.test_events(events)
    }

    /// The voice test's part of the speech events ([`Vm::drain_speech`] takes them all). Returns true while the
    /// sample still plays.
    fn test_events(&mut self, events: Vec<dr_runtime::speech::Ev>) -> bool {
        use dr_runtime::speech::Ev;
        let (Some(w), Some(id)) = (self.win(), self.test_running) else {
            return false;
        };
        let mut ended = false;
        for e in events {
            if e.test() != Some(id) {
                continue;
            }
            match e {
                Ev::Finished { .. } | Ev::Cut { .. } => ended = true,
                Ev::Error { kind, error, .. } => {
                    ended = true;
                    self.voices
                        .write_log(&format!("voice test: {kind:?}: {error}"));
                    w.set_notice(voices::error_text(kind).into());
                }
                _ => {}
            }
        }
        if ended {
            self.test_running = None;
            w.set_voice_testing(false);
            w.set_test_played(0);
            w.set_test_time("0:00".into());
            // a sample that did not play may mean that Kokoro failed to load
            self.check_kokoro();
            return false;
        }
        let Some(sp) = &self.voices.speech else {
            return false;
        };
        let st = sp.status();
        if st.test == Some(id) && st.len_ms > 0.0 {
            w.set_test_played(voices::bars_played(st.pos_ms, st.len_ms));
            w.set_test_time(voices::time_text(st.pos_ms, st.len_ms).into());
        }
        true
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

    /// Page 02, once a second: looks for windows that opened, closed or changed, and lists them again when there
    /// are any. The window picked before stays picked ([`Vm::refresh_windows`]).
    fn auto_refresh_windows(&mut self) {
        if self.real.is_none() {
            return;
        }
        if !self.pick_looking {
            self.pick_looking = true;
            let exes = self.profile_exes();
            let found = self.pick_found.clone();
            if let Some(real) = &self.real {
                real.desktop.windows_quick_then(
                    exes,
                    Box::new(move |list| {
                        if let Ok(mut f) = found.lock() {
                            *f = Some(list);
                        }
                    }),
                );
            }
        }
        // the answer: now (the fake desktop) or at a later tick (the real one reads it on another thread)
        let Some(list) = self.pick_found.lock().ok().and_then(|mut f| f.take()) else {
            return;
        };
        self.pick_looking = false;
        let key = |w: &WinInfo| (w.hwnd, w.title.clone(), w.exe.clone(), w.game, w.minimized);
        let same = list.len() == self.windows.len()
            && list
                .iter()
                .zip(&self.windows)
                .all(|(a, b)| key(a) == key(b));
        if !same {
            self.refresh_windows();
        }
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
            self.notice("Pick a window first. Start your game, then pick it.");
            return false;
        };
        // spec 2.8: several games can share an exe (told apart by the window title), but one window has one profile
        if let Some(i) = same_game(self.profiles(), &win) {
            self.notice(&format!(
                "{} already has a profile. Open it with the game switcher (Ctrl+K) to change its zones.",
                self.games[i].profile.name
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
        let shared_exe = !win.exe.is_empty()
            && self
                .games
                .iter()
                .any(|g| g.profile.matches.exe.eq_ignore_ascii_case(&win.exe));
        if win.exe.is_empty() || shared_exe {
            // the game's process could not be read (it runs as administrator, or anti-cheat hides it), or another
            // game uses the same exe (an emulator, javaw.exe): the window title tells them apart
            p.matches.title_contains = Some(win.title.clone());
        }
        p.mode = if w.get_automatic() {
            dr_runtime::Mode::Auto
        } else {
            dr_runtime::Mode::Manual
        };
        p.keys.read = w.get_read_key().into();
        p.keys.quick_menu = w.get_menu_key().into();
        // the voice picked on page 05 (none when there is no voice at all), its speed and volume
        if let Some(v) = usize::try_from(w.get_picked_voice())
            .ok()
            .and_then(|i| self.voices.list.get(i))
        {
            p.voice.engine = v.engine.to_string();
            p.voice.id = v.id.clone();
        }
        p.voice.speed = voices::speed_of_slider(w.get_voice_speed());
        p.voice.volume = w.get_voice_volume().clamp(0.0, 1.0);
        p.zones = zone_cfgs(&self.setup);
        if let Err(e) = shot
            .save_png(&real.paths.profile_png(&id))
            .and_then(|_| profile::save(&real.paths, &p))
        {
            self.notice(&format!("The game could not be saved: {e}"));
            return false;
        }
        let mut failed = None;
        if shared_exe {
            // The first game of this exe was saved with the exe only. It gets its title too (its name is the
            // window title at its setup), or it would take every other game of this exe.
            for g in &mut self.games {
                let m = &mut g.profile.matches;
                if m.exe.eq_ignore_ascii_case(&win.exe)
                    && m.title_contains
                        .as_deref()
                        .is_none_or(|t| t.trim().is_empty())
                {
                    m.title_contains = Some(g.profile.name.clone());
                    if let Err(e) = profile::save(&real.paths, &g.profile) {
                        failed = Some(e);
                    }
                }
            }
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
        if let Some(e) = failed {
            self.notice(&format!(
                "The other game of this program could not be saved: {e}"
            ));
        }
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
        self.reading_update();
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
            self.reading_update();
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
        let profiles = self.profiles();
        let Some(win) = self
            .windows
            .iter()
            .find(|w| best_profile(profiles.iter().copied(), w) == Some(i))
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
                if self.adopt_shot(i, &shot).is_ok() {
                    self.notice("");
                }
            }
            Err(e) => self.notice(&format!("No new screenshot: {e}.")),
        }
    }

    /// A new screenshot of game `i` (the current one): saved as its picture, its zones keep their place on it,
    /// and pages 10 and 11 show it. Retake and in-game zone edit use it.
    fn adopt_shot(&mut self, i: usize, shot: &Shot) -> Result<(), ()> {
        let Some(real) = &self.real else {
            return Err(());
        };
        let id = self.games[i].profile.id.clone();
        if let Err(e) = shot.save_png(&real.paths.profile_png(&id)) {
            self.notice(&format!("The screenshot could not be saved: {e}"));
            return Err(());
        }
        self.main.rescale(shot.width as f32, shot.height as f32);
        if let Some(w) = self.win() {
            w.set_shot(image_of(shot.width, shot.height, &shot.rgba));
            w.set_shot_w(shot.width as f32);
            w.set_shot_h(shot.height as f32);
        }
        self.games[i].cover = Some(cover_of(shot));
        self.games[i].shot_size = Some((shot.width, shot.height));
        // the line under the stage on 11 keeps the live view note ("Live", or why the saved screenshot shows)
        self.push_shot_info();
        self.save_main();
        self.push_main();
        Ok(())
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
            w.set_zone_kinds(ModelRc::new(VecModel::from(kind_chips(&self.main.zones))));
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
    // Settings (M5, prefs.rs)
    let v = vm.clone();
    w.on_settings_changed(move || v.borrow_mut().settings_changed());
    let v = vm.clone();
    w.on_check_updates(move || v.borrow().check_updates());
    let v = vm.clone();
    w.on_export_game(move || v.borrow().pick_file(true));
    let v = vm.clone();
    w.on_import_game(move || v.borrow().pick_file(false));
    let v = vm.clone();
    w.on_file_picked(move |save, path| v.borrow_mut().file_picked(save, path.as_str()));
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
    w.on_kind_toggle(move |k, on| {
        // the chips are on the Overview: they change the current game's zones
        let mut vm = v.borrow_mut();
        vm.commit_rename();
        let changed = vm.main.set_kind_on(disk_kind(k), on);
        vm.after(Page::Overview, changed);
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
    w.on_windows_refresh(move || {
        // the page's timer can fire while another callback holds the view model: the next tick tries again
        if let Ok(mut vm) = v.try_borrow_mut() {
            vm.auto_refresh_windows();
        }
    });
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
    w.on_switcher_search(move |q| {
        let mut vm = v.borrow_mut();
        vm.switcher_query = q.to_string();
        vm.push_switcher_hits();
    });
    let v = vm.clone();
    w.on_mode_changed(move |a| v.borrow_mut().set_mode(a));
    let v = vm.clone();
    w.on_setup_cancelled(move || v.borrow_mut().cancel_setup());
    let v = vm.clone();
    w.on_voice_test(move |setup| {
        if !v.borrow_mut().test_voice(setup) {
            return;
        }
        // poll the speech thread while the sample plays (the timer runs only with an event loop)
        let weak = Rc::downgrade(&v);
        v.borrow().test_timer.start(
            slint::TimerMode::Repeated,
            std::time::Duration::from_millis(100),
            move || {
                if let Some(v) = weak.upgrade()
                    && let Ok(mut vm) = v.try_borrow_mut()
                    && !vm.poll_test()
                {
                    vm.test_timer.stop();
                }
            },
        );
    });
    let v = vm.clone();
    w.on_voice_stop(move || v.borrow_mut().stop_test());
    let v = vm.clone();
    w.on_voices_add(move || {
        if let Some(real) = &v.borrow().real {
            real.desktop.open_speech_settings();
        }
    });
    let v = vm.clone();
    w.on_game_voice_picked(move |i| v.borrow_mut().pick_game_voice(i));
    let v = vm.clone();
    w.on_game_voice_changed(move || v.borrow_mut().game_voice_changed());
    let v = vm.clone();
    w.on_game_voice_saved(move || v.borrow_mut().game_voice_saved());
    let v = vm.clone();
    w.on_voice_volume_moved(move || v.borrow_mut().setup_volume_moved());
    // reading (reading.rs)
    let v = vm.clone();
    w.on_read_now(move || v.borrow_mut().read_now());
    let v = vm.clone();
    w.on_pause_clicked(move || v.borrow_mut().toggle_pause());
    let v = vm.clone();
    w.on_now_replay(move || v.borrow().speech_cmd(dr_runtime::speech::Cmd::Replay));
    let v = vm.clone();
    w.on_now_stop(move || v.borrow().speech_cmd(dr_runtime::speech::Cmd::Stop));
    let v = vm.clone();
    w.on_recent_replay(move |i| {
        if let Ok(i) = usize::try_from(i) {
            v.borrow().replay_row(i);
        }
    });
    let v = vm.clone();
    w.on_key_change(move |i| v.borrow_mut().key_change(i));
    let v = vm.clone();
    w.on_page_changed(move |_| {
        // a page change can come from inside another callback that holds the view model
        if let Ok(mut vm) = v.try_borrow_mut() {
            vm.page_changed();
        }
    });
    let v = vm.clone();
    let weak = w.as_weak();
    w.on_add_game(move || {
        let Some(w) = weak.upgrade() else { return };
        let mut vm = v.borrow_mut();
        vm.commit_rename();
        vm.setup_win = None;
        vm.setup_shot = None;
        vm.refresh_windows();
        w.set_setup_draw(false);
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

/// The games (indices into `names`) whose name holds `query`, case ignored. An empty search keeps them all.
pub fn switcher_hits(names: &[SharedString], query: &str) -> Vec<i32> {
    let q = query.trim().to_lowercase();
    names
        .iter()
        .enumerate()
        .filter(|(_, n)| q.is_empty() || n.to_lowercase().contains(&q))
        .map(|(i, _)| i as i32)
        .collect()
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
    fn games_that_share_an_exe_are_told_apart_by_the_title() {
        // an emulator: one profile made first (exe only), then two games with their window titles
        let first = Profile::new("a", "Emu - Game A", "emu.exe");
        let mut b = Profile::new("b", "Emu - Game B", "emu.exe");
        b.matches.title_contains = Some("Game B".into());
        let mut b2 = Profile::new("b2", "Emu - Game B2", "emu.exe");
        b2.matches.title_contains = Some("Game B2".into());
        let other = Profile::new("o", "Other", "other.exe");
        let all = [&first, &b, &b2, &other];
        let best = |t: &str, e: &str| best_profile(all.iter().copied(), &win(t, e));
        assert_eq!(best("Emu - Game B", "EMU.exe"), Some(1));
        // the longer title wins ("Game B" is in "Game B2" too)
        assert_eq!(best("Emu - Game B2 (60 fps)", "emu.exe"), Some(2));
        // no title fits: the exe-only profile takes it
        assert_eq!(best("Emu - Game C", "emu.exe"), Some(0));
        assert_eq!(best("Other", "notepad.exe"), None);
        // one window makes one profile "Running"
        assert_eq!(
            running(&all, &[win("Emu - Game B", "emu.exe")]),
            [false, true, false, false]
        );
        assert_eq!(
            running(
                &all,
                &[win("Emu - Game C", "emu.exe"), win("x", "other.exe")]
            ),
            [true, false, false, true]
        );
        // the duplicate check: the window's own profile only
        let same = |t: &str| same_game(all.iter().copied(), &win(t, "emu.exe"));
        assert_eq!(same("Emu - Game B"), Some(1), "by name");
        assert_eq!(same("emu - game a"), Some(0), "by name, the title at setup");
        assert_eq!(same("Emu - Game C"), None, "another game of the same exe");
    }

    #[test]
    fn counters_in_a_title_are_left_out() {
        assert_eq!(
            stable_title("RPCS3 0.0.30 | FPS: 59.94 | Vulkan | Persona 5 [BLUS31604]"),
            "rpcs3 | fps: | vulkan | persona 5 [blus31604]"
        );
        assert_eq!(
            stable_title("Dolphin | JIT64 DC | FPS: 60 - VPS: 60 - 100%"),
            "dolphin | jit64 dc | fps: - vps: -"
        );
        assert_eq!(stable_title("Game B2 (60 fps)  16.7ms"), "game b2 ( )");
        assert_eq!(stable_title("Doom3 v1 - 12:34"), "doom3 v1 -");
        // plain numbers stay: two games of one series stay apart
        assert_eq!(stable_title("Persona 4"), "persona 4");
        assert_ne!(stable_title("Persona 4"), stable_title("Persona 5"));
        assert_eq!(stable_title("Ys 8 - 60 Hz"), "ys 8 -");
        assert_ne!(
            stable_title("Need for Speed 2"),
            stable_title("Need for Speed 3")
        );
        assert_eq!(
            stable_title("Warframe 2 - Speed: 100%"),
            "warframe 2 - speed:"
        );
    }

    #[test]
    fn a_title_with_a_changing_counter_keeps_its_profile() {
        // set up while the emulator showed 59.94 FPS; the counter changes every second
        let mut a = Profile::new("a", "RPCS3 | FPS: 59.94 | Persona 5", "rpcs3.exe");
        a.matches.title_contains = Some("RPCS3 | FPS: 59.94 | Persona 5".into());
        let mut b = Profile::new("b", "RPCS3 | FPS: 60.00 | Persona 4", "rpcs3.exe");
        b.matches.title_contains = Some("RPCS3 | FPS: 60.00 | Persona 4".into());
        let all = [&a, &b];
        let best = |t: &str| best_profile(all.iter().copied(), &win(t, "rpcs3.exe"));
        assert_eq!(best("RPCS3 | FPS: 30.12 | Persona 5"), Some(0));
        assert_eq!(best("RPCS3 | FPS: 57 | Persona 4"), Some(1));
        assert_eq!(best("RPCS3 | FPS: 30.12 | Persona 3"), None);
        // the duplicate check uses the same rule: no second profile after the counter changed
        let same = |t: &str| same_game(all.iter().copied(), &win(t, "rpcs3.exe"));
        assert_eq!(same("RPCS3 | FPS: 12.5 | Persona 5"), Some(0));
        assert_eq!(same("RPCS3 | FPS: 12.5 | Persona 3"), None);
        // an exe-only profile: its name is the title at setup, counters aside
        let only = Profile::new("o", "Emu | 60 fps | Game A", "emu.exe");
        let same_o = |t: &str| same_game([&only], &win(t, "emu.exe"));
        assert_eq!(same_o("Emu | 30 fps | Game A"), Some(0));
        assert_eq!(same_o("Emu | 30 fps | Game B"), None);
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
