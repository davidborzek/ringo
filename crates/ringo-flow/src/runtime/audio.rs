//! Tone detection on recorded call audio (for `verify-audio`).
//!
//! baresip's `sndfile` module records each call's decoded (received) audio to a
//! `dump-…-dec.wav` in the agent's temp dir. We read the recent window of that
//! WAV and run a [Goertzel](https://en.wikipedia.org/wiki/Goertzel_algorithm)
//! filter at the expected frequency: the score is ~1.0 for a clean tone at that
//! frequency and ~0 for silence/noise/other tones, independent of sample count.
//!
//! The WAV is read with a tolerant parser, not a strict one: `verify-audio` runs
//! while the call is still active, and libsndfile only patches the RIFF/`data`
//! size fields when it closes the file. The PCM bytes are on disk regardless, so
//! we read the `data` chunk to EOF rather than trusting its (stale, often 0)
//! size field.

use anyhow::{Context, Result, bail};
use std::path::{Path, PathBuf};

/// A received audio tone counts as present at/above this score.
pub const TONE_THRESHOLD: f64 = 0.2;

/// Normalized Goertzel score for `freq` over `samples` at `sample_rate`.
/// `~1.0` for a pure tone at `freq`, `~0` otherwise; `0` when silent/empty.
pub fn tone_score(samples: &[i16], sample_rate: u32, freq: f64) -> f64 {
    let n = samples.len();
    if n == 0 || sample_rate == 0 {
        return 0.0;
    }
    let omega = 2.0 * std::f64::consts::PI * freq / f64::from(sample_rate);
    let coeff = 2.0 * omega.cos();
    let (mut s1, mut s2) = (0.0f64, 0.0f64);
    let mut energy = 0.0f64;
    for &x in samples {
        let xf = f64::from(x);
        energy += xf * xf;
        let s = xf + coeff * s1 - s2;
        s2 = s1;
        s1 = s;
    }
    if energy == 0.0 {
        return 0.0;
    }
    let power = s2 * s2 + s1 * s1 - coeff * s1 * s2;
    // Normalize so a pure sine at `freq` scores ~1 regardless of N (see derivation
    // in the module docs): |X(f)|² ≈ N²A²/4 and Σx² ≈ NA²/2 → 2P/(N·E) ≈ 1.
    (2.0 * power) / (n as f64 * energy)
}

/// The most recently written received-audio recording in `dir`, if any.
pub fn latest_received_wav(dir: &Path) -> Option<PathBuf> {
    std::fs::read_dir(dir)
        .ok()?
        .filter_map(std::result::Result::ok)
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("dump-") && n.ends_with("-dec.wav"))
        })
        .max_by_key(|p| p.metadata().and_then(|m| m.modified()).ok())
}

/// Result of analysing a recording for a tone, with diagnostics to tell apart
/// "tone present" / "audio but wrong tone" / "silence (no media)".
#[derive(Debug, Clone, Copy, Default)]
pub struct ToneAnalysis {
    /// Goertzel score at the target frequency (~1.0 = present, ~0 = absent).
    pub score: f64,
    /// RMS amplitude of the analysed window (~0 = silence / no audio received).
    pub rms: f64,
    /// Number of samples analysed.
    pub samples: usize,
}

/// Analyse the last `window` of the recorded WAV at `path` for `freq`
/// (16-bit PCM; channel 0 if multi-channel).
pub fn analyze_tone(path: &Path, freq: u32, window: std::time::Duration) -> Result<ToneAnalysis> {
    let bytes =
        std::fs::read(path).with_context(|| format!("read recording {}", path.display()))?;
    let (samples, sample_rate) =
        parse_wav_pcm16(&bytes).with_context(|| format!("parse recording {}", path.display()))?;
    // Analyze the tail: the tone may begin after call/media setup.
    let want = (f64::from(sample_rate) * window.as_secs_f64()) as usize;
    let tail = &samples[samples.len().saturating_sub(want)..];
    let rms = if tail.is_empty() {
        0.0
    } else {
        (tail.iter().map(|&x| f64::from(x).powi(2)).sum::<f64>() / tail.len() as f64).sqrt()
    };
    Ok(ToneAnalysis {
        score: tone_score(tail, sample_rate, f64::from(freq)),
        rms,
        samples: tail.len(),
    })
}

