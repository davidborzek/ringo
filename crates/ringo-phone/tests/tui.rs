use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::{Terminal, backend::TestBackend, layout::Rect};
use ringo::config::Theme;
use ringo::mouse::ClickAction;
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

fn mouse_event(kind: MouseEventKind, column: u16, row: u16) -> MouseEvent {
    MouseEvent {
        kind,
        column,
        row,
        modifiers: KeyModifiers::NONE,
    }
}

/// A left click: press and release on one cell. Whether the release did anything.
fn left_click(app: &mut App, (col, row): (u16, u16)) -> bool {
    app.handle_mouse(mouse_event(
        MouseEventKind::Down(MouseButton::Left),
        col,
        row,
    ));
    app.handle_mouse(mouse_event(MouseEventKind::Up(MouseButton::Left), col, row))
}

#[test]
fn mouse_command_toggles_and_is_offered_by_completion() {
    let (mut app, _rx) = test_app();
    assert!(!app.mouse_enabled, "off until config or :mouse says so");

    assert_eq!(app.dispatch("mouse", ""), Ok("Mouse on".to_string()));
    assert!(app.mouse_enabled);
    assert_eq!(app.dispatch("mouse", ""), Ok("Mouse off".to_string()));
    assert!(!app.mouse_enabled);

    app.command.input = "mou".into();
    app.cycle_completion();
    assert_eq!(app.command.input.trim(), "mouse");
}

#[test]
fn mouse_events_are_ignored_while_mouse_is_off() {
    let (mut app, _rx) = test_app();
    app.hits.push(
        Rect::new(0, 0, 10, 1),
        ClickAction::Key(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE)),
    );

    let handled = left_click(&mut app, (1, 0));

    assert!(!handled);
    assert_eq!(app.dial.mode, InputMode::Normal);
}

#[test]
fn only_a_left_click_and_the_wheel_count_as_handled() {
    let (mut app, _rx) = test_app();
    app.mouse_enabled = true;
    app.hits.push(
        Rect::new(0, 0, 10, 1),
        ClickAction::Key(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE)),
    );

    for kind in [
        MouseEventKind::Moved,
        MouseEventKind::Drag(MouseButton::Left),
        MouseEventKind::Up(MouseButton::Left),
        MouseEventKind::Down(MouseButton::Right),
    ] {
        assert!(!app.handle_mouse(mouse_event(kind, 1, 0)), "{kind:?}");
    }
    assert_eq!(app.dial.mode, InputMode::Normal);

    assert!(!left_click(&mut app, (20, 5)));
    assert!(left_click(&mut app, (1, 0)));
    assert_eq!(
        app.dial.mode,
        InputMode::Dial,
        "a Key region is a key press"
    );
}

// ─── Mouse regions ────────────────────────────────────────────────────────────

fn render_app(app: &mut App) -> Terminal<TestBackend> {
    let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
    // The same steps as the render loop: highlight, then keep the frame.
    let completed = terminal
        .draw(|f| {
            ringo::tui::ui::render(f, app);
            app.selector.highlight(f.buffer_mut());
        })
        .unwrap();
    if app.mouse_enabled {
        app.selector.set_frame(completed.buffer.clone());
    }
    terminal
}

/// Where `text` starts on the rendered screen, as (col, row).
fn find_on_screen(terminal: &Terminal<TestBackend>, text: &str) -> (u16, u16) {
    let buf = terminal.backend().buffer();
    let width = buf.area.width;
    for row in 0..buf.area.height {
        let line: String = (0..width).map(|col| buf[(col, row)].symbol()).collect();
        // Borders and bullets are multi-byte, so count columns, not bytes.
        if let Some(byte) = line.find(text) {
            return (line[..byte].chars().count() as u16, row);
        }
    }
    panic!("{text:?} is not on the screen");
}

fn wheel(app: &mut App, (col, row): (u16, u16), kind: MouseEventKind) -> bool {
    app.handle_mouse(mouse_event(kind, col, row))
}

