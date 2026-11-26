//! `arena live`: plays a scene in real time in a real window, captures that window with Windows
//! Graphics Capture (WGC) and feeds the captured frames to a reader as they arrive. Then it scores
//! the events and reports end-to-end latency and CPU use.
//!
//! Rules (spec, 'Live mode rules'): never take the foreground or show on Max's screen. Modes:
//! - `hidden-desktop-thread`: only the player thread moves to a desktop made with CreateDesktop.
//! - `hidden-desktop`: a child `arena` process runs the whole session on such a desktop.
//!   On this PC WGC refuses a window on another desktop in both variants (CreateForWindow gives
//!   E_INVALIDARG), so both fail here.
//! - `offscreen`: a layered, non-activating tool window placed outside all monitors. Layered is
//!   needed: plain GDI drawing is clipped off screen and WGC then only sends black.
//! - `auto` (default): try both hidden-desktop variants, fall back to `offscreen`.
//!
//! How frames are checked: the window is `STRIP_H` rows taller than the scene. Those extra rows
//! hold the frame index as black and white blocks. The capture side decodes the index, so it knows
//! which scene frame it got and when that frame was presented. The strip is cut off before the
//! reader sees the frame. A few captured frames are also compared pixel by pixel with a fresh
//! render, because the oracle reader never looks at pixels.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(clap::Args, Debug, Clone)]
pub struct Args {
    /// Scenario JSON to play (default: arena-data/suites/smoke/smoke-000.json)
    #[arg(long)]
    pub scenario: Option<PathBuf>,
    /// Reader name (see `arena run --help`)
    #[arg(long, default_value = "oracle")]
    pub reader: String,
    /// Folder for report.json / report.md / live.json (default arena-data/reports/live-<id>-<reader>)
    #[arg(long)]
    pub report: Option<PathBuf>,
    /// auto (hidden desktop variants, then off-screen), hidden-desktop-thread, hidden-desktop or offscreen
    #[arg(long, default_value = "auto")]
    pub mode: String,
    /// Reader option key=value (repeatable)
    #[arg(long = "reader-opt")]
    pub reader_opts: Vec<String>,
    /// How long to wait for the first valid captured frame before giving up on a mode, ms
    #[arg(long, default_value_t = 4000)]
    pub probe_ms: u64,
    /// Abort only when arena itself takes the foreground; log switches between other windows
    /// (default: abort on any foreground change, as the spec says)
    #[arg(long)]
    pub lenient_foreground: bool,
    /// Testing: sleep this long after each reader call, to exercise frame dropping
    #[arg(long, default_value_t = 0.0)]
    pub reader_delay_ms: f64,
    /// Internal: run the session in this process and write the result here (used on the hidden desktop)
    #[arg(long, hide = true)]
    pub child_out: Option<PathBuf>,
}

/// Rows added below the scene for the frame index code.
pub const STRIP_H: u32 = 8;
/// Width of one code block in pixels.
pub const BLOCK_W: u32 = 16;
/// Blocks: white sync, black sync, 16 data bits (most significant first), even parity.
pub const BLOCKS: u32 = 19;
/// Index shown while the probe waits for the first captured frame.
pub const PROBE_INDEX: u16 = u16::MAX;

/// Writes the code for `index` into the strip rows of an RGBA (or BGRA) frame of `w` x (`h` + STRIP_H).
pub fn encode_strip(buf: &mut [u8], w: u32, h: u32, index: u16) {
    let bits = strip_bits(index);
    for y in h..h + STRIP_H {
        for x in 0..w {
            let b = x / BLOCK_W;
            let v = if b < BLOCKS { if bits[b as usize] { 255 } else { 0 } } else { 128 };
            let o = ((y * w + x) * 4) as usize;
            buf[o..o + 4].copy_from_slice(&[v, v, v, 255]);
        }
    }
}

fn strip_bits(index: u16) -> [bool; BLOCKS as usize] {
    let mut bits = [false; BLOCKS as usize];
    bits[0] = true;
    bits[1] = false;
    for k in 0..16 {
        bits[2 + k] = (index >> (15 - k)) & 1 == 1;
    }
    bits[18] = index.count_ones() % 2 == 1;
    bits
}

/// Reads the frame index from the strip of a captured RGBA frame `stride_w` pixels wide.
/// `None` when the sync blocks or the parity do not match (torn or foreign frame).
pub fn decode_strip(rgba: &[u8], stride_w: u32, h: u32) -> Option<u16> {
    if stride_w < BLOCKS * BLOCK_W {
        return None;
    }
    let y = h + STRIP_H / 2;
    let mut bits = [false; BLOCKS as usize];
    for (b, bit) in bits.iter_mut().enumerate() {
        let x = b as u32 * BLOCK_W + BLOCK_W / 2;
        let o = ((y * stride_w + x) * 4) as usize;
        let px = rgba.get(o..o + 3)?;
        let luma = (px[0] as u32 + px[1] as u32 + px[2] as u32) / 3;
        *bit = match luma {
            0..=64 => false,
            192..=255 => true,
            _ => return None,
        };
    }
    if !bits[0] || bits[1] {
        return None;
    }
    let mut index = 0u16;
    for k in 0..16 {
        index = (index << 1) | bits[2 + k] as u16;
    }
    (bits[18] == (index.count_ones() % 2 == 1)).then_some(index)
}

/// Pixel check of one captured frame against a fresh render of the same frame index.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct PixelCheck {
    pub index: usize,
    /// Mean absolute difference over R, G and B, 0..255.
    pub mean_abs_diff: f64,
    /// Share of pixels where some channel differs by more than 16.
    pub share_off: f64,
}

pub fn pixel_check(index: usize, captured: &[u8], rendered: &[u8]) -> PixelCheck {
    let n = (captured.len().min(rendered.len()) / 4).max(1);
    let mut sum = 0u64;
    let mut off = 0usize;
    for (c, r) in captured.as_chunks::<4>().0.iter().zip(rendered.as_chunks::<4>().0) {
        let d = [0, 1, 2].map(|k| (c[k] as i32 - r[k] as i32).unsigned_abs());
        sum += (d[0] + d[1] + d[2]) as u64;
        if d.iter().any(|v| *v > 16) {
            off += 1;
        }
    }
    PixelCheck { index, mean_abs_diff: sum as f64 / (n as f64 * 3.0), share_off: off as f64 / n as f64 }
}

/// Timing and CPU numbers of one live run. Saved as `live.json` next to the report.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct LiveStats {
    /// `hidden-desktop` or `offscreen`.
    pub mode: String,
    /// What happened to the hidden-desktop attempt when it was tried and not used.
    pub hidden_desktop_note: Option<String>,
    pub window_pos: (i32, i32),
    pub captured_size: (u32, u32),
    /// Time from capture start to the first frame with a valid code, ms.
    pub probe_ms: f64,
    pub frames_scene: usize,
    pub frames_presented: usize,
    /// Presented more than one frame interval after their due time.
    pub frames_presented_late: usize,
    pub capture_callbacks: usize,
    /// Captured frames the reader thread did not take before a newer one came.
    pub capture_overwritten: usize,
    pub captured_undecodable: usize,
    /// Captured frames showing an index the reader already had.
    pub captured_repeat: usize,
    pub frames_read: usize,
    /// Present (UpdateLayeredWindow returned) to the WGC callback starting, ms.
    pub present_to_callback_ms: Percentiles,
    /// Copying the frame out of the GPU texture inside the callback, ms.
    pub callback_copy_ms: Percentiles,
    /// Copy done to the reader starting on the frame (queue wait), ms.
    pub copy_to_reader_ms: Percentiles,
    /// WGC frame timestamp minus present, ms. The timestamp is the planned composition time,
    /// so it can be later than the callback; kept for reference only.
    pub present_to_wgc_timestamp_ms: Percentiles,
    /// Reader `on_frame` wall time, ms.
    pub reader_ms: Percentiles,
    /// Present to the reader returning: end-to-end latency per frame, ms.
    pub end_to_end_ms: Percentiles,
    pub wall_s: f64,
    /// CPU time of the reader thread (only the reader and the frame copy), ms and % of one core.
    pub reader_cpu_ms: f64,
    pub reader_cpu_pct_core: f64,
    /// CPU time of the WGC callback thread (frame copy out of the GPU texture).
    pub capture_cpu_ms: f64,
    pub capture_cpu_pct_core: f64,
    /// Whole process: includes the scene renderer, which plays the game here.
    pub process_cpu_ms: f64,
    pub process_cpu_pct_core: f64,
    pub pixel_checks: Vec<PixelCheck>,
    /// Foreground switches during the run that were not arena (the user working).
    #[serde(default)]
    pub foreground_switches_by_others: Vec<String>,
    /// Why the capture cannot be trusted (empty for a valid run). A run with reasons fails.
    #[serde(default)]
    pub invalid_reasons: Vec<String>,
}

