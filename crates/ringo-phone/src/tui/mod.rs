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

use ringo_core::backend::Session;

/// The TUI's terminal writer. Boxed so it can be either the process stdout or a
/// private `dup` of it — the latter lets us render on the original stdout
/// terminal while redirecting fd 1/2 into the log, keeping baresip/libre's raw
/// stdout off the screen.
pub(crate) type TermWriter = Box<dyn std::io::Write + Send>;
pub(crate) type Term = Terminal<CrosstermBackend<TermWriter>>;

/// Renders the TUI on a private handle to the original stdout terminal while
/// redirecting the process's stdout+stderr (fd 1/2) into the log file. baresip/
/// libre print interface info via raw `re_printf` to fd 1, bypassing the log
/// handlers ringo-core installs; without this it corrupts the screen. The TUI
/// keeps its own `dup` of the terminal, so it still draws there while fd 1/2 go
/// to the log. `restore()` (also run on drop, idempotent) puts fd 1/2 back.
/// Unix-only; on other platforms it's a no-op and the TUI renders on stdout.
struct StdioRedirect {
    saved: Option<(i32, i32)>,
}

impl StdioRedirect {
    /// Duplicate the current stdout into a writer for the TUI, then point fd 1/2
    /// at `file`. Returns the writer plus the guard that restores fd 1/2.
    #[cfg(unix)]
    fn install(file: &std::fs::File) -> (TermWriter, Self) {
        use std::os::fd::{AsRawFd, FromRawFd};
        let fd = file.as_raw_fd();
        unsafe {
            // `tui` renders the TUI, `out`/`err` restore fd 1/2 on teardown —
            // three distinct dups so nothing gets double-closed.
            let tui = libc::dup(1);
            let out = libc::dup(1);
            let err = libc::dup(2);
            if tui < 0 || out < 0 || err < 0 {
                for f in [tui, out, err] {
                    if f >= 0 {
                        libc::close(f);
                    }
                }
                return (Box::new(io::stdout()), Self { saved: None });
            }
            libc::dup2(fd, 1);
            libc::dup2(fd, 2);
            let writer: TermWriter = Box::new(std::fs::File::from_raw_fd(tui));
            (
                writer,
                Self {
                    saved: Some((out, err)),
                },
            )
        }
    }

    #[cfg(not(unix))]
    fn install(_file: &std::fs::File) -> (TermWriter, Self) {
        (Box::new(io::stdout()), Self { saved: None })
    }

    fn restore(&mut self) {
        if let Some((out, err)) = self.saved.take() {
            #[cfg(unix)]
            unsafe {
                libc::dup2(out, 1);
                libc::dup2(err, 2);
                libc::close(out);
                libc::close(err);
            }
        }
    }
}

impl Drop for StdioRedirect {
    fn drop(&mut self) {
        self.restore();
    }
}

// ─── Main entry point ─────────────────────────────────────────────────────────

/// Parameters shared by [`run`] and [`run_headless`] to build a session.
pub struct SessionParams {
    pub profile_name: String,
    pub account_aor: String,
    /// Backend log file path (for TUI display); the binary owns the location.
    pub log_path: PathBuf,
    pub session: Session,
    pub control_socket: PathBuf,
    pub call_history_path: Option<PathBuf>,
    pub notify: bool,
    pub regint: Option<u32>,
    pub custom_headers: Vec<(String, String)>,
    pub theme: crate::config::Theme,
    pub hooks: Vec<crate::config::Hook>,
    pub profile: crate::profile::Profile,
    pub contacts: Vec<crate::contacts::Contact>,
}

/// The pieces `setup()` returns: the runtime, the assembled [`App`], the
/// backend event stream, the remote-control request channel, an optional
/// session registration, and the opaque backend handle (drop ends the session).
type SetupParts = (
    tokio::runtime::Runtime,
    App,
    mpsc::Receiver<AppEvent>,
    mpsc::Receiver<crate::control::RemoteRequest>,
    Option<crate::control::Registration>,
    Box<dyn Send>,
);

