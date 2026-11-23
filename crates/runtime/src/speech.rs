//! Speech: the queue rules (spec 2.5), the speech thread and the audio sinks.
//!
//! - [`Queue`]: pure rules, tested without threads. Lines are spoken in order. A speaker line and the dialogue
//!   line after it in the same read are spoken as one ("Arthur: We need to ..."). A new manual press replaces the
//!   queue. Stop clears it. Lines that waited longer than [`MAX_AGE_MS`] are dropped (they no longer match the
//!   screen).
//! - [`Speech`]: a thread that owns a [`Tts`] engine and an [`AudioOut`]. Commands go in ([`Cmd`]), events come
//!   out ([`Ev`]), and [`Speech::status`] says what is playing. Synthesis runs on this thread, never on the UI
//!   thread.
//! - [`AudioOut`]: where PCM goes. [`RodioOut`] is the speakers (opened at the first sound, never in tests);
//!   [`NullSink`] plays nothing and only logs, for tests. Volume, stop and mute act on the sink; speed goes to
//!   the engine.

use std::collections::VecDeque;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use dr_core::ZoneKind;
use dr_tts::{Pcm, Tts};

/// Lines that waited longer than this in the queue are dropped.
pub const MAX_AGE_MS: f64 = 10_000.0;

/// One line read from the screen.
#[derive(Clone, Debug, PartialEq)]
pub struct Line {
    pub text: String,
    /// Zone name, empty when the line is from no zone.
    pub zone: String,
    pub kind: Option<ZoneKind>,
}

impl Line {
    pub fn new(text: &str, zone: &str, kind: Option<ZoneKind>) -> Line {
        Line {
            text: text.into(),
            zone: zone.into(),
            kind,
        }
    }
}

/// What is spoken in one go: one line, or a speaker and their line.
#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    pub text: String,
    pub lines: Vec<Line>,
    /// When it was queued (ms on the queue's clock).
    pub queued_ms: f64,
    /// Spoken again (Replay): it is in the activity log already.
    pub replay: bool,
}

/// "Arthur" or "Arthur:" plus a line gives "Arthur: line".
fn joined(speaker: &str, line: &str) -> String {
    let name = speaker.trim().trim_end_matches(':').trim_end();
    format!("{name}: {}", line.trim())
}

/// Turns the lines of one read into items: a speaker line followed by a dialogue line become one item.
pub fn items_of(lines: Vec<Line>, now_ms: f64) -> Vec<Item> {
    let mut out = Vec::new();
    let mut it = lines
        .into_iter()
        .filter(|l| !l.text.trim().is_empty())
        .peekable();
    while let Some(l) = it.next() {
        if l.kind == Some(ZoneKind::Speaker)
            && let Some(next) = it.next_if(|n| n.kind == Some(ZoneKind::Dialogue))
        {
            out.push(Item {
                text: joined(&l.text, &next.text),
                lines: vec![l, next],
                queued_ms: now_ms,
                replay: false,
            });
        } else {
            out.push(Item {
                text: l.text.trim().to_string(),
                lines: vec![l],
                queued_ms: now_ms,
                replay: false,
            });
        }
    }
    out
}

/// The queue rules, with time passed in (ms on any clock that only goes forward).
#[derive(Debug, Default)]
pub struct Queue {
    items: VecDeque<Item>,
}

impl Queue {
    /// Adds the lines of one read after the ones waiting.
    pub fn push(&mut self, lines: Vec<Line>, now_ms: f64) {
        self.items.extend(items_of(lines, now_ms));
    }

    /// A new manual press: its lines replace everything waiting. Returns the items that waited.
    pub fn replace(&mut self, lines: Vec<Line>, now_ms: f64) -> Vec<Item> {
        let gone = self.clear();
        self.push(lines, now_ms);
        gone
    }

    /// Puts one item first, marked as a replay (Replay).
    pub fn push_front(&mut self, mut item: Item, now_ms: f64) {
        item.queued_ms = now_ms;
        item.replay = true;
        self.items.push_front(item);
    }

