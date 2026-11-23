//! The reader thread (spec 2.2): frames from a [`FrameSource`], `dr_core::Pipeline` (automatic) or
//! `dr_core::Manual` (the read key), OCR on this thread, and the lines to the speech thread.
//!
//! - The thread joins the multithreaded COM apartment first (WinRT OCR waits on its calls), then makes its OCR
//!   engine with the factory it was given (tests pass a fake or a counting one).
//! - The newest frame is kept. When nothing new comes (WGC sends frames only when the window changed), the kept
//!   frame is fed again every [`TICK`] with the time now, so a line on a still screen is still confirmed and a
//!   press reads the screen as it is, without waiting for a new frame.
//! - Automatic: every frame goes through `Pipeline::on_frame`; its lines are queued (`Say`). "Read now" (the read
//!   key, or the button) reads every zone once with `Manual::read_now` on the kept frame; its lines replace the
//!   queue. Then automatic reading goes on.
//! - On key press: frames only feed the ring of `Manual` (no OCR). A press reads every zone.
//! - Prompts off (the profile default): prompt zones are read for the activity log only. A press reads the zones
//!   that are spoken first, hands their lines to speech, and only then reads the prompt zones for the log, so a
//!   press never waits for them (two `Manual`s on disjoint zones give the same lines as one).
//! - Pause: no OCR runs. Frames are still taken and the newest kept (WGC sends none for a still window), so a
//!   press while paused reads the screen at once, and Resume reads it without waiting for a change.

use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread::JoinHandle;
use std::time::Duration;

use dr_core::{
    Config, Event, Frame, Manual, ManualConfig, Ocr, Pipeline, SkipReason, Zone, ZoneKind,
};

use crate::capture::{FrameSource, Next, OwnedFrame};
use crate::profile::Mode;
use crate::speech::{self, Line, SpeechTx};

/// The kept frame is fed again this often when nothing new comes.
pub const TICK: Duration = Duration::from_millis(40);

/// Makes the OCR engine on the reader thread.
pub type OcrMaker = Box<dyn FnOnce() -> Result<Box<dyn Ocr>, String> + Send>;

/// What the reader reads with.
#[derive(Clone, Debug, PartialEq)]
pub struct ReadCfg {
    /// The zones switched on.
    pub zones: Vec<Zone>,
    pub mode: Mode,
    pub read_prompts: bool,
}

pub enum ReaderCmd {
    Zones(Vec<Zone>),
    Mode(Mode),
    Prompts(bool),
    Pause(bool),
    /// Read every zone now (the read key, "Read now").
    ReadNow,
    Quit,
}

/// What the reader reports (besides the lines it hands to speech).
#[derive(Clone, Debug, PartialEq)]
pub enum ReaderEv {
    /// Lines read but not spoken, and why.
    Skipped(Vec<(Line, SkipReason)>),
    /// A press was read: `lines` were found (`repeat`: none, so the last lines were said again).
    Pressed { lines: usize, repeat: bool },
    /// OCR errors ("zone name: error").
    Errors(Vec<String>),
    /// No frame follows (the window closed, or the capture failed).
    Ended(String),
    /// The OCR engine could not start (for example no OCR language pack).
    NoOcr(String),
}

/// Where the reader reports. Runs on the reader thread; must not wait.
pub type OnEvent = Box<dyn Fn(ReaderEv) + Send>;

/// The running reader thread. Dropping it ends the thread (after an OCR call in progress).
pub struct Reader {
    tx: Sender<ReaderCmd>,
    thread: Option<JoinHandle<()>>,
}

impl Reader {
    pub fn start(
        cfg: ReadCfg,
        source: Box<dyn FrameSource>,
        ocr: OcrMaker,
        speech: SpeechTx,
        on_event: OnEvent,
    ) -> Reader {
        let (tx, rx) = mpsc::channel();
        let thread = std::thread::Builder::new()
            .name("reader".into())
            .spawn(move || {
                // WinRT OCR waits on its calls: never a single-threaded apartment with no message loop
                let com = speech::com::Mta::enter();
                {
                    let ocr = match ocr() {
                        Ok(o) => o,
                        Err(e) => {
                            // nothing to read with: the capture stops (no frame is copied for nothing), and the
                            // thread waits for the end
                            drop(source);
                            on_event(ReaderEv::NoOcr(e));
                            while !matches!(rx.recv(), Ok(ReaderCmd::Quit) | Err(_)) {}
                            return;
                        }
                    };
                    let mut w = Work::new(cfg, source, ocr, speech, on_event, rx);
                    w.run();
                }
                drop(com);
            })
            .expect("the reader thread starts");
        Reader {
            tx,
            thread: Some(thread),
        }
    }

