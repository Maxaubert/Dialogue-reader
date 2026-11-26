//! Kokoro's voices: the English voices Sonara offers and their style
//! vectors, read from `voices-v1.0.bin` (a NumPy `.npz`: an uncompressed
//! zip of one `.npy` per voice, each float32 of shape (510, 1, 256)).
//!
//! From Sonara (MIT, `crates/sonara-engine/src/kokoro/voices.rs` at commit 8fd2af7).
use super::{Error, Result};
use std::collections::HashMap;

/// Style vector width.
pub const STYLE_DIM: usize = 256;
/// Rows per voice: one style per token count, 0..=509.
pub const STYLE_ROWS: usize = 510;

/// The default voice (Kokoro's top-rated, as the Python reader).
pub const DEFAULT_VOICE: &str = "af_heart";

/// The English voices of Kokoro v1.0, in the upstream catalogue order
/// (`af_`/`am_` US female/male, `bf_`/`bm_` British female/male). The G2P is
/// US English for all of them.
pub const ENGLISH_VOICES: &[&str] = &[
    "af_heart",
    "af_bella",
    "bf_emma",
    "af_nicole",
    "af_aoede",
    "af_kore",
    "af_sarah",
    "am_fenrir",
    "am_michael",
    "am_puck",
    "af_alloy",
    "af_nova",
    "bf_isabella",
    "bm_fable",
    "bm_george",
    "af_sky",
    "bm_lewis",
    "af_jessica",
    "af_river",
    "am_echo",
    "am_eric",
    "am_liam",
    "am_onyx",
    "bf_alice",
    "bf_lily",
    "bm_daniel",
    "am_santa",
    "am_adam",
];

/// BCP 47 tag of a voice id.
pub fn language(id: &str) -> &'static str {
    if id.starts_with('b') {
        "en-GB"
    } else {
        "en-US"
    }
}

/// Display name: "Heart (Kokoro)".
pub fn display_name(id: &str) -> String {
    let name = id.split_once('_').map_or(id, |(_, n)| n);
    let mut c = name.chars();
    let name = match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    };
    format!("{name} (Kokoro)")
}

/// Style vectors by voice id.
pub struct Styles {
    voices: HashMap<String, Vec<f32>>,
}

impl Styles {
    /// Read the `voices` npz, keeping the voices in `wanted`.
    pub fn parse(bytes: &[u8], wanted: &[&str]) -> Result<Styles> {
        let mut voices = HashMap::new();
        for (name, data) in stored_zip_entries(bytes)? {
            let Some(id) = name.strip_suffix(".npy") else {
                continue;
            };
            if !wanted.contains(&id) {
                continue;
            }
            voices.insert(id.to_string(), npy_f32(data, STYLE_ROWS * STYLE_DIM)?);
        }
        if let Some(missing) = wanted.iter().find(|w| !voices.contains_key(**w)) {
            return Err(bad(format!("voice '{missing}' is missing")));
        }
        Ok(Styles { voices })
    }

    pub fn contains(&self, id: &str) -> bool {
        self.voices.contains_key(id)
    }

    /// The style for a batch of `tokens` tokens (Kokoro picks the row by
    /// the token count).
    pub fn style(&self, id: &str, tokens: usize) -> Option<&[f32]> {
        let v = self.voices.get(id)?;
        let row = tokens.min(STYLE_ROWS - 1);
        Some(&v[row * STYLE_DIM..(row + 1) * STYLE_DIM])
    }
}

fn bad(why: String) -> Error {
    Error::Engine(format!("invalid Kokoro voices file: {why}"))
}

fn u16_at(b: &[u8], at: usize) -> Result<u16> {
    b.get(at..at + 2)
        .map(|s| u16::from_le_bytes([s[0], s[1]]))
        .ok_or_else(|| bad("truncated".into()))
}

fn u32_at(b: &[u8], at: usize) -> Result<u32> {
    b.get(at..at + 4)
        .map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
        .ok_or_else(|| bad("truncated".into()))
}

fn u64_at(b: &[u8], at: usize) -> Result<u64> {
    b.get(at..at + 8)
        .map(|s| u64::from_le_bytes(s.try_into().expect("8 bytes")))
        .ok_or_else(|| bad("truncated".into()))
}