    /// Empties the queue. Returns the items that waited.
    pub fn clear(&mut self) -> Vec<Item> {
        self.items.drain(..).collect()
    }

    /// The next item to speak. Items that waited longer than [`MAX_AGE_MS`] are dropped first and returned in
    /// `dropped`.
    pub fn pop(&mut self, now_ms: f64, dropped: &mut Vec<Item>) -> Option<Item> {
        while let Some(i) = self.items.pop_front() {
            if now_ms - i.queued_ms > MAX_AGE_MS {
                dropped.push(i);
            } else {
                return Some(i);
            }
        }
        None
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

/// Where sound goes.
pub trait AudioOut {
    /// Starts playing `pcm` at `volume` (0 to 1), and stops what played before. An error means that no sound
    /// device could be opened.
    fn play(&mut self, pcm: &Pcm, volume: f32) -> Result<(), String>;
    /// Changes the volume of what plays now and later.
    fn set_volume(&mut self, volume: f32);
    fn stop(&mut self);
    /// Still playing.
    fn busy(&self) -> bool;
    /// Played so far of the current sound.
    fn position_ms(&self) -> f64;
    /// The device reported an error since the last call (unplugged, or taken away by Windows). Each error is
    /// given once.
    fn failed(&mut self) -> Option<String> {
        None
    }
    /// Lets go of the device: the next `play` opens the default device again.
    fn reset(&mut self) {
        self.stop();
    }
}

/// One sound a [`NullSink`] was asked to play.
#[derive(Clone, Debug, PartialEq)]
pub struct Played {
    pub ms: f64,
    pub volume: f32,
}

/// Plays nothing. It logs each sound, and stays "busy" for the sound's length times `scale` (0: done at once), so
/// tests can stop or skip a sound while it plays. [`NullSink::stuck`] and [`NullSink::fail_device`] act like a
/// device that stopped working.
#[derive(Clone)]
pub struct NullSink {
    scale: f64,
    stuck: bool,
    log: Arc<Mutex<Vec<Played>>>,
    volume: Arc<Mutex<f32>>,
    error: Arc<Mutex<Option<String>>>,
    resets: Arc<Mutex<usize>>,
    started: Option<(Instant, f64)>,
}

impl NullSink {
    /// Every sound ends at once.
    pub fn instant() -> NullSink {
        NullSink::timed(0.0)
    }

    /// Sounds last their length times `scale`.
    pub fn timed(scale: f64) -> NullSink {
        NullSink {
            scale,
            stuck: false,
            log: Arc::default(),
            volume: Arc::new(Mutex::new(1.0)),
            error: Arc::default(),
            resets: Arc::default(),
            started: None,
        }
    }

    /// A sound never ends (a device that stopped taking samples, with no error).
    pub fn stuck() -> NullSink {
        NullSink {
            stuck: true,
            ..NullSink::timed(1.0)
        }
    }

    /// The device reports `error` once (shared by clones), as a lost device does.
    pub fn fail_device(&self, error: &str) {
        if let Ok(mut e) = self.error.lock() {
            *e = Some(error.into());
        }
    }

    /// The sounds so far (shared by clones).
    pub fn played(&self) -> Vec<Played> {
        self.log.lock().map(|l| l.clone()).unwrap_or_default()
    }

    /// The volume now (shared by clones).
    pub fn volume(&self) -> f32 {
        self.volume.lock().map(|v| *v).unwrap_or(0.0)
    }

    /// How often the device was let go (shared by clones).
    pub fn resets(&self) -> usize {
        self.resets.lock().map(|r| *r).unwrap_or(0)
    }
}

impl AudioOut for NullSink {
    fn play(&mut self, pcm: &Pcm, volume: f32) -> Result<(), String> {
        let ms = pcm.duration_ms();
        if let Ok(mut l) = self.log.lock() {
            l.push(Played { ms, volume });
        }
        self.set_volume(volume);
        self.started = Some((Instant::now(), ms * self.scale));
        Ok(())
    }
    fn set_volume(&mut self, volume: f32) {
        if let Ok(mut v) = self.volume.lock() {
            *v = volume;
        }
    }
    fn stop(&mut self) {
        self.started = None;
    }
    fn busy(&self) -> bool {
        self.started
            .is_some_and(|(t, ms)| self.stuck || t.elapsed().as_secs_f64() * 1000.0 < ms)
    }
    fn position_ms(&self) -> f64 {
        if self.stuck {
            return 0.0;
        }
        self.started.map_or(0.0, |(t, _)| {
            t.elapsed().as_secs_f64() * 1000.0 / self.scale.max(1e-9)
        })
    }
    fn failed(&mut self) -> Option<String> {
        let e = self.error.lock().ok()?.take();
        if e.is_some() {
            self.started = None;
        }
        e
    }
    fn reset(&mut self) {
        self.stop();
        if let Ok(mut r) = self.resets.lock() {
            *r += 1;
        }
    }
}

/// The speakers, through rodio. The output device opens at the first sound, so making one plays nothing and
/// holds no device. Make it on the speech thread (the output stream cannot move between threads).
///
/// A device that reports an error (unplugged, taken away) is let go, and the next sound opens the default device
/// again. When Windows changes the default device (a headset is plugged in), the next sound moves to it.
#[derive(Default)]
pub struct RodioOut {
    stream: Option<rodio::OutputStream>,
    /// Name of the device the stream plays on.
    device: Option<String>,
    sink: Option<rodio::Sink>,
    /// Set by the stream's error callback (on an audio thread).
    error: Arc<Mutex<Option<String>>>,
}

impl RodioOut {
    pub fn new() -> RodioOut {
        RodioOut::default()
    }

    /// The default output device now, and its name.
    fn default_device() -> Option<(rodio::cpal::Device, String)> {
        use rodio::cpal::traits::{DeviceTrait, HostTrait};
        let d = rodio::cpal::default_host().default_output_device()?;
        let name = d.name().unwrap_or_default();
        Some((d, name))
    }

    fn open(&mut self) -> Result<(), String> {
        let (device, name) =
            RodioOut::default_device().ok_or("no sound output: no device".to_string())?;
        let flag = self.error.clone();
        let mut s = rodio::OutputStreamBuilder::from_device(device)
            .map_err(|e| format!("no sound output: {e}"))?
            .with_error_callback(move |e| {
                if let Ok(mut f) = flag.lock() {
                    *f = Some(e.to_string());
                }
            })
            .open_stream_or_fallback()
            .map_err(|e| format!("no sound output: {e}"))?;
        s.log_on_drop(false);
        if let Ok(mut f) = self.error.lock() {
            *f = None;
        }
        self.stream = Some(s);
        self.device = Some(name);
        Ok(())
    }
}

impl AudioOut for RodioOut {
    fn play(&mut self, pcm: &Pcm, volume: f32) -> Result<(), String> {
        self.stop();
        // a new default device (a headset plugged in): move to it
        if self.stream.is_some() && RodioOut::default_device().map(|(_, n)| n) != self.device {
            self.reset();
        }
        if self.stream.is_none() {
            self.open()?;
        }
        let stream = self.stream.as_ref().expect("opened above");
        let sink = rodio::Sink::connect_new(stream.mixer());
        sink.set_volume(volume);
        let samples: Vec<f32> = pcm.samples.iter().map(|&s| s as f32 / 32768.0).collect();
        sink.append(rodio::buffer::SamplesBuffer::new(
            pcm.channels,
            pcm.rate,
            samples,
        ));
        self.sink = Some(sink);
        Ok(())
    }
    fn set_volume(&mut self, volume: f32) {
        if let Some(s) = &self.sink {
            s.set_volume(volume);
        }
    }
    fn stop(&mut self) {
        if let Some(s) = self.sink.take() {
            s.stop();
        }
    }
    fn busy(&self) -> bool {
        self.sink.as_ref().is_some_and(|s| !s.empty())
    }
    fn position_ms(&self) -> f64 {
        self.sink
            .as_ref()
            .map_or(0.0, |s| s.get_pos().as_secs_f64() * 1000.0)
    }
    fn failed(&mut self) -> Option<String> {
        let e = self.error.lock().ok()?.take();
        if e.is_some() {
            self.reset();
        }
        e
    }
    fn reset(&mut self) {
        self.stop();
        self.stream = None;
        self.device = None;
    }
}

/// A sound counts as stuck when it plays longer than its length times [`STUCK_FACTOR`] plus [`STUCK_EXTRA_MS`]:
/// the device stopped taking samples without an error. The sound then ends with an error, and the device is let
/// go.
pub const STUCK_FACTOR: f64 = 1.5;
pub const STUCK_EXTRA_MS: f64 = 1_500.0;

/// Voice, speed and volume of the speech thread.
#[derive(Clone, Debug, PartialEq)]
pub struct VoiceSettings {
    pub voice: String,
    pub speed: f32,
    pub volume: f32,
}

impl Default for VoiceSettings {
    fn default() -> Self {
        VoiceSettings {
            voice: String::new(),
            speed: 1.0,
            volume: 0.8,
        }
    }
}

/// Commands to the speech thread.
#[derive(Clone, Debug, PartialEq)]
pub enum Cmd {
    /// Lines of one automatic read: queued after the ones waiting.
    Say(Vec<Line>),
    /// Lines of a manual press: they replace the queue (what plays now goes on).
    Replace(Vec<Line>),
    /// The voice test (page 05, Settings): stops everything and speaks `text` with this voice, speed and volume.
    /// All three are for this sample only. `id` comes back in its events and in [`Status::test`].
    Test {
        id: u64,
        text: String,
        voice: String,
        speed: f32,
        volume: f32,
    },
    /// Changes the volume of the voice test that plays now (a moved slider).
    SetTestVolume(f32),
    /// Stops the sound and clears the queue.
    Stop,
    /// Stops the sound; the queue goes on.
    Skip,
    /// Speaks the last item again, first.
    Replay,
    /// Speaks these lines (a row of the activity log) next, as a replay: what plays now and the queue go on.
    ReplayLines(Vec<Line>),
    /// Answered with [`Ev::Mark`] once every command sent before it is done (the app then knows which events
    /// belong to the game before).
    Mark(u64),
    Mute(bool),
    SetVoice(String),
    SetSpeed(f32),
    SetVolume(f32),
}

impl Cmd {
    /// Ends the item that plays now, or the one being made.
    fn cuts(&self) -> bool {
        matches!(self, Cmd::Stop | Cmd::Skip | Cmd::Replay | Cmd::Test { .. })
    }
}

/// What went wrong.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorKind {
    /// The voice engine could not speak the text.
    Engine,
    /// No sound device could be opened.
    NoDevice,
    /// The sound device stopped working while a sound played.
    DeviceLost,
}

/// Events from the speech thread. Each names its item; `test` is the id of a voice test (`None` for a read line).
#[derive(Clone, Debug, PartialEq)]
pub enum Ev {
    /// Started speaking `item`, `ms` long.
    Started {
        item: Item,
        ms: f64,
        test: Option<u64>,
    },
    /// Spoke `item` to the end.
    Finished { item: Item, test: Option<u64> },
    /// `item` was cut off by Stop, Skip, Replay or a test (also before its sound started).
    Cut { item: Item, test: Option<u64> },
    /// `item` waited too long and was dropped.
    Dropped(Item),
    /// `item` waited in the queue and was thrown out by a press, Stop or a voice test. It was never spoken.
    Cleared(Item),
    /// `item` was not spoken, or stopped, because of an error. `error` is the raw text, for the log.
    Error {
        item: Item,
        test: Option<u64>,
        kind: ErrorKind,
        error: String,
    },
    /// The answer to [`Cmd::Mark`].
    Mark(u64),
}

impl Ev {
    /// The text of the event's item ("" for a mark).
    pub fn text(&self) -> &str {
        match self {
            Ev::Started { item, .. }
            | Ev::Finished { item, .. }
            | Ev::Cut { item, .. }
            | Ev::Dropped(item)
            | Ev::Cleared(item)
            | Ev::Error { item, .. } => &item.text,
            Ev::Mark(_) => "",
        }
    }

