//! The speech thread with a fake engine and a null sink: nothing is played on the speakers.

use std::time::{Duration, Instant};

use dr_core::ZoneKind;
use dr_runtime::speech::{Cmd, ErrorKind, Ev, Line, NullSink, Speech, VoiceSettings};
use dr_tts::FakeTts;

const WAIT: Duration = Duration::from_secs(5);

fn line(text: &str) -> Line {
    Line::new(text, "Subtitles", Some(ZoneKind::Dialogue))
}

/// A speech thread whose sounds last `scale` times their length (fake PCM: 60 ms per character).
fn start(scale: f64) -> (Speech, FakeTts, NullSink) {
    start_with(NullSink::timed(scale))
}

fn start_with(sink: NullSink) -> (Speech, FakeTts, NullSink) {
    let tts = FakeTts::new();
    let (t, s) = (tts.clone(), sink.clone());
    let settings = VoiceSettings {
        voice: "fake-zira".into(),
        speed: 1.25,
        volume: 0.6,
    };
    (
        Speech::start(Box::new(t), move || Box::new(s), settings),
        tts,
        sink,
    )
}

fn test_cmd(id: u64, text: &str) -> Cmd {
    Cmd::Test {
        id,
        text: text.into(),
        voice: "fake-david".into(),
        speed: 1.0,
        volume: 0.9,
    }
}

/// Events until `done` matches one (or the wait ends).
fn until(sp: &Speech, done: impl Fn(&Ev) -> bool) -> Vec<Ev> {
    let end = Instant::now() + WAIT;
    let mut got = Vec::new();
    while Instant::now() < end {
        if let Some(e) = sp.next_event(Duration::from_millis(50)) {
            let stop = done(&e);
            got.push(e);
            if stop {
                return got;
            }
        }
    }
    panic!("no matching event within {WAIT:?}; got {got:?}");
}

fn is_finished(e: &Ev, text: &str) -> bool {
    matches!(e, Ev::Finished { item, .. } if item.text == text)
}

fn is_cut(e: &Ev, text: &str) -> bool {
    matches!(e, Ev::Cut { item, .. } if item.text == text)
}

