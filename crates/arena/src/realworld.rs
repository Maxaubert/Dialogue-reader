//! `arena realworld-import`: builds the real-world recorded set from a recorder session.
//!
//! Input: a recorder session folder with `session.json`, `marks/mark_NNN/fNNN_+OFFSETms.png`
//! (full resolution, offsets relative to the mark key press) and `marks_measured.json` (pop-ups
//! measured by hand: text, kind, 4K bbox, first and last visible ms).
//!
//! Output, in `<out>/<game>/`:
//! - `frames/mark_NNN/fNNN_+OFFSETms.jpg`: frames scaled to 1920 px wide, JPEG.
//! - `mark_NNN.recorded.json`: one `RecordedScenario` per mark (truth plus a user-style layout).
//! - `import.json`: where each mark key press sits in the new time base, the marked item ids,
//!   frame gaps and the source.
//!
//! Time base: t = 0 is the first frame of each mark. Frame times keep the true capture
//! timestamps, so dropped frames stay visible as gaps.

use crate::model::{Align, Anim, ItemKind, Layout, RecordedScenario, Rect, Style, TextItem, Zone, ZoneKind};
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(clap::Args, Debug, Clone)]
pub struct Args {
    /// Recorder session folder (has session.json, marks/ and marks_measured.json)
    #[arg(long)]
    pub recording: Option<PathBuf>,
    /// Output folder for the recorded set; the set goes in <out>/<game name>
    #[arg(long)]
    pub out: Option<PathBuf>,
    /// Width of the written frames (height keeps the aspect ratio)
    #[arg(long, default_value_t = 1920)]
    pub width: u32,
    /// JPEG quality of the written frames
    #[arg(long, default_value_t = 90)]
    pub quality: u8,
    /// Rewrite frames that already exist (by default they are kept, so a rerun only rebuilds truth)
    #[arg(long)]
    pub force_frames: bool,
    /// Worker threads for frame conversion
    #[arg(long, default_value_t = 8)]
    pub jobs: usize,
}

// ---------------------------------------------------------------------------------------------
// Input formats

#[derive(Deserialize, Debug, Clone)]
struct Session {
    name: String,
    #[serde(default)]
    window_size: Option<[u32; 2]>,
}

/// One mark of `marks_measured.json`.
#[derive(Deserialize, Debug, Clone)]
pub struct MeasuredMark {
    pub mark: u32,
    #[serde(default)]
    pub popups: Vec<Popup>,
    #[serde(default)]
    pub notes: String,
}

/// One measured pop-up. `bbox_4k` is `[x0, y0, x1, y1]` in source pixels.
#[derive(Deserialize, Debug, Clone)]
pub struct Popup {
    pub text: String,
    pub kind: String,
    pub bbox_4k: [f32; 4],
    pub first_visible_ms: f64,
    pub last_visible_ms: f64,
    #[serde(default)]
    pub likely_the_marked_one: bool,
}