    /// The voice test the event belongs to.
    pub fn test(&self) -> Option<u64> {
        match self {
            Ev::Started { test, .. }
            | Ev::Finished { test, .. }
            | Ev::Cut { test, .. }
            | Ev::Error { test, .. } => *test,
            Ev::Dropped(_) | Ev::Cleared(_) | Ev::Mark(_) => None,
        }
    }
}

/// What the speech thread does now. Written after every batch of commands and when an item starts being made.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Status {
    /// The text playing now, or being made.
    pub speaking: Option<String>,
    pub pos_ms: f64,
    /// 0 while the sound is being made.
    pub len_ms: f64,
    pub queued: usize,
    pub muted: bool,
    /// The id of the voice test that plays now, or is being made.
    pub test: Option<u64>,
}

enum Msg {
    Cmd(Cmd),
    Quit,
}

/// Handle to the speech thread. Dropping it ends the thread, after the sound being made (if any) is done.
pub struct Speech {
    tx: Sender<Msg>,
    events: Receiver<Ev>,
    status: Arc<Mutex<Status>>,
    thread: Option<JoinHandle<()>>,
}

impl Speech {
    /// Starts the speech thread with an engine and a sink. `make_out` runs on the new thread.
    pub fn start<F>(tts: Box<dyn Tts>, make_out: F, settings: VoiceSettings) -> Speech
    where
        F: FnOnce() -> Box<dyn AudioOut> + Send + 'static,
    {
        let (tx, rx) = mpsc::channel();
        let (etx, events) = mpsc::channel();
        let status = Arc::new(Mutex::new(Status::default()));
        let st = status.clone();
        let thread = std::thread::Builder::new()
            .name("speech".into())
            .spawn(move || {
                // The engine waits on WinRT calls here, and cpal opens the device on this thread. Join the
                // multithreaded COM apartment first, so the thread never becomes a single-threaded apartment with
                // no message loop (cpal accepts the mode it finds).
                let com = com::Mta::enter();
                {
                    let mut w = Worker {
                        tts,
                        out: make_out(),
                        settings,
                        muted: false,
                        queue: Queue::default(),
                        current: None,
                        last: None,
                        events: etx,
                        status: st,
                        clock: Instant::now(),
                        rx,
                        pending: VecDeque::new(),
                    };
                    w.run();
                }
                // the engine and the device are gone before COM is left
                drop(com);
            })
            .expect("the speech thread starts");
        Speech {
            tx,
            events,
            status,
            thread: Some(thread),
        }
    }

