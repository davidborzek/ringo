mod app;
mod call;
mod call_history;
mod contacts;
mod dial;
mod handler;
pub mod ui;

mod command;
mod keys;
mod log;
mod transfer;

pub use crate::event::AppEvent;
#[allow(unused_imports)]
pub use app::{App, CallDirection, CallState, RegStatus, TransferMode};

use anyhow::Result;
use crossterm::event::{self as ct_event, Event};
use ratatui::{Terminal, backend::CrosstermBackend};
use std::{io, path::PathBuf, sync::mpsc, time::Duration};
use tokio::{net::TcpStream, sync::mpsc as tokio_mpsc};

use crate::{client, phone::BaresipPhone};

/// Connect to baresip's ctrl_tcp port (retrying within `connect_timeout`),
/// then spawn reader/writer tasks and return. On connect timeout, sends
/// `AppEvent::BaresipConnectFailed` and returns without spawning.
async fn run_baresip_io(
    port: u16,
    connect_timeout: Duration,
    baresip_log_path: Option<PathBuf>,
    msg_tx: mpsc::Sender<AppEvent>,
    mut cmd_rx: tokio_mpsc::Receiver<(String, String)>,
) {
    use tokio::time::{Instant, sleep};

    let deadline = Instant::now() + connect_timeout;
    let stream = loop {
        match TcpStream::connect(("127.0.0.1", port)).await {
            Ok(s) => break s,
            Err(e) if Instant::now() >= deadline => {
                let reason = match baresip_log_path.as_ref() {
                    Some(p) => format!(
                        "Could not connect on port {} ({}). See log: {}",
                        port,
                        e,
                        p.display()
                    ),
                    None => format!("Could not connect on port {} ({})", port, e),
                };
                crate::rlog!(Error, "{}", reason);
                let _ = msg_tx.send(AppEvent::BaresipConnectFailed { reason });
                return;
            }
            Err(_) => sleep(Duration::from_millis(100)).await,
        }
    };

    let (mut reader, mut writer) = stream.into_split();

    tokio::spawn(async move {
        loop {
            match client::read_message(&mut reader).await {
                Ok(msg) => {
                    if msg_tx.send(AppEvent::from(msg)).is_err() {
                        break;
                    }
                }
                Err(e) => {
                    crate::rlog!(Error, "tcp reader: {}", e);
                    break;
                }
            }
        }
    });

    tokio::spawn(async move {
        while let Some((cmd, params)) = cmd_rx.recv().await {
            if let Err(e) = client::write_command(&mut writer, &cmd, &params).await {
                crate::rlog!(Error, "tcp writer: {} (cmd={})", e, cmd);
                break;
            }
        }
    });
}

// ─── Main entry point ─────────────────────────────────────────────────────────

/// Parameters shared by [`run`] and [`run_headless`] to build a session.
pub struct SessionParams {
    pub profile_name: String,
    pub account_aor: String,
    pub port: u16,
    pub control_socket: PathBuf,
    pub baresip_log_path: Option<PathBuf>,
    pub call_history_path: Option<PathBuf>,
    pub notify: bool,
    pub regint: Option<u32>,
    pub custom_headers: Vec<(String, String)>,
    pub theme: crate::config::Theme,
    pub hooks: Vec<crate::config::Hook>,
    pub profile: crate::profile::Profile,
    pub contacts: Vec<crate::contacts::Contact>,
}

