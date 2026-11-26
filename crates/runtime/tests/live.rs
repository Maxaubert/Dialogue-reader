//! The live view against a window that keeps changing, left of every monitor (Windows Graphics Capture does not
//! capture windows on a hidden desktop on this PC, docs/arena-spec.md). Nothing shows on the user's screen.

#![cfg(windows)]

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use dr_runtime::live::{INTERVAL, Live, LiveFrame, LiveSink};
use dr_runtime::testwin::{Kind, TestWindow};

#[derive(Default)]
struct Got {
    /// (src_w, src_h, w, h, bytes ok, when)
    frames: Vec<(u32, u32, u32, u32, bool, Instant)>,
    ended: Option<String>,
}

struct Sink(Arc<Mutex<Got>>);

impl LiveSink for Sink {
    fn frame(&mut self, f: &LiveFrame) {
        let ok = f.rgba.len() == (f.w * f.h * 4) as usize && f.rgba.chunks(4).all(|p| p[3] == 255);
        self.0
            .lock()
            .unwrap()
            .frames
            .push((f.src_w, f.src_h, f.w, f.h, ok, Instant::now()));
    }
    fn ended(&mut self, why: &str) {
        self.0.lock().unwrap().ended = Some(why.to_string());
    }
}

fn moving(w: u32, h: u32) -> TestWindow {
    TestWindow::spawn(Kind::OffscreenMoving { w, h, every_ms: 20 }, None)
        .expect("moving off-screen window")
}

