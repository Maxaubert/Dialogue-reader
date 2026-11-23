//! The runtime handle (spec 2.2): commands in ([`Cmd`]), events out ([`Ev`]). It owns the reader thread, the
//! capture of the game window and the global key hooks; the speech thread is passed in (the app's voice test
//! uses the same one).
//!
//! - A profile is active after [`Cmd::Start`]. Reading runs while its game window is known ([`Cmd::Window`]; the
//!   app finds the window with its profile rules and tells the runtime when it comes or goes).
//! - The read key goes from the hook thread straight to the reader (`ReadNow`), without a stop on the UI
//!   thread. The quick menu key and key capture come out as [`Ev::Key`].
//! - Hooks exist only while a profile is active or a key capture is open (spec Risks).
//! - [`Runtime::tick`] (the app calls it on a timer) notices a reader whose window closed, and checks for
//!   exclusive fullscreen.
//! - Tests pass [`Parts`] with a scripted frame source, a fake OCR, no hooks and no window checks.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use dr_core::{Ocr, Zone};

use crate::capture::FrameSource;
use crate::keys::{Action, Bindings, Hooks, Key};
use crate::live::Attacher;
use crate::profile::Mode;
use crate::reader::{ReadCfg, Reader, ReaderCmd, ReaderEv};
use crate::speech::SpeechTx;

/// Makes an OCR engine for a language (runs on the reader thread).
pub type MakeOcr = Arc<dyn Fn(&str) -> Result<Box<dyn Ocr>, String> + Send + Sync>;
/// Starts frames of a window, with the attacher of its capture session (for the live view) when it has one.
pub type MakeSource =
    Arc<dyn Fn(isize) -> Result<(Box<dyn FrameSource>, Option<Attacher>), String> + Send + Sync>;

/// A capture that failed on a window that is still open is tried again after this long.
pub const RETRY: Duration = Duration::from_secs(3);

/// What the runtime is built from.
pub struct Parts {
    pub ocr: MakeOcr,
    pub source: MakeSource,
    /// Install the global key hooks (off in tests).
    pub hooks: bool,
    /// Check the game's process and window: elevated, exclusive fullscreen (off in tests).
    pub checks: bool,
    pub speech: SpeechTx,
}

impl Parts {
    /// The real parts: WinRT OCR, Windows Graphics Capture, key hooks, window checks.
    pub fn real(speech: SpeechTx) -> Parts {
        Parts {
            ocr: Arc::new(|lang: &str| {
                dr_ocr::WinOcr::new(lang).map(|o| Box::new(o) as Box<dyn Ocr>)
            }),
            source: Arc::new(|hwnd| {
                let s = crate::capture::WgcSource::start(hwnd)?;
                let a = s.attacher();
                Ok((Box::new(s) as Box<dyn FrameSource>, Some(a)))
            }),
            hooks: true,
            checks: true,
            speech,
        }
    }
}

/// The reading settings of the active profile.
#[derive(Clone, Debug, PartialEq)]
pub struct RunCfg {
    pub read: ReadCfg,
    /// The read key and the quick menu key, as saved ("Mouse 4", "F7").
    pub read_key: String,
    pub menu_key: String,
    pub ocr_lang: String,
}

impl RunCfg {
    /// From a profile: the zones switched on, its mode, prompts, keys and OCR language.
    pub fn of(p: &crate::Profile) -> RunCfg {
        RunCfg {
            read: ReadCfg {
                zones: p.core_zones(),
                mode: p.mode,
                read_prompts: p.read_prompts,
            },
            read_key: p.keys.read.clone(),
            menu_key: p.keys.quick_menu.clone(),
            ocr_lang: p.ocr_lang.clone(),
        }
    }
}

/// The game window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Target {
    pub hwnd: isize,
    pub pid: u32,
}

