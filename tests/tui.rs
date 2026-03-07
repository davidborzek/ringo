use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ringo::client::BaresipMessage;
use ringo::config::Theme;
use ringo::phone::BaresipPhone;
use ringo::tui::{App, AppEvent, CallDirection, CallState, RegStatus, TransferMode};
use serde_json::{Value, json};

// ─── Helpers ──────────────────────────────────────────────────────────────────

fn test_app() -> (App, tokio::sync::mpsc::Receiver<(String, String)>) {
    let (cmd_tx, cmd_rx) = tokio::sync::mpsc::channel(16);
    let app = App::new(
        "test".into(),
        "sip:user@example.com".into(),
        None,
        None,
        false,
        Box::new(BaresipPhone::new(cmd_tx)),
        Theme::default(),
    );
    (app, cmd_rx)
}

/// Build an AppEvent from a raw baresip event (exercises the From conversion).
fn evt(type_: &str, param: &str, extra: Value) -> AppEvent {
    AppEvent::from(BaresipMessage::Event {
        class: "call".into(),
        type_: type_.into(),
        param: param.into(),
        extra: extra.as_object().cloned().unwrap_or_default(),
    })
}

fn key(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE)
}

fn shift_key(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c), KeyModifiers::SHIFT)
}

fn backspace() -> KeyEvent {
    KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE)
}

fn enter() -> KeyEvent {
    KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)
}

fn esc() -> KeyEvent {
    KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)
}

// ─── Registration ─────────────────────────────────────────────────────────────

#[test]
fn register_ok_sets_status_and_aor() {
    let (mut app, _) = test_app();
    app.handle_message(evt(
        "REGISTER_OK",
        "",
        json!({"accountaor": "sip:user@example.com"}),
    ));
    assert_eq!(app.reg_status, RegStatus::Ok);
    assert_eq!(app.account_aor, "sip:user@example.com");
}

#[test]
fn register_fail_sets_failed_status() {
    let (mut app, _) = test_app();
    app.handle_message(evt("REGISTER_FAIL", "401 Unauthorized", json!({})));
    assert!(matches!(app.reg_status, RegStatus::Failed(_)));
}

#[test]
fn register_ok_event_updates_status() {
    let (mut app, _) = test_app();
    app.handle_message(AppEvent::RegisterOk {
        account: "sip:user@example.com".into(),
    });
    assert_eq!(app.reg_status, RegStatus::Ok);
    assert_eq!(app.account_aor, "sip:user@example.com");
}

// ─── Calls ────────────────────────────────────────────────────────────────────

#[test]
fn call_incoming_adds_ringing_call() {
    let (mut app, _) = test_app();
    app.handle_message(evt(
        "CALL_INCOMING",
        "",
        json!({"id": "1", "peeruri": "sip:alice@example.com"}),
    ));
    assert_eq!(app.calls.len(), 1);
    assert_eq!(app.calls[0].direction, CallDirection::Incoming);
    assert_eq!(app.calls[0].state, CallState::Ringing);
    assert_eq!(app.calls[0].peer, "sip:alice@example.com");
}

#[test]
fn call_outgoing_adds_ringing_call() {
    let (mut app, _) = test_app();
    app.handle_message(evt(
        "CALL_OUTGOING",
        "",
        json!({"id": "2", "peeruri": "sip:bob@example.com"}),
    ));
    assert_eq!(app.calls.len(), 1);
    assert_eq!(app.calls[0].direction, CallDirection::Outgoing);
    assert_eq!(app.calls[0].state, CallState::Ringing);
}

#[test]
fn call_outgoing_during_attended_pending_selects_new_call() {
    let (mut app, _) = test_app();
    // Establish first call
    app.handle_message(evt(
        "CALL_INCOMING",
        "",
        json!({"id": "1", "peeruri": "sip:a@b"}),
    ));
    app.handle_message(evt("CALL_ESTABLISHED", "", json!({"id": "1"})));
    // Manually set AttendedPending (as if atransferstart was sent)
    app.transfer_mode = TransferMode::AttendedPending;
    // New outgoing call arrives (Line B)
    app.handle_message(evt(
        "CALL_OUTGOING",
        "",
        json!({"id": "2", "peeruri": "sip:c@d"}),
    ));
    assert_eq!(app.calls.len(), 2);
    assert_eq!(app.selected_call, 1); // auto-selected to new call
}

#[test]
fn call_established_sets_state_and_started_at() {
    let (mut app, _) = test_app();
    app.handle_message(evt(
        "CALL_INCOMING",
        "",
        json!({"id": "1", "peeruri": "sip:a@b"}),
    ));
    app.handle_message(evt("CALL_ESTABLISHED", "", json!({"id": "1"})));
    assert_eq!(app.calls[0].state, CallState::Established);
    assert!(app.calls[0].started_at.is_some());
}

#[test]
fn call_closed_removes_call() {
    let (mut app, _) = test_app();
    app.handle_message(evt(
        "CALL_INCOMING",
        "",
        json!({"id": "1", "peeruri": "sip:a@b"}),
    ));
    app.handle_message(evt("CALL_ESTABLISHED", "", json!({"id": "1"})));
    app.handle_message(evt("CALL_CLOSED", "", json!({"id": "1"})));
    assert_eq!(app.calls.len(), 0);
}

#[test]
fn call_closed_missed_incoming_removes_call() {
    let (mut app, _) = test_app();
    // Incoming but never established → missed
    app.handle_message(evt(
        "CALL_INCOMING",
        "",
        json!({"id": "1", "peeruri": "sip:a@b"}),
    ));
    assert_eq!(app.calls[0].started_at, None);
    app.handle_message(evt("CALL_CLOSED", "", json!({"id": "1"})));
    assert_eq!(app.calls.len(), 0);
}

