//! The quick menu state machine (app-plan M4): order, wrap, stepper limits, Quit twice, Esc, the voice list.

use dr_app::overlay::quick_menu::{self as qm, Menu, Out};
use dr_runtime::keys::Nav;

fn menu() -> Menu {
    Menu {
        voices: 3,
        voice: Some(1),
        ..Menu::default()
    }
}

#[test]
fn up_and_down_walk_the_rows_in_order_and_wrap() {
    let mut m = menu();
    assert_eq!(m.highlight, qm::ZONES);
    let mut seen = vec![m.highlight];
    for _ in 0..qm::ROWS {
        assert_eq!(m.key(Nav::Down), Out::None);
        seen.push(m.highlight);
    }
    // Manage zones, Stop, Mute, Voice, Speed, Volume, Mode, Settings, Quit, then round to the top
    assert_eq!(seen, vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 0]);
    m.key(Nav::Up);
    assert_eq!(m.highlight, qm::QUIT);
}

#[test]
fn enter_and_space_select() {
    let mut m = menu();
    assert_eq!(m.key(Nav::Enter), Out::ManageZones);
    m.highlight = qm::STOP;
    assert_eq!(m.key(Nav::Enter), Out::Pause(true));
    assert!(m.stopped);
    assert_eq!(m.key(Nav::Space), Out::Pause(false));
    m.highlight = qm::MUTE;
    assert_eq!(m.key(Nav::Enter), Out::Mute(true));
    assert_eq!(m.key(Nav::Enter), Out::Mute(false));
    m.highlight = qm::MODE;
    assert_eq!(m.key(Nav::Enter), Out::Mode(false));
    m.highlight = qm::SETTINGS;
    assert_eq!(m.key(Nav::Enter), Out::Settings);
    // rows with only a stepper do nothing on Enter
    m.highlight = qm::SPEED;
    assert_eq!(m.key(Nav::Enter), Out::None);
}

#[test]
fn quit_needs_two_presses_and_moving_away_disarms_it() {
    let mut m = menu();
    m.highlight = qm::QUIT;
    assert_eq!(m.key(Nav::Enter), Out::None);
    assert!(m.quit_armed);
    m.key(Nav::Up);
    assert!(!m.quit_armed);
    m.key(Nav::Down);
    assert_eq!(m.key(Nav::Enter), Out::None, "the first press only arms");
    assert_eq!(m.key(Nav::Enter), Out::Quit);
    // a click works the same way
    let mut m = menu();
    assert_eq!(m.activate(qm::QUIT), Out::None);
    assert_eq!(m.activate(qm::QUIT), Out::Quit);
    // a click on another row disarms
    let mut m = menu();
    m.activate(qm::QUIT);
    m.activate(qm::MUTE);
    assert_eq!(m.activate(qm::QUIT), Out::None);
}

#[test]
fn steppers_stop_at_their_limits() {
    let mut m = Menu {
        speed: 1.9,
        volume: 0.95,
        ..menu()
    };
    m.highlight = qm::SPEED;
    assert_eq!(m.key(Nav::Right), Out::Speed(2.0));
    assert_eq!(m.key(Nav::Right), Out::None, "2.0x is the most");
    for _ in 0..30 {
        m.key(Nav::Left);
    }
    assert_eq!(m.speed, qm::SPEED_MIN);
    assert_eq!(m.key(Nav::Left), Out::None);
    // many steps keep the 0.1 grid
    for _ in 0..7 {
        m.key(Nav::Right);
    }
    assert_eq!(m.speed, 1.2);
    m.highlight = qm::VOLUME;
    assert_eq!(m.key(Nav::Right), Out::Volume(1.0));
    assert_eq!(m.key(Nav::Right), Out::None);
    for _ in 0..30 {
        m.key(Nav::Left);
    }
    assert_eq!(m.volume, 0.0);
    m.highlight = qm::MODE;
    assert_eq!(m.key(Nav::Right), Out::Mode(false));
    assert_eq!(m.key(Nav::Right), Out::None);
    assert_eq!(m.key(Nav::Left), Out::Mode(true));
    // Left and Right on other rows do nothing
    m.highlight = qm::STOP;
    assert_eq!(m.key(Nav::Right), Out::None);
    assert_eq!(m.key(Nav::Left), Out::None);
}

#[test]
fn esc_closes_but_goes_back_from_the_voice_list() {
    let mut m = menu();
    assert!(m.esc_closes());
    m.highlight = qm::VOICE;
    assert_eq!(m.key(Nav::Right), Out::None);
    assert!(m.voice_open);
    assert_eq!(m.voice_hi, 1, "the list opens on the game's voice");
    assert!(!m.esc_closes());
    assert_eq!(m.key(Nav::Esc), Out::None);
    assert!(!m.voice_open);
    assert_eq!(m.key(Nav::Esc), Out::Close);
}

#[test]
fn the_voice_list_moves_previews_and_chooses() {
    let mut m = menu();
    m.highlight = qm::VOICE;
    assert_eq!(m.key(Nav::Enter), Out::None);
    assert!(m.voice_open);
    m.key(Nav::Down);
    m.key(Nav::Down);
    assert_eq!(m.voice_hi, 2, "no wrap at the end");
    assert_eq!(m.key(Nav::Space), Out::Preview(2));
    assert!(m.voice_open, "a preview keeps the list open");
    assert_eq!(m.key(Nav::Enter), Out::Voice(2));
    assert!(!m.voice_open);
    assert_eq!(m.voice, Some(2));
    // Left goes back too; Up stops at the top
    m.key(Nav::Right);
    for _ in 0..5 {
        m.key(Nav::Up);
    }
    assert_eq!(m.voice_hi, 0);
    m.key(Nav::Left);
    assert!(!m.voice_open);
    // no voices: the list does not open
    let mut m = Menu {
        highlight: qm::VOICE,
        ..Menu::default()
    };
    m.key(Nav::Right);
    assert!(!m.voice_open);
}

#[test]
fn a_long_voice_list_scrolls_with_the_highlight() {
    let mut m = Menu {
        voices: 20,
        voice: Some(15),
        ..Menu::default()
    };
    m.highlight = qm::VOICE;
    m.key(Nav::Right);
    let (first, n) = m.voice_slice();
    assert_eq!(n, qm::VOICE_ROWS);
    assert!(
        first <= 15 && 15 < first + n,
        "the game's voice is on screen"
    );
    for _ in 0..20 {
        m.key(Nav::Down);
    }
    assert_eq!(m.voice_hi, 19);
    assert_eq!(m.voice_slice(), (20 - qm::VOICE_ROWS, qm::VOICE_ROWS));
    for _ in 0..20 {
        m.key(Nav::Up);
    }
    assert_eq!(m.voice_slice(), (0, qm::VOICE_ROWS));
    // a short list shows whole
    let mut m = menu();
    m.highlight = qm::VOICE;
    m.key(Nav::Right);
    assert_eq!(m.voice_slice(), (0, 3));
}

#[test]
fn opening_again_starts_at_the_top() {
    let mut m = menu();
    m.highlight = qm::QUIT;
    m.key(Nav::Enter);
    m.open();
    assert_eq!(m.highlight, 0);
    assert!(!m.quit_armed);
}
