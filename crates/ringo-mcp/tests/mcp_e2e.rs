//! End-to-end tests that drive the real `ringo-mcp` binary over its MCP stdio
//! interface (the only way to exercise the full path — `ProcessClient` spawns
//! `current_exe agent`, which only the server binary implements).
//!
//! The test agents point at an unreachable registrar; that's fine — the worker
//! spawns and takes commands regardless (registration just fails in the
//! background).

use serde_json::Value;
use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::Sender;
use std::time::{Duration, Instant};

/// The MCP server under test: process, one pending-request writer thread (its
/// channel keeps stdin open for the server's lifetime) and a line reader.
struct Server {
    child: Child,
    #[allow(dead_code)]
    requests: Sender<String>,
    reader: BufReader<std::process::ChildStdout>,
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Server {
    fn start(config_path: &std::path::Path) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_ringo-mcp"))
            .arg("--config")
            .arg(config_path)
            // The default trace dir is the server's own; a developer's env must
            // not redirect it. The test kills the server hard (no cleanup), so
            // the dir goes into the test's own temp dir.
            .env_remove("RINGO_AGENT_SIPTRACE")
            .env("XDG_RUNTIME_DIR", config_path.parent().expect("config dir"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            // Inherited: shows up in the failing test's output (spawn errors).
            .stderr(Stdio::inherit())
            .spawn()
            .expect("spawn ringo-mcp");
        let stdin = child.stdin.take().expect("server stdin");
        let stdout = child.stdout.take().expect("server stdout");
        let (tx, rx) = std::sync::mpsc::channel::<String>();
        // Owns stdin: keeps it open while `tx` lives (dropped with the Server).
        std::thread::spawn(move || {
            use std::io::Write;
            let mut stdin = stdin;
            for line in rx {
                if writeln!(stdin, "{line}").is_err() {
                    break;
                }
            }
        });
        let mut server = Self {
            child,
            requests: tx,
            reader: BufReader::new(stdout),
        };
        server.request(
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"e2e","version":"0"}}}"#,
        );
        server.request(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#);
        let _ = server.reply();
        server
    }

    fn request(&mut self, json: &str) {
        self.requests.send(json.to_string()).expect("send request");
    }

    /// Read one JSON-RPC response (notifications are skipped).
    fn reply(&mut self) -> Value {
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            assert!(
                Instant::now() < deadline,
                "timed out waiting for a server reply"
            );
            let mut line = String::new();
            let n = self
                .reader
                .read_line(&mut line)
                .expect("read server stdout");
            assert!(n > 0, "server closed stdout");
            if let Ok(v) = serde_json::from_str::<Value>(&line) {
                if v.get("id").is_some() {
                    return v;
                }
            }
        }
    }

    /// Call a tool and return its content text.
    fn tool(&mut self, name: &str, args: Value) -> Value {
        let reply = self.tool_reply(name, args);
        let text = reply["result"]["content"][0]["text"].clone();
        assert!(text.is_string(), "`{name}` failed: {reply}");
        text
    }

    /// Call a tool and return the full JSON-RPC reply (errors included).
    fn tool_reply(&mut self, name: &str, args: Value) -> Value {
        let req = format!(
            r#"{{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{{"name":"{name}","arguments":{}}}}}"#,
            serde_json::to_string(&args).unwrap()
        );
        self.request(&req);
        self.reply()
    }
}

fn write_config(dir: &std::path::Path) -> std::path::PathBuf {
    let p = dir.join("config.toml");
    std::fs::write(
        &p,
        r#"
[[agent]]
name = "alice"
username = "1001"
domain = "10.255.255.1:5060"
password = "pw"
"#,
    )
    .unwrap();
    p
}

