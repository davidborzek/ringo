use std::collections::HashMap;

/// Headers of each INVITE seen in a trace, keyed by SIP `Call-ID`. First
/// INVITE per Call-ID wins (the call-establishing one).
pub type InviteHeaders = HashMap<String, Vec<(String, String)>>;

/// RTP media quality for a call (backend-neutral). `mos` is an estimated Mean
/// Opinion Score (1.0 = worst … 4.5 = best).
#[derive(Debug, Clone, Copy)]
pub struct MediaStats {
    /// Round-trip time in milliseconds.
    pub rtt_ms: f64,
    /// Receive-side inter-arrival jitter in milliseconds.
    pub jitter_ms: f64,
    /// Cumulative received RTP packets lost.
    pub rx_lost: i32,
    /// Receive-side packet loss as a percentage.
    pub packet_loss_pct: f64,
    /// Estimated MOS (1.0–4.5).
    pub mos: f64,
}

#[derive(Debug, Clone)]
pub enum AppEvent {
    Registering {
        account: String,
    },
    RegisterOk {
        account: String,
    },
    RegisterFailed {
        reason: String,
    },
    Unregistered {
        account: String,
    },
    CallIncoming {
        call_id: String,
        number: String,
        display_name: Option<String>,
    },
    CallOutgoing {
        call_id: String,
        number: String,
    },
    CallRinging {
        call_id: String,
    },
    CallEstablished {
        call_id: String,
    },
    CallClosed {
        call_id: String,
        reason: String,
        error: bool,
    },
    VoicemailStatus {
        waiting: bool,
        new_count: u32,
    },
    Response {
        ok: bool,
        data: String,
    },
    Unknown {
        class: String,
        type_: String,
    },
    BackendConnectFailed {
        reason: String,
    },
}

/// Whether a call-closed reason indicates an error (not a normal close).
/// Backend-neutral: shared by all backends that produce SIP reason strings.
pub fn is_error_reason(reason: &str) -> bool {
    if reason.is_empty() {
        return false;
    }
    const NORMAL: &[&str] = &[
        "Connection reset by peer",
        "Connection closed",
        "Rejected by user",
        "Call transfered",
    ];
    !NORMAL
        .iter()
        .any(|n| reason.to_lowercase().starts_with(&n.to_lowercase()))
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── is_error_reason ────────────────────────────────────────────────────────

    #[test]
    fn empty_reason_is_not_error() {
        assert!(!is_error_reason(""));
    }

    #[test]
    fn connection_reset_is_not_error() {
        assert!(!is_error_reason("Connection reset by peer"));
    }

    #[test]
    fn connection_reset_with_errno_is_not_error() {
        assert!(!is_error_reason("Connection reset by peer [104]"));
    }

    #[test]
    fn connection_closed_is_not_error() {
        assert!(!is_error_reason("Connection closed"));
    }

    #[test]
    fn rejected_by_user_is_not_error() {
        assert!(!is_error_reason("Rejected by user"));
    }

    #[test]
    fn sip_busy_is_error() {
        assert!(is_error_reason("486 Busy Here"));
    }

    #[test]
    fn sip_not_found_is_error() {
        assert!(is_error_reason("404 Not Found"));
    }
}
