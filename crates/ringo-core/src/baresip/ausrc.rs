//! Custom `ringo` baresip audio-source module.
//!
//! Why this exists: baresip's public `audio_set_source()` is *transient* — a
//! media re-negotiation (re-INVITE: hold/resume, transfer, line switch) rebuilds
//! the audio stream and resets the source to the account default. And the only
//! persistent knob (`account.ausrc_mod` / `config.audio.src_mod`) is either a
//! private struct field with no public setter, or process-global (all UAs in
//! this single process would collide — the old backend got away with a global
//! because each UA was its own process).
//!
//! So instead of fighting baresip's source management, ringo registers its OWN
//! source module via `ausrc_register()` (baresip's official extension point) and
//! points each UA's account at `audio_source=ringo,<key>`. baresip then re-allocs
//! *our* source on every stream rebuild, and we render whatever the per-key
//! [`REGISTRY`] currently says — race-free, persistent across re-INVITEs, and
//! per-UA isolated. Changing a UA's audio (tone/file/silence) is just a registry
//! update; the running render thread picks it up on the next frame.

use std::collections::{HashMap, VecDeque};
use std::os::raw::{c_char, c_void};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use super::bindings::*;

const AMPLITUDE: f64 = 0.25;

/// What a UA's source should emit. Cheap to clone (file PCM is shared via `Arc`).
#[derive(Clone)]
enum GenSpec {
    Silence,
    /// Sine wave at the given frequency in Hz.
    Tone(u32),
    /// Mono S16 samples at the given sample rate, looped.
    File(Arc<Vec<i16>>, u32),
}

/// A registry entry: the desired spec plus a version that bumps on every change,
/// so a render thread can cheaply detect "my spec changed, reset my phase".
struct Entry {
    version: u64,
    spec: GenSpec,
}

/// Per-UA desired source, keyed by the device string (the `<key>` in
/// `audio_source=ringo,<key>`, which ringo sets to the account username).
static REGISTRY: OnceLock<Mutex<HashMap<String, Entry>>> = OnceLock::new();

fn registry() -> &'static Mutex<HashMap<String, Entry>> {
    REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Registered `struct ausrc *` (kept alive for the process lifetime).
static AUSRC: OnceLock<usize> = OnceLock::new();

/// Translate a baresip-style source spec into a [`GenSpec`] and store it for
/// `key`, bumping the version so the render thread reloads it. Accepts the same
/// specs the engine already produces: `ausine,<freq>`, `aufile,<path>`, and
/// anything else (e.g. `aubridge,...`) → silence.
pub(super) fn set_generator(key: &str, spec: &str) {
    let parsed = parse_spec(spec);
    let mut map = registry().lock().unwrap_or_else(|e| e.into_inner());
    let version = map.get(key).map(|e| e.version + 1).unwrap_or(0);
    map.insert(
        key.to_string(),
        Entry {
            version,
            spec: parsed,
        },
    );
}

/// Pre-seed a UA's source with silence (called at session setup) so the source
/// has a defined default before the first `set_generator`.
pub(super) fn init_generator(key: &str) {
    let mut map = registry().lock().unwrap_or_else(|e| e.into_inner());
    map.entry(key.to_string()).or_insert(Entry {
        version: 0,
        spec: GenSpec::Silence,
    });
}

/// Drop a UA's audio state (source generator + received-audio buffer), called
/// when the session is dropped.
pub(super) fn remove_generator(key: &str) {
    registry()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .remove(key);
    rx_buffers()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .remove(key);
    tx_buffers()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .remove(key);
}

fn spec_name(spec: &GenSpec) -> String {
    match spec {
        GenSpec::Silence => "silence".into(),
        GenSpec::Tone(f) => format!("tone({f}Hz)"),
        GenSpec::File(s, sr) => format!("file({} samples @{sr}Hz)", s.len()),
    }
}

