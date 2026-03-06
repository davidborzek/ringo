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
        let _ = self.cmd_tx.try_send((cmd.to_string(), params.to_string()));
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
        self.send("uaaddheader", &format!("{}={} 0", key, value));
    }
}