/// Tolerant RIFF/WAVE reader for 16-bit PCM → (channel-0 samples, sample_rate).
/// Reads the `data` chunk to EOF, ignoring its size field, which is 0/stale
/// while libsndfile is still writing the file (we read it mid-call).
fn parse_wav_pcm16(b: &[u8]) -> Result<(Vec<i16>, u32)> {
    let u16le = |i: usize| u16::from_le_bytes([b[i], b[i + 1]]);
    let u32le = |i: usize| u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]]);
    if b.len() < 12 || &b[0..4] != b"RIFF" || &b[8..12] != b"WAVE" {
        bail!("not a RIFF/WAVE file");
    }
    let (mut sample_rate, mut channels, mut bits) = (0u32, 1u16, 16u16);
    let mut data: Option<&[u8]> = None;
    let mut pos = 12;
    while pos + 8 <= b.len() {
        let id = &b[pos..pos + 4];
        let declared = u32le(pos + 4) as usize;
        let body = pos + 8;
        if id == b"fmt " && body + 16 <= b.len() {
            channels = u16le(body + 2).max(1);
            sample_rate = u32le(body + 4);
            bits = u16le(body + 14);
            pos = body + declared.max(16);
        } else if id == b"data" {
            // size field unreliable (0 while still recording) → read to EOF
            let end = if declared == 0 || body + declared > b.len() {
                b.len()
            } else {
                body + declared
            };
            data = Some(&b[body..end]);
            break;
        } else if declared == 0 {
            break; // can't advance past a zero-size non-data chunk
        } else {
            pos = body + declared;
        }
    }
    if bits != 16 {
        bail!("expected 16-bit PCM, got {bits}-bit");
    }
    let data = data.context("no data chunk")?;
    let ch = channels as usize;
    // one frame = `ch` interleaved samples; keep channel 0
    let samples = data
        .chunks_exact(2 * ch)
        .map(|f| i16::from_le_bytes([f[0], f[1]]))
        .collect();
    Ok((samples, sample_rate))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    fn sine(freq: f64, sr: u32, n: usize) -> Vec<i16> {
        (0..n)
            .map(|i| (8000.0 * (2.0 * PI * freq * i as f64 / f64::from(sr)).sin()) as i16)
            .collect()
    }

    #[test]
    fn detects_matching_tone_rejects_others() {
        let sr = 8000;
        let s = sine(440.0, sr, 8000);
        assert!(tone_score(&s, sr, 440.0) > 0.8, "expected strong 440 match");
        assert!(
            tone_score(&s, sr, 1000.0) < 0.1,
            "1000 Hz should not match a 440 tone"
        );
    }

    #[test]
    fn silence_and_empty_score_zero() {
        assert_eq!(tone_score(&[], 8000, 440.0), 0.0);
        assert_eq!(tone_score(&[0i16; 4000], 8000, 440.0), 0.0);
    }

    /// Build a minimal PCM WAV; `finalized=false` mimics libsndfile mid-write
    /// (RIFF/data size fields left at 0, data still on disk).
    fn wav(samples: &[i16], sr: u32, finalized: bool) -> Vec<u8> {
        let data: Vec<u8> = samples.iter().flat_map(|s| s.to_le_bytes()).collect();
        let (riff, dlen) = if finalized {
            ((36 + data.len()) as u32, data.len() as u32)
        } else {
            (0, 0)
        };
        let mut v = Vec::new();
        v.extend_from_slice(b"RIFF");
        v.extend_from_slice(&riff.to_le_bytes());
        v.extend_from_slice(b"WAVEfmt ");
        v.extend_from_slice(&16u32.to_le_bytes());
        v.extend_from_slice(&1u16.to_le_bytes()); // PCM
        v.extend_from_slice(&1u16.to_le_bytes()); // mono
        v.extend_from_slice(&sr.to_le_bytes());
        v.extend_from_slice(&(sr * 2).to_le_bytes());
        v.extend_from_slice(&2u16.to_le_bytes());
        v.extend_from_slice(&16u16.to_le_bytes());
        v.extend_from_slice(b"data");
        v.extend_from_slice(&dlen.to_le_bytes());
        v.extend_from_slice(&data);
        v
    }

    #[test]
    fn parses_wav_even_with_unfinalized_size_fields() {
        let s = sine(440.0, 8000, 8000);
        for finalized in [true, false] {
            let (samples, sr) = parse_wav_pcm16(&wav(&s, 8000, finalized)).unwrap();
            assert_eq!(sr, 8000);
            assert_eq!(samples.len(), s.len(), "finalized={finalized}");
            assert!(
                tone_score(&samples, sr, 440.0) > 0.8,
                "finalized={finalized}"
            );
        }
    }

    #[test]
    fn threshold_separates_tone_from_noise() {
        let sr = 8000;
        // alternating +/- = high-frequency content, not 440 Hz
        let noise: Vec<i16> = (0..8000)
            .map(|i| if i % 2 == 0 { 4000 } else { -4000 })
            .collect();
        assert!(tone_score(&noise, sr, 440.0) < TONE_THRESHOLD);
        assert!(tone_score(&sine(440.0, sr, 8000), sr, 440.0) >= TONE_THRESHOLD);
    }
}
