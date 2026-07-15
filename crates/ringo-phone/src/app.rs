use anyhow::{Result, bail};
use crossterm::{
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use std::{fs, io};

use crate::backend::Backend;
use crate::profile;

pub fn run(name: Option<String>, notify: bool, headless: bool) -> Result<()> {
    // Clean up registry entries from sessions that are no longer reachable.
    crate::control::reap_stale();

    let mut current = match name {
        Some(n) => n,
        None if headless => bail!("--headless requires a profile name"),
        None => pick_profile(None)?,
    };

    loop {
        match run_one(&current, notify, headless)? {
            Some(next) => current = next,
            None => return Ok(()),
        }
    }
}

fn run_one(name: &str, notify: bool, headless: bool) -> Result<Option<String>> {
    // Backend log goes to the XDG state dir (e.g. ~/.local/state/ringo/<name>.log),
    // not /tmp. init_file creates the dir; a bad path just leaves logging silent.
    let log_path = profile::state_dir()?.join(format!("{name}.log"));
    crate::log::init_file(&log_path);

    let dir = profile::profile_dir(name)?;
    if !dir.join("profile.toml").exists() {
        bail!("Profile '{}' not found.", name);
    }

    let prof = profile::load(name)?;
    let config = crate::config::load();

    let account = account_from(&prof);
    let options = backend_options(&config.baresip);

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;

    let session =
        crate::backend::BaresipBackend.spawn_session(rt.handle(), name, &account, &options)?;

    let control_socket = crate::control::socket_path(name)?;

    let contacts = crate::contacts::load();

    crate::hooks::run(
        &config.hooks,
        crate::config::HookEvent::ProfileLoaded,
        name,
        &prof,
        serde_json::json!({}),
    );
    let theme = config.theme;
    let hooks = config.hooks;
    let params = crate::tui::SessionParams {
        profile_name: name.to_string(),
        account_aor: prof.aor(),
        log_path,
        session,
        control_socket,
        call_history_path: Some(dir.join("call_history")),
        notify: notify && prof.notify,
        regint: prof.regint,
        custom_headers: prof.custom_headers.clone(),
        theme,
        hooks,
        profile: prof,
        contacts,
    };

    if headless {
        crate::tui::run_headless(rt, params)?;
        Ok(None)
    } else {
        crate::tui::run(rt, params)
    }
}

/// Map a ringo profile to the backend-neutral account the engine registers.
fn account_from(p: &profile::Profile) -> crate::account::Account {
    crate::account::Account {
        username: p.username.clone(),
        domain: p.domain.clone(),
        password: p.password.clone(),
        display_name: p.display_name.clone(),
        transport: p.transport.clone(),
        auth_user: p.auth_user.clone(),
        outbound: p.outbound.clone(),
        stun_server: p.stun_server.clone(),
        media_enc: p.media_enc.clone(),
        regint: p.regint,
        mwi: p.mwi,
        dtmf_mode: None, // ringo-phone uses baresip's default (real, clocked audio device)
        catchall: p.catchall,
        audio_codecs: p.audio_codecs.clone(),
    }
}

/// Map ringo's `[baresip]` config section to the engine's backend options.
fn backend_options(c: &crate::config::BaresipConfig) -> crate::account::BackendOptions {
    crate::account::BackendOptions {
        audio_driver: c.audio_driver.clone(),
        audio_player_device: c.audio_player_device.clone(),
        audio_source_device: c.audio_source_device.clone(),
        audio_alert_device: c.audio_alert_device.clone(),
        sip_cafile: c.sip_cafile.clone(),
        sip_capath: c.sip_capath.clone(),
        user_agent: Some(concat!("ringo-phone/", env!("CARGO_PKG_VERSION")).into()),
        extra: c
            .extra
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect(),
        ..Default::default()
    }
}

/// Open the interactive profile picker; loops until a profile is selected to start.
/// Manages the terminal lifecycle; stays in alternate screen on success so the
/// TUI can take over seamlessly.
pub fn pick_profile(focus: Option<&str>) -> Result<String> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(Box::new(stdout) as crate::tui::TermWriter);
    let mut terminal = Terminal::new(backend)?;
    // Clear explicitly: when the phone TUI hands over on a profile switch we stay
    // in the alternate screen (no leave/re-enter to wipe it), so the picker must
    // clear any leftover frame itself.
    terminal.clear()?;

    let result = pick_profile_loop(&mut terminal, focus);

    if result.is_err() {
        let _ = disable_raw_mode();
        let _ = execute!(terminal.backend_mut(), LeaveAlternateScreen);
    }

    result
}

