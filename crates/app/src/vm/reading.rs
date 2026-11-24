//! Reading in the app (M3b): the runtime handle, the activity log, the status, the line being spoken, and key
//! capture.
//!
//! - `main.rs` builds the runtime ([`attach_runtime`]); tests can pass one with fake parts, or none. Without a
//!   runtime nothing is read, but the log of the current game still shows.
//! - A UI timer ([`TICK`]) runs [`Vm::reading_tick`]: the runtime's tick and events, the speech events, and the
//!   search for the current game's window while it is not being read (the window list is read on another thread,
//!   as for the live view). The speech events have one reader: [`Vm::drain_speech`] hands the voice test its
//!   events and turns the others into log lines.
//! - The activity log (`log\<id>.jsonl`): a line is written when it starts to be spoken, or when it is skipped
//!   (prompts off, already read, waited too long, a voice error, stopped or thrown out of the queue before it was
//!   spoken). A replay is not a new line: it shows in the "now speaking" card only. Overview shows the newest
//!   five, "Show all" the newest [`KEEP`].
//! - A game change stops the reader and the speech of the game before; what it read or queued is logged under it
//!   (a speech mark tells when its last events came).
//! - Key capture (a click on a keycap, on the keys page and in Settings): the runtime installs the hooks while it is
//!   open; the next key or mouse button (not left or right click) becomes the key. Esc, a second click on the
//!   keycap or the app window losing the focus ends it. Alt+Tab, Alt+Esc, Ctrl+Esc and Win combos are never taken
//!   (`dr_runtime::keys`).

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use dr_runtime::activity::{self, Entry, Log, Skip};
use dr_runtime::keys::{Action, Btn};
use dr_runtime::reader::ReaderEv;
use dr_runtime::runtime::{Cmd, Ev, RunCfg, Runtime, Status, Target};
use dr_runtime::speech::{self, Item};
use dr_runtime::windows_list::WinInfo;
use slint::{ModelRc, VecModel};

use super::{Page, Vm, best_profile};
use crate::{Kind, LineData, MainWindow};

/// How often the UI pumps the runtime and the speech events.
pub const TICK: Duration = Duration::from_millis(100);
/// How often the current game's window is looked for while it is not being read.
const SEARCH: Duration = Duration::from_secs(1);
/// Lines kept for "Show all".
pub const KEEP: usize = 200;
/// Rows of Recent on Overview.
pub const RECENT: usize = 5;

pub const STATUS_READING: &str = "Reading";
pub const STATUS_PAUSED: &str = "Paused";
pub const STATUS_NO_WINDOW: &str = "Game not running";
pub const STATUS_STOPPED: &str = "Stopped";
pub const STATUS_ERROR: &str = "Cannot read";

pub const NOTE_ELEVATED: &str =
    "This game runs as administrator. DialogReader needs to run as administrator too.";
pub const NOTE_FULLSCREEN: &str = "This game runs in exclusive fullscreen. Switch it to borderless or windowed in the game's settings.";
pub const NOTE_NO_OCR: &str = "Windows has no English text recognition. Add the English language in Windows Settings > Time & language > Language.";
pub const NOTE_NO_HOOKS: &str = "The keys could not be set up, so the read key does nothing. Restart DialogReader to try again.";
pub const NOTE_CLICK: &str =
    "Left and right click are for your game. Press another key or mouse button.";

/// The answer of a window search: its number and the window list.
type Found = (u64, Vec<WinInfo>);

pub struct ReadState {
    pub(super) rt: Option<Runtime>,
    timer: slint::Timer,
    log: Option<Log>,
    /// The profile id of the game whose log is open.
    game: Option<String>,
    /// Logs of games left while their speech still had events to come: line events go to the front one until its
    /// speech mark comes back.
    older: VecDeque<(u64, Log)>,
    marks: u64,
    /// The newest lines of the current game, oldest first, at most [`KEEP`].
    entries: Vec<Entry>,
    /// The item that started to play last (a later Cut of it was spoken already).
    started: Option<Item>,
    /// Key capture: 0 the read key, 1 the quick menu key.
    capture: Option<i32>,
    lookup: u64,
    looking: bool,
    found: Arc<Mutex<Option<Found>>>,
    last_search: Option<Instant>,
    /// Raw runtime errors go here (`reading.log` in the logs folder; only the app binary sets it).
    err_log: Option<PathBuf>,
    /// The status and Pause state on screen.
    status: Option<(Status, bool)>,
    recent: std::rc::Rc<VecModel<LineData>>,
    all: std::rc::Rc<VecModel<LineData>>,
}

