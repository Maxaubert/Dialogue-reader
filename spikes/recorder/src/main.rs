//! Spike 2 recorder: records a game window with Windows Graphics Capture so we can measure
//! how capture behaves per game and how long pop-up text (toasts) stays on screen.
//!
//! Output per session (in `<out>/<name>-<date>-<time>/`):
//! - `video_NNN.mp4` H.264 of every captured frame (a new file when the frame size changes)
//! - `frames.csv`    one row per captured frame: time, gap since last frame, brightness of sampled frames
//! - `marks.csv`     one row per mark key press
//! - `marks/mark_NNN/*.png`  lossless frames from `pre` seconds before to `post` seconds after each mark
//! - `session.json`, `stats.json`

use std::collections::VecDeque;
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender, SyncSender, TrySendError};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context as _, Result, anyhow, bail};
use clap::Parser;
use windows::core::BOOL;
use windows::Win32::System::Console::{CTRL_CLOSE_EVENT, CTRL_LOGOFF_EVENT, CTRL_SHUTDOWN_EVENT, SetConsoleCtrlHandler};
use windows::Win32::System::Performance::{QueryPerformanceCounter, QueryPerformanceFrequency};
use windows::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState;
use windows_capture::capture::{Context, GraphicsCaptureApiHandler};
use windows_capture::encoder::{
    AudioSettingsBuilder, ContainerSettingsBuilder, VideoEncoder, VideoSettingsBuilder, VideoSettingsSubType,
};
use windows_capture::frame::Frame;
use windows_capture::graphics_capture_api::InternalCaptureControl;
use windows_capture::settings::{
    ColorFormat, CursorCaptureSettings, DirtyRegionSettings, DrawBorderSettings, MinimumUpdateIntervalSettings,
    SecondaryWindowSettings, Settings,
};
use windows_capture::window::Window;

#[derive(Parser, Clone, Debug)]
#[command(about = "Record a game window to measure capture and pop-up text timing")]
struct Args {
    /// List windows that can be captured, then exit
    #[arg(long)]
    list: bool,
    /// Capture the window of this process, e.g. RDR2.exe
    #[arg(long)]
    process: Option<String>,
    /// Capture the first window whose title contains this text
    #[arg(long)]
    title: Option<String>,
    /// Short name for the session folder, e.g. rdr2
    #[arg(long, default_value = "game")]
    name: String,
    /// Output folder
    #[arg(long, default_value = "recordings")]
    out: PathBuf,
    /// Lossless frames per second kept in memory for marks
    #[arg(long, default_value_t = 10)]
    fps: u32,
    /// Seconds saved before a mark
    #[arg(long, default_value_t = 8)]
    pre: u32,
    /// Seconds saved after a mark
    #[arg(long, default_value_t = 3)]
    post: u32,
    /// Most memory (MB) the lossless frame buffer may use; oldest frames go first
    #[arg(long, default_value_t = 2000)]
    ring_mb: u64,
    /// Do not write video
    #[arg(long)]
    no_video: bool,
    /// Video bitrate in Mbit/s
    #[arg(long, default_value_t = 40)]
    mbps: u32,
    /// Seconds to switch to the game when neither --process nor --title is given
    #[arg(long, default_value_t = 5)]
    countdown: u32,
    /// Key that marks a pop-up: scrolllock, pause, insert, home, f8..f12
    #[arg(long, default_value = "scrolllock")]
    mark_key: String,
    /// Key that stops recording
    #[arg(long, default_value = "pause")]
    stop_key: String,
    /// Stop by itself after this many seconds (0 = no limit)
    #[arg(long, default_value_t = 0)]
    seconds: u64,
    /// Convert the .qoi mark frames of a session folder to PNG, then exit
    #[arg(long)]
    convert: Option<PathBuf>,
    /// Testing only: place one mark this many seconds after start
    #[arg(long, hide = true)]
    auto_mark: Option<u64>,
}

/// 100 ns ticks on the same clock as `Frame::timestamp` (SystemRelativeTime is QPC based).
fn now_ticks() -> i64 {
    let mut c = 0i64;
    let mut f = 0i64;
    unsafe {
        let _ = QueryPerformanceCounter(&mut c);
        let _ = QueryPerformanceFrequency(&mut f);
    }
    (c as i128 * 10_000_000 / f as i128) as i64
}

