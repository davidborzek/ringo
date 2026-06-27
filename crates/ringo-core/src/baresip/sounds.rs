//! Embedded sound playback — ring/ringback tones via `play_tone`.
//!
//! WAV files are embedded at compile time via `include_bytes!` and parsed
//! at runtime to extract raw PCM samples. The samples are written into a
//! libre `mbuf` and played via `play_tone` — no temp files needed.

use std::ffi::CString;
use std::sync::OnceLock;

use super::bindings::*;

/// WAV file embedded at compile time.
struct EmbeddedWav {
    data: &'static [u8],
}

/// Parsed WAV: raw PCM samples + sample rate + channel count.
pub(super) struct Pcm {
    pub(super) samples: Vec<u8>,
    pub(super) srate: u32,
    pub(super) channels: u8,
}

/// Minimal WAV (RIFF) parser — extracts the `data` chunk and format info.
/// Supports PCM (format 1) and G.711 mu-law (format 7). G.711 is decoded
/// to S16LE via the embedded lookup table (same as libre's g711_ulaw2pcm).
pub(super) fn parse_wav(data: &[u8]) -> Option<Pcm> {
    if data.len() < 44 {
        return None;
    }
    if &data[0..4] != b"RIFF" || &data[8..12] != b"WAVE" {
        return None;
    }
    let mut pos = 12;
    let mut audio_format: u16 = 1;
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
            }
            b"data" => {
                let raw = &data[chunk_start..chunk_end];
                // play_tone expects S16LE PCM. Convert if needed.
                let samples = match audio_format {
                    1 => raw.to_vec(), // PCM — already S16LE (assumed)
                    7 => mu2pcm(raw),  // G.711 mu-law
                    _ => raw.to_vec(), // Unknown — pass through (best effort)
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

/// Stable heap cell holding the current alert tone's `*mut play`.
///
/// `play_tone` records this address (`play->playp`) and its destructor writes
/// `*playp = NULL` when the tone is freed — which happens LATER, inside
/// `stop_alert`'s `mem_deref`. So `playp` must NOT be a stack slot: by then the
/// caller's frame is gone and the write corrupts memory (→ SIGSEGV). We leak
/// one heap cell for the process lifetime and reuse it as the single alert slot.
///
/// Process-wide single slot — one audio output device, one ring/ringback tone
/// at a time, replaced on each new alert. Only ever touched on the RE thread
/// (play_alert/stop_alert both run from the bevent handler), so no lock needed.
fn alert_playp() -> *mut *mut Play {
    static CELL: OnceLock<usize> = OnceLock::new();
    *CELL.get_or_init(|| Box::into_raw(Box::new(std::ptr::null_mut::<Play>())) as usize)
        as *mut *mut Play
}

/// Play an embedded WAV file on the alert device. Replaces any currently
/// playing tone. The WAV is parsed in Rust, the PCM samples are written
/// into a libre `mbuf`, and `play_tone` is called — no file I/O needed.
pub fn play_alert(filename: &str) {
    stop_alert();
    let wav = match filename {
        "ring.wav" => EmbeddedWav {
            data: include_bytes!("../../../../vendor/baresip/share/ring.wav"),
        },
        "ringback.wav" => EmbeddedWav {
            data: include_bytes!("../../../../vendor/baresip/share/ringback.wav"),
        },
        _ => return,
    };
    let pcm = match parse_wav(wav.data) {
        Some(p) => p,
        None => {
            crate::rlog!(Warn, "play_alert: failed to parse WAV '{filename}'");
            return;
        }
    };
    let (play_mod, play_dev) = get_alert_device();
    let samples = pcm.samples;
    let srate = pcm.srate;
    let ch = pcm.channels;
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
        let mb = mbuf_alloc(samples.len());
        if mb.is_null() {
            crate::rlog!(Warn, "play_alert: mbuf_alloc() failed");
            return;
        }
        let rc = mbuf_write_mem(mb, samples.as_ptr(), samples.len());
        if rc != 0 {
            crate::rlog!(Warn, "play_alert: mbuf_write_mem() failed (rc={rc})");
            mem_deref(mb as *mut std::os::raw::c_void);
            return;
        }
        // playp points at the stable heap cell (not a stack slot) so baresip's
        // destructor can null it out safely later. stop_alert (called above)
        // already freed any previous tone, leaving *playp NULL.
        let playp = alert_playp();
        let rc = play_tone(playp, player, mb, srate, ch, -1, play_mod, play_dev);
        mem_deref(mb as *mut std::os::raw::c_void);
        if rc != 0 {
            crate::rlog!(Warn, "play_alert: play_tone('{filename}') failed (rc={rc})");
        }
    }
}

/// Stop the currently playing alert tone (if any).
/// Like [`play_alert`], only called from the bevent handler (RE thread).
pub fn stop_alert() {
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