#[test]
fn agent_stop_respawn_and_wait_event_filter() {
    let tmp = tempfile::tempdir().unwrap();
    let config = write_config(tmp.path());
    let mut server = Server::start(&config);

    // The first tool call starts the worker.
    let text = server.tool("agent_status", serde_json::json!({"agent": "alice"}));
    let v: Value = serde_json::from_str(text.as_str().expect("text")).unwrap();
    assert_eq!(v["call_count"], 0, "{v:?}");

    // agent_stop tears the worker down; the next tool call starts it again.
    let stopped = server.tool("agent_stop", serde_json::json!({"agent": "alice"}));
    assert!(stopped.as_str().unwrap().contains("stopped agent `alice`"));
    let text = server.tool("list_agents", serde_json::json!({}));
    let v: Value = serde_json::from_str(text.as_str().expect("text")).unwrap();
    assert_eq!(
        v["agents"][0]["running"], false,
        "slot is empty right after stop"
    );

    // A later tool call starts the worker again — dial puts a call up
    // (the agent_status above the respawned worker confirms it), and the
    // wait_event FILTER is tested deterministically: the filtered wait goes
    // out first, then the hangup triggers the matching call_closed. (Sending
    // the wait only after the dial would race the subscribe — events are not
    // replayed.)
    let _ = server.tool(
        "dial",
        serde_json::json!({"agent": "alice", "target": "1002"}),
    );
    let st = server.tool("agent_status", serde_json::json!({"agent": "alice"}));
    let v: Value = serde_json::from_str(st.as_str().expect("text")).unwrap();
    assert_eq!(v["call_count"], 1, "the respawned worker took the dial");

    // Filtered wait, then the trigger (the harness's writer thread lets us
    // park a pending request behind the blocking one).
    let wait_req = serde_json::json!({
        "jsonrpc": "2.0", "id": 9, "method": "tools/call",
        "params": {"name": "wait_event", "arguments": {
            "agent": "alice", "timeout_ms": 15000, "event": ["call_closed"]}}
    });
    server.request(&wait_req.to_string());
    // Give the server a moment to take the subscription.
    std::thread::sleep(Duration::from_millis(300));
    let _ = server.tool("hangup", serde_json::json!({"agent": "alice"}));
    let text = server.reply();
    let inner = text["result"]["content"][0]["text"].clone();
    let v: Value = serde_json::from_str(inner.as_str().expect("text")).unwrap();
    assert_eq!(
        v["event"], "call_closed",
        "filter must deliver the matching event, got {v:?}"
    );
    assert_eq!(text["id"], 9);
}

#[test]
fn dial_wait_established_outcomes() {
    let tmp = tempfile::tempdir().unwrap();
    let p = tmp.path().join("config.toml");
    std::fs::write(
        &p,
        "[[agent]]\nname = \"alice\"\nusername = \"1001\"\ndomain = \"10.255.255.1:5060\"\npassword = \"pw\"\n\
         \n[[agent]]\nname = \"bob\"\nusername = \"1002\"\ndomain = \"127.0.0.1:15099\"\npassword = \"pw\"\n",
    )
    .unwrap();
    let mut server = Server::start(&p);

    // timeout_ms without wait_established is a parameter error.
    server.request(
        &serde_json::json!({
            "jsonrpc": "2.0", "id": 20, "method": "tools/call",
            "params": {"name": "dial", "arguments": {"agent": "alice", "target": "1002", "timeout_ms": 5000}}
        })
        .to_string(),
    );
    let reply = server.reply();
    assert_eq!(reply["error"]["code"], -32602, "{reply}");
    assert!(
        reply["error"]["message"]
            .as_str()
            .unwrap()
            .contains("wait_established"),
        "{reply}"
    );

    // A blackhole target rings forever: with a short budget the dial returns
    // still-ringing (the call CONTINUES — no auto-cancel) with its call_id.
    let text = server.tool(
        "dial",
        serde_json::json!({"agent": "alice", "target": "2001", "wait_established": true, "timeout_ms": 4000}),
    );
    let v: Value = serde_json::from_str(text.as_str().expect("text")).unwrap();
    assert_eq!(v["established"], false, "{v}");
    assert_eq!(v["state"], "ringing", "{v}");
    assert!(v["call_id"].as_str().is_some_and(|s| !s.is_empty()), "{v}");
    // …and the call is really still up for the caller to manage.
    let st = server.tool("agent_status", serde_json::json!({"agent": "alice"}));
    let v: Value = serde_json::from_str(st.as_str().expect("text")).unwrap();
    assert_eq!(v["call_count"], 1, "{v}");
    let _ = server.tool("hangup_all", serde_json::json!({"agent": "alice"}));

    // A locally-rejected target (baresip refuses loopback peers: "no laddr")
    // produces NO call at all — the confirm deadline surfaces it instead of
    // blocking out the full budget.
    let started = std::time::Instant::now();
    let text = server.tool(
        "dial",
        serde_json::json!({"agent": "bob", "target": "3001", "wait_established": true, "timeout_ms": 60000}),
    );
    let v: Value = serde_json::from_str(text.as_str().expect("text")).unwrap();
    assert_eq!(v["established"], false, "{v}");
    assert!(
        v["error"].as_str().is_some_and(|e| e.contains("no call")),
        "{v}"
    );
    // The 10 s confirm deadline, not the 60 s budget.
    assert!(
        started.elapsed() < std::time::Duration::from_secs(30),
        "NoCall must surface at the confirm deadline, took {:?}",
        started.elapsed()
    );
}