fn vk(name: &str) -> Result<i32> {
    Ok(match name.to_ascii_lowercase().as_str() {
        "scrolllock" => 0x91,
        "pause" => 0x13,
        "insert" => 0x2D,
        "home" => 0x24,
        "f8" => 0x77,
        "f9" => 0x78,
        "f10" => 0x79,
        "f11" => 0x7A,
        "f12" => 0x7B,
        other => bail!("unknown key '{other}'"),
    })
}

fn key_down(vk: i32) -> bool {
    (unsafe { GetAsyncKeyState(vk) } as u16 & 0x8000) != 0
}

// Console events (Ctrl+C, closing the console window, log off, shut down) only set STOP.
// For close, log off and shut down Windows kills the process once the handler returns, so the
// handler waits until main has saved everything (DONE) or 4.5 s pass.
static STOP: AtomicBool = AtomicBool::new(false);
static DONE: AtomicBool = AtomicBool::new(false);
static CLOSING: AtomicBool = AtomicBool::new(false);

unsafe extern "system" fn console_handler(event: u32) -> BOOL {
    STOP.store(true, Ordering::SeqCst);
    if event == CTRL_CLOSE_EVENT || event == CTRL_LOGOFF_EVENT || event == CTRL_SHUTDOWN_EVENT {
        CLOSING.store(true, Ordering::SeqCst);
        let t = Instant::now();
        while !DONE.load(Ordering::SeqCst) && t.elapsed() < Duration::from_millis(4500) {
            thread::sleep(Duration::from_millis(20));
        }
    }
    BOOL(1)
}

#[derive(Default)]
struct Stats {
    frames: AtomicU64,
    sampled: AtomicU64,
    sample_errors: AtomicU64,
    black: AtomicU64,
    max_gap_us: AtomicU64,
    first_ts: AtomicI64,
    last_ts: AtomicI64,
    width: AtomicU64,
    height: AtomicU64,
    size_changes: AtomicU64,
    dropped: AtomicU64,
    ring_pruned_for_memory: AtomicU64,
    video_files: AtomicU64,
    clock_offset_ms: AtomicI64,
}

struct Entry {
    ts: i64,
    qoi: Vec<u8>,
}

struct FrameMsg {
    ts: i64,
    w: u32,
    h: u32,
    rgba: Vec<u8>,
}

enum Ctl {
    Dump { id: u32, mark_ts: i64, dir: PathBuf },
    Quit,
}

struct Flags {
    video_dir: Option<PathBuf>,
    mbps: u32,
    sample_step: i64,
    tx: SyncSender<FrameMsg>,
    stats: Arc<Stats>,
    csv: PathBuf,
}

struct Rec {
    encoder: Option<VideoEncoder>,
    enc_size: (u32, u32),
    video_dir: Option<PathBuf>,
    mbps: u32,
    tx: SyncSender<FrameMsg>,
    csv: BufWriter<File>,
    last_flush: Instant,
    last_ts: Option<i64>,
    next_sample: i64,
    sample_step: i64,
    stats: Arc<Stats>,
    buf: Vec<u8>,
}

impl Rec {
    /// Starts a new video file when the frame size changes; the encoder cannot scale.
    fn video_frame(&mut self, frame: &Frame, w: u32, h: u32, n: u64) {
        let Some(dir) = self.video_dir.clone() else { return };
        let size = (w & !1, h & !1);
        if self.encoder.is_some() && self.enc_size == size {
            if let Err(e) = self.encoder.as_mut().unwrap().send_frame(frame) {
                eprintln!("\nvideo encoder error, this video file stops here: {e}");
                self.close_video();
            }
            return;
        }
        self.close_video();
        if size.0 < 2 || size.1 < 2 {
            return;
        }
        let idx = self.stats.video_files.fetch_add(1, Ordering::Relaxed) + 1;
        let path = dir.join(format!("video_{idx:03}.mp4"));
        match VideoEncoder::new(
            VideoSettingsBuilder::new(size.0, size.1)
                .sub_type(VideoSettingsSubType::H264)
                .bitrate(self.mbps * 1_000_000)
                .frame_rate(60),
            AudioSettingsBuilder::default().disabled(true),
            ContainerSettingsBuilder::default(),
            &path,
        ) {
            Ok(mut e) => {
                if n > 0 {
                    eprintln!("\nframe size is now {w}x{h}, new video file {}", path.display());
                }
                let _ = e.send_frame(frame);
                self.encoder = Some(e);
                self.enc_size = size;
            }
            Err(e) => eprintln!("\nvideo encoder failed to start, no video for now: {e}"),
        }
    }

