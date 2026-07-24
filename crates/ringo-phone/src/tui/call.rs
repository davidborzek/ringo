use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, List, ListItem, Paragraph},
};

use super::app::{Call, CallDirection, CallState};

impl super::app::App {
    pub(super) fn handle_call_incoming(
        &mut self,
        call_id: String,
        number: String,
        display_name: Option<String>,
    ) {
        self.deflected = None;
        self.last_call_reason = None;
        let notify_text = match &display_name {
            Some(name) => format!("{} ({})", name, number),
            None => number.clone(),
        };
        self.notify("Incoming call", &notify_text);

        crate::hooks::run(
            &self.hooks,
            crate::config::HookEvent::CallIncoming,
            &self.profile_name,
            &self.profile,
            serde_json::json!({
                "call_id": call_id,
                "number": number,
                "display_name": display_name.as_deref().unwrap_or(""),
            }),
        );

        self.calls.push(Call {
            id: call_id,
            peer: number,
            peer_display_name: display_name,
            direction: CallDirection::Incoming,
            state: CallState::Ringing,
            started_at: None,
        });
    }

    pub(super) fn handle_call_outgoing(&mut self, call_id: String, number: String) {
        self.last_call_reason = None;
        self.deflected = None;

        crate::hooks::run(
            &self.hooks,
            crate::config::HookEvent::CallOutgoing,
            &self.profile_name,
            &self.profile,
            serde_json::json!({
                "call_id": call_id,
                "number": number,
            }),
        );

        self.calls.push(Call {
            id: call_id,
            peer: number,
            peer_display_name: None,
            direction: CallDirection::Outgoing,
            state: CallState::Ringing,
            started_at: None,
        });
        // Make the new outgoing call the active/selected one, keeping
        // selected_call in sync with baresip's current (tail) call — hold(),
        // resume() and the media view all target that call.
        self.selected_call = self.calls.len() - 1;
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

    pub(super) fn handle_call_closed(&mut self, call_id: String, reason: String, error: bool) {
        // A completed attended transfer tears down BOTH legs. Detect it up front
        // (before `reason` may be moved below) so the surviving leg isn't given a
        // spurious resume re-INVITE that races the incoming BYE. baresip spells
        // it "Call transfered" (optionally prefixed with the SIP status code).
        let completed_transfer = reason.to_lowercase().contains("transfer");
        let mut closed: Option<super::app::LastCall> = None;
        if let Some(call) = self.calls.iter().find(|c| c.id == call_id) {
            if call.direction == CallDirection::Incoming && call.started_at.is_none() {
                self.notify("Missed call", &call.peer.clone());
            }
            let direction = match call.direction {
                CallDirection::Outgoing => "outgoing",
                CallDirection::Incoming => "incoming",
            };
            let duration_secs = call.started_at.map(|t| t.elapsed().as_secs()).unwrap_or(0);
            closed = Some(super::app::LastCall {
                peer: call.peer.clone(),
                direction: direction.to_string(),
                reason: reason.clone(),
                error,
                duration_secs,
                answered: call.started_at.is_some(),
            });
            crate::hooks::run(
                &self.hooks,
                crate::config::HookEvent::CallEnded,
                &self.profile_name,
                &self.profile,
                serde_json::json!({
                    "call_id": call_id,
                    "number": call.peer,
                    "direction": direction,
                    "duration_secs": duration_secs,
                    "reason": reason,
                    "error": error,
                }),
            );
            self.append_call_history(call);
        }
        if let Some(lc) = closed {
            self.last_call = Some(lc);
        }
        if error {
            self.last_call_reason = Some(reason);
        }
        self.muted = false;
        self.dial.dtmf.clear();
        self.calls.retain(|c| c.id != call_id);
        if self.selected_call >= self.calls.len() && !self.calls.is_empty() {
            self.selected_call = self.calls.len() - 1;
        }
        // The consultation leg of an attended transfer is gone — leave the
        // pending state so keys route normally again. (A completed transfer
        // already reset transfer_mode to None when 'X' was pressed.)
        if self.transfer_mode == super::app::TransferMode::AttendedPending && self.calls.len() < 2 {
            self.transfer_mode = super::app::TransferMode::None;
        }
        // Auto-resume: closing the active (non-held) call promotes the call that
        // becomes the new current one. If it's on hold, resume it — ringo doesn't
        // load baresip's `menu` module, so nothing does this automatically. The
        // selected_call fixup above already points at the new current call; we
        // make it baresip's current (by id) and resume it. Works for any call
        // count. When the closed call was itself on hold, the active call keeps
        // the selection and isn't on hold, so nothing is resumed. Skipped for a
        // completed transfer, whose other leg is being torn down too.
        if !completed_transfer
            && self
                .calls
                .get(self.selected_call)
                .is_some_and(|c| c.state == CallState::OnHold)
        {
            let id = self.calls[self.selected_call].id.clone();
            self.phone.select_call(&id);
            self.phone.resume();
            self.calls[self.selected_call].state = CallState::Established;
            // stale: re-polled for the newly-active call next tick
            self.media = None;
            self.codec = None;
        }
    }

    pub(super) fn handle_call_deflected(
        &mut self,
        from: String,
        display_name: Option<String>,
        target: String,
    ) {
        let peer = display_name
            .as_ref()
            .map(|n| format!("{n} ({from})"))
            .unwrap_or_else(|| from.clone());
        self.notify("Call deflected", &format!("{peer} → {target}"));
        self.deflected = Some(super::app::DeflectedInfo {
            from,
            display_name,
            target,
            at: std::time::Instant::now(),
        });
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

    /// Switch to the next call line. With `auto_hold` on (default) the current
    /// call is held and the next resumed; with it off, switching only changes
    /// focus and leaves both calls as they are.
    pub(super) fn switch_line(&mut self) {
        if self.calls.len() < 2 {
            return;
        }
        let cur = self.selected_call;
        // Hold the current call — make it baresip's current first, then hold()
        // targets it.
        if self.profile.auto_hold
            && self
                .calls
                .get(cur)
                .is_some_and(|c| c.state == CallState::Established)
        {
            let id = self.calls[cur].id.clone();
            self.phone.select_call(&id);
            self.phone.hold();
            self.calls[cur].state = CallState::OnHold;
        }
        let next = (cur + 1) % self.calls.len();
        self.selected_call = next;
        // stale: re-polled for the newly-active call next tick
        self.media = None;
        self.codec = None;
        // Select the next call by its SIP call-id — robust to baresip reusing
        // line numbers after a call is removed (the old index+1 mapping wasn't).
        let next_id = self.calls[next].id.clone();
        self.phone.select_call(&next_id);
        if self.profile.auto_hold && self.calls[next].state == CallState::OnHold {
            self.phone.resume();
            self.calls[next].state = CallState::Established;
        }
    }

    /// Hang up the selected call (not necessarily baresip's current call) by
    /// making it current first, then hanging up. Falls back to the current call
    /// if the selection is somehow out of range.
    pub(super) fn hangup_selected(&mut self) {
        if let Some(c) = self.calls.get(self.selected_call) {
            let id = c.id.clone();
            self.phone.select_call(&id);
        }
        self.phone.hangup();
    }
}

pub(super) fn render_calls(f: &mut Frame, app: &super::app::App, area: Rect) {
    let mut items: Vec<ListItem> = app
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

            let mut spans = vec![
                Span::styled(format!(" {} [{}] ", marker, i + 1), base_style),
                Span::styled(format!("{} ", arrow), arrow_style),
                Span::styled(
                    format!(
                        "{:<8}  {:<40}  ",
                        dir,
                        format_peer(
                            &call.peer,
                            call.peer_display_name.as_deref(),
                            crate::contacts::resolve_name(&app.contacts, &call.peer),
                        )
                    ),
                    base_style,
                ),
            ];

            // Live quality for the active (selected) call only — baresip reports
            // media stats for its current call, which `switch_line` keeps in sync
            // with `selected_call`.
            let quality = if selected && call.state == CallState::Established {
                app.media.as_ref()
            } else {
                None
            };
            spans.push(Span::styled(state_str, state_style));

            let mut lines = vec![Line::from(spans)];
            // Expand the focused/active call with a metrics row: MOS score plus
            // the jitter / loss / rtt / codec detail, all in one quiet line.
            if let Some(m) = quality {
                let mut detail = format!(
                    "MOS {:.1}  ·  jitter {:.0}ms · loss {:.1}% · rtt {:.0}ms",
                    m.mos, m.jitter_ms, m.packet_loss_pct, m.rtt_ms
                );
                if let Some(c) = &app.codec {
                    detail.push_str(&format!(" · {} {}kHz", c.name, c.srate / 1000));
                }
                lines.push(Line::from(Span::styled(
                    format!("        {detail}"),
                    Style::default().fg(app.theme.subtle.get()),
                )));
            }

            ListItem::new(lines)
        })
        .collect();

