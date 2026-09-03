//! The observable per-agent state, reduced from the worker's baresip events.
//! A minimal port of ringo-flow's `AgentState`: registration plus the live
//! call list — enough to answer `agent_status` without replaying events.

use ringo_core::event::{AppEvent, InviteHeaders};
use serde::Serialize;

/// Life phase of a call currently tracked by an agent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CallPhase {
    /// Incoming (not yet accepted) or outgoing (not yet answered) call.
    Ringing,
    /// Audio flowing.
    Established,
    /// The remote party put the call on hold (call up, media paused).
    Held,
}

/// One tracked call.
#[derive(Debug, Clone, Serialize)]
pub struct CallView {
    /// baresip call id (== SIP Call-ID).
    pub id: String,
    pub phase: CallPhase,
    /// Remote party URI (caller for incoming, callee for outgoing), if known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub peer: Option<String>,
    /// Remote party display name, if the INVITE carried one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub peer_name: Option<String>,
}

/// How many recent calls keep their received INVITE headers (bounded so a
/// long-running server can't grow the state without limit).
const MAX_RECEIVED_HEADERS: usize = 128;

/// The observable state of one agent, reduced from baresip events.
#[derive(Debug, Clone, Default)]
pub struct AgentState {
    pub registered: bool,
    pub reg_error: Option<String>,
    pub calls: Vec<CallView>,
    /// Why the most recently closed call ended (and whether it looked like an
    /// error, per [`ringo_core::event::is_error_reason`]).
    pub last_call_reason: Option<String>,
    pub last_call_error: bool,
    /// Set when the worker's event stream closed (worker exit/crash) — every
    /// tool call on this agent will fail afterwards.
    pub worker_dead: bool,
    /// Received INVITE headers per call, `(Call-ID, [(name, value), …])` in
    /// first-seen order. Filled by the hub's header poll (see `Agent::connect`),
    /// not by `reduce` — INVITE headers arrive on a separate channel from the
    /// worker. Persists after the call closes (capped at the most recent
    /// [`MAX_RECEIVED_HEADERS`] calls).
    pub received_headers: Vec<(String, Vec<(String, String)>)>,
}

/// Fold one event into the agent state.
pub fn reduce(state: &mut AgentState, event: &AppEvent) {
    match event {
        AppEvent::Registering { .. } => {}
        AppEvent::RegisterOk { .. } => {
            state.registered = true;
            state.reg_error = None;
        }
        AppEvent::RegisterFailed { reason } => {
            state.registered = false;
            state.reg_error = Some(reason.clone());
        }
        AppEvent::Unregistered { .. } => {
            state.registered = false;
        }
        AppEvent::CallIncoming {
            call_id,
            number,
            display_name,
        } => state.upsert_call(
            call_id,
            CallPhase::Ringing,
            Some(number),
            display_name.clone(),
        ),
        AppEvent::CallOutgoing { call_id, number } => {
            state.upsert_call(call_id, CallPhase::Ringing, Some(number), None)
        }
        AppEvent::CallRinging { call_id } => state.set_phase(call_id, CallPhase::Ringing),
        AppEvent::CallEstablished { call_id } => state.set_phase(call_id, CallPhase::Established),
        AppEvent::CallClosed {
            call_id,
            reason,
            error,
        } => {
            state.calls.retain(|c| &c.id != call_id);
            state.last_call_reason = Some(reason.clone());
            state.last_call_error = *error;
        }
        AppEvent::CallHold { call_id } => state.set_phase(call_id, CallPhase::Held),
        AppEvent::CallResume { call_id } => state.set_phase(call_id, CallPhase::Established),
        // Not state-relevant for the tool surface; surfaced live via wait_event.
        AppEvent::CallDeflected { .. }
        | AppEvent::CallTransferFailed { .. }
        | AppEvent::VoicemailStatus { .. }
        | AppEvent::Response { .. }
        | AppEvent::Unknown { .. }
        | AppEvent::BackendConnectFailed { .. } => {}
    }
}

impl AgentState {
    /// Merge freshly polled INVITE headers (per Call-ID) into the history,
    /// keeping first-seen order and evicting the oldest calls beyond
    /// [`MAX_RECEIVED_HEADERS`]. Headers for an already-known Call-ID are
    /// replaced (the worker only hands each INVITE out once, but a re-INVITE
    /// on the same Call-ID overwrites cleanly).
    pub fn merge_invites(&mut self, invites: InviteHeaders) {
        if invites.is_empty() {
            return;
        }
        // A poll batch is a HashMap (arbitrary iteration order) — sort it by
        // Call-ID so the history order is deterministic.
        let mut batch: Vec<_> = invites.into_iter().collect();
        batch.sort_by(|a, b| a.0.cmp(&b.0));
        for (call_id, headers) in batch {
            match self
                .received_headers
                .iter_mut()
                .find(|(id, _)| *id == call_id)
            {
                Some(entry) => entry.1 = headers,
                None => self.received_headers.push((call_id, headers)),
            }
        }
        let excess = self
            .received_headers
            .len()
            .saturating_sub(MAX_RECEIVED_HEADERS);
        if excess > 0 {
            self.received_headers.drain(..excess);
        }
    }