    fn close_video(&mut self) {
        if let Some(enc) = self.encoder.take()
            && let Err(e) = enc.finish()
        {
            eprintln!("\ncould not finish a video file: {e}");
        }
    }

    fn finish(&mut self) {
        let _ = self.csv.flush();
        self.close_video();
    }
}

impl GraphicsCaptureApiHandler for Rec {
    type Flags = Flags;
    type Error = Box<dyn std::error::Error + Send + Sync>;

    fn new(ctx: Context<Self::Flags>) -> Result<Self, Self::Error> {
        let f = ctx.flags;
        let mut csv = BufWriter::new(File::create(&f.csv)?);
        writeln!(csv, "frame,t_ms,ticks,gap_ms,width,height,sampled,mean_luma,cb_ms")?;
        Ok(Self {
            encoder: None,
            enc_size: (0, 0),
            video_dir: f.video_dir,
            mbps: f.mbps,
            tx: f.tx,
            csv,
            last_flush: Instant::now(),
            last_ts: None,
            next_sample: 0,
            sample_step: f.sample_step,
            stats: f.stats,
            buf: Vec::new(),
        })
    }

    // Never returns Err for a single bad frame: any Err here ends the whole capture.
    fn on_frame_arrived(&mut self, frame: &mut Frame, _c: InternalCaptureControl) -> Result<(), Self::Error> {
        let cb_start = Instant::now();
        let Ok(ts) = frame.timestamp().map(|t| t.Duration) else {
            return Ok(());
        };
        let (w, h) = (frame.width(), frame.height());
        let n = self.stats.frames.fetch_add(1, Ordering::Relaxed);
        if n == 0 {
            self.stats.first_ts.store(ts, Ordering::Relaxed);
            self.next_sample = ts;
            // Marks use QPC "now"; frames use composition time. They should be close.
            let off = (now_ticks() - ts) / 10_000;
            self.stats.clock_offset_ms.store(off, Ordering::Relaxed);
            if !(-50..=500).contains(&off) {
                eprintln!("\nwarning: frame clock is {off} ms away from the mark clock; mark offsets may be wrong");
            }
        }
        self.stats.last_ts.store(ts, Ordering::Relaxed);
        let gap_us = self.last_ts.map(|l| ((ts - l).max(0) / 10) as u64).unwrap_or(0);
        self.stats.max_gap_us.fetch_max(gap_us, Ordering::Relaxed);
        self.last_ts = Some(ts);
        let old_w = self.stats.width.swap(w as u64, Ordering::Relaxed);
        let old_h = self.stats.height.swap(h as u64, Ordering::Relaxed);
        if n > 0 && (old_w != w as u64 || old_h != h as u64) {
            self.stats.size_changes.fetch_add(1, Ordering::Relaxed);
        }

        self.video_frame(frame, w, h, n);

        let mut mean = String::new();
        let sampled = ts >= self.next_sample;
        if sampled {
            // Keep the sample grid fixed so a slow frame does not shift every later sample.
            while self.next_sample <= ts {
                self.next_sample += self.sample_step;
            }
            match frame.buffer() {
                Ok(fb) => {
                    let rgba = fb.as_nopadding_buffer(&mut self.buf).to_vec();
                    let m = mean_luma(&rgba);
                    mean = m.to_string();
                    self.stats.sampled.fetch_add(1, Ordering::Relaxed);
                    if m < 3 {
                        self.stats.black.fetch_add(1, Ordering::Relaxed);
                    }
                    if let Err(TrySendError::Full(_)) = self.tx.try_send(FrameMsg { ts, w, h, rgba }) {
                        self.stats.dropped.fetch_add(1, Ordering::Relaxed);
                    }
                }
                Err(_) => {
                    self.stats.sample_errors.fetch_add(1, Ordering::Relaxed);
                }
            }
        }
        let first = self.stats.first_ts.load(Ordering::Relaxed);
        let _ = writeln!(
            self.csv,
            "{n},{:.3},{ts},{:.3},{w},{h},{},{mean},{:.2}",
            (ts - first) as f64 / 10_000.0,
            gap_us as f64 / 1000.0,
            sampled as u8,
            cb_start.elapsed().as_secs_f64() * 1000.0
        );
        if self.last_flush.elapsed() >= Duration::from_secs(1) {
            let _ = self.csv.flush();
            self.last_flush = Instant::now();
        }
        Ok(())
    }

