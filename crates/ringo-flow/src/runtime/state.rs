//! The observable per-agent state, reduced from baresip events. The Rhai host
//! reads it through getters (`a.registered`, `a.state`); `received_header_value`
//! backs the `header` getter and header assertions.

use ringo_core::event::AppEvent;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, PartialEq)]
pub enum CallPhase {
    Ringing,
    Established,
}

#[derive(Debug, Clone)]
pub struct CallView {
    pub id: String,
    pub phase: CallPhase,
    /// Remote party URI (the caller for an incoming call, the callee for an
    /// outgoing one), from baresip's `peeruri`. `None` if the call was only ever
    /// seen via a phase event.
    pub peer: Option<String>,
    /// Remote party display name (`peerdisplayname`), if the INVITE carried one.
    pub peer_name: Option<String>,
}

/// The observable state of one agent, reduced from baresip events.
#[derive(Debug, Clone, Default)]
pub struct AgentState {
    pub registered: bool,
    pub reg_error: Option<String>,
    pub calls: Vec<CallView>,
    pub last_call_reason: Option<String>,
    /// Headers of received INVITEs, keyed by SIP Call-ID (== the incoming call's
    /// id). Populated from baresip's SIP trace, since the ctrl_tcp event API
    /// doesn't expose inbound headers. Persists after a call closes.
    pub received_headers: HashMap<String, Vec<(String, String)>>,
}

impl AgentState {
    fn call_mut(&mut self, id: &str) -> Option<&mut CallView> {
        self.calls.iter_mut().find(|c| c.id == id)
    }
    fn set_phase(&mut self, id: String, phase: CallPhase) {
        match self.call_mut(&id) {
            Some(c) => c.phase = phase,
            None => self.calls.push(CallView {
                id,
                phase,
                peer: None,
                peer_name: None,
            }),
        }
    }
    /// Create or update a call, recording the remote party (caller/callee). Phase
    /// updates keep an already-known peer; a provided peer/name overwrites.
    fn upsert_call(
        &mut self,
        id: String,
        phase: CallPhase,
        peer: Option<String>,
        peer_name: Option<String>,
    ) {
        match self.call_mut(&id) {
            Some(c) => {
                c.phase = phase;
                if peer.is_some() {
                    c.peer = peer;
                }
                if peer_name.is_some() {
                    c.peer_name = peer_name;
                }
            }
            None => self.calls.push(CallView {
                id,
                phase,
                peer,
                peer_name,
            }),
        }
    }
    /// The current call's remote party `(uri, display_name)`, if any — the most
    /// recently seen call. For an incoming call this is the caller ID.
    pub fn peer(&self) -> Option<(String, Option<String>)> {
        self.calls
            .last()
            .and_then(|c| c.peer.clone().map(|uri| (uri, c.peer_name.clone())))
    }
    /// All received INVITE headers across calls, flattened in insertion order
    /// (preserves duplicates, e.g. History-Info). Backs `headers()`.
    pub fn received_headers_flat(&self) -> Vec<(String, String)> {
        self.received_headers.values().flatten().cloned().collect()
    }
    /// IDs of all current calls (any phase).
    pub fn call_ids(&self) -> HashSet<String> {
        self.calls.iter().map(|c| c.id.clone()).collect()
    }
    /// IDs of calls that are currently established.
    pub fn established_ids(&self) -> HashSet<String> {
        self.calls
            .iter()
            .filter(|c| c.phase == CallPhase::Established)
            .map(|c| c.id.clone())
            .collect()
    }
}

/// Fold one event into the agent state.
pub fn reduce(state: &mut AgentState, event: &AppEvent) {
    match event {
        AppEvent::RegisterOk { .. } => {
            state.registered = true;
            state.reg_error = None;
        }
        AppEvent::RegisterFailed { reason } => {
            state.registered = false;
            state.reg_error = Some(reason.clone());
        }
        AppEvent::CallIncoming {
            call_id,
            number,
            display_name,
        } => state.upsert_call(
            call_id.clone(),
            CallPhase::Ringing,
            Some(number.clone()),
            display_name.clone(),
        ),
        AppEvent::CallOutgoing { call_id, number } => state.upsert_call(
            call_id.clone(),
            CallPhase::Ringing,
            Some(number.clone()),
            None,
        ),
        AppEvent::CallRinging { call_id } => state.set_phase(call_id.clone(), CallPhase::Ringing),
        AppEvent::CallEstablished { call_id } => {
            state.set_phase(call_id.clone(), CallPhase::Established)
        }
        AppEvent::CallClosed {
            call_id, reason, ..
        } => {
            state.calls.retain(|c| &c.id != call_id);
            state.last_call_reason = Some(reason.clone());
        }
        _ => {}
    }
}

/// First value of header `name` (case-insensitive) across received INVITEs.
/// Backs the `a.header("…")` getter and header assertions.
pub fn received_header_value(state: &AgentState, name: &str) -> Option<String> {
    state
        .received_headers
        .values()
        .flatten()
        .find(|(n, _)| n.eq_ignore_ascii_case(name))
        .map(|(_, v)| v.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev_incoming(id: &str) -> AppEvent {
        AppEvent::CallIncoming {
            call_id: id.into(),
            number: "sip:x@y".into(),
            display_name: None,
        }
    }

    #[test]
    fn reduces_call_lifecycle() {
        let mut s = AgentState::default();
        reduce(
            &mut s,
            &AppEvent::RegisterOk {
                account: "a".into(),
            },
        );
        assert!(s.registered);

        reduce(&mut s, &ev_incoming("c1"));
        assert_eq!(s.calls.len(), 1);
        assert_eq!(s.calls[0].phase, CallPhase::Ringing);
        // the caller id (peeruri) is retained on the call
        assert_eq!(s.peer(), Some(("sip:x@y".into(), None)));

        reduce(
            &mut s,
            &AppEvent::CallEstablished {
                call_id: "c1".into(),
            },
        );
        assert_eq!(s.calls[0].phase, CallPhase::Established);

        reduce(
            &mut s,
            &AppEvent::CallClosed {
                call_id: "c1".into(),
                reason: "486 Busy Here".into(),
                error: true,
            },
        );
        assert!(s.calls.is_empty());
        assert_eq!(s.last_call_reason.as_deref(), Some("486 Busy Here"));
    }

    #[test]
    fn received_header_lookup_is_case_insensitive() {
        let mut s = AgentState::default();
        s.received_headers.insert(
            "callid-1".into(),
            vec![("X-Trace-Id".into(), "abc-123".into())],
        );
        assert_eq!(
            received_header_value(&s, "x-trace-id").as_deref(),
            Some("abc-123")
        );
        assert_eq!(received_header_value(&s, "X-Absent"), None);
    }
}
