use super::app::{Call, CallDirection, CallState, TransferMode};

impl super::app::App {
    pub(super) fn handle_call_incoming(&mut self, call_id: String, number: String) {
        self.notify("Incoming call", &number);
        self.calls.push(Call {
            id: call_id,
            peer: number,
            direction: CallDirection::Incoming,
            state: CallState::Ringing,
            started_at: None,
        });
    }

    pub(super) fn handle_call_outgoing(&mut self, call_id: String, number: String) {
        self.calls.push(Call {
            id: call_id,
            peer: number,
            direction: CallDirection::Outgoing,
            state: CallState::Ringing,
            started_at: None,
        });
        // During attended transfer, auto-select the new outgoing call (Line B)
        if self.transfer_mode == TransferMode::AttendedPending {
            self.selected_call = self.calls.len() - 1;
        }
    }

    pub(super) fn handle_call_ringing(&mut self, call_id: String) {
        if let Some(c) = self.calls.iter_mut().find(|c| c.id == call_id) {
            c.state = CallState::Ringing;
        }
    }

    pub(super) fn handle_call_established(&mut self, call_id: String) {
        if let Some(c) = self.calls.iter_mut().find(|c| c.id == call_id) {
            c.state = CallState::Established;
            if c.started_at.is_none() {
                c.started_at = Some(std::time::Instant::now());
                self.dial.dtmf.clear();
            }
        }
    }

    pub(super) fn handle_call_closed(&mut self, call_id: String) {
        if let Some(call) = self.calls.iter().find(|c| c.id == call_id) {
            if call.direction == CallDirection::Incoming && call.started_at.is_none() {
                self.notify("Missed call", &call.peer.clone());
            }
            self.append_call_history(call);
        }
        self.dial.dtmf.clear();
        self.calls.retain(|c| c.id != call_id);
        if self.selected_call >= self.calls.len() && !self.calls.is_empty() {
            self.selected_call = self.calls.len() - 1;
        }
    }

    pub(super) fn handle_call_on_hold(&mut self, call_id: String) {
        if let Some(c) = self.calls.iter_mut().find(|c| c.id == call_id) {
            c.state = CallState::OnHold;
        }
    }

    pub(super) fn handle_call_resumed(&mut self, call_id: String) {
        if let Some(c) = self.calls.iter_mut().find(|c| c.id == call_id) {
            c.state = CallState::Established;
        }
    }

    pub(super) fn in_active_call(&self) -> bool {
        self.calls
            .get(self.selected_call)
            .map(|c| c.state == CallState::Established)
            .unwrap_or(false)
    }

    pub(super) fn has_any_call(&self) -> bool {
        !self.calls.is_empty()
    }

    pub(super) fn selected_call_on_hold(&self) -> bool {
        self.calls
            .get(self.selected_call)
            .map(|c| c.state == CallState::OnHold)
            .unwrap_or(false)
    }

    pub(super) fn has_incoming_ringing(&self) -> bool {
        self.calls
            .iter()
            .any(|c| c.direction == CallDirection::Incoming && c.state == CallState::Ringing)
    }

    /// Send a DTMF tone during an active call.
    pub(super) fn send_dtmf(&mut self, digit: char) {
        self.phone.send_dtmf(digit);
        self.dial.dtmf.push(digit);
    }

    /// Switch to the next call line, automatically holding the current and resuming the next.
    pub(super) fn switch_line(&mut self) {
        if self.calls.len() < 2 {
            return;
        }
        let cur = self.selected_call;
        if self
            .calls
            .get(cur)
            .map(|c| c.state == CallState::Established)
            .unwrap_or(false)
        {
            self.phone.hold();
            if let Some(c) = self.calls.get_mut(cur) {
                c.state = CallState::OnHold;
            }
        }
        let next = (cur + 1) % self.calls.len();
        self.selected_call = next;
        self.phone.switch_line(next + 1);
        if self
            .calls
            .get(next)
            .map(|c| c.state == CallState::OnHold)
            .unwrap_or(false)
        {
            self.phone.resume();
            if let Some(c) = self.calls.get_mut(next) {
                c.state = CallState::Established;
            }
        }
    }
}
