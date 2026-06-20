//! End-to-end test of the remote-control transport: bind a session-side socket,
//! drive it from the client, and verify command forwarding + the whitelist.

use ringo::control::{self, ControlResponse, RemoteRequest};
use std::{sync::mpsc, thread, time::Duration};

#[test]
fn socket_roundtrip_forwards_allowed_and_rejects_denied() {
    let dir = std::env::temp_dir().join(format!("ringo-ctrl-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let sock = dir.join("session.sock");

    let (tx, rx) = mpsc::channel::<RemoteRequest>();

    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.spawn(control::serve(sock.clone(), tx));

    // Stand in for the render loop: echo the command back as success.
    thread::spawn(move || {
        while let Ok(req) = rx.recv() {
            let _ = req.reply.send(ControlResponse::ok(format!(
                "got {} {}",
                req.command, req.params
            )));
        }
    });

    // Wait for the listener to bind.
    for _ in 0..100 {
        if sock.exists() {
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }

    // Allowed command is forwarded to the responder and answered.
    let resp = control::send(&sock, "dial", "4711").unwrap();
    assert!(resp.ok, "expected ok, got {resp:?}");
    assert_eq!(resp.data, "got dial 4711");

    // Disallowed UI command is rejected by the server before reaching the app.
    let resp = control::send(&sock, "quit", "").unwrap();
    assert!(!resp.ok);
    assert!(resp.error.unwrap().contains("not allowed"));

    let _ = std::fs::remove_dir_all(&dir);
}

/// Two sessions of the same profile must each own a PID-keyed socket/registry
/// and both show up in the listing — regression test for the case where the
/// second session clobbered the first's files.
#[test]
fn multiple_sessions_same_profile_coexist() {
    let tmp = std::env::temp_dir().join(format!("ringo-multi-{}", std::process::id()));
    let sessions = tmp.join("ringo/sessions");
    std::fs::create_dir_all(&sessions).unwrap();
    // SAFETY: single-threaded setup before any session lookups in this test.
    unsafe {
        std::env::set_var("XDG_RUNTIME_DIR", &tmp);
    }

    // A live, connectable socket + matching registry entry for each PID.
    let make = |pid: u32| -> std::os::unix::net::UnixListener {
        let sock = sessions.join(format!("dup-{pid}.sock"));
        let listener = std::os::unix::net::UnixListener::bind(&sock).unwrap();
        let info = serde_json::json!({
            "profile": "dup",
            "pid": pid,
            "socket_path": sock,
            "aor": "sip:x@y",
            "started_at": "now",
        });
        std::fs::write(sessions.join(format!("dup-{pid}.json")), info.to_string()).unwrap();
        listener
    };
    let _a = make(111_111);
    let _b = make(222_222);

    let running = control::list_running();
    let dup: Vec<_> = running.iter().filter(|s| s.profile == "dup").collect();
    assert_eq!(dup.len(), 2, "both same-profile sessions should be listed");

    let _ = std::fs::remove_dir_all(&tmp);
}
