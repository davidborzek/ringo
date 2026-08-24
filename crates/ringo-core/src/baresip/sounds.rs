//! Alert-sound playback — ring/ringback/busy/error/message.
//!
//! Two paths, because the two cases genuinely differ:
//!
//! * The **defaults** are generated, every one of them, from `crate::tones` —
//!   call-progress tones from their specification, ring and message as struck
//!   chimes. ringo therefore embeds no audio at all: nothing to license, and a
//!   single binary with no share directory to install. There is no file to
//!   open, so their samples go to `play_tone` as a libre `mbuf`.
//! * A **user's file** goes to `play_file`, which opens and decodes it itself.
//!   Only the path stays resident, so a long ringtone does not occupy memory
//!   for the process lifetime, and baresip's own WAV loader decides what it can
//!   read instead of us second-guessing it. The price is that the read happens
//!   at play time, on the RE thread, inside the event handler — the same thing
//!   baresip's own menu module does. Fine for a file of a few hundred kB in the
//!   page cache; a large file on cold or networked storage stalls SIP and RTP
//!   for as long as it takes to read.
//!
//! [`load_sounds`] installs the set at session start and preflights every
//! configured file — header only, never the samples — so a bad path is a
//! warning while ringo starts rather than silence at the first call. If `play_file` still fails at play time, the
//! built-in tone takes over for that alert.

use std::ffi::CString;
use std::os::raw::{c_char, c_int};
use std::path::Path;
use std::sync::{Arc, OnceLock, RwLock};

use super::bindings::*;

/// The alerts ringo plays. The `Alert as usize` discriminants index
/// [`SoundSet`], so the order here must match [`Alert::ALL`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Alert {
    /// Incoming call. Loops until the call is answered or gone.
    Ring,
    /// Outgoing call, remote end is ringing. Loops until the call is up or gone.
    Ringback,
    /// Outgoing call rejected as busy (SIP 486/600/603). Played once.
    Busy,
    /// Outgoing call failed for any other reason. Played once.
    Error,
    /// New voicemail (MWI). Played once.
    Message,
}

impl Alert {
    /// Every alert, in `Alert as usize` order.
    pub const ALL: [Alert; 5] = [
        Alert::Ring,
        Alert::Ringback,
        Alert::Busy,
        Alert::Error,
        Alert::Message,
    ];

    /// The name this alert is configured under (`[sounds] ring = "…"`).
    pub fn key(self) -> &'static str {
        match self {
            Alert::Ring => "ring",
            Alert::Ringback => "ringback",
            Alert::Busy => "busy",
            Alert::Error => "error",
            Alert::Message => "message",
        }
    }

    /// The alert configured under `key`, if any.
    pub fn from_key(key: &str) -> Option<Alert> {
        Alert::ALL.into_iter().find(|a| a.key() == key)
    }

    /// The tone ringo falls back to when nothing is configured.
    ///
    /// Everything is generated at 48 kHz rather than shipped as audio: exact
    /// cadences, no 8 kHz mu-law, and no third-party file to license.
    ///
    /// `None` only if a built-in spec fails to parse, which would be a build
    /// problem rather than anything a user can cause.
    fn default_pcm(self) -> Option<Pcm> {
        let spec = match self {
            // Ring and message are chimes — struck notes with a decay, each
            // carrying its own trailing silence so a looping alert needs no
            // gap bolted on. Nothing specifies what an incoming call should
            // sound like, but "a motif of struck notes" is still a description
            // rather than a recording, which keeps ringo free of foreign audio.
            Alert::Ring => {
                return Some(to_pcm(crate::tones::render_chime(
                    &crate::tones::RING,
                    SYNTH_RATE,
                )));
            }
            Alert::Message => {
                return Some(to_pcm(crate::tones::render_chime(
                    &crate::tones::MESSAGE,
                    SYNTH_RATE,
                )));
            }
            Alert::Ringback => crate::tones::DE_RINGBACK,
            Alert::Busy => crate::tones::DE_BUSY,
            Alert::Error => crate::tones::DE_CONGESTION,
        };
        // The built-in tones go through the very same parser a user's config
        // does, so that path is exercised on every start.
        synth(spec, self.periods()).ok()
    }

    /// How many cadence periods to render. One is enough for a tone that loops
    /// — `play_tone` repeats it seamlessly. The one-shots need more to be
    /// recognised: a single 480 ms burst of busy tone reads as a glitch rather
    /// than as a state.
    fn periods(self) -> u32 {
        match self {
            Alert::Ring | Alert::Ringback => 1,
            Alert::Busy => 3,
            Alert::Error | Alert::Message => 4,
        }
    }

    /// `-1` loops until [`stop_alert`]; `1` plays once, then baresip frees the
    /// tone itself (its destructor clears the slot — see [`alert_playp`]).
    fn repeat(self) -> c_int {
        match self {
            Alert::Ring | Alert::Ringback => -1,
            Alert::Busy | Alert::Error | Alert::Message => 1,
        }
    }

    /// How [`Self::repeat`] reads in a log line.
    fn mode(self) -> &'static str {
        if self.repeat() < 0 { "looping" } else { "once" }
    }
}

/// Parsed WAV: raw S16LE PCM samples + sample rate + channel count.
pub(super) struct Pcm {
    pub(super) samples: Vec<u8>,
    pub(super) srate: u32,
    pub(super) channels: u8,
}

