use crate::client::BaresipMessage;
use serde_json::{Map, Value};

#[derive(Debug)]
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
    CallIncoming {
        call_id: String,
        number: String,
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
    CallOnHold {
        call_id: String,
    },
    CallResumed {
        call_id: String,
    },
    TransferOk {
        info: String,
    },
    TransferFailed {
        reason: String,
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
}

impl From<BaresipMessage> for AppEvent {
    fn from(msg: BaresipMessage) -> Self {
        match msg {
            BaresipMessage::Event {
                class,
                type_,
                param,
                extra,
            } => map_event(&class, &type_, param, &extra),
            BaresipMessage::Response { ok, data, .. } => AppEvent::Response { ok, data },
        }
    }
}

fn map_event(class: &str, type_: &str, param: String, extra: &Map<String, Value>) -> AppEvent {
    let t = type_.trim_start_matches("BEVENT_");
    let call_id = || {
        extra
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string()
    };
    let account = || {
        extra
            .get("accountaor")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string()
    };
    let number = || {
        extra
            .get("peeruri")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| param.clone())
    };

    match t {
        "REGISTERING" => AppEvent::Registering { account: account() },
        "REGISTER_OK" | "FALLBACK_OK" => AppEvent::RegisterOk { account: account() },
        "REGISTER_FAIL" | "FALLBACK_FAIL" => AppEvent::RegisterFailed { reason: param },
        "CALL_INCOMING" => AppEvent::CallIncoming {
            call_id: call_id(),
            number: number(),
        },
        "CALL_OUTGOING" => AppEvent::CallOutgoing {
            call_id: call_id(),
            number: number(),
        },
        "CALL_RINGING" => AppEvent::CallRinging { call_id: call_id() },
        "CALL_ESTABLISHED" => AppEvent::CallEstablished { call_id: call_id() },
        "CALL_CLOSED" => {
            let error = is_error_reason(&param);
            AppEvent::CallClosed {
                call_id: call_id(),
                reason: param,
                error,
            }
        }
        "CALL_HOLD" => AppEvent::CallOnHold { call_id: call_id() },
        "CALL_RESUME" => AppEvent::CallResumed { call_id: call_id() },
        "TRANSFER" => AppEvent::TransferOk { info: param },
        "TRANSFER_FAILED" => AppEvent::TransferFailed { reason: param },
        "MWI_NOTIFY" => parse_mwi(&param),
        _ => AppEvent::Unknown {
            class: class.to_string(),
            type_: type_.to_string(),
        },
    }
}

fn is_error_reason(reason: &str) -> bool {
    if reason.is_empty() {
        return false;
    }
    const NORMAL: &[&str] = &[
        "Connection reset by peer",
        "Connection closed",
        "Rejected by user",
    ];
    !NORMAL
        .iter()
        .any(|n| reason.to_lowercase().starts_with(&n.to_lowercase()))
}

fn parse_mwi(param: &str) -> AppEvent {
    let mut waiting = false;
    let mut new_count = 0u32;
    for line in param.lines() {
        if let Some(val) = line.strip_prefix("Messages-Waiting:") {
            waiting = val.trim().eq_ignore_ascii_case("yes");
        }
        if let Some(val) = line.strip_prefix("Voice-Message:") {
            if let Some(new) = val.trim().split('/').next() {
                new_count = new.trim().parse().unwrap_or(0);
            }
        }
    }
    AppEvent::VoicemailStatus { waiting, new_count }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::BaresipMessage;
    use serde_json::json;

    fn event_msg(type_: &str, param: &str, extra: serde_json::Value) -> BaresipMessage {
        let mut map = extra.as_object().cloned().unwrap_or_default();
        map.insert("class".into(), json!("call"));
        BaresipMessage::Event {
            class: "call".into(),
            type_: type_.into(),
            param: param.into(),
            extra: map,
        }
    }

    // ── MWI ────────────────────────────────────────────────────────────────────

    #[test]
    fn mwi_waiting_yes() {
        let param = "Messages-Waiting: yes\r\nVoice-Message: 3/0";
        let event = AppEvent::from(event_msg("BEVENT_MWI_NOTIFY", param, json!({})));
        assert!(matches!(
            event,
            AppEvent::VoicemailStatus {
                waiting: true,
                new_count: 3
            }
        ));
    }

    #[test]
    fn mwi_waiting_no() {
        let param = "Messages-Waiting: no\r\nVoice-Message: 0/0";
        let event = AppEvent::from(event_msg("BEVENT_MWI_NOTIFY", param, json!({})));
        assert!(matches!(
            event,
            AppEvent::VoicemailStatus {
                waiting: false,
                new_count: 0
            }
        ));
    }

    // ── event mapping ──────────────────────────────────────────────────────────

    #[test]
    fn register_ok_event() {
        let extra = json!({"accountaor": "sip:bob@example.com"});
        let event = AppEvent::from(event_msg("BEVENT_REGISTER_OK", "", extra));
        assert!(
            matches!(event, AppEvent::RegisterOk { account } if account == "sip:bob@example.com")
        );
    }

    #[test]
    fn call_incoming_event() {
        let extra = json!({"id": "call-1", "peeruri": "sip:carol@example.com"});
        let event = AppEvent::from(event_msg("BEVENT_CALL_INCOMING", "", extra));
        assert!(matches!(event, AppEvent::CallIncoming { call_id, number }
                if call_id == "call-1" && number == "sip:carol@example.com"));
    }

    #[test]
    fn unknown_event() {
        let event = AppEvent::from(event_msg("BEVENT_SOMETHING_NEW", "", json!({})));
        assert!(matches!(event, AppEvent::Unknown { .. }));
    }

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

    #[test]
    fn call_closed_error_flag() {
        let extra = json!({"id": "call-1"});
        let event = AppEvent::from(event_msg("BEVENT_CALL_CLOSED", "486 Busy Here", extra));
        assert!(matches!(event, AppEvent::CallClosed { error: true, .. }));
    }

    #[test]
    fn call_closed_no_error_flag_for_normal_close() {
        let extra = json!({"id": "call-1"});
        let event = AppEvent::from(event_msg(
            "BEVENT_CALL_CLOSED",
            "Connection reset by peer [104]",
            extra,
        ));
        assert!(matches!(event, AppEvent::CallClosed { error: false, .. }));
    }
}
