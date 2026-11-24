//! Reading in the app (M3b) with a runtime on fake parts: a still frame, a fake OCR, no key hooks, no window
//! checks, a fake voice and a null sink. Covers the status, the line being spoken, Recent and the log file, Pause,
//! key capture (key events are fed as the hook would hand them) and the notices for elevated and fullscreen games.

mod common;

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use common::{FakeDesktop, fake_voices, real_window_with, temp_paths};
use dr_app::vm::reading::{self, NOTE_CLICK, NOTE_ELEVATED, NOTE_FULLSCREEN};
use dr_app::{MainWindow, Page};
use dr_core::{Ocr, OcrLine, Rect};
use dr_runtime::capture::{FrameSource, Next, OwnedFrame};
use dr_runtime::keys::{Action, Btn, Key};
use dr_runtime::runtime::{Ev, Parts, Runtime};
use dr_runtime::{Kind, Profile, ZoneCfg, profile};
use slint::Model;

const LINE: &str = "Hello there, friend.";

/// One frame, then nothing new (a still window).
struct Still {
    sent: bool,
    t: f64,
}

impl FrameSource for Still {
    fn next(&mut self, wait: Duration) -> Next {
        if !self.sent {
            self.sent = true;
            return Next::Frame(OwnedFrame {
                t_ms: 0.0,
                w: 640,
                h: 360,
                rgba: [40u8, 40, 40, 255].repeat(640 * 360),
            });
        }
        std::thread::sleep(wait);
        self.t += wait.as_secs_f64() * 1000.0;
        Next::Same(self.t)
    }
}

/// Reads the same line in every crop, and counts its calls.
struct FakeOcr(Arc<AtomicU64>);

impl Ocr for FakeOcr {
    fn recognize(&mut self, w: u32, h: u32, _rgba: &[u8]) -> Result<Vec<OcrLine>, String> {
        self.0.fetch_add(1, Ordering::Relaxed);
        Ok(vec![OcrLine {
            text: LINE.into(),
            rect: Rect::new(2.0, 2.0, w as f32 - 4.0, h as f32 - 4.0),
        }])
    }
}

struct App {
    w: MainWindow,
    vm: dr_app::vm::Shared,
    paths: dr_runtime::Paths,
    calls: Arc<AtomicU64>,
    _dir: tempfile::TempDir,
}

/// A saved game ("Test Game", one dialogue zone) whose window runs on the fake desktop, and the runtime.
fn app() -> App {
    app_with(None)
}

/// [`app`] with one more saved game (its window does not run).
fn app_with(extra: Option<Profile>) -> App {
    let (dir, paths) = temp_paths();
    if let Some(p) = extra {
        profile::save(&paths, &p).unwrap();
    }
    let mut p = Profile::new("test-game", "Test Game", "TestGame.exe");
    p.zones.push(ZoneCfg {
        name: "Subtitles".into(),
        kind: Kind::Dialogue,
        on: true,
        rect: [40.0, 260.0, 560.0, 60.0],
        base: [640, 360],
    });
    profile::save(&paths, &p).unwrap();
    let voices = fake_voices();
    let speech = voices.speech.as_ref().unwrap().sender();
    let (w, vm) = real_window_with(&paths, FakeDesktop::new(), voices);
    assert_eq!(w.get_page(), Page::Overview);
    // nothing reads before the runtime comes
    assert_eq!(w.get_status_text().as_str(), reading::STATUS_STOPPED);
    let calls = Arc::new(AtomicU64::new(0));
    let c = calls.clone();
    let parts = Parts {
        ocr: Arc::new(move |_lang: &str| Ok(Box::new(FakeOcr(c.clone())) as Box<dyn Ocr>)),
        source: Arc::new(|_hwnd| {
            Ok((
                Box::new(Still {
                    sent: false,
                    t: 0.0,
                }) as Box<dyn FrameSource>,
                None,
            ))
        }),
        hooks: false,
        checks: false,
        speech,
    };
    dr_app::vm::attach_runtime(&vm, Runtime::new(parts), None);
    App {
        w,
        vm,
        paths,
        calls,
        _dir: dir,
    }
}

