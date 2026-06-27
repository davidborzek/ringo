//! Optional SIP message tracing — every SIP request/response (sent + received)
//! written to its OWN sink, independent of the regular log. Two formats:
//!
//! - **text** (default / non-`.pcap` path / stderr): human-readable, one block
//!   per message.
//! - **pcap** (path ending in `.pcap`): a libpcap capture readable by sngrep /
//!   Wireshark (VoIP flow graph). Crucial with TLS: the on-the-wire traffic is
//!   encrypted, so a live sniffer sees nothing — but the trace handler gets the
//!   PLAINTEXT SIP (pre-encrypt / post-decrypt), which `pcap` frames as
//!   Ethernet/IP/UDP so the tools can parse the flow.
//!
//! We install our own handler via libre's `sip_set_trace_handler()`, NOT
//! baresip's `uag_enable_sip_trace()` (which `re_printf`s to stdout with ANSI and
//! would corrupt the reporter/NDJSON / TUI).

mod pcap;

use std::fs::OpenOptions;
use std::io::Write;
use std::os::raw::c_void;
use std::panic::{self, AssertUnwindSafe};
use std::path::Path;
use std::sync::{Mutex, OnceLock};
use std::time::SystemTime;

use super::bindings::*;

/// `AF_INET` is 2 on both Linux and macOS; `AF_INET6` differs (10 vs 30), so we
/// only test for IPv4 and treat everything else as IPv6.
const AF_INET: i32 = 2;

enum Sink {
    Text(Box<dyn Write + Send>),
    Pcap(Box<dyn Write + Send>),
}

/// Dedicated SIP-trace sink. Set ⇒ tracing is on (handler installed in baresip
/// init); unset ⇒ off. Separate from the regular log sink.
static SINK: OnceLock<Mutex<Sink>> = OnceLock::new();

/// Trace SIP messages to `path`. A `.pcap` extension writes a libpcap capture
/// (sngrep/Wireshark); anything else writes the text format. Parent dirs are
/// created; first call wins. Call before the first session is spawned.
pub fn init_file(path: impl AsRef<Path>) {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let Ok(mut f) = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(path)
    else {
        return;
    };
    let pcap = path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("pcap"));
    let sink = if pcap {
        if f.write_all(&pcap::global_header()).is_err() {
            return;
        }
        Sink::Pcap(Box::new(f))
    } else {
        Sink::Text(Box::new(f))
    };
    let _ = SINK.set(Mutex::new(sink));
}

/// Trace SIP messages to stderr (text). Call before the first session is spawned.
pub fn init_stderr() {
    let _ = SINK.set(Mutex::new(Sink::Text(Box::new(std::io::stderr()))));
}

/// libre SIP trace callback. Runs on the RE thread; a panic here would cross
/// FFI, so guard it.
unsafe extern "C" fn trace_cb(
    tx: bool,
    tp: sip_transp,
    src: *const sa,
    dst: *const sa,
    pkt: *const u8,
    len: usize,
    _arg: *mut c_void,
) {
    let _ = panic::catch_unwind(AssertUnwindSafe(|| {
        let Some(mtx) = SINK.get() else {
            return;
        };
        if pkt.is_null() || len == 0 {
            return;
        }
        let bytes = unsafe { std::slice::from_raw_parts(pkt, len) };
        let mut sink = mtx.lock().unwrap_or_else(|e| e.into_inner());
        match &mut *sink {
            Sink::Text(w) => {
                let msg = String::from_utf8_lossy(bytes);
                let dir = if tx { "TX →" } else { "RX ←" };
                let transp = transp_name(tp);
                let ts = chrono::Local::now().format("%H:%M:%S%.3f");
                let _ = writeln!(w, "[{ts}] SIP {dir} {transp}\n{}\n", msg.trim_end());
            }
            Sink::Pcap(w) => {
                if let Some(rec) = unsafe { pcap_record(src, dst, bytes) } {
                    let _ = w.write_all(&rec);
                }
            }
        }
    }));
}

fn transp_name(tp: sip_transp) -> &'static str {
    let p = unsafe { sip_transp_name(tp) };
    if p.is_null() {
        return "?";
    }
    unsafe { std::ffi::CStr::from_ptr(p) }
        .to_str()
        .unwrap_or("?")
}

/// Read src/dst from libre's `sa` structs and hand them to the protocol-agnostic
/// `pcap` writer. `None` if the addresses can't be read.
unsafe fn pcap_record(src: *const sa, dst: *const sa, payload: &[u8]) -> Option<Vec<u8>> {
    if src.is_null() || dst.is_null() {
        return None;
    }
    let sport = unsafe { sa_port(src) };
    let dport = unsafe { sa_port(dst) };
    let (s, d): (Vec<u8>, Vec<u8>) = if unsafe { sa_af(src) } == AF_INET {
        (
            unsafe { sa_in(src) }.to_be_bytes().to_vec(),
            unsafe { sa_in(dst) }.to_be_bytes().to_vec(),
        )
    } else {
        let (mut s, mut d) = ([0u8; 16], [0u8; 16]);
        unsafe {
            sa_in6(src, s.as_mut_ptr());
            sa_in6(dst, d.as_mut_ptr());
        }
        (s.to_vec(), d.to_vec())
    };
    Some(pcap::record(
        &s,
        &d,
        sport,
        dport,
        payload,
        SystemTime::now(),
    ))
}

/// Install the trace handler if a sink was configured. Called from the one-time
/// baresip init (already on the RE thread, after `ua_init`).
pub(super) fn install_if_requested() {
    if SINK.get().is_none() {
        return;
    }
    unsafe {
        let sip = uag_sip();
        if !sip.is_null() {
            sip_set_trace_handler(sip, Some(trace_cb));
        }
    }
}