#[test]
fn event_log_cursor_closes_the_roundtrip_race() {
    let tmp = tempfile::tempdir().unwrap();
    let p = tmp.path().join("config.toml");
    std::fs::write(
        &p,
        "[[agent]]\nname = \"alice\"\nusername = \"1001\"\ndomain = \"10.255.255.1:5060\"\npassword = \"pw\"\n",
    )
    .unwrap();
    let mut server = Server::start(&p);

    // Take a cursor, then act. The call_outgoing fires while nobody waits —
    // with the cursor it is NOT lost.
    let text = server.tool("agent_events", serde_json::json!({"agent": "alice"}));
    let v: Value = serde_json::from_str(text.as_str().expect("text")).unwrap();
    let cursor = v["newest_id"].as_u64().expect("newest_id");
    let _ = server.tool(
        "dial",
        serde_json::json!({"agent": "alice", "target": "1002"}),
    );
    std::thread::sleep(Duration::from_millis(1500));

    // The log has the event, tagged with ids.
    let text = server.tool(
        "agent_events",
        serde_json::json!({"agent": "alice", "after_id": cursor}),
    );
    let v: Value = serde_json::from_str(text.as_str().expect("text")).unwrap();
    let events = v["events"].as_array().expect("events");
    assert!(
        events.iter().any(|e| e["event"] == "call_outgoing"),
        "{v:?}"
    );
    assert!(events.iter().all(|e| e["id"].is_u64()), "{v:?}");
    assert!(!v["truncated"].as_bool().unwrap(), "{v:?}");

    // The SAME event would be lost with live-only semantics — wait_event with
    // the cursor returns it from the log, immediately, with its id.
    let text = server.tool(
        "wait_event",
        serde_json::json!({
            "agent": "alice", "timeout_ms": 3000, "after_id": cursor,
            "event": "call_outgoing"
        }),
    );
    let v: Value = serde_json::from_str(text.as_str().expect("text")).unwrap();
    assert_eq!(v["event"], "call_outgoing", "{v:?}");
    let seen = v["id"].as_u64().expect("id");

    // The next cursor sees nothing new: no duplicates.
    let text = server.tool(
        "agent_events",
        serde_json::json!({"agent": "alice", "after_id": seen}),
    );
    let v: Value = serde_json::from_str(text.as_str().expect("text")).unwrap();
    assert!(
        v["events"]
            .as_array()
            .unwrap()
            .iter()
            .all(|e| e["id"].as_u64().unwrap() > seen),
        "{v:?}"
    );

    let _ = server.tool("hangup_all", serde_json::json!({"agent": "alice"}));
}