// ─── MWI ──────────────────────────────────────────────────────────────────────

#[test]
fn mwi_notify_messages_waiting_yes() {
    let (mut app, _) = test_app();
    app.handle_message(evt(
        "MWI_NOTIFY",
        "Messages-Waiting: yes\nVoice-Message: 3/10 (1/2)",
        json!({}),
    ));
    assert!(app.mwi.waiting);
    assert_eq!(app.mwi.new_messages, 3);
}

#[test]
fn mwi_notify_messages_waiting_no() {
    let (mut app, _) = test_app();
    app.mwi.waiting = true;
    app.mwi.new_messages = 5;
    app.handle_message(evt(
        "MWI_NOTIFY",
        "Messages-Waiting: no\nVoice-Message: 0/10 (0/2)",
        json!({}),
    ));
    assert!(!app.mwi.waiting);
}

// ─── Transfer Key Handling ────────────────────────────────────────────────────

#[test]
fn t_without_active_call_is_noop_in_normal_mode() {
    let (mut app, _) = test_app();
    app.handle_key(key('t'));
    assert_eq!(app.dial.input, "");
    assert_eq!(app.transfer_mode, TransferMode::None);
}

#[test]
fn t_with_active_call_enters_blind_input() {
    let (mut app, _) = test_app();
    app.handle_message(evt(
        "CALL_INCOMING",
        "",
        json!({"id": "1", "peeruri": "sip:a@b"}),
    ));
    app.handle_message(evt("CALL_ESTABLISHED", "", json!({"id": "1"})));
    app.handle_key(key('t'));
    assert_eq!(app.transfer_mode, TransferMode::BlindInput(String::new()));
}

#[test]
fn blind_input_char_appends_to_buffer() {
    let (mut app, _) = test_app();
    app.handle_message(evt(
        "CALL_INCOMING",
        "",
        json!({"id": "1", "peeruri": "sip:a@b"}),
    ));
    app.handle_message(evt("CALL_ESTABLISHED", "", json!({"id": "1"})));
    app.handle_key(key('t'));
    app.handle_key(key('5'));
    assert_eq!(app.transfer_mode, TransferMode::BlindInput("5".into()));
}

#[test]
fn blind_input_backspace_clears_last_char() {
    let (mut app, _) = test_app();
    app.handle_message(evt(
        "CALL_INCOMING",
        "",
        json!({"id": "1", "peeruri": "sip:a@b"}),
    ));
    app.handle_message(evt("CALL_ESTABLISHED", "", json!({"id": "1"})));
    app.handle_key(key('t'));
    app.handle_key(key('5'));
    app.handle_key(backspace());
    assert_eq!(app.transfer_mode, TransferMode::BlindInput(String::new()));
}

#[test]
fn blind_input_esc_cancels_transfer() {
    let (mut app, _) = test_app();
    app.handle_message(evt(
        "CALL_INCOMING",
        "",
        json!({"id": "1", "peeruri": "sip:a@b"}),
    ));
    app.handle_message(evt("CALL_ESTABLISHED", "", json!({"id": "1"})));
    app.handle_key(key('t'));
    app.handle_key(esc());
    assert_eq!(app.transfer_mode, TransferMode::None);
}

#[test]
fn blind_input_enter_sends_transfer_command() {
    let (mut app, mut cmd_rx) = test_app();
    app.handle_message(evt(
        "CALL_INCOMING",
        "",
        json!({"id": "1", "peeruri": "sip:a@b"}),
    ));
    app.handle_message(evt("CALL_ESTABLISHED", "", json!({"id": "1"})));
    app.handle_key(key('t'));
    app.handle_key(key('1'));
    app.handle_key(key('2'));
    app.handle_key(key('3'));
    app.handle_key(enter());
    assert_eq!(app.transfer_mode, TransferMode::None);
    let (cmd, params) = cmd_rx.try_recv().unwrap();
    assert_eq!(cmd, "transfer");
    assert_eq!(params, "sip:123@example.com");
}

#[test]
fn attended_transfer_enter_sets_attended_pending() {
    let (mut app, mut cmd_rx) = test_app();
    app.handle_message(evt(
        "CALL_INCOMING",
        "",
        json!({"id": "1", "peeruri": "sip:a@b"}),
    ));
    app.handle_message(evt("CALL_ESTABLISHED", "", json!({"id": "1"})));
    app.handle_key(shift_key('T'));
    app.handle_key(key('4'));
    app.handle_key(key('2'));
    app.handle_key(enter());
    assert_eq!(app.transfer_mode, TransferMode::AttendedPending);
    let (cmd, params) = cmd_rx.try_recv().unwrap();
    assert_eq!(cmd, "atransferstart");
    assert_eq!(params, "sip:42@example.com");
}

#[test]
fn attended_pending_x_executes_transfer() {
    let (mut app, mut cmd_rx) = test_app();
    app.transfer_mode = TransferMode::AttendedPending;
    app.handle_key(shift_key('X'));
    assert_eq!(app.transfer_mode, TransferMode::None);
    let (cmd, _) = cmd_rx.try_recv().unwrap();
    assert_eq!(cmd, "atransferexec");
}

#[test]
fn attended_pending_esc_aborts_transfer() {
    let (mut app, mut cmd_rx) = test_app();
    app.transfer_mode = TransferMode::AttendedPending;
    app.handle_key(esc());
    assert_eq!(app.transfer_mode, TransferMode::None);
    let (cmd, _) = cmd_rx.try_recv().unwrap();
    assert_eq!(cmd, "atransferabort");
}
