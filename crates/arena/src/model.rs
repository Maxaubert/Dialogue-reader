//! Scenario types: the scene description and its answer key (serde JSON).

use serde::{Deserialize, Serialize};

/// Axis-aligned rectangle in pixels of the scenario frame.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Default)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Rect { x, y, w, h }
    }
    pub fn right(&self) -> f32 {
        self.x + self.w
    }
    pub fn bottom(&self) -> f32 {
        self.y + self.h
    }
    pub fn area(&self) -> f32 {
        self.w.max(0.0) * self.h.max(0.0)
    }
    pub fn center(&self) -> (f32, f32) {
        (self.x + self.w / 2.0, self.y + self.h / 2.0)
    }
    pub fn contains(&self, px: f32, py: f32) -> bool {
        px >= self.x && px < self.right() && py >= self.y && py < self.bottom()
    }
    /// Overlap area of two rects (0 when they do not touch).
    pub fn intersection(&self, o: &Rect) -> f32 {
        let w = self.right().min(o.right()) - self.x.max(o.x);
        let h = self.bottom().min(o.bottom()) - self.y.max(o.y);
        if w <= 0.0 || h <= 0.0 { 0.0 } else { w * h }
    }
    pub fn intersects(&self, o: &Rect) -> bool {
        self.intersection(o) > 0.0
    }
    /// Grows the rect by `m` pixels on every side.
    pub fn inflate(&self, m: f32) -> Rect {
        Rect::new(self.x - m, self.y - m, self.w + 2.0 * m, self.h + 2.0 * m)
    }
    pub fn scale(&self, sx: f32, sy: f32) -> Rect {
        Rect::new(self.x * sx, self.y * sy, self.w * sx, self.h * sy)
    }
}

/// RGBA colour, 0..=255 per channel.
pub type Color = [u8; 4];

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Scenario {
    pub id: String,
    pub seed: u64,
    /// Genre template names this scenario was built from (informational).
    #[serde(default)]
    pub genres: Vec<String>,
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    pub duration_ms: f64,
    pub background: Background,
    pub layout: Layout,
    pub items: Vec<TextItem>,
}