    pub fn send(&self, cmd: Cmd) {
        let _ = self.tx.send(Msg::Cmd(cmd));
    }

    /// A handle that sends commands from another thread (the reader thread).
    pub fn sender(&self) -> SpeechTx {
        SpeechTx(self.tx.clone())
    }

    pub fn status(&self) -> Status {
        self.status.lock().map(|s| s.clone()).unwrap_or_default()
    }

    /// Events since the last call.
    pub fn events(&self) -> Vec<Ev> {
        self.events.try_iter().collect()
    }

    /// Waits up to `timeout` for the next event.
    pub fn next_event(&self, timeout: Duration) -> Option<Ev> {
        self.events.recv_timeout(timeout).ok()
    }
}

/// Sends commands to the speech thread from any thread. Sending to a thread that ended does nothing.
#[derive(Clone)]
pub struct SpeechTx(Sender<Msg>);

impl SpeechTx {
    pub fn send(&self, cmd: Cmd) {
        let _ = self.0.send(Msg::Cmd(cmd));
    }
}

impl Drop for Speech {
    fn drop(&mut self) {
        let _ = self.tx.send(Msg::Quit);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

/// COM on the speech thread (and the reader thread, which waits on WinRT OCR calls).
pub(crate) mod com {
    /// The thread is in the multithreaded apartment while this lives (Windows only).
    pub struct Mta {
        #[cfg(windows)]
        entered: bool,
    }

    impl Mta {
        pub fn enter() -> Mta {
            #[cfg(windows)]
            {
                use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx};
                // S_OK or S_FALSE: joined, and must be left. Another mode already set: leave it alone.
                let entered = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.is_ok();
                Mta { entered }
            }
            #[cfg(not(windows))]
            Mta {}
        }
    }

    impl Drop for Mta {
        fn drop(&mut self) {
            #[cfg(windows)]
            if self.entered {
                unsafe { windows::Win32::System::Com::CoUninitialize() };
            }
        }
    }
}

struct Playing {
    item: Item,
    ms: f64,
    test: Option<u64>,
    /// The voice test's own volume.
    test_volume: f32,
    started: Instant,
}

struct Worker {
    tts: Box<dyn Tts>,
    out: Box<dyn AudioOut>,
    settings: VoiceSettings,
    muted: bool,
    queue: Queue,
    current: Option<Playing>,
    last: Option<Item>,
    events: Sender<Ev>,
    status: Arc<Mutex<Status>>,
    clock: Instant,
    rx: Receiver<Msg>,
    /// Messages that came in while an item was being made. They are read before `rx`, in order.
    pending: VecDeque<Msg>,
}

impl Worker {
    fn now(&self) -> f64 {
        self.clock.elapsed().as_secs_f64() * 1000.0
    }