impl Default for ReadState {
    fn default() -> Self {
        ReadState {
            rt: None,
            timer: slint::Timer::default(),
            log: None,
            game: None,
            older: VecDeque::new(),
            marks: 0,
            entries: Vec::new(),
            started: None,
            capture: None,
            lookup: 0,
            looking: false,
            found: Arc::new(Mutex::new(None)),
            last_search: None,
            err_log: None,
            status: None,
            recent: std::rc::Rc::new(VecModel::default()),
            all: std::rc::Rc::new(VecModel::default()),
        }
    }
}

/// The zone kind of a kind's name on disk.
fn zone_kind(name: &str) -> Option<dr_core::ZoneKind> {
    use dr_core::ZoneKind as Z;
    [
        Z::Dialogue,
        Z::Speaker,
        Z::Notification,
        Z::Prompt,
        Z::Ignore,
    ]
    .into_iter()
    .find(|k| k.name() == name)
}

/// The speech lines of a log line, to replay it: "Arthur:" and their line are a speaker line and a dialogue line,
/// as when they were read.
pub fn lines_of(e: &Entry) -> Vec<speech::Line> {
    let mut lines = Vec::new();
    if !e.who.is_empty() {
        lines.push(speech::Line::new(
            &e.who,
            "",
            Some(dr_core::ZoneKind::Speaker),
        ));
    }
    lines.push(speech::Line::new(&e.text, &e.zone, zone_kind(&e.kind)));
    lines
}

/// The UI kind of a kind's name on disk.
fn kind_of(name: &str) -> Kind {
    match name {
        "speaker" => Kind::Speaker,
        "notification" => Kind::Pickups,
        "prompt" => Kind::Prompts,
        "ignore" => Kind::Ignore,
        _ => Kind::Dialogue,
    }
}

/// Speaker names longer than this are cut with "..." in the list rows, so the grey name always fits on the
/// first line of a two-line row (ui/components/base.slint, ListRow).
pub const SPEAKER_MAX: usize = 20;

/// The speaker part of a row as a list row shows it ("Arthur:"): a name of at most [`SPEAKER_MAX`] characters,
/// then "...", and the colon it came with.
pub fn short_speaker(who: &str) -> String {
    let who = who.trim();
    let name = who.trim_end_matches(':').trim_end();
    if name.chars().count() <= SPEAKER_MAX {
        return who.to_string();
    }
    let cut: String = name.chars().take(SPEAKER_MAX).collect();
    let colon = if who.ends_with(':') { ":" } else { "" };
    format!("{}...{colon}", cut.trim_end())
}

/// A log line as a Recent or activity row.
pub fn row(e: &Entry) -> LineData {
    LineData {
        time: activity::local_time(e.t).into(),
        kind: kind_of(&e.kind),
        who: short_speaker(&e.who).into(),
        line: e.text.as_str().into(),
        skip: e.skip.map_or("", Skip::text).into(),
    }
}

/// "Arthur" or "Arthur:" as the speaker part of a row ("Arthur:").
fn who_of(speaker: &str) -> String {
    let s = speaker.trim().trim_end_matches(':').trim_end();
    if s.is_empty() {
        String::new()
    } else {
        format!("{s}:")
    }
}

/// The log line of a speech item: a speaker and their line give "Arthur:" and the line.
pub fn entry_of(item: &Item, skip: Option<Skip>) -> Entry {
    let first = item.lines.first();
    let kind = first
        .and_then(|l| l.kind)
        .map_or(String::new(), |k| k.name().to_string());
    let zone = first.map_or(String::new(), |l| l.zone.clone());
    match item.lines.as_slice() {
        [s, d] if s.kind == Some(dr_core::ZoneKind::Speaker) => Entry {
            t: activity::now_ms(),
            zone: d.zone.clone(),
            kind: d.kind.map_or("dialogue".into(), |k| k.name().to_string()),
            who: who_of(&s.text),
            text: d.text.trim().to_string(),
            skip,
        },
        _ => Entry {
            t: activity::now_ms(),
            zone,
            kind,
            who: String::new(),
            text: item.text.trim().to_string(),
            skip,
        },
    }
}