// ─── Sound set ───────────────────────────────────────────────────────────────

/// How much of a configured file the preflight reads. The header sits at the
/// front; the samples are baresip's business at play time.
const PROBE_BYTES: usize = 64 * 1024;

const N_ALERTS: usize = Alert::ALL.len();

/// Sample rate for synthesized tones. The rate every current audio backend
/// opens without argument, and far above what an 8 kHz tone can carry.
const SYNTH_RATE: u32 = 48000;

/// The prefix that marks a configured value as a tone to synthesize rather
/// than a file to play.
const TONE_PREFIX: &str = "tone:";

/// Parse a tone spec and render `periods` of it into the shape `play_tone`
/// wants. `Err` carries a message that completes "the tone …".
fn synth(spec: &str, periods: u32) -> Result<Pcm, crate::tones::ParseError> {
    let tone: crate::tones::Tone = spec.parse()?;
    Ok(to_pcm(crate::tones::render(&tone, SYNTH_RATE, periods)))
}

/// Wrap rendered mono samples in the shape `play_tone` reads.
fn to_pcm(samples: Vec<i16>) -> Pcm {
    let mut bytes = Vec::with_capacity(samples.len() * 2);
    for s in samples {
        bytes.extend_from_slice(&s.to_le_bytes());
    }
    Pcm {
        samples: bytes,
        srate: SYNTH_RATE,
        channels: 1,
    }
}

/// Where an alert's audio comes from.
enum Source {
    /// The built-in tone — synthesized or parsed once, played from an mbuf.
    Builtin(Pcm),
    /// A file the user configured, as an absolute path for `play_file`.
    File(CString),
}

/// The tones in use, indexed by `Alert as usize`. `None` = silent: either the
/// user configured `off`, or their file was unusable AND the built-in default
/// was unavailable (which would be a build problem, not a user one).
struct SoundSet([Option<Source>; N_ALERTS]);

impl SoundSet {
    /// The built-in defaults, with `overrides` applied on top. Each override is
    /// a `(key, value)` pair where `value` is either a WAV path or `off`.
    /// Anything unusable falls back to the built-in default with a warning — a
    /// typo in the config must never leave an incoming call silent.
    fn resolve(overrides: &[(String, String)]) -> Self {
        let mut set = SoundSet(Alert::ALL.map(|a| a.default_pcm().map(Source::Builtin)));
        for (key, value) in overrides {
            let Some(alert) = Alert::from_key(key) else {
                crate::rlog!(Warn, "sounds: unknown alert '{key}' — ignored");
                continue;
            };
            let value = value.trim();
            if is_muted(value) {
                set.0[alert as usize] = None;
                continue;
            }
            if let Some(spec) = value.strip_prefix(TONE_PREFIX) {
                match synth(spec.trim(), alert.periods()) {
                    Ok(pcm) => set.0[alert as usize] = Some(Source::Builtin(pcm)),
                    Err(e) => crate::rlog!(Warn, "sounds: {key}: the tone '{spec}' {e}"),
                }
                continue;
            }
            if let Some(source) = load_file(alert, value) {
                set.0[alert as usize] = Some(source);
            }
        }
        set
    }

    fn get(&self, alert: Alert) -> Option<&Source> {
        self.0[alert as usize].as_ref()
    }
}

/// Values that turn an alert off rather than naming a file.
fn is_muted(value: &str) -> bool {
    value.is_empty() || value.eq_ignore_ascii_case("off") || value.eq_ignore_ascii_case("none")
}

/// The active sound set. Swapped wholesale by [`load_sounds`]; read (and the
/// `Arc` cloned) by [`play_alert`], so the RE thread never holds the lock
/// across the FFI calls.
fn sound_slot() -> &'static RwLock<Arc<SoundSet>> {
    static SOUNDS: OnceLock<RwLock<Arc<SoundSet>>> = OnceLock::new();
    SOUNDS.get_or_init(|| RwLock::new(Arc::new(SoundSet::resolve(&[]))))
}

/// Install the alert sounds for this process from `(key, value)` pairs, where
/// each value is a path to a WAV file or `off` to mute that alert. Keys are the
/// [`Alert::key`] names. Alerts not named here keep their built-in default.
///
/// Every configured file is preflighted here — call it from session setup, not
/// from the RE thread.
pub fn load_sounds(overrides: &[(String, String)]) {
    let set = Arc::new(SoundSet::resolve(overrides));
    *sound_slot().write().unwrap_or_else(|e| e.into_inner()) = set;
}