fn pick_profile_loop(
    terminal: &mut crate::tui::Term,
    initial_focus: Option<&str>,
) -> Result<String> {
    use crate::picker::{PickerAction, PickerItem};
    let mut focus: Option<String> = initial_focus.map(|s| s.to_string());
    loop {
        let config = crate::config::load();
        let theme = &config.theme;
        let names = profile::list_names().unwrap_or_default();
        let items: Vec<PickerItem> = names
            .iter()
            .map(|name| {
                let subtitle = profile::load(name)
                    .map(|p| build_subtitle(&p, &config.picker.info))
                    .unwrap_or_default();
                PickerItem {
                    name: name.clone(),
                    subtitle,
                }
            })
            .collect();
        match crate::picker::run(terminal, &items, theme, focus.as_deref())? {
            PickerAction::Start(name) => return Ok(name),
            PickerAction::New => {
                if let Some((name, p)) = crate::form::run_form(
                    terminal,
                    None,
                    &profile::Profile::default(),
                    &names,
                    theme,
                )? {
                    profile::save(&name, &p)?;
                    focus = Some(name);
                }
            }
            PickerAction::Clone(source) => {
                let current = profile::load(&source)?;
                if let Some((name, p)) =
                    crate::form::run_form(terminal, None, &current, &names, theme)?
                {
                    profile::save(&name, &p)?;
                    focus = Some(name);
                }
            }
            PickerAction::Edit(name) => {
                let current = profile::load(&name)?;
                if let Some((_, p)) =
                    crate::form::run_form(terminal, Some(&name), &current, &[], theme)?
                {
                    profile::save(&name, &p)?;
                }
                focus = Some(name);
            }
            PickerAction::Settings => {
                open_settings(terminal)?;
            }
            PickerAction::Rename(name) => {
                if let Some(new_name) = crate::form::run_rename(terminal, &name, &names, theme)? {
                    profile::rename(&name, &new_name)?;
                    focus = Some(new_name);
                } else {
                    focus = Some(name);
                }
            }
            PickerAction::Delete(name) => {
                if crate::form::run_confirm(terminal, &name, theme)? {
                    fs::remove_dir_all(profile::profile_dir(&name)?)?;
                }
            }
        }
    }
}

fn open_settings(terminal: &mut crate::tui::Term) -> Result<()> {
    use crossterm::terminal::{
        EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
    };
    use std::process::Command;

    let config_path = crate::config::config_path()
        .ok_or_else(|| anyhow::anyhow!("Cannot determine config path"))?;

    if !config_path.exists() {
        fs::create_dir_all(config_path.parent().unwrap())?;
        fs::write(
            &config_path,
            "# ringo configuration\n# See: https://github.com/davidborzek/ringo#configuration\n",
        )?;
    }

    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".into());

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;

    let status = Command::new(&editor).arg(&config_path).status();

    enable_raw_mode()?;
    execute!(terminal.backend_mut(), EnterAlternateScreen)?;
    terminal.clear()?;

    status
        .map(|_| ())
        .map_err(|e| anyhow::anyhow!("Failed to open editor '{}': {}", editor, e))
}

fn build_subtitle(profile: &profile::Profile, fields: &[String]) -> String {
    fields
        .iter()
        .filter_map(|f| match f.as_str() {
            "aor" => Some(profile.aor()),
            "username" => Some(profile.username.clone()),
            "domain" => Some(profile.domain.clone()),
            "display_name" => profile.display_name.clone().filter(|s| !s.is_empty()),
            "transport" => Some(
                profile
                    .transport
                    .as_deref()
                    .unwrap_or("default")
                    .to_string(),
            ),
            "notes" => profile.notes.clone().filter(|s| !s.is_empty()),
            "auth_user" => profile.auth_user.clone().filter(|s| !s.is_empty()),
            "outbound" => profile.outbound.clone().filter(|s| !s.is_empty()),
            "stun_server" => profile.stun_server.clone().filter(|s| !s.is_empty()),
            "media_enc" => Some(profile.media_enc.as_deref().unwrap_or("none").to_string()),
            _ => None,
        })
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("  ·  ")
}
