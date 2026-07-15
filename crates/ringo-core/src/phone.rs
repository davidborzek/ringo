use tokio::sync::mpsc::Sender;

pub trait Phone: Send {
    fn register(&self, aor: &str, regint: u32);
    fn dial(&self, number: &str);
    fn hangup(&self);
    fn hangup_all(&self);
    fn accept(&self);
    fn hold(&self);
    fn resume(&self);
    fn mute(&self);
    fn send_dtmf(&self, digit: char);
    fn switch_line(&self, line: usize);
    fn transfer(&self, uri: &str);
    fn attended_transfer_start(&self, uri: &str);
    fn attended_transfer_exec(&self);
    fn attended_transfer_abort(&self);
    fn add_header(&self, key: &str, value: &str);
    fn rm_header(&self, key: &str);
    /// Switch the audio source at runtime, e.g. `ausine,440` to send a tone,
    /// `aufile,/path.wav` to play a file, or `aubridge,default` to go silent.
    /// Applies to the active call.
    fn set_audio_source(&self, spec: &str);

    /// RTP media stats (jitter/loss/RTT + estimated MOS) for the active call, or
    /// the last finished call's snapshot. `None` if unavailable (no call yet, or
    /// before the first RTCP report). Default: `None` (mocks).
    fn media_stats(&self) -> Option<crate::event::MediaStats> {
        None
    }

    /// The negotiated audio codec on the active call. `None` if there's no call
    /// or it isn't negotiated yet. Default: `None` (mocks).
    fn audio_codec(&self) -> Option<crate::event::CodecInfo> {
        None
    }

    /// DTMF digits received on the active/last call so far, in order (e.g.
    /// `"1234#"`). Default: empty (mocks).
    fn received_dtmf(&self) -> String {
        String::new()
    }

    /// Arm a custom SIP response for the next inbound INVITE(s): answer with
    /// `scode`/`reason` and the extra `headers` (each a full header line like
    /// `Contact: <sip:…>`, no trailing CRLF) instead of accepting the call.
    /// Sticky until [`Phone::disarm_invite_response`]. Arm it *before* the call
    /// arrives for deterministic behaviour.
    fn arm_invite_response(&self, scode: u16, reason: &str, headers: Vec<String>);

    /// Clear any armed response — subsequent inbound INVITEs are accepted again.
    fn disarm_invite_response(&self);

    /// Deflect inbound calls to `contact` with a `302 Moved Temporarily` (plus an
    /// RFC 5806 `Diversion` header when `diversion` is set). Thin wrapper over
    /// [`Phone::arm_invite_response`].
    fn deflect_incoming(&self, contact: &str, diversion: Option<&str>) {
        let mut headers = vec![format!("Contact: <{contact}>")];
        if let Some(div) = diversion {
            headers.push(format!("Diversion: <{div}>"));
        }
        self.arm_invite_response(302, "Moved Temporarily", headers);
    }
}

// ─── Test mock ────────────────────────────────────────────────────────────────
//
// A simple `Phone` impl that records every command as a `(String, String)` pair
// into a channel. Used by the TUI tests to verify that user interactions
// produce the expected phone commands. The real implementation lives in
// `baresip/phone.rs` and calls libbaresip C functions via FFI.

pub struct MockPhone {
    cmd_tx: Sender<(String, String)>,
}

impl MockPhone {
    pub fn new(cmd_tx: Sender<(String, String)>) -> Self {
        Self { cmd_tx }
    }

    fn send(&self, cmd: &str, params: &str) {
        if let Err(e) = self.cmd_tx.try_send((cmd.to_string(), params.to_string())) {
            crate::rlog!(Warn, "cmd dropped: {} ({})", cmd, e);
        }
    }
}

