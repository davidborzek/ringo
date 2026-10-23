use super::app::RegStatus;
use crate::config::HookEvent;
use crate::event::AppEvent;

impl super::app::App {
    pub fn handle_message(&mut self, event: AppEvent) {
        let reg_before = self.reg_status.clone();
        self.apply_event(event);
        if self.reg_status != reg_before {
            self.registration_changed();
        }
    }

    /// `registration_changed` fires on every transition, not on each REGISTER
    /// refresh: baresip re-reports `RegisterOk` every interval.
    fn registration_changed(&self) {
        let (status, reason) = match &self.reg_status {
            RegStatus::Unknown => ("unknown", ""),
            RegStatus::Registering => ("registering", ""),
            RegStatus::Ok => ("registered", ""),
            RegStatus::Unregistered => ("unregistered", ""),
            RegStatus::Failed(reason) => ("failed", reason.as_str()),
        };
        self.run_hook(
            HookEvent::RegistrationChanged,
            serde_json::json!({
                "status": status,
                "reason": reason,
                "account": self.account_aor,
            }),
        );
    }

    fn apply_event(&mut self, event: AppEvent) {
        match event {
            AppEvent::Registering { account } => {
                self.reg_status = RegStatus::Registering;
                if !account.is_empty() {
                    self.account_aor = account;
                }
            }
            AppEvent::RegisterOk { account } => {
                self.reg_status = RegStatus::Ok;
                if !account.is_empty() {
                    self.account_aor = account;
                }
            }
            AppEvent::RegisterFailed { reason } => {
                self.reg_status = RegStatus::Failed(reason.clone());
            }
            AppEvent::Unregistered { .. } => {
                self.reg_status = RegStatus::Unregistered;
            }
            AppEvent::CallIncoming {
                call_id,
                number,
                display_name,
            } => self.handle_call_incoming(call_id, number, display_name),
            AppEvent::CallOutgoing { call_id, number } => {
                self.handle_call_outgoing(call_id, number)
            }
            AppEvent::CallRinging { call_id } => self.handle_call_ringing(call_id),
            AppEvent::CallEstablished { call_id } => self.handle_call_established(call_id),
            AppEvent::CallClosed {
                call_id,
                reason,
                error,
            } => self.handle_call_closed(call_id, reason, error),
            AppEvent::CallDeflected {
                from,
                display_name,
                target,
            } => self.handle_call_deflected(from, display_name, target),
            AppEvent::VoicemailStatus { waiting, new_count } => {
                let changed = self.mwi.waiting != waiting || self.mwi.new_messages != new_count;
                self.mwi.waiting = waiting;
                self.mwi.new_messages = new_count;
                if changed && waiting {
                    self.notify("Voicemail", &format!("{} new message(s)", new_count));
                }
                if changed {
                    self.run_hook(
                        HookEvent::Voicemail,
                        serde_json::json!({ "waiting": waiting, "new_count": new_count }),
                    );
                }
            }
            // The remote party holds/resumes — mirror it into the call state.
            AppEvent::CallHold { call_id } => self.handle_call_held(call_id),
            AppEvent::CallResume { call_id } => self.handle_call_resumed(call_id),
            // A failed transfer leaves the original call untouched — nothing to
            // update in the TUI (the transfer mode was left by its own flow).
            AppEvent::CallTransferFailed { .. } => {}
            // Remote-control responses go back over the socket; there's no TUI echo.
            AppEvent::Response { .. } => {}
            AppEvent::Unknown { .. } => {}
            AppEvent::BackendConnectFailed { reason } => {
                self.reg_status = RegStatus::Failed(format!("backend unreachable: {}", reason));
            }
        }
    }
}