    fn emit(&self, e: Ev) {
        let _ = self.events.send(e);
    }

    fn volume(&self) -> f32 {
        let v = match &self.current {
            Some(p) if p.test.is_some() => p.test_volume,
            _ => self.settings.volume,
        };
        if self.muted { 0.0 } else { v.clamp(0.0, 1.0) }
    }

    /// The next message: a pending one first, else one from the channel. With `wait`, it waits up to 10 ms while
    /// a sound plays, else until one comes. `None`: nothing came.
    fn next(&mut self, wait: bool) -> Option<Msg> {
        if let Some(m) = self.pending.pop_front() {
            return Some(m);
        }
        if !wait {
            return self.rx.try_recv().ok();
        }
        if self.current.is_some() {
            match self.rx.recv_timeout(Duration::from_millis(10)) {
                Ok(m) => Some(m),
                Err(RecvTimeoutError::Timeout) => None,
                Err(RecvTimeoutError::Disconnected) => Some(Msg::Quit),
            }
        } else {
            Some(self.rx.recv().unwrap_or(Msg::Quit))
        }
    }

    fn run(&mut self) {
        loop {
            // wait for one message, then take every one that is waiting before speaking the next item
            let mut wait = true;
            while let Some(m) = self.next(wait) {
                wait = false;
                match m {
                    Msg::Quit => {
                        self.out.stop();
                        return;
                    }
                    Msg::Cmd(c) => self.handle(c),
                }
            }
            self.step();
        }
    }

