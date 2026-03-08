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
use std::{io, path::PathBuf, sync::mpsc};
use tokio::{net::TcpStream, sync::mpsc as tokio_mpsc};

use crate::{client, phone::BaresipPhone};

// ─── Main entry point ─────────────────────────────────────────────────────────

pub fn run(
    profile_name: String,
    account_aor: String,
    port: u16,
    baresip_log_path: Option<PathBuf>,
    call_history_path: Option<PathBuf>,
    notify: bool,
    regint: Option<u32>,
    custom_headers: std::collections::HashMap<String, String>,
    theme: crate::config::Theme,
    hooks: Vec<crate::config::Hook>,
    profile: crate::profile::Profile,
    contacts: Vec<crate::contacts::Contact>,
) -> Result<Option<String>> {
    let (msg_tx, msg_rx) = mpsc::channel::<AppEvent>();
    let (cmd_tx, cmd_rx) = tokio_mpsc::channel::<(String, String)>(32);

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;

    // Connect to baresip ctrl_tcp
    let stream = rt.block_on(TcpStream::connect(("127.0.0.1", port)))?;
    let (read_half, write_half) = stream.into_split();

    let phone = BaresipPhone::new(cmd_tx);

    // TCP reader task: baresip → msg_tx (converts BaresipMessage → AppEvent)
    rt.spawn(async move {
        let mut reader = read_half;
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

    // TCP writer task: cmd_rx → baresip
    rt.spawn(async move {
        let mut writer = write_half;
        let mut rx = cmd_rx;
        while let Some((cmd, params)) = rx.recv().await {
            if let Err(e) = client::write_command(&mut writer, &cmd, &params).await {
                crate::rlog!(Error, "tcp writer: {} (cmd={})", e, cmd);
                break;
            }
        }
    });

    // Set up terminal
    crossterm::terminal::enable_raw_mode()?;
    let mut stdout = io::stdout();
    crossterm::execute!(stdout, crossterm::terminal::EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(
        profile_name,
        account_aor,
        baresip_log_path,
        call_history_path,
        notify,
        Box::new(phone),
        theme,
        hooks,
        profile,
        contacts,
    );

    let aor = app.account_aor.clone();
    app.phone.register(&aor, regint.unwrap_or(3600));

    for (key, value) in &custom_headers {
        app.phone.add_header(key, value);
    }

    let mut do_restart = false;
    loop {
        render_loop(&mut terminal, &mut app, &msg_rx)?;

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
        match crate::app::pick_profile() {
            Ok(name) => return Ok(Some(name)),
            Err(_) => {}
        }
    }

    Ok(None)
}

// ─── Render loop ──────────────────────────────────────────────────────────────

fn render_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
    msg_rx: &mpsc::Receiver<AppEvent>,
) -> Result<()> {
    use std::time::Duration;
    loop {
        if app.needs_clear {
            terminal.clear()?;
            app.needs_clear = false;
        }

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
    }
    Ok(())
}
