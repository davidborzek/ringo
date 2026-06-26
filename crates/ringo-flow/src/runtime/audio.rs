//! Tone detection on received call audio (for `verify-audio`) and WAV output
//! (for `--save-audio`).
//!
//! The backend captures each agent's received (and, when saving, sent) audio
//! in-process via ringo's own ausrc/auplay module — no baresip sndfile, no WAV
//! dumps on disk. We run a [Goertzel](https://en.wikipedia.org/wiki/Goertzel_algorithm)
//! filter on the captured samples at the expected frequency: the score is ~1.0
//! for a clean tone at that frequency and ~0 for silence/noise/other tones,
//! independent of sample count.

use anyhow::{Context, Result};
use std::path::Path;

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

/// Analyse the last `window` of mono 16-bit `samples` at `sample_rate` for
/// `freq`. The tail is used because the tone may begin after media setup.
pub fn analyze_tone_samples(
    samples: &[i16],
    sample_rate: u32,
    freq: u32,
    window: std::time::Duration,
) -> ToneAnalysis {
    let want = (f64::from(sample_rate) * window.as_secs_f64()) as usize;
    let tail = &samples[samples.len().saturating_sub(want)..];
    let rms = if tail.is_empty() {
        0.0
    } else {
        (tail.iter().map(|&x| f64::from(x).powi(2)).sum::<f64>() / tail.len() as f64).sqrt()
    };
    ToneAnalysis {
        score: tone_score(tail, sample_rate, f64::from(freq)),
        rms,
        samples: tail.len(),
    }
}

/// Write mono 16-bit PCM `samples` at `sample_rate` to `path` as a WAV file
/// (used by `--save-audio`; the audio is captured in-process, so we serialise
/// it ourselves — no libsndfile).
pub fn write_wav(path: &Path, samples: &[i16], sample_rate: u32) -> Result<()> {
    let data_len = samples.len() * 2;
    let byte_rate = sample_rate * 2; // mono, 16-bit
    let mut v = Vec::with_capacity(44 + data_len);
    v.extend_from_slice(b"RIFF");
    v.extend_from_slice(&((36 + data_len) as u32).to_le_bytes());
    v.extend_from_slice(b"WAVE");
    v.extend_from_slice(b"fmt ");
    v.extend_from_slice(&16u32.to_le_bytes()); // fmt chunk size
    v.extend_from_slice(&1u16.to_le_bytes()); // PCM
    v.extend_from_slice(&1u16.to_le_bytes()); // mono
    v.extend_from_slice(&sample_rate.to_le_bytes());
    v.extend_from_slice(&byte_rate.to_le_bytes());
    v.extend_from_slice(&2u16.to_le_bytes()); // block align
    v.extend_from_slice(&16u16.to_le_bytes()); // bits/sample
    v.extend_from_slice(b"data");
    v.extend_from_slice(&(data_len as u32).to_le_bytes());
    for s in samples {
        v.extend_from_slice(&s.to_le_bytes());
    }
    std::fs::write(path, v).with_context(|| format!("write WAV {}", path.display()))
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

    #[test]
    fn write_wav_roundtrips_header_and_samples() {
        let s = sine(440.0, 8000, 8000);
        let dir = std::env::temp_dir();
        let path = dir.join("ringo-flow-write-wav-test.wav");
        write_wav(&path, &s, 8000).unwrap();
        let b = std::fs::read(&path).unwrap();
        let _ = std::fs::remove_file(&path);
        assert_eq!(&b[0..4], b"RIFF");
        assert_eq!(&b[8..12], b"WAVE");
        assert_eq!(u32::from_le_bytes([b[24], b[25], b[26], b[27]]), 8000); // srate
        // 44-byte header + 2 bytes/sample
        assert_eq!(b.len(), 44 + s.len() * 2);
        let parsed: Vec<i16> = b[44..]
            .chunks_exact(2)
            .map(|c| i16::from_le_bytes([c[0], c[1]]))
            .collect();
        assert_eq!(parsed, s);
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