fn parse_spec(spec: &str) -> GenSpec {
    let (driver, device) = spec.split_once(',').unwrap_or((spec, ""));
    match driver {
        "ausine" => match device.split(',').next().unwrap_or("").parse::<u32>() {
            Ok(f) if (10..=20000).contains(&f) => GenSpec::Tone(f),
            _ => {
                crate::rlog!(
                    Warn,
                    "ringo ausrc: invalid tone freq in '{spec}', using silence"
                );
                GenSpec::Silence
            }
        },
        "aufile" => match load_wav_mono(device) {
            Some((samples, srate)) => GenSpec::File(Arc::new(samples), srate),
            None => {
                crate::rlog!(
                    Warn,
                    "ringo ausrc: failed to load '{device}', using silence"
                );
                GenSpec::Silence
            }
        },
        _ => GenSpec::Silence,
    }
}

/// Load a WAV file as mono S16 samples + its sample rate. Stereo is downmixed.
fn load_wav_mono(path: &str) -> Option<(Vec<i16>, u32)> {
    let data = std::fs::read(path).ok()?;
    let pcm = super::sounds::parse_wav(&data)?;
    // `samples` is S16LE bytes. Convert to i16, downmixing stereo to mono.
    let i16s: Vec<i16> = pcm
        .samples
        .chunks_exact(2)
        .map(|b| i16::from_le_bytes([b[0], b[1]]))
        .collect();
    let mono = if pcm.channels >= 2 {
        i16s.chunks_exact(pcm.channels as usize)
            .map(|frame| {
                let sum: i32 = frame.iter().take(2).map(|&s| s as i32).sum();
                (sum / 2) as i16
            })
            .collect()
    } else {
        i16s
    };
    if mono.is_empty() {
        return None;
    }
    Some((mono, pcm.srate))
}

// ─── baresip ausrc module ──────────────────────────────────────────────────

