//! Read access to the workers' SIP traces for the `sip_trace` tool.
//!
//! The workers already write every SIP message (plaintext, pre-encrypt /
//! post-decrypt) to a per-agent file when `RINGO_AGENT_SIPTRACE` is set — see
//! ringo-agent's `init_logging`. ringo-mcp turns that on by default (a text
//! trace in a private temp dir) and parses the current worker's file on
//! demand, so the LLM can see why a call failed without any change to the
//! worker protocol.

use anyhow::{Context, Result, bail};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

/// The env var the workers read their trace destination from.
pub const TRACE_ENV: &str = "RINGO_AGENT_SIPTRACE";

/// A single message is cut here — SDP bodies are small, but the tool reply must
/// stay bounded even for a pathological peer.
const MAX_MESSAGE_BYTES: usize = 8 * 1024;

/// Only the newest part of a trace file is read per call, so a long-running
/// worker's growing file can't blow up memory or the reply.
const MAX_READ_BYTES: u64 = 2 * 1024 * 1024;

/// Where the worker trace files live, and whether the tool can read them.
#[derive(Debug, Clone)]
pub enum SipTraceFiles {
    /// Text traces; `base` is the `RINGO_AGENT_SIPTRACE` value the workers
    /// derive their per-agent file names from.
    Text {
        /// The configured trace path (before the per-agent tag).
        base: String,
    },
    /// Tracing goes somewhere the tool can't read (stderr, pcap); the reason
    /// is reported to the caller.
    Unavailable(String),
}

impl SipTraceFiles {
    /// Derive the file layout from a `RINGO_AGENT_SIPTRACE` value.
    pub fn from_env_value(value: &str) -> Self {
        match value {
            "" => {
                Self::Unavailable("SIP tracing is disabled (RINGO_AGENT_SIPTRACE is empty)".into())
            }
            "-" => Self::Unavailable("SIP traces go to stderr (RINGO_AGENT_SIPTRACE=-)".into()),
            path => {
                let pcap = Path::new(path)
                    .extension()
                    .is_some_and(|e| e.eq_ignore_ascii_case("pcap"));
                if pcap {
                    return Self::Unavailable(format!(
                        "SIP traces are written as pcap ({path}); open them with sngrep/Wireshark"
                    ));
                }
                Self::Text {
                    base: path.to_string(),
                }
            }
        }
    }

    /// The trace file of the worker `pid` running `agent` — exactly the name
    /// the worker itself writes (shared naming function).
    pub fn worker_file(&self, agent: &str, pid: u32) -> Result<PathBuf> {
        match self {
            Self::Text { base } => Ok(ringo_agent::worker::per_agent_path(base, agent, pid)),
            Self::Unavailable(reason) => bail!("{reason}"),
        }
    }
}

/// One SIP message from a trace file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SipMessage {
    /// Byte offset of the message in the trace file plus one: stable across
    /// reads and never 0, so it doubles as the `after_id` cursor (0 = from
    /// the start).
    pub id: u64,
    /// Local wall-clock time `HH:MM:SS.mmm`.
    pub ts: String,
    /// `"tx"` (sent) or `"rx"` (received).
    pub direction: &'static str,
    /// SIP transport (`UDP`, `TCP`, `TLS`, …).
    pub transport: String,
    /// Request line or status line.
    pub first_line: String,
    /// The `Call-ID` header value, if present.
    pub call_id: Option<String>,
    /// The full message (headers + body), `\r` stripped, possibly cut.
    pub text: String,
}

/// What one read of a trace file yields.
#[derive(Debug, Default)]
pub struct TracePage {
    /// Complete messages after the cursor, oldest first.
    pub messages: Vec<SipMessage>,
    /// Older messages exist that this read did not reach (outside the read
    /// window, or the cursor fell behind it).
    pub truncated: bool,
}