#[test]
fn clicking_the_reject_hint_of_a_ringing_call_rejects_it() {
    let (mut app, mut rx) = test_app();
    app.mouse_enabled = true;
    ring(&mut app, "1");
    let _ = commands(&mut rx);
    let terminal = render_app(&mut app);

    assert!(left_click(&mut app, find_on_screen(&terminal, "b reject")));

    assert_eq!(commands(&mut rx), vec!["reject"]);
}

#[test]
fn clicking_a_call_row_selects_that_call() {
    let (mut app, mut rx) = test_app();
    app.mouse_enabled = true;
    established(&mut app, "1");
    established(&mut app, "2");
    app.selected_call = 0;
    let _ = commands(&mut rx);
    let terminal = render_app(&mut app);

    assert!(left_click(&mut app, find_on_screen(&terminal, "[2]")));

    assert_eq!(app.selected_call, 1);
    assert_eq!(
        app.calls[0].state,
        CallState::OnHold,
        "same auto-hold as Tab"
    );
}

#[test]
fn the_wheel_scrolls_the_log_overlay() {
    let (mut app, _rx) = test_app();
    app.mouse_enabled = true;
    app.log.lines = (0..200).map(|i| format!("line {i}")).collect();
    app.handle_key(key('l'));
    app.log.lines = (0..200).map(|i| format!("line {i}")).collect();
    let terminal = render_app(&mut app);
    assert_eq!(app.log.scroll, 0);
    let inside = find_on_screen(&terminal, "line 19");

    assert!(wheel(&mut app, inside, MouseEventKind::ScrollUp));
    assert_eq!(app.log.scroll, 3);
    assert!(wheel(&mut app, inside, MouseEventKind::ScrollDown));
    assert_eq!(app.log.scroll, 0);
    assert!(
        !wheel(&mut app, (0, 0), MouseEventKind::ScrollUp),
        "outside the overlay"
    );
}

#[test]
fn clicking_quit_in_the_quit_confirm_quits() {
    let (mut app, mut rx) = test_app();
    app.mouse_enabled = true;
    app.handle_key(key('q'));
    assert!(app.quit_confirm);
    app.confirm_yes = false;
    let _ = commands(&mut rx);
    let terminal = render_app(&mut app);

    assert!(left_click(&mut app, find_on_screen(&terminal, "Quit  ")));

    assert!(app.quit);
}

#[test]
fn clicking_cancel_in_the_quit_confirm_closes_it() {
    let (mut app, _rx) = test_app();
    app.mouse_enabled = true;
    app.handle_key(key('q'));
    let terminal = render_app(&mut app);

    assert!(left_click(&mut app, find_on_screen(&terminal, "Cancel")));

    assert!(!app.quit_confirm);
    assert!(!app.quit);
}

#[test]
fn nothing_behind_a_confirm_popup_is_clickable() {
    let (mut app, _rx) = test_app();
    app.mouse_enabled = true;
    app.handle_key(key('q'));
    let terminal = render_app(&mut app);
    let (col, row) = find_on_screen(&terminal, "Quit ringo?");

    // The popup body outside its buttons, and the screen outside the popup.
    assert!(!left_click(&mut app, (col, row)));
    assert!(!left_click(&mut app, (0, 29)));
    assert!(app.quit_confirm);
}

#[test]
fn a_click_beside_an_overlay_closes_it_a_click_inside_does_not() {
    // Each overlay key, and whether it is open afterwards.
    let open = |app: &App| app.help_show || app.log.show || app.call_history.show;
    for overlay in ['?', 'l', 'c'] {
        let (mut app, _rx) = test_app();
        app.mouse_enabled = true;
        app.handle_key(key(overlay));
        assert!(open(&app), "{overlay}");

        // Inside: the overlay is centred, so the screen centre is in it.
        render_app(&mut app);
        left_click(&mut app, (50, 15));
        assert!(open(&app), "{overlay}: a click inside keeps it");

        // Beside: the top-left corner is outside every centred overlay.
        render_app(&mut app);
        assert!(left_click(&mut app, (0, 0)), "{overlay}");
        assert!(!open(&app), "{overlay}: a click beside closes it");
    }
}