/// The entries of a zip whose members are stored (not compressed), as
/// NumPy writes an `.npz` with `savez`. Sizes come from the central
/// directory (with its ZIP64 extra field when present).
pub fn stored_zip_entries(b: &[u8]) -> Result<Vec<(String, &[u8])>> {
    const EOCD: u32 = 0x0605_4b50;
    const CENTRAL: u32 = 0x0201_4b50;
    const LOCAL: u32 = 0x0403_4b50;
    // The end-of-central-directory record is in the last 64 KiB + 22.
    let floor = b.len().saturating_sub(22 + 65_535);
    let eocd = (floor..=b.len().saturating_sub(22))
        .rev()
        .find(|&i| u32_at(b, i).ok() == Some(EOCD))
        .ok_or_else(|| bad("not a zip file".into()))?;
    let count = u16_at(b, eocd + 10)? as usize;
    let mut at = u32_at(b, eocd + 16)? as usize;
    let mut out = Vec::with_capacity(count);
    for _ in 0..count {
        if u32_at(b, at)? != CENTRAL {
            return Err(bad("bad central directory".into()));
        }
        let method = u16_at(b, at + 10)?;
        let mut size = u32_at(b, at + 24)? as u64;
        let name_len = u16_at(b, at + 28)? as usize;
        let extra_len = u16_at(b, at + 30)? as usize;
        let comment_len = u16_at(b, at + 32)? as usize;
        let mut offset = u32_at(b, at + 42)? as u64;
        let name = b
            .get(at + 46..at + 46 + name_len)
            .ok_or_else(|| bad("truncated".into()))?;
        let name = String::from_utf8_lossy(name).into_owned();
        // ZIP64: the 0xFFFFFFFF fields are in extra field 0x0001, in the
        // order uncompressed size, compressed size, offset.
        let mut e = at + 46 + name_len;
        let end = e + extra_len;
        while e + 4 <= end {
            let (id, len) = (u16_at(b, e)?, u16_at(b, e + 2)? as usize);
            if id == 1 {
                let mut f = e + 4;
                if size == 0xFFFF_FFFF {
                    size = u64_at(b, f)?;
                    f += 8;
                }
                if u32_at(b, at + 20)? == 0xFFFF_FFFF {
                    f += 8;
                }
                if offset == 0xFFFF_FFFF {
                    offset = u64_at(b, f)?;
                }
            }
            e += 4 + len;
        }
        if method != 0 {
            return Err(bad(format!("{name} is compressed (method {method})")));
        }
        let local = offset as usize;
        if u32_at(b, local)? != LOCAL {
            return Err(bad(format!("{name}: bad local header")));
        }
        let start = local + 30 + u16_at(b, local + 26)? as usize + u16_at(b, local + 28)? as usize;
        let data = b
            .get(start..start + size as usize)
            .ok_or_else(|| bad(format!("{name} is truncated")))?;
        out.push((name, data));
        at += 46 + name_len + extra_len + comment_len;
    }
    Ok(out)
}