/// Preflight a configured sound file. `None` (with a warning) if baresip will
/// not be able to play it, so the caller can keep the built-in default.
fn load_file(alert: Alert, path: &str) -> Option<Source> {
    let key = alert.key();
    // Make it absolute: `play_file` resolves a relative path against baresip's
    // `audio_path`, which is not what someone writing a path in ringo.toml
    // means. canonicalize also settles "does it exist" in the same call.
    let full = match std::fs::canonicalize(path) {
        Ok(p) => p,
        Err(e) => {
            crate::rlog!(Warn, "sounds: {key}: cannot open '{path}': {e}");
            return None;
        }
    };
    let shown = full.display().to_string();
    // play_file runs parse_play_settings (vendor/baresip/src/play.c) over the
    // name first, which splits on commas and writes the truncated head back —
    // `ring,long.wav` reaches the loader as `ring`. Catching it here keeps the
    // preflight honest instead of passing a file that then fails at play time.
    if shown.contains(',') {
        crate::rlog!(
            Warn,
            "sounds: {key}: '{shown}' contains a comma, which baresip's player \
             reads as a repeat/delay suffix — rename the file or the directory"
        );
        return None;
    }
    let head = match read_head(&full) {
        Ok(h) => h,
        Err(e) => {
            crate::rlog!(Warn, "sounds: {key}: cannot read '{shown}': {e}");
            return None;
        }
    };
    let info = match probe_wav(&head) {
        Ok(i) => i,
        Err(why) => {
            crate::rlog!(Warn, "sounds: {key}: '{shown}' {why}");
            return None;
        }
    };
    // A NUL byte cannot survive the trip through the C API. Vanishingly rare on
    // a real path, but it is the one way CString::new can fail.
    let c_path = {
        use std::os::unix::ffi::OsStrExt;
        match CString::new(full.as_os_str().as_bytes()) {
            Ok(c) => c,
            Err(_) => {
                crate::rlog!(Warn, "sounds: {key}: '{shown}' contains a NUL byte");
                return None;
            }
        }
    };
    let (srate, channels) = (info.srate, info.channels);
    crate::rlog!(
        Info,
        "sounds: {key}: using '{shown}' ({srate} Hz, {channels} ch)"
    );
    Some(Source::File(c_path))
}

/// The first [`PROBE_BYTES`] of `path` (or the whole file, if shorter).
fn read_head(path: &Path) -> std::io::Result<Vec<u8>> {
    use std::io::Read;
    let mut f = std::fs::File::open(path)?;
    let mut buf = vec![0u8; PROBE_BYTES];
    let mut filled = 0;
    while filled < buf.len() {
        match f.read(&mut buf[filled..])? {
            0 => break,
            n => filled += n,
        }
    }
    buf.truncate(filled);
    Ok(buf)
}

/// What baresip will make of a file, read from its header.
#[derive(Debug)]
struct WavInfo {
    srate: u32,
    channels: u16,
}

/// Mirror of the header check in libre's `aufile_open` (`rem/aufile/wave.c`
/// and `aufile.c`): RIFF/WAVE, `fmt ` as the FIRST chunk, and a sample format
/// of 16-bit PCM, A-law or mu-law. Failing it there means a bare error code at
/// play time; failing it here means a sentence in the log while ringo starts.
///
/// Only the header is judged. Whether the samples are intact is baresip's
/// problem, and a `play_file` that fails anyway falls back to the built-in tone.
fn probe_wav(head: &[u8]) -> Result<WavInfo, String> {
    if head.len() < 36 {
        return Err("is too short to be a WAV".into());
    }
    if &head[0..4] != b"RIFF" || &head[8..12] != b"WAVE" {
        return Err("is not a RIFF/WAVE file".into());
    }
    // libre reads exactly one chunk after "WAVE" and insists it is `fmt `.
    if &head[12..16] != b"fmt " {
        return Err("must have its 'fmt ' chunk first — baresip's loader reads no further".into());
    }
    let fmt_size = u32::from_le_bytes([head[16], head[17], head[18], head[19]]) as usize;
    if fmt_size < 16 || 20 + fmt_size > head.len() {
        return Err("has a truncated 'fmt ' chunk".into());
    }
    let format = u16::from_le_bytes([head[20], head[21]]);
    let channels = u16::from_le_bytes([head[22], head[23]]);
    let srate = u32::from_le_bytes([head[24], head[25], head[26], head[27]]);
    let bits = u16::from_le_bytes([head[34], head[35]]);
    match (format, bits) {
        (WAVE_FMT_PCM, 16) | (WAVE_FMT_ALAW, 8) | (WAVE_FMT_ULAW, 8) => {}
        (WAVE_FMT_EXTENSIBLE, _) => {
            return Err(
                "is WAVE_FORMAT_EXTENSIBLE, which baresip's loader rejects — \
                 re-encode it as plain 16-bit PCM"
                    .into(),
            );
        }
        _ => {
            return Err(format!(
                "has an unsupported format (wFormatTag={format}, {bits} bit) — \
                 needs 16-bit PCM, A-law or mu-law"
            ));
        }
    }
    if channels == 0 || srate == 0 {
        return Err("declares no channels or no sample rate".into());
    }
    Ok(WavInfo { srate, channels })
}

/// `wFormatTag` values, as libre names them (`rem/aufile/aufile.h`).
const WAVE_FMT_PCM: u16 = 0x0001;
const WAVE_FMT_ALAW: u16 = 0x0006;
const WAVE_FMT_ULAW: u16 = 0x0007;
/// Not in libre's list — worth its own message because converters emit it often.
const WAVE_FMT_EXTENSIBLE: u16 = 0xFFFE;

// ─── WAV parsing ─────────────────────────────────────────────────────────────