/// A cell of the dial line, which opens the pad; `None` if no cell does.
fn dial_line(app: &App) -> Option<(u16, u16)> {
    (0..30u16)
        .flat_map(|row| (0..100u16).map(move |col| (col, row)))
        .find(|&(col, row)| app.hits.hit(col, row) == Some(&ClickAction::OpenDialpad))
}

#[test]
fn the_dial_line_opens_the_pad_and_its_buttons_dial() {
    let (mut app, mut rx) = test_app();
    app.mouse_enabled = true;
    render_app(&mut app);

    // Idle: the empty dial line opens the pad and the dial input together.
    let line = dial_line(&app).expect("idle dial line is clickable");
    assert!(left_click(&mut app, line));
    assert!(app.dialpad_show);
    assert_eq!(app.dial.mode, InputMode::Dial);

    let terminal = render_app(&mut app);
    for digit in ["4", "7", "1"] {
        let at = find_on_screen(&terminal, &format!(" {digit} "));
        assert!(left_click(&mut app, at), "{digit}");
    }
    assert_eq!(app.dial.input, "471");

    let terminal = render_app(&mut app);
    assert!(left_click(&mut app, find_on_screen(&terminal, "⌫")));
    assert_eq!(app.dial.input, "47");

    let terminal = render_app(&mut app);
    let _ = commands(&mut rx);
    assert!(left_click(&mut app, find_on_screen(&terminal, "Call")));
    assert_eq!(commands(&mut rx), vec!["dial"]);
    render_app(&mut app);
    assert!(!app.dialpad_show, "the pad goes once the call is placed");
}

#[test]
fn the_pad_sends_dtmf_during_a_call_and_closes_with_it() {
    let (mut app, mut rx) = test_app();
    app.mouse_enabled = true;
    established(&mut app, "1");
    let _ = commands(&mut rx);
    render_app(&mut app);

    let line = dial_line(&app).expect("DTMF line");
    assert!(left_click(&mut app, line));
    assert!(app.dialpad_show);
    let terminal = render_app(&mut app);
    assert!(left_click(&mut app, find_on_screen(&terminal, " # ")));
    assert_eq!(commands(&mut rx), vec!["sndcode"]);
    assert_eq!(app.dial.dtmf, "#");

    app.handle_message(AppEvent::CallClosed {
        call_id: "1".into(),
        reason: "".into(),
        error: false,
    });
    render_app(&mut app);
    assert!(!app.dialpad_show);
}

#[test]
fn esc_or_a_click_beside_closes_the_pad_and_keeps_the_number() {
    let (mut app, _rx) = test_app();
    app.mouse_enabled = true;
    render_app(&mut app);
    let line = dial_line(&app).unwrap();
    left_click(&mut app, line);
    app.handle_key(key('5'));

    app.handle_key(esc());
    assert!(!app.dialpad_show);
    assert_eq!(app.dial.mode, InputMode::Dial);
    assert_eq!(app.dial.input, "5");

    render_app(&mut app);
    let line = dial_line(&app).unwrap();
    left_click(&mut app, line);
    assert!(app.dialpad_show);
    render_app(&mut app);
    assert!(left_click(&mut app, (0, 0)));
    assert!(!app.dialpad_show);
    assert_eq!(app.dial.input, "5");
}

#[test]
fn without_the_mouse_there_is_no_pad() {
    let (mut app, _rx) = test_app();
    render_app(&mut app);
    assert_eq!(dial_line(&app), None);
}

