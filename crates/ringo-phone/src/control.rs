//! Remote control of a running ringo session.
//!
//! Each running session exposes a Unix domain socket and registers itself in a
//! per-user runtime directory. A separate `ringo control …` invocation looks up
//! the session by profile name, connects to its socket and sends a single
//! command, receiving one response back. The wire format mirrors baresip's
//! ctrl_tcp protocol: a netstring-framed JSON object.

use anyhow::{Context, Result, bail};
use serde::{Serialize, de::DeserializeOwned};
use std::{
    fs,
    path::{Path, PathBuf},
};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    sync::oneshot,
};

/// Commands accepted over the control socket. ringo's UI-only commands
/// (`quit`, `edit`, `switch`, panel toggles) are intentionally excluded.
pub const ALLOWED_COMMANDS: &[&str] = &[
    "dial", "d", "hangup", "accept", "a", "hold", "resume", "mute", "dtmf", "transfer", "xfer",
    "status", "shutdown",
];

const MAX_FRAME_LEN: usize = 1_000_000;

// ─── Wire protocol ─────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, serde::Deserialize)]
pub struct ControlRequest {
    pub command: String,
    #[serde(default)]
    pub params: String,
}

#[derive(Debug, Serialize, serde::Deserialize)]
pub struct ControlResponse {
    pub ok: bool,
    #[serde(default)]
    pub data: String,
    #[serde(default)]
    pub error: Option<String>,
}

impl ControlResponse {
    pub fn ok(data: impl Into<String>) -> Self {
        Self {
            ok: true,
            data: data.into(),
            error: None,
        }
    }

    pub fn err(msg: impl Into<String>) -> Self {
        Self {
            ok: false,
            data: String::new(),
            error: Some(msg.into()),
        }
    }
}

/// A command received over the socket, paired with a channel to reply on.
/// The render loop dispatches `command`/`params` against the live `App` state
/// and sends the result back through `reply`.
pub struct RemoteRequest {
    pub command: String,
    pub params: String,
    pub reply: oneshot::Sender<ControlResponse>,
}

/// Write `val` as a netstring-framed JSON object: `<len>:<json>,`.
async fn write_frame<W: AsyncWrite + Unpin, T: Serialize>(writer: &mut W, val: &T) -> Result<()> {
    let json = serde_json::to_string(val)?;
    let frame = format!("{}:{},", json.len(), json);
    writer.write_all(frame.as_bytes()).await?;
    writer.flush().await?;
    Ok(())
}

/// Read one netstring-framed JSON object and deserialize it into `T`.
async fn read_frame<R: AsyncRead + Unpin, T: DeserializeOwned>(reader: &mut R) -> Result<T> {
    let mut len_bytes: Vec<u8> = Vec::new();
    loop {
        let mut b = [0u8; 1];
        reader
            .read_exact(&mut b)
            .await
            .context("Connection closed")?;
        if b[0] == b':' {
            break;
        }
        if !b[0].is_ascii_digit() {
            bail!("Invalid netstring: expected digit, got 0x{:02x}", b[0]);
        }
        len_bytes.push(b[0]);
    }

    let len: usize = std::str::from_utf8(&len_bytes)?
        .parse()
        .context("Invalid netstring length")?;
    if len > MAX_FRAME_LEN {
        bail!("Frame too large: {} bytes", len);
    }

    let mut payload = vec![0u8; len + 1];
    reader
        .read_exact(&mut payload)
        .await
        .context("Connection closed reading payload")?;
    if payload.last() != Some(&b',') {
        bail!("Invalid netstring: missing trailing ','");
    }
    payload.pop();

    serde_json::from_slice(&payload).context("Invalid JSON in netstring")
}

// ─── Runtime directory & registry ──────────────────────────────────────────────

/// Per-user runtime directory for ringo control state, created 0700.
/// Prefers `$XDG_RUNTIME_DIR`, falling back to the system temp dir.
fn runtime_dir() -> Result<PathBuf> {
    let base = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(std::env::temp_dir);
    let dir = base.join("ringo");
    fs::create_dir_all(&dir).context("Failed to create ringo runtime dir")?;
    set_mode(&dir, 0o700);
    Ok(dir)
}

fn sessions_dir() -> Result<PathBuf> {
    let dir = runtime_dir()?.join("sessions");
    fs::create_dir_all(&dir).context("Failed to create sessions dir")?;
    Ok(dir)
}