// ---------------------------------------------------------------------------------------------
// Output manifest

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ImportManifest {
    pub game: String,
    pub source: String,
    pub source_size: [u32; 2],
    pub size: [u32; 2],
    pub time_base: String,
    pub marks: Vec<MarkInfo>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MarkInfo {
    pub id: String,
    pub mark: u32,
    pub file: String,
    pub frames: usize,
    pub duration_ms: f64,
    /// Time of the mark key press in this scenario's time base.
    pub mark_key_ms: f64,
    /// Truth items the measurer thought the mark was for.
    pub marked_item_ids: Vec<String>,
    pub max_frame_gap_ms: f64,
    /// Gaps longer than 250 ms as `[from, to]` in this scenario's time base.
    pub long_gaps_ms: Vec<[f64; 2]>,
    /// Truth ids whose start is not exact (on the first frame, or right after a long gap).
    #[serde(default)]
    pub start_uncertain: Vec<String>,
    /// Truth ids whose end is not exact (on the last frame, or gone after a long gap).
    #[serde(default)]
    pub end_uncertain: Vec<String>,
    pub notes: String,
}

// ---------------------------------------------------------------------------------------------
// Per-game knowledge: HUD text and the zones a user would draw

/// What the importer knows about a game beyond the measurements. Rects are in source pixels
/// (3840x2160) as `[x0, y0, x1, y1]`.
struct GameProfile {
    hud: Vec<(&'static str, [f32; 4])>,
    zones: Vec<(&'static str, ZoneKind, [f32; 4])>,
}

fn game_profile(game: &str) -> Option<GameProfile> {
    match game {
        // Measured from the satisfactory-20261121-183704 frames (marks_measured.json static_hud_text
        // and contact sheets, 2026-11-21). The HUD is on screen in every mark; the objective is
        // blurred while the pause menu is open (marks 6 and 7) but stays in place.
        "satisfactory" => Some(GameProfile {
            hud: vec![("1.2.5.1 CL#520140", [365.0, 30.0, 625.0, 62.0]), ("Objective: Build the HUB", [2830.0, 125.0, 3705.0, 300.0])],
            zones: vec![
                // Pickup toasts sit at x 40-800, y 1272-1480; the zone leaves room for a taller stack.
                ("notifications", ZoneKind::Notification, [20.0, 1100.0, 860.0, 1560.0]),
                // Interaction prompts sit at x 1470-2370, y 1190-1300 (centre, below the crosshair).
                ("prompt", ZoneKind::Prompt, [1300.0, 1120.0, 2540.0, 1360.0]),
            ],
        }),
        _ => None,
    }
}

// ---------------------------------------------------------------------------------------------
// Turning measured pop-ups into truth items

/// One piece of truth before times are applied: text, kind and a rect in source pixels.
#[derive(Debug, Clone, PartialEq)]
struct Piece {
    text: String,
    kind: ItemKind,
    rect: [f32; 4],
}

const PAUSE_MENU: [&str; 10] = [
    "Resume Game",
    "Save Game",
    "Load Game",
    "Manage Session",
    "Go to feedback site",
    "Options",
    "Server Manager",
    "Exit to Main Menu",
    "Exit to Desktop",
    "Respawn",
];
const SESSION_PANEL: [&str; 4] = ["Current Session: testt", "Show Seed", "Max", "Version: 1.2.5.1 CL#520140"];
/// Row centres (source y) of the pause menu entries, read from mark 6 frames; the first three
/// entries use a larger font.
const PAUSE_MENU_ROWS_Y: [f32; 10] = [312.0, 424.0, 536.0, 684.0, 764.0, 844.0, 924.0, 1056.0, 1136.0, 1272.0];
const SESSION_PANEL_RECT: [f32; 4] = [0.0, 1660.0, 680.0, 2060.0];

/// Maps a measured kind to an item kind. Menus, wheels and search panels ("other") are menus.
fn map_kind(kind: &str) -> ItemKind {
    let k = kind.to_ascii_lowercase();
    if k.contains("toast") {
        ItemKind::PickupToast
    } else if k.contains("prompt") {
        ItemKind::Prompt
    } else if k.contains("tooltip") {
        ItemKind::Tooltip
    } else {
        ItemKind::Menu
    }
}

/// Splits `rect` into `n` equal rows (top to bottom).
fn split_rows(rect: [f32; 4], n: usize) -> Vec<[f32; 4]> {
    let h = (rect[3] - rect[1]) / n as f32;
    (0..n).map(|i| [rect[0], rect[1] + h * i as f32, rect[2], rect[1] + h * (i + 1) as f32]).collect()
}

/// Rects of the pause menu entries, x 85-470 in source pixels.
fn pause_menu_rows() -> Vec<[f32; 4]> {
    PAUSE_MENU_ROWS_Y
        .iter()
        .enumerate()
        .map(|(i, &y)| {
            let h = if i < 3 { 100.0 } else { 70.0 };
            [85.0, y - h / 2.0, 470.0, y + h / 2.0]
        })
        .collect()
}

/// Splits `rect` into `n` equal columns (left to right).
fn split_cols(rect: [f32; 4], n: usize) -> Vec<[f32; 4]> {
    let w = (rect[2] - rect[0]) / n as f32;
    (0..n).map(|i| [rect[0] + w * i as f32, rect[1], rect[0] + w * (i + 1) as f32, rect[3]]).collect()
}

fn pieces_from(texts: &[&str], kind: ItemKind, rects: Vec<[f32; 4]>) -> Vec<Piece> {
    texts.iter().zip(rects).map(|(t, r)| Piece { text: t.trim().to_string(), kind, rect: r }).collect()
}

/// Turns one measured pop-up into the text pieces a reader could see on screen.
///
/// The measurements mix literal text with descriptions, so some known descriptions are replaced
/// by the literal lines seen in the frames. Multi-line pop-ups become one item per line. Rows are
/// the bbox split evenly (close for toast stacks and lists), except the pause menu, whose rows
/// were read from the frames.
fn popup_pieces(p: &Popup) -> Vec<Piece> {
    let text = p.text.trim();
    let kind = map_kind(&p.kind);
    let bbox = p.bbox_4k;

    // Pause menu (mark 6): a description, not literal text.
    if text.starts_with("Pause menu:") {
        return pieces_from(&PAUSE_MENU, ItemKind::Menu, pause_menu_rows());
    }
    // Session side panel of the pause menu (mark 6).
    if text.starts_with("Current Session:") {
        return pieces_from(&SESSION_PANEL, ItemKind::Menu, split_rows(bbox, SESSION_PANEL.len()));
    }
    // Pause menu plus side panel in one description (mark 7). The version string at the top is
    // the static HUD item, so it is not repeated here.
    if text.starts_with("Resume Game /") {
        let mut v = pieces_from(&PAUSE_MENU, ItemKind::Menu, pause_menu_rows());
        v.extend(pieces_from(&SESSION_PANEL, ItemKind::Menu, split_rows(SESSION_PANEL_RECT, SESSION_PANEL.len())));
        return v;
    }
    // Quick search title (mark 11); the logo reads FICSIT.
    if text.starts_with("FICSIT QUICK SEARCH") {
        return vec![Piece { text: "FICSIT QUICK SEARCH".into(), kind: ItemKind::Menu, rect: [bbox[0], bbox[1], bbox[2], bbox[1] + 60.0] }];
    }
    // A pop-up hidden behind a blurred menu cannot be read; reading it counts as noise.
    if let Some(stripped) = text.strip_suffix("(blurred, unreadable)") {
        return vec![Piece { text: stripped.trim().to_string(), kind: ItemKind::Menu, rect: bbox }];
    }
    match kind {
        // Stacked toast rows: "+ 9 Leaves (9)  /  + 1 Wood (1)".
        ItemKind::PickupToast if text.contains("  /  ") => {
            let rows: Vec<&str> = text.split("  /  ").collect();
            pieces_from(&rows, kind, split_rows(bbox, rows.len()))
        }
        // A control-hint bar of "Label / Key" pairs separated by wide gaps (mark 3). It is static
        // UI, not a pop-up to speak, so each hint is HUD text (should_read false).
        ItemKind::Tooltip if text.contains("   ") && text.contains(" / ") => {
            let hints: Vec<String> = text.split("   ").filter(|s| !s.trim().is_empty()).map(|s| s.trim().replace(" / ", " ")).collect();
            let refs: Vec<&str> = hints.iter().map(|s| s.as_str()).collect();
            pieces_from(&refs, ItemKind::HudStatic, split_cols(bbox, refs.len()))
        }
        // A list of search results (mark 11, measured as a tooltip). Search panels are menus.
        ItemKind::Tooltip if text.contains(" / ") => {
            let rows: Vec<&str> = text.split(" / ").collect();
            pieces_from(&rows, ItemKind::Menu, split_rows(bbox, rows.len()))
        }
        // Single-line menus such as the wheel's "Cancel".
        _ => vec![Piece { text: text.to_string(), kind, rect: bbox }],
    }
}

/// The name after the count of a toast row: "+ 12 Iron Ore (33)" gives "iron ore".
fn toast_item_name(text: &str) -> String {
    let n = crate::text::normalize(text);
    n.split(' ').filter(|w| !w.chars().all(|c| c.is_ascii_digit())).collect::<Vec<_>>().join(" ")
}

fn style_for(kind: ItemKind, rect: &Rect) -> Style {
    // The real game's font is unknown to the arena; the style only documents roughly how it looks.
    Style {
        font: "Satisfactory UI (recorded)".into(),
        size_px: (rect.h * 0.45).clamp(8.0, 40.0),
        color: [255, 255, 255, 255],
        outline: None,
        shadow: None,
        panel: None,
        bold: matches!(kind, ItemKind::PickupToast),
        italic: false,
        align: Align::Left,
        line_spacing: 1.25,
    }
}

/// Frame times of one mark in the recorder time base (mark key press = 0), sorted.
pub struct MarkFrames {
    pub times: Vec<f64>,
}

impl MarkFrames {
    /// Time of the first frame after `t`, or `t` plus the usual frame spacing at the end.
    fn next_after(&self, t: f64) -> f64 {
        self.times.iter().copied().find(|&x| x > t + 0.5).unwrap_or(t + self.typical_step())
    }
    /// Frame time nearest to `t`.
    fn snap(&self, t: f64) -> f64 {
        self.times.iter().copied().min_by(|a, b| (a - t).abs().total_cmp(&(b - t).abs())).unwrap_or(t)
    }
    fn typical_step(&self) -> f64 {
        let mut d: Vec<f64> = self.times.windows(2).map(|w| w[1] - w[0]).collect();
        if d.is_empty() {
            return 100.0;
        }
        d.sort_by(f64::total_cmp);
        d[d.len() / 2]
    }
    fn first(&self) -> f64 {
        self.times.first().copied().unwrap_or(0.0)
    }
    fn end(&self) -> f64 {
        self.times.last().copied().unwrap_or(0.0) + self.typical_step()
    }
}

/// Builds the truth of one mark. Times are shifted so the first frame is t = 0; `scale` maps
/// source pixels to output pixels. Returns the items and the ids of the marked ones.
pub fn build_truth(m: &MeasuredMark, frames: &MarkFrames, scale: f32, game: &str) -> (Vec<TextItem>, Vec<String>) {
    let t0 = frames.first();
    let to_rect = |r: [f32; 4]| Rect::new(r[0], r[1], r[2] - r[0], r[3] - r[1]).scale(scale, scale);
    let mut items: Vec<TextItem> = Vec::new();
    let mut marked = Vec::new();

    for (pi, p) in m.popups.iter().enumerate() {
        let start = frames.snap(p.first_visible_ms);
        // Visible through the last measured frame; gone at the next captured frame.
        let end = frames.next_after(frames.snap(p.last_visible_ms.max(p.first_visible_ms)));
        let pieces = popup_pieces(p);
        let many = pieces.len() > 1;
        for (k, piece) in pieces.into_iter().enumerate() {
            let id = if many { format!("m{:02}-p{}-{}", m.mark, pi, k) } else { format!("m{:02}-p{}", m.mark, pi) };
            let rect = to_rect(piece.rect);
            if p.likely_the_marked_one {
                marked.push(id.clone());
            }
            items.push(TextItem {
                id,
                should_read: piece.kind.default_should_read(),
                style: style_for(piece.kind, &rect),
                text: piece.text,
                kind: piece.kind,
                rect,
                start_ms: start - t0,
                end_ms: end - t0,
                anim: Anim::default(),
            });
        }
    }

    // A toast whose count changes in place ("+ 6 Iron Ore (27)" then "+ 12 Iron Ore (33)") is a
    // replacement: same item name, same place, starting when (or just after) the other ends.
    for i in 0..items.len() {
        if items[i].kind != ItemKind::PickupToast {
            continue;
        }
        let name = toast_item_name(&items[i].text);
        let cur = &items[i];
        let gap = |o: &TextItem| (cur.start_ms - o.end_ms).abs();
        let prev = items
            .iter()
            .filter(|o| {
                o.id != cur.id
                    && o.kind == ItemKind::PickupToast
                    && o.text != cur.text
                    && toast_item_name(&o.text) == name
                    && gap(o) <= 250.0
                    && o.rect.intersection(&cur.rect) > 0.5 * cur.rect.area()
            })
            .min_by(|a, b| gap(a).total_cmp(&gap(b)))
            .map(|o| o.id.clone());
        items[i].anim.replace_of = prev;
    }

    // Static HUD text, on screen for the whole mark.
    if let Some(profile) = game_profile(game) {
        for (k, (text, r)) in profile.hud.iter().enumerate() {
            let rect = to_rect(*r);
            items.push(TextItem {
                id: format!("m{:02}-hud{}", m.mark, k),
                text: text.to_string(),
                kind: ItemKind::HudStatic,
                should_read: false,
                style: style_for(ItemKind::HudStatic, &rect),
                rect,
                start_ms: 0.0,
                end_ms: frames.end() - t0,
                anim: Anim::default(),
            });
        }
    }
    (items, marked)
}

/// A frame gap longer than this makes the times next to it uncertain (same as `long_gaps_ms`).
pub const LONG_GAP_MS: f64 = 250.0;

/// Truth items whose times are not exact, as `(start_uncertain, end_uncertain)` ids. A start is
/// uncertain when the item is already there on the first frame or appears right after a long
/// frame gap; an end when it is still there on the last frame or is gone after a long gap. HUD
/// items are left out (they are never read). Times are in the scenario base (first frame = 0).
pub fn uncertain_times(items: &[TextItem], frames: &MarkFrames) -> (Vec<String>, Vec<String>) {
    let t0 = frames.first();
    let times: Vec<f64> = frames.times.iter().map(|t| t - t0).collect();
    let last = times.last().copied().unwrap_or(0.0);
    let gap_before = |t: f64| {
        let k = times.iter().position(|&x| (x - t).abs() < 0.5);
        k.filter(|&k| k > 0).map(|k| times[k] - times[k - 1]).unwrap_or(0.0)
    };
    let (mut start, mut end) = (Vec::new(), Vec::new());
    for it in items.iter().filter(|i| i.kind != ItemKind::HudStatic) {
        if it.start_ms <= 0.5 || gap_before(it.start_ms) > LONG_GAP_MS {
            start.push(it.id.clone());
        }
        // `end_ms` is the first frame without the item, so the gap before it is the one it ends in.
        if it.end_ms > last + 0.5 || gap_before(it.end_ms) > LONG_GAP_MS {
            end.push(it.id.clone());
        }
    }
    (start, end)
}

/// Zones a user would draw for this game, in output pixels.
pub fn game_layout(game: &str, scale: f32) -> Layout {
    let zones = game_profile(game)
        .map(|p| {
            p.zones
                .iter()
                .map(|(name, kind, r)| Zone {
                    name: name.to_string(),
                    kind: *kind,
                    rect: Rect::new(r[0], r[1], r[2] - r[0], r[3] - r[1]).scale(scale, scale),
                })
                .collect()
        })
        .unwrap_or_default();
    Layout { zones }
}

// ---------------------------------------------------------------------------------------------
// Frames

/// The recorder frames of one mark folder, `(offset_ms, path)` sorted by time.
fn list_mark_frames(dir: &Path) -> Result<Vec<(f64, PathBuf)>> {
    let mut v: Vec<(f64, PathBuf)> = std::fs::read_dir(dir)
        .with_context(|| format!("read {}", dir.display()))?
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            if !name.ends_with(".png") {
                return None;
            }
            crate::runner::parse_offset_ms(&name).map(|t| (t, e.path()))
        })
        .collect();
    v.sort_by(|a, b| a.0.total_cmp(&b.0));
    if v.is_empty() {
        bail!("no fNNN_+OFFSETms.png frames in {}", dir.display());
    }
    Ok(v)
}

/// Downscales RGB8 by an area average. Exact for integer factors; for other factors it falls
/// back to bilinear.
pub fn downscale_rgb(src: &[u8], sw: u32, sh: u32, dw: u32, dh: u32) -> Vec<u8> {
    if sw == dw && sh == dh {
        return src.to_vec();
    }
    if sw.is_multiple_of(dw) && sh.is_multiple_of(dh) && sw / dw == sh / dh {
        let f = (sw / dw) as usize;
        let (sw, dw, dh) = (sw as usize, dw as usize, dh as usize);
        let n = (f * f) as u32;
        let mut out = vec![0u8; dw * dh * 3];
        for y in 0..dh {
            for x in 0..dw {
                for c in 0..3 {
                    let mut sum = 0u32;
                    for yy in 0..f {
                        let row = (y * f + yy) * sw;
                        for xx in 0..f {
                            sum += src[(row + x * f + xx) * 3 + c] as u32;
                        }
                    }
                    out[(y * dw + x) * 3 + c] = ((sum + n / 2) / n) as u8;
                }
            }
        }
        return out;
    }
    let rgba: Vec<u8> = src.as_chunks::<3>().0.iter().flat_map(|&[r, g, b]| [r, g, b, 255]).collect();
    let r = crate::background::resize_bilinear(&rgba, sw, sh, dw, dh);
    r.as_chunks::<4>().0.iter().flat_map(|&[r, g, b, _]| [r, g, b]).collect()
}

fn convert_frame(src: &Path, dst: &Path, dw: u32, dh: u32, quality: u8) -> Result<()> {
    let img = image::open(src).with_context(|| format!("decode {}", src.display()))?.to_rgb8();
    let (sw, sh) = img.dimensions();
    let small = downscale_rgb(img.as_raw(), sw, sh, dw, dh);
    let tmp = dst.with_extension("jpg.tmp");
    {
        let file = std::fs::File::create(&tmp).with_context(|| format!("create {}", tmp.display()))?;
        let mut w = std::io::BufWriter::new(file);
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut w, quality).encode(&small, dw, dh, image::ExtendedColorType::Rgb8)?;
    }
    std::fs::rename(&tmp, dst)?;
    Ok(())
}