/// Minimal WAV (RIFF) parser — extracts the `data` chunk and format info.
/// Supports 16-bit PCM (format 1, and 0xFFFE/extensible, which is 16-bit PCM in
/// everything that writes it) and G.711 mu-law (format 7). mu-law is decoded to
/// S16LE via the embedded lookup table (same as libre's `g711_ulaw2pcm`).
///
/// Anything else returns `None` rather than passing raw bytes through: at
/// 24-bit or float32 the "samples" would reach the speaker as loud noise.
pub(super) fn parse_wav(data: &[u8]) -> Option<Pcm> {
    if data.len() < 44 {
        return None;
    }
    if &data[0..4] != b"RIFF" || &data[8..12] != b"WAVE" {
        return None;
    }
    let mut pos = 12;
    let mut audio_format: u16 = 0;
    let mut bits: u16 = 0;
    let mut srate: u32 = 8000;
    let mut channels: u16 = 1;
    let mut pcm: Option<Vec<u8>> = None;
    while pos + 8 <= data.len() {
        let chunk_id = &data[pos..pos + 4];
        let chunk_size =
            u32::from_le_bytes([data[pos + 4], data[pos + 5], data[pos + 6], data[pos + 7]])
                as usize;
        let chunk_start = pos + 8;
        let chunk_end = chunk_start + chunk_size;
        if chunk_end > data.len() {
            break;
        }
        match chunk_id {
            b"fmt " if chunk_size >= 16 => {
                audio_format = u16::from_le_bytes([data[chunk_start], data[chunk_start + 1]]);
                channels = u16::from_le_bytes([data[chunk_start + 2], data[chunk_start + 3]]);
                srate = u32::from_le_bytes([
                    data[chunk_start + 4],
                    data[chunk_start + 5],
                    data[chunk_start + 6],
                    data[chunk_start + 7],
                ]);
                bits = u16::from_le_bytes([data[chunk_start + 14], data[chunk_start + 15]]);
            }
            b"data" => {
                let raw = &data[chunk_start..chunk_end];
                // play_tone expects S16LE PCM. Convert what we can, reject the rest.
                let samples = match (audio_format, bits) {
                    // Already S16LE; drop a trailing odd byte so every frame is whole.
                    (1, 16) | (0xFFFE, 16) => raw[..raw.len() & !1].to_vec(),
                    (7, 8) => mu2pcm(raw),
                    _ => return None,
                };
                pcm = Some(samples);
            }
            _ => {}
        }
        pos = chunk_end + (chunk_size % 2);
    }
    Some(Pcm {
        samples: pcm?,
        srate,
        channels: channels as u8,
    })
}

/// G.711 mu-law to S16LE PCM conversion (same table as libre's g711_u2l).
fn mu2pcm(data: &[u8]) -> Vec<u8> {
    static U2L: [i16; 256] = [
        -32124, -31100, -30076, -29052, -28028, -27004, -25980, -24956, -23932, -22908, -21884,
        -20860, -19836, -18812, -17788, -16764, -15996, -15484, -14972, -14460, -13948, -13436,
        -12924, -12412, -11900, -11388, -10876, -10364, -9852, -9340, -8828, -8316, -7932, -7676,
        -7420, -7164, -6908, -6652, -6396, -6140, -5884, -5628, -5372, -5116, -4860, -4604, -4348,
        -4092, -3900, -3772, -3644, -3516, -3388, -3260, -3132, -3004, -2876, -2748, -2620, -2492,
        -2364, -2236, -2108, -1980, -1884, -1820, -1756, -1692, -1628, -1564, -1500, -1436, -1372,
        -1308, -1244, -1180, -1116, -1052, -988, -924, -876, -844, -812, -780, -748, -716, -684,
        -652, -620, -588, -556, -524, -492, -460, -428, -396, -372, -356, -340, -324, -308, -292,
        -276, -260, -244, -228, -212, -196, -180, -164, -148, -132, -120, -112, -104, -96, -88,
        -80, -72, -64, -56, -48, -40, -32, -24, -16, -8, -2, 32124, 31100, 30076, 29052, 28028,
        27004, 25980, 24956, 23932, 22908, 21884, 20860, 19836, 18812, 17788, 16764, 15996, 15484,
        14972, 14460, 13948, 13436, 12924, 12412, 11900, 11388, 10876, 10364, 9852, 9340, 8828,
        8316, 7932, 7676, 7420, 7164, 6908, 6652, 6396, 6140, 5884, 5628, 5372, 5116, 4860, 4604,
        4348, 4092, 3900, 3772, 3644, 3516, 3388, 3260, 3132, 3004, 2876, 2748, 2620, 2492, 2364,
        2236, 2108, 1980, 1884, 1820, 1756, 1692, 1628, 1564, 1500, 1436, 1372, 1308, 1244, 1180,
        1116, 1052, 988, 924, 876, 844, 812, 780, 748, 716, 684, 652, 620, 588, 556, 524, 492, 460,
        428, 396, 372, 356, 340, 324, 308, 292, 276, 260, 244, 228, 212, 196, 180, 164, 148, 132,
        120, 112, 104, 96, 88, 80, 72, 64, 56, 48, 40, 32, 24, 16, 8, 2,
    ];
    let mut out = Vec::with_capacity(data.len() * 2);
    for &b in data {
        let s = U2L[b as usize];
        out.push((s & 0xff) as u8);
        out.push((s >> 8) as u8);
    }
    out
}

// ─── Playback ────────────────────────────────────────────────────────────────

