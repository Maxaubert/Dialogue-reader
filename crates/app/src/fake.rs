//! Fake data from the approved mockups (Red Dead Redemption 2, five zones, Kokoro · Adam, Mouse 4 and F7).
//! It sits behind the view-model layer (`vm::Source::Fake`): render-screens and the tests use it. With real data
//! (M2), profiles and the window list replace the zones, windows and games; voices (M3a), the activity log
//! (M3b) and zone edit (M4) are still filled from here.

use std::rc::Rc;

use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};

use crate::geom::{self, Rect};
use crate::vm::zones::EdZone;
use crate::{EditZone, GameData, Kind, LineData, MainWindow, VoiceData, WindowData, ZoneData};

/// Zones of the fake scene (1600 x 900 image px): kind, name, x1, y1, x2, y2.
pub const ZONES: [(Kind, &str, f32, f32, f32, f32); 5] = [
    (Kind::Speaker, "Speaker name", 712.0, 754.0, 888.0, 798.0),
    (Kind::Dialogue, "Subtitles", 360.0, 800.0, 1240.0, 850.0),
    (Kind::Pickups, "Item pickups", 88.0, 48.0, 378.0, 132.0),
    (
        Kind::Prompts,
        "Button prompts",
        1228.0,
        690.0,
        1512.0,
        758.0,
    ),
    (Kind::Ignore, "Minimap", 112.0, 682.0, 288.0, 858.0),
];

/// Zone edit frame: the scene at 1440 x 900 is cropped by 80 px on each side; zones keep 12 px from the edge.
const FRAME_W: f32 = 1440.0;
const FRAME_H: f32 = 900.0;
const EDGE: f32 = 12.0;

fn s(v: &str) -> SharedString {
    v.into()
}

fn model<T: Clone + 'static>(v: Vec<T>) -> ModelRc<T> {
    ModelRc::from(Rc::new(VecModel::from(v)))
}

pub fn zones(prompts_on: bool) -> Vec<ZoneData> {
    ZONES
        .iter()
        .map(|&(kind, name, x1, y1, x2, y2)| ZoneData {
            name: s(name),
            kind,
            on: prompts_on || kind != Kind::Prompts,
            x: x1,
            y: y1,
            w: x2 - x1,
            h: y2 - y1,
        })
        .collect()
}

/// The fake zones for the zone editor (1600 x 900 image px).
pub fn ed_zones(prompts_on: bool) -> Vec<EdZone> {
    zones(prompts_on)
        .into_iter()
        .map(|z| EdZone {
            name: z.name.to_string(),
            kind: crate::vm::disk_kind(z.kind),
            on: z.on,
            rect: Rect::new(z.x, z.y, z.w, z.h),
        })
        .collect()
}

fn extra_zones() -> Vec<ZoneData> {
    [
        (Kind::Dialogue, "Letters"),
        (Kind::Pickups, "Shop prices"),
        (Kind::Dialogue, "Journal"),
    ]
    .iter()
    .map(|&(kind, name)| ZoneData {
        name: s(name),
        kind,
        on: true,
        x: 0.0,
        y: 0.0,
        w: 0.0,
        h: 0.0,
    })
    .collect()
}

pub fn edit_zones() -> Vec<EditZone> {
    ZONES
        .iter()
        .map(|&(kind, _, x1, y1, x2, y2)| {
            let (x1, x2) = ((x1 - 80.0).max(EDGE), (x2 - 80.0).min(FRAME_W - EDGE));
            EditZone {
                kind,
                x: x1,
                y: y1,
                w: x2 - x1,
                h: y2 - y1,
            }
        })
        .collect()
}

pub fn windows() -> (Vec<WindowData>, Vec<WindowData>) {
    let w = |name: &str, exe: &str, meta: &str, art: &str| WindowData {
        name: s(name),
        exe: s(exe),
        meta: s(meta),
        art: s(art),
        icon: Default::default(),
    };
    (
        vec![
            w(
                "Red Dead Redemption 2",
                "RDR2.exe",
                "3840 × 2160 · borderless",
                "rdr2",
            ),
            w(
                "Satisfactory",
                "FactoryGame.exe",
                "2560 × 1440 · windowed",
                "satis",
            ),
            w(
                "Persona 4 Golden",
                "P4G.exe",
                "1920 × 1080 · borderless",
                "p4g",
            ),
        ],
        vec![
            w("Discord", "Discord.exe", "", ""),
            w("Spotify", "Spotify.exe", "", ""),
            w("Zen Browser", "zen.exe", "", ""),
        ],
    )
}

fn voices() -> Vec<VoiceData> {
    [
        ("Adam", "American English · Male", "US · Male"),
        ("Bella", "American English · Female", "US · Female"),
        ("Michael", "American English · Male", "US · Male"),
        ("Emma", "British English · Female", "UK · Female"),
        ("George", "British English · Male", "UK · Male"),
        ("Sky", "American English · Female", "US · Female"),
        // below the fold of the list (the header count says 11)
        ("Sarah", "American English · Female", "US · Female"),
        ("Nicole", "American English · Female", "US · Female"),
        ("Eric", "American English · Male", "US · Male"),
        ("Alice", "British English · Female", "UK · Female"),
        ("Lewis", "British English · Male", "UK · Male"),
    ]
    .iter()
    .map(|&(n, d, sh)| VoiceData {
        name: s(n),
        desc: s(d),
        short: s(sh),
    })
    .collect()
}