    fn on_closed(&mut self) -> Result<(), Self::Error> {
        eprintln!("\nThe game window closed.");
        Ok(())
    }
}

/// Mean brightness 0..255 over a sparse pixel grid.
fn mean_luma(rgba: &[u8]) -> u8 {
    let px = rgba.len() / 4;
    if px == 0 {
        return 0;
    }
    let step = (px / 4096).max(1);
    let (mut sum, mut n) = (0u64, 0u64);
    let mut i = 0;
    while i < px {
        let p = &rgba[i * 4..i * 4 + 3];
        sum += (p[0] as u64 * 299 + p[1] as u64 * 587 + p[2] as u64 * 114) / 1000;
        n += 1;
        i += step;
    }
    (sum / n) as u8
}

struct Ring {
    frames: VecDeque<Entry>,
    bytes: u64,
    max_bytes: u64,
    keep_ticks: i64,
}

impl Ring {
    fn push(&mut self, e: Entry, stats: &Stats) {
        let ts = e.ts;
        self.bytes += e.qoi.len() as u64;
        self.frames.push_back(e);
        // Keep one frame older than the cutoff: it is what was on screen at the cutoff.
        while self.frames.len() > 1 && self.frames[1].ts < ts - self.keep_ticks {
            self.pop();
        }
        while self.frames.len() > 1 && self.bytes > self.max_bytes {
            self.pop();
            stats.ring_pruned_for_memory.fetch_add(1, Ordering::Relaxed);
        }
    }

    fn pop(&mut self) {
        if let Some(old) = self.frames.pop_front() {
            self.bytes -= old.qoi.len() as u64;
        }
    }

    /// Writes the raw QOI frames of a mark at once (fast, no decode). They become PNGs at exit.
    fn dump(&self, dir: &Path, mark_ts: i64, pre: i64, post: i64) -> Result<usize> {
        fs::create_dir_all(dir)?;
        let start = mark_ts - pre;
        let on_screen = self.frames.iter().rev().find(|e| e.ts < start);
        let picked = on_screen.into_iter().chain(self.frames.iter().filter(|e| e.ts >= start && e.ts <= mark_ts + post));
        let mut n = 0;
        for (i, e) in picked.enumerate() {
            let off_ms = (e.ts - mark_ts) / 10_000;
            fs::write(dir.join(format!("f{i:03}_{off_ms:+06}ms.qoi")), &e.qoi)?;
            n += 1;
        }
        Ok(n)
    }
}

/// Owns the in-memory ring of lossless frames and writes mark folders.
fn worker(frames: Receiver<FrameMsg>, ctl: Receiver<Ctl>, mut ring: Ring, pre: i64, post: i64, stats: Arc<Stats>) {
    let mut frames_open = true;
    loop {
        if frames_open {
            match frames.recv_timeout(Duration::from_millis(30)) {
                Ok(f) => encode_into(&mut ring, f, &stats),
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => frames_open = false,
            }
        } else {
            thread::sleep(Duration::from_millis(30));
        }
        // Frames that already arrived go in before a dump looks at the ring.
        while let Ok(f) = frames.try_recv() {
            encode_into(&mut ring, f, &stats);
        }
        while let Ok(c) = ctl.try_recv() {
            match c {
                Ctl::Dump { id, mark_ts, dir } => match ring.dump(&dir, mark_ts, pre, post) {
                    Ok(n) => eprintln!("\nmark {id}: {n} frames saved"),
                    Err(e) => eprintln!("\nmark {id} failed: {e:#}"),
                },
                Ctl::Quit => return,
            }
        }
    }
}