/// Stable heap cell holding the current alert tone's `*mut play`.
///
/// `play_tone`/`play_file` record this address (`play->playp`) and the
/// destructor writes
/// `*playp = NULL` when the tone is freed — which happens LATER, inside
/// `stop_alert`'s `mem_deref` (or on its own, once a one-shot tone reaches its
/// end). So `playp` must NOT be a stack slot: by then the caller's frame is
/// gone and the write corrupts memory (→ SIGSEGV). We leak one heap cell for
/// the process lifetime and reuse it as the single alert slot.
///
/// Process-wide single slot — one audio output device, one alert tone at a
/// time, replaced on each new alert. Only ever touched on the RE thread
/// (play_alert/stop_alert both run from the bevent handler), so no lock needed.
fn alert_playp() -> *mut *mut Play {
    static CELL: OnceLock<usize> = OnceLock::new();
    *CELL.get_or_init(|| Box::into_raw(Box::new(std::ptr::null_mut::<Play>())) as usize)
        as *mut *mut Play
}

/// Which alert occupies the slot, as `Alert as u8 + 1`; 0 means none.
///
/// Only the RE thread touches it, but an atomic costs nothing here and spares
/// the file another `unsafe`. Kept in step with [`alert_playp`]: set when a
/// tone starts, cleared by [`stop_alert`]. A one-shot frees itself when it ends
/// without clearing this, which is harmless — the value is only ever consulted
/// to ask whether a *looping* alert is in progress, and those end only by being
/// stopped or replaced.
static PLAYING: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

/// The alert currently sounding, if the slot is occupied.
///
/// # Safety
/// Must run on the RE thread, which owns the play slot.
unsafe fn playing_alert() -> Option<Alert> {
    if unsafe { (*alert_playp()).is_null() } {
        return None;
    }
    let tag = PLAYING.load(std::sync::atomic::Ordering::Relaxed);
    (tag > 0).then(|| Alert::ALL[tag as usize - 1])
}

/// Play `alert` on the alert device, replacing any currently playing tone.
/// A muted alert is a no-op (it still stops whatever was playing).
///
/// A configured file goes to `play_file`, which opens and decodes it here on
/// the RE thread — the same thing baresip's own menu module does. If that
/// fails, the built-in tone stands in, so a rate the audio driver refuses ends
/// as a lesser tone rather than as silence.
pub fn play_alert(alert: Alert) {
    // SAFETY: on the RE thread, see the note below.
    if let Some(current) = unsafe { playing_alert() } {
        // A one-shot must never cut off a tone that is still saying something.
        // Voicemail arriving mid-ring used to tear the ring down and leave the
        // call sounding on screen only, because nothing restarts a loop.
        if alert.repeat() > 0 && current.repeat() < 0 {
            let (new, held) = (alert.key(), current.key());
            crate::rlog!(Debug, "alert: {new} suppressed, {held} is still playing");
            return;
        }
    }
    stop_alert();
    let set = sound_slot()
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .clone();
    let key = alert.key();
    let Some(tone) = set.get(alert) else {
        // Worth a line: a muted alert and a broken one look identical from the
        // outside, and the log is the only place that tells them apart.
        crate::rlog!(Debug, "alert: {key} is muted, playing nothing");
        return;
    };
    // SAFETY: play_alert is only called from the bevent handler, which baresip
    // invokes synchronously on the RE thread (bevent_emit in src/bevent.c is
    // not marshalled). So these FFI calls are already on the correct thread —
    // no re_thread_enter wrapper needed (matching stop_alert).
    unsafe {
        let player = baresip_player();
        if player.is_null() {
            crate::rlog!(Warn, "play_alert: baresip_player() returned null");
            return;
        }
        let (play_mod, play_dev) = get_alert_device();
        PLAYING.store(alert as u8 + 1, std::sync::atomic::Ordering::Relaxed);
        match tone {
            Source::Builtin(pcm) => play_pcm(alert, pcm, player, play_mod, play_dev),
            Source::File(path) => {
                let shown = path.to_string_lossy();
                let mode = alert.mode();
                crate::rlog!(Debug, "alert: playing {key} from '{shown}' ({mode})");
                let rc = play_file(
                    alert_playp(),
                    player,
                    path.as_ptr(),
                    alert.repeat(),
                    play_mod,
                    play_dev,
                );
                if rc != 0 {
                    crate::rlog!(
                        Warn,
                        "play_alert: play_file('{shown}') failed (rc={rc}), \
                         falling back to the built-in {key}"
                    );
                    // play_file only writes *playp on success, so the slot is
                    // still free for the fallback.
                    if let Some(pcm) = alert.default_pcm() {
                        play_pcm(alert, &pcm, player, play_mod, play_dev);
                    }
                }
            }
        }
    }
}