#[test]
fn sip_trace_returns_sent_messages_filterable_by_call_id() {
    let tmp = tempfile::tempdir().unwrap();
    let config = write_config(tmp.path());
    let mut server = Server::start(&config);

    let _ = server.tool(
        "dial",
        serde_json::json!({"agent": "alice", "target": "1002"}),
    );
    let st = server.tool("agent_status", serde_json::json!({"agent": "alice"}));
    let v: Value = serde_json::from_str(st.as_str().expect("text")).unwrap();
    let call_id = v["calls"][0]["id"]
        .as_str()
        .expect("dialed call has a call_id")
        .to_string();

    // The worker writes the INVITE as soon as it is sent; poll briefly.
    let deadline = Instant::now() + Duration::from_secs(10);
    let trace = loop {
        let text = server.tool(
            "sip_trace",
            serde_json::json!({"agent": "alice", "call_id": call_id}),
        );
        let v: Value = serde_json::from_str(text.as_str().expect("text")).unwrap();
        if v["count"].as_u64().unwrap_or(0) > 0 || Instant::now() > deadline {
            break v;
        }
        std::thread::sleep(Duration::from_millis(200));
    };
    let messages = trace["messages"].as_array().expect("messages");
    let invite = messages
        .iter()
        .find(|m| {
            m["first_line"]
                .as_str()
                .unwrap_or("")
                .starts_with("INVITE ")
        })
        .unwrap_or_else(|| panic!("no INVITE in {trace:?}"));
    assert_eq!(invite["direction"], "tx");
    assert_eq!(invite["call_id"], call_id.as_str());
    assert!(invite["text"].as_str().unwrap().contains("Call-ID"));
    // The filter really filters: no REGISTER (other Call-ID) in this view.
    assert!(
        messages.iter().all(|m| m["call_id"] == call_id.as_str()),
        "{trace:?}"
    );

    // Unfiltered, the cursor pages without duplicates.
    let text = server.tool("sip_trace", serde_json::json!({"agent": "alice"}));
    let v: Value = serde_json::from_str(text.as_str().expect("text")).unwrap();
    let newest = v["newest_id"].as_u64().expect("newest_id");
    assert_eq!(v["truncated"], false, "{v:?}");
    assert!(newest >= 1, "{v:?}");
    let text = server.tool(
        "sip_trace",
        serde_json::json!({"agent": "alice", "after_id": newest}),
    );
    let v: Value = serde_json::from_str(text.as_str().expect("text")).unwrap();
    assert!(
        v["messages"]
            .as_array()
            .unwrap()
            .iter()
            .all(|m| m["id"].as_u64().unwrap() > newest),
        "{v:?}"
    );

    let _ = server.tool("hangup_all", serde_json::json!({"agent": "alice"}));
}

/// A tool error surfaces as a JSON-RPC error; its message tells the caller
/// what to do.
fn tool_error_message(reply: &Value) -> String {
    reply["error"]["message"]
        .as_str()
        .unwrap_or_else(|| panic!("expected a JSON-RPC error, got {reply:?}"))
        .to_string()
}

#[test]
fn save_audio_without_and_with_recording() {
    let tmp = tempfile::tempdir().unwrap();
    let config = write_config(tmp.path());
    let mut server = Server::start(&config);

    // Without recording: a clear error naming the ways out (must mention
    // `record`, so the model learns how to turn recording on).
    let reply = server.tool_reply(
        "save_audio",
        serde_json::json!({"agent": "alice", "prefix": "rec"}),
    );
    let msg = tool_error_message(&reply);
    assert!(msg.contains("record"), "{msg}");
    assert!(msg.contains("record_start"), "{msg}");

    // record_start arms recording; with no call there is no audio yet — a
    // DIFFERENT error than "recording is off".
    let started = server.tool("record_start", serde_json::json!({"agent": "alice"}));
    assert!(started.as_str().unwrap().contains("recording started"));
    let reply = server.tool_reply(
        "save_audio",
        serde_json::json!({"agent": "alice", "prefix": "rec"}),
    );
    let msg = tool_error_message(&reply);
    assert!(msg.contains("no audio"), "{msg}");
    assert!(!msg.contains("recording is off"), "{msg}");

    // record_stop disarms (idempotent), keeping whatever was recorded.
    let stopped = server.tool("record_stop", serde_json::json!({"agent": "alice"}));
    assert!(stopped.as_str().unwrap().contains("recording stopped"));
    let _ = server.tool("record_stop", serde_json::json!({"agent": "alice"}));
}

#[cfg(unix)]
#[test]
fn default_trace_dir_is_private() {
    use std::os::unix::fs::PermissionsExt;
    let tmp = tempfile::tempdir().unwrap();
    let config = write_config(tmp.path());
    let mut server = Server::start(&config);
    let _ = server.tool("agent_status", serde_json::json!({"agent": "alice"}));
    let dir = std::fs::read_dir(tmp.path())
        .unwrap()
        .flatten()
        .find(|e| e.file_name().to_string_lossy().starts_with("ringo-mcp-"))
        .expect("trace dir in XDG_RUNTIME_DIR");
    let mode = dir.metadata().unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o700, "trace dir must be owner-only, got {mode:o}");
}