/// The line being spoken: (speaker part, line).
fn now_of(item: &Item) -> (String, String) {
    let e = entry_of(item, None);
    (e.who, e.text)
}

/// Builds the runtime into the view model and starts its timer. Only `main.rs` passes a real one.
pub fn attach_runtime(vm: &super::Shared, rt: Runtime, err_log: Option<PathBuf>) {
    {
        let mut v = vm.borrow_mut();
        v.reading.rt = Some(rt);
        v.reading.err_log = err_log;
        v.reading_start();
    }
    let weak = std::rc::Rc::downgrade(vm);
    vm.borrow()
        .reading
        .timer
        .start(slint::TimerMode::Repeated, TICK, move || {
            if let Some(v) = weak.upgrade()
                && let Ok(mut vm) = v.try_borrow_mut()
            {
                vm.reading_tick();
            }
        });
}

impl Vm {
    /// The runtime (tests).
    pub fn runtime(&self) -> Option<&Runtime> {
        self.reading.rt.as_ref()
    }

    pub fn runtime_mut(&mut self) -> Option<&mut Runtime> {
        self.reading.rt.as_mut()
    }

    /// The log lines of the current game, oldest first (tests).
    pub fn log_entries(&self) -> &[Entry] {
        &self.reading.entries
    }

    /// Puts the activity models on the window (once, at install).
    pub(super) fn reading_models(&self, w: &MainWindow) {
        w.set_recent(ModelRc::from(self.reading.recent.clone()));
        w.set_log(ModelRc::from(self.reading.all.clone()));
    }

    pub(super) fn err(&self, text: &str) {
        if let Some(p) = &self.reading.err_log {
            super::voices::append_log(p, text);
        }
    }

    /// The current game changed (or the runtime came): its log shows, and the runtime reads it.
    pub(super) fn reading_start(&mut self) {
        if self.real.is_none() {
            return;
        }
        let id = self
            .current
            .and_then(|i| self.games.get(i))
            .map(|g| g.profile.id.clone());
        if id != self.reading.game {
            self.leave_game();
        }
        let Some(real) = &self.real else { return };
        let game = self.current.and_then(|i| self.games.get(i));
        let log = game.map(|g| Log::new(real.paths.activity_log(&g.profile.id)));
        self.reading.entries = log.as_ref().map(|l| l.last(KEEP)).unwrap_or_default();
        self.reading.log = log;
        self.reading.game = id;
        // `started` stays: the Cut of a line that played when the game changed comes later, and must not be
        // logged as "Stopped" (a stale item never matches a new one: its queue time differs)
        self.push_log();
        if let Some(w) = self.win() {
            w.set_now_who("".into());
            w.set_now_line("".into());
        }
        let cfg = game.map(|g| RunCfg::of(&g.profile));
        // a window search still running is for the game before
        self.reading.looking = false;
        self.reading.lookup += 1;
        self.reading.last_search = None;
        if let Some(rt) = &mut self.reading.rt {
            match cfg {
                Some(c) => {
                    rt.send(Cmd::Window(None));
                    rt.send(Cmd::Start(Box::new(c)));
                }
                None => rt.send(Cmd::Stop),
            }
        }
        self.reading_tick();
    }

    /// The game changes: the reader of the game before stops, and what it read or queued is logged under it, not
    /// under the new game.
    fn leave_game(&mut self) {
        // the reader is joined: its last skipped lines are in the channel now
        if let Some(rt) = &mut self.reading.rt {
            rt.send(Cmd::Window(None));
        }
        let events = self
            .reading
            .rt
            .as_ref()
            .map(|r| r.events())
            .unwrap_or_default();
        for e in events {
            self.runtime_event(e);
        }
        let tests = self.drain_speech();
        if !tests.is_empty() {
            self.test_events(tests);
        }
        // its lines are not spoken in the new game; the events of that Stop come later and go to its log
        if let (Some(sp), Some(log)) = (&self.voices.speech, self.reading.log.take()) {
            self.reading.marks += 1;
            let m = self.reading.marks;
            sp.send(speech::Cmd::Stop);
            sp.send(speech::Cmd::Mark(m));
            self.reading.older.push_back((m, log));
        }
    }