    // Transient deflected-call entry (302). Shown alongside active calls or
    // alone when the call list is empty; auto-cleared after 10 s by the
    // render loop.
    if let Some(d) = &app.deflected {
        let caller = d
            .display_name
            .as_deref()
            .map(|n| format!("{n} ({})", d.from))
            .unwrap_or_else(|| d.from.clone());
        let style = Style::default().fg(app.theme.accent.get());
        items.push(ListItem::new(vec![Line::from(vec![
            Span::styled("   ↪ ", style),
            Span::styled(format!("deflected  {caller}  → {}", d.target), style),
        ])]));
    }

    if items.is_empty() {
        let w = Paragraph::new("  (no active calls)")
            .style(Style::default().fg(app.theme.subtle.get()))
            .block(Block::default().title(Span::styled(
                "CALLS",
                Style::default().fg(app.theme.subtle.get()),
            )));
        f.render_widget(w, area);
        return;
    }

    let list = List::new(items).block(Block::default().title(Span::styled(
        "CALLS",
        Style::default().fg(app.theme.subtle.get()),
    )));
    f.render_widget(list, area);
}

/// Format peer for display.
/// Priority: contact_name > display_name > raw peer.
/// When contact_name is set, display_name is shown in brackets if different.
fn format_peer(peer: &str, display_name: Option<&str>, contact_name: Option<&str>) -> String {
    match (contact_name, display_name) {
        (Some(cn), Some(dn)) => format!("{} [{}] ({})", cn, dn, peer),
        (Some(cn), None) => format!("{} ({})", cn, peer),
        (None, Some(dn)) => format!("{} ({})", dn, peer),
        (None, None) => peer.to_string(),
    }
}
