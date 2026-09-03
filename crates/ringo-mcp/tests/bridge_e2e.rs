//! End-to-end test of the live-audio WebSocket bridge: drives the real
//! `ringo-mcp` binary over its MCP stdio interface (the only way to exercise
//! the full path — `ProcessClient` spawns `current_exe agent`, which only the
//! server binary implements), then connects to the minted WS URL and speaks
//! the stream protocol.
//!
//! The test agent points at an unreachable registrar; that's fine — the worker
//! spawns and streams regardless (registration just fails in the background).

use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::Sender;
use std::time::{Duration, Instant};
use tokio_tungstenite::tungstenite::Message;

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
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
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
        let req = format!(
            r#"{{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{{"name":"{name}","arguments":{}}}}}"#,
            serde_json::to_string(&args).unwrap()
        );
        self.request(&req);
        self.reply()["result"]["content"][0]["text"].clone()
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
domain = "127.0.0.1:15099"
password = "pw"
"#,
    )
    .unwrap();
    p
}

#[tokio::test]
async fn bridge_stream_open_connect_close() {
    let tmp = tempfile::tempdir().unwrap();
    let config = write_config(tmp.path());
    let mut server = Server::start(&config);

    // Open a duplex stream.
    let text = server.tool(
        "stream_open",
        serde_json::json!({"agent": "alice", "mode": "duplex", "tx_rate": 16000}),
    );
    let info: Value =
        serde_json::from_str(text.as_str().expect("text content")).expect("stream_open reply");
    let url = info["url"].as_str().expect("url in reply").to_string();
    let stream_id = info["stream_id"].as_str().expect("stream_id").to_string();
    assert!(url.starts_with("ws://127.0.0.1:"), "{url}");
    assert_eq!(info["mode"], "duplex");
    assert_eq!(info["tx_rate"], 16000);

    // Connect with the token.
    let (mut ws, _resp) = tokio_tungstenite::connect_async(url.clone())
        .await
        .expect("connect to stream");

    // ping → pong.
    ws.send(Message::Text(r#"{"type":"ping"}"#.into()))
        .await
        .unwrap();
    let msg = tokio::time::timeout(Duration::from_secs(10), ws.next())
        .await
        .expect("timely reply")
        .expect("open connection")
        .expect("ws frame");
    assert_eq!(msg, Message::Text(r#"{"type":"pong"}"#.into()));

    // Binary TX audio is accepted (no error frame back).
    let pcm: Vec<u8> = vec![0u8; 320];
    ws.send(Message::Binary(pcm.into())).await.unwrap();

    // flush_tx → tx_flushed (drain any other frames first).
    ws.send(Message::Text(r#"{"type":"flush_tx"}"#.into()))
        .await
        .unwrap();
    let mut saw_flushed = false;
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        let msg = tokio::time::timeout(Duration::from_secs(5), ws.next())
            .await
            .expect("timely frame")
            .expect("connection open")
            .expect("ws frame");
        if msg == Message::Text(r#"{"type":"tx_flushed"}"#.into()) {
            saw_flushed = true;
            break;
        }
    }
    assert!(saw_flushed, "expected a tx_flushed reply");

    // stream_close kills the connection.
    let _ = server.tool("stream_close", serde_json::json!({"stream_id": stream_id}));
    let closed = tokio::time::timeout(Duration::from_secs(10), async {
        while let Some(Ok(_)) = ws.next().await {
            // Drain until the server closes the socket.
        }
    })
    .await;
    assert!(closed.is_ok(), "connection should close after stream_close");

    // The token is one-shot: a second connect is rejected (404 handshake).
    let second = tokio_tungstenite::connect_async(url).await;
    assert!(second.is_err(), "token must be single-use");
}