/// A little-endian float32 `.npy` array of exactly `len` values.
pub fn npy_f32(b: &[u8], len: usize) -> Result<Vec<f32>> {
    if b.get(..6) != Some(b"\x93NUMPY") {
        return Err(bad("not an .npy array".into()));
    }
    let (header_len, header_at) = match b.get(6) {
        Some(1) => (u16_at(b, 8)? as usize, 10),
        Some(2) | Some(3) => (u32_at(b, 8)? as usize, 12),
        _ => return Err(bad("unknown .npy version".into())),
    };
    let header = b
        .get(header_at..header_at + header_len)
        .ok_or_else(|| bad("truncated .npy header".into()))?;
    let header = String::from_utf8_lossy(header);
    if !header.contains("'<f4'") || header.contains("'fortran_order': True") {
        return Err(bad(format!("unexpected .npy layout: {}", header.trim())));
    }
    let data = &b[header_at + header_len..];
    if data.len() != len * 4 {
        return Err(bad(format!(
            "expected {len} floats, found {} bytes",
            data.len()
        )));
    }
    let (words, _) = data.as_chunks::<4>();
    Ok(words.iter().map(|c| f32::from_le_bytes(*c)).collect())
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// An `.npy` of `values` (float32, shape (n,)).
    pub fn npy(values: &[f32]) -> Vec<u8> {
        let mut header = format!(
            "{{'descr': '<f4', 'fortran_order': False, 'shape': ({},), }}",
            values.len()
        );
        while (10 + header.len() + 1) % 64 != 0 {
            header.push(' ');
        }
        header.push('\n');
        let mut b = b"\x93NUMPY\x01\x00".to_vec();
        b.extend((header.len() as u16).to_le_bytes());
        b.extend(header.as_bytes());
        for v in values {
            b.extend(v.to_le_bytes());
        }
        b
    }

    /// A stored zip of `entries`, with ZIP64 local headers like NumPy's.
    pub fn zip(entries: &[(&str, Vec<u8>)]) -> Vec<u8> {
        let mut out = Vec::new();
        let mut central = Vec::new();
        for (name, data) in entries {
            let offset = out.len() as u32;
            out.extend(0x0403_4b50u32.to_le_bytes());
            out.extend([45, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
            out.extend(0u32.to_le_bytes()); // crc (unchecked)
            out.extend(0xFFFF_FFFFu32.to_le_bytes());
            out.extend(0xFFFF_FFFFu32.to_le_bytes());
            out.extend((name.len() as u16).to_le_bytes());
            out.extend(20u16.to_le_bytes());
            out.extend(name.as_bytes());
            out.extend(1u16.to_le_bytes());
            out.extend(16u16.to_le_bytes());
            out.extend((data.len() as u64).to_le_bytes());
            out.extend((data.len() as u64).to_le_bytes());
            out.extend(data);
            central.extend(0x0201_4b50u32.to_le_bytes());
            central.extend([45, 0, 45, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
            central.extend(0u32.to_le_bytes());
            central.extend((data.len() as u32).to_le_bytes());
            central.extend((data.len() as u32).to_le_bytes());
            central.extend((name.len() as u16).to_le_bytes());
            central.extend([0u8; 12]);
            central.extend(offset.to_le_bytes());
            central.extend(name.as_bytes());
        }
        let cd_at = out.len() as u32;
        let cd_len = central.len() as u32;
        out.extend(central);
        out.extend(0x0605_4b50u32.to_le_bytes());
        out.extend([0u8; 4]);
        out.extend((entries.len() as u16).to_le_bytes());
        out.extend((entries.len() as u16).to_le_bytes());
        out.extend(cd_len.to_le_bytes());
        out.extend(cd_at.to_le_bytes());
        out.extend([0u8; 2]);
        out
    }

    /// A voices file where voice `id` has style rows filled with `row`.
    pub fn voices_file(ids: &[&str]) -> Vec<u8> {
        let entries: Vec<(String, Vec<u8>)> = ids
            .iter()
            .map(|id| {
                let values: Vec<f32> = (0..STYLE_ROWS * STYLE_DIM)
                    .map(|i| (i / STYLE_DIM) as f32)
                    .collect();
                (format!("{id}.npy"), npy(&values))
            })
            .collect();
        let refs: Vec<(&str, Vec<u8>)> = entries
            .iter()
            .map(|(n, d)| (n.as_str(), d.clone()))
            .collect();
        zip(&refs)
    }

    #[test]
    fn styles_are_read_from_a_numpy_zip() {
        let file = voices_file(&["af_heart", "am_adam", "zf_xiaobei"]);
        let s = Styles::parse(&file, &["af_heart", "am_adam"]).unwrap();
        assert!(s.contains("af_heart") && !s.contains("zf_xiaobei"));
        assert_eq!(s.style("af_heart", 7).unwrap(), &[7.0; STYLE_DIM][..]);
        assert_eq!(s.style("am_adam", 9_999).unwrap()[0], 509.0);
        assert!(s.style("bf_emma", 1).is_none());
        let e = Styles::parse(&file, &["bf_emma"]).err().unwrap();
        assert!(e.to_string().contains("'bf_emma' is missing"), "{e}");
    }

    #[test]
    fn broken_files_are_errors_not_panics() {
        assert!(Styles::parse(b"", &[]).is_err());
        assert!(Styles::parse(b"PK\x05\x06", &[]).is_err());
        let mut file = voices_file(&["af_heart"]);
        file.truncate(file.len() / 2);
        assert!(Styles::parse(&file, &["af_heart"]).is_err());
        assert!(npy_f32(&npy(&[1.0, 2.0]), 3).is_err());
        assert_eq!(npy_f32(&npy(&[1.0, 2.0]), 2).unwrap(), vec![1.0, 2.0]);
    }

    #[test]
    fn names_and_languages() {
        assert_eq!(display_name("af_heart"), "Heart (Kokoro)");
        assert_eq!(language("bm_george"), "en-GB");
        assert_eq!(language("am_adam"), "en-US");
        assert_eq!(ENGLISH_VOICES.len(), 28);
        assert!(ENGLISH_VOICES.contains(&DEFAULT_VOICE));
    }
}
