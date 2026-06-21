//! Recover SIP headers from baresip's `-s` SIP trace (captured in `baresip.log`).
//!
//! baresip's ctrl_tcp event API exposes only a fixed set of fields — **not**
//! arbitrary custom headers from an inbound INVITE. The trace, however, prints
//! every SIP message in full. This module parses INVITE requests out of the
//! trace and keys their headers by SIP `Call-ID`.
//!
//! Why keying by Call-ID is enough to get *inbound* headers without detecting
//! message direction: for an **incoming** call baresip sets its ctrl_tcp call id
//! to the SIP Call-ID, so looking up by that id hits the received INVITE. For an
//! **outgoing** call the ctrl_tcp id is unrelated random hex, so a sent INVITE's
//! Call-ID never collides with it. A caller therefore does
//! `headers_for(&trace, &call_incoming_id)` and gets exactly the inbound headers.
//!
//! The parser is tolerant: it strips ANSI colour codes (baresip should be run
//! with `-c`, but we don't rely on it), accepts CRLF or LF, and ignores
//! interleaved non-SIP log lines.

use std::collections::HashMap;
use std::path::Path;

/// Headers of each INVITE seen in the trace, keyed by SIP `Call-ID`. First
/// INVITE per Call-ID wins (the call-establishing one).
pub type InviteHeaders = HashMap<String, Vec<(String, String)>>;

/// Incrementally tails a baresip log file that contains an `-s` SIP trace,
/// re-parsing only when the file has grown. Cheap to poll on a timer; lets a
/// caller pick up an inbound INVITE's headers shortly after they're written
/// (which can lag the ctrl_tcp `CALL_INCOMING` event).
#[derive(Debug, Default)]
pub struct TraceTail {
    last_len: u64,
}

impl TraceTail {
    pub fn new() -> Self {
        Self::default()
    }

    /// Re-scan `log_path` if it grew since the last poll, returning the INVITE
    /// headers parsed from the whole file. `None` when the file is unchanged,
    /// missing, or unreadable — so a caller can skip work on idle ticks.
    ///
    /// Assumes an append-only log (each baresip instance writes one fresh
    /// `baresip.log`): a shorter file is treated as "unchanged", not re-read.
    /// Re-parses the whole file on growth, which is fine for the short scenarios
    /// this targets but is O(n²) over a very long, chatty run.
    pub fn poll(&mut self, log_path: &Path) -> Option<InviteHeaders> {
        let len = std::fs::metadata(log_path).ok()?.len();
        if len <= self.last_len {
            return None;
        }
        self.last_len = len;
        let trace = std::fs::read_to_string(log_path).ok()?;
        Some(parse_invites(&trace))
    }
}

/// Parse a (possibly partial) `-s` trace dump into INVITE headers by Call-ID.
// The outer and inner loops pull from the same iterator (header block is nested
// inside the message scan), so `while let … next()` is the right shape here.
#[allow(clippy::while_let_on_iterator)]
pub fn parse_invites(trace: &str) -> InviteHeaders {
    let mut out: InviteHeaders = HashMap::new();
    let mut lines = trace.lines().map(strip_ansi).peekable();

    while let Some(line) = lines.next() {
        let line = line.trim_end_matches('\r');
        if !is_invite_request_line(line) {
            continue;
        }
        // Collect header lines until the blank line that ends the header block.
        let mut headers: Vec<(String, String)> = Vec::new();
        while let Some(raw) = lines.next() {
            let h = raw.trim_end_matches('\r');
            if h.is_empty() {
                break; // end of headers (start of body)
            }
            if (h.starts_with(' ') || h.starts_with('\t')) && !headers.is_empty() {
                // Folded continuation: append to the previous header's value.
                let last = headers.last_mut().unwrap();
                last.1.push(' ');
                last.1.push_str(h.trim());
            } else if let Some((name, value)) = h.split_once(':') {
                headers.push((name.trim().to_string(), value.trim().to_string()));
            }
        }

        if let Some(call_id) = headers
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case("Call-ID"))
            .map(|(_, v)| v.clone())
        {
            out.entry(call_id).or_insert(headers);
        }
    }
    out
}

/// Headers of the INVITE whose Call-ID equals `call_id` (the ctrl_tcp id of an
/// incoming call). Empty slice if the trace hasn't been flushed yet.
pub fn headers_for<'a>(trace: &'a InviteHeaders, call_id: &str) -> &'a [(String, String)] {
    trace.get(call_id).map(Vec::as_slice).unwrap_or(&[])
}

/// First value of header `name` (case-insensitive) in a parsed header list.
pub fn header_value<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|(n, _)| n.eq_ignore_ascii_case(name))
        .map(|(_, v)| v.as_str())
}

fn is_invite_request_line(line: &str) -> bool {
    line.starts_with("INVITE ") && line.ends_with(" SIP/2.0")
}