#[test]
fn a_click_outside_the_command_bar_leaves_it_and_does_nothing_else() {
    let (mut app, mut rx) = test_app();
    app.mouse_enabled = true;
    ring(&mut app, "1");
    let _ = commands(&mut rx);
    app.handle_key(key(':'));
    app.handle_key(key('x'));
    assert!(app.command.active);

    // A click on the call row closes the bar instead of acting on the call.
    let terminal = render_app(&mut app);
    assert!(left_click(&mut app, find_on_screen(&terminal, "[1]")));
    assert!(!app.command.active);
    assert!(app.command.input.is_empty());
    assert!(commands(&mut rx).is_empty());

    // Over an overlay only the bar goes; the overlay stays.
    app.handle_key(key('l'));
    app.handle_key(key(':'));
    render_app(&mut app);
    assert!(left_click(&mut app, (0, 0)));
    assert!(!app.command.active);
    assert!(app.log.show);
}

#[test]
fn a_click_beside_the_dial_history_search_leaves_it() {
    let (mut app, _rx) = test_app();
    app.mouse_enabled = true;
    app.dial.history = ["100".to_string()].into();
    app.handle_key(key('d'));
    app.handle_key(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL));
    assert_eq!(app.dial.mode, InputMode::HistorySearch);

    render_app(&mut app);
    assert!(left_click(&mut app, (0, 0)));
    assert_ne!(app.dial.mode, InputMode::HistorySearch);
}

#[test]
fn a_click_beside_a_confirmation_does_not_dismiss_it() {
    let (mut app, _rx) = test_app();
    app.mouse_enabled = true;
    app.contacts = vec![ringo::contacts::Contact {
        name: "Alice".into(),
        numbers: vec!["100".into()],
    }];
    app.handle_key(key('f'));
    app.handle_key(key('d'));
    assert!(app.contacts_state.delete_confirm.is_some());

    render_app(&mut app);
    assert!(!left_click(&mut app, (0, 0)));
    assert!(app.contacts_state.delete_confirm.is_some());
    assert!(app.contacts_state.show);
}

#[test]
fn clicking_a_character_hint_while_dialing_does_not_type_it() {
    let (mut app, _rx) = test_app();
    app.mouse_enabled = true;
    app.handle_key(key('d'));
    assert_eq!(app.dial.mode, InputMode::Dial);
    let terminal = render_app(&mut app);

    // `Tab contacts` is a non-character hint and stays clickable.
    assert!(left_click(
        &mut app,
        find_on_screen(&terminal, "Esc cancel")
    ));
    assert_eq!(app.dial.mode, InputMode::Normal);

    app.handle_key(key('d'));
    let terminal = render_app(&mut app);
    // No region records a plain character while a text input is active.
    for col in 0..100 {
        for row in 0..30 {
            if let Some(ClickAction::Key(k)) = app.hits.hit(col, row) {
                assert!(
                    !matches!(k.code, KeyCode::Char(_))
                        || k.modifiers.contains(KeyModifiers::CONTROL),
                    "{k:?} at ({col},{row})"
                );
            }
        }
    }
    let _ = terminal;
    assert!(app.dial.input.is_empty());
}

#[test]
fn clicking_an_overlay_hint_while_the_command_bar_is_open_does_not_type_it() {
    let (mut app, _rx) = test_app();
    app.mouse_enabled = true;
    app.handle_key(key('l'));
    assert!(app.log.show);
    app.handle_key(key(':'));
    assert!(app.command.active, "`:` opens the command bar over the log");
    let terminal = render_app(&mut app);

    left_click(&mut app, find_on_screen(&terminal, "w wrap"));

    assert!(app.command.input.is_empty(), "{:?}", app.command.input);
}

#[test]
fn clicking_the_failed_registration_registers_again() {
    let (mut app, mut rx) = test_app();
    app.mouse_enabled = true;
    app.handle_message(AppEvent::RegisterFailed {
        reason: "401".into(),
    });
    let _ = commands(&mut rx);
    let terminal = render_app(&mut app);

    assert!(left_click(&mut app, find_on_screen(&terminal, "Failed")));

    assert_eq!(commands(&mut rx), vec!["uareg"]);
}

