//! The sherpa-onnx offline STT backend (feature `sherpa`): Silero VAD
//! segments the audio into utterances, a Whisper (or other offline) model
//! transcribes each one. Fully local.
//!
//! Expected model layout:
//!
//! ```text
//! <vad>/silero_vad.onnx             the VAD model
//! <asr>/tiny-encoder.int8.onnx     the ASR encoder
//! <asr>/tiny-decoder.int8.onnx     the ASR decoder
//! <asr>/tiny-tokens.txt            the vocabulary
//! ```

use crate::{Recognizer, Utterance};
use anyhow::{Context, Result, bail};
use sherpa_onnx::{
    OfflineModelConfig, OfflineRecognizer, OfflineRecognizerConfig, OfflineWhisperModelConfig,
    SileroVadModelConfig, VadModelConfig, VoiceActivityDetector,
};
use std::path::Path;

/// A VAD + offline-ASR recognizer, fully local.
pub struct SherpaStt {
    vad: VoiceActivityDetector,
    recognizer: OfflineRecognizer,
    /// The ASR model's expected sample rate (Whisper = 16000).
    sample_rate: u32,
}

impl SherpaStt {
    /// Load from a VAD model path and a Whisper model directory.
    pub fn load(vad_model: &Path, asr_dir: &Path, language: &str) -> Result<Self> {
        if !vad_model.is_file() {
            bail!("VAD model `{}` not found", vad_model.display());
        }
        let encoder = find_model(asr_dir, "encoder")?;
        let decoder = find_model(asr_dir, "decoder")?;
        let tokens = find_model(asr_dir, "tokens")?;

        let vad = VoiceActivityDetector::create(
            &VadModelConfig {
                silero_vad: SileroVadModelConfig {
                    model: Some(vad_model.to_string_lossy().into_owned()),
                    ..Default::default()
                },
                ..Default::default()
            },
            60.0, // buffer seconds
        )
        .context("create the Silero VAD engine")?;

        let recognizer = OfflineRecognizer::create(&OfflineRecognizerConfig {
            model_config: OfflineModelConfig {
                whisper: OfflineWhisperModelConfig {
                    encoder: Some(encoder.to_string_lossy().into_owned()),
                    decoder: Some(decoder.to_string_lossy().into_owned()),
                    language: (language != "auto").then(|| language.to_string()),
                    task: Some("transcribe".to_string()),
                    tail_paddings: 0,
                    enable_token_timestamps: false,
                    enable_segment_timestamps: false,
                },
                tokens: Some(tokens.to_string_lossy().into_owned()),
                ..Default::default()
            },
            ..Default::default()
        })
        .context("create the offline ASR engine (Whisper)")?;

        Ok(Self {
            vad,
            recognizer,
            sample_rate: 16000,
        })
    }

    fn recognize(&self, samples: &[f32]) -> Option<String> {
        let stream = self.recognizer.create_stream();
        stream.accept_waveform(self.sample_rate as i32, samples);
        self.recognizer.decode(&stream);
        stream.get_result().map(|r| r.text.trim().to_string())
    }
}

/// Find `*-<kind>.onnx` / `*-<kind>.txt` in a model dir, preferring int8.
fn find_model(dir: &Path, kind: &str) -> Result<std::path::PathBuf> {
    let mut candidates: Vec<_> = std::fs::read_dir(dir)
        .with_context(|| format!("read ASR model dir `{}`", dir.display()))?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            let name = p
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_lowercase();
            name.contains(kind)
        })
        .collect();
    candidates.sort();
    // Prefer int8 (smaller, faster).
    candidates
        .iter()
        .find(|p| p.to_string_lossy().contains("int8"))
        .or_else(|| candidates.first())
        .cloned()
        .with_context(|| format!("no `{kind}` model in `{}`", dir.display()))
}

impl Recognizer for SherpaStt {
    fn feed(&mut self, pcm: &[i16], rate: u32) -> Vec<Utterance> {
        // The VAD and Whisper both expect f32 in [-1, 1].
        let samples: Vec<f32> = pcm.iter().map(|&s| s as f32 / i16::MAX as f32).collect();
        // Sherpa's VAD works at 16 kHz; resample if needed.
        let samples = if rate != self.sample_rate {
            resample(&samples, rate, self.sample_rate)
        } else {
            samples
        };
        self.vad.accept_waveform(&samples);

        let mut utterances = Vec::new();
        while self.vad.detected() {
            if let Some(segment) = self.vad.front() {
                let dur_ms = segment.n() as u64 * 1000 / self.sample_rate as u64;
                if let Some(text) = self.recognize(segment.samples()) {
                    if !text.is_empty() {
                        utterances.push(Utterance {
                            text,
                            duration_ms: dur_ms,
                        });
                    }
                }
            }
            self.vad.pop();
        }
        utterances
    }

    fn flush(&mut self) -> Vec<Utterance> {
        self.vad.flush();
        let mut utterances = Vec::new();
        while self.vad.detected() {
            if let Some(segment) = self.vad.front() {
                let dur_ms = segment.n() as u64 * 1000 / self.sample_rate as u64;
                if let Some(text) = self.recognize(segment.samples()) {
                    if !text.is_empty() {
                        utterances.push(Utterance {
                            text,
                            duration_ms: dur_ms,
                        });
                    }
                }
            }
            self.vad.pop();
        }
        utterances
    }
}

/// Linear resample (nearest) — VAD and Whisper are tolerant of minor artifacts.
fn resample(samples: &[f32], from: u32, to: u32) -> Vec<f32> {
    if from == to || from == 0 {
        return samples.to_vec();
    }
    let ratio = to as f64 / from as f64;
    let n = (samples.len() as f64 * ratio).ceil() as usize;
    (0..n)
        .map(|i| {
            let src = (i as f64 / ratio) as usize;
            samples[src.min(samples.len() - 1)]
        })
        .collect()
}