/// Hand PCM samples to `play_tone` on the alert slot.
///
/// # Safety
/// Must run on the RE thread with a non-null `player`, and the alert slot must
/// be free (i.e. after [`stop_alert`]).
unsafe fn play_pcm(
    alert: Alert,
    pcm: &Pcm,
    player: *mut player,
    play_mod: *const c_char,
    play_dev: *const c_char,
) {
    let key = alert.key();
    let mode = alert.mode();
    let (srate, ch) = (pcm.srate, pcm.channels);
    crate::rlog!(Debug, "alert: playing {key} ({srate} Hz, {ch} ch, {mode})");
    unsafe {
        let mb = mbuf_alloc(pcm.samples.len());
        if mb.is_null() {
            crate::rlog!(Warn, "play_alert: mbuf_alloc() failed");
            return;
        }
        let rc = mbuf_write_mem(mb, pcm.samples.as_ptr(), pcm.samples.len());
        if rc != 0 {
            crate::rlog!(Warn, "play_alert: mbuf_write_mem() failed (rc={rc})");
            mem_deref(mb as *mut std::os::raw::c_void);
            return;
        }
        // Rewind: mbuf_write_mem leaves pos at the end, and play's write_handler
        // reads from pos (`mbuf_get_left`). baresip's own loader does the same
        // (src/play.c, aufile_load). Without this the first pass reads zero
        // bytes, which a looping tone survives (it rewinds itself one frame
        // later) but a one-shot does not: check_restart takes repeat 1 -> 0,
        // sets eof, and the tone is freed having played nothing at all.
        (*mb).pos = 0;
        // playp points at the stable heap cell (not a stack slot) so baresip's
        // destructor can null it out safely later.
        let rc = play_tone(
            alert_playp(),
            player,
            mb,
            srate,
            ch,
            alert.repeat(),
            play_mod,
            play_dev,
        );
        mem_deref(mb as *mut std::os::raw::c_void);
        if rc != 0 {
            crate::rlog!(Warn, "play_alert: play_tone('{key}') failed (rc={rc})");
        }
    }
}

/// Stop the currently playing alert tone (if any).
///
/// Reached from two directions: the bevent handler, which is already on the RE
/// thread, and `Phone::silence_alert`, which marshals onto it. Both are the RE
/// thread by the time they get here — anything else would race the destructor.
pub fn stop_alert() {
    PLAYING.store(0, std::sync::atomic::Ordering::Relaxed);
    // SAFETY: on the RE thread. mem_deref runs the play destructor, which sets
    // *playp = NULL via the stable cell — so a second stop_alert is a no-op.
    unsafe {
        let playp = alert_playp();
        if !(*playp).is_null() {
            mem_deref(*playp as *mut std::os::raw::c_void);
        }
    }
}

