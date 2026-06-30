//! Language-neutral audio verbs: `send_audio` (switch an agent's call source),
//! `verify_audio` (detect a tone in what an agent received), and
//! `verify_audio_connection` (assert two-way audio). Headless — no sound
//! hardware; sent/received audio is rendered and captured in-process by the
//! backend (ringo's own ausrc/auplay), so verification needs no sndfile.

use super::ctx::Ctx;
use super::duration::parse_duration;
use crate::runtime::report::Event;
use ringo_agent::audio::{self, ToneAnalysis};
use std::sync::Arc;
use std::time::Duration;

/// How many `verify_audio` windows to record before giving up — settle-time
/// tolerance for real-carrier media (the loop exits on the first match).
const VERIFY_AUDIO_ATTEMPTS: u32 = 5;
/// Brief gap between switching one side silent and the other side sending in
/// `verify_audio_connection`, so the `ausrc` commands apply in order.
const AUDIO_SWITCH_GAP: Duration = Duration::from_millis(250);
const DEFAULT_TONE: u32 = 1000;
const DEFAULT_WINDOW: Duration = Duration::from_secs(2);

/// What `send_audio` feeds into the call (built by `tone()`/`file()`/`silent()`).
#[derive(Clone)]
pub enum AudioSpec {
    Tone(u32),
    File(String),
    Silent,
}

impl AudioSpec {
    /// (baresip ausrc spec, human description). No driver names leak to output.
    fn parts(&self) -> (String, String) {
        match self {
            AudioSpec::Tone(f) => (format!("ausine,{f}"), format!("tone {f} Hz")),
            AudioSpec::File(p) => (format!("aufile,{p}"), format!("file {p}")),
            // Clocked silence (a 0 Hz sine), NOT an idle source: it keeps the RTP
            // TX clock running so out-of-band DTMF (RTP telephone-events) still
            // transmits while "silent". An idle source (aubridge) stops the TX
            // clock — fine for audio, but DTMF after going silent is then dropped.
            AudioSpec::Silent => ("ausine,0".to_string(), "silence".to_string()),
        }
    }
}

pub fn send_audio(ctx: &Arc<Ctx>, name: &str, spec: AudioSpec) -> Result<(), String> {
    let (ausrc, detail) = spec.parts();
    ctx.set_audio_source(name, &ausrc)?;
    ctx.emit_action(name, "send-audio", Some(&detail));
    Ok(())
}

/// Record the agent's received audio and poll for `freq`, returning
/// `(detected, diagnostics)`. `rms ~0` means no audio arrived (media not
/// flowing) vs. audio present but the wrong tone.
fn detect(
    ctx: &Arc<Ctx>,
    name: &str,
    freq: u32,
    window: Duration,
) -> Result<(bool, String), String> {
    // Warm the RX stream before the first sleep so the opening window already
    // captures audio (the stream is lazy, started on first touch).
    ctx.prime_received_audio(name)?;
    let mut last = ToneAnalysis::default();
    for _ in 0..VERIFY_AUDIO_ATTEMPTS {
        std::thread::sleep(window);
        // The worker streams its received audio over the agent proto; the Goertzel
        // analysis runs here, on the parent, over the streamed tail.
        last = ctx.analyze_tone(name, freq, window)?;
        if last.score >= audio::TONE_THRESHOLD {
            return Ok((true, fmt_analysis(&last)));
        }
    }
    Ok((false, fmt_analysis(&last)))
}

fn fmt_analysis(a: &ToneAnalysis) -> String {
    format!(
        "score {:.2}, rms {:.0}, {} samples",
        a.score, a.rms, a.samples
    )
}

/// `a.verify_audio(1000, "2s")` — assert `a` is receiving the tone.
pub fn verify_audio(ctx: &Arc<Ctx>, name: &str, freq: i64, within: &str) -> Result<(), String> {
    let window = parse_duration(within)?;
    let freq = freq.max(0) as u32;
    let (ok, actual) = detect(ctx, name, freq, window)?;
    ctx.emit(&Event::Assertion {
        label: Some(name),
        expect: format!("audio tone {freq} Hz"),
        ok,
        actual: Some(actual.clone()),
    });
    if ok {
        Ok(())
    } else {
        Err(format!(
            "verify_audio on `{name}`: tone {freq} Hz not detected ({actual})"
        ))
    }
}

/// One direction of `verify_audio_connection`: `from` sends, `to` must receive.
fn direction(
    ctx: &Arc<Ctx>,
    from: &str,
    to: &str,
    freq: u32,
    window: Duration,
) -> Result<(), String> {
    send_audio(ctx, from, AudioSpec::Tone(freq))?;
    let (ok, actual) = detect(ctx, to, freq, window)?;
    ctx.emit(&Event::Assertion {
        label: Some(to),
        expect: format!("audio {from} → {to} {freq} Hz"),
        ok,
        actual: Some(actual.clone()),
    });
    send_audio(ctx, from, AudioSpec::Silent)?;
    if ok {
        Ok(())
    } else {
        Err(format!(
            "verify_audio_connection {from} → {to}: tone {freq} Hz not detected ({actual})"
        ))
    }
}

/// `verify_audio_connection(a, b)` — assert two-way audio (a→b, then b→a).
pub fn verify_audio_connection(ctx: &Arc<Ctx>, a: &str, b: &str) -> Result<(), String> {
    direction(ctx, a, b, DEFAULT_TONE, DEFAULT_WINDOW)?;
    std::thread::sleep(AUDIO_SWITCH_GAP);
    direction(ctx, b, a, DEFAULT_TONE, DEFAULT_WINDOW)?;
    Ok(())
}