    fn cut(&mut self) {
        if let Some(p) = self.current.take() {
            self.out.stop();
            if p.test.is_none() {
                self.last = Some(p.item.clone());
            }
            self.emit(Ev::Cut {
                item: p.item,
                test: p.test,
            });
        }
    }

    /// Items thrown out of the queue are said, so none goes missing from the activity log.
    fn cleared(&self, items: Vec<Item>) {
        for item in items {
            self.emit(Ev::Cleared(item));
        }
    }

    fn handle(&mut self, c: Cmd) {
        let now = self.now();
        match c {
            Cmd::Say(lines) => self.queue.push(lines, now),
            Cmd::Replace(lines) => {
                let gone = self.queue.replace(lines, now);
                self.cleared(gone);
            }
            Cmd::Test {
                id,
                text,
                voice,
                speed,
                volume,
            } => {
                self.cut();
                let gone = self.queue.clear();
                self.cleared(gone);
                let item = Item {
                    text,
                    lines: Vec::new(),
                    queued_ms: now,
                    replay: false,
                };
                self.speak(item, &voice, speed, Some((id, volume)));
            }
            Cmd::SetTestVolume(v) => {
                if let Some(p) = self.current.as_mut().filter(|p| p.test.is_some()) {
                    p.test_volume = v;
                    let v = self.volume();
                    self.out.set_volume(v);
                }
            }
            Cmd::Stop => {
                self.cut();
                let gone = self.queue.clear();
                self.cleared(gone);
            }
            Cmd::Skip => self.cut(),
            Cmd::Replay => {
                let again = match &self.current {
                    Some(p) if p.test.is_none() => Some(p.item.clone()),
                    _ => self.last.clone(),
                };
                self.cut();
                if let Some(i) = again {
                    self.queue.push_front(i, now);
                }
            }
            Cmd::ReplayLines(lines) => {
                for i in items_of(lines, now).into_iter().rev() {
                    self.queue.push_front(i, now);
                }
            }
            Cmd::Mark(id) => self.emit(Ev::Mark(id)),
            Cmd::Mute(m) => {
                self.muted = m;
                let v = self.volume();
                self.out.set_volume(v);
            }
            Cmd::SetVoice(v) => self.settings.voice = v,
            Cmd::SetSpeed(s) => self.settings.speed = s,
            Cmd::SetVolume(v) => {
                self.settings.volume = v;
                let v = self.volume();
                self.out.set_volume(v);
            }
        }
    }