fn finished(evs: &[Ev]) -> Vec<String> {
    evs.iter()
        .filter_map(|e| match e {
            Ev::Finished { item, .. } => Some(item.text.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn lines_are_spoken_in_order_with_the_profile_voice() {
    let (sp, tts, sink) = start(0.0);
    sp.send(Cmd::Say(vec![line("one"), line("two")]));
    sp.send(Cmd::Say(vec![
        Line::new("Arthur:", "Speaker", Some(ZoneKind::Speaker)),
        line("We need to go."),
    ]));
    let evs = until(&sp, |e| is_finished(e, "Arthur: We need to go."));
    assert_eq!(finished(&evs), ["one", "two", "Arthur: We need to go."]);
    // the events carry the item: its lines and zones (for the activity log)
    let Some(Ev::Finished { item, test }) = evs.last() else {
        panic!("{evs:?}")
    };
    assert_eq!(*test, None);
    assert_eq!(
        item.lines
            .iter()
            .map(|l| l.zone.as_str())
            .collect::<Vec<_>>(),
        ["Speaker", "Subtitles"]
    );
    let calls = tts.calls();
    assert!(
        calls
            .iter()
            .all(|c| c.voice == "fake-zira" && c.speed == 1.25),
        "{calls:?}"
    );
    let played = sink.played();
    assert_eq!(played.len(), 3);
    assert!(played.iter().all(|p| p.volume == 0.6));
    assert!(played[2].ms > played[0].ms, "longer text, longer sound");
}

#[test]
fn stop_clears_the_queue_and_skip_goes_on() {
    // each sound lasts its full length (60 ms per character)
    let (sp, _, sink) = start(1.0);
    sp.send(Cmd::Say(vec![
        line("a long first line to cut"),
        line("second line that plays a while"),
        line("third"),
    ]));
    until(&sp, |e| matches!(e, Ev::Started { .. }));
    sp.send(Cmd::Skip);
    // fake PCM: 30 characters at 60 ms, at speed 1.25
    let evs = until(
        &sp,
        |e| matches!(e, Ev::Started { item, ms, .. } if item.text == "second line that plays a while" && (*ms - 1440.0).abs() < 1.0),
    );
    assert!(
        evs.iter().any(|e| is_cut(e, "a long first line to cut")),
        "{evs:?}"
    );
    sp.send(Cmd::Stop);
    until(&sp, |e| is_cut(e, "second line that plays a while"));
    std::thread::sleep(Duration::from_millis(100));
    let st = sp.status();
    assert_eq!(
        (st.speaking, st.queued),
        (None, 0),
        "third was dropped by Stop"
    );
    assert_eq!(sink.played().len(), 2);
}

#[test]
fn replay_speaks_the_last_line_again() {
    let (sp, _, _) = start(0.0);
    sp.send(Cmd::Say(vec![line("hello")]));
    until(&sp, |e| is_finished(e, "hello"));
    sp.send(Cmd::Replay);
    until(&sp, |e| is_finished(e, "hello"));
}

#[test]
fn a_manual_press_replaces_what_waits() {
    let (sp, _, _) = start(1.0);
    sp.send(Cmd::Say(vec![
        line("playing now"),
        line("waiting one"),
        line("waiting two"),
    ]));
    until(&sp, |e| matches!(e, Ev::Started { .. }));
    sp.send(Cmd::Replace(vec![line("pressed")]));
    let evs = until(&sp, |e| is_finished(e, "pressed"));
    assert_eq!(finished(&evs), ["playing now", "pressed"]);
}

#[test]
fn mute_and_volume_act_on_the_sink() {
    let (sp, _, sink) = start(1.0);
    sp.send(Cmd::Say(vec![line("a line that plays for a while")]));
    until(&sp, |e| matches!(e, Ev::Started { .. }));
    sp.send(Cmd::Mute(true));
    sp.send(Cmd::SetVolume(0.3));
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(
        sink.volume(),
        0.0,
        "muted stays silent when the volume changes"
    );
    assert!(sp.status().muted);
    sp.send(Cmd::Mute(false));
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(sink.volume(), 0.3);
}

#[test]
fn the_voice_test_uses_its_own_voice_speed_and_volume() {
    let (sp, tts, sink) = start(1.0);
    sp.send(Cmd::Say(vec![
        line("a queued line that is long enough"),
        line("next"),
    ]));
    until(&sp, |e| matches!(e, Ev::Started { .. }));
    sp.send(Cmd::Test {
        id: 7,
        text: "sample".into(),
        voice: "fake-david".into(),
        speed: 2.0,
        volume: 0.9,
    });
    until(&sp, |e| matches!(e, Ev::Started { test: Some(7), .. }));
    assert_eq!(sp.status().test, Some(7));
    // the page slider moves while the sample plays
    sp.send(Cmd::SetTestVolume(0.5));
    let evs = until(&sp, |e| matches!(e, Ev::Finished { test: Some(7), .. }));
    assert!(
        !finished(&evs).contains(&"next".to_string()),
        "the test clears the queue: {evs:?}"
    );
    let last = tts.calls().pop().unwrap();
    assert_eq!(
        (last.text.as_str(), last.voice.as_str(), last.speed),
        ("sample", "fake-david", 2.0)
    );
    assert_eq!(sink.played().last().unwrap().volume, 0.9);
    assert_eq!(sink.volume(), 0.5);
    // the test's volume was for the sample only: read lines keep the profile volume
    sp.send(Cmd::Say(vec![line("after")]));
    until(
        &sp,
        |e| matches!(e, Ev::Started { item, .. } if item.text == "after"),
    );
    assert_eq!(sink.played().last().unwrap().volume, 0.6);
    // a test is never replayed as a read line
    sp.send(Cmd::Stop);
    sp.send(Cmd::Replay);
    let evs = until(&sp, |e| matches!(e, Ev::Finished { .. }));
    assert_eq!(finished(&evs), ["after"]);
}

#[test]
fn an_engine_error_is_reported_and_the_queue_goes_on() {
    let (sp, tts, _) = start(0.0);
    tts.fail_with(Some("no voice"));
    sp.send(Cmd::Say(vec![line("lost")]));
    until(
        &sp,
        |e| matches!(e, Ev::Error { item, kind: ErrorKind::Engine, error, .. } if item.text == "lost" && error == "no voice"),
    );
    tts.fail_with(None);
    sp.send(Cmd::Say(vec![line("works")]));
    until(&sp, |e| is_finished(e, "works"));
}

/// Play, Stop, Play pressed while the first sample is still being made: each event names its own test, so the
/// second sample is not taken as ended by the first one's Cut.
#[test]
fn a_test_started_while_another_is_made_keeps_its_own_events() {
    let (sp, tts, sink) = start(1.0);
    tts.set_delay(Duration::from_millis(300));
    sp.send(test_cmd(1, "first sample"));
    std::thread::sleep(Duration::from_millis(50));
    // the status already says which test is being made
    assert_eq!(sp.status().test, Some(1));
    sp.send(Cmd::Stop);
    sp.send(test_cmd(2, "second"));
    let evs = until(&sp, |e| matches!(e, Ev::Finished { test: Some(2), .. }));
    // the first sample was cut before its sound started: it never played
    assert!(
        evs.iter()
            .any(|e| matches!(e, Ev::Cut { test: Some(1), .. })),
        "{evs:?}"
    );
    assert!(
        !evs.iter()
            .any(|e| matches!(e, Ev::Started { test: Some(1), .. })),
        "{evs:?}"
    );
    let started_2 = evs
        .iter()
        .position(|e| matches!(e, Ev::Started { test: Some(2), .. }));
    let cut_1 = evs
        .iter()
        .position(|e| matches!(e, Ev::Cut { test: Some(1), .. }));
    assert!(cut_1 < started_2, "{evs:?}");
    assert_eq!(sink.played().len(), 1, "only the second sample played");
}

/// Stop while a line is being made: its sound is dropped, not played for a moment.
#[test]
fn stop_during_synth_plays_nothing() {
    let (sp, tts, sink) = start(1.0);
    tts.set_delay(Duration::from_millis(300));
    sp.send(Cmd::Say(vec![line("being made"), line("waiting")]));
    std::thread::sleep(Duration::from_millis(50));
    assert_eq!(sp.status().speaking.as_deref(), Some("being made"));
    sp.send(Cmd::Stop);
    until(&sp, |e| is_cut(e, "being made"));
    std::thread::sleep(Duration::from_millis(400));
    assert!(sink.played().is_empty(), "{:?}", sink.played());
    let st = sp.status();
    assert_eq!((st.speaking, st.queued), (None, 0));
    // Replay speaks the line that was cut before it played
    tts.set_delay(Duration::ZERO);
    sp.send(Cmd::Replay);
    until(
        &sp,
        |e| matches!(e, Ev::Started { item, .. } if item.text == "being made"),
    );
}

/// A device that stops taking samples (no error, the sound never ends) does not stall the queue: the line ends
/// with an error, the device is let go, and the next line plays.
#[test]
fn a_stuck_device_ends_the_line_and_the_queue_goes_on() {
    let (sp, _, sink) = start_with(NullSink::stuck());
    sp.send(Cmd::Say(vec![line("hi"), line("next")]));
    let evs = until(&sp, |e| matches!(e, Ev::Error { .. }));
    assert!(
        matches!(evs.last(), Some(Ev::Error { item, kind: ErrorKind::DeviceLost, .. }) if item.text == "hi"),
        "{evs:?}"
    );
    assert_eq!(sink.resets(), 1);
    until(
        &sp,
        |e| matches!(e, Ev::Started { item, .. } if item.text == "next"),
    );
}

/// A device that reports an error (unplugged) ends the line at once and is let go.
#[test]
fn a_device_error_ends_the_line() {
    let (sp, _, sink) = start(1.0);
    sp.send(Cmd::Say(vec![line("a line that plays for a while")]));
    until(&sp, |e| matches!(e, Ev::Started { .. }));
    sink.fail_device("device unplugged");
    until(
        &sp,
        |e| matches!(e, Ev::Error { kind: ErrorKind::DeviceLost, error, .. } if error == "device unplugged"),
    );
    assert_eq!(sink.resets(), 1);
    assert_eq!(sp.status().speaking, None);
    sp.send(Cmd::Say(vec![line("again")]));
    until(
        &sp,
        |e| matches!(e, Ev::Started { item, .. } if item.text == "again"),
    );
}

/// A device error while nothing plays: the next line opens the device again and plays to the end.
#[test]
fn a_device_error_while_idle_does_not_lose_the_next_line() {
    let (sp, _, sink) = start(0.0);
    sink.fail_device("device changed");
    sp.send(Cmd::Say(vec![line("after the error")]));
    let evs = until(&sp, |e| is_finished(e, "after the error"));
    assert!(
        !evs.iter().any(|e| matches!(e, Ev::Error { .. })),
        "{evs:?}"
    );
    assert_eq!(sink.resets(), 1);
}
