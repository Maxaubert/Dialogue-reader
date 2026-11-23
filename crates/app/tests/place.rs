//! Overlay placement (app-plan M4): the pill and the quick menu over the game at 100 %, 150 % and 200 %, on a
//! monitor left of the main one, and the zone edit mapping from window px to capture px (also for a game with a
//! title bar, where the capture covers more than the client area).

use dr_app::geom::Rect;
use dr_app::overlay::place::{self, CaptureMap, PillAt, PxRect};
use dr_app::overlay::zone_edit;
use dr_app::vm::zones::Editor;

#[test]
fn the_pill_sits_24_px_in_from_the_top_right_at_every_scale() {
    for scale in [1.0f32, 1.5, 2.0] {
        let client = PxRect::new(100, 50, 2560, 1440);
        let (x, y) = place::pill(client, scale, PillAt::TopRight);
        let s = |v: f32| (v * scale).round() as i32;
        // the pill's right edge (8 px inside its window) is 24 px from the game's right edge
        assert_eq!(
            x + s(place::PILL_W - place::PILL_PAD),
            client.right() - s(24.0),
            "{scale}"
        );
        // its top (8 px inside the window) is 24 px below the game's top
        assert_eq!(y + s(place::PILL_PAD), client.y + s(24.0), "{scale}");
    }
}

#[test]
fn the_pill_at_bottom_center() {
    let client = PxRect::new(0, 0, 1920, 1080);
    let (x, y) = place::pill(client, 1.5, PillAt::BottomCenter);
    assert_eq!(x + (place::PILL_W * 1.5).round() as i32 / 2, 960);
    // the pill's bottom edge (8 px inside the window) is 24 px above the game's bottom
    assert_eq!(
        y + ((place::PILL_H - place::PILL_PAD) * 1.5).round() as i32,
        1080 - 36
    );
    assert_eq!(PillAt::of(0), PillAt::TopRight);
    assert_eq!(PillAt::of(1), PillAt::BottomCenter);
    assert_eq!(PillAt::of(2), PillAt::Off);
}

#[test]
fn a_game_on_a_monitor_left_of_the_main_one() {
    // a 200 % monitor left of the main one: negative screen x
    let client = PxRect::new(-3840, 0, 3840, 2160);
    let (x, _) = place::pill(client, 2.0, PillAt::TopRight);
    assert_eq!(x, -3840 + 3840 - 48 - 624);
    let (mx, my) = place::center_on(client, 2.0, 500.0, 300.0);
    assert_eq!((mx, my), (-1920 - 1000, 1080 - 600));
}

#[test]
fn the_menu_point_lands_on_the_middle_of_the_game() {
    for scale in [1.0f32, 1.5, 2.0] {
        let client = PxRect::new(10, 20, 1920, 1080);
        let (ax, ay) = (538.0, 300.0);
        let (x, y) = place::center_on(client, scale, ax, ay);
        assert_eq!(x + (ax * scale).round() as i32, 10 + 960);
        assert_eq!(y + (ay * scale).round() as i32, 20 + 540);
    }
}

#[test]
fn zone_edit_maps_window_px_to_capture_px() {
    // borderless 4K game at 150 %: the window is 2560 x 1440 logical, the capture 3840 x 2160
    let m = CaptureMap::new(PxRect::new(0, 0, 3840, 2160), 1.5, 3840, 2160);
    assert_eq!(m.view_size(), (2560.0, 1440.0));
    assert_eq!(m.to_capture(100.0, 200.0), (150.0, 300.0));
    assert_eq!(m.to_view(150.0, 300.0), (100.0, 200.0));
    let r = zone_edit::capture_rect(Rect::new(100.0, 200.0, 400.0, 40.0), &m);
    assert_eq!(r, Rect::new(150.0, 300.0, 600.0, 60.0));
}

#[test]
fn a_game_with_a_title_bar_at_150_and_225_percent() {
    // the capture covers the frame and title bar; zone edit covers the same rect, so a box drawn over a line of
    // the frozen screenshot is saved on that line, whatever the client offset
    for scale in [1.5f32, 2.25] {
        let frame = PxRect::new(16, 0, 2848, 1604);
        let m = CaptureMap::new(frame, scale, 2848, 1604);
        let (lw, lh) = m.view_size();
        assert!((lw * scale - 2848.0).abs() < 0.01 && (lh * scale - 1604.0).abs() < 0.01);
        // a subtitle line at capture px (700, 1400) to (2100, 1460)
        let (vx, vy) = m.to_view(700.0, 1400.0);
        let (cx, cy) = m.to_capture(vx, vy);
        assert!(
            (cx - 700.0).abs() < 0.01 && (cy - 1400.0).abs() < 0.01,
            "{scale}"
        );
        assert!((vx - 700.0 / scale).abs() < 0.01, "{scale}");
    }
    // a screenshot smaller than the window (the game shrank after the shot): it is stretched over the window
    let m = CaptureMap::new(PxRect::new(0, 0, 2000, 1000), 1.0, 1000, 500);
    assert_eq!(m.to_capture(200.0, 100.0), (100.0, 50.0));
}

#[test]
fn zone_edit_adds_picks_and_removes_in_capture_px() {
    let m = CaptureMap::new(PxRect::new(0, 0, 3840, 2160), 2.0, 3840, 2160);
    let mut e = Editor::new(Vec::new(), 3840.0, 2160.0);
    let i = zone_edit::add(
        &mut e,
        &m,
        Rect::new(100.0, 900.0, 800.0, 60.0),
        dr_runtime::Kind::Dialogue,
    );
    assert_eq!(i, Some(0));
    assert_eq!(e.zones[0].rect, Rect::new(200.0, 1800.0, 1600.0, 120.0));
    assert_eq!(e.zones[0].name, "Dialogue");
    // the window shows it where it was drawn
    let v = zone_edit::view_zones(&e, &m);
    assert_eq!(
        (v[0].x, v[0].y, v[0].w, v[0].h),
        (100.0, 900.0, 800.0, 60.0)
    );
    assert_eq!(zone_edit::pick(&e, &m, 150.0, 920.0), Some(0));
    assert_eq!(zone_edit::pick(&e, &m, 10.0, 10.0), None);
    // the dim layer has a hole for it
    assert_eq!(
        zone_edit::dim(&e, &m),
        "M0 0H1920V1080H0ZM100 900V960H900V900Z"
    );
    // a box over the edge is clamped to the picture
    let j = zone_edit::add(
        &mut e,
        &m,
        Rect::new(1800.0, 1000.0, 400.0, 400.0),
        dr_runtime::Kind::Speaker,
    );
    assert_eq!(j, Some(1));
    assert_eq!(e.zones[1].rect, Rect::new(3600.0, 2000.0, 240.0, 160.0));
    assert!(zone_edit::remove(&mut e, 0));
    assert_eq!(e.zones.len(), 1);
    assert!(!zone_edit::remove(&mut e, 5));
}
