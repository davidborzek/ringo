//! Speech for ringo agents: text-to-synthesis of prompts and (building blocks
//! for) recognition — the speech layer native bots and the MCP `speak` tool
//! share. Telephony stays in ringo-agent/ringo-core; this crate turns text
//! into PCM and PCM into text, nothing more.
//!
//! The default build has NO speech backend (traits + WAV utilities only) —
//! enable the `sherpa` feature for the offline sherpa-onnx backend (VITS /
//! piper voices, Silero VAD, ASR).

#![warn(missing_docs)]

mod wav;

#[cfg(feature = "sherpa")]
mod sherpa_stt;

#[cfg(feature = "sherpa")]
mod sherpa_tts;

pub use wav::{wav_duration, write_mono_wav};

/// Neutral TTS configuration — no backend types leak here.
#[derive(Debug, Clone)]
pub struct TtsConfig {
    /// Directory of the voice to load (layout is backend-neutral: a
    /// `model.onnx` + `tokens.txt` + `espeak-ng-data/` VITS/piper voice for
    /// the sherpa backend; other backends define their own).
    pub model_dir: std::path::PathBuf,
    /// Speed factor (1.0 = natural).
    pub speed: f32,
}

/// Load a TTS engine for `config`. The backend is chosen at compile time
/// (feature `sherpa`); callers see only [`Synthesizer`].
///
/// Errors with a clear message when no backend is compiled in.
pub fn load_tts(config: &TtsConfig) -> anyhow::Result<SynthesizerHolder> {
    #[cfg(feature = "sherpa")]
    {
        Ok(SynthesizerHolder(Box::new(sherpa_tts::SherpaTts::load(
            &config.model_dir,
            config.speed,
        )?)))
    }
    #[cfg(not(feature = "sherpa"))]
    {
        let _ = config;
        anyhow::bail!(
            "no TTS backend compiled in — rebuild ringo-speech/ringo-mcp with \
             the `sherpa` feature"
        );
    }
}

/// Synthesized speech: mono s16 PCM at `sample_rate`, ready for the agent
/// audio paths (aubridge `aufile`, `push_tx_audio`).
#[derive(Debug, Clone)]
pub struct Synthesized {
    /// Mono s16 samples.
    pub samples: Vec<i16>,
    /// Sample rate in Hz.
    pub sample_rate: u32,
}

impl Synthesized {
    /// Playback duration.
    pub fn duration(&self) -> std::time::Duration {
        if self.sample_rate == 0 {
            return std::time::Duration::ZERO;
        }
        std::time::Duration::from_secs_f64(self.samples.len() as f64 / self.sample_rate as f64)
    }
}

/// A recognized utterance (one complete speech segment).
#[derive(Debug, Clone)]
pub struct Utterance {
    /// The transcribed text.
    pub text: String,
    /// The utterance's audio duration in ms.
    pub duration_ms: u64,
}

/// A speech-to-text recognizer: feed audio, get utterances.
///
/// The recognizer maintains VAD state internally; call [`Recognizer::feed`]
/// with each incoming audio chunk. Utterances are emitted when the VAD detects
/// a complete speech segment (speech + trailing silence).
pub trait Recognizer: Send {
    /// Feed a chunk of mono s16 PCM; returns any utterances that completed.
    fn feed(&mut self, pcm: &[i16], sample_rate: u32) -> Vec<Utterance>;

    /// Flush any pending speech (e.g. at the end of a call).
    fn flush(&mut self) -> Vec<Utterance>;
}

/// An owned STT engine behind the vendor-agnostic trait.
pub struct RecognizerHolder(pub(crate) Box<dyn Recognizer>);

impl std::fmt::Debug for RecognizerHolder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Recognizer")
    }
}

impl RecognizerHolder {
    /// Feed a chunk (delegates to the engine).
    pub fn feed(&mut self, pcm: &[i16], rate: u32) -> Vec<Utterance> {
        self.0.feed(pcm, rate)
    }

    /// Flush pending speech.
    pub fn flush(&mut self) -> Vec<Utterance> {
        self.0.flush()
    }
}

/// Neutral STT configuration.
#[derive(Debug, Clone)]
pub struct SttConfig {
    /// The VAD model file (e.g. silero_vad.onnx).
    pub vad_model: std::path::PathBuf,
    /// The ASR model directory (e.g. a whisper-tiny dir with encoder/decoder/tokens).
    pub asr_model_dir: std::path::PathBuf,
    /// Recognition language ("de", "en", ...; "auto" for auto-detect).
    pub language: String,
}

/// Load an STT engine. The backend is chosen at compile time (feature
/// `sherpa`); callers see only [`Recognizer`].
pub fn load_stt(config: &SttConfig) -> anyhow::Result<RecognizerHolder> {
    #[cfg(feature = "sherpa")]
    {
        Ok(RecognizerHolder(Box::new(sherpa_stt::SherpaStt::load(
            &config.vad_model,
            &config.asr_model_dir,
            &config.language,
        )?)))
    }
    #[cfg(not(feature = "sherpa"))]
    {
        let _ = config;
        anyhow::bail!("no STT backend compiled in — rebuild ringo-speech with the `sherpa` feature")
    }
}

/// A owned, shareable TTS engine behind the vendor-agnostic trait —
/// what [`load_tts`] hands out. (A newtype so the trait object is nameable
/// without leaking any backend type.)
pub struct SynthesizerHolder(pub(crate) Box<dyn Synthesizer>);

impl std::fmt::Debug for SynthesizerHolder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Synthesizer")
    }
}

impl SynthesizerHolder {
    /// Synthesize `text` (delegates to the engine).
    pub fn synth(&self, text: &str) -> anyhow::Result<Synthesized> {
        self.0.synth(text)
    }
}

/// A text-to-speech engine.
pub trait Synthesizer: Send + Sync {
    /// Synthesize `text` to mono s16 PCM.
    fn synth(&self, text: &str) -> anyhow::Result<Synthesized>;
}