/// Build the tokio runtime, spawn the control-socket task, and construct the
/// [`App`] with registration + static headers already issued. The backend
/// I/O tasks are already running (spawned by `Backend::spawn_session`).
/// Shared by the TUI and headless entry points.
fn setup(rt: tokio::runtime::Runtime, p: SessionParams) -> Result<SetupParts> {
    let (remote_tx, remote_rx) = mpsc::channel::<crate::control::RemoteRequest>();

    let log_path = Some(p.log_path.clone());
    let msg_rx = p.session.events;
    let phone = p.session.phone;
    let backend_handle = p.session.handle;

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
        log_path,
        p.call_history_path,
        p.notify,
        phone,
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

    // Arm call deflection (302) if configured — before any inbound call.
    if app.profile.deflect {
        if let Some(target) = &app.profile.deflect_target {
            if !target.is_empty() {
                let uri = command::normalize_sip_uri(target, &aor);
                app.phone.deflect_incoming(&uri, Some(&aor));
                crate::rlog!(Info, "call deflection armed: 302 -> {uri}");
            }
        }
    }

    Ok((rt, app, msg_rx, remote_rx, control, backend_handle))
}

pub fn run(rt: tokio::runtime::Runtime, params: SessionParams) -> Result<Option<String>> {
    let log_path = params.log_path.clone();
    let (rt, mut app, msg_rx, remote_rx, _control, _backend) = setup(rt, params)?;

    // Render the TUI on a private `dup` of the original stdout while redirecting
    // fd 1/2 into the log file: baresip/libre print interface info via raw
    // `re_printf` to fd 1, bypassing ringo-core's log handlers, which would
    // otherwise corrupt the screen. If the log can't be opened we just render on
    // stdout with no redirect (e.g. CI/headless).
    crossterm::terminal::enable_raw_mode()?;
    let (writer, mut redirect): (TermWriter, Option<StdioRedirect>) =
        match std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
        {
            Ok(log) => {
                let (writer, redirect) = StdioRedirect::install(&log);
                (writer, Some(redirect))
            }
            Err(_) => (Box::new(io::stdout()), None),
        };
    let mut terminal = Terminal::new(CrosstermBackend::new(writer))?;
    crossterm::execute!(
        terminal.backend_mut(),
        crossterm::terminal::EnterAlternateScreen
    )?;
    terminal.clear()?;

    let mut do_restart = false;
    loop {
        render_loop(&mut terminal, &mut app, &msg_rx, &remote_rx)?;

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

    // Tear down the session (fires ua_unregister) and wait for the PBX to
    // process the de-register before the caller stops the RE thread — otherwise
    // we leave a stale contact. Bounded so an unresponsive PBX can't hang exit.
    // Do this *before* restoring the screen so the wait happens behind the
    // alternate screen rather than flashing the shell.
    drop(_backend);
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(500);
    while ringo_core::is_registered() && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(20));
    }

    // Drop runtime without waiting for blocked TCP tasks
    rt.shutdown_background();

    // On restart or a profile switch we continue in-process into another
    // full-screen view (the next session's TUI, or the picker), which renders on
    // stdout — so restore fd 1/2 first. Stay in the alternate screen and hand
    // over seamlessly; dropping it here would flash the shell between views.
    if do_restart {
        if let Some(r) = redirect.as_mut() {
            r.restore();
        }
        return Ok(Some(app.profile_name.clone()));
    }
    if app.switch_to {
        if let Some(r) = redirect.as_mut() {
            r.restore();
        }
        if let Ok(name) = crate::app::pick_profile(Some(&app.profile_name)) {
            return Ok(Some(name));
        }
    }

    // Genuine exit: leave fd 1/2 pointing at the log (skip the restore, incl. the
    // Drop one) so any buffered or trailing baresip stdout flushes there instead
    // of onto the shell after we quit. The parent shell keeps its own fds, so
    // it's unaffected.
    std::mem::forget(redirect);
    let _ = crossterm::terminal::disable_raw_mode();
    let _ = crossterm::execute!(
        terminal.backend_mut(),
        crossterm::terminal::LeaveAlternateScreen
    );
    Ok(None)
}