/// Output frame name in the new time base, e.g. `f003_+00312ms.jpg`.
pub fn frame_name(i: usize, t_ms: f64) -> String {
    format!("f{:03}_{:+06}ms.jpg", i, t_ms.round() as i64)
}

// ---------------------------------------------------------------------------------------------
// Command

pub fn run(args: &Args) -> Result<()> {
    let rec = args.recording.clone().context("give --recording <recorder session folder>")?;
    let session: Session = serde_json::from_str(
        &std::fs::read_to_string(rec.join("session.json")).with_context(|| format!("read {}/session.json", rec.display()))?,
    )?;
    let measured: Vec<MeasuredMark> = serde_json::from_str(
        &std::fs::read_to_string(rec.join("marks_measured.json")).with_context(|| format!("read {}/marks_measured.json", rec.display()))?,
    )
    .context("parse marks_measured.json")?;
    let game = session.name.trim().to_lowercase();
    if game_profile(&game).is_none() {
        eprintln!("warning: no game profile for '{game}': no static HUD items and an empty layout");
    }
    let out_root = args.out.clone().unwrap_or_else(|| crate::data_dir().join("realworld"));
    let out = out_root.join(&game);
    std::fs::create_dir_all(&out)?;

    let mut manifest = ImportManifest {
        game: game.clone(),
        source: crate::portable_path(&rec),
        source_size: [0, 0],
        size: [0, 0],
        time_base: "t = 0 is the first frame of each mark; mark_key_ms is the mark key press. Notes keep the recorder base (mark key press = 0)".into(),
        marks: vec![],
    };
    let mut total_frames = 0usize;
    let mut converted = 0usize;

    for m in &measured {
        let mark_dir = rec.join("marks").join(format!("mark_{:03}", m.mark));
        let src_frames = list_mark_frames(&mark_dir)?;
        let (sw, sh) = match session.window_size {
            Some([w, h]) => (w, h),
            None => image::image_dimensions(&src_frames[0].1)?,
        };
        let dw = args.width.min(sw);
        let dh = ((sh as f64) * dw as f64 / sw as f64).round() as u32;
        let scale = dw as f32 / sw as f32;
        manifest.source_size = [sw, sh];
        manifest.size = [dw, dh];

        let frames = MarkFrames { times: src_frames.iter().map(|f| f.0).collect() };
        let t0 = frames.first();
        let id = format!("{game}-m{:02}", m.mark);
        let frames_rel = format!("frames/mark_{:03}", m.mark);
        let frames_dir = out.join(&frames_rel);
        std::fs::create_dir_all(&frames_dir)?;

        // Remove frames of an older import with a different time base or count.
        let wanted: Vec<String> = src_frames.iter().enumerate().map(|(i, (t, _))| frame_name(i, t - t0)).collect();
        for e in std::fs::read_dir(&frames_dir)?.flatten() {
            let n = e.file_name().to_string_lossy().to_string();
            if !wanted.contains(&n) {
                std::fs::remove_file(e.path())?;
            }
        }
        let todo: Vec<usize> = (0..src_frames.len()).filter(|&i| args.force_frames || !frames_dir.join(&wanted[i]).exists()).collect();
        crate::runner::parallel(todo.len(), args.jobs, |k| {
            let i = todo[k];
            convert_frame(&src_frames[i].1, &frames_dir.join(&wanted[i]), dw, dh, args.quality)
        })?;
        converted += todo.len();
        total_frames += src_frames.len();

        let (truth, marked) = build_truth(m, &frames, scale, &game);
        let (start_uncertain, end_uncertain) = uncertain_times(&truth, &frames);
        let scenario = RecordedScenario {
            id: id.clone(),
            frames_dir: frames_rel,
            truth,
            layout: game_layout(&game, scale),
            start_uncertain: start_uncertain.clone(),
            end_uncertain: end_uncertain.clone(),
        };
        let file = format!("mark_{:03}.recorded.json", m.mark);
        std::fs::write(out.join(&file), serde_json::to_string_pretty(&scenario)?)?;

        let gaps: Vec<[f64; 2]> = frames.times.windows(2).filter(|w| w[1] - w[0] > LONG_GAP_MS).map(|w| [w[0] - t0, w[1] - t0]).collect();
        let max_gap = frames.times.windows(2).map(|w| w[1] - w[0]).fold(0.0, f64::max);
        println!(
            "{id}: {} frames, {:.1} s, {} truth items ({} to read, {} prompts), {} long gaps (max {:.0} ms)",
            src_frames.len(),
            (frames.end() - t0) / 1000.0,
            scenario.truth.len(),
            scenario.truth.iter().filter(|t| t.should_read).count(),
            scenario.truth.iter().filter(|t| t.kind == ItemKind::Prompt).count(),
            gaps.len(),
            max_gap
        );
        manifest.marks.push(MarkInfo {
            id,
            mark: m.mark,
            file,
            frames: src_frames.len(),
            duration_ms: frames.end() - t0,
            mark_key_ms: -t0,
            marked_item_ids: marked,
            max_frame_gap_ms: max_gap,
            long_gaps_ms: gaps,
            start_uncertain,
            end_uncertain,
            notes: m.notes.clone(),
        });
    }
    std::fs::write(out.join("import.json"), serde_json::to_string_pretty(&manifest)?)?;
    println!("wrote {} marks, {} frames ({} converted) to {}", manifest.marks.len(), total_frames, converted, out.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn popup(text: &str, kind: &str, bbox: [f32; 4], a: f64, b: f64) -> Popup {
        Popup { text: text.into(), kind: kind.into(), bbox_4k: bbox, first_visible_ms: a, last_visible_ms: b, likely_the_marked_one: false }
    }

    fn frames() -> MarkFrames {
        // 100 ms spacing from -1000 to 1000 with a dropped stretch between -300 and 200.
        let times = (-10..=10).map(|i| i as f64 * 100.0).filter(|t| !(*t > -300.0 && *t < 200.0)).collect();
        MarkFrames { times }
    }

    #[test]
    fn kinds_map_as_specified() {
        assert_eq!(map_kind("pickup toast"), ItemKind::PickupToast);
        assert_eq!(map_kind("interaction prompt"), ItemKind::Prompt);
        assert_eq!(map_kind("tooltip"), ItemKind::Tooltip);
        assert_eq!(map_kind("other (menu overlay)"), ItemKind::Menu);
        assert_eq!(map_kind("other"), ItemKind::Menu);
    }

    #[test]
    fn stacked_toast_splits_into_rows() {
        let p = popup("+ 9 Leaves (9)  /  + 1 Wood (1)", "pickup toast", [0.0, 0.0, 100.0, 200.0], 0.0, 100.0);
        let v = popup_pieces(&p);
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].text, "+ 9 Leaves (9)");
        assert_eq!(v[1].text, "+ 1 Wood (1)");
        assert_eq!(v[1].rect, [0.0, 100.0, 100.0, 200.0]);
    }

    #[test]
    fn hint_bar_is_hud_and_search_list_is_menu() {
        let p = popup("Dismantle / Hold LMB   Close / F/Escape", "tooltip", [0.0, 0.0, 200.0, 10.0], 0.0, 100.0);
        let v = popup_pieces(&p);
        assert_eq!(v.iter().map(|x| x.text.as_str()).collect::<Vec<_>>(), ["Dismantle Hold LMB", "Close F/Escape"]);
        assert!(v.iter().all(|x| x.kind == ItemKind::HudStatic));
        let s = popup("Smelter / Biomass", "tooltip", [0.0, 0.0, 200.0, 10.0], 0.0, 100.0);
        assert!(popup_pieces(&s).iter().all(|x| x.kind == ItemKind::Menu));
        let b = popup("Press E to mine (blurred, unreadable)", "interaction prompt", [0.0; 4], 0.0, 0.0);
        assert_eq!(popup_pieces(&b), vec![Piece { text: "Press E to mine".into(), kind: ItemKind::Menu, rect: [0.0; 4] }]);
    }

    #[test]
    fn truth_times_start_at_first_frame_and_end_at_next_frame() {
        let m = MeasuredMark {
            mark: 1,
            notes: String::new(),
            popups: vec![
                popup("Press E to pick up Leaves", "interaction prompt", [1630.0, 1190.0, 2210.0, 1290.0], -800.0, -400.0),
                popup("+ 1 Wood (1)", "pickup toast", [40.0, 1360.0, 800.0, 1480.0], 300.0, 300.0),
            ],
        };
        let (items, _) = build_truth(&m, &frames(), 0.5, "satisfactory");
        let prompt = &items[0];
        assert_eq!(prompt.kind, ItemKind::Prompt);
        assert!(!prompt.should_read);
        // -800 is 200 ms after the first frame (-1000); -400 is last seen, -300 is the next frame.
        assert_eq!((prompt.start_ms, prompt.end_ms), (200.0, 700.0));
        assert_eq!(prompt.rect, Rect::new(815.0, 595.0, 290.0, 50.0));
        // A one-frame toast still lasts until the next frame. The dropped stretch is kept as is.
        let toast = &items[1];
        assert!(toast.should_read);
        assert_eq!((toast.start_ms, toast.end_ms), (1300.0, 1400.0));
        // Two static HUD items covering the whole mark (last frame 1000 plus one 100 ms step).
        let hud: Vec<_> = items.iter().filter(|i| i.kind == ItemKind::HudStatic).collect();
        assert_eq!(hud.len(), 2);
        assert!(hud.iter().all(|h| !h.should_read && h.start_ms == 0.0 && h.end_ms == 2100.0));
    }

    #[test]
    fn times_next_to_gaps_and_edges_are_uncertain() {
        // Frames every 100 ms from -1000 to 1000, none between -300 and 200 (a 500 ms gap).
        let m = MeasuredMark {
            mark: 2,
            notes: String::new(),
            popups: vec![
                popup("+ 1 Wood (1)", "pickup toast", [0.0; 4], -1000.0, -700.0), // from the first frame
                popup("+ 2 Wood (2)", "pickup toast", [0.0; 4], 200.0, 400.0),    // right after the gap
                popup("+ 3 Wood (3)", "pickup toast", [0.0; 4], -600.0, -400.0),  // clean
                popup("+ 4 Wood (4)", "pickup toast", [0.0; 4], -500.0, -300.0),  // gone after the gap
                popup("+ 5 Wood (5)", "pickup toast", [0.0; 4], 800.0, 1000.0),   // to the last frame
            ],
        };
        let (items, _) = build_truth(&m, &frames(), 0.5, "other-game");
        let (start, end) = uncertain_times(&items, &frames());
        assert_eq!(start, ["m02-p0", "m02-p1"]);
        assert_eq!(end, ["m02-p3", "m02-p4"]);
    }

    #[test]
    fn counter_change_is_a_replacement() {
        let r = [40.0, 1360.0, 800.0, 1480.0];
        let m = MeasuredMark {
            mark: 9,
            notes: String::new(),
            popups: vec![
                popup("+ 2 Leaves (18)", "pickup toast", r, -1000.0, -900.0),
                popup("+ 6 Iron Ore (27)", "pickup toast", r, 200.0, 400.0),
                popup("+ 12 Iron Ore (33)", "pickup toast", r, 500.0, 900.0),
            ],
        };
        let (items, _) = build_truth(&m, &frames(), 0.5, "other-game");
        assert_eq!(items.len(), 3, "no HUD items for an unknown game");
        assert_eq!(items[0].anim.replace_of, None);
        assert_eq!(items[1].anim.replace_of, None);
        assert_eq!(items[2].anim.replace_of.as_deref(), Some("m09-p1"));
    }

    #[test]
    fn downscale_averages_blocks() {
        // 2x2 RGB image to 1x1.
        let src = [0, 0, 0, 100, 100, 100, 200, 200, 200, 100, 100, 100];
        assert_eq!(downscale_rgb(&src, 2, 2, 1, 1), vec![100, 100, 100]);
        assert_eq!(frame_name(3, 312.0), "f003_+00312ms.jpg");
        assert_eq!(frame_name(0, 0.0), "f000_+00000ms.jpg");
    }
}
