use super::app::RegStatus;
use crate::event::AppEvent;

impl super::app::App {
    pub fn handle_message(&mut self, event: AppEvent) {
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
                self.reg_status = RegStatus::Failed("Unregistered".into());
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
            }
            // Remote-control responses go back over the socket; there's no TUI echo.
            AppEvent::Response { .. } => {}
            AppEvent::Unknown { .. } => {}
            AppEvent::BackendConnectFailed { reason } => {
                self.reg_status = RegStatus::Failed(format!("backend unreachable: {}", reason));
            }
        }
    }
}