/// Run a session without a TUI: process events and remote-control
/// commands until a remote `shutdown` (sets `app.quit`) or Ctrl-C. Intended for
/// automated/headless telephony testing driven via `ringo control`.
pub fn run_headless(rt: tokio::runtime::Runtime, params: SessionParams) -> Result<()> {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    let profile_name = params.profile_name.clone();
    let (rt, mut app, msg_rx, remote_rx, _control, _backend) = setup(rt, params)?;

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
        if app.quit || stop.load(Ordering::SeqCst) {
            break;
        }
        std::thread::sleep(Duration::from_millis(40));
    }

    app.phone.hangup_all();
    // Poll until baresip has torn down the calls (BYE sent) instead of a blind
    // sleep; cap the wait so a stuck call can't hang shutdown forever.
    let deadline = std::time::Instant::now() + Duration::from_millis(500);
    while ringo_core::call_count() > 0 && std::time::Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    rt.shutdown_background();
    println!("ringo headless: stopped");
    Ok(())
}

// ─── Render loop ──────────────────────────────────────────────────────────────

fn render_loop(
    terminal: &mut Term,
    app: &mut App,
    msg_rx: &mpsc::Receiver<AppEvent>,
    remote_rx: &mpsc::Receiver<crate::control::RemoteRequest>,
) -> Result<()> {
    use std::time::Duration;
    // Redraw only when something changed. The loop still spins every 16ms to stay
    // responsive to backend/remote events, but we skip the terminal write on idle
    // frames — that's what stops the constant repaint/flicker.
    let mut dirty = true;
    loop {
        app.tick = app.tick.wrapping_add(1);
        // Auto-clear deflected indicator after 10 s.
        if let Some(d) = &app.deflected {
            if d.at.elapsed().as_secs() >= 10 {
                app.deflected = None;
                dirty = true;
            }
        }
        // Refresh the log file every ~500ms (30 ticks × 16ms) while the modal is open
        if app.log.show && app.tick % 30 == 0 {
            app.refresh_log();
            dirty = true;
        }

        // Poll live call quality every ~1s (60 ticks × 16ms) for the active call.
        // Mark dirty only while a call is up so the stats refresh live; when idle
        // both stay None and we don't force an otherwise-needless repaint.
        if app.tick % 60 == 0 {
            let active = app.in_active_call();
            app.media = if active {
                app.phone.media_stats()
            } else {
                None
            };
            app.codec = if active {
                app.phone.audio_codec()
            } else {
                None
            };
            if active {
                dirty = true;
            }
        }

        if dirty {
            // Wrap the frame in a synchronized-output block so the terminal renders
            // it atomically instead of tearing mid-repaint. Ignored by terminals
            // that don't support it. Issued on the TUI writer (the tty), not fd 1
            // (which is redirected to the log).
            let _ = crossterm::execute!(
                terminal.backend_mut(),
                crossterm::terminal::BeginSynchronizedUpdate
            );
            terminal.draw(|frame| ui::render(frame, app))?;
            let _ = crossterm::execute!(
                terminal.backend_mut(),
                crossterm::terminal::EndSynchronizedUpdate
            );
            dirty = false;
        }

        if ct_event::poll(Duration::from_millis(16))? {
            match ct_event::read()? {
                Event::Key(key) => {
                    app.handle_key(key);
                    dirty = true;
                    if app.quit {
                        break;
                    }
                }
                Event::Resize(_, _) => dirty = true,
                _ => {}
            }
        }

        while let Ok(event) = msg_rx.try_recv() {
            app.handle_message(event);
            dirty = true;
        }

        // Dispatch any remote-control commands through the same path as the
        // command line, replying to the waiting socket connection.
        while let Ok(req) = remote_rx.try_recv() {
            let resp = match app.dispatch(&req.command, &req.params) {
                Ok(data) => crate::control::ControlResponse::ok(data),
                Err(e) => crate::control::ControlResponse::err(e),
            };
            let _ = req.reply.send(resp);
            dirty = true;
        }
        // A remote `shutdown` sets `app.quit` outside the key handler.
        if app.quit {
            break;
        }
    }
    Ok(())
}
