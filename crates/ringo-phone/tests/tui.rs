use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ringo::config::Theme;
use ringo::phone::MockPhone;
use ringo::tui::{App, AppEvent, CallDirection, CallState, InputMode, RegStatus, TransferMode};
use serde_json::Value;

// ─── Helpers ──────────────────────────────────────────────────────────────────

fn test_app() -> (App, tokio::sync::mpsc::Receiver<(String, String)>) {
    let (cmd_tx, cmd_rx) = tokio::sync::mpsc::channel(16);
    let app = App::new(
        "test".into(),
        "sip:user@example.com".into(),
        None,
        None,
        false,
        Box::new(MockPhone::new(cmd_tx)),
        Theme::default(),
        Vec::new(),
        ringo::profile::Profile::default(),
        Vec::new(),
        Vec::new(),
    );
    (app, cmd_rx)
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
    app.handle_message(AppEvent::RegisterOk {
        account: "sip:user@example.com".into(),
    });
    assert_eq!(app.reg_status, RegStatus::Ok);
    assert_eq!(app.account_aor, "sip:user@example.com");
}

#[test]
fn register_fail_sets_failed_status() {
    let (mut app, _) = test_app();
    app.handle_message(AppEvent::RegisterFailed {
        reason: "401 Unauthorized".into(),
    });
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
    app.handle_message(AppEvent::CallIncoming {
        call_id: "1".into(),
        number: "sip:alice@example.com".into(),
        display_name: None,
    });
    assert_eq!(app.calls.len(), 1);
    assert_eq!(app.calls[0].direction, CallDirection::Incoming);
    assert_eq!(app.calls[0].state, CallState::Ringing);
    assert_eq!(app.calls[0].peer, "sip:alice@example.com");
}

/// Drain the mock's command channel into a list of command names.
fn commands(rx: &mut tokio::sync::mpsc::Receiver<(String, String)>) -> Vec<String> {
    let mut out = Vec::new();
    while let Ok((cmd, _)) = rx.try_recv() {
        out.push(cmd);
    }
    out
}

fn ring(app: &mut App, id: &str) {
    app.handle_message(AppEvent::CallIncoming {
        call_id: id.into(),
        number: format!("sip:{id}@example.com"),
        display_name: None,
    });
}

fn established(app: &mut App, id: &str) {
    app.handle_message(AppEvent::CallOutgoing {
        call_id: id.into(),
        number: format!("sip:{id}@example.com"),
    });
    app.handle_message(AppEvent::CallEstablished { call_id: id.into() });
}

/// The hint bar's key column, for asserting on what it offers.
fn hint_keys(app: &App) -> Vec<String> {
    app.hints()
        .into_iter()
        .map(|(k, _)| k.to_string())
        .collect()
}

#[test]
fn every_command_is_offered_by_completion() {
    // dispatch and COMMANDS are two lists that have to agree, and the entries
    // added last are exactly the ones that get forgotten in the second one.
    for cmd in ["register", "unregister", "deafen", "silence"] {
        let (mut app, _rx) = test_app();
        app.command.input = cmd[..3].to_string();
        app.command.tab_prefix = None;
        app.command.tab_index = 0;

        // A single match completes with a trailing space, several cycle.
        let mut seen = Vec::new();
        for _ in 0..24 {
            app.cycle_completion();
            seen.push(app.command.input.trim().to_string());
        }
        assert!(
            seen.iter().any(|c| c == cmd),
            "'{cmd}' is dispatchable but Tab never offers it; got {seen:?}"
        );
    }
}

#[test]
fn unregister_signs_off_without_looking_like_a_failure() {
    // The registration is gone either way, but one of them is what the user
    // asked for and must not be dressed up as an error.
    let (mut app, mut rx) = test_app();
    app.handle_message(AppEvent::RegisterOk {
        account: "sip:user@example.com".into(),
    });

    assert!(app.dispatch("unregister", "").is_ok());
    assert_eq!(commands(&mut rx), vec!["unregister"]);

    // baresip answers with the event; the status follows from that, not from
    // the command optimistically setting it.
    app.handle_message(AppEvent::Unregistered {
        account: "sip:user@example.com".into(),
    });
    assert_eq!(app.reg_status, RegStatus::Unregistered);
    assert!(!matches!(app.reg_status, RegStatus::Failed(_)));
}