pub enum Cmd {
    /// A profile is active: its window is read when it is known.
    Start(Box<RunCfg>),
    /// The active profile's window came (Some) or went (None).
    Window(Option<Target>),
    /// No profile is active: reading stops.
    Stop,
    Pause(bool),
    /// Read every zone now (the button; the read key goes straight to the reader).
    ReadNow,
    SetMode(Mode),
    SetZones(Vec<Zone>),
    SetPrompts(bool),
    SetKeys {
        read: String,
        menu: String,
    },
    /// Key capture on or off: the next key or mouse button comes out as [`Ev::Key`].
    CaptureKey(bool),
}

/// What reading does now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    /// No profile is active.
    Stopped,
    /// The game is not running (or its window could not be captured yet).
    NoWindow,
    Reading,
    Paused,
    /// Reading cannot run: no OCR engine.
    Error,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Ev {
    Status(Status),
    Reader(ReaderEv),
    /// A key action other than the read key: the quick menu key, or key capture.
    Key(Action),
    /// The game runs as administrator and DialogReader does not.
    Elevated,
    /// The game runs in exclusive fullscreen.
    Fullscreen,
    /// The key hooks could not be installed (raw error, for the log).
    HooksFailed(String),
    /// The capture of the window could not start (raw error, for the log).
    CaptureFailed(String),
}

pub struct Runtime {
    parts: Parts,
    tx: Sender<Ev>,
    rx: Receiver<Ev>,
    cfg: Option<RunCfg>,
    target: Option<Target>,
    paused: bool,
    reader: Option<Reader>,
    attacher: Option<Attacher>,
    /// Set by the reader when its frames ended or it has no OCR.
    ended: Arc<AtomicBool>,
    no_ocr: Arc<AtomicBool>,
    /// The reader's command channel, for the hook thread (the read key).
    to_reader: Arc<Mutex<Option<Sender<ReaderCmd>>>>,
    bindings: Arc<Mutex<Bindings>>,
    hooks: Option<Hooks>,
    capturing: bool,
    status: Status,
    /// A window whose capture failed, and when to try it again.
    failed: Option<(isize, Instant)>,
    /// What was said about this window already (elevated, exclusive fullscreen).
    said_elevated: bool,
    said_fullscreen: bool,
    /// The last exclusive fullscreen check (once a second is enough).
    fullscreen_checked: Option<Instant>,
}

impl Runtime {
    pub fn new(parts: Parts) -> Runtime {
        let (tx, rx) = mpsc::channel();
        Runtime {
            parts,
            tx,
            rx,
            cfg: None,
            target: None,
            paused: false,
            reader: None,
            attacher: None,
            ended: Arc::default(),
            no_ocr: Arc::default(),
            to_reader: Arc::default(),
            bindings: Arc::default(),
            hooks: None,
            capturing: false,
            status: Status::Stopped,
            failed: None,
            said_elevated: false,
            said_fullscreen: false,
            fullscreen_checked: None,
        }
    }

    /// Events since the last call.
    pub fn events(&self) -> Vec<Ev> {
        self.rx.try_iter().collect()
    }

    pub fn status(&self) -> Status {
        self.status
    }

    /// The window being read (when reading runs).
    pub fn target(&self) -> Option<Target> {
        self.target.filter(|_| self.reader.is_some())
    }

    pub fn is_paused(&self) -> bool {
        self.paused
    }

    /// The capture session of the window being read, for the live view of the same window.
    pub fn attacher(&self) -> Option<&Attacher> {
        self.attacher.as_ref().filter(|a| a.is_running())
    }

    /// The hooks are installed now.
    pub fn hooked(&self) -> bool {
        self.hooks.is_some()
    }