impl App {
    /// Runs the reading timer (no event loop in tests) until `done`.
    fn until(&self, limit: Duration, done: impl Fn(&App) -> bool) -> bool {
        let end = Instant::now() + limit;
        while Instant::now() < end {
            self.vm.borrow_mut().reading_tick();
            if done(self) {
                return true;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        done(self)
    }

    /// The Overview read list (it leaves out the line in Now).
    fn recent(&self) -> Vec<String> {
        let m = self.w.get_recent();
        (0..m.row_count())
            .filter_map(|i| m.row_data(i))
            .map(|r| r.line.to_string())
            .collect()
    }
    /// The Activity list (every line, the one in Now included).
    fn log(&self) -> Vec<String> {
        let m = self.w.get_log();
        (0..m.row_count())
            .filter_map(|i| m.row_data(i))
            .map(|r| r.line.to_string())
            .collect()
    }
}

const WAIT: Duration = Duration::from_secs(10);

#[test]
fn the_game_is_read_and_its_line_shows_and_is_logged() {
    let a = app();
    // the window of the game is found, the reader starts
    assert!(a.until(WAIT, |a| a.w.get_status_text().as_str()
        == reading::STATUS_READING));
    assert!(a.w.get_status_ok());
    assert!(a.vm.borrow().reading_target().is_some());
    // the line on the still screen is spoken: the "now speaking" card, the Activity list, the log file. The
    // Overview read list leaves it out, as it shows in Now.
    assert!(a.until(WAIT, |a| a.w.get_now_line().as_str() == LINE));
    // while it is spoken, its zone's outline is lime (the zone of the line)
    assert!(a.until(WAIT, |a| a.w.get_live_zone() == 0));
    assert!(a.until(WAIT, |a| a.log().first().is_some_and(|l| l == LINE)));
    assert!(a.recent().is_empty(), "{:?}", a.recent());
    let log = dr_runtime::activity::Log::new(a.paths.activity_log("test-game"));
    let saved = log.last(10);
    assert_eq!(saved.len(), 1, "{saved:?}");
    assert_eq!(saved[0].text, LINE);
    assert_eq!(saved[0].skip, None);
}

#[test]
fn pause_stops_ocr_and_resume_reads_again() {
    let a = app();
    assert!(a.until(WAIT, |a| a.calls.load(Ordering::Relaxed) > 0));
    a.vm.borrow_mut().toggle_pause();
    assert_eq!(a.w.get_status_text().as_str(), reading::STATUS_PAUSED);
    assert!(a.w.get_paused());
    assert!(!a.w.get_status_ok());
    // let the reader see the pause
    std::thread::sleep(Duration::from_millis(200));
    let n = a.calls.load(Ordering::Relaxed);
    std::thread::sleep(Duration::from_millis(1500));
    a.vm.borrow_mut().reading_tick();
    assert_eq!(a.calls.load(Ordering::Relaxed), n, "OCR ran while paused");
    // Read now while paused reads the still screen at once (no new frame comes from a still window)
    a.vm.borrow_mut().read_now();
    assert!(a.until(WAIT, |a| a.calls.load(Ordering::Relaxed) > n));
    let n = a.calls.load(Ordering::Relaxed);
    a.vm.borrow_mut().toggle_pause();
    assert_eq!(a.w.get_status_text().as_str(), reading::STATUS_READING);
    assert!(!a.w.get_paused());
    // after Resume the reader works on the kept frame (a still screen sends no new one; the automatic read
    // reuses its last read of the same pixels without OCR, so a press shows it)
    a.vm.borrow_mut().read_now();
    assert!(a.until(WAIT, |a| a.calls.load(Ordering::Relaxed) > n));
}

#[test]
fn key_capture_takes_a_key_refuses_clicks_and_saves_in_settings() {
    let a = app();
    a.w.set_page(Page::Settings);
    a.vm.borrow_mut().key_change(0);
    assert_eq!(a.w.get_capturing_key(), 0);
    // left click: refused with a plain line, still waiting
    a.vm.borrow_mut()
        .runtime_event(Ev::Key(Action::Refused(Btn::Left)));
    assert_eq!(a.w.get_notice().as_str(), NOTE_CLICK);
    assert_eq!(a.w.get_capturing_key(), 0);
    // the quick menu key: refused, it is taken
    a.vm.borrow_mut()
        .key_event(Action::Captured(Key::parse("F7").unwrap()));
    assert!(
        a.w.get_notice().contains("menu key"),
        "{}",
        a.w.get_notice()
    );
    assert_eq!(a.w.get_capturing_key(), 0);
    // Mouse 5: taken, shown and saved in the profile
    a.vm.borrow_mut().key_event(Action::Captured(Key::Mouse(5)));
    assert_eq!(a.w.get_capturing_key(), -1);
    assert_eq!(a.w.get_read_key().as_str(), "Mouse 5");
    assert_eq!(a.w.get_notice().as_str(), "");
    let (saved, _) = profile::load_all(&a.paths);
    assert_eq!(saved[0].keys.read, "Mouse 5");
    // Esc cancels; a page change ends a capture too
    a.vm.borrow_mut().key_change(1);
    assert_eq!(a.w.get_capturing_key(), 1);
    a.vm.borrow_mut().key_event(Action::Cancelled);
    assert_eq!(a.w.get_capturing_key(), -1);
    assert_eq!(a.w.get_menu_key().as_str(), "F7");
    a.vm.borrow_mut().key_change(1);
    a.w.set_page(Page::Overview);
    a.w.invoke_page_changed(Page::Overview);
    assert_eq!(a.w.get_capturing_key(), -1);
    // "Cancel" on the same row ends it
    a.vm.borrow_mut().key_change(0);
    a.vm.borrow_mut().key_change(0);
    assert_eq!(a.w.get_capturing_key(), -1);
}

#[test]
fn key_capture_ends_as_cancel_when_the_window_loses_the_focus() {
    let a = app();
    for page in [Page::Keys, Page::Settings] {
        a.w.set_page(page);
        let before = a.w.get_read_key();
        a.vm.borrow_mut().key_change(0);
        assert_eq!(a.w.get_capturing_key(), 0);
        // a refused click put a notice up; it goes with the capture
        a.vm.borrow_mut().key_event(Action::Refused(Btn::Right));
        assert_eq!(a.w.get_notice().as_str(), NOTE_CLICK);
        // Alt+Tab, Win, a click in the game: the window loses the focus
        dr_app::focus_lost(&a.w, &a.vm);
        assert_eq!(a.w.get_capturing_key(), -1, "{page:?}");
        assert_eq!(a.w.get_read_key(), before, "the key stays as it was");
        assert_eq!(a.w.get_notice().as_str(), "");
        // a key the hook hands on after that is not taken
        a.vm.borrow_mut()
            .key_event(Action::Captured(Key::parse("F9").unwrap()));
        assert_eq!(a.w.get_read_key(), before);
    }
    // focus loss without a capture changes nothing; a new capture works as before
    dr_app::focus_lost(&a.w, &a.vm);
    a.vm.borrow_mut().key_change(1);
    assert_eq!(a.w.get_capturing_key(), 1);
    a.vm.borrow_mut()
        .key_event(Action::Captured(Key::parse("F9").unwrap()));
    assert_eq!(a.w.get_menu_key().as_str(), "F9");
    assert_eq!(a.w.get_capturing_key(), -1);
}

#[test]
fn elevated_and_fullscreen_games_are_explained() {
    let a = app();
    a.vm.borrow_mut().runtime_event(Ev::Elevated);
    assert_eq!(a.w.get_notice().as_str(), NOTE_ELEVATED);
    a.vm.borrow_mut().runtime_event(Ev::Fullscreen);
    assert_eq!(a.w.get_notice().as_str(), NOTE_FULLSCREEN);
}

#[test]
fn a_replay_adds_no_row() {
    let a = app();
    assert!(a.until(WAIT, |a| a.log().first().is_some_and(|l| l == LINE)));
    let idle = |a: &App| {
        a.vm.borrow()
            .speech_status()
            .is_some_and(|s| s.speaking.is_none() && s.queued == 0)
    };
    let speaking = |a: &App| {
        a.vm.borrow()
            .speech_status()
            .is_some_and(|s| s.speaking.as_deref() == Some(LINE))
    };
    assert!(a.until(WAIT, idle));
    // the replay button of the Activity row
    a.vm.borrow().replay_row(0);
    assert!(a.until(WAIT, speaking), "the row was not replayed");
    assert!(a.until(WAIT, idle));
    // Replay in the "now speaking" card
    a.vm.borrow().speech_cmd(dr_runtime::speech::Cmd::Replay);
    assert!(a.until(WAIT, speaking), "the line was not replayed");
    assert!(a.until(WAIT, idle));
    assert_eq!(
        a.w.get_live_zone(),
        -1,
        "no zone is lime once the line ends"
    );
    assert_eq!(a.log(), [LINE]);
    assert!(
        a.recent().is_empty(),
        "the line in Now is not repeated in the list"
    );
    let log = dr_runtime::activity::Log::new(a.paths.activity_log("test-game"));
    assert_eq!(log.last(10).len(), 1, "{:?}", log.last(10));
}

/// A game switch while a line plays: speech stops, and what the game before spoke or queued stays in its own log.
#[test]
fn a_game_switch_stops_speech_and_keeps_the_logs_apart() {
    let mut other = Profile::new("zed-game", "Zed Game", "ZedGame.exe");
    other.zones.push(ZoneCfg {
        name: "Subtitles".into(),
        kind: Kind::Dialogue,
        on: true,
        rect: [40.0, 260.0, 560.0, 60.0],
        base: [640, 360],
    });
    let a = app_with(Some(other));
    // the line plays (about 1.2 s on the timed sink)
    assert!(a.until(WAIT, |a| a.w.get_now_line().as_str() == LINE));
    let i = (0..2)
        .find(|&i| {
            a.w.get_games()
                .row_data(i)
                .is_some_and(|g| g.name.as_str() == "Zed Game")
        })
        .expect("the other game is listed");
    a.w.invoke_game_chosen(i as i32);
    assert!(a.vm.borrow().log_entries().is_empty());
    assert_eq!(a.w.get_now_line().as_str(), "");
    let idle = |a: &App| {
        a.vm.borrow()
            .speech_status()
            .is_some_and(|s| s.speaking.is_none() && s.queued == 0)
    };
    assert!(a.until(WAIT, idle), "speech did not stop");
    // the events of the Stop come after it: let them in
    assert!(!a.until(Duration::from_millis(500), |a| !a.log().is_empty()));
    let old = dr_runtime::activity::Log::new(a.paths.activity_log("test-game")).last(10);
    assert_eq!(old.len(), 1, "{old:?}");
    assert_eq!((old[0].text.as_str(), old[0].skip), (LINE, None));
    let new = dr_runtime::activity::Log::new(a.paths.activity_log("zed-game")).last(10);
    assert!(new.is_empty(), "{new:?}");
    assert_eq!(a.w.get_now_line().as_str(), "");
}
