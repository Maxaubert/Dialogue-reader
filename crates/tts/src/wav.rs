//! WAV (RIFF) to [`Pcm`]. Windows voices give 16-bit PCM WAV; other formats are refused with a plain error.

use crate::Pcm;

fn u16_at(b: &[u8], i: usize) -> Option<u16> {
    Some(u16::from_le_bytes(b.get(i..i + 2)?.try_into().ok()?))
}

fn u32_at(b: &[u8], i: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(i..i + 4)?.try_into().ok()?))
}

/// Parses a 16-bit PCM WAV file. Chunks other than `fmt ` and `data` are skipped.
pub fn parse(bytes: &[u8]) -> Result<Pcm, String> {
    if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err("not a WAV file".into());
    }
    let mut at = 12;
    let mut fmt: Option<(u16, u16, u32, u16)> = None;
    while at + 8 <= bytes.len() {
        let id = &bytes[at..at + 4];
        let size = u32_at(bytes, at + 4).ok_or("WAV chunk cut off")? as usize;
        let body = at + 8;
        // a data chunk may claim more than the stream holds (streamed WAV): take what is there
        let end = body.saturating_add(size).min(bytes.len());
        match id {
            b"fmt " => {
                let b = &bytes[body..end];
                fmt = Some((
                    u16_at(b, 0).ok_or("WAV format chunk too short")?,
                    u16_at(b, 2).ok_or("WAV format chunk too short")?,
                    u32_at(b, 4).ok_or("WAV format chunk too short")?,
                    u16_at(b, 14).ok_or("WAV format chunk too short")?,
                ));
            }
            b"data" => {
                let (format, channels, rate, bits) = fmt.ok_or("WAV data before its format")?;
                // 1 = PCM; 0xFFFE = extensible (Windows uses it for PCM too)
                if !(format == 1 || format == 0xFFFE) || bits != 16 {
                    return Err(format!(
                        "WAV format {format} with {bits} bits is not supported (16-bit PCM only)"
                    ));
                }
                if channels == 0 || rate == 0 {
                    return Err("WAV with no channels or no sample rate".into());
                }
                let samples = bytes[body..end]
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|&c| i16::from_le_bytes(c))
                    .collect();
                return Ok(Pcm {
                    rate,
                    channels,
                    samples,
                });
            }
            _ => {}
        }
        // chunks are padded to an even size
        at = body.saturating_add(size).saturating_add(size & 1);
    }
    Err("WAV file has no data".into())
}

/// A 16-bit PCM WAV file of `pcm` (for tests and for saving a sample).
pub fn write(pcm: &Pcm) -> Vec<u8> {
    let data_len = (pcm.samples.len() * 2) as u32;
    let mut b = Vec::with_capacity(44 + data_len as usize);
    b.extend_from_slice(b"RIFF");
    b.extend_from_slice(&(36 + data_len).to_le_bytes());
    b.extend_from_slice(b"WAVE");
    b.extend_from_slice(b"fmt ");
    b.extend_from_slice(&16u32.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&pcm.channels.to_le_bytes());
    b.extend_from_slice(&pcm.rate.to_le_bytes());
    let block = pcm.channels as u32 * 2;
    b.extend_from_slice(&(pcm.rate * block).to_le_bytes());
    b.extend_from_slice(&(block as u16).to_le_bytes());
    b.extend_from_slice(&16u16.to_le_bytes());
    b.extend_from_slice(b"data");
    b.extend_from_slice(&data_len.to_le_bytes());
    for s in &pcm.samples {
        b.extend_from_slice(&s.to_le_bytes());
    }
    b
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let pcm = Pcm {
            rate: 22050,
            channels: 1,
            samples: vec![0, 1, -1, i16::MAX, i16::MIN],
        };
        assert_eq!(parse(&write(&pcm)).unwrap(), pcm);
    }

    #[test]
    fn skips_other_chunks_and_odd_padding() {
        let pcm = Pcm {
            rate: 16000,
            channels: 2,
            samples: vec![5, -5, 7, -7],
        };
        let w = write(&pcm);
        // insert a 3-byte LIST chunk (padded to 4) between fmt and data
        let mut b = w[..36].to_vec();
        b.extend_from_slice(b"LIST");
        b.extend_from_slice(&3u32.to_le_bytes());
        b.extend_from_slice(&[1, 2, 3, 0]);
        b.extend_from_slice(&w[36..]);
        assert_eq!(parse(&b).unwrap(), pcm);
        assert_eq!(pcm.duration_ms(), 2.0 / 16000.0 * 1000.0);
    }

    #[test]
    fn refuses_what_it_cannot_play() {
        assert!(parse(b"hello").is_err());
        let mut w = write(&Pcm {
            rate: 8000,
            channels: 1,
            samples: vec![1],
        });
        w[34] = 8; // 8 bits per sample
        assert!(parse(&w).unwrap_err().contains("16-bit"));
        let w = write(&Pcm {
            rate: 8000,
            channels: 1,
            samples: vec![],
        });
        assert!(parse(&w[..36]).unwrap_err().contains("no data"));
    }

    #[test]
    fn a_cut_data_chunk_keeps_what_is_there() {
        let mut w = write(&Pcm {
            rate: 8000,
            channels: 1,
            samples: vec![1, 2, 3],
        });
        w.truncate(w.len() - 2);
        assert_eq!(parse(&w).unwrap().samples, vec![1, 2]);
    }
}