fn menu_voices() -> Vec<VoiceData> {
    [
        ("Adam", "US · Male"),
        ("Bella", "US · Female"),
        ("Michael", "US · Male"),
        ("Sarah", "US · Female"),
        ("Emma", "UK · Female"),
    ]
    .iter()
    .map(|&(n, sh)| VoiceData {
        name: s(n),
        desc: s(sh),
        short: s(sh),
    })
    .collect()
}

pub fn log() -> Vec<LineData> {
    [
        (
            "21:42:18",
            Kind::Dialogue,
            "Arthur:",
            "We need to get these horses moving before the snow buries us.",
            "",
        ),
        ("21:42:11", Kind::Pickups, "", "+1 Ginseng", ""),
        (
            "21:42:09",
            Kind::Prompts,
            "",
            "Hold E to talk",
            "Prompts off",
        ),
        (
            "21:41:57",
            Kind::Dialogue,
            "Dutch:",
            "We ride at first light. Nobody stays behind.",
            "",
        ),
        ("21:41:50", Kind::Dialogue, "Arthur:", "I hear you.", ""),
        (
            "21:41:49",
            Kind::Dialogue,
            "Arthur:",
            "I hear you.",
            "Already read",
        ),
        ("21:41:32", Kind::Pickups, "", "+2 Yarrow", ""),
        (
            "21:40:58",
            Kind::Dialogue,
            "Sadie:",
            "Then stop talking and start digging.",
            "",
        ),
        ("21:40:41", Kind::Pickups, "", "Donated to camp: $5.00", ""),
        (
            "21:40:12",
            Kind::Dialogue,
            "Charles:",
            "Tracks lead north. Two riders, maybe three.",
            "",
        ),
        ("21:39:55", Kind::Pickups, "", "+1 Venison", ""),
        (
            "21:39:30",
            Kind::Dialogue,
            "Arthur:",
            "Keep your voice down.",
            "",
        ),
        (
            "21:39:02",
            Kind::Dialogue,
            "Javier:",
            "The wagon wheel is cracked. It will not last the night.",
            "",
        ),
    ]
    .iter()
    .map(|&(t, kind, who, line, skip)| LineData {
        time: s(t),
        kind,
        who: s(who),
        line: s(line),
        skip: s(skip),
    })
    .collect()
}

pub fn games() -> Vec<GameData> {
    [
        ("Red Dead Redemption 2", "Reading · 5 zones", "rdr2", true),
        ("Satisfactory", "Paused · 4 zones", "satis", true),
        (
            "Persona 4 Golden",
            "Last played 2 days ago · 3 zones",
            "p4g",
            false,
        ),
        (
            "Clair Obscur: Expedition 33",
            "Last played last week · 4 zones",
            "e33",
            false,
        ),
        (
            "Kingdom Come: Deliverance II",
            "Last played Oct 30 · 2 zones",
            "kcd2",
            false,
        ),
    ]
    .iter()
    .map(|&(n, m, a, r)| GameData {
        name: s(n),
        meta: s(m),
        art: s(a),
        running: r,
        cover: Default::default(),
    })
    .collect()
}

fn rect(z: &EditZone) -> Rect {
    Rect::new(z.x, z.y, z.w, z.h)
}

fn refresh_dim(w: &MainWindow) {
    let rects: Vec<Rect> = w.get_edit_zones().iter().map(|z| rect(&z)).collect();
    w.set_edit_dim(geom::dim_path(FRAME_W, FRAME_H, &rects).into());
}

/// Puts the fake data that has no real source yet into the window and wires the zone edit callbacks.
/// Zones, windows and games come through the view-model layer (`vm::install`).
pub fn fill(w: &MainWindow) {
    w.set_extra_zones(model(extra_zones()));
    w.set_voices(model(voices()));
    w.set_menu_voices(model(menu_voices()));
    let log = log();
    w.set_recent(model(log[..5].to_vec()));
    w.set_log(model(log));
    w.set_edit_zones(model(edit_zones()));
    w.set_version(env!("CARGO_PKG_VERSION").into());
    refresh_dim(w);

    let weak = w.as_weak();
    w.on_edit_pick(move |x, y| {
        let Some(w) = weak.upgrade() else { return -1 };
        let rects: Vec<Rect> = w.get_edit_zones().iter().map(|z| rect(&z)).collect();
        geom::hit_zone(&rects, x, y).map_or(-1, |i| i as i32)
    });
    let weak = w.as_weak();
    w.on_edit_remove(move |i| {
        let Some(w) = weak.upgrade() else { return };
        let zones = w.get_edit_zones();
        if let Some(vm) = zones.as_any().downcast_ref::<VecModel<EditZone>>()
            && i >= 0
            && (i as usize) < vm.row_count()
        {
            vm.remove(i as usize);
            w.set_edit_selected(-1);
            refresh_dim(&w);
        }
    });
    let weak = w.as_weak();
    w.on_edit_commit(move |kind, x, y, ww, hh| {
        let Some(w) = weak.upgrade() else { return };
        let bounds = Rect::new(0.0, 0.0, FRAME_W, FRAME_H);
        let Some(r) = geom::drag_to_rect(x, y, x + ww, y + hh, bounds) else {
            return;
        };
        let zones = w.get_edit_zones();
        if let Some(vm) = zones.as_any().downcast_ref::<VecModel<EditZone>>() {
            vm.push(EditZone {
                kind,
                x: r.x,
                y: r.y,
                w: r.w,
                h: r.h,
            });
            w.set_edit_selected(vm.row_count() as i32 - 1);
            w.set_edit_draft(false);
            refresh_dim(&w);
        }
    });
}