    /// The zones, mode, prompts or keys of the current game changed: the runtime reads with the new ones.
    pub(super) fn reading_update(&mut self) {
        let Some(g) = self.current.and_then(|i| self.games.get(i)) else {
            return;
        };
        let c = RunCfg::of(&g.profile);
        if let Some(rt) = &mut self.reading.rt {
            rt.send(Cmd::SetZones(c.read.zones));
            rt.send(Cmd::SetMode(c.read.mode));
            rt.send(Cmd::SetPrompts(c.read.read_prompts));
            rt.send(Cmd::SetKeys {
                read: c.read_key,
                menu: c.menu_key,
            });
        }
    }

    /// The timer: runtime events and tick, speech events, the window search, the status.
    pub fn reading_tick(&mut self) {
        if let Some(rt) = &mut self.reading.rt {
            rt.tick();
        }
        let events = self
            .reading
            .rt
            .as_ref()
            .map(|r| r.events())
            .unwrap_or_default();
        for e in events {
            self.runtime_event(e);
        }
        // the voice test has its own timer; line events are logged here
        let tests = self.drain_speech();
        if !tests.is_empty() {
            self.test_events(tests);
        }
        self.search_window();
        self.push_status();
        self.overlay_sync();
    }

    /// Takes every speech event: line events go to the log and the "now speaking" card; the voice test's events
    /// are returned. A replay is in the log already: it only shows in the card.
    pub(super) fn drain_speech(&mut self) -> Vec<speech::Ev> {
        let Some(sp) = &self.voices.speech else {
            return Vec::new();
        };
        let mut tests = Vec::new();
        let mut new = Vec::new();
        for e in sp.events() {
            if e.test().is_some() {
                tests.push(e);
                continue;
            }
            // events of a game left before go to its log, and not to the card
            let old = !self.reading.older.is_empty();
            let line = match e {
                speech::Ev::Started { item, .. } => {
                    if !old && let Some(w) = self.win() {
                        let (who, line) = now_of(&item);
                        w.set_now_who(who.into());
                        w.set_now_line(line.into());
                    }
                    let e = (!item.replay).then(|| entry_of(&item, None));
                    self.reading.started = Some(item);
                    e
                }
                // cut before it was heard: say so in the log
                speech::Ev::Cut { item, .. } => (!item.replay
                    && self.reading.started.as_ref() != Some(&item))
                .then(|| entry_of(&item, Some(Skip::Stopped))),
                speech::Ev::Cleared(item) => {
                    (!item.replay).then(|| entry_of(&item, Some(Skip::Stopped)))
                }
                speech::Ev::Dropped(item) => {
                    (!item.replay).then(|| entry_of(&item, Some(Skip::TooOld)))
                }
                speech::Ev::Error {
                    item, kind, error, ..
                } => {
                    self.voices
                        .write_log(&format!("reading: {kind:?}: {error}"));
                    (!item.replay && self.reading.started.as_ref() != Some(&item))
                        .then(|| entry_of(&item, Some(Skip::VoiceError)))
                }
                speech::Ev::Finished { .. } => None,
                speech::Ev::Mark(m) => {
                    while self.reading.older.front().is_some_and(|(id, _)| *id <= m) {
                        self.reading.older.pop_front();
                    }
                    None
                }
            };
            let Some(line) = line else { continue };
            match self.reading.older.front() {
                Some((_, log)) => {
                    if let Err(e) = log.append(&[line]) {
                        self.err(&format!("activity log: {e}"));
                    }
                }
                None => new.push(line),
            }
        }
        self.add_entries(new);
        tests
    }

    /// The speech thread's status (tests).
    pub fn speech_status(&self) -> Option<speech::Status> {
        self.voices.speech.as_ref().map(speech::Speech::status)
    }

    fn add_entries(&mut self, new: Vec<Entry>) {
        if new.is_empty() {
            return;
        }
        if let Some(log) = &self.reading.log
            && let Err(e) = log.append(&new)
        {
            self.err(&format!("activity log: {e}"));
        }
        self.reading.entries.extend(new);
        let n = self.reading.entries.len();
        if n > KEEP {
            self.reading.entries.drain(..n - KEEP);
        }
        self.push_log();
    }

