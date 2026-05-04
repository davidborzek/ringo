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
}

// ─── Baresip implementation ───────────────────────────────────────────────────

pub struct BaresipPhone {
    cmd_tx: Sender<(String, String)>,
}

impl BaresipPhone {
    pub fn new(cmd_tx: Sender<(String, String)>) -> Self {
        Self { cmd_tx }
    }

    fn send(&self, cmd: &str, params: &str) {
        if let Err(e) = self.cmd_tx.try_send((cmd.to_string(), params.to_string())) {
            crate::rlog!(Warn, "cmd dropped: {} ({})", cmd, e);
        }
    }
}

impl Phone for BaresipPhone {
    fn register(&self, _aor: &str, regint: u32) {
        // uareg requires "<regint> <ua_index>" — UA index must be specified
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
    use super::uri_header_escape;

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
}
