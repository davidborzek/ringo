use anyhow::{Result, bail};
use crossterm::{
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use std::{fs, io};

use crate::profile;

pub fn run(name: Option<String>, notify: bool) -> Result<()> {
    let mut current = match name {
        Some(n) => n,
        None => pick_profile()?,
    };

    loop {
        match run_one(&current, notify)? {
            Some(next) => current = next,
            None => return Ok(()),
        }
    }
}

fn run_one(name: &str, notify: bool) -> Result<Option<String>> {
    crate::log::init(name);

    let dir = profile::profile_dir(name)?;
    if !dir.join("profile.toml").exists() {
        bail!("Profile '{}' not found.", name);
    }

    let prof = profile::load(name)?;
    let instance = crate::baresip::Instance::spawn(name, &prof)?;

    let contacts = crate::contacts::load();

    let config = crate::config::load();
    crate::hooks::run(
        &config.hooks,
        crate::config::HookEvent::ProfileLoaded,
        name,
        &prof,
        serde_json::json!({}),
    );
    let theme = config.theme;
    let hooks = config.hooks;
    crate::tui::run(
        name.to_string(),
        prof.aor(),
        instance.port,
        Some(instance.log_path.clone()),
        Some(dir.join("call_history")),
        notify && prof.notify,
        prof.regint,
        prof.custom_headers.clone(),
        theme,
        hooks,
        prof,
        contacts,
    )
}

/// Open the interactive profile picker; loops until a profile is selected to start.
/// Manages the terminal lifecycle; stays in alternate screen on success so the
/// TUI can take over seamlessly.
pub fn pick_profile() -> Result<String> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = pick_profile_loop(&mut terminal);

    if result.is_err() {
        let _ = disable_raw_mode();
        let _ = execute!(terminal.backend_mut(), LeaveAlternateScreen);
    }

    result
}

fn pick_profile_loop(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<String> {
    use crate::picker::{PickerAction, PickerItem};
    let mut focus: Option<String> = None;
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

fn open_settings(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
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