fn encode_into(ring: &mut Ring, f: FrameMsg, stats: &Stats) {
    match qoi::encode_to_vec(&f.rgba, f.w, f.h) {
        Ok(q) => ring.push(Entry { ts: f.ts, qoi: q }, stats),
        Err(e) => eprintln!("\nqoi encode failed: {e}"),
    }
}

/// Turns every saved .qoi under `marks/` into a .png, using all cores.
fn convert_marks(dir: &Path) {
    let mut files = Vec::new();
    let Ok(marks) = fs::read_dir(dir.join("marks")) else { return };
    for m in marks.flatten() {
        if let Ok(rd) = fs::read_dir(m.path()) {
            files.extend(rd.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "qoi")));
        }
    }
    if files.is_empty() {
        return;
    }
    eprintln!("Converting {} saved frames to PNG...", files.len());
    let threads = thread::available_parallelism().map(|n| n.get()).unwrap_or(4).min(8);
    let files = Arc::new(files);
    let next = Arc::new(AtomicU64::new(0));
    let hs: Vec<_> = (0..threads)
        .map(|_| {
            let (files, next) = (files.clone(), next.clone());
            thread::spawn(move || {
                loop {
                    let i = next.fetch_add(1, Ordering::Relaxed) as usize;
                    let Some(p) = files.get(i) else { break };
                    match qoi_to_png(p) {
                        Ok(()) => {
                            let _ = fs::remove_file(p);
                        }
                        Err(e) => eprintln!("could not convert {}: {e:#} (the .qoi file is kept)", p.display()),
                    }
                }
            })
        })
        .collect();
    for h in hs {
        let _ = h.join();
    }
}

fn qoi_to_png(p: &Path) -> Result<()> {
    let (hdr, rgba) = qoi::decode_to_vec(fs::read(p)?)?;
    let mut enc = png::Encoder::new(BufWriter::new(File::create(p.with_extension("png"))?), hdr.width, hdr.height);
    enc.set_color(if hdr.channels.as_u8() == 4 { png::ColorType::Rgba } else { png::ColorType::Rgb });
    enc.set_depth(png::BitDepth::Eight);
    enc.set_compression(png::Compression::Fast);
    enc.write_header()?.write_image_data(&rgba)?;
    Ok(())
}

fn pick_window(args: &Args) -> Result<Window> {
    if let Some(p) = &args.process {
        for w in Window::enumerate()? {
            if w.process_name().map(|n| n.eq_ignore_ascii_case(p)).unwrap_or(false) {
                return Ok(w);
            }
        }
        bail!("no window for process '{p}'. Run with --list to see names.");
    }
    if let Some(t) = &args.title {
        return Window::from_contains_name(t).map_err(|e| anyhow!("no window title contains '{t}': {e}"));
    }
    for s in (1..=args.countdown).rev() {
        eprint!("\rSwitch to the game now. Capturing the active window in {s} s ");
        thread::sleep(Duration::from_secs(1));
    }
    eprintln!();
    Ok(Window::foreground()?)
}

