//! Page 05 and Settings > Voices on fake voices (M3a). The engine is `FakeTts` and the sink a `NullSink`, so no
//! sound plays; the real Windows voices are only used by the app binary.

mod common;

use std::time::{Duration, Instant};

use common::*;
use dr_app::Page;
use dr_app::vm::voices::Voices;
use dr_runtime::profile;
use dr_runtime::speech::{NullSink, Speech, VoiceSettings};
use dr_tts::{Engines, FakeTts};
use i_slint_backend_testing::{AccessibleRole, ElementHandle};
use slint::{ComponentHandle, Model};

/// Fake voices with access to the engine's call log and the sink's log.
fn voices(scale: f64) -> (Voices, FakeTts, NullSink) {
    let tts = FakeTts::new();
    let sink = NullSink::timed(scale);
    let (t, s) = (tts.clone(), sink.clone());
    // Windows voices only (no Kokoro), through the same engine router as the app
    let speech = Speech::start(
        Box::new(Engines::new(None, Some(Box::new(t)))),
        move || Box::new(s),
        VoiceSettings::default(),
    );
    let mut v = Voices::of(&Engines::new(None, Some(Box::new(tts.clone()))), 2, None);
    v.speech = Some(speech);
    (v, tts, sink)
}

/// Lets the Slint change handlers run (`changed page`), as the event loop does.
fn settle() {
    slint::platform::update_timers_and_animations();
}