/// Build the tokio runtime, spawn the baresip I/O + control-socket tasks, and
/// construct the [`App`] with registration + static headers already issued.
/// Shared by the TUI and headless entry points.
fn setup(
    p: SessionParams,
) -> Result<(
    tokio::runtime::Runtime,
    App,
    mpsc::Receiver<AppEvent>,
    mpsc::Receiver<crate::control::RemoteRequest>,
    Option<crate::control::Registration>,
)> {
    let (msg_tx, msg_rx) = mpsc::channel::<AppEvent>();
    let (cmd_tx, cmd_rx) = tokio_mpsc::channel::<(String, String)>(32);
    let (remote_tx, remote_rx) = mpsc::channel::<crate::control::RemoteRequest>();

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;

    let phone = BaresipPhone::new(cmd_tx);

    rt.spawn(run_baresip_io(
        p.port,
        Duration::from_secs(10),
        p.baresip_log_path.clone(),
        msg_tx,
        cmd_rx,
    ));

    // Bind the per-session control socket synchronously (within the runtime),
    // then register — so the registry entry never advertises a socket that
    // isn't yet connectable. On bind failure, surface it and run without remote
    // control (no phantom registry entry is left behind).
    let control = {
        let _enter = rt.enter();
        match crate::control::bind(&p.control_socket) {
            Ok(listener) => {
                let info = crate::control::session_info(
                    &p.profile_name,
                    &p.account_aor,
                    &p.control_socket,
                );
                let guard = crate::control::register(&info);
                rt.spawn(crate::control::serve(listener, remote_tx));
                Some(guard)
            }
            Err(e) => {
                crate::rlog!(Error, "remote control unavailable: {}", e);
                eprintln!("warning: remote control unavailable: {e}");
                None
            }
        }
    };

    let app = App::new(
        p.profile_name,
        p.account_aor,
        p.baresip_log_path,
        p.call_history_path,
        p.notify,
        Box::new(phone),
        p.theme,
        p.hooks,
        p.profile,
        p.contacts,
        p.custom_headers
            .into_iter()
            .map(|(k, v)| (k, crate::header::HeaderTemplate::new(v)))
            .collect(),
    );

    let aor = app.account_aor.clone();
    app.phone.register(&aor, p.regint.unwrap_or(3600));

    // Add only static headers at startup. Dynamic templates (e.g. `$uuid`)
    // are re-added per call by App::dial so each call gets a fresh value.
    for (key, tpl) in &app.custom_headers {
        if !tpl.is_dynamic() {
            app.phone.add_header(key, tpl.raw());
        }
    }

    Ok((rt, app, msg_rx, remote_rx, control))
}

