//! Minimal WAV writing (mono s16 PCM) for the `aufile` playback path — the
//! counterpart to ringo-core's WAV parser, small enough not to warrant a
//! dependency.

use anyhow::{Context, Result};
use std::io::Write;
use std::path::Path;

/// Write `samples` (mono s16) as a PCM WAV file at `sample_rate`.
pub fn write_mono_wav(path: &Path, samples: &[i16], sample_rate: u32) -> Result<()> {
    let data_len = samples.len() * 2;
    let mut out = std::io::BufWriter::new(
        std::fs::File::create(path).with_context(|| format!("create WAV `{}`", path.display()))?,
    );
    let byte_rate = sample_rate * 2;
    out.write_all(b"RIFF")?;
    out.write_all(&(36 + data_len as u32).to_le_bytes())?;
    out.write_all(b"WAVE")?;
    out.write_all(b"fmt ")?;
    out.write_all(&16u32.to_le_bytes())?;
    out.write_all(&1u16.to_le_bytes())?; // PCM
    out.write_all(&1u16.to_le_bytes())?; // mono
    out.write_all(&sample_rate.to_le_bytes())?;
    out.write_all(&byte_rate.to_le_bytes())?;
    out.write_all(&2u16.to_le_bytes())?; // block align
    out.write_all(&16u16.to_le_bytes())?; // bits
    out.write_all(b"data")?;
    out.write_all(&(data_len as u32).to_le_bytes())?;
    for &s in samples {
        out.write_all(&s.to_le_bytes())?;
    }
    out.flush().context("flush WAV")?;
    Ok(())
}

/// A PCM WAV's playback duration (parses only the header; 0 on a bad file).
pub fn wav_duration(path: &Path) -> std::time::Duration {
    let Ok(data) = std::fs::read(path) else {
        return std::time::Duration::ZERO;
    };
    if data.len() < 44 || &data[0..4] != b"RIFF" || &data[8..12] != b"WAVE" {
        return std::time::Duration::ZERO;
    }
    let byte_rate = u32::from_le_bytes([data[28], data[29], data[30], data[31]]);
    // Walk to the data chunk (fmt may be followed by extra chunks).
    let mut pos = 12;
    while pos + 8 <= data.len() {
        let id = &data[pos..pos + 4];
        let size = u32::from_le_bytes([data[pos + 4], data[pos + 5], data[pos + 6], data[pos + 7]])
            as usize;
        if id == b"data" {
            if byte_rate == 0 {
                return std::time::Duration::ZERO;
            }
            return std::time::Duration::from_secs_f64(size as f64 / byte_rate as f64);
        }
        pos += 8 + size + (size & 1);
    }
    std::time::Duration::ZERO
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wav_roundtrip_duration() {
        // 1 s of 8 kHz samples (non-zero, so energy checks in callers work).
        let samples: Vec<i16> = (0..8000).map(|i| (i % 100) as i16).collect();
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("t.wav");
        write_mono_wav(&p, &samples, 8000).unwrap();
        let d = wav_duration(&p);
        assert!((d.as_millis() as i64 - 1000).abs() < 5, "{d:?}");
    }

    #[test]
    fn empty_wav_is_zero_duration() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("e.wav");
        write_mono_wav(&p, &[], 16000).unwrap();
        assert_eq!(wav_duration(&p), std::time::Duration::ZERO);
    }
}
