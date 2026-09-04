//! The sherpa-onnx offline TTS backend (feature `sherpa`): VITS-family voices
//! (piper models) from a model directory, fully local — no network, no cloud.
//!
//! Expected directory layout (e.g. a downloaded piper voice like
//! `de_DE-thorsten-high`):
//!
//! ```text
//! <dir>/model.onnx        the voice
//! <dir>/tokens.txt        the phoneme vocabulary
//! <dir>/espeak-ng-data/   phonemizer data (non-English voices)
//! ```
//!
//! Voices: <https://huggingface.co/rhasspy/piper-voices> (espeak-ng-data from
//! the piper-phonemize releases). Licenses are per-voice — check before
//! shipping one.

use crate::{Synthesized, Synthesizer};
use anyhow::{Context, Result, bail};
use sherpa_onnx::{
    GenerationConfig, OfflineTts, OfflineTtsConfig, OfflineTtsModelConfig,
    OfflineTtsVitsModelConfig,
};
use std::path::Path;

/// A local VITS/piper voice, loaded once and reused for every synthesis.
pub struct SherpaTts {
    tts: OfflineTts,
    /// Speed factor (1.0 = natural).
    speed: f32,
}

impl SherpaTts {
    /// Load the VITS voice in `model_dir` (see the module docs for the layout).
    pub fn load(model_dir: &Path, speed: f32) -> Result<Self> {
        let model = model_dir.join("model.onnx");
        let tokens = model_dir.join("tokens.txt");
        let data_dir = model_dir.join("espeak-ng-data");
        for p in [&model, &tokens] {
            if !p.is_file() {
                bail!(
                    "TTS model dir `{}` is missing {} (expected model.onnx, \
                     tokens.txt, espeak-ng-data/)",
                    model_dir.display(),
                    p.file_name().unwrap_or_default().to_string_lossy()
                );
            }
        }
        let config = OfflineTtsConfig {
            model: OfflineTtsModelConfig {
                vits: OfflineTtsVitsModelConfig {
                    model: Some(model.to_string_lossy().into_owned()),
                    tokens: Some(tokens.to_string_lossy().into_owned()),
                    // Optional pieces: espeak-ng-data for non-English voices,
                    // lexicon/dict for some languages.
                    data_dir: data_dir
                        .is_dir()
                        .then(|| data_dir.to_string_lossy().into_owned()),
                    ..Default::default()
                },
                ..Default::default()
            },
            ..Default::default()
        };
        let tts = OfflineTts::create(&config)
            .context("create the sherpa-onnx offline TTS engine from the model dir")?;
        Ok(Self { tts, speed })
    }
}

impl Synthesizer for SherpaTts {
    fn synth(&self, text: &str) -> Result<Synthesized> {
        if text.trim().is_empty() {
            bail!("nothing to synthesize (empty text)");
        }
        let audio = self
            .tts
            .generate_with_config::<fn(&[f32], f32) -> bool>(
                text,
                &GenerationConfig {
                    speed: self.speed,
                    ..Default::default()
                },
                None,
            )
            .context("synthesize text")?;
        let samples: Vec<i16> = audio
            .samples()
            .iter()
            .map(|&f| (f.clamp(-1.0, 1.0) * i16::MAX as f32) as i16)
            .collect();
        Ok(Synthesized {
            samples,
            sample_rate: audio.sample_rate().max(1) as u32,
        })
    }
}