/// Path to this process's control socket for `profile`. Keyed by PID so that
/// multiple sessions of the same profile each own their own socket and never
/// clobber one another.
pub fn socket_path(profile: &str) -> Result<PathBuf> {
    Ok(sessions_dir()?.join(format!("{}-{}.sock", profile, std::process::id())))
}

fn registry_path(profile: &str, pid: u32) -> Result<PathBuf> {
    Ok(sessions_dir()?.join(format!("{}-{}.json", profile, pid)))
}

#[cfg(unix)]
fn set_mode(path: &Path, mode: u32) {
    use std::os::unix::fs::PermissionsExt;
    let _ = fs::set_permissions(path, fs::Permissions::from_mode(mode));
}

#[cfg(not(unix))]
fn set_mode(_path: &Path, _mode: u32) {}

#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct SessionInfo {
    pub profile: String,
    pub pid: u32,
    pub socket_path: PathBuf,
    pub aor: String,
    pub started_at: String,
}

/// Removes the registry file and socket for a session when dropped.
pub struct Registration {
    registry_path: PathBuf,
    socket_path: PathBuf,
}

impl Drop for Registration {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.registry_path);
        let _ = fs::remove_file(&self.socket_path);
    }
}

/// Build the registry entry for this process's session.
pub fn session_info(profile: &str, aor: &str, socket_path: &Path) -> SessionInfo {
    SessionInfo {
        profile: profile.to_string(),
        pid: std::process::id(),
        socket_path: socket_path.to_path_buf(),
        aor: aor.to_string(),
        started_at: chrono::Local::now().to_rfc3339(),
    }
}

/// Write the registry entry for an already-bound session and return a guard
/// that removes the registry file and socket on drop.
///
/// Call this only *after* the control socket is bound (see [`bind`]): the entry
/// must never advertise a socket that isn't yet connectable, or a concurrent
/// `list_running`/`reap_stale` would treat the session as dead and delete it.
/// The returned guard always cleans up the socket, even if the registry write
/// failed.
pub fn register(info: &SessionInfo) -> Registration {
    let registry_path = registry_path(&info.profile, info.pid).ok();
    if let Some(path) = &registry_path {
        match serde_json::to_vec_pretty(info) {
            Ok(bytes) => {
                if let Err(e) = fs::write(path, bytes) {
                    crate::rlog!(Warn, "session registry write failed: {}", e);
                } else {
                    set_mode(path, 0o600);
                }
            }
            Err(e) => crate::rlog!(Warn, "session registry encode failed: {}", e),
        }
    }
    Registration {
        registry_path: registry_path.unwrap_or_default(),
        socket_path: info.socket_path.clone(),
    }
}

/// Liveness of a session, probed by connecting to its socket.
enum Liveness {
    Alive,
    /// Socket missing or refusing — the session is gone; safe to reap.
    Dead,
    /// Transient connect error (e.g. fd exhaustion) — leave the entry alone.
    Unknown,
}

fn probe(socket_path: &Path) -> Liveness {
    use std::io::ErrorKind;
    match std::os::unix::net::UnixStream::connect(socket_path) {
        Ok(_) => Liveness::Alive,
        Err(e) if matches!(e.kind(), ErrorKind::NotFound | ErrorKind::ConnectionRefused) => {
            Liveness::Dead
        }
        Err(_) => Liveness::Unknown,
    }
}

/// Remove the registry + socket files for a dead session.
fn reap(info: &SessionInfo) {
    if let Ok(p) = registry_path(&info.profile, info.pid) {
        let _ = fs::remove_file(p);
    }
    let _ = fs::remove_file(&info.socket_path);
}

/// List sessions that are currently reachable, reaping any stale entries.
pub fn list_running() -> Vec<SessionInfo> {
    let Ok(dir) = sessions_dir() else {
        return Vec::new();
    };
    let Ok(entries) = fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let Ok(bytes) = fs::read(&path) else { continue };
        let Ok(info) = serde_json::from_slice::<SessionInfo>(&bytes) else {
            continue;
        };
        match probe(&info.socket_path) {
            Liveness::Alive => out.push(info),
            Liveness::Dead => reap(&info),
            // Transient error: neither list nor reap — try again next time.
            Liveness::Unknown => {}
        }
    }
    out.sort_by(|a, b| a.profile.cmp(&b.profile));
    out
}