fn wait_until(limit: Duration, mut f: impl FnMut() -> bool) -> bool {
    let end = Instant::now() + limit;
    while Instant::now() < end {
        if f() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    f()
}

#[test]
fn frames_arrive_small_and_spaced_then_stop() {
    let win = moving(640, 360);
    let got = Arc::new(Mutex::new(Got::default()));
    let live = Live::start(win.hwnd, 320, 320, INTERVAL, Box::new(Sink(got.clone()))).unwrap();
    std::thread::sleep(Duration::from_millis(1500));
    let n = got.lock().unwrap().frames.len();
    assert!(n >= 8, "only {n} frames in 1.5 s");
    assert!(n <= 26, "{n} frames in 1.5 s: the interval is not kept");
    {
        let g = got.lock().unwrap();
        for f in &g.frames {
            assert_eq!((f.0, f.1, f.2, f.3), (640, 360, 320, 180));
            assert!(f.4, "tight RGBA, alpha 255");
        }
        // spaced about INTERVAL apart (WGC may send a little early or late)
        let gaps: Vec<_> = g.frames.windows(2).map(|p| p[1].5 - p[0].5).collect();
        let min = gaps.iter().min().unwrap();
        assert!(*min >= Duration::from_millis(45), "gap {min:?}");
    }
    // a smaller stage: the next frames are smaller
    live.set_target(160, 160);
    assert!(wait_until(Duration::from_secs(2), || {
        got.lock()
            .unwrap()
            .frames
            .last()
            .is_some_and(|f| (f.2, f.3) == (160, 90))
    }));
    let s = live.stats();
    assert!(s.frames >= n as u64);
    live.stop();
    let after = got.lock().unwrap().frames.len();
    std::thread::sleep(Duration::from_millis(500));
    assert_eq!(
        got.lock().unwrap().frames.len(),
        after,
        "no frame after stop"
    );
    assert!(got.lock().unwrap().ended.is_none());
    drop(win);
}

#[test]
fn a_window_that_changes_size_gives_its_new_size() {
    let win = moving(640, 360);
    let got = Arc::new(Mutex::new(Got::default()));
    let live = Live::start(win.hwnd, 2000, 2000, INTERVAL, Box::new(Sink(got.clone()))).unwrap();
    assert!(wait_until(Duration::from_secs(3), || !got
        .lock()
        .unwrap()
        .frames
        .is_empty()));
    win.resize(800, 600);
    assert!(
        wait_until(Duration::from_secs(3), || {
            got.lock()
                .unwrap()
                .frames
                .last()
                .is_some_and(|f| (f.0, f.1, f.2, f.3) == (800, 600, 800, 600))
        }),
        "last {:?}",
        got.lock().unwrap().frames.last().map(|f| (f.0, f.1))
    );
    live.stop();
}

#[test]
fn a_closed_window_ends_the_live_view() {
    let win = moving(320, 200);
    let got = Arc::new(Mutex::new(Got::default()));
    let live = Live::start(win.hwnd, 320, 200, INTERVAL, Box::new(Sink(got.clone()))).unwrap();
    assert!(wait_until(Duration::from_secs(3), || !got
        .lock()
        .unwrap()
        .frames
        .is_empty()));
    drop(win);
    assert!(
        wait_until(Duration::from_secs(3), || got
            .lock()
            .unwrap()
            .ended
            .is_some()),
        "no end after the window closed"
    );
    drop(live);
}

#[test]
fn a_gone_window_does_not_start() {
    let win = moving(64, 64);
    let hwnd = win.hwnd;
    drop(win);
    let got = Arc::new(Mutex::new(Got::default()));
    assert!(Live::start(hwnd, 64, 64, INTERVAL, Box::new(Sink(got))).is_err());
}

#[test]
fn the_start_returns_at_once_and_a_stop_during_the_start_ends_it() {
    // the session is built on a helper thread: the caller (the UI thread) does not wait for Windows
    let win = moving(640, 360);
    for _ in 0..5 {
        let got = Arc::new(Mutex::new(Got::default()));
        let t = Instant::now();
        let live = Live::start(win.hwnd, 320, 320, INTERVAL, Box::new(Sink(got.clone()))).unwrap();
        let took = t.elapsed();
        assert!(took < Duration::from_millis(20), "start took {took:?}");
        // stopped before the session was built: it is stopped when it is, and nothing reaches the sink
        live.stop();
        std::thread::sleep(Duration::from_millis(600));
        let g = got.lock().unwrap();
        assert!(g.frames.is_empty(), "{} frames after stop", g.frames.len());
        assert!(g.ended.is_none());
    }
}

#[test]
fn a_window_that_goes_during_the_start_ends_the_session() {
    // the start runs on a helper thread: a window that closes meanwhile (Windows refuses the start, or the
    // session starts and then sees the close) must still reach the sink as an end, or the app never retries
    for _ in 0..3 {
        let win = moving(320, 200);
        let got = Arc::new(Mutex::new(Got::default()));
        let live = Live::start(win.hwnd, 320, 200, INTERVAL, Box::new(Sink(got.clone()))).unwrap();
        drop(win);
        assert!(
            wait_until(Duration::from_secs(3), || got
                .lock()
                .unwrap()
                .ended
                .is_some()),
            "no end after the window went during the start"
        );
        assert!(wait_until(Duration::from_secs(3), || live.is_finished()));
        drop(live);
    }
}

/// The band of fine stripes in [`quarters`]: (x0, y0, x1, y1) as parts of the picture.
const BARS: (f32, f32, f32, f32) = (0.6, 0.6, 0.8, 0.8);

/// A static picture with four colored quarters (red, green / blue, white), so up, down, left and right show,
/// and a band of 8 px black and white stripes in the white quarter, so a blurry scale shows.
fn quarters(w: u32, h: u32) -> Vec<u8> {
    let mut v = Vec::with_capacity((w * h * 4) as usize);
    let (bx0, by0, bx1, by1) = (
        (w as f32 * BARS.0) as u32,
        (h as f32 * BARS.1) as u32,
        (w as f32 * BARS.2) as u32,
        (h as f32 * BARS.3) as u32,
    );
    for y in 0..h {
        for x in 0..w {
            if (bx0..bx1).contains(&x) && (by0..by1).contains(&y) {
                v.extend_from_slice(if (x / 8) % 2 == 0 {
                    &[0, 0, 0, 255]
                } else {
                    &[255, 255, 255, 255]
                });
                continue;
            }
            v.extend_from_slice(match (x < w / 2, y < h / 2) {
                (true, true) => &[220, 30, 30, 255],
                (false, true) => &[30, 200, 40, 255],
                (true, false) => &[40, 50, 210, 255],
                (false, false) => &[240, 240, 240, 255],
            });
        }
    }
    v
}

#[derive(Default)]
struct Last(Option<(u32, u32, Vec<u8>)>);

struct Keep(Arc<Mutex<Last>>);

impl LiveSink for Keep {
    fn frame(&mut self, f: &LiveFrame) {
        self.0.lock().unwrap().0 = Some((f.w, f.h, f.rgba.to_vec()));
    }
    fn ended(&mut self, _why: &str) {}
}

/// The first frame of `hwnd` made to fit `max_w` x `max_h`, on the GPU or the CPU path, and how many frames
/// took the GPU path.
fn one_frame(hwnd: isize, max_w: u32, max_h: u32, cpu_only: bool) -> (u32, u32, Vec<u8>, u64) {
    let got = Arc::new(Mutex::new(Last::default()));
    let live = Live::start_with(
        hwnd,
        max_w,
        max_h,
        INTERVAL,
        cpu_only,
        Box::new(Keep(got.clone())),
    )
    .unwrap();
    assert!(
        wait_until(Duration::from_secs(5), || got.lock().unwrap().0.is_some()),
        "no frame (cpu_only {cpu_only})"
    );
    let gpu = live.stats().gpu_frames;
    live.stop();
    let (w, h, rgba) = got.lock().unwrap().0.take().unwrap();
    (w, h, rgba, gpu)
}

#[test]
fn the_gpu_scale_matches_the_cpu_box_filter() {
    // a ratio of about 0.65: the case the old mip-only path left to the CPU
    let (sw, sh) = (1280u32, 720u32);
    let win = TestWindow::spawn(
        Kind::Offscreen {
            w: sw,
            h: sh,
            rgba: quarters(sw, sh),
        },
        None,
    )
    .unwrap();
    let (gw, gh, gpu, n_gpu) = one_frame(win.hwnd, 832, 832, false);
    let (cw, ch, cpu, n_cpu) = one_frame(win.hwnd, 832, 832, true);
    assert_eq!((gw, gh), (832, 468), "exactly the fitted size");
    assert_eq!((cw, ch), (832, 468));
    assert!(n_gpu >= 1, "the GPU path ran");
    assert_eq!(n_cpu, 0, "cpu_only skips the GPU");
    assert!(gpu.chunks(4).all(|p| p[3] == 255), "alpha 255");
    // inside the stripes, by how much each pixel differs from their mean (red channel): the GPU picture must be
    // about as sharp as the CPU box filter (a mip level too small would wash the stripes out to grey)
    let (bx0, by0, bx1, by1) = (
        (gw as f32 * BARS.0) as u32 + 2,
        (gh as f32 * BARS.1) as u32 + 2,
        (gw as f32 * BARS.2) as u32 - 2,
        (gh as f32 * BARS.3) as u32 - 2,
    );
    let in_bars = |x: u32, y: u32| (bx0..bx1).contains(&x) && (by0..by1).contains(&y);
    let contrast = |img: &[u8]| {
        let v: Vec<f64> = (by0..by1)
            .flat_map(|y| (bx0..bx1).map(move |x| (x, y)))
            .map(|(x, y)| img[((y * gw + x) * 4) as usize] as f64)
            .collect();
        let m = v.iter().sum::<f64>() / v.len() as f64;
        v.iter().map(|p| (p - m).abs()).sum::<f64>() / v.len() as f64
    };
    let (cg, cc) = (contrast(&gpu), contrast(&cpu));
    assert!(cc > 60.0, "the CPU picture keeps the stripes ({cc:.1})");
    assert!(cg > 0.75 * cc, "GPU stripes {cg:.1}, CPU stripes {cc:.1}");
    // outside the stripes the same picture: the mean difference is small (only the edges between the quarters
    // differ a little)
    let (mut diff, mut n) = (0u64, 0u64);
    for y in 0..gh {
        for x in 0..gw {
            if in_bars(x, y) || (x + 4 > bx0 && x < bx1 + 4 && y + 4 > by0 && y < by1 + 4) {
                continue;
            }
            let o = ((y * gw + x) * 4) as usize;
            for c in 0..3 {
                diff += (gpu[o + c] as i32 - cpu[o + c] as i32).unsigned_abs() as u64;
            }
            n += 3;
        }
    }
    let mean = diff as f64 / n as f64;
    assert!(mean < 3.0, "mean difference {mean:.2}");
    // the right way up and round: each corner has its quarter's color
    let px = |x: u32, y: u32| {
        let o = ((y * gw + x) * 4) as usize;
        [gpu[o], gpu[o + 1], gpu[o + 2]]
    };
    let close = |a: [u8; 3], b: [u8; 3]| {
        a.iter()
            .zip(b)
            .all(|(x, y)| (*x as i32 - y as i32).abs() < 12)
    };
    assert!(
        close(px(10, 10), [220, 30, 30]),
        "top left {:?}",
        px(10, 10)
    );
    assert!(
        close(px(gw - 10, 10), [30, 200, 40]),
        "top right {:?}",
        px(gw - 10, 10)
    );
    assert!(
        close(px(10, gh - 10), [40, 50, 210]),
        "bottom left {:?}",
        px(10, gh - 10)
    );
    assert!(
        close(px(gw - 10, gh - 10), [240, 240, 240]),
        "bottom right {:?}",
        px(gw - 10, gh - 10)
    );
    // no scale (a stage bigger than the capture): the picture as it is
    let (w, h, same, _) = one_frame(win.hwnd, 4000, 4000, false);
    assert_eq!((w, h), (sw, sh));
    assert!(same == quarters(sw, sh), "the unscaled picture differs");
}

/// CPU of this process, user + kernel, without the thread `skip_tid` (the test window, which paints 33 MB each
/// step and would hide the live view's cost).
fn process_cpu(skip_tid: u32) -> Duration {
    use windows::Win32::Foundation::{CloseHandle, FILETIME};
    use windows::Win32::System::Threading::{
        GetCurrentProcess, GetProcessTimes, GetThreadTimes, OpenThread,
        THREAD_QUERY_LIMITED_INFORMATION,
    };
    let t = |f: FILETIME| ((f.dwHighDateTime as u64) << 32 | f.dwLowDateTime as u64) * 100;
    let (mut a, mut b, mut k, mut u) = (
        FILETIME::default(),
        FILETIME::default(),
        FILETIME::default(),
        FILETIME::default(),
    );
    unsafe { GetProcessTimes(GetCurrentProcess(), &mut a, &mut b, &mut k, &mut u).unwrap() };
    let all = t(k) + t(u);
    let (mut a, mut b, mut k, mut u) = (
        FILETIME::default(),
        FILETIME::default(),
        FILETIME::default(),
        FILETIME::default(),
    );
    unsafe {
        let h = OpenThread(THREAD_QUERY_LIMITED_INFORMATION, false, skip_tid).unwrap();
        GetThreadTimes(h, &mut a, &mut b, &mut k, &mut u).unwrap();
        let _ = CloseHandle(h);
    }
    Duration::from_nanos(all - (t(k) + t(u)))
}

/// The live view's CPU while a `src_w` x `src_h` window changes all the time, for each stage size, on the GPU
/// path and the CPU path. Prints one line per row of the table in crates/app/README.md.
fn cost(src_w: u32, src_h: u32, stages: &[(u32, u32)], secs: u64) {
    let win = TestWindow::spawn(
        Kind::OffscreenMoving {
            w: src_w,
            h: src_h,
            every_ms: 30,
        },
        None,
    )
    .unwrap();
    std::thread::sleep(Duration::from_secs(1));
    // the process without live view (the test window's own thread is left out of every number)
    let c0 = process_cpu(win.thread_id);
    let t0 = Instant::now();
    std::thread::sleep(Duration::from_secs(secs));
    let base = (process_cpu(win.thread_id) - c0).as_secs_f64() / t0.elapsed().as_secs_f64();
    for &(mw, mh) in stages {
        for cpu_only in [false, true] {
            let got = Arc::new(Mutex::new(Got::default()));
            let t = Instant::now();
            let live = Live::start_with(
                win.hwnd,
                mw,
                mh,
                INTERVAL,
                cpu_only,
                Box::new(Sink(got.clone())),
            )
            .unwrap();
            let start = t.elapsed();
            std::thread::sleep(Duration::from_secs(1));
            let s0 = live.stats();
            let c0 = process_cpu(win.thread_id);
            let t0 = Instant::now();
            std::thread::sleep(Duration::from_secs(secs));
            let wall = t0.elapsed().as_secs_f64();
            let cpu = (process_cpu(win.thread_id) - c0).as_secs_f64() / wall;
            let s = live.stats();
            let n = (s.frames - s0.frames).max(1) as f64;
            let ms = |d: Duration| d.as_secs_f64() * 1000.0 / n;
            let size = got.lock().unwrap().frames.last().map(|f| (f.2, f.3));
            let t = Instant::now();
            live.stop();
            let stop = t.elapsed();
            println!(
                "{src_w}x{src_h} -> {mw}x{mh} {}: frame {size:?}, {:.1} fps, gpu frames {}, readback {:.2} ms, \
                 scale/copy {:.2} ms, sink {:.2} ms per frame; live adds {:.1}% of one core \
                 ({:.1}% with, {:.1}% without); start {:.2} ms, stop {:.2} ms on the caller",
                if cpu_only { "CPU" } else { "GPU" },
                n / wall,
                s.gpu_frames - s0.gpu_frames,
                ms(s.readback - s0.readback),
                ms(s.scale - s0.scale),
                ms(s.sink - s0.sink),
                (cpu - base) * 100.0,
                cpu * 100.0,
                base * 100.0,
                start.as_secs_f64() * 1000.0,
                stop.as_secs_f64() * 1000.0,
            );
        }
    }
    drop(win);
}

/// The cost numbers in crates/app/README.md: a 3840 x 2160 and a 2560 x 1440 window that change all the time,
/// made small for the stages of pages 03, 10 and 11 at 225 % and 150 %, plus the window list the app runs every
/// few seconds while the game is closed. Run by hand (about 2 minutes):
/// `cargo test --release -p dr-runtime --test live -- --ignored --nocapture live_cost_4k`
#[test]
#[ignore = "a measurement, about 2 minutes"]
fn live_cost_4k() {
    println!(
        "{} cores",
        std::thread::available_parallelism().map_or(0, |n| n.get())
    );
    // page 03 at 225 %, Overview at 225 %, Zones (1440 x 900 window) at 225 %, Zones maximized at 225 %,
    // Overview at 150 %, page 03 at 100 %
    cost(
        3840,
        2160,
        &[
            (1962, 1102),
            (2516, 1415),
            (2286, 1286),
            (2880, 1620),
            (1677, 944),
            (872, 490),
        ],
        5,
    );
    cost(2560, 1440, &[(1962, 1102), (2516, 1415)], 5);
    // the window list without icons (Desktop::windows_quick), as the app runs it on the UI thread
    let mut o = dr_runtime::windows_list::ListOpts::for_app(vec!["TestGame.exe".into()]);
    o.icons = false;
    let mut times: Vec<Duration> = (0..20)
        .map(|_| {
            let t = Instant::now();
            std::hint::black_box(dr_runtime::windows_list::list(&o).len());
            t.elapsed()
        })
        .collect();
    times.sort();
    println!(
        "window list without icons: median {:.2} ms, worst {:.2} ms",
        times[10].as_secs_f64() * 1000.0,
        times[19].as_secs_f64() * 1000.0
    );
}

#[test]
fn one_session_serves_the_reader_and_an_attached_live_view() {
    use dr_runtime::capture::{FrameSource, Next, WgcSource};
    let win = moving(640, 360);
    let mut src = WgcSource::start(win.hwnd).unwrap();
    let got = Arc::new(Mutex::new(Got::default()));
    let attached = src
        .attacher()
        .attach(160, 160, INTERVAL, Box::new(Sink(got.clone())))
        .expect("the reader's session runs");
    // full-size frames for the reader, and small ones for the live view, from the same captured frames
    let end = Instant::now() + Duration::from_secs(2);
    let mut full = 0;
    while Instant::now() < end {
        if let Next::Frame(f) = src.next(Duration::from_millis(100)) {
            assert_eq!((f.w, f.h), (640, 360));
            assert_eq!(f.rgba.len(), 640 * 360 * 4);
            full += 1;
            src.recycle(f.rgba);
        }
    }
    assert!(full >= 15, "only {full} full frames in 2 s");
    let n = got.lock().unwrap().frames.len();
    assert!(n >= 8, "only {n} live view frames in 2 s");
    assert!(
        got.lock()
            .unwrap()
            .frames
            .iter()
            .all(|f| (f.0, f.1, f.2, f.3, f.4) == (640, 360, 160, 90, true))
    );
    // detached: no frame reaches it after the drop, while the reader goes on
    drop(attached);
    let n = got.lock().unwrap().frames.len();
    let end = Instant::now() + Duration::from_millis(600);
    let mut more = 0;
    while Instant::now() < end {
        if let Next::Frame(f) = src.next(Duration::from_millis(100)) {
            more += 1;
            src.recycle(f.rgba);
        }
    }
    assert!(more > 0);
    assert_eq!(got.lock().unwrap().frames.len(), n);
    // a live view attached when the reader stops gets the end
    let got2 = Arc::new(Mutex::new(Got::default()));
    let a2 = src
        .attacher()
        .attach(160, 160, INTERVAL, Box::new(Sink(got2.clone())))
        .unwrap();
    drop(src);
    assert_eq!(
        got2.lock().unwrap().ended.as_deref(),
        Some(dr_runtime::live::STOPPED)
    );
    assert!(!a2.is_attached());
}