/// Worst allowed mean pixel difference between a captured frame and a fresh render (0..255).
pub const MAX_PIXEL_DIFF: f64 = 2.0;

/// Checks that the capture itself worked, so a broken capture never looks like a bad reader.
/// A slow reader may skip frames (that is measured, not an error), so `frames_read` is not gated.
pub fn validity_problems(st: &LiveStats) -> Vec<String> {
    let mut v = Vec::new();
    if st.pixel_checks.is_empty() {
        v.push("no pixel check was made".to_string());
    }
    let worst = st.pixel_checks.iter().map(|c| c.mean_abs_diff).fold(0.0, f64::max);
    if worst > MAX_PIXEL_DIFF {
        v.push(format!("captured frames differ from the render (worst mean abs diff {worst:.2} > {MAX_PIXEL_DIFF})"));
    }
    if st.frames_presented * 10 < st.frames_scene * 9 {
        v.push(format!("only {} of {} scene frames were presented", st.frames_presented, st.frames_scene));
    }
    let decoded = st.capture_callbacks.saturating_sub(st.captured_undecodable);
    if decoded * 10 < st.frames_presented * 8 {
        v.push(format!("only {decoded} decodable captured frames for {} presented", st.frames_presented));
    }
    if st.captured_undecodable * 20 > st.capture_callbacks.max(1) {
        v.push(format!("{} of {} captured frames had no readable frame code", st.captured_undecodable, st.capture_callbacks));
    }
    if st.frames_read == 0 {
        v.push("the reader got no frame".to_string());
    }
    v
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct Percentiles {
    pub n: usize,
    pub p50: f64,
    pub p95: f64,
    pub max: f64,
}

impl Percentiles {
    pub fn of(v: &[f64]) -> Self {
        Percentiles {
            n: v.len(),
            p50: crate::score::percentile(v, 0.5),
            p95: crate::score::percentile(v, 0.95),
            max: v.iter().copied().fold(0.0, f64::max),
        }
    }
}

/// Result of one session, passed from the hidden-desktop child to the parent as JSON.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum ChildResult {
    Ok { run: Box<crate::runner::ScenarioRun>, stats: Box<LiveStats> },
    /// No valid frame arrived within the probe time: the mode does not work here.
    NoFrames(String),
    Failed(String),
}

#[cfg(not(windows))]
pub fn run(_args: &Args) -> Result<()> {
    anyhow::bail!("live: needs Windows Graphics Capture (Windows only)")
}

#[cfg(windows)]
pub use win::run;