    pub fn send(&self, cmd: ReaderCmd) {
        let _ = self.tx.send(cmd);
    }

    /// A sender for another thread (the keys thread sends `ReadNow`).
    pub fn sender(&self) -> Sender<ReaderCmd> {
        self.tx.clone()
    }
}

impl Drop for Reader {
    fn drop(&mut self) {
        let _ = self.tx.send(ReaderCmd::Quit);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

/// A dr-core line as a speech line.
pub fn line_of(e: &Event) -> Line {
    Line {
        text: e.text.clone(),
        zone: e.zone.clone().unwrap_or_default(),
        kind: e.kind,
    }
}

/// The two `Manual`s of a press: the zones that are spoken, and the prompt zones read for the log only (prompts
/// off). With prompts on, every zone is in the first.
fn manuals(cfg: &Config, zones: &[Zone]) -> (Manual, Option<Manual>) {
    let mcfg = ManualConfig::default();
    if cfg.read_prompts {
        return (Manual::new(cfg.clone(), mcfg, zones), None);
    }
    let (quiet, spoken): (Vec<Zone>, Vec<Zone>) = zones
        .iter()
        .cloned()
        .partition(|z| z.kind == ZoneKind::Prompt);
    let quiet = (!quiet.is_empty()).then(|| Manual::new(cfg.clone(), mcfg.clone(), &quiet));
    (Manual::new(cfg.clone(), mcfg, &spoken), quiet)
}

/// At least one zone is read aloud: not Ignore, and not a prompt zone with prompts off.
pub fn speaks(cfg: &ReadCfg) -> bool {
    cfg.zones.iter().any(|z| match z.kind {
        ZoneKind::Ignore => false,
        ZoneKind::Prompt => cfg.read_prompts,
        _ => true,
    })
}

fn config(read_prompts: bool) -> Config {
    Config {
        read_prompts,
        ..Config::default()
    }
}

struct Work {
    cfg: ReadCfg,
    source: Box<dyn FrameSource>,
    ocr: Box<dyn Ocr>,
    speech: SpeechTx,
    on_event: OnEvent,
    rx: Receiver<ReaderCmd>,
    pipe: Pipeline,
    spoken: Manual,
    quiet: Option<Manual>,
    last: Option<OwnedFrame>,
    paused: bool,
    press: bool,
}

impl Work {
    fn new(
        cfg: ReadCfg,
        source: Box<dyn FrameSource>,
        ocr: Box<dyn Ocr>,
        speech: SpeechTx,
        on_event: OnEvent,
        rx: Receiver<ReaderCmd>,
    ) -> Work {
        let c = config(cfg.read_prompts);
        let pipe = Pipeline::new(c.clone(), &cfg.zones);
        let (spoken, quiet) = manuals(&c, &cfg.zones);
        Work {
            cfg,
            source,
            ocr,
            speech,
            on_event,
            rx,
            pipe,
            spoken,
            quiet,
            last: None,
            paused: false,
            press: false,
        }
    }

    /// New zones or prompt setting: the engines start again (their state is for the old zones).
    fn rebuild(&mut self) {
        let c = config(self.cfg.read_prompts);
        self.pipe = Pipeline::new(c.clone(), &self.cfg.zones);
        let (spoken, quiet) = manuals(&c, &self.cfg.zones);
        self.spoken = spoken;
        self.quiet = quiet;
    }

    /// Takes the waiting commands. False: quit.
    fn commands(&mut self) -> bool {
        loop {
            let c = match self.rx.try_recv() {
                Ok(c) => c,
                Err(mpsc::TryRecvError::Empty) => return true,
                Err(mpsc::TryRecvError::Disconnected) => return false,
            };
            match c {
                ReaderCmd::Quit => return false,
                ReaderCmd::Zones(z) => {
                    if z != self.cfg.zones {
                        self.cfg.zones = z;
                        self.rebuild();
                    }
                }
                ReaderCmd::Prompts(p) => {
                    if p != self.cfg.read_prompts {
                        self.cfg.read_prompts = p;
                        self.rebuild();
                    }
                }
                ReaderCmd::Mode(m) => {
                    if m != self.cfg.mode {
                        self.cfg.mode = m;
                        // automatic reading starts fresh (it did not see the frames in between)
                        self.pipe = Pipeline::new(config(self.cfg.read_prompts), &self.cfg.zones);
                    }
                }
                // frames are still taken while paused: WGC sends none for a still window, so a press or Resume
                // needs the newest one at hand
                ReaderCmd::Pause(p) => self.paused = p,
                ReaderCmd::ReadNow => self.press = true,
            }
        }
    }

    fn run(&mut self) {
        loop {
            if !self.commands() {
                return;
            }
            match self.source.next(TICK) {
                Next::Frame(f) => {
                    if let Some(old) = self.last.replace(f) {
                        self.source.recycle(old.rgba);
                    }
                }
                Next::Same(t) => {
                    if let Some(f) = &mut self.last {
                        f.t_ms = f.t_ms.max(t);
                    }
                }
                Next::Ended(why) => {
                    (self.on_event)(ReaderEv::Ended(why));
                    // no frame follows: wait for the end
                    loop {
                        match self.rx.recv_timeout(Duration::from_secs(1)) {
                            Ok(ReaderCmd::Quit) | Err(RecvTimeoutError::Disconnected) => return,
                            _ => {}
                        }
                    }
                }
            }
            self.step();
        }
    }

    /// One round on the kept frame.
    fn step(&mut self) {
        let Some(f) = self.last.take() else { return };
        let frame = Frame::new(f.t_ms, f.w, f.h, &f.rgba);
        if std::mem::take(&mut self.press) {
            self.read_press(&frame);
        }
        // no zone to speak from (none switched on, or only prompts with prompts off): automatic reading would watch
        // the whole screen (dr-core's no-zone mode) and read everything on it aloud, so it does not run
        if !self.paused && speaks(&self.cfg) {
            match self.cfg.mode {
                Mode::Auto => {
                    let out = self.pipe.on_frame(&frame, self.ocr.as_mut());
                    if !out.events.is_empty() {
                        self.speech
                            .send(speech::Cmd::Say(out.events.iter().map(line_of).collect()));
                    }
                    self.report(out.skipped, out.errors);
                }
                Mode::Manual => {
                    self.spoken.observe(&frame);
                    if let Some(q) = &mut self.quiet {
                        q.observe(&frame);
                    }
                }
            }
        }
        self.last = Some(f);
    }

    /// A press: the spoken zones first (their lines replace the queue), then the prompt zones for the log.
    fn read_press(&mut self, frame: &Frame) {
        let p = self.spoken.read_now(frame, self.ocr.as_mut());
        if !p.events.is_empty() {
            self.speech
                .send(speech::Cmd::Replace(p.events.iter().map(line_of).collect()));
        }
        (self.on_event)(ReaderEv::Pressed {
            lines: p.events.len(),
            repeat: p.repeat,
        });
        let mut skipped = p.skipped;
        let mut errors = p.errors;
        if let Some(q) = &mut self.quiet {
            let qp = q.read_now(frame, self.ocr.as_mut());
            skipped.extend(qp.skipped);
            errors.extend(qp.errors);
        }
        self.report(skipped, errors);
    }

    fn report(&self, skipped: Vec<dr_core::Skipped>, errors: Vec<String>) {
        if !skipped.is_empty() {
            (self.on_event)(ReaderEv::Skipped(
                skipped
                    .iter()
                    .map(|s| (line_of(&s.line), s.reason))
                    .collect(),
            ));
        }
        if !errors.is_empty() {
            (self.on_event)(ReaderEv::Errors(errors));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dr_core::Rect;

    #[test]
    fn no_spoken_zone_means_no_automatic_reading() {
        let r = Rect::new(0.0, 0.0, 10.0, 10.0);
        let cfg = |zones: Vec<Zone>, read_prompts| ReadCfg {
            zones,
            mode: Mode::Auto,
            read_prompts,
        };
        assert!(!speaks(&cfg(vec![], false)));
        assert!(!speaks(&cfg(
            vec![Zone::new("h", ZoneKind::Ignore, r)],
            false
        )));
        assert!(!speaks(&cfg(
            vec![Zone::new("p", ZoneKind::Prompt, r)],
            false
        )));
        assert!(speaks(&cfg(
            vec![Zone::new("p", ZoneKind::Prompt, r)],
            true
        )));
        assert!(speaks(&cfg(
            vec![Zone::new("d", ZoneKind::Dialogue, r)],
            false
        )));
    }

    /// Reads "Hello there" in every crop, and counts its calls.
    struct Count(std::sync::Arc<std::sync::atomic::AtomicU64>);

    impl Ocr for Count {
        fn recognize(&mut self, w: u32, h: u32, _: &[u8]) -> Result<Vec<dr_core::OcrLine>, String> {
            self.0.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            Ok(vec![dr_core::OcrLine {
                text: "Hello there".into(),
                rect: Rect::new(1.0, 1.0, w as f32 - 2.0, h as f32 - 2.0),
            }])
        }
    }

    #[test]
    fn with_no_zone_switched_on_nothing_is_read() {
        use std::sync::atomic::{AtomicU64, Ordering};
        let sp = speech::Speech::start(
            Box::new(dr_tts::FakeTts::new()),
            || Box::new(speech::NullSink::instant()),
            speech::VoiceSettings::default(),
        );
        let frames: crate::capture::Render =
            Box::new(|_| Some((64, 36, [200u8, 200, 200, 255].repeat(64 * 36))));
        let calls = std::sync::Arc::new(AtomicU64::new(0));
        let c = calls.clone();
        let (tx, rx) = mpsc::channel();
        // the source holds at 2 s until the press is in
        let gate = crate::capture::Gate::new(2_000.0);
        let r = Reader::start(
            ReadCfg {
                zones: Vec::new(),
                mode: Mode::Auto,
                read_prompts: false,
            },
            Box::new(
                crate::capture::ScriptSource::new(frames, 40.0, 4_000.0).with_gate(gate.clone()),
            ),
            Box::new(move || Ok(Box::new(Count(c)) as Box<dyn Ocr>)),
            sp.sender(),
            Box::new(move |e| {
                let _ = tx.send(e);
            }),
        );
        // a press while frames still come: it reads no zone either
        assert!(gate.wait_at(2_000.0, Duration::from_secs(20)));
        r.send(ReaderCmd::ReadNow);
        gate.open_until(f64::MAX);
        // the scripted frames run out: the reader saw every one
        let (mut pressed, mut ended) = (None, false);
        let end = std::time::Instant::now() + Duration::from_secs(20);
        while std::time::Instant::now() < end && !ended {
            match rx.recv_timeout(Duration::from_millis(100)) {
                Ok(ReaderEv::Ended(_)) => ended = true,
                Ok(ReaderEv::Pressed { lines, .. }) => pressed = Some(lines),
                _ => {}
            }
        }
        assert!(ended, "the frames did not run out");
        assert_eq!(pressed, Some(0), "the press was not read");
        assert_eq!(
            calls.load(Ordering::Relaxed),
            0,
            "the whole screen was read"
        );
        drop(r);
        assert!(sp.events().is_empty());
    }

    #[test]
    fn prompts_off_puts_prompt_zones_in_their_own_manual() {
        let r = Rect::new(0.0, 0.0, 10.0, 10.0);
        let zones = vec![
            Zone::new("d", ZoneKind::Dialogue, r),
            Zone::new("p", ZoneKind::Prompt, r),
        ];
        let (_, quiet) = manuals(&config(false), &zones);
        assert!(quiet.is_some());
        let (_, quiet) = manuals(&config(true), &zones);
        assert!(quiet.is_none());
        let (_, quiet) = manuals(&config(false), &zones[..1]);
        assert!(quiet.is_none());
    }
}