#[test]
fn register_signs_back_on() {
    let (mut app, mut rx) = test_app();
    app.handle_message(AppEvent::Unregistered {
        account: "sip:user@example.com".into(),
    });

    assert!(app.dispatch("register", "").is_ok());
    assert_eq!(commands(&mut rx), vec!["uareg"]);
    assert_eq!(app.reg_status, RegStatus::Registering);

    app.handle_message(AppEvent::RegisterOk {
        account: "sip:user@example.com".into(),
    });
    assert_eq!(app.reg_status, RegStatus::Ok);
}

#[test]
fn leaving_a_transfer_returns_to_normal_not_the_dial_field() {
    // Entering transfer mode never touches dial.mode, so leaving it must not
    // either — landing in the dial field is a mode the user never asked for.
    for leave in [esc(), enter()] {
        let (mut app, _rx) = test_app();
        established(&mut app, "1");

        app.handle_key(key('t'));
        app.handle_key(key('9'));
        app.handle_key(leave);

        assert!(matches!(app.transfer_mode, TransferMode::None));
        assert_eq!(app.dial.mode, InputMode::Normal, "after {leave:?}");
    }
}

#[test]
fn leaving_an_attended_transfer_returns_to_normal_too() {
    let (mut app, _rx) = test_app();
    established(&mut app, "1");

    app.handle_key(shift_key('T'));
    app.handle_key(key('9'));
    app.handle_key(esc());

    assert_eq!(app.dial.mode, InputMode::Normal);
}

#[test]
fn h_toggles_hold_both_ways() {
    let (mut app, mut rx) = test_app();
    established(&mut app, "1");
    let _ = commands(&mut rx);

    app.handle_key(key('h'));
    assert_eq!(app.calls[0].state, CallState::OnHold);
    assert_eq!(commands(&mut rx), vec!["hold"]);

    app.handle_key(key('h'));
    assert_eq!(app.calls[0].state, CallState::Established);
    assert_eq!(commands(&mut rx), vec!["resume"]);
}

#[test]
fn the_hold_hint_says_which_way_it_goes() {
    let (mut app, _rx) = test_app();
    established(&mut app, "1");
    assert!(app.hints().contains(&("h", "hold")));

    app.handle_key(key('h'));
    assert!(app.hints().contains(&("h", "resume")));
}

#[test]
fn the_hint_bar_makes_room_during_a_call() {
    // Idle it lists where you can go; in a call it lists what you can do. The
    // global keys keep working either way — they are in `?`.
    let (mut app, _rx) = test_app();
    let idle = hint_keys(&app);
    assert!(idle.contains(&"d".to_string()), "idle offers dialling");
    assert!(idle.contains(&"q".to_string()), "idle offers quit");

    established(&mut app, "1");
    let busy = hint_keys(&app);
    assert!(busy.contains(&"b".to_string()), "hangup is the first thing");
    assert!(
        busy.contains(&"m/M".to_string()),
        "mute and deafen share a hint"
    );
    assert!(busy.contains(&"h".to_string()));
    assert!(
        !busy.contains(&"q".to_string()),
        "quit is not a mid-call action"
    );
    assert!(!busy.contains(&"c".to_string()), "history is not either");
    assert!(busy.len() <= 6, "too crowded: {busy:?}");
    assert_eq!(busy.last().unwrap(), "?", "the escape hatch comes last");
}

#[test]
fn a_ringing_call_leads_with_accept() {
    let (mut app, _rx) = test_app();
    ring(&mut app, "1");
    let keys = hint_keys(&app);
    assert_eq!(keys.first().unwrap(), "a");
    assert!(
        keys.contains(&"s".to_string()),
        "silence is offered while ringing"
    );

    app.handle_key(key('s'));
    assert!(
        !hint_keys(&app).contains(&"s".to_string()),
        "a silenced ring stops advertising the key that silences it"
    );
}

#[test]
fn switching_is_only_offered_with_more_than_one_call() {
    let (mut app, _rx) = test_app();
    established(&mut app, "1");
    assert!(!hint_keys(&app).contains(&"Tab".to_string()));

    ring(&mut app, "2");
    assert!(hint_keys(&app).contains(&"Tab".to_string()));
}

#[test]
fn deafening_takes_the_microphone_with_it() {
    // Hearing nothing while still being heard is a trap.
    let (mut app, mut rx) = test_app();
    established(&mut app, "1");
    let _ = commands(&mut rx);

    app.handle_key(shift_key('M'));
    assert!(app.deafened);
    assert!(app.muted, "deafening must mute the microphone too");
    assert_eq!(commands(&mut rx), vec!["speaker", "mute"]);
}