impl Phone for MockPhone {
    fn register(&self, _aor: &str, regint: u32) {
        self.send("uareg", &format!("{} 0", regint));
    }
    fn dial(&self, number: &str) {
        self.send("dial", number);
    }
    fn hangup(&self) {
        self.send("hangup", "");
    }
    fn hangup_all(&self) {
        self.send("hangupall", "");
    }
    fn accept(&self) {
        self.send("accept", "");
    }
    fn hold(&self) {
        self.send("hold", "");
    }
    fn resume(&self) {
        self.send("resume", "");
    }
    fn mute(&self) {
        self.send("mute", "");
    }
    fn send_dtmf(&self, digit: char) {
        self.send("sndcode", &digit.to_string());
    }
    fn switch_line(&self, line: usize) {
        self.send("line", &line.to_string());
    }
    fn transfer(&self, uri: &str) {
        self.send("transfer", uri);
    }
    fn attended_transfer_start(&self, uri: &str) {
        self.send("atransferstart", uri);
    }
    fn attended_transfer_exec(&self) {
        self.send("atransferexec", "");
    }
    fn attended_transfer_abort(&self) {
        self.send("atransferabort", "");
    }
    fn add_header(&self, key: &str, value: &str) {
        self.send(
            "uaaddheader",
            &format!("{}={} 0", key, uri_header_escape(value)),
        );
    }
    fn rm_header(&self, key: &str) {
        self.send("uarmheader", &format!("{} 0", key));
    }
    fn set_audio_source(&self, spec: &str) {
        self.send("ausrc", spec);
    }
    fn arm_invite_response(&self, scode: u16, reason: &str, headers: Vec<String>) {
        self.send(
            "armresponse",
            &format!("{scode} {reason} [{}]", headers.join("; ")),
        );
    }
    fn disarm_invite_response(&self) {
        self.send("disarmresponse", "");
    }
}

/// Percent-encode a SIP header value for the `uaaddheader` baresip command.
///
/// Why: baresip's command parser splits params on the first space, so a raw
/// space silently truncates the value. baresip then runs the value through
/// `uri_header_unescape`, which only accepts the RFC 3261 `hvalue` charset
/// (alnum, unreserved marks, and `[ ] / ? : + $`). Anything outside that set
/// must be `%HH`-encoded so it survives the round trip.
fn uri_header_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for &b in s.as_bytes() {
        if is_hvalue(b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{:02X}", b));
        }
    }
    out
}

fn is_hvalue(b: u8) -> bool {
    b.is_ascii_alphanumeric()
        || matches!(
            b,
            b'-' | b'_'
                | b'.'
                | b'!'
                | b'~'
                | b'*'
                | b'\''
                | b'('
                | b')'
                | b'['
                | b']'
                | b'/'
                | b'?'
                | b':'
                | b'+'
                | b'$'
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn passes_through_hvalue_chars() {
        assert_eq!(uri_header_escape("Foo-Bar_1.0"), "Foo-Bar_1.0");
        assert_eq!(uri_header_escape("a[b]/c?d:e+f$"), "a[b]/c?d:e+f$");
    }

    #[test]
    fn escapes_spaces_and_specials() {
        assert_eq!(uri_header_escape("Foo Bar"), "Foo%20Bar");
        assert_eq!(uri_header_escape("a=b;c,d"), "a%3Db%3Bc%2Cd");
        assert_eq!(uri_header_escape("100%"), "100%25");
    }

    #[test]
    fn escapes_non_ascii() {
        assert_eq!(uri_header_escape("ä"), "%C3%A4");
    }

    fn make_phone() -> (MockPhone, tokio::sync::mpsc::Receiver<(String, String)>) {
        let (tx, rx) = tokio::sync::mpsc::channel(8);
        (MockPhone::new(tx), rx)
    }

    #[test]
    fn add_header_emits_uaaddheader_with_ua_index_zero() {
        let (phone, mut rx) = make_phone();
        phone.add_header("X-Foo", "bar");
        let (cmd, params) = rx.try_recv().expect("one message");
        assert_eq!(cmd, "uaaddheader");
        assert_eq!(params, "X-Foo=bar 0");
    }

    #[test]
    fn add_header_encodes_unsafe_chars_in_value() {
        let (phone, mut rx) = make_phone();
        phone.add_header("History-Info", "<sip:1@x.com>;index=1");
        let (_, params) = rx.try_recv().expect("one message");
        assert_eq!(params, "History-Info=%3Csip:1%40x.com%3E%3Bindex%3D1 0");
    }

    #[test]
    fn add_header_preserves_order_across_multiple_calls() {
        let (phone, mut rx) = make_phone();
        phone.add_header("History-Info", "<sip:1@x.com>;index=1");
        phone.add_header("History-Info", "<sip:2@x.com>;index=2");
        phone.add_header("X-Other", "hi");

        let msgs: Vec<_> = std::iter::from_fn(|| rx.try_recv().ok()).collect();
        assert_eq!(msgs.len(), 3);
        assert_eq!(msgs[0].1, "History-Info=%3Csip:1%40x.com%3E%3Bindex%3D1 0");
        assert_eq!(msgs[1].1, "History-Info=%3Csip:2%40x.com%3E%3Bindex%3D2 0");
        assert_eq!(msgs[2].1, "X-Other=hi 0");
    }
}