/// Read the messages after `after_id` from the newest part of `path`. A
/// missing file is an empty page (the worker hasn't sent anything yet).
pub fn read_page(path: &Path, after_id: Option<u64>) -> Result<TracePage> {
    let mut file = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(TracePage::default()),
        Err(e) => return Err(e).with_context(|| format!("open {}", path.display())),
    };
    let len = file.metadata()?.len();
    let window_start = len.saturating_sub(MAX_READ_BYTES);
    // The cursor message's own offset: reading resumes there, and the
    // mid-file parse skips that (already seen) message.
    let seen_until = after_id.unwrap_or(0).saturating_sub(1);
    let start = seen_until.max(window_start).min(len);
    file.seek(SeekFrom::Start(start))?;
    let mut buf = Vec::new();
    file.take(len - start).read_to_end(&mut buf)?;

    let messages = parse(&buf, start)
        .into_iter()
        .filter(|m| after_id.is_none_or(|a| m.id > a))
        .collect();
    Ok(TracePage {
        messages,
        truncated: window_start > seen_until,
    })
}

/// Parse the worker's text trace format — per message one write of
/// `[HH:MM:SS.mmm] SIP TX → UDP\n<message>\n\n` — from `buf`, which starts at
/// file offset `offset`.
///
/// - Only complete writes count: everything after the last `\n\n` is still
///   being written and is ignored.
/// - A block header is only recognised right after the previous write's
///   terminator (a bare empty line), so header-like lines inside a message
///   body (whose lines end in `\r\n`) don't split it.
/// - A read starting mid-file (`offset > 0`) skips to the first header after
///   such a terminator.
pub fn parse(buf: &[u8], offset: u64) -> Vec<SipMessage> {
    let complete_len = buf
        .windows(2)
        .rposition(|w| w == b"\n\n")
        .map_or(0, |i| i + 2);
    let buf = &buf[..complete_len];

    let mut messages = Vec::new();
    let mut current: Option<Block> = None;
    // At file start the first line may be a header; mid-file the first
    // (possibly partial) line never is.
    let mut prev_was_terminator = offset == 0;
    let mut pos = 0usize;
    for raw in buf.split(|&b| b == b'\n') {
        let line_offset = offset + pos as u64;
        pos += raw.len() + 1;
        let header = if prev_was_terminator {
            block_header(&String::from_utf8_lossy(raw))
        } else {
            None
        };
        prev_was_terminator = raw.is_empty();
        if let Some((ts, direction, transport)) = header {
            if let Some(block) = current.take() {
                messages.extend(block.finish());
            }
            current = Some(Block {
                id: line_offset + 1,
                ts,
                direction,
                transport,
                lines: Vec::new(),
            });
        } else if let Some(block) = current.as_mut() {
            block.lines.push(raw);
        }
    }
    if let Some(block) = current {
        messages.extend(block.finish());
    }
    messages
}

/// A trace block being collected.
struct Block<'a> {
    id: u64,
    ts: String,
    direction: &'static str,
    transport: String,
    lines: Vec<&'a [u8]>,
}

impl Block<'_> {
    fn finish(self) -> Option<SipMessage> {
        let text: Vec<String> = self
            .lines
            .iter()
            .map(|l| String::from_utf8_lossy(l.strip_suffix(b"\r").unwrap_or(l)).into_owned())
            .collect();
        let text = text.join("\n").trim().to_string();
        if text.is_empty() {
            return None;
        }
        let first_line = text.lines().next().unwrap_or_default().to_string();
        let call_id = call_id(&text);
        Some(SipMessage {
            id: self.id,
            ts: self.ts,
            direction: self.direction,
            transport: self.transport,
            first_line,
            call_id,
            text: cut(text),
        })
    }
}

/// Cut `text` to at most [`MAX_MESSAGE_BYTES`] bytes on a char boundary.
fn cut(mut text: String) -> String {
    if text.len() <= MAX_MESSAGE_BYTES {
        return text;
    }
    let mut end = MAX_MESSAGE_BYTES;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text.truncate(end);
    text.push_str("\n[… truncated]");
    text
}