fn main() -> Result<()> {
    let args = Args::parse();
    // Real pixel sizes, not DPI-scaled ones.
    unsafe {
        let _ = windows::Win32::UI::HiDpi::SetProcessDpiAwarenessContext(
            windows::Win32::UI::HiDpi::DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
        );
    }
    if let Some(d) = &args.convert {
        convert_marks(d);
        return Ok(());
    }
    if args.list {
        for w in Window::enumerate()? {
            let title = w.title().unwrap_or_default();
            if title.trim().is_empty() {
                continue;
            }
            let proc = w.process_name().unwrap_or_default();
            let (wd, ht) = (w.width().unwrap_or(0), w.height().unwrap_or(0));
            println!("{proc:<28} {wd:>5}x{ht:<5} {title}");
        }
        return Ok(());
    }
    let mark_vk = vk(&args.mark_key)?;
    let stop_vk = vk(&args.stop_key)?;
    if args.fps == 0 || args.fps > 60 {
        bail!("--fps must be 1..60");
    }

    let window = pick_window(&args)?;
    let title = window.title().unwrap_or_default();
    let process = window.process_name().unwrap_or_default();
    let size = (window.width().unwrap_or(0), window.height().unwrap_or(0));

    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let dir = args.out.join(format!("{}-{stamp}", args.name));
    fs::create_dir_all(&dir).with_context(|| format!("create {}", dir.display()))?;
    let dir = dir.canonicalize()?;
    fs::write(
        dir.join("session.json"),
        serde_json::to_string_pretty(&serde_json::json!({
            "name": args.name, "window_title": title, "process": process,
            "window_size": [size.0, size.1], "ring_fps": args.fps, "pre_s": args.pre, "post_s": args.post,
            "ring_mb": args.ring_mb, "video": !args.no_video, "video_mbps": args.mbps,
            "started": chrono::Local::now().to_rfc3339(),
        }))?,
    )?;
    let mut marks = BufWriter::new(File::create(dir.join("marks.csv"))?);
    writeln!(marks, "mark,t_ms,ticks")?;
    marks.flush()?;

    unsafe { SetConsoleCtrlHandler(Some(console_handler), true)? };

    let stats = Arc::new(Stats::default());
    // Small frame queue: at 4K each queued frame is about 33 MB.
    let (ftx, frx) = mpsc::sync_channel::<FrameMsg>(3);
    let (ctx, crx): (Sender<Ctl>, Receiver<Ctl>) = mpsc::channel();
    let pre_ticks = args.pre as i64 * 10_000_000;
    let post_ticks = args.post as i64 * 10_000_000;
    let ring = Ring {
        frames: VecDeque::new(),
        bytes: 0,
        max_bytes: args.ring_mb * 1024 * 1024,
        keep_ticks: pre_ticks + post_ticks + 10_000_000,
    };
    let wstats = stats.clone();
    let worker_h = thread::spawn(move || worker(frx, crx, ring, pre_ticks, post_ticks, wstats));

    let flags = Flags {
        video_dir: (!args.no_video).then(|| dir.clone()),
        mbps: args.mbps,
        sample_step: 10_000_000 / args.fps as i64,
        tx: ftx,
        stats: stats.clone(),
        csv: dir.join("frames.csv"),
    };
    let settings = Settings::new(
        window,
        CursorCaptureSettings::WithoutCursor,
        DrawBorderSettings::WithoutBorder,
        SecondaryWindowSettings::Default,
        MinimumUpdateIntervalSettings::Default,
        DirtyRegionSettings::Default,
        ColorFormat::Rgba8,
        flags,
    );
    let control = match Rec::start_free_threaded(settings) {
        Ok(c) => c,
        Err(e) => {
            let _ = ctx.send(Ctl::Quit);
            let _ = worker_h.join();
            DONE.store(true, Ordering::SeqCst);
            bail!("capture failed to start: {e}");
        }
    };
    let callback = control.callback();

    eprintln!("Recording '{title}' ({process})");
    eprintln!("Saving to {}", dir.display());
    eprintln!("Press {} when a pop-up appears. Press {} (or Ctrl+C) to stop.", args.mark_key, args.stop_key);

    // Dumps wait a little past `post` so frames from the end of the window have reached the ring.
    let dump_delay = Duration::from_millis(args.post as u64 * 1000 + 500);
    let mut pending: Vec<(u32, i64, Instant)> = Vec::new();
    let mut mark_id = 0u32;
    let (mut mark_was, mut stop_was) = (key_down(mark_vk), key_down(stop_vk));
    let mut last_print = Instant::now();
    let started = Instant::now();
    let mut reason = "stop key";

    // Nothing in this loop may return early: the save steps after it must always run.
    loop {
        thread::sleep(Duration::from_millis(10));
        let (m, s) = (key_down(mark_vk), key_down(stop_vk));
        let auto = args.auto_mark.is_some_and(|a| mark_id == 0 && started.elapsed() >= Duration::from_secs(a));
        if (m && !mark_was) || auto {
            mark_id += 1;
            let ts = now_ticks();
            let first = stats.first_ts.load(Ordering::Relaxed);
            let t_ms = if stats.frames.load(Ordering::Relaxed) > 0 {
                format!("{:.1}", (ts - first) as f64 / 10_000.0)
            } else {
                String::new()
            };
            if writeln!(marks, "{mark_id},{t_ms},{ts}").and_then(|_| marks.flush()).is_err() {
                eprintln!("\ncould not write marks.csv");
            }
            pending.push((mark_id, ts, Instant::now() + dump_delay));
            eprintln!("\nmark {mark_id}: saving frames in {:.1} s", dump_delay.as_secs_f32());
        }
        mark_was = m;
        let stop_now = s && !stop_was;
        stop_was = s;

        pending.retain(|(id, ts, due)| {
            if Instant::now() >= *due {
                let _ = ctx.send(Ctl::Dump { id: *id, mark_ts: *ts, dir: dir.join(format!("marks/mark_{id:03}")) });
                false
            } else {
                true
            }
        });

        if last_print.elapsed() >= Duration::from_secs(1) {
            last_print = Instant::now();
            let fr = stats.frames.load(Ordering::Relaxed);
            let secs = started.elapsed().as_secs_f64();
            eprint!(
                "\r{:>6.0} s  {fr} frames  {:.1} fps avg  {} marks  {} black samples   ",
                secs,
                fr as f64 / secs.max(0.001),
                mark_id,
                stats.black.load(Ordering::Relaxed)
            );
        }

        if stop_now {
            break;
        }
        if args.seconds > 0 && started.elapsed() >= Duration::from_secs(args.seconds) {
            reason = "time limit";
            break;
        }
        if STOP.load(Ordering::SeqCst) {
            reason = "console stop (Ctrl+C, window closed or shut down)";
            break;
        }
        if control.is_finished() {
            reason = "capture ended (game window closed or capture error)";
            break;
        }
    }
    eprintln!("\nStopping ({reason})...");

    if let Err(e) = control.stop() {
        eprintln!("capture thread ended with an error: {e}");
    }
    callback.lock().finish();
    for (id, ts, _) in pending.drain(..) {
        let _ = ctx.send(Ctl::Dump { id, mark_ts: ts, dir: dir.join(format!("marks/mark_{id:03}")) });
    }
    let _ = ctx.send(Ctl::Quit);
    let _ = worker_h.join();

    let fr = stats.frames.load(Ordering::Relaxed);
    let span_s = (stats.last_ts.load(Ordering::Relaxed) - stats.first_ts.load(Ordering::Relaxed)) as f64 / 1e7;
    let summary = serde_json::json!({
        "stop_reason": reason,
        "frames": fr,
        "span_s": span_s,
        "avg_fps": if span_s > 0.0 { fr as f64 / span_s } else { 0.0 },
        "max_gap_ms": stats.max_gap_us.load(Ordering::Relaxed) as f64 / 1000.0,
        "sampled_frames": stats.sampled.load(Ordering::Relaxed),
        "sample_errors": stats.sample_errors.load(Ordering::Relaxed),
        "black_sampled_frames": stats.black.load(Ordering::Relaxed),
        "size_changes": stats.size_changes.load(Ordering::Relaxed),
        "last_size": [stats.width.load(Ordering::Relaxed), stats.height.load(Ordering::Relaxed)],
        "video_files": stats.video_files.load(Ordering::Relaxed),
        "marks": mark_id,
        "ring_frames_dropped": stats.dropped.load(Ordering::Relaxed),
        "ring_frames_pruned_for_memory": stats.ring_pruned_for_memory.load(Ordering::Relaxed),
        "clock_offset_ms": stats.clock_offset_ms.load(Ordering::Relaxed),
    });
    let _ = fs::write(dir.join("stats.json"), serde_json::to_string_pretty(&summary).unwrap_or_default());
    // Everything is saved now; a console close may let the process end. PNG conversion is extra.
    DONE.store(true, Ordering::SeqCst);
    eprintln!("{}", serde_json::to_string_pretty(&summary).unwrap_or_default());
    if fr == 0 {
        eprintln!("No frames arrived. The game may block capture or run in exclusive fullscreen.");
    }
    if CLOSING.load(Ordering::SeqCst) {
        eprintln!("Mark frames are saved as .qoi. Run: recorder --convert \"{}\"", dir.display());
    } else {
        convert_marks(&dir);
    }
    Ok(())
}