#[cfg(windows)]
mod win {
    use super::*;
    use crate::background::BgTextMap;
    use crate::generator;
    use crate::model::Scenario;
    use crate::reader::FrameRef;
    use crate::render::SceneRenderer;
    use crate::runner::{Clock, ScenarioRun};
    use crate::score;
    use anyhow::{Context as _, bail};
    use std::collections::BTreeMap;
    use std::path::Path;
    use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, AtomicUsize, Ordering};
    use std::sync::{Arc, Condvar, Mutex};
    use std::thread;
    use std::time::{Duration, Instant};
    use windows::Win32::Foundation::{CloseHandle, FILETIME, HANDLE, HWND, LPARAM, LRESULT, WAIT_OBJECT_0, WPARAM};
    use windows::Win32::Foundation::{COLORREF, POINT, SIZE};
    use windows::Win32::Graphics::Gdi::{
        AC_SRC_ALPHA, AC_SRC_OVER, BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BLENDFUNCTION, CreateCompatibleDC, CreateDIBSection,
        DIB_RGB_COLORS, DeleteDC, DeleteObject, GdiFlush, HBITMAP, HDC, HGDIOBJ, SelectObject,
    };
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::System::Performance::{QueryPerformanceCounter, QueryPerformanceFrequency};
    use windows::Win32::System::StationsAndDesktops::{
        CloseDesktop, CreateDesktopW, DESKTOP_CONTROL_FLAGS, GetThreadDesktop, GetUserObjectInformationW, HDESK, SetThreadDesktop, UOI_NAME,
    };
    use windows::Win32::System::Threading::{
        CREATE_NO_WINDOW, GetCurrentProcess, GetCurrentProcessId, GetCurrentThread, GetCurrentThreadId, GetExitCodeProcess, GetProcessTimes,
        GetThreadTimes, PROCESS_INFORMATION, STARTUPINFOW, TerminateProcess, WaitForSingleObject,
    };
    use windows::Win32::UI::HiDpi::{DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetThreadDpiAwarenessContext};
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetForegroundWindow, GetSystemMetrics,
        GetWindowThreadProcessId, MA_NOACTIVATE, MSG, PM_REMOVE, PeekMessageW, RegisterClassW, SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN, SW_SHOWNOACTIVATE, ShowWindow, TranslateMessage, WM_ACTIVATE, WM_ERASEBKGND, WM_NCACTIVATE, WM_SETFOCUS,
        WM_MOUSEACTIVATE, WNDCLASSW, ULW_ALPHA, UpdateLayeredWindow, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_POPUP,
    };
    use windows::core::{PCWSTR, PWSTR, w};
    use windows_capture::capture::{Context, GraphicsCaptureApiHandler};
    use windows_capture::frame::Frame;
    use windows_capture::graphics_capture_api::InternalCaptureControl;
    use windows_capture::settings::{
        ColorFormat, CursorCaptureSettings, DirtyRegionSettings, DrawBorderSettings, MinimumUpdateIntervalSettings,
        SecondaryWindowSettings, Settings,
    };
    use windows_capture::window::Window;

    /// Pixel checks kept per run.
    const PIXEL_CHECKS: usize = 6;
    /// The player holds the last frame this long before it says it is done, ms.
    const HOLD_MS: f64 = 400.0;

    #[derive(Debug)]
    struct NoFrames(String);
    impl std::fmt::Display for NoFrames {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "no valid frame captured: {}", self.0)
        }
    }
    impl std::error::Error for NoFrames {}

    // ---------- clock ----------

    /// 100 ns ticks on the QPC clock, the same clock as WGC frame timestamps.
    fn now_ticks() -> i64 {
        let mut c = 0i64;
        let mut f = 0i64;
        unsafe {
            let _ = QueryPerformanceCounter(&mut c);
            let _ = QueryPerformanceFrequency(&mut f);
        }
        (c as i128 * 10_000_000 / f.max(1) as i128) as i64
    }

    fn ticks_to_ms(t: i64) -> f64 {
        t as f64 / 10_000.0
    }

    fn ms_to_ticks(ms: f64) -> i64 {
        (ms * 10_000.0) as i64
    }

    /// Sleeps until `target` ticks, waking a little early and then yielding for precision.
    fn sleep_until(target: i64) {
        loop {
            let left = target - now_ticks();
            if left <= 0 {
                return;
            }
            if left > 30_000 {
                thread::sleep(Duration::from_micros(((left - 20_000) / 10) as u64));
            } else {
                thread::yield_now();
            }
        }
    }

    fn filetime(f: FILETIME) -> u64 {
        ((f.dwHighDateTime as u64) << 32) | f.dwLowDateTime as u64
    }

    /// Kernel + user CPU of the calling thread, ms.
    fn thread_cpu_ms() -> f64 {
        let (mut a, mut b, mut k, mut u) = (FILETIME::default(), FILETIME::default(), FILETIME::default(), FILETIME::default());
        unsafe {
            if GetThreadTimes(GetCurrentThread(), &mut a, &mut b, &mut k, &mut u).is_err() {
                return 0.0;
            }
        }
        (filetime(k) + filetime(u)) as f64 / 10_000.0
    }

    fn process_cpu_ms() -> f64 {
        let (mut a, mut b, mut k, mut u) = (FILETIME::default(), FILETIME::default(), FILETIME::default(), FILETIME::default());
        unsafe {
            if GetProcessTimes(GetCurrentProcess(), &mut a, &mut b, &mut k, &mut u).is_err() {
                return 0.0;
            }
        }
        (filetime(k) + filetime(u)) as f64 / 10_000.0
    }

    // ---------- foreground guard ----------

    fn foreground() -> (isize, u32) {
        unsafe {
            let h = GetForegroundWindow();
            let mut pid = 0u32;
            if !h.0.is_null() {
                GetWindowThreadProcessId(h, Some(&mut pid));
            }
            (h.0 as isize, pid)
        }
    }

    /// Polls the foreground window every 50 ms and records the first change.
    /// Polls the foreground window every 50 ms during the run.
    /// A switch to a window of this run (or its hidden-desktop child) aborts the run: that would
    /// be arena stealing focus. Switches between other windows are the user working; they are
    /// logged and do not abort only with `--lenient-foreground`. The window procedure also flags any
    /// activation or focus message, which a 50 ms poll could miss.
    struct ForegroundGuard {
        before: (isize, u32),
        abort: Arc<Mutex<Option<String>>>,
        others: Arc<Mutex<Vec<String>>>,
        stop: Arc<AtomicBool>,
        handle: Option<thread::JoinHandle<()>>,
    }

    impl ForegroundGuard {
        fn start(child_pid: Arc<AtomicU64>, strict: bool) -> Self {
            let before = foreground();
            let abort = Arc::new(Mutex::new(None));
            let others = Arc::new(Mutex::new(Vec::new()));
            let stop = Arc::new(AtomicBool::new(false));
            let (a, o, s) = (abort.clone(), others.clone(), stop.clone());
            let handle = thread::spawn(move || {
                let started = Instant::now();
                let mut prev = before;
                while !s.load(Ordering::SeqCst) {
                    let now = foreground();
                    if now != prev {
                        let child = child_pid.load(Ordering::SeqCst);
                        let ours = now.0 != 0 && (now.1 == unsafe { GetCurrentProcessId() } || (child != 0 && now.1 as u64 == child));
                        let msg = format!(
                            "after {:.1} s the foreground went from hwnd {:#x} (pid {}) to hwnd {:#x} (pid {})",
                            started.elapsed().as_secs_f64(),
                            prev.0,
                            prev.1,
                            now.0,
                            now.1
                        );
                        if ours {
                            *a.lock().unwrap() = Some(format!("{msg}, a window of this arena run took the foreground"));
                            return;
                        }
                        if strict {
                            *a.lock().unwrap() = Some(format!("{msg} (run with --lenient-foreground to only abort when arena takes it)"));
                            return;
                        }
                        let mut v = o.lock().unwrap();
                        if v.len() < 20 {
                            v.push(msg);
                        }
                        prev = now;
                    }
                    thread::sleep(Duration::from_millis(50));
                }
            });
            ForegroundGuard { before, abort, others, stop, handle: Some(handle) }
        }

        fn aborted(&self) -> Option<String> {
            self.abort.lock().unwrap().clone()
        }

        fn abort_flag(&self) -> Arc<Mutex<Option<String>>> {
            self.abort.clone()
        }

        /// Stops polling and checks the foreground once more. Returns the switches by others.
        fn finish(mut self, strict: bool) -> Result<Vec<String>> {
            self.stop.store(true, Ordering::SeqCst);
            if let Some(h) = self.handle.take() {
                let _ = h.join();
            }
            if let Some(m) = self.aborted() {
                bail!("live: aborted, {m}");
            }
            if ACTIVATED.load(Ordering::SeqCst) {
                bail!("live: aborted, the player window received an activation or focus message");
            }
            let after = foreground();
            if after.0 != 0 && after.1 == unsafe { GetCurrentProcessId() } {
                bail!("live: aborted, a window of this arena run is the foreground at the end");
            }
            if strict && after != self.before {
                bail!("live: aborted, foreground changed from {:#x} to {:#x} by the end of the run", self.before.0, after.0);
            }
            let v = self.others.lock().unwrap().clone();
            Ok(v)
        }
    }

    // ---------- entry ----------

    pub fn run(args: &Args) -> Result<()> {
        let scenario_path = args
            .scenario
            .clone()
            .unwrap_or_else(|| crate::data_dir().join("suites").join("smoke").join("smoke-000.json"));
        let scenario_path = std::fs::canonicalize(&scenario_path).with_context(|| format!("scenario {}", scenario_path.display()))?;
        if !crate::readers::names().contains(&args.reader.as_str()) {
            bail!("unknown reader {:?}; known: {}", args.reader, crate::readers::names().join(", "));
        }
        let opts = parse_opts(&args.reader_opts)?;
        let scenario = generator::load_scenario(&scenario_path)?;

        if let Some(out) = &args.child_out {
            // Child on the hidden desktop: run here, report through the file. Its window sits at
            // (0, 0), so refuse to run on the user's desktop (someone started it by hand).
            let desk = thread_desktop_name().context("read the desktop name")?;
            if desk.eq_ignore_ascii_case("Default") {
                bail!("--child-out is internal: it only runs on a hidden desktop started by `arena live`, not on {desk:?}");
            }
            let cfg = Cfg { reader: &args.reader, opts: &opts, probe_ms: args.probe_ms, delay_ms: args.reader_delay_ms, abort: None };
            let result = match session(&scenario, &cfg, Placement::Origin) {
                Ok((run, stats)) => ChildResult::Ok { run: Box::new(run), stats: Box::new(stats) },
                Err(e) => match e.downcast_ref::<NoFrames>() {
                    Some(n) => ChildResult::NoFrames(n.0.clone()),
                    None => ChildResult::Failed(format!("{e:#}")),
                },
            };
            std::fs::write(out, serde_json::to_string(&result)?)?;
            return Ok(());
        }

        let child_pid = Arc::new(AtomicU64::new(0));
        let guard = ForegroundGuard::start(child_pid.clone(), !args.lenient_foreground);
        let started = Instant::now();
        let outcome = play_with_mode(args, &scenario_path, &scenario, &opts, &guard, &child_pid);
        let guard_result = guard.finish(!args.lenient_foreground);
        let (run, mut stats) = outcome?;
        stats.foreground_switches_by_others = guard_result?;
        stats.invalid_reasons = validity_problems(&stats);
        eprintln!("live session took {:.1} s", started.elapsed().as_secs_f64());
        let invalid = stats.invalid_reasons.clone();
        write_report(args, &scenario, run, stats)?;
        if !invalid.is_empty() {
            bail!("live: the capture is not valid, so the scores mean nothing: {}", invalid.join("; "));
        }
        Ok(())
    }

    /// Name of the desktop this thread runs on ("Default" for the user's normal desktop).
    pub(super) fn thread_desktop_name() -> Result<String> {
        unsafe {
            let desk = GetThreadDesktop(GetCurrentThreadId()).context("GetThreadDesktop")?;
            let mut buf = [0u16; 256];
            let mut needed = 0u32;
            GetUserObjectInformationW(
                HANDLE(desk.0),
                UOI_NAME,
                Some(buf.as_mut_ptr() as *mut std::ffi::c_void),
                (buf.len() * 2) as u32,
                Some(&mut needed),
            )
            .context("GetUserObjectInformationW")?;
            let len = buf.iter().position(|c| *c == 0).unwrap_or(buf.len());
            Ok(String::from_utf16_lossy(&buf[..len]))
        }
    }

    /// One way of running the session; tried in order by `--mode auto`.
    type Attempt<'a> = dyn Fn() -> Result<(ScenarioRun, LiveStats)> + 'a;

    fn play_with_mode(
        args: &Args,
        path: &Path,
        s: &Scenario,
        opts: &BTreeMap<String, String>,
        guard: &ForegroundGuard,
        child_pid: &Arc<AtomicU64>,
    ) -> Result<(ScenarioRun, LiveStats)> {
        let cfg = Cfg { reader: &args.reader, opts, probe_ms: args.probe_ms, delay_ms: args.reader_delay_ms, abort: Some(guard.abort_flag()) };
        let child = || -> Result<(ScenarioRun, LiveStats)> {
            match run_hidden_child(args, path, s, guard, child_pid)? {
                ChildResult::Ok { run, stats } => Ok((*run, *stats)),
                ChildResult::NoFrames(m) => Err(NoFrames(m).into()),
                ChildResult::Failed(m) => bail!("{m}"),
            }
        };
        let thread = || hidden_thread_session(s, &cfg);
        match args.mode.as_str() {
            "hidden-desktop-thread" => thread(),
            "hidden-desktop" => child().context("hidden desktop (child process)"),
            "offscreen" => session(s, &cfg, Placement::OffScreen),
            "auto" => {
                let mut notes = Vec::new();
                let tries: [(&str, &Attempt); 2] =
                    [("thread on a hidden desktop", &thread), ("child process on a hidden desktop", &child)];
                for (name, f) in tries {
                    match f() {
                        Ok(v) => return Ok(v),
                        Err(e) => notes.push(format!("{name}: {e:#}")),
                    }
                    if let Some(m) = guard.aborted() {
                        bail!("live: aborted, {m}");
                    }
                }
                let note = notes.join("; ");
                eprintln!("hidden desktop did not work ({note}); using an off-screen tool window");
                let (run, mut stats) = session(s, &cfg, Placement::OffScreen)?;
                stats.hidden_desktop_note = Some(note);
                Ok((run, stats))
            }
            m => bail!("unknown --mode {m:?} (auto, hidden-desktop-thread, hidden-desktop, offscreen)"),
        }
    }

    /// Hidden desktop, in process: only the player thread moves to the new desktop
    /// (SetThreadDesktop before it makes any window); capture and reader stay on the default one.
    fn hidden_thread_session(s: &Scenario, cfg: &Cfg) -> Result<(ScenarioRun, LiveStats)> {
        let name = wide(&format!("arena-live-thread-{}", std::process::id()));
        let desk = unsafe { CreateDesktopW(PCWSTR(name.as_ptr()), PCWSTR::null(), None, DESKTOP_CONTROL_FLAGS(0), 0x1000_0000, None) }
            .context("CreateDesktop")?;
        let r = session(s, cfg, Placement::HiddenThread(desk.0 as isize));
        unsafe {
            let _ = CloseDesktop(desk);
        }
        r
    }

    fn parse_opts(v: &[String]) -> Result<BTreeMap<String, String>> {
        v.iter()
            .map(|kv| kv.split_once('=').map(|(k, v)| (k.to_string(), v.to_string())).with_context(|| format!("--reader-opt {kv:?} is not key=value")))
            .collect()
    }

    fn write_report(args: &Args, s: &Scenario, run: ScenarioRun, stats: LiveStats) -> Result<()> {
        let dir = args.report.clone().unwrap_or_else(|| crate::data_dir().join("reports").join(format!("live-{}-{}", s.id, args.reader)));
        std::fs::create_dir_all(dir.join("events"))?;
        std::fs::write(dir.join("events").join(format!("{}.json", s.id)), serde_json::to_string_pretty(&run)?)?;
        let bg = BgTextMap::for_background(&s.background, s.width, s.height);
        let scored = score::score_scenario(&s.id, &s.genres, &s.items, &run, &bg);
        let mut report = score::build_report(&format!("live:{}", s.id), &args.reader, "live", vec![scored]);
        report.reader_options = run.reader_options.clone();
        report.truth_fingerprint = score::suite_fingerprint(&[(s.id.clone(), run.truth_fingerprint.clone())]);
        std::fs::write(dir.join("report.json"), serde_json::to_string_pretty(&report)?)?;
        std::fs::write(dir.join("report.md"), score::report_markdown(&report) + &live_markdown(&stats))?;
        std::fs::write(dir.join("live.json"), serde_json::to_string_pretty(&stats)?)?;
        let m = &report.totals;
        println!(
            "live {} / {} / {}: recall {:.3} ({}/{}), CER {:.3}, dup {}, noise {}, background {}, halluc {}, item latency p50/p95 {:.0}/{:.0} ms",
            s.id, args.reader, stats.mode, m.recall, m.items_read, m.items_should_read, m.mean_cer, m.duplicates, m.noise_reads, m.background_reads, m.hallucinations, m.latency_p50_ms, m.latency_p95_ms
        );
        println!(
            "frames: scene {}, presented {} ({} late), captured {}, read {}, repeat {}, undecodable {}, overwritten {}",
            stats.frames_scene, stats.frames_presented, stats.frames_presented_late, stats.capture_callbacks, stats.frames_read, stats.captured_repeat, stats.captured_undecodable, stats.capture_overwritten
        );
        let p = |x: &Percentiles| format!("p50 {:.1} / p95 {:.1} / max {:.1} ms", x.p50, x.p95, x.max);
        println!("present->callback {}", p(&stats.present_to_callback_ms));
        println!("callback copy     {}", p(&stats.callback_copy_ms));
        println!("copy->reader      {}", p(&stats.copy_to_reader_ms));
        println!("present->WGC ts   {}", p(&stats.present_to_wgc_timestamp_ms));
        println!("reader on_frame    {}", p(&stats.reader_ms));
        println!("end to end        {}", p(&stats.end_to_end_ms));
        println!(
            "cpu: reader {:.0} ms ({:.1}% of a core), capture {:.0} ms ({:.1}%), process {:.0} ms ({:.1}%, includes the scene renderer) over {:.1} s",
            stats.reader_cpu_ms, stats.reader_cpu_pct_core, stats.capture_cpu_ms, stats.capture_cpu_pct_core, stats.process_cpu_ms, stats.process_cpu_pct_core, stats.wall_s
        );
        let worst = stats.pixel_checks.iter().map(|c| c.mean_abs_diff).fold(0.0, f64::max);
        println!("pixel check: {} frames, worst mean abs diff {:.2}", stats.pixel_checks.len(), worst);
        println!("foreground: never taken by arena; {} switches by other windows during the run", stats.foreground_switches_by_others.len());
        if !stats.invalid_reasons.is_empty() {
            println!("INVALID capture: {}", stats.invalid_reasons.join("; "));
        }
        println!("report: {}", dir.join("report.md").display());
        Ok(())
    }

    fn live_markdown(s: &LiveStats) -> String {
        let p = |x: &Percentiles| format!("{:.1} | {:.1} | {:.1} | {}", x.p50, x.p95, x.max, x.n);
        let mut md = String::from("\n## Live run\n\n");
        md += &format!("- Mode: {}, window at {:?}, captured {}x{}\n", s.mode, s.window_pos, s.captured_size.0, s.captured_size.1);
        if let Some(n) = &s.hidden_desktop_note {
            md += &format!("- Hidden desktop: {n}\n");
        }
        md += &format!(
            "- Frames: scene {}, presented {} ({} late), captured {}, read {}, repeat {}, undecodable {}, overwritten {}\n",
            s.frames_scene, s.frames_presented, s.frames_presented_late, s.capture_callbacks, s.frames_read, s.captured_repeat, s.captured_undecodable, s.capture_overwritten
        );
        md += "\n| Stage | p50 ms | p95 ms | max ms | n |\n|---|---|---|---|---|\n";
        md += &format!("| present to WGC callback | {} |\n", p(&s.present_to_callback_ms));
        md += &format!("| copy in callback | {} |\n", p(&s.callback_copy_ms));
        md += &format!("| copy to reader | {} |\n", p(&s.copy_to_reader_ms));
        md += &format!("| reader on_frame | {} |\n", p(&s.reader_ms));
        md += &format!("| end to end (present to reader done) | {} |\n", p(&s.end_to_end_ms));
        md += &format!("| present to WGC timestamp (reference) | {} |\n\n", p(&s.present_to_wgc_timestamp_ms));
        md += &format!(
            "- CPU over {:.1} s: reader thread {:.0} ms ({:.1}% of a core), capture thread {:.0} ms ({:.1}%), process {:.0} ms ({:.1}%, includes the scene renderer)\n",
            s.wall_s, s.reader_cpu_ms, s.reader_cpu_pct_core, s.capture_cpu_ms, s.capture_cpu_pct_core, s.process_cpu_ms, s.process_cpu_pct_core
        );
        md += &format!("- Foreground: never taken by arena; {} switches by other windows during the run\n", s.foreground_switches_by_others.len());
        for c in &s.pixel_checks {
            md += &format!("- Pixel check frame {}: mean abs diff {:.2}, {:.2}% of pixels off by more than 16\n", c.index, c.mean_abs_diff, c.share_off * 100.0);
        }
        md
    }

    // ---------- hidden desktop child ----------

    fn quote(a: &str) -> String {
        format!("\"{}\"", a.replace('"', "\\\""))
    }

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(Some(0)).collect()
    }

    /// Runs this session in a child `arena` process started on a new desktop.
    fn run_hidden_child(args: &Args, path: &Path, s: &Scenario, guard: &ForegroundGuard, child_pid: &Arc<AtomicU64>) -> Result<ChildResult> {
        let exe = std::env::current_exe()?;
        let out = std::env::temp_dir().join(format!("arena-live-{}-{}.json", std::process::id(), s.id));
        let _ = std::fs::remove_file(&out);
        let mut cmd = format!("{} live --scenario {} --reader {} --probe-ms {}", quote(&exe.to_string_lossy()), quote(&path.to_string_lossy()), quote(&args.reader), args.probe_ms);
        for o in &args.reader_opts {
            cmd += &format!(" --reader-opt {}", quote(o));
        }
        cmd += &format!(" --child-out {}", quote(&out.to_string_lossy()));

        let desk_name = format!("arena-live-{}", std::process::id());
        let desk_w = wide(&desk_name);
        // GENERIC_ALL, as in Prism's HiddenDesktop.cs.
        let desk = unsafe { CreateDesktopW(PCWSTR(desk_w.as_ptr()), PCWSTR::null(), None, DESKTOP_CONTROL_FLAGS(0), 0x1000_0000, None) }
            .context("CreateDesktop")?;
        let mut desk_buf = desk_w.clone();
        let mut cmd_w = wide(&cmd);
        let si = STARTUPINFOW { cb: size_of::<STARTUPINFOW>() as u32, lpDesktop: PWSTR(desk_buf.as_mut_ptr()), ..Default::default() };
        let mut pi = PROCESS_INFORMATION::default();
        let created = unsafe {
            windows::Win32::System::Threading::CreateProcessW(PCWSTR::null(), Some(PWSTR(cmd_w.as_mut_ptr())), None, None, false, CREATE_NO_WINDOW, None, PCWSTR::null(), &si, &mut pi)
        };
        if let Err(e) = created {
            unsafe {
                let _ = CloseDesktop(desk);
            }
            bail!("CreateProcess on the hidden desktop: {e}");
        }
        child_pid.store(pi.dwProcessId as u64, Ordering::SeqCst);
        let limit_ms = args.probe_ms as f64 + s.duration_ms + 30_000.0;
        let start = Instant::now();
        let mut aborted = None;
        loop {
            if unsafe { WaitForSingleObject(pi.hProcess, 100) } == WAIT_OBJECT_0 {
                break;
            }
            if let Some(m) = guard.aborted() {
                aborted = Some(format!("aborted, {m}"));
            } else if start.elapsed().as_secs_f64() * 1000.0 > limit_ms {
                aborted = Some(format!("child did not finish within {:.0} s", limit_ms / 1000.0));
            }
            if aborted.is_some() {
                unsafe {
                    let _ = TerminateProcess(pi.hProcess, 1);
                    let _ = WaitForSingleObject(pi.hProcess, 5000);
                }
                break;
            }
        }
        let mut code = 0u32;
        unsafe {
            let _ = GetExitCodeProcess(pi.hProcess, &mut code);
            let _ = CloseHandle(pi.hProcess);
            let _ = CloseHandle(pi.hThread);
            let _ = CloseDesktop(desk);
        }
        child_pid.store(0, Ordering::SeqCst);
        if let Some(m) = aborted {
            bail!("{m}");
        }
        let text = std::fs::read_to_string(&out).with_context(|| format!("child exited with code {code} and wrote no result"))?;
        let _ = std::fs::remove_file(&out);
        Ok(serde_json::from_str(&text)?)
    }

    // ---------- the session ----------

    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    enum Placement {
        /// (0, 0): used on the hidden desktop, where nobody sees it.
        Origin,
        /// Left of the whole virtual screen, outside every monitor.
        OffScreen,
        /// (0, 0) on a hidden desktop (HDESK as isize) that only the player thread switches to.
        HiddenThread(isize),
    }

    /// Settings of one session.
    struct Cfg<'a> {
        reader: &'a str,
        opts: &'a BTreeMap<String, String>,
        probe_ms: u64,
        /// Extra sleep after each reader call (testing frame dropping).
        delay_ms: f64,
        abort: Option<Arc<Mutex<Option<String>>>>,
    }

    /// Set by the window procedure when the player window gets an activation or focus message.
    static ACTIVATED: AtomicBool = AtomicBool::new(false);

    /// State shared by the player, the capture callback and the reader thread.
    struct Shared {
        /// Scene t = 0 in QPC ticks; 0 until the reader says go.
        t0: AtomicI64,
        /// Present time (ticks) of each scene frame, 0 when not presented.
        present: Vec<AtomicI64>,
        presented: AtomicUsize,
        late: AtomicUsize,
        player_done: AtomicBool,
        stop: AtomicBool,
        player_error: Mutex<Option<String>>,
        slot: Mutex<Option<Captured>>,
        cv: Condvar,
        callbacks: AtomicUsize,
        overwritten: AtomicUsize,
        /// CPU of the capture thread, 100 ns units.
        capture_cpu: AtomicU64,
    }

    struct Captured {
        /// WGC frame timestamp (planned composition time), ticks.
        ts: i64,
        /// Callback start and copy done, ticks.
        arrive: i64,
        copied: i64,
        w: u32,
        h: u32,
        rgba: Vec<u8>,
    }

    struct Grabber {
        shared: Arc<Shared>,
        cpu_base: Option<f64>,
    }

    impl GraphicsCaptureApiHandler for Grabber {
        type Flags = Arc<Shared>;
        type Error = Box<dyn std::error::Error + Send + Sync>;

        fn new(ctx: Context<Self::Flags>) -> Result<Self, Self::Error> {
            Ok(Grabber { shared: ctx.flags, cpu_base: None })
        }

        // Never returns Err for one bad frame: an Err ends the whole capture.
        fn on_frame_arrived(&mut self, frame: &mut Frame, _c: InternalCaptureControl) -> Result<(), Self::Error> {
            let arrive = now_ticks();
            let base = *self.cpu_base.get_or_insert_with(thread_cpu_ms);
            let Ok(ts) = frame.timestamp().map(|t| t.Duration) else {
                return Ok(());
            };
            let (w, h) = (frame.width(), frame.height());
            let mut tmp = Vec::new();
            let rgba = match frame.buffer() {
                Ok(fb) => fb.as_nopadding_buffer(&mut tmp).to_vec(),
                Err(_) => return Ok(()),
            };
            let copied = now_ticks();
            self.shared.callbacks.fetch_add(1, Ordering::Relaxed);
            {
                let mut slot = self.shared.slot.lock().unwrap();
                if slot.is_some() {
                    self.shared.overwritten.fetch_add(1, Ordering::Relaxed);
                }
                *slot = Some(Captured { ts, arrive, copied, w, h, rgba });
            }
            self.shared.cv.notify_one();
            let used = ((thread_cpu_ms() - base) * 10_000.0) as u64;
            self.shared.capture_cpu.store(used, Ordering::Relaxed);
            Ok(())
        }
    }

    /// Plays `s` in a window, captures it and feeds the reader on this thread.
    fn session(s: &Scenario, cfg: &Cfg, placement: Placement) -> Result<(ScenarioRun, LiveStats)> {
        let w = s.width;
        let n = s.frame_count();
        if n >= PROBE_INDEX as usize {
            bail!("scene has {n} frames; live mode codes at most {} frame indices", PROBE_INDEX);
        }
        if w < BLOCKS * BLOCK_W {
            bail!("scene is {w} px wide; live mode needs at least {}", BLOCKS * BLOCK_W);
        }
        // A flag left from an earlier session (auto mode tries several) must not abort this one.
        ACTIVATED.store(false, Ordering::SeqCst);
        let shared = Arc::new(Shared {
            t0: AtomicI64::new(0),
            present: (0..n).map(|_| AtomicI64::new(0)).collect(),
            presented: AtomicUsize::new(0),
            late: AtomicUsize::new(0),
            player_done: AtomicBool::new(false),
            stop: AtomicBool::new(false),
            player_error: Mutex::new(None),
            slot: Mutex::new(None),
            cv: Condvar::new(),
            callbacks: AtomicUsize::new(0),
            overwritten: AtomicUsize::new(0),
            capture_cpu: AtomicU64::new(0),
        });

        let (hwnd_tx, hwnd_rx) = std::sync::mpsc::channel::<Result<(isize, (i32, i32))>>();
        let player = {
            let shared = shared.clone();
            let s = s.clone();
            thread::spawn(move || {
                if let Err(e) = player_thread(&s, placement, &shared, &hwnd_tx) {
                    *shared.player_error.lock().unwrap() = Some(format!("{e:#}"));
                    let _ = hwnd_tx.send(Err(e));
                }
                shared.player_done.store(true, Ordering::SeqCst);
                shared.cv.notify_all();
            })
        };
        let stop_player = |shared: &Arc<Shared>| {
            shared.stop.store(true, Ordering::SeqCst);
        };

        let (hwnd, window_pos) = match hwnd_rx.recv_timeout(Duration::from_secs(10)) {
            Ok(Ok(v)) => v,
            Ok(Err(e)) => {
                let _ = player.join();
                return Err(e.context("player window"));
            }
            Err(_) => {
                stop_player(&shared);
                let _ = player.join();
                bail!("player window was not created within 10 s");
            }
        };

        let result = capture_and_read(s, cfg, &shared, hwnd, window_pos, placement);
        stop_player(&shared);
        let _ = player.join();
        let (run, mut stats) = result?;
        if let Some(e) = shared.player_error.lock().unwrap().clone() {
            bail!("player: {e}");
        }
        if ACTIVATED.load(Ordering::SeqCst) {
            bail!("live: aborted, the player window received an activation or focus message");
        }
        stats.frames_presented = shared.presented.load(Ordering::SeqCst);
        stats.frames_presented_late = shared.late.load(Ordering::SeqCst);
        Ok((run, stats))
    }

    fn capture_and_read(s: &Scenario, cfg: &Cfg, shared: &Arc<Shared>, hwnd: isize, window_pos: (i32, i32), placement: Placement) -> Result<(ScenarioRun, LiveStats)> {
        let n = s.frame_count();
        let settings = Settings::new(
            Window::from_raw_hwnd(hwnd as *mut std::ffi::c_void),
            CursorCaptureSettings::WithoutCursor,
            DrawBorderSettings::WithoutBorder,
            SecondaryWindowSettings::Default,
            MinimumUpdateIntervalSettings::Default,
            DirtyRegionSettings::Default,
            ColorFormat::Rgba8,
            shared.clone(),
        );
        let capture_start = now_ticks();
        let control = match Grabber::start_free_threaded(settings) {
            Ok(c) => c,
            Err(e) => return Err(NoFrames(format!("WGC could not start on the window: {e}")).into()),
        };
        let r = read_loop(s, cfg, shared, capture_start);
        let _ = control.stop();
        let (run, mut stats, checks_raw) = r?;

        // Pixel check after the run, so it costs nothing during it.
        let mut renderer = SceneRenderer::new(s.clone())?;
        let mut buf = Vec::new();
        for (i, cap) in checks_raw {
            renderer.render(s.frame_time_ms(i), &mut buf)?;
            stats.pixel_checks.push(pixel_check(i, &cap, &buf));
        }
        stats.mode = match placement {
            Placement::Origin => "hidden-desktop".into(),
            Placement::OffScreen => "offscreen".into(),
            Placement::HiddenThread(_) => "hidden-desktop-thread".into(),
        };
        stats.window_pos = window_pos;
        stats.frames_scene = n;
        Ok((run, stats))
    }

    type ReadOutcome = (ScenarioRun, LiveStats, Vec<(usize, Vec<u8>)>);

    fn take_frame(shared: &Shared, timeout: Duration) -> Option<Captured> {
        let slot = shared.slot.lock().unwrap();
        let (mut slot, _) = shared.cv.wait_timeout_while(slot, timeout, |s| s.is_none() && !shared.player_done.load(Ordering::SeqCst)).unwrap();
        slot.take()
    }

    /// Cuts the scene area out of a captured frame; `None` when the size is wrong.
    fn crop_scene(c: &Captured, w: u32, h: u32, out: &mut Vec<u8>) -> bool {
        if c.w < w || c.h < h + STRIP_H {
            return false;
        }
        out.clear();
        let row = (w * 4) as usize;
        for y in 0..h as usize {
            let o = y * c.w as usize * 4;
            out.extend_from_slice(&c.rgba[o..o + row]);
        }
        true
    }

    fn read_loop(s: &Scenario, cfg: &Cfg, shared: &Arc<Shared>, capture_start: i64) -> Result<ReadOutcome> {
        let (reader_name, probe_ms) = (cfg.reader, cfg.probe_ms);
        let (w, h) = (s.width, s.height);
        let n = s.frame_count();
        let check_abort = || -> Result<()> {
            if let Some(a) = &cfg.abort
                && let Some(m) = a.lock().unwrap().clone()
            {
                bail!("live: aborted, {m}");
            }
            Ok(())
        };

        // Probe: wait for a frame that shows the probe code.
        let probe_deadline = capture_start + ms_to_ticks(probe_ms as f64);
        let mut stats = LiveStats::default();
        let mut undecodable = 0usize;
        let mut last_seen = String::from("none");
        loop {
            check_abort()?;
            if let Some(e) = shared.player_error.lock().unwrap().clone() {
                bail!("player: {e}");
            }
            if now_ticks() > probe_deadline {
                let got = shared.callbacks.load(Ordering::SeqCst);
                let why = if got == 0 { "no frame callbacks".to_string() } else { format!("{got} callbacks, none with a valid code (last frame: {last_seen})") };
                return Err(NoFrames(format!("{why} within {probe_ms} ms")).into());
            }
            let Some(c) = take_frame(shared, Duration::from_millis(50)) else { continue };
            if decode_strip(&c.rgba, c.w, h) == Some(PROBE_INDEX) && c.w >= w && c.h >= h + STRIP_H {
                stats.captured_size = (c.w, c.h);
                stats.probe_ms = ticks_to_ms(now_ticks() - capture_start);
                break;
            }
            let luma = c.rgba.as_chunks::<4>().0.iter().map(|p| p[0] as u64 + p[1] as u64 + p[2] as u64).sum::<u64>() / (c.rgba.len() as u64 / 4 * 3).max(1);
            last_seen = format!("{}x{}, mean luma {luma}", c.w, c.h);
            undecodable += 1;
        }

        // The reader is built on this thread (readers need not be Send).
        let ctx = crate::runner::reader_context(&s.id, w, h, &s.layout, &s.items, cfg.opts);
        let mut reader = crate::readers::create(reader_name, &ctx)?;
        reader.setup(&s.layout);

        let cpu0 = thread_cpu_ms();
        let pcpu0 = process_cpu_ms();
        let t0 = now_ticks() + ms_to_ticks(300.0);
        let cb0 = shared.callbacks.load(Ordering::SeqCst);
        let ow0 = shared.overwritten.load(Ordering::SeqCst);
        shared.t0.store(t0, Ordering::SeqCst);

        let mut events = Vec::new();
        let mut costs = Vec::new();
        let (mut p2cb, mut copy, mut c2r, mut p2ts, mut e2e) = (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new());
        let mut last_index: Option<usize> = None;
        let mut repeat = 0usize;
        let mut crop = Vec::new();
        let mut checks: Vec<(usize, Vec<u8>)> = Vec::new();
        let check_every = (n / PIXEL_CHECKS).max(1);
        let hard_end = t0 + ms_to_ticks(s.duration_ms + 15_000.0);
        loop {
            check_abort()?;
            if now_ticks() > hard_end {
                bail!("live: run did not end in time");
            }
            let Some(c) = take_frame(shared, Duration::from_millis(50)) else {
                if shared.player_done.load(Ordering::SeqCst) {
                    break;
                }
                continue;
            };
            let Some(code) = decode_strip(&c.rgba, c.w, h) else {
                undecodable += 1;
                continue;
            };
            let i = code as usize;
            if code == PROBE_INDEX || i >= n {
                continue;
            }
            if last_index.is_some_and(|l| i <= l) {
                repeat += 1;
                continue;
            }
            if !crop_scene(&c, w, h, &mut crop) {
                bail!("captured frame is {}x{}, expected at least {}x{}", c.w, c.h, w, h + STRIP_H);
            }
            last_index = Some(i);
            let start = now_ticks();
            let frame = FrameRef { t_ms: s.frame_time_ms(i), width: w, height: h, rgba: &crop };
            let mut evs = reader.on_frame(&frame);
            if cfg.delay_ms > 0.0 {
                sleep_until(now_ticks() + ms_to_ticks(cfg.delay_ms));
            }
            let done = now_ticks();
            let present = shared.present[i].load(Ordering::SeqCst);
            costs.push(ticks_to_ms(done - start));
            copy.push(ticks_to_ms(c.copied - c.arrive));
            c2r.push(ticks_to_ms(start - c.copied));
            if present > 0 {
                p2cb.push(ticks_to_ms(c.arrive - present));
                p2ts.push(ticks_to_ms(c.ts - present));
                e2e.push(ticks_to_ms(done - present));
            }
            // Event time: wall time since scene t = 0 when the reader returned.
            let ev_t = ticks_to_ms(done - t0);
            for e in &mut evs {
                e.t_ms = ev_t;
            }
            events.extend(evs);
            if i.is_multiple_of(check_every) && checks.len() < PIXEL_CHECKS {
                checks.push((i, crop.clone()));
            }
        }
        let end = now_ticks();
        events.extend(reader.finish(ticks_to_ms(end - t0)));

        let wall_ms = ticks_to_ms(end - t0);
        stats.wall_s = wall_ms / 1000.0;
        stats.capture_callbacks = shared.callbacks.load(Ordering::SeqCst) - cb0;
        stats.capture_overwritten = shared.overwritten.load(Ordering::SeqCst) - ow0;
        stats.captured_undecodable = undecodable;
        stats.captured_repeat = repeat;
        stats.frames_read = costs.len();
        stats.present_to_callback_ms = Percentiles::of(&p2cb);
        stats.callback_copy_ms = Percentiles::of(&copy);
        stats.copy_to_reader_ms = Percentiles::of(&c2r);
        stats.present_to_wgc_timestamp_ms = Percentiles::of(&p2ts);
        stats.reader_ms = Percentiles::of(&costs);
        stats.end_to_end_ms = Percentiles::of(&e2e);
        let pct = |ms: f64| if wall_ms > 0.0 { ms / wall_ms * 100.0 } else { 0.0 };
        stats.reader_cpu_ms = thread_cpu_ms() - cpu0;
        stats.reader_cpu_pct_core = pct(stats.reader_cpu_ms);
        stats.capture_cpu_ms = shared.capture_cpu.load(Ordering::SeqCst) as f64 / 10_000.0;
        stats.capture_cpu_pct_core = pct(stats.capture_cpu_ms);
        stats.process_cpu_ms = process_cpu_ms() - pcpu0;
        stats.process_cpu_pct_core = pct(stats.process_cpu_ms);

        let run = ScenarioRun {
            scenario_id: s.id.clone(),
            reader: reader_name.to_string(),
            clock: Clock::Live,
            events,
            frame_costs_ms: costs,
            frames_total: n,
            frames_processed: stats.frames_read,
            truth_fingerprint: crate::model::truth_fingerprint(&s.items),
            reader_options: cfg.opts.clone(),
        };
        Ok((run, stats, checks))
    }

    // ---------- player window ----------

    unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
        match msg {
            WM_MOUSEACTIVATE => LRESULT(MA_NOACTIVATE as isize),
            WM_SETFOCUS => {
                ACTIVATED.store(true, Ordering::SeqCst);
                LRESULT(0)
            }
            WM_ACTIVATE | WM_NCACTIVATE if wp.0 & 0xFFFF != 0 => {
                ACTIVATED.store(true, Ordering::SeqCst);
                unsafe { DefWindowProcW(hwnd, msg, wp, lp) }
            }
            WM_ERASEBKGND => LRESULT(1),
            _ => unsafe { DefWindowProcW(hwnd, msg, wp, lp) },
        }
    }

    fn pump() {
        let mut msg = MSG::default();
        unsafe {
            while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
    }

    /// RGBA scene + strip, converted to BGRA for GDI.
    fn compose(scene: &[u8], w: u32, h: u32, index: u16, out: &mut Vec<u8>) {
        out.resize((w * (h + STRIP_H) * 4) as usize, 0);
        for (d, s) in out.as_chunks_mut::<4>().0.iter_mut().zip(scene.as_chunks::<4>().0) {
            d.copy_from_slice(&[s[2], s[1], s[0], 255]);
        }
        encode_strip(out, w, h, index);
    }

    /// A layered window's picture: a 32-bit DIB section pushed with UpdateLayeredWindow.
    /// Plain GDI drawing (GetDC + SetDIBitsToDevice) is clipped away when the window is outside
    /// every monitor, so WGC only ever saw a black frame; a layered window's content is not.
    struct Surface {
        dc: HDC,
        bmp: HBITMAP,
        old: HGDIOBJ,
        bits: *mut u8,
        w: u32,
        hh: u32,
    }

    impl Surface {
        fn new(w: u32, hh: u32) -> Result<Self> {
            let bmi = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: w as i32,
                    biHeight: -(hh as i32),
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    ..Default::default()
                },
                ..Default::default()
            };
            unsafe {
                let dc = CreateCompatibleDC(None);
                let mut bits: *mut std::ffi::c_void = std::ptr::null_mut();
                let bmp = match CreateDIBSection(Some(dc), &bmi, DIB_RGB_COLORS, &mut bits, None, 0) {
                    Ok(b) => b,
                    Err(e) => {
                        let _ = DeleteDC(dc);
                        return Err(e).context("CreateDIBSection");
                    }
                };
                let old = SelectObject(dc, bmp.into());
                Ok(Surface { dc, bmp, old, bits: bits as *mut u8, w, hh })
            }
        }

        /// Shows a BGRA frame (alpha 255) in the window.
        fn show(&mut self, hwnd: HWND, bgra: &[u8]) -> Result<()> {
            let len = (self.w * self.hh * 4) as usize;
            unsafe {
                std::ptr::copy_nonoverlapping(bgra.as_ptr(), self.bits, len.min(bgra.len()));
                let _ = GdiFlush();
                let size = SIZE { cx: self.w as i32, cy: self.hh as i32 };
                let src = POINT { x: 0, y: 0 };
                let blend = BLENDFUNCTION { BlendOp: AC_SRC_OVER as u8, BlendFlags: 0, SourceConstantAlpha: 255, AlphaFormat: AC_SRC_ALPHA as u8 };
                UpdateLayeredWindow(hwnd, None, None, Some(&size), Some(self.dc), Some(&src), COLORREF(0), Some(&blend), ULW_ALPHA)
                    .context("UpdateLayeredWindow")
            }
        }
    }

    impl Drop for Surface {
        fn drop(&mut self) {
            unsafe {
                SelectObject(self.dc, self.old);
                let _ = DeleteObject(self.bmp.into());
                let _ = DeleteDC(self.dc);
            }
        }
    }

    fn player_thread(s: &Scenario, placement: Placement, shared: &Shared, hwnd_tx: &std::sync::mpsc::Sender<Result<(isize, (i32, i32))>>) -> Result<()> {
        let (w, h) = (s.width, s.height);
        let hh = h + STRIP_H;
        if let Placement::HiddenThread(desk) = placement {
            // Must happen before this thread owns any window.
            unsafe { SetThreadDesktop(HDESK(desk as *mut std::ffi::c_void)) }.context("SetThreadDesktop")?;
        }
        unsafe {
            // Pixel sizes must be exact, whatever the display scaling.
            let _ = SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        }
        let mut renderer = SceneRenderer::new(s.clone())?;
        let mut scene = Vec::new();
        let mut out = Vec::new();
        renderer.render(0.0, &mut scene)?;
        compose(&scene, w, h, PROBE_INDEX, &mut out);

        let pos = match placement {
            Placement::Origin | Placement::HiddenThread(_) => (0, 0),
            Placement::OffScreen => unsafe {
                let vx = GetSystemMetrics(SM_XVIRTUALSCREEN);
                let vy = GetSystemMetrics(SM_YVIRTUALSCREEN);
                (vx - w as i32 - 200, vy)
            },
        };
        let hwnd = unsafe {
            let inst = GetModuleHandleW(None)?;
            let class = w!("ArenaLivePlayer");
            let wc = WNDCLASSW { lpfnWndProc: Some(wndproc), hInstance: inst.into(), lpszClassName: class, ..Default::default() };
            RegisterClassW(&wc); // 0 when already registered by an earlier session; fine.
            CreateWindowExW(
                WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW | WS_EX_LAYERED,
                class,
                w!("arena live"),
                WS_POPUP,
                pos.0,
                pos.1,
                w as i32,
                hh as i32,
                None,
                None,
                Some(inst.into()),
                None,
            )?
        };
        let mut surface = match Surface::new(w, hh) {
            Ok(v) => v,
            Err(e) => {
                unsafe {
                    let _ = DestroyWindow(hwnd);
                }
                return Err(e);
            }
        };
        // Content first, then show: a layered window has no picture until UpdateLayeredWindow.
        if let Err(e) = surface.show(hwnd, &out) {
            unsafe {
                let _ = DestroyWindow(hwnd);
            }
            return Err(e);
        }
        unsafe {
            let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        }
        pump();
        let _ = hwnd_tx.send(Ok((hwnd.0 as isize, pos)));

        let r = play_frames(s, &mut renderer, shared, hwnd, &mut surface, &mut scene, &mut out);
        drop(surface);
        unsafe {
            let _ = DestroyWindow(hwnd);
        }
        pump();
        r
    }

    fn play_frames(s: &Scenario, renderer: &mut SceneRenderer, shared: &Shared, hwnd: HWND, surface: &mut Surface, scene: &mut Vec<u8>, out: &mut Vec<u8>) -> Result<()> {
        let (w, h) = (s.width, s.height);
        let n = s.frame_count();
        // Until go: show the probe frame, refreshed so WGC has something to send.
        let mut last_probe = 0i64;
        let t0 = loop {
            if shared.stop.load(Ordering::SeqCst) {
                return Ok(());
            }
            let t0 = shared.t0.load(Ordering::SeqCst);
            if t0 != 0 {
                break t0;
            }
            pump();
            let now = now_ticks();
            if now - last_probe > ms_to_ticks(100.0) {
                surface.show(hwnd, out)?;
                last_probe = now;
            }
            thread::sleep(Duration::from_millis(5));
        };

        let frame_ticks = ms_to_ticks(1000.0 / s.fps);
        let mut i = 0usize;
        while i < n {
            if shared.stop.load(Ordering::SeqCst) {
                return Ok(());
            }
            // Render ahead, then wait for the due time, so present happens on time.
            renderer.render(s.frame_time_ms(i), scene)?;
            compose(scene, w, h, i as u16, out);
            let due = t0 + ms_to_ticks(s.frame_time_ms(i));
            let now = now_ticks();
            if now < due {
                pump();
                sleep_until(due);
            }
            surface.show(hwnd, out)?;
            let presented = now_ticks();
            shared.present[i].store(presented, Ordering::SeqCst);
            shared.presented.fetch_add(1, Ordering::SeqCst);
            if presented - due > frame_ticks {
                shared.late.fetch_add(1, Ordering::SeqCst);
            }
            pump();
            // When late, jump to the frame that is due now, like a game that drops frames.
            let elapsed_ms = ticks_to_ms(now_ticks() - t0);
            let due_now = (elapsed_ms * s.fps / 1000.0).floor() as usize;
            i = (i + 1).max(due_now.min(n));
            if i >= n && presented < t0 + ms_to_ticks(s.duration_ms) {
                sleep_until(t0 + ms_to_ticks(s.duration_ms));
            }
        }
        // Hold the last frame so the capture can catch it.
        let hold_end = now_ticks() + ms_to_ticks(HOLD_MS);
        while now_ticks() < hold_end && !shared.stop.load(Ordering::SeqCst) {
            pump();
            thread::sleep(Duration::from_millis(10));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(w: u32, h: u32) -> Vec<u8> {
        vec![40u8; (w * (h + STRIP_H) * 4) as usize]
    }

    #[test]
    fn strip_round_trips() {
        let (w, h) = (320, 10);
        for index in [0u16, 1, 2, 29, 255, 1000, 12345, 65534, PROBE_INDEX] {
            let mut f = frame(w, h);
            encode_strip(&mut f, w, h, index);
            assert_eq!(decode_strip(&f, w, h), Some(index), "index {index}");
        }
    }

    #[test]
    fn strip_rejects_garbage() {
        let (w, h) = (320, 10);
        // Mid grey: no block is clearly black or white.
        let f = frame(w, h);
        assert_eq!(decode_strip(&f, w, h), None);
        // All white: sync block 1 must be black.
        let white = vec![255u8; (w * (h + STRIP_H) * 4) as usize];
        assert_eq!(decode_strip(&white, w, h), None);
        // Too narrow for the code.
        assert_eq!(decode_strip(&white, 100, h), None);
    }

    #[test]
    fn strip_parity_catches_a_flipped_bit() {
        let (w, h) = (320, 10);
        let mut f = frame(w, h);
        encode_strip(&mut f, w, h, 77);
        // Flip data bit block 10 in the sampled row.
        let y = h + STRIP_H / 2;
        let x = 10 * BLOCK_W + BLOCK_W / 2;
        let o = ((y * w + x) * 4) as usize;
        let v = if f[o] > 128 { 0 } else { 255 };
        f[o..o + 3].copy_from_slice(&[v, v, v]);
        assert_eq!(decode_strip(&f, w, h), None);
    }

    #[test]
    fn strip_decodes_inside_a_wider_capture() {
        // Capture can be wider than the scene: decode uses the capture stride.
        let (w, h) = (320, 10);
        let mut scene = frame(w, h);
        encode_strip(&mut scene, w, h, 4242);
        let cw = 400u32;
        let mut cap = vec![0u8; (cw * (h + STRIP_H) * 4) as usize];
        for y in 0..h + STRIP_H {
            let src = (y * w * 4) as usize;
            let dst = (y * cw * 4) as usize;
            cap[dst..dst + (w * 4) as usize].copy_from_slice(&scene[src..src + (w * 4) as usize]);
        }
        assert_eq!(decode_strip(&cap, cw, h), Some(4242));
    }

    #[test]
    fn pixel_check_counts_differences() {
        let a = vec![100u8; 4 * 10];
        let mut b = a.clone();
        assert_eq!(pixel_check(0, &a, &b).mean_abs_diff, 0.0);
        b[0] = 130; // one pixel, one channel off by 30
        let c = pixel_check(0, &a, &b);
        assert!((c.mean_abs_diff - 1.0).abs() < 1e-9);
        assert!((c.share_off - 0.1).abs() < 1e-9);
    }

    #[test]
    fn percentiles_of_empty_is_zero() {
        let p = Percentiles::of(&[]);
        assert_eq!((p.n, p.p50, p.max), (0, 0.0, 0.0));
        let p = Percentiles::of(&[1.0, 2.0, 3.0, 10.0]);
        assert_eq!((p.p50, p.max), (2.0, 10.0));
    }

    #[test]
    fn validity_gate() {
        let good = LiveStats {
            frames_scene: 100,
            frames_presented: 100,
            capture_callbacks: 100,
            frames_read: 40,
            pixel_checks: vec![PixelCheck { index: 1, mean_abs_diff: 0.0, share_off: 0.0 }],
            ..Default::default()
        };
        assert!(validity_problems(&good).is_empty(), "{:?}", validity_problems(&good));
        let mut bad = good.clone();
        bad.pixel_checks[0].mean_abs_diff = 9.0;
        assert_eq!(validity_problems(&bad).len(), 1);
        let mut bad = good.clone();
        bad.captured_undecodable = 30;
        assert_eq!(validity_problems(&bad).len(), 2);
        let mut bad = good;
        bad.pixel_checks.clear();
        bad.frames_read = 0;
        assert_eq!(validity_problems(&bad).len(), 2);
    }

    /// The `--child-out` guard reads this name; tests run on the user's desktop.
    #[cfg(windows)]
    #[test]
    fn this_thread_is_on_the_default_desktop() {
        assert_eq!(win::thread_desktop_name().unwrap(), "Default");
    }
}