    /// Headers of one call's INVITE, if that call was seen.
    pub fn headers_of(&self, call_id: &str) -> Option<&[(String, String)]> {
        self.received_headers
            .iter()
            .find(|(id, _)| id == call_id)
            .map(|(_, h)| h.as_slice())
    }
}

impl AgentState {
    fn set_phase(&mut self, id: &str, phase: CallPhase) {
        if let Some(c) = self.calls.iter_mut().find(|c| c.id == id) {
            c.phase = phase;
        }
    }

    /// Create or update a call; phase updates keep an already-known peer.
    fn upsert_call(
        &mut self,
        id: &str,
        phase: CallPhase,
        peer: Option<&str>,
        peer_name: Option<String>,
    ) {
        let peer = peer.map(|p| p.to_string());
        match self.calls.iter_mut().find(|c| c.id == id) {
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
                id: id.to_string(),
                phase,
                peer,
                peer_name,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn incoming_then_established_then_closed() {
        let mut s = AgentState::default();
        reduce(
            &mut s,
            &AppEvent::CallIncoming {
                call_id: "c1".into(),
                number: "1002".into(),
                display_name: Some("Bob".into()),
            },
        );
        assert_eq!(s.calls.len(), 1);
        assert_eq!(s.calls[0].phase, CallPhase::Ringing);
        assert_eq!(s.calls[0].peer.as_deref(), Some("1002"));

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
                reason: "Connection reset by peer".into(),
                error: false,
            },
        );
        assert!(s.calls.is_empty());
        assert_eq!(
            s.last_call_reason.as_deref(),
            Some("Connection reset by peer")
        );
        assert!(!s.last_call_error);
    }

    #[test]
    fn peer_hold_and_resume_track_the_held_phase() {
        let mut s = AgentState::default();
        reduce(
            &mut s,
            &AppEvent::CallIncoming {
                call_id: "c1".into(),
                number: "1002".into(),
                display_name: None,
            },
        );
        reduce(
            &mut s,
            &AppEvent::CallEstablished {
                call_id: "c1".into(),
            },
        );
        reduce(
            &mut s,
            &AppEvent::CallHold {
                call_id: "c1".into(),
            },
        );
        assert_eq!(s.calls[0].phase, CallPhase::Held);
        reduce(
            &mut s,
            &AppEvent::CallResume {
                call_id: "c1".into(),
            },
        );
        assert_eq!(s.calls[0].phase, CallPhase::Established);
    }

    #[test]
    fn registration_lifecycle() {
        let mut s = AgentState::default();
        reduce(
            &mut s,
            &AppEvent::RegisterOk {
                account: "a".into(),
            },
        );
        assert!(s.registered);
        reduce(
            &mut s,
            &AppEvent::RegisterFailed {
                reason: "403".into(),
            },
        );
        assert!(!s.registered);
        assert_eq!(s.reg_error.as_deref(), Some("403"));
    }

    fn hdrs(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn invite_headers_merge_ordered_and_bounded() {
        let mut s = AgentState::default();
        s.merge_invites(HashMap::from([
            ("c1".into(), hdrs(&[("From", "<sip:1002@x>")])),
            ("c2".into(), hdrs(&[("X-Foo", "bar")])),
        ]));
        s.merge_invites(HashMap::from([(
            "c3".into(),
            hdrs(&[("History-Info", "a")]),
        )]));
        // First-seen order.
        let ids: Vec<&str> = s
            .received_headers
            .iter()
            .map(|(id, _)| id.as_str())
            .collect();
        assert_eq!(ids, vec!["c1", "c2", "c3"]);
        assert_eq!(s.headers_of("c2"), Some(&hdrs(&[("X-Foo", "bar")])[..]));
        assert_eq!(s.headers_of("nope"), None);

        // Bounded: the oldest calls' headers are evicted beyond the cap.
        for i in 4..=MAX_RECEIVED_HEADERS {
            s.merge_invites(HashMap::from([(format!("c{i}"), hdrs(&[("X", "y")]))]));
        }
        assert_eq!(s.received_headers.len(), MAX_RECEIVED_HEADERS);
        assert!(
            s.headers_of("c1").is_some(),
            "exactly at the cap, nothing evicted yet"
        );
        // One more call pushes it over the cap: only the oldest is evicted.
        s.merge_invites(HashMap::from([("cz".into(), hdrs(&[("X", "z")]))]));
        assert_eq!(s.received_headers.len(), MAX_RECEIVED_HEADERS);
        assert_eq!(s.headers_of("c1"), None, "oldest evicted");
        assert!(s.headers_of("c2").is_some());
        assert!(s.headers_of("c3").is_some());
        assert!(s.headers_of("cz").is_some());
    }
}