pub fn run(params: SessionParams) -> Result<Option<String>> {
    let (rt, mut app, msg_rx, remote_rx, _control) = setup(params)?;

    // Set up terminal
    crossterm::terminal::enable_raw_mode()?;
    let mut stdout = io::stdout();
    crossterm::execute!(stdout, crossterm::terminal::EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    let mut do_restart = false;
    loop {
        render_loop(&mut terminal, &mut app, &msg_rx, &remote_rx)?;

        if app.edit_contacts {
            app.edit_contacts = false;
            app.quit = false;

            open_contacts_editor(&mut terminal)?;

            app.contacts = crate::contacts::load();
            app.contacts_state.selected = 0;
            app.contacts_state.search_query.clear();
            app.contacts_state.search_mode = false;
            continue;
        }

        if !app.edit_profile {
            break;
        }

        // Open edit form over the still-running TUI terminal
        app.edit_profile = false;
        app.quit = false;

        let profile = crate::profile::load(&app.profile_name)?;
        let result = crate::form::run_form(
            &mut terminal,
            Some(&app.profile_name),
            &profile,
            &[],
            &app.theme,
        )?;
        if let Some((_, new_profile)) = result {
            crate::profile::save(&app.profile_name, &new_profile)?;
            if crate::form::run_restart_confirm(&mut terminal, &app.theme)? {
                do_restart = true;
                break;
            }
        }
        // Form cancelled or "Later" → resume TUI
    }

    // Restore terminal unconditionally
    let _ = crossterm::terminal::disable_raw_mode();
    let _ = crossterm::execute!(
        terminal.backend_mut(),
        crossterm::terminal::LeaveAlternateScreen
    );

    // Drop runtime without waiting for blocked TCP tasks
    rt.shutdown_background();

    if do_restart {
        return Ok(Some(app.profile_name.clone()));
    }

    if app.switch_to {
        match crate::app::pick_profile(Some(&app.profile_name)) {
            Ok(name) => return Ok(Some(name)),
            Err(_) => {}
        }
    }

    Ok(None)
}

/// Run a session without a TUI: process baresip events and remote-control
/// commands until a remote `shutdown` (sets `app.quit`) or Ctrl-C. Intended for
/// automated/headless telephony testing driven via `ringo control`.
pub fn run_headless(params: SessionParams) -> Result<()> {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    let profile_name = params.profile_name.clone();
    let (rt, mut app, msg_rx, remote_rx, _control) = setup(params)?;

    let stop = Arc::new(AtomicBool::new(false));
    let stop_signal = stop.clone();
    rt.spawn(async move {
        if tokio::signal::ctrl_c().await.is_ok() {
            stop_signal.store(true, Ordering::SeqCst);
        }
    });

    println!(
        "ringo headless: profile '{}' (pid {}) — drive via `ringo control -t {} …`, Ctrl-C to stop",
        profile_name,
        std::process::id(),
        profile_name
    );

    loop {
        while let Ok(event) = msg_rx.try_recv() {
            app.handle_message(event);
        }
        while let Ok(req) = remote_rx.try_recv() {
            let resp = match app.dispatch(&req.command, &req.params) {
                Ok(data) => crate::control::ControlResponse::ok(data),
                Err(e) => crate::control::ControlResponse::err(e),
            };
            let _ = req.reply.send(resp);
        }
        // `app.quit` is set by the remote `shutdown` command.
        if app.quit || stop.load(Ordering::SeqCst) {
            break;
        }
        std::thread::sleep(Duration::from_millis(40));
    }

    // Hang up active calls, then give the I/O tasks a brief moment to flush —
    // the BYE to baresip and the `shutdown` ack back to the client — before the
    // runtime is torn down.
    app.phone.hangup_all();
    std::thread::sleep(Duration::from_millis(150));
    rt.shutdown_background();
    println!("ringo headless: stopped");
    Ok(())
}

// ─── Contacts editor ─────────────────────────────────────────────────────────

fn open_contacts_editor(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
) -> anyhow::Result<()> {
    use crossterm::terminal::{EnterAlternateScreen, LeaveAlternateScreen};
    use std::process::Command;

    let Some(path) = crate::contacts::contacts_path() else {
        return Ok(());
    };

    if !path.exists() {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(
            &path,
            "# ringo contacts\n\
             # [[contacts]]\n\
             # name = \"Alice\"\n\
             # numbers = [\"+49123456789\", \"alice.work\"]\n",
        )?;
    }

    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".into());

    crossterm::terminal::disable_raw_mode()?;
    crossterm::execute!(terminal.backend_mut(), LeaveAlternateScreen)?;

    let status = Command::new(&editor).arg(&path).status();

    crossterm::terminal::enable_raw_mode()?;
    crossterm::execute!(terminal.backend_mut(), EnterAlternateScreen)?;
    terminal.clear()?;

    status
        .map(|_| ())
        .map_err(|e| anyhow::anyhow!("Failed to open editor '{}': {}", editor, e))
}

// ─── Render loop ──────────────────────────────────────────────────────────────

fn render_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
    msg_rx: &mpsc::Receiver<AppEvent>,
    remote_rx: &mpsc::Receiver<crate::control::RemoteRequest>,
) -> Result<()> {
    use std::time::Duration;
    loop {
        app.tick = app.tick.wrapping_add(1);
        // Refresh baresip log every ~500ms (30 ticks × 16ms) when visible
        if app.log.show_baresip && app.tick % 30 == 0 {
            app.refresh_baresip_log();
        }

        terminal.draw(|frame| ui::render(frame, app))?;

        if ct_event::poll(Duration::from_millis(16))? {
            if let Event::Key(key) = ct_event::read()? {
                app.handle_key(key);
                if app.quit {
                    break;
                }
            }
        }

        while let Ok(event) = msg_rx.try_recv() {
            app.handle_message(event);
        }

        // Dispatch any remote-control commands through the same path as the
        // command line, replying to the waiting socket connection.
        while let Ok(req) = remote_rx.try_recv() {
            let resp = match app.dispatch(&req.command, &req.params) {
                Ok(data) => crate::control::ControlResponse::ok(data),
                Err(e) => crate::control::ControlResponse::err(e),
            };
            let _ = req.reply.send(resp);
        }
        // A remote `shutdown` sets `app.quit` outside the key handler.
        if app.quit {
            break;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pick an OS-assigned ephemeral port and immediately drop the listener,
    /// so connect attempts will fail with "connection refused".
    fn unbound_port() -> u16 {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);
        port
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn connect_failure_emits_baresip_connect_failed() {
        let (msg_tx, msg_rx) = mpsc::channel::<AppEvent>();
        let (_cmd_tx, cmd_rx) = tokio_mpsc::channel::<(String, String)>(1);
        let port = unbound_port();
        let log_path = PathBuf::from("/tmp/ringo-test/baresip.log");

        run_baresip_io(
            port,
            Duration::from_millis(150),
            Some(log_path.clone()),
            msg_tx,
            cmd_rx,
        )
        .await;

        let event = msg_rx.try_recv().expect("expected an AppEvent");
        match event {
            AppEvent::BaresipConnectFailed { reason } => {
                assert!(
                    reason.contains(&format!("port {}", port)),
                    "reason: {reason}"
                );
                assert!(reason.contains("See log:"), "reason: {reason}");
                assert!(
                    reason.contains(log_path.to_str().unwrap()),
                    "reason: {reason}"
                );
                println!("UI will see: {reason}");
            }
            other => panic!("expected BaresipConnectFailed, got {other:?}"),
        }
        assert!(msg_rx.try_recv().is_err(), "no further events expected");
    }
}
