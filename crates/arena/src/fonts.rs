//! Font lookup (by file name in C:\Windows\Fonts and arena-data/fonts) and text layout.

use ab_glyph::{Font, FontArc, GlyphId, PxScale, ScaleFont};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

/// Game-like families the generator draws from, when their files exist.
pub const GAME_FAMILIES: &[&str] = &[
    "Bahnschrift",
    "Segoe UI",
    "Impact",
    "Trebuchet MS",
    "Georgia",
    "Consolas",
    "Arial",
    "Verdana",
    "Tahoma",
    "Calibri",
];

/// Files tried when a family is missing, in order.
const FALLBACK_FILES: &[&str] = &["segoeui.ttf", "arial.ttf", "tahoma.ttf", "verdana.ttf"];

/// File names for (regular, bold, italic, bold italic) of the known families.
fn family_files(family: &str) -> Option<[&'static str; 4]> {
    let f = match family.to_ascii_lowercase().as_str() {
        "segoe ui" => ["segoeui.ttf", "segoeuib.ttf", "segoeuii.ttf", "segoeuiz.ttf"],
        "bahnschrift" => ["bahnschrift.ttf"; 4],
        "impact" => ["impact.ttf"; 4],
        "trebuchet ms" | "trebuchet" => ["trebuc.ttf", "trebucbd.ttf", "trebucit.ttf", "trebucbi.ttf"],
        "georgia" => ["georgia.ttf", "georgiab.ttf", "georgiai.ttf", "georgiaz.ttf"],
        "consolas" => ["consola.ttf", "consolab.ttf", "consolai.ttf", "consolaz.ttf"],
        "arial" => ["arial.ttf", "arialbd.ttf", "ariali.ttf", "arialbi.ttf"],
        "verdana" => ["verdana.ttf", "verdanab.ttf", "verdanai.ttf", "verdanaz.ttf"],
        "tahoma" => ["tahoma.ttf", "tahomabd.ttf", "tahoma.ttf", "tahomabd.ttf"],
        "calibri" => ["calibri.ttf", "calibrib.ttf", "calibrii.ttf", "calibriz.ttf"],
        _ => return None,
    };
    Some(f)
}

fn font_dirs() -> Vec<PathBuf> {
    let mut dirs = vec![crate::data_dir().join("fonts")];
    let windir = std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".to_string());
    dirs.push(Path::new(&windir).join("Fonts"));
    dirs
}

fn find_file(name: &str) -> Option<PathBuf> {
    font_dirs().into_iter().map(|d| d.join(name)).find(|p| p.is_file())
}

/// Candidate file names for a family and variant, best first.
fn candidates(family: &str, bold: bool, italic: bool) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(files) = family_files(family) {
        let idx = match (bold, italic) {
            (false, false) => 0,
            (true, false) => 1,
            (false, true) => 2,
            (true, true) => 3,
        };
        out.push(files[idx].to_string());
        out.push(files[0].to_string());
    } else {
        // Unknown family: a file named after it (arena-data/fonts/<family>.ttf/.otf).
        let stem = family.to_ascii_lowercase().replace(' ', "");
        out.push(format!("{family}.ttf"));
        out.push(format!("{family}.otf"));
        out.push(format!("{stem}.ttf"));
        out.push(format!("{stem}.otf"));
    }
    out.extend(FALLBACK_FILES.iter().map(|s| s.to_string()));
    out
}

type Cache = Mutex<HashMap<String, Option<FontArc>>>;

fn cache() -> &'static Cache {
    static C: OnceLock<Cache> = OnceLock::new();
    C.get_or_init(|| Mutex::new(HashMap::new()))
}

fn load_file(name: &str) -> Option<FontArc> {
    let mut c = cache().lock().unwrap();
    if let Some(f) = c.get(name) {
        return f.clone();
    }
    let font = find_file(name)
        .and_then(|p| std::fs::read(p).ok())
        .and_then(|bytes| FontArc::try_from_vec(bytes).ok());
    c.insert(name.to_string(), font.clone());
    font
}

/// Loads the font for a family and variant, falling back through common Windows fonts.
pub fn font(family: &str, bold: bool, italic: bool) -> Option<FontArc> {
    candidates(family, bold, italic).iter().find_map(|f| load_file(f))
}

/// True when the family's own regular file exists (no fallback).
pub fn family_available(family: &str) -> bool {
    match family_files(family) {
        Some(files) => find_file(files[0]).is_some(),
        None => false,
    }
}