/// `[12:34:56.789] SIP TX → UDP` → `("12:34:56.789", "tx", "UDP")`.
fn block_header(line: &str) -> Option<(String, &'static str, String)> {
    let rest = line.strip_prefix('[')?;
    let (ts, rest) = rest.split_once("] SIP ")?;
    let is_ts = ts.len() == 12
        && ts.bytes().enumerate().all(|(i, b)| match i {
            2 | 5 => b == b':',
            8 => b == b'.',
            _ => b.is_ascii_digit(),
        });
    if !is_ts {
        return None;
    }
    let (direction, transport) = match rest.strip_prefix("TX → ") {
        Some(t) => ("tx", t),
        None => ("rx", rest.strip_prefix("RX ← ")?),
    };
    if transport.is_empty() || transport.contains(' ') {
        return None;
    }
    Some((ts.to_string(), direction, transport.to_string()))
}

/// The `Call-ID` (or compact `i`) header value from the message's header part.
fn call_id(text: &str) -> Option<String> {
    text.lines()
        .skip(1)
        .take_while(|l| !l.is_empty())
        .find_map(|l| {
            let (name, value) = l.split_once(':')?;
            let name = name.trim();
            (name.eq_ignore_ascii_case("call-id") || name.eq_ignore_ascii_case("i"))
                .then(|| value.trim().to_string())
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    const INVITE: &str = "[10:00:00.001] SIP TX → UDP\nINVITE sip:1002@pbx SIP/2.0\r\nVia: SIP/2.0/UDP 10.0.0.1\r\nCall-ID: abc@host\r\nContent-Length: 4\r\n\r\nv=0\n\n";
    const TRYING: &str = "[10:00:00.050] SIP RX ← UDP\nSIP/2.0 100 Trying\r\ni: abc@host\n\n";
    const REGISTER: &str =
        "[10:00:01.000] SIP TX → TLS\nREGISTER sip:pbx SIP/2.0\r\nCALL-ID: reg-1\n\n";

    #[test]
    fn parses_blocks_with_direction_transport_and_call_id() {
        let all = format!("{INVITE}{TRYING}{REGISTER}");
        let msgs = parse(all.as_bytes(), 0);
        assert_eq!(msgs.len(), 3);
        assert_eq!(msgs[0].id, 1);
        assert_eq!(msgs[0].ts, "10:00:00.001");
        assert_eq!(msgs[0].direction, "tx");
        assert_eq!(msgs[0].transport, "UDP");
        assert_eq!(msgs[0].first_line, "INVITE sip:1002@pbx SIP/2.0");
        assert_eq!(msgs[0].call_id.as_deref(), Some("abc@host"));
        assert!(msgs[0].text.ends_with("v=0"), "body kept: {}", msgs[0].text);
        assert!(!msgs[0].text.contains('\r'));
        // Ids are file offsets of the block headers, plus one.
        assert_eq!(msgs[1].id, INVITE.len() as u64 + 1);
        assert_eq!(msgs[2].id, (INVITE.len() + TRYING.len()) as u64 + 1);
        // Compact form `i:` and case-insensitive `CALL-ID:`.
        assert_eq!(msgs[1].direction, "rx");
        assert_eq!(msgs[1].call_id.as_deref(), Some("abc@host"));
        assert_eq!(msgs[2].transport, "TLS");
        assert_eq!(msgs[2].call_id.as_deref(), Some("reg-1"));
    }

    #[test]
    fn ignores_writes_in_progress() {
        // A partial next block — even just the start of its header line —
        // must not swallow the complete block before it.
        for partial in [
            "[10:00:",
            "[10:00:02.000] SIP TX → UDP\nBYE sip:1002@pbx SIP/2.0\r\nCall-",
        ] {
            let msgs = parse(format!("{INVITE}{partial}").as_bytes(), 0);
            assert_eq!(msgs.len(), 1, "{partial:?}");
            assert_eq!(msgs[0].first_line, "INVITE sip:1002@pbx SIP/2.0");
        }
    }

    #[test]
    fn header_like_body_lines_do_not_split_a_message() {
        let tricky = "[10:00:00.001] SIP RX ← TLS\nMESSAGE sip:x SIP/2.0\r\n\r\n[10:00:00.002] SIP RX ← TLS\r\nSIP/2.0 403 Forbidden\r\n\n";
        let msgs = parse(tricky.as_bytes(), 0);
        assert_eq!(msgs.len(), 1);
        assert!(msgs[0].text.contains("403 Forbidden"));
    }

    #[test]
    fn mid_file_reads_start_at_the_next_header() {
        let all = format!("{INVITE}{TRYING}{REGISTER}");
        // Start inside the INVITE: it is skipped, the rest keeps file offsets.
        let start = 10;
        let msgs = parse(&all.as_bytes()[start..], start as u64);
        assert_eq!(msgs.len(), 2);
        assert_eq!(msgs[0].id, INVITE.len() as u64 + 1);
        assert_eq!(msgs[0].first_line, "SIP/2.0 100 Trying");
    }

    #[test]
    fn long_messages_are_cut_by_bytes_on_a_char_boundary() {
        let body = "ä".repeat(MAX_MESSAGE_BYTES);
        let msgs = parse(
            format!("[10:00:00.001] SIP RX ← UDP\nSIP/2.0 200 OK\r\n\r\n{body}\n\n").as_bytes(),
            0,
        );
        assert!(msgs[0].text.ends_with("[… truncated]"));
        assert!(msgs[0].text.len() <= MAX_MESSAGE_BYTES + "\n[… truncated]".len());
    }

    #[test]
    fn read_page_pages_by_offset_cursor() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("sip.alice.1.trace");
        assert!(read_page(&path, None).unwrap().messages.is_empty());

        std::fs::write(&path, format!("{INVITE}{TRYING}{REGISTER}")).unwrap();
        let page = read_page(&path, None).unwrap();
        assert_eq!(page.messages.len(), 3);
        assert!(!page.truncated);
        let page = read_page(&path, Some(page.messages[0].id)).unwrap();
        assert_eq!(page.messages.len(), 2);
        assert_eq!(page.messages[0].first_line, "SIP/2.0 100 Trying");
        let last = page.messages[1].id;
        assert!(read_page(&path, Some(last)).unwrap().messages.is_empty());
    }

    #[test]
    fn cursor_from_an_empty_trace_still_sees_the_first_message() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("sip.alice.1.trace");
        std::fs::write(&path, "").unwrap();
        assert!(read_page(&path, Some(0)).unwrap().messages.is_empty());
        // The tool hands out 0 as the cursor of an empty trace.
        std::fs::write(&path, INVITE).unwrap();
        let page = read_page(&path, Some(0)).unwrap();
        assert_eq!(page.messages.len(), 1);
        assert!(!page.truncated);
    }

    #[test]
    fn env_value_maps_to_layout() {
        for unavailable in ["", "-", "/tmp/x/run.PCAP"] {
            assert!(
                matches!(
                    SipTraceFiles::from_env_value(unavailable),
                    SipTraceFiles::Unavailable(_)
                ),
                "{unavailable:?}"
            );
        }
        let err = SipTraceFiles::from_env_value("-")
            .worker_file("alice", 1)
            .unwrap_err()
            .to_string();
        assert!(err.contains("stderr"), "{err}");
        // Same naming as the worker, including name sanitising.
        let files = SipTraceFiles::from_env_value("/tmp/x/sip.trace");
        assert_eq!(
            files.worker_file("alice.b", 42).unwrap(),
            PathBuf::from("/tmp/x/sip.alice_b.42.trace")
        );
    }
}