    /// Makes the sound of `item` and plays it. `test`: the id and the volume of a voice test.
    fn speak(&mut self, item: Item, voice: &str, speed: f32, test: Option<(u64, f32)>) {
        let speed = speed.clamp(dr_tts::SPEED_MIN, dr_tts::SPEED_MAX);
        let id = test.map(|(id, _)| id);
        self.write(Status {
            speaking: Some(item.text.clone()),
            pos_ms: 0.0,
            len_ms: 0.0,
            test: id,
            ..self.status_now()
        });
        let made = self.tts.synth(&item.text, voice, speed);
        // Commands sent while the sound was made are read first. A Stop, Skip, Replay or another test among them
        // means the sound is not played at all.
        while let Ok(m) = self.rx.try_recv() {
            self.pending.push_back(m);
        }
        let cut = self.pending.iter().any(|m| match m {
            Msg::Quit => true,
            Msg::Cmd(c) => c.cuts(),
        });
        let pcm = match made {
            Ok(pcm) => pcm,
            Err(error) => {
                self.emit(Ev::Error {
                    item,
                    test: id,
                    kind: ErrorKind::Engine,
                    error,
                });
                return;
            }
        };
        if cut {
            if id.is_none() {
                self.last = Some(item.clone());
            }
            self.emit(Ev::Cut { item, test: id });
            return;
        }
        // a device error that came while nothing played (sleep and resume, a new format): let go of the device,
        // so this line opens it again instead of playing into a dead stream
        if self.out.failed().is_some() {
            self.out.reset();
        }
        let ms = pcm.duration_ms();
        self.current = Some(Playing {
            item,
            ms,
            test: id,
            test_volume: test.map_or(0.0, |(_, v)| v),
            started: Instant::now(),
        });
        let volume = self.volume();
        match self.out.play(&pcm, volume) {
            Ok(()) => {
                let item = self.current.as_ref().expect("set above").item.clone();
                self.emit(Ev::Started { item, ms, test: id });
            }
            Err(error) => {
                let p = self.current.take().expect("set above");
                self.emit(Ev::Error {
                    item: p.item,
                    test: id,
                    kind: ErrorKind::NoDevice,
                    error,
                });
            }
        }
    }