impl Scenario {
    pub fn frame_count(&self) -> usize {
        ((self.duration_ms / 1000.0) * self.fps).floor().max(1.0) as usize
    }
    pub fn frame_time_ms(&self, i: usize) -> f64 {
        i as f64 * 1000.0 / self.fps
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProceduralKind {
    Gradient,
    Noise,
    MovingShapes,
    GrassLike,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Background {
    Procedural {
        kind: ProceduralKind,
        seed: u64,
    },
    /// Image sequence, looped and scaled to the scenario size. Files are the sorted
    /// `.jpg`/`.png` files of `dir`; `first_frame` and `count` pick a slice of them.
    Images {
        dir: String,
        fps: f64,
        #[serde(default)]
        first_frame: usize,
        #[serde(default)]
        count: Option<usize>,
        /// Start offset into the slice, so scenarios do not all start on the same frame.
        #[serde(default)]
        offset_ms: f64,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct Layout {
    pub zones: Vec<Zone>,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ZoneKind {
    Dialogue,
    Speaker,
    Notification,
    Prompt,
    Hud,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Zone {
    pub name: String,
    pub kind: ZoneKind,
    pub rect: Rect,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ItemKind {
    Dialogue,
    Speaker,
    PickupToast,
    Objective,
    Subtitle,
    Tooltip,
    Prompt,
    HudStatic,
    DamageNumber,
    Menu,
}

impl ItemKind {
    /// Default `should_read` per the spec. `Prompt` is false here and scored in its own bucket.
    pub fn default_should_read(self) -> bool {
        matches!(
            self,
            ItemKind::Dialogue
                | ItemKind::Speaker
                | ItemKind::PickupToast
                | ItemKind::Objective
                | ItemKind::Subtitle
                | ItemKind::Tooltip
        )
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct TextItem {
    pub id: String,
    /// Text as drawn. `\n` separates lines.
    pub text: String,
    pub kind: ItemKind,
    pub should_read: bool,
    /// Final on-screen box (panel included, shadow excluded), after any slide ends.
    pub rect: Rect,
    pub start_ms: f64,
    pub end_ms: f64,
    pub style: Style,
    #[serde(default)]
    pub anim: Anim,
}

impl TextItem {
    pub fn duration_ms(&self) -> f64 {
        self.end_ms - self.start_ms
    }
    pub fn visible_at(&self, t: f64) -> bool {
        t >= self.start_ms && t < self.end_ms
    }
    /// Time typing ends: the first time `render::visible_chars` shows every character.
    pub fn typed_at_ms(&self) -> f64 {
        match self.anim.typewriter_cps {
            Some(cps) if cps > 0.0 => {
                let rest = self.text.chars().count().saturating_sub(1) as f64;
                self.start_ms + rest / cps as f64 * 1000.0
            }
            _ => self.start_ms,
        }
    }
    /// First time the whole text is shown at full opacity: the later of fade-in end and typing end.
    pub fn readable_from_ms(&self) -> f64 {
        (self.start_ms + self.anim.fade_in_ms).max(self.typed_at_ms())
    }
    /// Time the fade-out starts.
    pub fn readable_until_ms(&self) -> f64 {
        self.end_ms - self.anim.fade_out_ms
    }
    /// How long the whole text is shown at full opacity (0 when never).
    pub fn readable_ms(&self) -> f64 {
        (self.readable_until_ms() - self.readable_from_ms()).max(0.0)
    }
}

/// Fingerprint of an answer key: FNV-1a 64 of its JSON, as 16 hex digits. Runs and reports store
/// it, so events are never scored against a different truth than the one they were made for.
pub fn truth_fingerprint(items: &[TextItem]) -> String {
    let json = serde_json::to_string(items).unwrap_or_default();
    fnv_hex(json.as_bytes())
}

/// FNV-1a 64 of `bytes`, as 16 hex digits.
pub fn fnv_hex(bytes: &[u8]) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h ^= *b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{h:016x}")
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum Align {
    #[default]
    Left,
    Center,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Style {
    /// Family name, e.g. "Segoe UI". Resolved to a file in C:\Windows\Fonts or arena-data/fonts.
    pub font: String,
    pub size_px: f32,
    pub color: Color,
    #[serde(default)]
    pub outline: Option<Outline>,
    #[serde(default)]
    pub shadow: Option<Shadow>,
    #[serde(default)]
    pub panel: Option<Panel>,
    #[serde(default)]
    pub bold: bool,
    #[serde(default)]
    pub italic: bool,
    /// Horizontal text alignment inside `rect`.
    #[serde(default)]
    pub align: Align,
    /// Line height as a multiple of `size_px`.
    #[serde(default = "default_line_spacing")]
    pub line_spacing: f32,
}

fn default_line_spacing() -> f32 {
    1.25
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Outline {
    pub color: Color,
    pub px: f32,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Shadow {
    pub color: Color,
    pub dx: f32,
    pub dy: f32,
    pub blur: f32,
}

/// Box drawn behind the text. It fills `rect`; text starts `padding` px inside it.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Panel {
    pub color: Color,
    pub alpha: f32,
    pub padding: f32,
    pub radius: f32,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct Anim {
    #[serde(default)]
    pub fade_in_ms: f64,
    #[serde(default)]
    pub fade_out_ms: f64,
    /// Offset (dx, dy) the item starts at; it slides to `rect` over `slide_ms`.
    #[serde(default)]
    pub slide_in: [f32; 2],
    /// Offset (dx, dy) the item slides to over the last `slide_ms` before `end_ms`.
    #[serde(default)]
    pub slide_out: [f32; 2],
    #[serde(default)]
    pub slide_ms: f64,
    /// Characters per second; `None` shows all text at once.
    #[serde(default)]
    pub typewriter_cps: Option<f32>,
    /// Id of the item this one replaces in place (a toast whose counter changed).
    #[serde(default)]
    pub replace_of: Option<String>,
}

/// A suite on disk: `suite.json` next to one JSON file per scenario.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Suite {
    pub name: String,
    pub seed: u64,
    pub scenarios: Vec<String>,
}

/// Real recorded frames plus a hand-made answer key (`*.recorded.json`).
///
/// Frames are the `.png`/`.jpg` files of `frames_dir` (relative to the JSON file), named like the
/// recorder's mark frames: `f{index:03}_{offset_ms:+06}ms.png`. The signed offset is the frame
/// time. Truth item times use the same time base. The recorder writes offsets relative to the
/// mark key press (frames before it are negative); `arena realworld-import` shifts them so t = 0
/// is the first frame of the mark.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct RecordedScenario {
    pub id: String,
    pub frames_dir: String,
    pub truth: Vec<TextItem>,
    #[serde(default)]
    pub layout: Layout,
    /// Truth ids whose start time is not exact: the item was already on screen at the first
    /// frame, or appeared after a frame gap. Left out of latency and duration buckets.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub start_uncertain: Vec<String>,
    /// Truth ids whose end time is not exact: still on screen at the last frame, or gone after a
    /// frame gap. Left out of duration buckets.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub end_uncertain: Vec<String>,
}