#[test]
fn undeafening_restores_a_microphone_that_was_already_muted() {
    // Mute by hand, deafen, undeafen — you stay muted.
    let (mut app, _rx) = test_app();
    established(&mut app, "1");
    app.handle_key(key('m'));
    assert!(app.muted);

    app.handle_key(shift_key('M'));
    app.handle_key(shift_key('M'));

    assert!(!app.deafened);
    assert!(app.muted, "the manual mute must survive the round trip");
}

#[test]
fn undeafening_unmutes_a_microphone_that_was_not() {
    let (mut app, _rx) = test_app();
    established(&mut app, "1");

    app.handle_key(shift_key('M'));
    app.handle_key(shift_key('M'));

    assert!(!app.deafened);
    assert!(!app.muted);
}

#[test]
fn deafening_needs_an_active_call() {
    let (mut app, mut rx) = test_app();
    app.handle_key(shift_key('M'));
    assert!(!app.deafened);
    assert!(commands(&mut rx).is_empty());
}

#[test]
fn hanging_up_clears_deafening() {
    // Both states live on the call's audio object and die with it.
    let (mut app, _rx) = test_app();
    established(&mut app, "1");
    app.handle_key(shift_key('M'));

    app.handle_message(AppEvent::CallClosed {
        call_id: "1".into(),
        reason: "Connection closed".into(),
        error: false,
    });
    assert!(!app.deafened);
    assert!(!app.muted);
}

#[test]
fn silencing_stops_the_ring_but_not_the_call() {
    // The whole point: your side goes quiet, the caller keeps hearing ringback,
    // and the call is still there to answer.
    let (mut app, mut rx) = test_app();
    ring(&mut app, "1");
    let _ = commands(&mut rx);

    app.handle_key(key('s'));

    assert_eq!(commands(&mut rx), vec!["silence"]);
    assert!(app.ring_silenced, "the UI must be able to show it happened");
    assert_eq!(app.calls.len(), 1, "the call survives");
    assert_eq!(app.calls[0].state, CallState::Ringing);
}

#[test]
fn a_silenced_call_can_still_be_answered() {
    let (mut app, mut rx) = test_app();
    ring(&mut app, "1");
    app.handle_key(key('s'));
    let _ = commands(&mut rx);

    app.handle_key(key('a'));
    assert_eq!(commands(&mut rx), vec!["accept"]);
}

#[test]
fn silencing_lasts_only_until_the_next_call() {
    let (mut app, _rx) = test_app();
    ring(&mut app, "1");
    app.handle_key(key('s'));
    assert!(app.ring_silenced);

    app.handle_message(AppEvent::CallClosed {
        call_id: "1".into(),
        reason: "Rejected by user".into(),
        error: false,
    });
    ring(&mut app, "2");
    assert!(!app.ring_silenced, "a new call rings again");
}

#[test]
fn silencing_does_nothing_when_nothing_rings() {
    let (mut app, mut rx) = test_app();
    app.handle_key(key('s'));
    assert!(commands(&mut rx).is_empty());
    assert!(!app.ring_silenced);
}

#[test]
fn the_silence_command_reports_when_nothing_rings() {
    let (mut app, _rx) = test_app();
    assert!(app.dispatch("silence", "").is_err());
    ring(&mut app, "1");
    assert!(app.dispatch("silence", "").is_ok());
}

#[test]
fn call_outgoing_adds_ringing_call() {
    let (mut app, _) = test_app();
    app.handle_message(AppEvent::CallOutgoing {
        call_id: "2".into(),
        number: "sip:bob@example.com".into(),
    });
    assert_eq!(app.calls.len(), 1);
    assert_eq!(app.calls[0].direction, CallDirection::Outgoing);
    assert_eq!(app.calls[0].state, CallState::Ringing);
}

#[test]
fn call_outgoing_during_attended_pending_selects_new_call() {
    let (mut app, _) = test_app();
    app.handle_message(AppEvent::CallIncoming {
        call_id: "1".into(),
        number: "sip:a@b".into(),
        display_name: None,
    });
    app.handle_message(AppEvent::CallEstablished {
        call_id: "1".into(),
    });
    app.transfer_mode = TransferMode::AttendedPending;
    app.handle_message(AppEvent::CallOutgoing {
        call_id: "2".into(),
        number: "sip:c@d".into(),
    });
    assert_eq!(app.calls.len(), 2);
    assert_eq!(app.selected_call, 1);
}