    pub fn send(&mut self, cmd: Cmd) {
        match cmd {
            Cmd::Start(cfg) => {
                let cfg = *cfg;
                let restart = self.cfg.as_ref().is_none_or(|c| c.ocr_lang != cfg.ocr_lang);
                self.set_keys(&cfg.read_key, &cfg.menu_key);
                if restart {
                    self.stop_reader();
                    self.cfg = Some(cfg);
                } else {
                    let read = cfg.read.clone();
                    self.cfg = Some(cfg);
                    self.to_reader(ReaderCmd::Zones(read.zones));
                    self.to_reader(ReaderCmd::Mode(read.mode));
                    self.to_reader(ReaderCmd::Prompts(read.read_prompts));
                }
                self.sync();
            }
            Cmd::Window(t) => {
                if t != self.target {
                    self.stop_reader();
                    self.target = t;
                    self.said_elevated = false;
                    self.said_fullscreen = false;
                }
                self.sync();
            }
            Cmd::Stop => {
                self.stop_reader();
                self.cfg = None;
                self.target = None;
                // no profile: no read key or quick menu key (the hooks may stay for a key capture)
                {
                    let mut b = self.lock_bindings();
                    b.read = None;
                    b.menu = None;
                }
                self.sync();
            }
            Cmd::Pause(p) => {
                self.paused = p;
                self.to_reader(ReaderCmd::Pause(p));
                self.sync();
            }
            Cmd::ReadNow => self.to_reader(ReaderCmd::ReadNow),
            Cmd::SetMode(m) => {
                if let Some(c) = &mut self.cfg {
                    c.read.mode = m;
                }
                self.to_reader(ReaderCmd::Mode(m));
            }
            Cmd::SetZones(z) => {
                if let Some(c) = &mut self.cfg {
                    c.read.zones = z.clone();
                }
                self.to_reader(ReaderCmd::Zones(z));
            }
            Cmd::SetPrompts(p) => {
                if let Some(c) = &mut self.cfg {
                    c.read.read_prompts = p;
                }
                self.to_reader(ReaderCmd::Prompts(p));
            }
            Cmd::SetKeys { read, menu } => {
                if let Some(c) = &mut self.cfg {
                    c.read_key = read.clone();
                    c.menu_key = menu.clone();
                }
                self.set_keys(&read, &menu);
            }
            Cmd::CaptureKey(on) => {
                self.capturing = on;
                self.lock_bindings().capture = on;
                self.sync();
            }
        }
    }

    fn lock_bindings(&self) -> std::sync::MutexGuard<'_, Bindings> {
        self.bindings.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn set_keys(&self, read: &str, menu: &str) {
        let mut b = self.lock_bindings();
        b.read = Key::parse(read);
        b.menu = Key::parse(menu);
    }

    fn to_reader(&self, c: ReaderCmd) {
        if let Some(r) = &self.reader {
            r.send(c);
        }
    }

    fn stop_reader(&mut self) {
        *self.to_reader.lock().unwrap_or_else(|e| e.into_inner()) = None;
        self.attacher = None;
        // joins the thread: at most one OCR call
        self.reader = None;
        self.ended.store(false, Ordering::Release);
    }

    /// Starts or stops the reader and the hooks for the state now, and says the status when it changed.
    fn sync(&mut self) {
        self.sync_hooks();
        if let (Some(cfg), Some(t), None) = (&self.cfg, self.target, &self.reader) {
            let blocked = self
                .failed
                .is_some_and(|(h, at)| h == t.hwnd && Instant::now() < at);
            if !blocked {
                self.start_reader(cfg.clone(), t);
            }
        }
        let st = self.compute_status();
        if st != self.status {
            self.status = st;
            let _ = self.tx.send(Ev::Status(st));
        }
    }

    fn compute_status(&self) -> Status {
        if self.cfg.is_none() {
            Status::Stopped
        } else if self.no_ocr.load(Ordering::Acquire) {
            Status::Error
        } else if self.reader.is_none() {
            Status::NoWindow
        } else if self.paused {
            Status::Paused
        } else {
            Status::Reading
        }
    }