#[test]
fn a_registered_account_has_no_register_click() {
    let (mut app, mut rx) = test_app();
    app.mouse_enabled = true;
    app.handle_message(AppEvent::RegisterOk {
        account: "sip:user@example.com".into(),
    });
    let terminal = render_app(&mut app);

    assert!(!left_click(
        &mut app,
        find_on_screen(&terminal, "Registered")
    ));
    assert!(commands(&mut rx).is_empty());
}

#[test]
fn a_double_click_on_a_history_row_dials_it() {
    let (mut app, _rx) = test_app();
    app.mouse_enabled = true;
    app.dial.history = ["one", "two", "three"].map(String::from).into();
    app.handle_key(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE));
    app.handle_key(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL));
    assert_eq!(app.dial.mode, InputMode::HistorySearch);
    let terminal = render_app(&mut app);
    let row = find_on_screen(&terminal, " two");

    assert!(left_click(&mut app, row));
    assert_eq!(app.dial.selected, 1);
    assert!(left_click(&mut app, row));
    assert_eq!(app.dial.mode, InputMode::Dial);
    assert_eq!(app.dial.input, "two");
}

#[test]
fn register_signs_back_on() {
    let (mut app, mut rx) = test_app();
    app.handle_message(AppEvent::Unregistered {
        account: "sip:user@example.com".into(),
    });

    assert!(app.dispatch("register", "").is_ok());
    assert_eq!(commands(&mut rx), vec!["uareg"]);

    // baresip reports REGISTERING from ua_register; the status follows that
    // event, not the command.
    app.handle_message(AppEvent::Registering {
        account: "sip:user@example.com".into(),
    });
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
fn hangup_key_rejects_a_ringing_call_with_the_profile_code() {
    let (mut app, mut rx) = test_app();
    app.profile.reject_code = 603;
    ring(&mut app, "1");
    let _ = commands(&mut rx);

    app.handle_key(key('b'));
    let sent: Vec<_> = std::iter::from_fn(|| rx.try_recv().ok())
        .filter(|(cmd, _)| cmd == "reject")
        .collect();
    assert_eq!(sent, vec![("reject".into(), "603 Decline".into())]);
    assert_eq!(app.calls[0].declined, Some(ringo::tui::Declined::Rejected));
}

#[test]
fn hangup_key_still_hangs_up_an_established_call() {
    let (mut app, mut rx) = test_app();
    established(&mut app, "1");
    let _ = commands(&mut rx);

    app.handle_key(key('b'));
    let cmds = commands(&mut rx);
    assert!(cmds.contains(&"hangup".to_string()));
    assert!(!cmds.contains(&"reject".to_string()));
}

#[test]
fn the_reject_command_needs_a_ringing_call() {
    let (mut app, mut rx) = test_app();
    assert!(app.dispatch("reject", "").is_err());
    ring(&mut app, "1");
    let _ = commands(&mut rx);
    assert!(app.dispatch("reject", "").is_ok());
    assert!(commands(&mut rx).contains(&"reject".to_string()));
}

#[test]
fn the_reject_command_takes_an_optional_code() {
    let (mut app, mut rx) = test_app();
    ring(&mut app, "1");
    let _ = commands(&mut rx);

    assert!(app.dispatch("reject", "abc").is_err());
    assert!(app.dispatch("reject", "200").is_err());
    assert!(app.dispatch("reject", "603").is_ok());
    let sent: Vec<_> = std::iter::from_fn(|| rx.try_recv().ok())
        .filter(|(cmd, _)| cmd == "reject")
        .collect();
    assert_eq!(sent, vec![("reject".into(), "603 Decline".into())]);
}

#[test]
fn tab_completes_the_reject_code() {
    let (mut app, _rx) = test_app();
    app.command.input = "reject 6".into();
    app.cycle_completion();
    assert_eq!(app.command.input, "reject 603");
    app.cycle_completion();
    assert_eq!(app.command.input, "reject 600");
    app.cycle_completion();
    assert_eq!(app.command.input, "reject 603");

    app.reset_tab();
    app.command.input = "reject 48".into();
    app.cycle_completion();
    app.cycle_completion();
    assert_eq!(app.command.input, "reject 480");

    // Commands without completable arguments are left alone.
    app.reset_tab();
    app.command.input = "dial 12".into();
    app.cycle_completion();
    assert_eq!(app.command.input, "dial 12");
}

fn deflects(rx: &mut tokio::sync::mpsc::Receiver<(String, String)>) -> Vec<String> {
    std::iter::from_fn(|| rx.try_recv().ok())
        .filter(|(cmd, _)| cmd == "deflect")
        .map(|(_, params)| params)
        .collect()
}

#[test]
fn t_on_a_ringing_call_deflects_it() {
    let (mut app, mut rx) = test_app();
    ring(&mut app, "1");
    assert!(app.hints().contains(&("t", "deflect")));

    app.handle_key(key('t'));
    assert_eq!(
        app.transfer_mode,
        TransferMode::DeflectInput {
            call_id: "1".into(),
            target: String::new()
        }
    );
    for c in "4711".chars() {
        app.handle_key(key(c));
    }
    app.handle_key(enter());

    assert_eq!(app.transfer_mode, TransferMode::None);
    assert_eq!(
        deflects(&mut rx),
        vec!["1 sip:4711@example.com sip:user@example.com"]
    );
}

#[test]
fn t_on_an_established_call_still_transfers() {
    let (mut app, _rx) = test_app();
    established(&mut app, "1");
    assert!(!app.hints().contains(&("t", "deflect")));
    app.handle_key(key('t'));
    assert_eq!(app.transfer_mode, TransferMode::BlindInput(String::new()));
}

#[test]
fn the_deflect_command_needs_a_target_and_a_ringing_call() {
    let (mut app, mut rx) = test_app();
    assert!(app.dispatch("deflect", "").is_err());
    assert!(app.dispatch("deflect", "4711").is_err());
    ring(&mut app, "1");
    assert!(app.dispatch("deflect", "sip:bob@other.example").is_ok());
    assert_eq!(
        deflects(&mut rx),
        vec!["1 sip:bob@other.example sip:user@example.com"]
    );
}

#[test]
fn declining_a_call_is_not_reported_as_an_error() {
    for (cmd, arg, reason) in [
        ("reject", "", "Busy Here"),
        ("deflect", "4711", "Moved Temporarily"),
    ] {
        let (mut app, _rx) = test_app();
        ring(&mut app, "1");
        app.dispatch(cmd, arg).unwrap();
        // baresip reports our own reason phrase back as the close reason.
        app.handle_message(AppEvent::CallClosed {
            call_id: "1".into(),
            reason: reason.into(),
            error: true,
        });
        assert_eq!(app.last_call_reason, None, "{cmd}");
        assert!(!app.last_call.as_ref().unwrap().error, "{cmd}");
    }
}

#[test]
fn deflect_input_is_dropped_when_its_call_ends() {
    // Two calls ring; the one being deflected ends while the target is typed.
    // Enter must not deflect the other one.
    let (mut app, mut rx) = test_app();
    ring(&mut app, "A");
    ring(&mut app, "B");
    app.selected_call = 0;
    app.handle_key(key('t'));
    app.handle_key(key('9'));
    app.handle_message(AppEvent::CallClosed {
        call_id: "A".into(),
        reason: "".into(),
        error: false,
    });
    assert_eq!(app.transfer_mode, TransferMode::None);

    app.handle_key(enter());
    assert!(deflects(&mut rx).is_empty());
    assert_eq!(app.calls[0].declined, None);
}

#[test]
fn an_unusable_deflect_target_leaves_the_call_ringing() {
    let (mut app, mut rx) = test_app();
    ring(&mut app, "1");
    assert!(
        app.dispatch("deflect", "sip:bob@example.com\r\nX-Test: bad")
            .is_err()
    );
    assert!(deflects(&mut rx).is_empty());
    assert_eq!(app.calls[0].declined, None, "still counts as missed");
}

#[test]
fn a_deflected_call_is_not_missed() {
    let (mut app, _rx) = test_app();
    ring(&mut app, "1");
    app.dispatch("deflect", "4711").unwrap();
    assert!(matches!(
        app.calls[0].declined,
        Some(ringo::tui::Declined::Deflected(ref t)) if t == "sip:4711@example.com"
    ));
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

// ─── Mouse selection ──────────────────────────────────────────────────────────

fn mouse_at(app: &mut App, kind: MouseEventKind, (col, row): (u16, u16)) -> bool {
    app.handle_mouse(mouse_event(kind, col, row))
}

/// Drag from `from` to `to` with the left button and let go.
fn drag_select(app: &mut App, from: (u16, u16), to: (u16, u16)) {
    mouse_at(app, MouseEventKind::Down(MouseButton::Left), from);
    mouse_at(app, MouseEventKind::Drag(MouseButton::Left), to);
    mouse_at(app, MouseEventKind::Up(MouseButton::Left), to);
}

/// What the render loop would write to the terminal after this frame.
fn pending_copy(app: &mut App) -> Vec<u8> {
    let mut out = Vec::new();
    app.selector.flush_copy(&mut out).unwrap();
    out
}

fn reversed_cells(terminal: &Terminal<TestBackend>) -> usize {
    let buf = terminal.backend().buffer();
    buf.content()
        .iter()
        .filter(|cell| cell.modifier.contains(ratatui::style::Modifier::REVERSED))
        .count()
}

#[test]
fn dragging_across_the_call_view_copies_exactly_the_selected_text() {
    let (mut app, _rx) = test_app();
    app.mouse_enabled = true;
    established(&mut app, "1");
    let terminal = render_app(&mut app);
    let (col, row) = find_on_screen(&terminal, "hangup");

    drag_select(&mut app, (col, row), (col + 5, row));

    assert_eq!(app.command.notice.as_deref(), Some("Copied 6 chars"));
    let written = String::from_utf8(pending_copy(&mut app)).unwrap();
    // "hangup" in base64; under tmux the wrapper only surrounds it.
    assert!(written.contains("]52;c;aGFuZ3Vw\x07"), "{written:?}");
    let terminal = render_app(&mut app);
    assert_eq!(
        reversed_cells(&terminal),
        6,
        "the highlight stays after the release"
    );
}

#[test]
fn a_drag_over_a_hint_does_not_press_it() {
    let (mut app, mut rx) = test_app();
    app.mouse_enabled = true;
    ring(&mut app, "1");
    let _ = commands(&mut rx);
    let terminal = render_app(&mut app);
    let (col, row) = find_on_screen(&terminal, "b reject");

    drag_select(&mut app, (col, row), (col + 6, row));

    assert!(commands(&mut rx).is_empty(), "a drag is not a click");
}

#[test]
fn pressing_a_hint_alone_does_nothing_until_the_release() {
    let (mut app, mut rx) = test_app();
    app.mouse_enabled = true;
    ring(&mut app, "1");
    let _ = commands(&mut rx);
    let terminal = render_app(&mut app);
    let at = find_on_screen(&terminal, "b reject");

    assert!(!mouse_at(
        &mut app,
        MouseEventKind::Down(MouseButton::Left),
        at
    ));
    assert!(commands(&mut rx).is_empty());
    assert!(mouse_at(
        &mut app,
        MouseEventKind::Up(MouseButton::Left),
        at
    ));
    assert_eq!(commands(&mut rx), vec!["reject"]);
}

#[test]
fn dragging_within_one_cell_or_to_an_unchanged_range_needs_no_redraw() {
    let (mut app, _rx) = test_app();
    app.mouse_enabled = true;
    established(&mut app, "1");
    let terminal = render_app(&mut app);
    let (col, row) = find_on_screen(&terminal, "hangup");

    mouse_at(
        &mut app,
        MouseEventKind::Down(MouseButton::Left),
        (col, row),
    );
    assert!(!mouse_at(
        &mut app,
        MouseEventKind::Drag(MouseButton::Left),
        (col, row)
    ));
    assert!(mouse_at(
        &mut app,
        MouseEventKind::Drag(MouseButton::Left),
        (col + 3, row)
    ));
    assert!(!mouse_at(
        &mut app,
        MouseEventKind::Drag(MouseButton::Left),
        (col + 3, row)
    ));
}

#[test]
fn a_selection_is_dropped_by_the_next_key_click_or_resize_not_by_a_move() {
    let (mut app, _rx) = test_app();
    app.mouse_enabled = true;
    established(&mut app, "1");
    let terminal = render_app(&mut app);
    let (col, row) = find_on_screen(&terminal, "hangup");

    drag_select(&mut app, (col, row), (col + 5, row));
    assert!(!mouse_at(&mut app, MouseEventKind::Moved, (0, 0)));
    assert_eq!(reversed_cells(&render_app(&mut app)), 6);

    app.handle_key(key('x'));
    assert_eq!(reversed_cells(&render_app(&mut app)), 0, "a key");
    assert_eq!(app.command.notice, None);

    drag_select(&mut app, (col, row), (col + 5, row));
    assert!(mouse_at(
        &mut app,
        MouseEventKind::Down(MouseButton::Left),
        (0, 0)
    ));
    assert_eq!(reversed_cells(&render_app(&mut app)), 0, "a press");

    drag_select(&mut app, (col, row), (col + 5, row));
    app.selector.clear();
    assert_eq!(reversed_cells(&render_app(&mut app)), 0, "a resize");
}

#[test]
fn the_copied_notice_is_not_drawn_as_an_error() {
    let (mut app, _rx) = test_app();
    app.mouse_enabled = true;
    established(&mut app, "1");
    let terminal = render_app(&mut app);
    let (col, row) = find_on_screen(&terminal, "hangup");

    drag_select(&mut app, (col, row), (col + 5, row));
    let terminal = render_app(&mut app);

    let (notice_col, notice_row) = find_on_screen(&terminal, "Copied 6 chars");
    let fg = terminal.backend().buffer()[(notice_col, notice_row)].fg;
    let theme = Theme::default();
    assert_eq!(fg, theme.subtle.get());
    assert_ne!(fg, theme.danger.get());
}

#[test]
fn nothing_is_selected_or_written_while_mouse_is_off() {
    let (mut app, _rx) = test_app();
    established(&mut app, "1");
    let terminal = render_app(&mut app);
    let (col, row) = find_on_screen(&terminal, "hangup");

    mouse_at(
        &mut app,
        MouseEventKind::Down(MouseButton::Left),
        (col, row),
    );
    assert!(!mouse_at(
        &mut app,
        MouseEventKind::Drag(MouseButton::Left),
        (col + 5, row)
    ));
    assert!(!mouse_at(
        &mut app,
        MouseEventKind::Up(MouseButton::Left),
        (col + 5, row)
    ));

    assert!(pending_copy(&mut app).is_empty());
    assert_eq!(app.command.notice, None);
    assert_eq!(reversed_cells(&render_app(&mut app)), 0);
}

#[test]
fn selecting_only_blanks_copies_nothing_and_shows_no_notice() {
    let (mut app, _rx) = test_app();
    app.mouse_enabled = true;
    render_app(&mut app);

    // Row 26, the call-error line, is blank while there is no reason to show.
    drag_select(&mut app, (5, 26), (20, 26));

    assert!(pending_copy(&mut app).is_empty());
    assert_eq!(app.command.notice, None);
}