#[test]
fn call_established_sets_state_and_started_at() {
    let (mut app, _) = test_app();
    app.handle_message(AppEvent::CallIncoming {
        call_id: "1".into(),
        number: "sip:a@b".into(),
        display_name: None,
    });
    app.handle_message(AppEvent::CallEstablished {
        call_id: "1".into(),
    });
    assert_eq!(app.calls[0].state, CallState::Established);
    assert!(app.calls[0].started_at.is_some());
}

#[test]
fn call_closed_removes_call() {
    let (mut app, _) = test_app();
    app.handle_message(AppEvent::CallIncoming {
        call_id: "1".into(),
        number: "sip:a@b".into(),
        display_name: None,
    });
    app.handle_message(AppEvent::CallEstablished {
        call_id: "1".into(),
    });
    app.handle_message(AppEvent::CallClosed {
        call_id: "1".into(),
        reason: "".into(),
        error: false,
    });
    assert_eq!(app.calls.len(), 0);
}

#[test]
fn call_closed_missed_incoming_removes_call() {
    let (mut app, _) = test_app();
    app.handle_message(AppEvent::CallIncoming {
        call_id: "1".into(),
        number: "sip:a@b".into(),
        display_name: None,
    });
    assert_eq!(app.calls[0].started_at, None);
    app.handle_message(AppEvent::CallClosed {
        call_id: "1".into(),
        reason: "".into(),
        error: false,
    });
    assert_eq!(app.calls.len(), 0);
}

// ─── MWI ──────────────────────────────────────────────────────────────────────

#[test]
fn mwi_notify_messages_waiting_yes() {
    let (mut app, _) = test_app();
    app.handle_message(AppEvent::VoicemailStatus {
        waiting: true,
        new_count: 3,
    });
    assert!(app.mwi.waiting);
    assert_eq!(app.mwi.new_messages, 3);
}

#[test]
fn mwi_notify_messages_waiting_no() {
    let (mut app, _) = test_app();
    app.mwi.waiting = true;
    app.mwi.new_messages = 5;
    app.handle_message(AppEvent::VoicemailStatus {
        waiting: false,
        new_count: 0,
    });
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
    app.handle_message(AppEvent::CallIncoming {
        call_id: "1".into(),
        number: "sip:a@b".into(),
        display_name: None,
    });
    app.handle_message(AppEvent::CallEstablished {
        call_id: "1".into(),
    });
    app.handle_key(key('t'));
    assert_eq!(app.transfer_mode, TransferMode::BlindInput(String::new()));
}

#[test]
fn blind_input_char_appends_to_buffer() {
    let (mut app, _) = test_app();
    app.handle_message(AppEvent::CallIncoming {
        call_id: "1".into(),
        number: "sip:a@b".into(),
        display_name: None,
    });
    app.handle_message(AppEvent::CallEstablished {
        call_id: "1".into(),
    });
    app.handle_key(key('t'));
    app.handle_key(key('5'));
    assert_eq!(app.transfer_mode, TransferMode::BlindInput("5".into()));
}

#[test]
fn blind_input_backspace_clears_last_char() {
    let (mut app, _) = test_app();
    app.handle_message(AppEvent::CallIncoming {
        call_id: "1".into(),
        number: "sip:a@b".into(),
        display_name: None,
    });
    app.handle_message(AppEvent::CallEstablished {
        call_id: "1".into(),
    });
    app.handle_key(key('t'));
    app.handle_key(key('5'));
    app.handle_key(backspace());
    assert_eq!(app.transfer_mode, TransferMode::BlindInput(String::new()));
}

#[test]
fn blind_input_esc_cancels_transfer() {
    let (mut app, _) = test_app();
    app.handle_message(AppEvent::CallIncoming {
        call_id: "1".into(),
        number: "sip:a@b".into(),
        display_name: None,
    });
    app.handle_message(AppEvent::CallEstablished {
        call_id: "1".into(),
    });
    app.handle_key(key('t'));
    app.handle_key(esc());
    assert_eq!(app.transfer_mode, TransferMode::None);
}