    fn start_reader(&mut self, cfg: RunCfg, t: Target) {
        // a process that just exited cannot be opened either: only an open window counts
        if self.parts.checks
            && !self.said_elevated
            && crate::windows_list::is_open(t.hwnd)
            && crate::elevation::needs_admin(t.pid)
        {
            self.said_elevated = true;
            let _ = self.tx.send(Ev::Elevated);
        }
        let (source, attacher) = match (self.parts.source)(t.hwnd) {
            Ok(s) => s,
            Err(e) => {
                self.failed = Some((t.hwnd, Instant::now() + RETRY));
                let _ = self.tx.send(Ev::CaptureFailed(e));
                return;
            }
        };
        self.failed = None;
        self.ended.store(false, Ordering::Release);
        self.no_ocr.store(false, Ordering::Release);
        let tx = self.tx.clone();
        let (ended, no_ocr) = (self.ended.clone(), self.no_ocr.clone());
        let make = self.parts.ocr.clone();
        let lang = cfg.ocr_lang.clone();
        let reader = Reader::start(
            cfg.read,
            source,
            Box::new(move || make(&lang)),
            self.parts.speech.clone(),
            Box::new(move |e| {
                match &e {
                    ReaderEv::Ended(_) => ended.store(true, Ordering::Release),
                    ReaderEv::NoOcr(_) => no_ocr.store(true, Ordering::Release),
                    _ => {}
                }
                let _ = tx.send(Ev::Reader(e));
            }),
        );
        if self.paused {
            reader.send(ReaderCmd::Pause(true));
        }
        *self.to_reader.lock().unwrap_or_else(|e| e.into_inner()) = Some(reader.sender());
        self.reader = Some(reader);
        self.attacher = attacher;
    }

    fn sync_hooks(&mut self) {
        let want = self.parts.hooks && (self.cfg.is_some() || self.capturing);
        if want == self.hooks.is_some() {
            return;
        }
        if !want {
            self.hooks = None;
            return;
        }
        let tx = self.tx.clone();
        let to_reader = self.to_reader.clone();
        let on_action: crate::keys::OnAction = Arc::new(move |a| match a {
            // straight to the reader: no stop on the UI thread
            Action::Read => {
                if let Some(r) = to_reader.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
                    let _ = r.send(ReaderCmd::ReadNow);
                }
            }
            Action::None => {}
            other => {
                let _ = tx.send(Ev::Key(other));
            }
        });
        match Hooks::install(self.bindings.clone(), on_action) {
            Ok(h) => self.hooks = Some(h),
            Err(e) => {
                // not tried again until the hooks are wanted anew
                self.parts.hooks = false;
                let _ = self.tx.send(Ev::HooksFailed(e));
            }
        }
    }

    /// Runs on the app's timer: a reader whose window closed is let go (the app looks for the window again);
    /// a capture that failed is tried again; exclusive fullscreen is said once per window.
    pub fn tick(&mut self) {
        if self.reader.is_some() && self.ended.load(Ordering::Acquire) {
            let hwnd = self.target.map(|t| t.hwnd);
            self.stop_reader();
            // a window that is still open gave no picture: wait before the next try
            if let Some(h) = hwnd {
                self.failed = Some((h, Instant::now() + RETRY));
            }
            if hwnd.is_some_and(|h| !crate::windows_list::is_open(h)) {
                self.target = None;
            }
        }
        if self.parts.checks
            && !self.said_fullscreen
            && let Some(t) = self.target
            && self
                .fullscreen_checked
                .is_none_or(|at| at.elapsed() >= Duration::from_secs(1))
            && {
                self.fullscreen_checked = Some(Instant::now());
                crate::elevation::exclusive_fullscreen(t.hwnd)
            }
        {
            self.said_fullscreen = true;
            let _ = self.tx.send(Ev::Fullscreen);
        }
        self.sync();
    }
}

impl Drop for Runtime {
    fn drop(&mut self) {
        self.stop_reader();
        self.hooks = None;
    }
}