    /// Recent (newest five) and the activity list (newest first).
    fn push_log(&self) {
        let rows: Vec<LineData> = self.reading.entries.iter().rev().map(row).collect();
        super::sync(
            &self.reading.recent,
            rows.iter().take(RECENT).cloned().collect(),
        );
        super::sync(&self.reading.all, rows);
    }

    pub fn runtime_event(&mut self, e: Ev) {
        match e {
            Ev::Status(_) => {}
            Ev::Reader(ReaderEv::Skipped(v)) => {
                let new = v
                    .into_iter()
                    .map(|(l, why)| Entry {
                        t: activity::now_ms(),
                        zone: l.zone,
                        kind: l.kind.map_or(String::new(), |k| k.name().to_string()),
                        who: String::new(),
                        text: l.text.trim().to_string(),
                        skip: Some(match why {
                            dr_core::SkipReason::PromptsOff => Skip::PromptsOff,
                            dr_core::SkipReason::AlreadyRead => Skip::AlreadyRead,
                        }),
                    })
                    .collect();
                self.add_entries(new);
            }
            Ev::Reader(ReaderEv::Errors(v)) => self.err(&format!("OCR: {}", v.join("; "))),
            Ev::Reader(ReaderEv::Ended(why)) => self.err(&format!("capture ended: {why}")),
            Ev::Reader(ReaderEv::NoOcr(why)) => {
                self.err(&format!("no OCR: {why}"));
                self.notice(NOTE_NO_OCR);
            }
            Ev::Reader(ReaderEv::Pressed { .. }) => {}
            Ev::Key(a) => self.key_event(a),
            Ev::Elevated => self.notice(NOTE_ELEVATED),
            Ev::Fullscreen => self.notice(NOTE_FULLSCREEN),
            Ev::HooksFailed(why) => {
                self.err(&format!("key hooks: {why}"));
                self.notice(NOTE_NO_HOOKS);
            }
            Ev::CaptureFailed(why) => self.err(&format!("capture: {why}")),
        }
    }

    /// Looks for the current game's window while it is not being read (once a second, on another thread).
    fn search_window(&mut self) {
        let Some(i) = self.current else { return };
        let Some(rt) = &self.reading.rt else { return };
        if rt.target().is_some() {
            return;
        }
        let Some(real) = &self.real else { return };
        // an answer that came: the window of the current game, if it runs
        let id = self.reading.lookup;
        let list = self
            .reading
            .found
            .lock()
            .ok()
            .and_then(|mut f| f.take_if(|(n, _)| *n == id))
            .map(|(_, l)| l);
        if let Some(list) = list {
            self.reading.looking = false;
            let profiles = self.profiles();
            let win = list
                .iter()
                .find(|w| best_profile(profiles.iter().copied(), w) == Some(i))
                .map(|w| Target {
                    hwnd: w.hwnd,
                    pid: w.pid,
                });
            if let (Some(t), Some(rt)) = (win, &mut self.reading.rt) {
                rt.send(Cmd::Window(Some(t)));
            }
            return;
        }
        if self.reading.looking
            || self
                .reading
                .last_search
                .is_some_and(|t| t.elapsed() < SEARCH)
        {
            return;
        }
        self.reading.looking = true;
        self.reading.last_search = Some(Instant::now());
        self.reading.lookup += 1;
        let (id, found) = (self.reading.lookup, self.reading.found.clone());
        real.desktop.windows_quick_then(
            self.profile_exes(),
            Box::new(move |list| {
                if let Ok(mut f) = found.lock() {
                    *f = Some((id, list));
                }
            }),
        );
    }

    /// The status chip, the control bar and the Pause button.
    pub(super) fn push_status(&mut self) {
        let Some(w) = self.win() else { return };
        let Some(rt) = &self.reading.rt else { return };
        let st = rt.status();
        if self.reading.status == Some((st, rt.is_paused())) {
            return;
        }
        self.reading.status = Some((st, rt.is_paused()));
        let text = match st {
            Status::Reading => STATUS_READING,
            Status::Paused => STATUS_PAUSED,
            Status::NoWindow => STATUS_NO_WINDOW,
            Status::Stopped => STATUS_STOPPED,
            Status::Error => STATUS_ERROR,
        };
        w.set_status_text(text.into());
        w.set_status_ok(st == Status::Reading);
        w.set_paused(rt.is_paused());
    }

