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
                self.push_log("✓ Registered");
            }
            AppEvent::RegisterFailed { reason } => {
                self.reg_status = RegStatus::Failed(reason.clone());
                self.push_log(format!("✗ Registration failed: {}", reason));
            }
            AppEvent::CallIncoming { call_id, number } => {
                self.handle_call_incoming(call_id, number)
            }
            AppEvent::CallOutgoing { call_id, number } => {
                self.handle_call_outgoing(call_id, number)
            }
            AppEvent::CallRinging { call_id } => self.handle_call_ringing(call_id),
            AppEvent::CallEstablished { call_id } => self.handle_call_established(call_id),
            AppEvent::CallClosed { call_id } => self.handle_call_closed(call_id),
            AppEvent::CallOnHold { call_id } => self.handle_call_on_hold(call_id),
            AppEvent::CallResumed { call_id } => self.handle_call_resumed(call_id),
            AppEvent::TransferOk { info } => self.handle_transfer_ok(info),
            AppEvent::TransferFailed { reason } => self.handle_transfer_failed(reason),
            AppEvent::VoicemailStatus { waiting, new_count } => {
                self.mwi.waiting = waiting;
                self.mwi.new_messages = new_count;
                if waiting {
                    self.push_log(format!("✉ {} new voicemail message(s)", new_count));
                    self.notify("Voicemail", &format!("{} new message(s)", new_count));
                }
            }
            AppEvent::Response { ok, data } => {
                if data.is_empty() {
                    self.push_log(format!("[resp] ok={}", ok));
                } else {
                    for line in data.lines() {
                        self.push_log(format!("[resp] {}", line));
                    }
                }
            }
            AppEvent::Unknown { class, type_ } => {
                self.push_log(format!("[{}] {}", class, type_));
            }
        }
    }
}