/// Get alert_mod and alert_dev from config (audio_alert = "driver,device").
/// Cached for the process lifetime: `audio_alert` doesn't change at runtime,
/// and caching avoids leaking two `CString`s on every alert.
fn get_alert_device() -> (*const std::os::raw::c_char, *const std::os::raw::c_char) {
    use std::ffi::CStr;
    use std::os::raw::c_char;
    static CACHED: OnceLock<Option<(CString, CString)>> = OnceLock::new();
    let cached = CACHED.get_or_init(|| {
        let conf = unsafe { conf_cur() };
        if conf.is_null() {
            return None;
        }
        // `c_char` is i8 on x86_64 but u8 on aarch64 — type the buffer as c_char
        // so `conf_get_str`/`CStr::from_ptr` match the FFI signature on both.
        let mut buf = [0 as c_char; 256];
        let rc =
            unsafe { conf_get_str(conf, c"audio_alert".as_ptr(), buf.as_mut_ptr(), buf.len()) };
        if rc != 0 {
            return None;
        }
        let s = unsafe { CStr::from_ptr(buf.as_ptr()) }
            .to_str()
            .unwrap_or("");
        let mut parts = s.splitn(2, ',');
        let m = parts.next().unwrap_or("aubridge");
        let d = parts.next().unwrap_or("default");
        Some((
            CString::new(m).unwrap_or_default(),
            CString::new(d).unwrap_or_default(),
        ))
    });
    match cached {
        Some((m, d)) => (m.as_ptr(), d.as_ptr()),
        None => (std::ptr::null(), std::ptr::null()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a minimal WAV in memory.
    fn wav(audio_format: u16, bits: u16, channels: u16, srate: u32, data: &[u8]) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(b"RIFF");
        v.extend_from_slice(&(36u32 + data.len() as u32).to_le_bytes());
        v.extend_from_slice(b"WAVEfmt ");
        v.extend_from_slice(&16u32.to_le_bytes());
        v.extend_from_slice(&audio_format.to_le_bytes());
        v.extend_from_slice(&channels.to_le_bytes());
        v.extend_from_slice(&srate.to_le_bytes());
        v.extend_from_slice(&(srate * channels as u32 * bits as u32 / 8).to_le_bytes());
        v.extend_from_slice(&(channels * bits / 8).to_le_bytes());
        v.extend_from_slice(&bits.to_le_bytes());
        v.extend_from_slice(b"data");
        v.extend_from_slice(&(data.len() as u32).to_le_bytes());
        v.extend_from_slice(data);
        v
    }

    /// One second of silence, as S16LE bytes.
    fn silence(srate: u32, channels: u16) -> Vec<u8> {
        vec![0u8; srate as usize * channels as usize * 2]
    }

    #[test]
    fn alert_all_matches_the_discriminant_order() {
        // SoundSet indexes by `Alert as usize`, so ALL must stay in that order.
        for (i, a) in Alert::ALL.iter().enumerate() {
            assert_eq!(i, *a as usize, "{} is out of order in ALL", a.key());
        }
    }

    #[test]
    fn every_alert_has_a_built_in_default() {
        for a in Alert::ALL {
            let pcm = a
                .default_pcm()
                .unwrap_or_else(|| panic!("{} has no built-in tone", a.key()));
            assert!(!pcm.samples.is_empty(), "{} is empty", a.key());
            assert_eq!(pcm.samples.len() % 2, 0, "{} has a half sample", a.key());
        }
    }

    #[test]
    fn every_default_is_synthesized_at_48k() {
        // Nothing is an embedded 8 kHz mu-law file any more; if one creeps back
        // in, this is where it shows up.
        for a in Alert::ALL {
            assert_eq!(a.default_pcm().unwrap().srate, SYNTH_RATE, "{}", a.key());
        }
    }

    #[test]
    fn the_ring_chime_ends_in_silence_so_it_loops_cleanly() {
        // The ring repeats forever; if the motif ran to the end of the buffer,
        // every repetition would start with an audible seam.
        let pcm = Alert::Ring.default_pcm().unwrap();
        let tail = &pcm.samples[pcm.samples.len() - 4800 * 2..];
        assert!(
            tail.iter().all(|&b| b == 0),
            "the ring must fade into silence"
        );
    }

    #[test]
    fn the_one_shot_tones_are_long_enough_to_recognise() {
        // A single 480 ms burst of busy tone reads as a glitch. Both one-shots
        // must carry enough cadence periods to be heard as a state.
        for a in [Alert::Busy, Alert::Error] {
            let pcm = a.default_pcm().unwrap();
            let secs = pcm.samples.len() as f64 / 2.0 / pcm.srate as f64;
            assert!(secs > 1.5, "{} lasts only {secs:.2}s", a.key());
        }
    }

    #[test]
    fn ring_and_ringback_loop_the_rest_play_once() {
        assert_eq!(Alert::Ring.repeat(), -1);
        assert_eq!(Alert::Ringback.repeat(), -1);
        assert_eq!(Alert::Busy.repeat(), 1);
        assert_eq!(Alert::Error.repeat(), 1);
        assert_eq!(Alert::Message.repeat(), 1);
    }

    #[test]
    fn keys_round_trip() {
        for a in Alert::ALL {
            assert_eq!(Alert::from_key(a.key()), Some(a));
        }
        assert_eq!(Alert::from_key("nope"), None);
    }

    #[test]
    fn parses_16_bit_pcm() {
        let pcm = parse_wav(&wav(1, 16, 1, 16000, &silence(16000, 1))).unwrap();
        assert_eq!(pcm.srate, 16000);
        assert_eq!(pcm.channels, 1);
        assert_eq!(pcm.samples.len(), 32000);
    }

    #[test]
    fn parses_extensible_as_16_bit_pcm() {
        // Anything that writes WAVE_FORMAT_EXTENSIBLE at 16 bits means PCM.
        let pcm = parse_wav(&wav(0xFFFE, 16, 2, 48000, &silence(48000, 2))).unwrap();
        assert_eq!(pcm.channels, 2);
    }

    #[test]
    fn decodes_mu_law_to_s16() {
        let pcm = parse_wav(&wav(7, 8, 1, 8000, &[0xFF; 160])).unwrap();
        // One byte in, one 16-bit sample out.
        assert_eq!(pcm.samples.len(), 320);
    }

    #[test]
    fn rejects_float_and_24_bit() {
        // Passing these through would reach the speaker as loud noise.
        assert!(parse_wav(&wav(3, 32, 1, 48000, &[0; 4096])).is_none());
        assert!(parse_wav(&wav(1, 24, 1, 48000, &[0; 4096])).is_none());
    }

    #[test]
    fn rejects_non_riff_data() {
        assert!(parse_wav(&[0u8; 128]).is_none());
        assert!(parse_wav(b"RIFF").is_none());
    }

    #[test]
    fn off_and_none_and_empty_mute() {
        for v in ["off", "OFF", "none", "None", "", "  "] {
            assert!(is_muted(v.trim()), "'{v}' should mute");
        }
        assert!(!is_muted("~/sounds/ring.wav"));
    }

    #[test]
    fn resolve_defaults_to_the_built_in_tones() {
        let set = SoundSet::resolve(&[]);
        for a in Alert::ALL {
            assert!(set.get(a).is_some(), "{} has no default", a.key());
        }
    }

    #[test]
    fn resolve_mutes_and_ignores_unknown_keys() {
        let set = SoundSet::resolve(&[
            ("ring".into(), "off".into()),
            ("nonsense".into(), "off".into()),
        ]);
        assert!(set.get(Alert::Ring).is_none());
        assert!(set.get(Alert::Ringback).is_some());
    }

    #[test]
    fn a_bad_path_keeps_the_built_in_default() {
        // A typo in the config must never leave an incoming call silent.
        let set = SoundSet::resolve(&[("ring".into(), "/nope/does-not-exist.wav".into())]);
        assert!(set.get(Alert::Ring).is_some());
    }

    #[test]
    fn a_custom_file_is_handed_to_baresip_by_path() {
        let dir = std::env::temp_dir().join(format!("ringo-sounds-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("custom.wav");
        std::fs::write(&path, wav(1, 16, 1, 44100, &silence(1000, 1))).unwrap();

        let set = SoundSet::resolve(&[("ring".into(), path.display().to_string())]);
        match set.get(Alert::Ring) {
            Some(Source::File(p)) => assert_eq!(
                p.to_str().unwrap(),
                std::fs::canonicalize(&path).unwrap().to_str().unwrap(),
                "the path must reach play_file absolute"
            ),
            _ => panic!("the custom file should be in use"),
        }
        // The samples stay on disk — nothing but the path is held.
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_configured_tone_is_synthesized_not_read_from_disk() {
        // The North American ringback, pasted straight from Asterisk's [us] zone.
        let set = SoundSet::resolve(&[("ringback".into(), "tone:440+480/2000,0/4000".into())]);
        match set.get(Alert::Ringback) {
            Some(Source::Builtin(pcm)) => {
                assert_eq!(pcm.srate, SYNTH_RATE);
                let secs = pcm.samples.len() as f64 / 2.0 / pcm.srate as f64;
                assert!((secs - 6.0).abs() < 0.01, "one period is 6 s, got {secs}");
            }
            _ => panic!("a tone: value must be synthesized"),
        }
    }

    #[test]
    fn a_broken_tone_spec_keeps_the_built_in_default() {
        let set = SoundSet::resolve(&[("busy".into(), "tone:nonsense".into())]);
        match set.get(Alert::Busy) {
            Some(Source::Builtin(pcm)) => assert_eq!(
                pcm.samples,
                Alert::Busy.default_pcm().unwrap().samples,
                "a typo must leave the default in place"
            ),
            _ => panic!("busy should still have its default"),
        }
    }

    #[test]
    fn a_tone_value_is_never_treated_as_a_path() {
        // "tone:425/480,0/480" contains slashes; it must not reach the file
        // loader and fail as a missing path.
        let set = SoundSet::resolve(&[("error".into(), "tone:425/240,0/240".into())]);
        assert!(matches!(set.get(Alert::Error), Some(Source::Builtin(_))));
    }

    #[test]
    fn a_comma_in_the_path_is_rejected_up_front() {
        // baresip's player reads a comma as a repeat/delay suffix and truncates
        // the name, so such a file would pass the header check and then fail to
        // open. Better to say so while ringo starts.
        let dir = std::env::temp_dir().join(format!("ringo-comma-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("ring,long.wav");
        std::fs::write(&path, wav(1, 16, 1, 48000, &silence(100, 1))).unwrap();

        let set = SoundSet::resolve(&[("ring".into(), path.display().to_string())]);
        assert!(
            matches!(set.get(Alert::Ring), Some(Source::Builtin(_))),
            "a comma path must fall back to the built-in tone"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_one_shot_never_outranks_a_looping_alert() {
        // Voicemail arriving mid-ring used to tear the ring down for good:
        // stop_alert killed the loop, the chime played once, and nothing
        // restarted the ring.
        for loud in [Alert::Ring, Alert::Ringback] {
            for quiet in [Alert::Busy, Alert::Error, Alert::Message] {
                assert!(
                    loud.repeat() < 0 && quiet.repeat() > 0,
                    "{} must loop and {} must not",
                    loud.key(),
                    quiet.key()
                );
            }
        }
    }

    #[test]
    fn probe_accepts_what_baresip_can_read() {
        for (fmt, bits) in [(1u16, 16u16), (6, 8), (7, 8)] {
            let info = probe_wav(&wav(fmt, bits, 1, 8000, &silence(100, 1)))
                .unwrap_or_else(|e| panic!("format {fmt}/{bits} rejected: {e}"));
            assert_eq!(info.srate, 8000);
            assert_eq!(info.channels, 1);
        }
    }

    #[test]
    fn probe_rejects_what_baresip_cannot() {
        // 8-bit PCM, float32 and 24-bit all fail libre's wavfmt_to_aufmt.
        assert!(probe_wav(&wav(1, 8, 1, 8000, &silence(100, 1))).is_err());
        assert!(probe_wav(&wav(3, 32, 1, 48000, &silence(100, 1))).is_err());
        assert!(probe_wav(&wav(1, 24, 1, 48000, &silence(100, 1))).is_err());
        assert!(probe_wav(&[0u8; 128]).is_err());
    }

    #[test]
    fn probe_names_extensible_specifically() {
        // ffmpeg and friends emit it readily, and our own parser used to accept
        // it — so the message has to say what to do, not just "unsupported".
        let err = probe_wav(&wav(0xFFFE, 16, 2, 48000, &silence(100, 2))).unwrap_err();
        assert!(err.contains("EXTENSIBLE"), "unhelpful message: {err}");
        assert!(err.contains("16-bit PCM"), "unhelpful message: {err}");
    }

    #[test]
    fn probe_requires_fmt_to_come_first() {
        // libre reads exactly one chunk after "WAVE" and gives up if it is not
        // `fmt `, so a file with a leading LIST chunk is unplayable for it.
        let mut v = Vec::new();
        v.extend_from_slice(b"RIFF");
        v.extend_from_slice(&64u32.to_le_bytes());
        v.extend_from_slice(b"WAVELIST");
        v.extend_from_slice(&4u32.to_le_bytes());
        v.extend_from_slice(b"INFO");
        v.extend_from_slice(&[0u8; 32]);
        let err = probe_wav(&v).unwrap_err();
        assert!(err.contains("fmt"), "unhelpful message: {err}");
    }
}