    /// "Read now" on Overview.
    pub fn read_now(&mut self) {
        let name = self
            .current
            .and_then(|i| self.games.get(i))
            .map(|g| g.profile.name.clone());
        let Some(rt) = &mut self.reading.rt else {
            return;
        };
        if rt.target().is_none() {
            if let Some(n) = name {
                self.notice(&format!(
                    "{n} is not running. Start it, then press Read now."
                ));
            }
            return;
        }
        rt.send(Cmd::ReadNow);
    }

    /// Pause or Resume on Overview.
    pub fn toggle_pause(&mut self) {
        let Some(rt) = &mut self.reading.rt else {
            // no runtime (--fake): the button still shows both states
            if self.real.is_none()
                && let Some(w) = self.win()
            {
                let p = !w.get_paused();
                w.set_paused(p);
                w.set_status_text(if p { STATUS_PAUSED } else { STATUS_READING }.into());
                w.set_status_ok(!p);
            }
            return;
        };
        let p = !rt.is_paused();
        rt.send(Cmd::Pause(p));
        if p && let Some(sp) = &self.voices.speech {
            sp.send(speech::Cmd::Stop);
        }
        self.push_status();
    }

    /// Replay and Stop in the "now speaking" card.
    pub fn speech_cmd(&self, c: speech::Cmd) {
        if let Some(sp) = &self.voices.speech {
            sp.send(c);
        }
    }

    /// The replay button of a Recent row (newest first): spoken next, as a replay (no new log line). What plays
    /// now and the lines that wait go on.
    pub fn replay_row(&self, i: usize) {
        let Some(e) = self.reading.entries.iter().rev().nth(i) else {
            return;
        };
        self.speech_cmd(speech::Cmd::ReplayLines(lines_of(e)));
    }

    /// A click on a keycap: starts key capture for it (0 read key, 1 quick menu key), or ends it (a second click).
    pub fn key_change(&mut self, which: i32) {
        if self.reading.capture == Some(which) {
            self.end_capture();
            return;
        }
        self.reading.capture = Some(which);
        if let Some(w) = self.win() {
            w.set_capturing_key(which);
        }
        if let Some(rt) = &mut self.reading.rt {
            rt.send(Cmd::CaptureKey(true));
        }
    }

    pub fn end_capture(&mut self) {
        if self.reading.capture.take().is_none() {
            return;
        }
        if let Some(w) = self.win() {
            w.set_capturing_key(-1);
            // the refusal of a click (the click on the keycap, for one) is not news any more
            if w.get_notice().as_str() == NOTE_CLICK {
                w.set_notice("".into());
            }
        }
        if let Some(rt) = &mut self.reading.rt {
            rt.send(Cmd::CaptureKey(false));
        }
    }

    /// A key action from the hooks: the quick menu key and the keys of an open overlay (M4), or key capture.
    pub fn key_event(&mut self, a: Action) {
        match a {
            // the hook changed the open overlay already: the windows follow
            Action::Menu => {
                self.overlay_sync();
                return;
            }
            Action::Nav(n) => {
                // a key that came with the quick menu key in the same batch finds its window open
                self.overlay_sync();
                self.overlay_nav(n);
                return;
            }
            _ => {}
        }
        let Some(which) = self.reading.capture else {
            return;
        };
        let Some(w) = self.win() else { return };
        match a {
            Action::Captured(k) => {
                let text = k.text();
                let other = if which == 0 {
                    w.get_menu_key()
                } else {
                    w.get_read_key()
                };
                if other.as_str() == text {
                    let msg = if which == 0 {
                        "That key is already the menu key. Press another one."
                    } else {
                        "That key is already the read key. Press another one."
                    };
                    self.notice(msg);
                    // the hook ended the capture with that key: take the next one
                    if let Some(rt) = &mut self.reading.rt {
                        rt.send(Cmd::CaptureKey(true));
                    }
                    return;
                }
                if which == 0 {
                    w.set_read_key(text.as_str().into());
                } else {
                    w.set_menu_key(text.as_str().into());
                }
                self.notice("");
                self.end_capture();
                // Settings changes the current game; the keys page fills the game being set up (saved at Finish)
                if w.get_page() == Page::Settings
                    && let Some(i) = self.current
                {
                    let keys = &mut self.games[i].profile.keys;
                    keys.read = w.get_read_key().into();
                    keys.quick_menu = w.get_menu_key().into();
                    self.save_current();
                    self.reading_update();
                }
            }
            Action::Refused(Btn::Left | Btn::Right) => self.notice(NOTE_CLICK),
            Action::Cancelled => self.cancel_capture(),
            _ => {}
        }
    }