/// Polls the voice test until it ends (or panics after 5 s).
fn wait_test_end(vm: &dr_app::vm::Shared) {
    let end = Instant::now() + Duration::from_secs(5);
    while vm.borrow_mut().poll_test() {
        assert!(Instant::now() < end, "the voice test never ended");
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn page_05_lists_the_voices_and_plays_a_sample_with_the_page_settings() {
    let (_dir, paths) = temp_paths();
    let (vs, tts, sink) = voices(0.0);
    let (w, vm) = real_window_with(&paths, FakeDesktop::new(), vs);
    let names: Vec<String> = w.get_voices().iter().map(|v| v.name.to_string()).collect();
    assert_eq!(names, ["David", "Zira"]);
    let zira = w.get_voices().row_data(1).unwrap();
    assert_eq!(
        (zira.desc.as_str(), zira.short.as_str()),
        ("American English · Female", "US · Female")
    );
    assert_eq!(w.get_voices_problem(), "");
    assert_eq!(w.get_voices_desc(), "Windows 2");
    assert!(!w.get_voice_testing(), "nothing plays at the start");

    w.set_page(Page::Voice);
    // two thirds of the 0.5x to 2.0x track is 1.5x
    w.set_voice_speed(2.0 / 3.0);
    w.set_voice_volume(0.4);
    // the play button of the Zira row picks Zira and plays her sample
    press(&w, "Play a sample of Zira", AccessibleRole::Button);
    assert_eq!(w.get_picked_voice(), 1);
    assert!(w.get_voice_testing());
    wait_test_end(&vm);
    assert!(!w.get_voice_testing());
    let call = tts.calls().pop().expect("one synth call");
    assert_eq!(
        (call.text.as_str(), call.voice.as_str(), call.speed),
        (dr_app::vm::voices::SAMPLE, "fake-zira", 1.5)
    );
    assert_eq!(sink.played().last().unwrap().volume, 0.4);
}

#[test]
fn stop_ends_the_sample() {
    let (_dir, paths) = temp_paths();
    // sounds last their full length (about 4 s for the sample)
    let (vs, _, _) = voices(1.0);
    let (w, vm) = real_window_with(&paths, FakeDesktop::new(), vs);
    w.set_page(Page::Voice);
    press(&w, "Play a sample of David", AccessibleRole::Button);
    std::thread::sleep(Duration::from_millis(200));
    assert!(vm.borrow_mut().poll_test());
    assert!(
        w.get_test_time().ends_with("/ 0:05"),
        "{}",
        w.get_test_time()
    );
    press(&w, "Stop", AccessibleRole::Button);
    assert!(!w.get_voice_testing());
    assert_eq!(w.get_test_played(), 0);
}

#[test]
fn with_no_voice_installed_the_pages_say_so_and_finish_still_saves() {
    let (_dir, paths) = temp_paths();
    let desktop = FakeDesktop::new();
    let opened = desktop.opened_settings.clone();
    let (w, _vm) = real_window_with(&paths, desktop, Voices::none());
    assert_eq!(w.get_voices().row_count(), 0);
    // one form everywhere (TEXT.md: "No voices found." plus Add voices)
    assert_eq!(w.get_voices_desc(), "No voices found.");
    assert_eq!(w.get_voices_problem(), "No voices found.");
    assert_eq!(w.get_voice_label(), "No voice");
    w.set_page(Page::Voice);
    // nothing to play: no play button, the page says so
    assert!(
        ElementHandle::find_by_accessible_label(&w, "Play a sample of David")
            .next()
            .is_none()
    );
    assert!(
        ElementHandle::find_by_accessible_label(&w, "No voices found.")
            .next()
            .is_some()
    );
    assert!(!w.get_voice_testing());
    assert_eq!(w.get_notice(), "");
    press(&w, "Add voices", AccessibleRole::Button);
    assert_eq!(
        opened.get(),
        1,
        "Add voices opens the Windows speech settings"
    );

    w.set_page(Page::Welcome);
    w.set_notice("".into());
    press(&w, "Get started", AccessibleRole::Button);
    press(&w, "Continue", AccessibleRole::Button);
    for _ in 0..3 {
        press(&w, "Continue", AccessibleRole::Button);
    }
    press(&w, "Finish", AccessibleRole::Button);
    assert_eq!(w.get_page(), Page::Overview);
    let p = profile::load(&paths.profile_json("test-game")).unwrap();
    assert_eq!(p.voice.id, "");
}

#[test]
fn settings_voice_speed_and_volume_are_saved_in_the_current_profile() {
    let (_dir, paths) = temp_paths();
    let (vs, tts, _) = voices(0.0);
    let (w, vm) = real_window_with(&paths, FakeDesktop::new(), vs);
    press(&w, "Get started", AccessibleRole::Button);
    press(&w, "Continue", AccessibleRole::Button);
    for _ in 0..3 {
        press(&w, "Continue", AccessibleRole::Button);
    }
    press(&w, "Finish", AccessibleRole::Button);
    assert_eq!(w.get_voice_label(), "David");
    assert_eq!(w.get_game_voice(), 0);

    w.set_page(Page::Settings);
    w.invoke_game_voice_picked(1);
    w.set_game_speed(2.0 / 3.0);
    w.set_game_volume(0.3);
    w.invoke_game_voice_changed();
    assert_eq!(w.get_voice_label(), "Zira");
    // a moving slider is not saved on every step; letting it go saves
    let p = profile::load(&paths.profile_json("test-game")).unwrap();
    assert!((p.voice.volume - 0.8).abs() < 1e-4, "{}", p.voice.volume);
    w.invoke_game_voice_saved();
    let p = profile::load(&paths.profile_json("test-game")).unwrap();
    assert_eq!(p.voice.id, "win:fake-zira");
    assert!((p.voice.speed - 1.5).abs() < 1e-4, "{}", p.voice.speed);
    assert!((p.voice.volume - 0.3).abs() < 1e-4);

    // Test in Settings speaks with the game's voice (the list scrolls to Voices first: hidden rows are not found)
    w.invoke_settings_jump(3);
    press(&w, "Play a sample", AccessibleRole::Button);
    wait_test_end(&vm);
    let call = tts.calls().pop().unwrap();
    assert_eq!((call.voice.as_str(), call.speed), ("fake-zira", 1.5));

    // a second window on the same folder shows the saved voice
    drop(w);
    drop(vm);
    let (vs, _, _) = voices(0.0);
    let (w, _vm) = real_window_with(&paths, FakeDesktop::new(), vs);
    assert_eq!(w.get_page(), Page::Overview);
    assert_eq!(w.get_game_voice(), 1);
    assert!((w.get_game_speed() - 2.0 / 3.0).abs() < 1e-4);
    assert!((w.get_game_volume() - 0.3).abs() < 1e-4);
}

#[test]
fn a_saved_voice_that_is_gone_falls_back_to_the_first() {
    let (_dir, paths) = temp_paths();
    let mut p = dr_runtime::Profile::new("g", "Game", "TestGame.exe");
    p.voice.id = "uninstalled-voice".into();
    profile::save(&paths, &p).unwrap();
    let (vs, _, _) = voices(0.0);
    let (w, _vm) = real_window_with(&paths, FakeDesktop::new(), vs);
    assert_eq!(w.get_game_voice(), 0);
    assert_eq!(w.get_voice_label(), "David");
}

#[test]
fn an_old_windows_id_without_a_prefix_still_picks_its_voice() {
    // profiles saved before Kokoro (M3a) hold the bare Windows id
    let (_dir, paths) = temp_paths();
    let mut p = dr_runtime::Profile::new("g", "Game", "TestGame.exe");
    p.voice.id = "fake-zira".into();
    profile::save(&paths, &p).unwrap();
    let (vs, _, _) = voices(0.0);
    let (w, _vm) = real_window_with(&paths, FakeDesktop::new(), vs);
    assert_eq!(w.get_game_voice(), 1);
    assert_eq!(w.get_voice_label(), "Zira");
}

/// A finished game, its window and view model, on fake voices whose engine takes `delay` to make a sound.
fn with_game(
    paths: &dr_runtime::Paths,
    scale: f64,
) -> (dr_app::MainWindow, dr_app::vm::Shared, FakeTts, NullSink) {
    let (vs, tts, sink) = voices(scale);
    let (w, vm) = real_window_with(paths, FakeDesktop::new(), vs);
    press(&w, "Get started", AccessibleRole::Button);
    press(&w, "Continue", AccessibleRole::Button);
    for _ in 0..3 {
        press(&w, "Continue", AccessibleRole::Button);
    }
    press(&w, "Finish", AccessibleRole::Button);
    (w, vm, tts, sink)
}

#[test]
fn leaving_the_page_stops_the_sample_and_saves_moved_sliders() {
    let (_dir, paths) = temp_paths();
    let (w, vm, _, _) = with_game(&paths, 1.0);
    settle();
    w.set_page(Page::Voice);
    settle();
    press(&w, "Play a sample of David", AccessibleRole::Button);
    assert!(vm.borrow_mut().poll_test());
    w.set_page(Page::Keys);
    settle();
    assert!(!w.get_voice_testing(), "the sample stops with the page");
    assert!(!vm.borrow_mut().poll_test());

    // Settings: a slider moved but never let go (the pointer left the window) is saved when the page changes
    w.set_page(Page::Settings);
    settle();
    w.set_game_volume(0.25);
    w.invoke_game_voice_changed();
    w.set_page(Page::Overview);
    settle();
    let p = profile::load(&paths.profile_json("test-game")).unwrap();
    assert!((p.voice.volume - 0.25).abs() < 1e-4, "{}", p.voice.volume);
}

/// Play, Stop, Play pressed while the first sample is still being made: the second sample keeps the Stop button
/// until it ends (the first one's Cut does not end it).
#[test]
fn a_quick_stop_and_play_follows_the_second_sample() {
    let (_dir, paths) = temp_paths();
    let (vs, tts, sink) = voices(0.05);
    let (w, vm) = real_window_with(&paths, FakeDesktop::new(), vs);
    tts.set_delay(Duration::from_millis(300));
    w.set_page(Page::Voice);
    press(&w, "Play a sample of David", AccessibleRole::Button);
    press(&w, "Stop", AccessibleRole::Button);
    press(&w, "Play a sample of David", AccessibleRole::Button);
    // the first sample is cut while it is made; the second one is made after it
    std::thread::sleep(Duration::from_millis(400));
    assert!(vm.borrow_mut().poll_test(), "the second sample still plays");
    assert!(w.get_voice_testing());
    wait_test_end(&vm);
    assert!(!w.get_voice_testing());
    assert_eq!(sink.played().len(), 1, "only the second sample played");
}

#[test]
fn a_failed_test_says_why_in_plain_words() {
    let (_dir, paths) = temp_paths();
    let (vs, tts, _) = voices(0.0);
    let (w, vm) = real_window_with(&paths, FakeDesktop::new(), vs);
    tts.fail_with(Some(
        "Windows voices: speak: The system cannot find the file specified.",
    ));
    w.set_page(Page::Voice);
    press(&w, "Play a sample of David", AccessibleRole::Button);
    wait_test_end(&vm);
    assert!(!w.get_voice_testing());
    assert_eq!(w.get_notice(), "The voice could not speak the sample.");
}

#[test]
fn the_page_05_volume_slider_changes_a_playing_sample() {
    let (_dir, paths) = temp_paths();
    let (vs, _, sink) = voices(1.0);
    let (w, vm) = real_window_with(&paths, FakeDesktop::new(), vs);
    w.set_page(Page::Voice);
    w.set_voice_volume(0.4);
    press(&w, "Play a sample of David", AccessibleRole::Button);
    let end = Instant::now() + Duration::from_secs(5);
    while sink.played().is_empty() {
        assert!(Instant::now() < end);
        std::thread::sleep(Duration::from_millis(10));
    }
    w.set_voice_volume(0.7);
    w.invoke_voice_volume_moved();
    std::thread::sleep(Duration::from_millis(100));
    assert!((sink.volume() - 0.7).abs() < 1e-4, "{}", sink.volume());
    vm.borrow_mut().stop_test();
}

#[test]
fn a_kokoro_model_that_fails_to_load_switches_the_screens_to_the_windows_voices() {
    use dr_tts::kokoro::{LoadState, LoadWatch};
    let (_dir, paths) = temp_paths();
    let kokoro = FakeTts::with_voices(vec![
        dr_tts::kokoro::voice_of("af_heart"),
        dr_tts::kokoro::voice_of("am_adam"),
    ]);
    let engines = Engines::new(Some(Box::new(kokoro)), Some(Box::new(FakeTts::new())));
    let watch = LoadWatch::default();
    let vs = Voices {
        kokoro_load: Some(watch.clone()),
        ..Voices::of(&engines, 2, None)
    };
    let (w, _vm) = real_window_with(&paths, FakeDesktop::new(), vs);
    let names = |w: &dr_app::MainWindow| -> Vec<String> {
        w.get_voices().iter().map(|v| v.name.to_string()).collect()
    };
    assert_eq!(names(&w), ["Heart", "Adam"]);
    assert_eq!(w.get_voices_desc(), "Kokoro 2 · Windows 2");
    assert_eq!(w.get_voice_label(), "Heart");

    // the model fails on its own thread; the next page change shows the Windows voices
    watch.set(LoadState::Failed("onnxruntime.dll is too old".into()));
    w.set_page(Page::Voice);
    settle();
    assert_eq!(names(&w), ["David", "Zira"]);
    assert_eq!(w.get_voices_desc(), "Windows 2");
    assert_eq!(w.get_voice_label(), "David");
}

/// The Voice button of the Overview toolbar opens a voice picker in place (Max, 2026-11-24), not Settings. A
/// pick sets and saves the game's voice and closes the list; Esc and a click outside close it too.
#[test]
fn the_overview_voice_button_picks_a_voice_in_place() {
    let (_dir, paths) = temp_paths();
    let (vs, _, _) = voices(0.0);
    let (w, _vm) = real_window_with(&paths, FakeDesktop::new(), vs);
    press(&w, "Get started", AccessibleRole::Button);
    press(&w, "Continue", AccessibleRole::Button);
    for _ in 0..3 {
        press(&w, "Continue", AccessibleRole::Button);
    }
    press(&w, "Finish", AccessibleRole::Button);
    settle();
    assert_eq!(w.get_page(), Page::Overview);
    assert_eq!(w.get_voice_label(), "David");
    let rows = |w: &dr_app::MainWindow| {
        ElementHandle::find_by_accessible_label(w, "Zira")
            .filter(|e| e.accessible_role() == Some(AccessibleRole::ListItem))
            .count()
    };
    assert_eq!(rows(&w), 0, "closed at first");

    // open: the page stays, the voices show, the game's voice is the picked row
    press(&w, "Voice", AccessibleRole::Button);
    assert!(w.get_voice_menu());
    assert_eq!(w.get_page(), Page::Overview);
    assert_eq!(rows(&w), 1);
    let david = ElementHandle::find_by_accessible_label(&w, "David")
        .find(|e| e.accessible_role() == Some(AccessibleRole::ListItem))
        .expect("David in the list");
    assert_eq!(david.accessible_item_selected(), Some(true));

    // Esc closes it, and the page stays
    key(&w, slint::platform::Key::Escape);
    assert!(!w.get_voice_menu());
    assert_eq!(w.get_page(), Page::Overview);

    // a click outside closes it
    press(&w, "Voice", AccessibleRole::Button);
    assert!(w.get_voice_menu());
    let win = w.window();
    let at = slint::LogicalPosition::new(400.0, 400.0);
    win.dispatch_event(slint::platform::WindowEvent::PointerPressed {
        position: at,
        button: slint::platform::PointerEventButton::Left,
    });
    win.dispatch_event(slint::platform::WindowEvent::PointerReleased {
        position: at,
        button: slint::platform::PointerEventButton::Left,
    });
    assert!(!w.get_voice_menu());
    assert_eq!(w.get_page(), Page::Overview, "the click did not open Zones");

    // a pick sets the game's voice, saves it and closes the list
    press(&w, "Voice", AccessibleRole::Button);
    press(&w, "Zira", AccessibleRole::ListItem);
    assert!(!w.get_voice_menu());
    assert_eq!(w.get_game_voice(), 1);
    assert_eq!(w.get_voice_label(), "Zira");
    let p = profile::load(&paths.profile_json("test-game")).unwrap();
    assert_eq!(p.voice.id, "win:fake-zira");
}