/// Families usable on this PC: the game-like list plus every font file in arena-data/fonts.
/// Sorted, so generation does not depend on directory order.
pub fn available_families() -> Vec<String> {
    let mut out: Vec<String> =
        GAME_FAMILIES.iter().filter(|f| family_available(f)).map(|s| s.to_string()).collect();
    if let Ok(rd) = std::fs::read_dir(crate::data_dir().join("fonts")) {
        let mut extra: Vec<String> = rd
            .flatten()
            .filter_map(|e| {
                let p = e.path();
                let ext = p.extension()?.to_str()?.to_ascii_lowercase();
                if ext == "ttf" || ext == "otf" { Some(p.file_stem()?.to_str()?.to_string()) } else { None }
            })
            .collect();
        extra.sort();
        out.extend(extra);
    }
    out
}

/// One glyph placed in block coordinates (top-left of the text block is 0,0).
#[derive(Clone, Debug)]
pub struct PlacedGlyph {
    pub id: GlyphId,
    pub x: f32,
    pub baseline: f32,
    /// Index of the source char in the whole text (newlines count), for typewriter clipping.
    pub char_index: usize,
}

#[derive(Clone, Debug)]
pub struct TextLayout {
    pub glyphs: Vec<PlacedGlyph>,
    pub line_widths: Vec<f32>,
    pub line_of_glyph: Vec<usize>,
    pub width: f32,
    pub height: f32,
    pub line_height: f32,
    pub char_count: usize,
}

/// Lays out `text` (lines split on `\n`) at `size_px`.
pub fn layout(font: &FontArc, size_px: f32, line_spacing: f32, text: &str) -> TextLayout {
    let sf = font.as_scaled(PxScale::from(size_px));
    let ascent = sf.ascent();
    let descent = sf.descent();
    let line_height = size_px * line_spacing;
    let mut glyphs = Vec::new();
    let mut line_of_glyph = Vec::new();
    let mut line_widths = Vec::new();
    let mut char_index = 0usize;
    for (li, line) in text.split('\n').enumerate() {
        let baseline = ascent + li as f32 * line_height;
        let mut x = 0.0f32;
        let mut prev: Option<GlyphId> = None;
        for ch in line.chars() {
            let id = sf.glyph_id(ch);
            if let Some(p) = prev {
                x += sf.kern(p, id);
            }
            glyphs.push(PlacedGlyph { id, x, baseline, char_index });
            line_of_glyph.push(li);
            x += sf.h_advance(id);
            prev = Some(id);
            char_index += 1;
        }
        line_widths.push(x);
        char_index += 1; // the newline
    }
    let lines = line_widths.len().max(1);
    let width = line_widths.iter().cloned().fold(0.0, f32::max);
    let height = (lines - 1) as f32 * line_height + (ascent - descent);
    TextLayout {
        glyphs,
        line_widths,
        line_of_glyph,
        width,
        height,
        line_height,
        char_count: text.chars().count(),
    }
}

/// Size (w, h) of a text block. Uses an estimate when no font file can be found.
pub fn measure(family: &str, bold: bool, italic: bool, size_px: f32, line_spacing: f32, text: &str) -> (f32, f32) {
    match font(family, bold, italic) {
        Some(f) => {
            let l = layout(&f, size_px, line_spacing, text);
            (l.width, l.height)
        }
        None => {
            let lines: Vec<&str> = text.split('\n').collect();
            let longest = lines.iter().map(|l| l.chars().count()).max().unwrap_or(0) as f32;
            let w = longest * size_px * 0.55;
            let h = (lines.len().max(1) - 1) as f32 * size_px * line_spacing + size_px * 1.2;
            (w, h)
        }
    }
}

/// Greedy word wrap so no line is wider than `max_w`.
pub fn wrap(family: &str, bold: bool, italic: bool, size_px: f32, text: &str, max_w: f32) -> String {
    let mut lines: Vec<String> = Vec::new();
    for para in text.split('\n') {
        let mut cur = String::new();
        for word in para.split_whitespace() {
            let trial = if cur.is_empty() { word.to_string() } else { format!("{cur} {word}") };
            let (w, _) = measure(family, bold, italic, size_px, 1.0, &trial);
            if w > max_w && !cur.is_empty() {
                lines.push(std::mem::take(&mut cur));
                cur = word.to_string();
            } else {
                cur = trial;
            }
        }
        lines.push(cur);
    }
    lines.join("\n")
}