#[test]
fn blind_input_enter_sends_transfer_command() {
    let (mut app, mut cmd_rx) = test_app();
    app.handle_message(AppEvent::CallIncoming {
        call_id: "1".into(),
        number: "sip:a@b".into(),
        display_name: None,
    });
    app.handle_message(AppEvent::CallEstablished {
        call_id: "1".into(),
    });
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
    app.handle_message(AppEvent::CallIncoming {
        call_id: "1".into(),
        number: "sip:a@b".into(),
        display_name: None,
    });
    app.handle_message(AppEvent::CallEstablished {
        call_id: "1".into(),
    });
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

// ─── DTMF dispatch ───────────────────────────────────────────────────────────

#[test]
fn dtmf_sends_sndcode_per_digit_during_call() {
    let (mut app, mut cmd_rx) = test_app();
    app.handle_message(AppEvent::CallIncoming {
        call_id: "1".into(),
        number: "sip:a@b".into(),
        display_name: None,
    });
    app.handle_message(AppEvent::CallEstablished {
        call_id: "1".into(),
    });

    let res = app.dispatch("dtmf", "1 2#");
    assert!(res.is_ok(), "{res:?}");

    let sent: Vec<String> = std::iter::from_fn(|| cmd_rx.try_recv().ok())
        .filter(|(cmd, _)| cmd == "sndcode")
        .map(|(_, params)| params)
        .collect();
    assert_eq!(sent, vec!["1", "2", "#"]);
}

#[test]
fn dtmf_without_active_call_errors() {
    let (mut app, _rx) = test_app();
    assert_eq!(app.dispatch("dtmf", "123"), Err("No active call".into()));
}

#[test]
fn dtmf_rejects_invalid_digit() {
    let (mut app, _rx) = test_app();
    app.handle_message(AppEvent::CallIncoming {
        call_id: "1".into(),
        number: "sip:a@b".into(),
        display_name: None,
    });
    app.handle_message(AppEvent::CallEstablished {
        call_id: "1".into(),
    });
    let err = app.dispatch("dtmf", "12x").unwrap_err();
    assert!(err.contains("Invalid DTMF"), "{err}");
}

// ─── status JSON ─────────────────────────────────────────────────────────────

#[test]
fn status_returns_structured_json() {
    let (mut app, _rx) = test_app();
    app.handle_message(AppEvent::RegisterOk {
        account: "sip:user@example.com".into(),
    });
    app.handle_message(AppEvent::CallIncoming {
        call_id: "1".into(),
        number: "sip:a@b".into(),
        display_name: None,
    });
    app.handle_message(AppEvent::CallEstablished {
        call_id: "1".into(),
    });

    let out = app.dispatch("status", "").unwrap();
    let v: Value = serde_json::from_str(&out).expect("status must be valid JSON");

    assert_eq!(v["registration"], "registered");
    assert_eq!(v["muted"], false);
    assert!(v["last_call"].is_null(), "no call closed yet");
    let calls = v["calls"].as_array().unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0]["state"], "established");
    assert_eq!(calls[0]["peer"], "sip:a@b");
}

#[test]
fn status_exposes_last_call_after_close() {
    let (mut app, _rx) = test_app();
    app.handle_message(AppEvent::CallOutgoing {
        call_id: "1".into(),
        number: "sip:bob@b".into(),
    });
    app.handle_message(AppEvent::CallEstablished {
        call_id: "1".into(),
    });
    app.handle_message(AppEvent::CallClosed {
        call_id: "1".into(),
        reason: "486 Busy Here".into(),
        error: true,
    });

    let v: Value = serde_json::from_str(&app.dispatch("status", "").unwrap()).unwrap();
    assert_eq!(v["calls"].as_array().unwrap().len(), 0);
    let lc = &v["last_call"];
    assert_eq!(lc["peer"], "sip:bob@b");
    assert_eq!(lc["direction"], "outgoing");
    assert_eq!(lc["reason"], "486 Busy Here");
    assert_eq!(lc["error"], true);
    assert_eq!(lc["answered"], true);
}

#[test]
fn shutdown_hangs_up_and_sets_quit() {
    let (mut app, mut cmd_rx) = test_app();
    app.handle_message(AppEvent::CallIncoming {
        call_id: "1".into(),
        number: "sip:a@b".into(),
        display_name: None,
    });
    app.handle_message(AppEvent::CallEstablished {
        call_id: "1".into(),
    });

    assert!(app.dispatch("shutdown", "").is_ok());
    assert!(app.quit);

    let cmds: Vec<String> = std::iter::from_fn(|| cmd_rx.try_recv().ok())
        .map(|(c, _)| c)
        .collect();
    assert!(cmds.contains(&"hangupall".to_string()));
}