/// Rust-owned state for one active source, referenced from the mem_zalloc'd
/// `ausrc_st` cell. Its `Drop` stops and joins the render thread.
struct SrcState {
    key: String,
    run: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Drop for SrcState {
    fn drop(&mut self) {
        crate::rlog!(Info, "ringo ausrc: FREE key={}", self.key);
        self.run.store(false, Ordering::Release);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

/// Pointers handed to the render thread. `rh`/`arg` come from baresip and are
/// only ever used to call back into baresip from this one thread (the same
/// pattern baresip's own `ausine` uses).
struct ReadCb {
    rh: ausrc_read_h,
    arg: usize,
}
// SAFETY: `arg` is a baresip pointer used solely to call `rh` from the render
// thread; baresip's audio path is built to have the ausrc read handler invoked
// from the source's own thread. No use-after-free: baresip frees the source via
// `stop_tx()` (src/audio.c), which mem_derefs `tx->ausrc` (→ our destructor →
// thread join) BEFORE freeing `tx->aubuf`/`tx->mtx` (the `arg`) — "audio source
// must be stopped first". So `rh` can never run against a freed `arg`.
unsafe impl Send for ReadCb {}

/// Called by baresip to free the `ausrc_st` (on stream teardown / re-INVITE).
unsafe extern "C" fn destructor(arg: *mut c_void) {
    // `arg` points at the mem_zalloc'd cell holding our `*mut SrcState`.
    let cell = arg as *mut *mut SrcState;
    let state = unsafe { *cell };
    if !state.is_null() {
        // Reclaim and drop → SrcState::Drop stops + joins the render thread.
        drop(unsafe { Box::from_raw(state) });
        unsafe { *cell = std::ptr::null_mut() };
    }
}

unsafe extern "C" fn alloc_handler(
    stp: *mut *mut ausrc_st,
    _as: *const ausrc,
    prm: *mut ausrc_prm,
    dev: *const c_char,
    rh: ausrc_read_h,
    _errh: ausrc_error_h,
    arg: *mut c_void,
) -> i32 {
    const EINVAL: i32 = 22;
    const ENOMEM: i32 = 12;
    const ENOTSUP: i32 = 95;

    if stp.is_null() || prm.is_null() || dev.is_null() || rh.is_none() {
        return EINVAL;
    }

    let prm = unsafe { &*prm };
    let key = unsafe { std::ffi::CStr::from_ptr(dev) }
        .to_string_lossy()
        .into_owned();

    let srate = prm.srate;
    let ptime = if prm.ptime == 0 { 20 } else { prm.ptime };
    let fmt = prm.fmt;

    // Reject degenerate params rather than masking them (e.g. `ch.max(1)`): a
    // channel/rate that doesn't match what the TX path negotiated would
    // mis-interleave or divide-by-zero.
    if srate == 0 || prm.ch == 0 {
        crate::rlog!(Warn, "ringo ausrc: invalid prm srate={srate} ch={}", prm.ch);
        return EINVAL;
    }
    let ch = prm.ch;

    // The audio TX drops any frame whose fmt != tx->src_fmt, so we must render
    // in exactly the requested format. Only S16LE and FLOAT are implemented
    // (matching ausine); telephony configs use S16LE.
    if fmt != aufmt::AUFMT_S16LE as i32 && fmt != aufmt::AUFMT_FLOAT as i32 {
        crate::rlog!(Warn, "ringo ausrc: unsupported sample format {fmt}");
        return ENOTSUP;
    }

    // Allocate the cell baresip will mem_deref (running our destructor).
    let cell = unsafe { mem_zalloc(std::mem::size_of::<*mut SrcState>(), Some(destructor)) };
    if cell.is_null() {
        return ENOMEM;
    }

    crate::rlog!(
        Info,
        "ringo ausrc: ALLOC key={key} srate={srate} ch={ch} ptime={ptime} fmt={fmt}"
    );

    let run = Arc::new(AtomicBool::new(true));
    let run_thread = run.clone();
    let cb = ReadCb {
        rh,
        arg: arg as usize,
    };

    // Reset the sent-audio buffer so a save sees only this call.
    reset_buffer(tx_buffers(), &key, srate);

    let key_thread = key.clone();
    let thread = match std::thread::Builder::new()
        .name("ringo-ausrc".into())
        .spawn(move || render_loop(key_thread, srate, ch, ptime, fmt, run_thread, cb))
    {
        Ok(t) => t,
        Err(e) => {
            // Don't hand baresip a source that never produces frames; free the
            // cell (destructor no-ops on the still-null state) and fail.
            crate::rlog!(Error, "ringo ausrc: spawn render thread failed: {e}");
            unsafe { mem_deref(cell) };
            return ENOMEM;
        }
    };

    let state = Box::new(SrcState {
        key,
        run,
        thread: Some(thread),
    });
    unsafe {
        *(cell as *mut *mut SrcState) = Box::into_raw(state);
        *stp = cell as *mut ausrc_st;
    }
    0
}

/// The render thread: every `ptime` ms, generate one frame from the current
/// registry spec for `key` and hand it to baresip via `rh`.
fn render_loop(
    key: String,
    srate: u32,
    ch: u8,
    ptime: u32,
    fmt: i32,
    run: Arc<AtomicBool>,
    cb: ReadCb,
) {
    let is_float = fmt == aufmt::AUFMT_FLOAT as i32;
    let sample_size = if is_float { 4 } else { 2 };
    let frames = (srate as usize * ptime as usize / 1000).max(1);
    let sampc = frames * ch as usize;

    let mut sampv = vec![0u8; sampc * sample_size];
    let mut mono = vec![0i16; frames];

    // Render state (reset when the spec version changes).
    let mut cur_version = u64::MAX;
    let mut cur_spec = GenSpec::Silence;
    let mut phase = 0.0f64; // tone phase accumulator
    let mut file_pos = 0.0f64; // fractional read position into the file

    let mut start = Instant::now();
    let mut frame_idx: u64 = 0;

    while run.load(Ordering::Acquire) {
        // Pace to real time: wake at start + frame_idx * ptime.
        let target = start + Duration::from_millis(frame_idx * ptime as u64);
        let now = Instant::now();
        if target > now {
            std::thread::sleep(target - now);
        } else if now - target > Duration::from_millis(ptime as u64 * 4) {
            // Fell far behind (suspend / scheduling stall): rebase the clock so
            // we don't fire a long burst of catch-up frames (which the TX aubuf
            // would just drop as overruns anyway).
            start = now;
            frame_idx = 0;
        }
        if !run.load(Ordering::Acquire) {
            break;
        }

        // Reload spec if it changed.
        let present = {
            let map = registry().lock().unwrap_or_else(|e| e.into_inner());
            map.get(&key).map(|e| (e.version, e.spec.clone()))
        };
        match &present {
            Some((version, spec)) => {
                if *version != cur_version {
                    cur_version = *version;
                    cur_spec = spec.clone();
                    phase = 0.0;
                    file_pos = 0.0;
                    crate::rlog!(
                        Info,
                        "ringo ausrc: key={key} spec={} (v{version})",
                        spec_name(spec)
                    );
                }
            }
            None => {
                // Registry entry gone (shouldn't happen while a source is live):
                // render silence rather than a stale spec.
                if cur_version != u64::MAX {
                    crate::rlog!(Warn, "ringo ausrc: key={key} no registry entry, silence");
                    cur_version = u64::MAX;
                    cur_spec = GenSpec::Silence;
                }
            }
        }

        // Render `frames` mono samples.
        match &cur_spec {
            GenSpec::Silence => mono.iter_mut().for_each(|s| *s = 0),
            GenSpec::Tone(freq) => {
                let step = std::f64::consts::TAU * (*freq as f64) / (srate as f64);
                for s in mono.iter_mut() {
                    *s = (phase.sin() * AMPLITUDE * 32767.0) as i16;
                    phase += step;
                    if phase >= std::f64::consts::TAU {
                        phase -= std::f64::consts::TAU;
                    }
                }
            }
            GenSpec::File(samples, file_srate) => {
                let step = *file_srate as f64 / srate as f64;
                let len = samples.len() as f64;
                for s in mono.iter_mut() {
                    // `while`, not `if`: a single step may overshoot `len` for a
                    // tiny file (len < step), so wrap fully to keep looping.
                    while file_pos >= len {
                        file_pos -= len;
                    }
                    let i = file_pos as usize;
                    *s = samples.get(i).copied().unwrap_or(0);
                    file_pos += step;
                }
            }
        }

        // Capture the sent audio for --save-audio (full capture only).
        if FULL_CAPTURE.load(Ordering::Acquire) {
            capture_mono(tx_buffers(), &key, &mono);
        }

        // Pack mono → interleaved sampv in the requested format.
        if is_float {
            let out =
                unsafe { std::slice::from_raw_parts_mut(sampv.as_mut_ptr() as *mut f32, sampc) };
            for (f, &m) in out.chunks_mut(ch as usize).zip(mono.iter()) {
                let v = m as f32 / 32768.0;
                f.iter_mut().for_each(|x| *x = v);
            }
        } else {
            let out =
                unsafe { std::slice::from_raw_parts_mut(sampv.as_mut_ptr() as *mut i16, sampc) };
            for (f, &m) in out.chunks_mut(ch as usize).zip(mono.iter()) {
                f.iter_mut().for_each(|x| *x = m);
            }
        }

        // Build the auframe and push it to baresip.
        let mut af: auframe = unsafe { std::mem::zeroed() };
        unsafe {
            auframe_init(
                &mut af,
                if is_float {
                    aufmt::AUFMT_FLOAT
                } else {
                    aufmt::AUFMT_S16LE
                },
                sampv.as_mut_ptr() as *mut c_void,
                sampc,
                srate,
                ch,
            );
        }
        af.timestamp = frame_idx * ptime as u64 * 1000; // AUDIO_TIMEBASE = microseconds

        if let Some(rh) = cb.rh {
            unsafe { rh(&mut af, cb.arg as *mut c_void) };
        }

        frame_idx += 1;
    }
}

// ─── ringo auplay (self-clocked RX sink) ───────────────────────────────────
//
// Why we also need a player: the headless RX path (decode → aubuf → player) is
// *pull-driven* — something must call the player's write handler at real-time
// pace, or the decoder never advances and the received audio is silent.
// aubridge's player is clocked by its device thread, which only runs when BOTH
// an aubridge source AND player share a device (modules/aubridge/device.c) — but
// ringo replaced the source, so that thread would never start. So ringo provides
// its own self-clocked player: a timer thread that pulls `wh()` every ptime,
// which both clocks the decode pipeline and yields the received frames we
// capture for verify-audio / --save-audio.

/// Registered `struct auplay *` (kept alive for the process lifetime).
static AUPLAY: OnceLock<usize> = OnceLock::new();

struct PlayState {
    run: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Drop for PlayState {
    fn drop(&mut self) {
        self.run.store(false, Ordering::Release);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

struct WriteCb {
    wh: auplay_write_h,
    arg: usize,
}
// SAFETY: same contract as ReadCb — `arg`/`wh` are baresip pointers used only to
// call back into baresip from this player's own thread. baresip frees the player
// via `aurecv_stop()` (src/aureceiver.c), which mem_derefs `ar->auplay` (→ our
// destructor → thread join) before freeing the receiver `arg`, so `wh` never
// runs against a freed `arg`.
unsafe impl Send for WriteCb {}

/// Per-UA mono audio buffer (one for received, one for sent). Lets ringo-flow
/// verify a received tone in-process (Goertzel on these samples) and save
/// recordings — instead of baresip's sndfile WAV dumps. No shared-dir race, no
/// disk round-trip, per-UA isolated by construction.
struct AudioBuf {
    srate: u32,
    samples: VecDeque<i16>,
}

/// Seconds of audio retained per UA for verification (the verify window tail).
const VERIFY_RETAIN_SECS: usize = 3;
/// Seconds retained when full capture is on (`--save-audio`); bounds a runaway
/// call. Test calls are far shorter.
const FULL_RETAIN_SECS: usize = 600;

/// Whether to retain the whole call (for `--save-audio`) vs. just the verify
/// window. Set once per process from `BackendOptions.record_audio`.
static FULL_CAPTURE: AtomicBool = AtomicBool::new(false);

static RX_BUFFERS: OnceLock<Mutex<HashMap<String, AudioBuf>>> = OnceLock::new();
static TX_BUFFERS: OnceLock<Mutex<HashMap<String, AudioBuf>>> = OnceLock::new();

fn rx_buffers() -> &'static Mutex<HashMap<String, AudioBuf>> {
    RX_BUFFERS.get_or_init(|| Mutex::new(HashMap::new()))
}
fn tx_buffers() -> &'static Mutex<HashMap<String, AudioBuf>> {
    TX_BUFFERS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Enable/disable full-call capture (for `--save-audio`). When off, only the
/// rolling verify window is retained and the sent buffer isn't captured.
pub(super) fn set_full_capture(on: bool) {
    FULL_CAPTURE.store(on, Ordering::Release);
}

fn retain_secs() -> usize {
    if FULL_CAPTURE.load(Ordering::Acquire) {
        FULL_RETAIN_SECS
    } else {
        VERIFY_RETAIN_SECS
    }
}

/// Reset a UA's buffer in `buffers` to empty at `srate` (called on (re)alloc so
/// a verify/save sees only the current call).
fn reset_buffer(buffers: &Mutex<HashMap<String, AudioBuf>>, key: &str, srate: u32) {
    buffers.lock().unwrap_or_else(|e| e.into_inner()).insert(
        key.to_string(),
        AudioBuf {
            srate,
            samples: VecDeque::new(),
        },
    );
}

/// The retained received mono samples for `key` and their sample rate.
pub(super) fn received_window(key: &str) -> Option<(Vec<i16>, u32)> {
    buffer_window(rx_buffers(), key)
}

/// The retained sent mono samples for `key` and their sample rate (full capture
/// only; empty otherwise).
pub(super) fn sent_window(key: &str) -> Option<(Vec<i16>, u32)> {
    buffer_window(tx_buffers(), key)
}

fn buffer_window(buffers: &Mutex<HashMap<String, AudioBuf>>, key: &str) -> Option<(Vec<i16>, u32)> {
    let map = buffers.lock().unwrap_or_else(|e| e.into_inner());
    let buf = map.get(key)?;
    Some((buf.samples.iter().copied().collect(), buf.srate))
}

/// Append already-mono `samples` to the UA's buffer in `buffers`, capped.
fn capture_mono(buffers: &Mutex<HashMap<String, AudioBuf>>, key: &str, samples: &[i16]) {
    let mut map = buffers.lock().unwrap_or_else(|e| e.into_inner());
    let Some(buf) = map.get_mut(key) else {
        return;
    };
    buf.samples.extend(samples.iter().copied());
    let cap = (buf.srate as usize * retain_secs()).max(1);
    while buf.samples.len() > cap {
        buf.samples.pop_front();
    }
}

/// Append the channel-0 mono samples from an interleaved player frame to the
/// UA's received buffer. `wh` always fills the full `sampc` (aurecv_read pads an
/// underrun with silence), so the whole `sampv` is valid decoded audio.
fn capture_rx(key: &str, sampv: &[u8], is_float: bool, ch: usize) {
    let ch = ch.max(1);
    let mono: Vec<i16> = if is_float {
        let f =
            unsafe { std::slice::from_raw_parts(sampv.as_ptr() as *const f32, sampv.len() / 4) };
        f.chunks_exact(ch)
            .map(|fr| (fr[0] * 32767.0) as i16)
            .collect()
    } else {
        let s =
            unsafe { std::slice::from_raw_parts(sampv.as_ptr() as *const i16, sampv.len() / 2) };
        s.chunks_exact(ch).map(|fr| fr[0]).collect()
    };
    capture_mono(rx_buffers(), key, &mono);
}

unsafe extern "C" fn play_destructor(arg: *mut c_void) {
    let cell = arg as *mut *mut PlayState;
    let state = unsafe { *cell };
    if !state.is_null() {
        drop(unsafe { Box::from_raw(state) });
        unsafe { *cell = std::ptr::null_mut() };
    }
}

unsafe extern "C" fn play_alloc_handler(
    stp: *mut *mut auplay_st,
    _ap: *const auplay,
    prm: *mut auplay_prm,
    dev: *const c_char,
    wh: auplay_write_h,
    arg: *mut c_void,
) -> i32 {
    const EINVAL: i32 = 22;
    const ENOMEM: i32 = 12;

    if stp.is_null() || prm.is_null() || wh.is_none() {
        return EINVAL;
    }

    let prm = unsafe { &*prm };
    let srate = prm.srate;
    let ptime = if prm.ptime == 0 { 20 } else { prm.ptime };
    let fmt = prm.fmt;

    if srate == 0 || prm.ch == 0 {
        crate::rlog!(
            Warn,
            "ringo auplay: invalid prm srate={srate} ch={}",
            prm.ch
        );
        return EINVAL;
    }
    let ch = prm.ch;

    // The device key (= account username) identifies this UA's RX buffer. Reset
    // it on (re)alloc so a verify reads only the current call's received audio.
    let key = if dev.is_null() {
        String::new()
    } else {
        unsafe { std::ffi::CStr::from_ptr(dev) }
            .to_string_lossy()
            .into_owned()
    };
    if !key.is_empty() {
        reset_buffer(rx_buffers(), &key, srate);
    }

    let cell = unsafe { mem_zalloc(std::mem::size_of::<*mut PlayState>(), Some(play_destructor)) };
    if cell.is_null() {
        return ENOMEM;
    }

    let run = Arc::new(AtomicBool::new(true));
    let run_thread = run.clone();
    let cb = WriteCb {
        wh,
        arg: arg as usize,
    };

    let thread = match std::thread::Builder::new()
        .name("ringo-auplay".into())
        .spawn(move || play_loop(key, srate, ch, ptime, fmt, run_thread, cb))
    {
        Ok(t) => t,
        Err(e) => {
            // A player that never pulls would stall the decode pipeline; free the
            // cell (destructor no-ops on the null state) and fail.
            crate::rlog!(Error, "ringo auplay: spawn play thread failed: {e}");
            unsafe { mem_deref(cell) };
            return ENOMEM;
        }
    };

    let state = Box::new(PlayState {
        run,
        thread: Some(thread),
    });
    unsafe {
        *(cell as *mut *mut PlayState) = Box::into_raw(state);
        *stp = cell as *mut auplay_st;
    }
    0
}

/// Self-clocked player: every `ptime` ms, pull one frame from baresip via `wh`
/// (which drives the decode pipeline) and capture it into the UA's RX buffer for
/// in-process tone verification.
fn play_loop(
    key: String,
    srate: u32,
    ch: u8,
    ptime: u32,
    fmt: i32,
    run: Arc<AtomicBool>,
    cb: WriteCb,
) {
    let is_float = fmt == aufmt::AUFMT_FLOAT as i32;
    let sample_size = if is_float { 4 } else { 2 };
    let frames = (srate as usize * ptime as usize / 1000).max(1);
    let sampc = frames * ch as usize;
    let mut sampv = vec![0u8; sampc * sample_size];

    let af_fmt = if is_float {
        aufmt::AUFMT_FLOAT
    } else {
        aufmt::AUFMT_S16LE
    };

    let mut start = Instant::now();
    let mut frame_idx: u64 = 0;

    while run.load(Ordering::Acquire) {
        let target = start + Duration::from_millis(frame_idx * ptime as u64);
        let now = Instant::now();
        if target > now {
            std::thread::sleep(target - now);
        } else if now - target > Duration::from_millis(ptime as u64 * 4) {
            start = now;
            frame_idx = 0;
        }
        if !run.load(Ordering::Acquire) {
            break;
        }

        let mut af: auframe = unsafe { std::mem::zeroed() };
        unsafe {
            auframe_init(
                &mut af,
                af_fmt,
                sampv.as_mut_ptr() as *mut c_void,
                sampc,
                srate,
                ch,
            );
        }
        af.timestamp = frame_idx * ptime as u64 * 1000;

        if let Some(wh) = cb.wh {
            // wh fills sampv with the decoded received audio (aurecv_read →
            // aubuf_read_auframe), then we capture it for in-process verify.
            unsafe { wh(&mut af, cb.arg as *mut c_void) };
            if !key.is_empty() {
                capture_rx(&key, &sampv, is_float, ch as usize);
            }
        }

        frame_idx += 1;
    }
}

/// Register the `ringo` audio source + player modules with baresip. Must run
/// once after `baresip_init`/`ua_init` (so `baresip_ausrcl()`/`baresip_auplayl()`
/// exist) on the RE thread. Returns `Err` if registration fails — the caller
/// must abort, since the agents would otherwise start with no working audio.
/// The registered `ausrc`/`auplay` are intentionally never `mem_deref`'d (kept
/// for the process lifetime); we stash the pointers for documentation.
pub(super) fn register_module() -> Result<(), String> {
    let mut asp: *mut ausrc = std::ptr::null_mut();
    let rc = unsafe {
        ausrc_register(
            &mut asp,
            baresip_ausrcl(),
            c"ringo".as_ptr(),
            Some(alloc_handler),
        )
    };
    if rc != 0 {
        return Err(format!("ausrc_register(ringo) failed (rc={rc})"));
    }
    let _ = AUSRC.set(asp as usize);

    let mut pp: *mut auplay = std::ptr::null_mut();
    let rc = unsafe {
        auplay_register(
            &mut pp,
            baresip_auplayl(),
            c"ringo".as_ptr(),
            Some(play_alloc_handler),
        )
    };
    if rc != 0 {
        return Err(format!("auplay_register(ringo) failed (rc={rc})"));
    }
    let _ = AUPLAY.set(pp as usize);
    Ok(())
}