/// Reap any registry entries whose sessions are no longer reachable.
pub fn reap_stale() {
    let _ = list_running();
}

// ─── Server (session side) ──────────────────────────────────────────────────────

/// Bind the control socket (0600), clearing any stale socket file first.
/// Must be called within a tokio runtime context. Binding synchronously — and
/// registering only afterwards — guarantees the registry entry never points at
/// a socket that isn't yet connectable.
pub fn bind(socket_path: &Path) -> Result<tokio::net::UnixListener> {
    let _ = fs::remove_file(socket_path);
    let listener = tokio::net::UnixListener::bind(socket_path)
        .with_context(|| format!("bind control socket at {}", socket_path.display()))?;
    set_mode(socket_path, 0o600);
    Ok(listener)
}

/// Accept connections on a bound control socket and forward commands to `tx`.
/// Runs until the runtime is shut down.
pub async fn serve(listener: tokio::net::UnixListener, tx: std::sync::mpsc::Sender<RemoteRequest>) {
    crate::rlog!(Info, "control socket listening");
    loop {
        match listener.accept().await {
            Ok((stream, _)) => {
                let tx = tx.clone();
                tokio::spawn(handle_conn(stream, tx));
            }
            Err(e) => crate::rlog!(Warn, "control accept failed: {}", e),
        }
    }
}

async fn handle_conn(
    mut stream: tokio::net::UnixStream,
    tx: std::sync::mpsc::Sender<RemoteRequest>,
) {
    let req: ControlRequest = match read_frame(&mut stream).await {
        Ok(r) => r,
        Err(e) => {
            crate::rlog!(Debug, "control read failed: {}", e);
            return;
        }
    };

    let resp = if !ALLOWED_COMMANDS.contains(&req.command.as_str()) {
        ControlResponse::err(format!("command not allowed: {}", req.command))
    } else {
        let (reply_tx, reply_rx) = oneshot::channel();
        let forwarded = tx
            .send(RemoteRequest {
                command: req.command,
                params: req.params,
                reply: reply_tx,
            })
            .is_ok();
        if !forwarded {
            ControlResponse::err("session not accepting commands")
        } else {
            reply_rx
                .await
                .unwrap_or_else(|_| ControlResponse::err("no response from session"))
        }
    };

    if let Err(e) = write_frame(&mut stream, &resp).await {
        crate::rlog!(Debug, "control write failed: {}", e);
    }
}

// ─── Client (CLI side) ───────────────────────────────────────────────────────────

/// Connect to a session's socket, send one command and return its response.
pub fn send(socket_path: &Path, command: &str, params: &str) -> Result<ControlResponse> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    rt.block_on(async {
        let mut stream = tokio::net::UnixStream::connect(socket_path)
            .await
            .with_context(|| format!("Failed to connect to {}", socket_path.display()))?;
        write_frame(
            &mut stream,
            &ControlRequest {
                command: command.to_string(),
                params: params.to_string(),
            },
        )
        .await?;
        let resp: ControlResponse = read_frame(&mut stream).await?;
        Ok(resp)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn frame_roundtrip() {
        let req = ControlRequest {
            command: "dial".into(),
            params: "4711".into(),
        };
        let mut buf: Vec<u8> = Vec::new();
        write_frame(&mut buf, &req).await.unwrap();
        // Frame is `<len>:<json>,`
        let s = String::from_utf8(buf.clone()).unwrap();
        assert!(s.ends_with(','));
        assert!(s.contains("\"command\":\"dial\""));

        let mut cursor = std::io::Cursor::new(buf);
        let back: ControlRequest = read_frame(&mut cursor).await.unwrap();
        assert_eq!(back.command, "dial");
        assert_eq!(back.params, "4711");
    }

    #[tokio::test]
    async fn read_frame_rejects_oversized_length() {
        let mut cursor = std::io::Cursor::new(b"2000000:x,".to_vec());
        let res: Result<ControlRequest> = read_frame(&mut cursor).await;
        assert!(res.is_err());
    }

    #[test]
    fn allowed_excludes_ui_commands() {
        for ui in [
            "quit", "edit", "switch", "events", "log", "history", "contacts",
        ] {
            assert!(
                !ALLOWED_COMMANDS.contains(&ui),
                "{ui} must not be remotely allowed"
            );
        }
        for ok in ["dial", "hangup", "accept", "status"] {
            assert!(ALLOWED_COMMANDS.contains(&ok));
        }
    }
}