/// Remove ANSI escape sequences (`ESC [ … m`) from a line.
fn strip_ansi(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut chars = line.chars();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            // Skip "[…<final byte>"; CSI final bytes are in 0x40..=0x7e.
            if chars.next() == Some('[') {
                for d in chars.by_ref() {
                    if ('@'..='~').contains(&d) {
                        break;
                    }
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    // A received INVITE block as printed by `baresip -s` (ANSI timestamp line,
    // UDP direction line, CRLF message), modeled on baresip's `-s` output.
    const TRACE: &str = "\x1b[36;1m10:13:16.228#\r\n\
UDP 192.0.2.1:5060 -> 192.0.2.1:5070\r\n\
INVITE sip:bob@192.0.2.1:5070 SIP/2.0\r\n\
Via: SIP/2.0/UDP 192.0.2.1:5060;branch=z9hG4bK75826597781dd06f;rport\r\n\
To: <sip:bob@192.0.2.1:5070>\r\n\
From: <sip:alice@192.0.2.1:5060>;tag=17527a49416fad9c\r\n\
Call-ID: 6f30f685a993adf8\r\n\
CSeq: 63450 INVITE\r\n\
X-Trace-Id: spike-CAFEBABE-42\r\n\
Content-Type: application/sdp\r\n\
Content-Length: 330\r\n\
\r\n\
v=0\r\n\
o=- 1402857639 698399491 IN IP4 192.0.2.1\r\n";

    #[test]
    fn extracts_headers_keyed_by_call_id() {
        let map = parse_invites(TRACE);
        let h = headers_for(&map, "6f30f685a993adf8");
        assert_eq!(header_value(h, "X-Trace-Id"), Some("spike-CAFEBABE-42"));
        assert_eq!(header_value(h, "call-id"), Some("6f30f685a993adf8"));
        // The SDP body must not leak into the header list.
        assert!(header_value(h, "v").is_none());
    }

    #[test]
    fn unknown_call_id_yields_empty() {
        let map = parse_invites(TRACE);
        assert!(headers_for(&map, "does-not-exist").is_empty());
    }

    #[test]
    fn header_lookup_is_case_insensitive() {
        let map = parse_invites(TRACE);
        let h = headers_for(&map, "6f30f685a993adf8");
        assert_eq!(header_value(h, "X-TRACE-ID"), Some("spike-CAFEBABE-42"));
    }

    #[test]
    fn folded_header_is_joined() {
        // NB: no `\`-continuation before " part two" — that would eat the
        // leading space that marks the fold. One literal line with \n escapes.
        let trace = "INVITE sip:x SIP/2.0\nCall-ID: abc\nSubject: part one\n part two\n\n";
        let map = parse_invites(trace);
        let h = headers_for(&map, "abc");
        assert_eq!(header_value(h, "Subject"), Some("part one part two"));
    }

    #[test]
    fn first_invite_per_call_id_wins() {
        let trace = "INVITE sip:x SIP/2.0\r\nCall-ID: dup\r\nX-N: first\r\n\r\n\
INVITE sip:x SIP/2.0\r\nCall-ID: dup\r\nX-N: second\r\n\r\n";
        let map = parse_invites(trace);
        assert_eq!(header_value(headers_for(&map, "dup"), "X-N"), Some("first"));
    }

    #[test]
    fn non_invite_messages_are_ignored() {
        let trace = "SIP/2.0 180 Ringing\r\nCall-ID: ring\r\n\r\n\
BYE sip:x SIP/2.0\r\nCall-ID: bye\r\n\r\n";
        assert!(parse_invites(trace).is_empty());
    }

    #[test]
    fn trace_tail_reparses_only_on_growth() {
        use std::io::Write;
        let path = std::env::temp_dir().join(format!(
            "ringo_tracetail_{}_{}.log",
            std::process::id(),
            line!()
        ));
        let _ = std::fs::remove_file(&path);
        let mut tail = TraceTail::new();

        // Missing file → None.
        assert!(tail.poll(&path).is_none());

        // First content with an INVITE → parsed.
        let mut f = std::fs::File::create(&path).unwrap();
        write!(f, "INVITE sip:x SIP/2.0\r\nCall-ID: c1\r\nX-N: a\r\n\r\n").unwrap();
        f.flush().unwrap();
        let first = tail.poll(&path).expect("grew → Some");
        assert_eq!(header_value(headers_for(&first, "c1"), "X-N"), Some("a"));

        // Unchanged → None.
        assert!(tail.poll(&path).is_none());

        // Appended a second INVITE → re-parsed, both present.
        let mut f = std::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap();
        write!(f, "INVITE sip:y SIP/2.0\r\nCall-ID: c2\r\nX-N: b\r\n\r\n").unwrap();
        f.flush().unwrap();
        let second = tail.poll(&path).expect("grew again → Some");
        assert_eq!(header_value(headers_for(&second, "c1"), "X-N"), Some("a"));
        assert_eq!(header_value(headers_for(&second, "c2"), "X-N"), Some("b"));

        let _ = std::fs::remove_file(&path);
    }
}