    /// Ends the current item when its sound is done, or when the device failed or got stuck.
    fn end_current(&mut self) {
        let Some(p) = self.current.as_ref() else {
            return;
        };
        let stuck_ms = p.ms * STUCK_FACTOR + STUCK_EXTRA_MS;
        let played_ms = p.started.elapsed().as_secs_f64() * 1000.0;
        let error = self.out.failed().or_else(|| {
            (self.out.busy() && played_ms > stuck_ms)
                .then(|| "the sound device stopped playing".to_string())
        });
        if error.is_none() && self.out.busy() {
            return;
        }
        let p = self.current.take().expect("checked");
        if p.test.is_none() {
            self.last = Some(p.item.clone());
        }
        match error {
            Some(error) => {
                // let go of the device: the next sound opens the default device again
                self.out.reset();
                self.emit(Ev::Error {
                    item: p.item,
                    test: p.test,
                    kind: ErrorKind::DeviceLost,
                    error,
                });
            }
            None => self.emit(Ev::Finished {
                item: p.item,
                test: p.test,
            }),
        }
    }

    fn step(&mut self) {
        self.end_current();
        // commands that came in while an item was made go first
        while self.current.is_none() && self.pending.is_empty() {
            let mut dropped = Vec::new();
            let next = self.queue.pop(self.now(), &mut dropped);
            for d in dropped {
                self.emit(Ev::Dropped(d));
            }
            let Some(item) = next else { break };
            let (voice, speed) = (self.settings.voice.clone(), self.settings.speed);
            self.speak(item, &voice, speed, None);
            // a sound that ends at once (null sink) is finished here, so the loop goes on
            self.end_current();
        }
        self.write_status();
    }

    fn status_now(&self) -> Status {
        Status {
            speaking: self.current.as_ref().map(|p| p.item.text.clone()),
            pos_ms: if self.current.is_some() {
                self.out.position_ms()
            } else {
                0.0
            },
            len_ms: self.current.as_ref().map_or(0.0, |p| p.ms),
            queued: self.queue.len(),
            muted: self.muted,
            test: self.current.as_ref().and_then(|p| p.test),
        }
    }

    fn write(&self, s: Status) {
        if let Ok(mut st) = self.status.lock() {
            *st = s;
        }
    }

    fn write_status(&self) {
        self.write(self.status_now());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn l(text: &str, kind: ZoneKind) -> Line {
        Line::new(text, "z", Some(kind))
    }

    #[test]
    fn speaker_and_line_are_joined_in_one_read() {
        let items = items_of(
            vec![
                l("Arthur:", ZoneKind::Speaker),
                l("We need to go.", ZoneKind::Dialogue),
                l("Iron Ore x3", ZoneKind::Notification),
                l("Dutch", ZoneKind::Speaker),
                l("  ", ZoneKind::Dialogue),
                l("Press E", ZoneKind::Prompt),
            ],
            0.0,
        );
        let texts: Vec<_> = items.iter().map(|i| i.text.as_str()).collect();
        // an empty line is skipped, so "Dutch" is followed by a prompt and stays alone
        assert_eq!(
            texts,
            ["Arthur: We need to go.", "Iron Ore x3", "Dutch", "Press E"]
        );
        assert_eq!(items[0].lines.len(), 2);
    }

    #[test]
    fn queue_order_replace_clear_and_age() {
        let mut q = Queue::default();
        let mut dropped = Vec::new();
        q.push(
            vec![l("one", ZoneKind::Dialogue), l("two", ZoneKind::Dialogue)],
            0.0,
        );
        q.push(vec![l("three", ZoneKind::Dialogue)], 5_000.0);
        assert_eq!(q.pop(1_000.0, &mut dropped).unwrap().text, "one");
        // "two" waited 10.5 s: dropped; "three" waited 5.5 s: spoken
        assert_eq!(q.pop(10_500.0, &mut dropped).unwrap().text, "three");
        assert_eq!(
            dropped.iter().map(|d| d.text.as_str()).collect::<Vec<_>>(),
            ["two"]
        );
        q.push(
            vec![l("a", ZoneKind::Dialogue), l("b", ZoneKind::Dialogue)],
            11_000.0,
        );
        q.replace(vec![l("press", ZoneKind::Dialogue)], 11_000.0);
        assert_eq!(q.len(), 1);
        q.clear();
        assert!(q.pop(11_000.0, &mut dropped).is_none());
    }
}
