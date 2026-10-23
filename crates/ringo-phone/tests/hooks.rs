//! Which hook events a sequence of call/registration events fires, and with
//! what data. Every hook appends `$RINGO_EVENT $RINGO_EVENT_DATA` to a file;
//! hooks run in the background, so the tests poll for the lines.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use ringo::config::{Hook, HookEvent, Theme};
use ringo::phone::MockPhone;
use ringo::tui::{App, AppEvent};
use serde_json::Value;

struct Recorder {
    path: PathBuf,
    // Kept alive: dropping it closes the channel, and the mock phone logs every
    // dropped command.
    _rx: tokio::sync::mpsc::Receiver<(String, String)>,
}

fn app_with_hooks(test: &str) -> (App, Recorder) {
    let dir = std::env::temp_dir().join(format!("ringo-hooks-{test}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("events");
    let _ = std::fs::remove_file(&path);
    let target = shell_quote(&path.display().to_string());
    let hooks = HookEvent::ALL
        .iter()
        .map(|e| Hook {
            event: e.as_str().into(),
            command: format!(
                "printf '%s %s\\n' \"$RINGO_EVENT\" \"$RINGO_EVENT_DATA\" >> {target}"
            ),
        })
        .collect();
    let (tx, rx) = tokio::sync::mpsc::channel(64);
    let app = App::new(
        "test".into(),
        "sip:user@example.com".into(),
        None,
        None,
        false,
        Box::new(MockPhone::new(tx)),
        Theme::default(),
        hooks,
        ringo::profile::Profile::default(),
        Vec::new(),
        Vec::new(),
    );
    (app, Recorder { path, _rx: rx })
}

/// `s` as one single-quoted shell word, whatever `TMPDIR` contains.
fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

/// Wait until `n` hook lines were written, then return them as
/// `(event, data)`, sorted by event name — hooks run concurrently, so the
/// order in the file is not the order they were started in.
fn fired(rec: &Recorder, n: usize) -> Vec<(String, Value)> {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let lines = read_lines(&rec.path);
        if lines.len() >= n || Instant::now() > deadline {
            // Give a stray extra hook a moment to show up, so "exactly n" holds.
            std::thread::sleep(Duration::from_millis(100));
            let mut out: Vec<_> = read_lines(&rec.path)
                .iter()
                .map(|l| {
                    let (event, data) = l.split_once(' ').unwrap_or((l, "{}"));
                    (event.to_string(), serde_json::from_str(data).unwrap())
                })
                .collect();
            out.sort_by(|a, b| a.0.cmp(&b.0));
            return out;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn read_lines(path: &Path) -> Vec<String> {
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .map(str::to_string)
        .collect()
}

fn names(events: &[(String, Value)]) -> Vec<&str> {
    events.iter().map(|(e, _)| e.as_str()).collect()
}

fn ring(app: &mut App, id: &str) {
    app.handle_message(AppEvent::CallIncoming {
        call_id: id.into(),
        number: format!("sip:{id}@example.com"),
        display_name: Some("Alice".into()),
    });
}

fn close(app: &mut App, id: &str) {
    app.handle_message(AppEvent::CallClosed {
        call_id: id.into(),
        reason: "".into(),
        error: false,
    });
}

#[test]
fn an_answered_call_fires_established_once() {
    let (mut app, rec) = app_with_hooks("established");
    ring(&mut app, "1");
    app.handle_message(AppEvent::CallEstablished {
        call_id: "1".into(),
    });
    // A re-INVITE reports ESTABLISHED again; that is not a new answer.
    app.handle_message(AppEvent::CallEstablished {
        call_id: "1".into(),
    });
    close(&mut app, "1");

    let events = fired(&rec, 3);
    assert_eq!(
        names(&events),
        vec!["call_ended", "call_established", "call_incoming"]
    );
    assert_eq!(events[1].1["direction"], "incoming");
    assert_eq!(events[1].1["number"], "sip:1@example.com");
}

#[test]
fn an_unanswered_call_is_missed() {
    let (mut app, rec) = app_with_hooks("missed");
    ring(&mut app, "1");
    close(&mut app, "1");

    let events = fired(&rec, 3);
    assert_eq!(
        names(&events),
        vec!["call_ended", "call_incoming", "call_missed"]
    );
    assert_eq!(events[2].1["display_name"], "Alice");
}

#[test]
fn a_rejected_call_is_not_missed() {
    let (mut app, rec) = app_with_hooks("rejected");
    ring(&mut app, "1");
    app.dispatch("reject", "").unwrap();
    close(&mut app, "1");

    let events = fired(&rec, 3);
    assert_eq!(
        names(&events),
        vec!["call_ended", "call_incoming", "call_rejected"]
    );
}

#[test]
fn a_deflected_call_names_its_target() {
    let (mut app, rec) = app_with_hooks("deflected");
    ring(&mut app, "1");
    app.dispatch("deflect", "4711").unwrap();
    close(&mut app, "1");

    let events = fired(&rec, 3);
    assert_eq!(
        names(&events),
        vec!["call_deflected", "call_ended", "call_incoming"]
    );
    assert_eq!(events[0].1["target"], "sip:4711@example.com");
    assert_eq!(events[0].1["call_id"], "1");
}

#[test]
fn an_auto_deflected_call_fires_deflected_without_a_call() {
    let (mut app, rec) = app_with_hooks("auto-deflect");
    app.handle_message(AppEvent::CallDeflected {
        from: "sip:alice@example.com".into(),
        display_name: None,
        target: "sip:4711@example.com".into(),
    });

    let events = fired(&rec, 1);
    assert_eq!(names(&events), vec!["call_deflected"]);
    assert_eq!(events[0].1["call_id"], "");
    assert_eq!(events[0].1["number"], "sip:alice@example.com");
}

#[test]
fn an_outgoing_call_is_never_missed() {
    let (mut app, rec) = app_with_hooks("outgoing");
    app.handle_message(AppEvent::CallOutgoing {
        call_id: "1".into(),
        number: "sip:1@example.com".into(),
    });
    close(&mut app, "1");

    let events = fired(&rec, 2);
    assert_eq!(names(&events), vec!["call_ended", "call_outgoing"]);
}

#[test]
fn registration_changes_fire_but_refreshes_do_not() {
    let (mut app, rec) = app_with_hooks("registration");
    let ok = || AppEvent::RegisterOk {
        account: "sip:user@example.com".into(),
    };
    app.handle_message(ok());
    // A refresh re-reports the same state.
    app.handle_message(ok());
    app.handle_message(AppEvent::RegisterFailed {
        reason: "408 Request Timeout".into(),
    });

    let events = fired(&rec, 2);
    assert_eq!(
        names(&events),
        vec!["registration_changed", "registration_changed"]
    );
    let statuses: Vec<_> = events.iter().map(|(_, d)| d["status"].clone()).collect();
    assert!(statuses.contains(&"registered".into()), "{statuses:?}");
    assert!(statuses.contains(&"failed".into()), "{statuses:?}");
    let failed = events
        .iter()
        .find(|(_, d)| d["status"] == "failed")
        .unwrap();
    assert_eq!(failed.1["reason"], "408 Request Timeout");
}

#[test]
fn re_registering_by_hand_fires_registering() {
    let (mut app, rec) = app_with_hooks("re-register");
    app.handle_message(AppEvent::Unregistered {
        account: "sip:user@example.com".into(),
    });
    app.dispatch("register", "").unwrap();
    app.handle_message(AppEvent::Registering {
        account: "sip:user@example.com".into(),
    });

    let events = fired(&rec, 2);
    let statuses: Vec<_> = events.iter().map(|(_, d)| d["status"].clone()).collect();
    assert!(statuses.contains(&"unregistered".into()), "{statuses:?}");
    assert!(statuses.contains(&"registering".into()), "{statuses:?}");
}

#[test]
fn voicemail_fires_on_change_only() {
    let (mut app, rec) = app_with_hooks("voicemail");
    let mwi = |waiting, new_count| AppEvent::VoicemailStatus { waiting, new_count };
    app.handle_message(mwi(true, 2));
    app.handle_message(mwi(true, 2));
    app.handle_message(mwi(false, 0));

    let events = fired(&rec, 2);
    assert_eq!(names(&events), vec!["voicemail", "voicemail"]);
}