    /// Ends key capture as Esc or a second click on the keycap does: the key stays as it was. Also when the app window loses the
    /// focus (`crate::focus_lost`): Alt+Tab, Win or a click in the game must not leave capture open.
    pub fn cancel_capture(&mut self) {
        if self.reading.capture.is_some() {
            self.notice("");
            self.end_capture();
        }
    }

    /// The reader's capture session when it captures `hwnd` (the live view attaches to it).
    pub(super) fn reading_attacher(&self, hwnd: isize) -> Option<dr_runtime::live::Attacher> {
        self.reading
            .rt
            .as_ref()?
            .attacher()
            .filter(|a| a.hwnd() == hwnd)
            .cloned()
    }

    /// The current game's window being read (tests).
    pub fn reading_target(&self) -> Option<Target> {
        self.reading.rt.as_ref().and_then(Runtime::target)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dr_core::ZoneKind;
    use dr_runtime::speech::Line;

    #[test]
    fn long_speaker_names_are_cut() {
        assert_eq!(short_speaker("Arthur:"), "Arthur:");
        assert_eq!(short_speaker(""), "");
        let max = format!("{}:", "x".repeat(SPEAKER_MAX));
        assert_eq!(short_speaker(&max), max);
        assert_eq!(
            short_speaker("Third Officer of the Royal Guard:"),
            "Third Officer of the...:"
        );
        assert_eq!(
            short_speaker("Third Officer of the Royal Guard"),
            "Third Officer of the..."
        );
    }

    fn item(lines: Vec<Line>) -> Item {
        let text = lines
            .iter()
            .map(|l| l.text.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        Item {
            text,
            lines,
            queued_ms: 0.0,
            replay: false,
        }
    }

    #[test]
    fn a_replayed_row_has_its_speaker_and_kind() {
        let e = entry_of(
            &item(vec![
                Line::new("ARTHUR", "Speaker", Some(ZoneKind::Speaker)),
                Line::new("We need to go.", "Subtitles", Some(ZoneKind::Dialogue)),
            ]),
            None,
        );
        let again = dr_runtime::speech::items_of(lines_of(&e), 0.0);
        assert_eq!(again.len(), 1, "{again:?}");
        let back = entry_of(&again[0], None);
        assert_eq!(
            (back.who.as_str(), back.text.as_str(), back.kind.as_str()),
            ("ARTHUR:", "We need to go.", "dialogue")
        );
        assert_eq!(again[0].text, "ARTHUR: We need to go.");
        let p = Entry {
            kind: "prompt".into(),
            ..entry_of(&item(vec![Line::new("Hold E", "P", None)]), None)
        };
        assert_eq!(lines_of(&p)[0].kind, Some(ZoneKind::Prompt));
        assert_eq!(zone_kind(""), None);
    }

    #[test]
    fn a_speaker_and_their_line_are_one_row() {
        let i = item(vec![
            Line::new("ARTHUR", "Speaker", Some(ZoneKind::Speaker)),
            Line::new(" We need to go. ", "Subtitles", Some(ZoneKind::Dialogue)),
        ]);
        let e = entry_of(&i, None);
        assert_eq!(
            (
                e.who.as_str(),
                e.text.as_str(),
                e.kind.as_str(),
                e.zone.as_str()
            ),
            ("ARTHUR:", "We need to go.", "dialogue", "Subtitles")
        );
        let r = row(&Entry {
            skip: Some(Skip::PromptsOff),
            ..entry_of(
                &item(vec![Line::new("Hold E", "P", Some(ZoneKind::Prompt))]),
                None,
            )
        });
        assert_eq!(r.kind, Kind::Prompts);
        assert_eq!(r.skip.as_str(), "Prompts off");
        assert_eq!(r.who.as_str(), "");
        assert_eq!(who_of("Dutch:"), "Dutch:");
        assert_eq!(who_of("  "), "");
    }
}
