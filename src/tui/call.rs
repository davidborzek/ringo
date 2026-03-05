use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, List, ListItem, Paragraph},
};

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

pub(super) fn render_calls(f: &mut Frame, app: &super::app::App, area: Rect) {
    if app.calls.is_empty() {
        let w = Paragraph::new("  (no active calls)")
            .style(Style::default().fg(app.theme.subtle.get()))
            .block(Block::default().title(Span::styled(
                "CALLS",
                Style::default().fg(app.theme.subtle.get()),
            )));
        f.render_widget(w, area);
        return;
    }

    let items: Vec<ListItem> = app
        .calls
        .iter()
        .enumerate()
        .map(|(i, call)| {
            let arrow = match call.direction {
                CallDirection::Outgoing => "↗",
                CallDirection::Incoming => "↙",
            };
            let dir = match call.direction {
                CallDirection::Outgoing => "outgoing",
                CallDirection::Incoming => "incoming",
            };

            let selected = i == app.selected_call;
            let on_hold = call.state == CallState::OnHold;

            let base_style = if selected {
                Style::default()
                    .fg(app.theme.attention.get())
                    .add_modifier(Modifier::BOLD)
            } else if on_hold {
                Style::default().fg(app.theme.subtle.get())
            } else {
                Style::default()
            };

            let (state_str, state_style) = match &call.state {
                CallState::Ringing => (
                    "RINGING".to_string(),
                    Style::default()
                        .fg(app.theme.attention.get())
                        .add_modifier(Modifier::BOLD),
                ),
                CallState::OnHold => (
                    "ON HOLD".to_string(),
                    Style::default()
                        .fg(app.theme.subtle.get())
                        .add_modifier(Modifier::DIM),
                ),
                CallState::Established => {
                    let s = call.started_at.map(|t| t.elapsed().as_secs()).unwrap_or(0);
                    (
                        format!("{:02}:{:02}:{:02}", s / 3600, (s % 3600) / 60, s % 60),
                        Style::default().fg(app.theme.success.get()),
                    )
                }
            };

            let marker = if selected { "►" } else { " " };

            let arrow_style = if selected || on_hold {
                base_style
            } else if call.direction == CallDirection::Outgoing {
                Style::default().fg(app.theme.accent.get())
            } else {
                Style::default().fg(app.theme.success.get())
            };

            let line = Line::from(vec![
                Span::styled(format!(" {} [{}] ", marker, i + 1), base_style),
                Span::styled(format!("{} ", arrow), arrow_style),
                Span::styled(format!("{:<8}  {:<40}  ", dir, call.peer), base_style),
                Span::styled(state_str, state_style),
            ]);

            ListItem::new(line)
        })
        .collect();

    let list = List::new(items).block(Block::default().title(Span::styled(
        "CALLS",
        Style::default().fg(app.theme.subtle.get()),
    )));
    f.render_widget(list, area);
}
