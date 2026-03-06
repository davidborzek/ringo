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
    let dir = profile::profile_dir(name)?;
    if !dir.join("profile.toml").exists() {
        bail!("Profile '{}' not found.", name);
    }

    let prof = profile::load(name)?;
    let instance = crate::baresip::Instance::spawn(name, &prof)?;

    let theme = crate::config::load().theme;
    crate::tui::run(
        name.to_string(),
        prof.aor(),
        instance.port,
        Some(instance.log_path.clone()),
        Some(dir.join("call_history")),
        notify && prof.notify,
        prof.regint,
        prof.custom_headers,
        theme,
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
    let config = crate::config::load();
    let theme = &config.theme;
    loop {
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
        match crate::picker::run(terminal, &items, theme)? {
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
                }
            }
            PickerAction::Clone(source) => {
                let current = profile::load(&source)?;
                if let Some((name, p)) =
                    crate::form::run_form(terminal, None, &current, &names, theme)?
                {
                    profile::save(&name, &p)?;
                }
            }
            PickerAction::Edit(name) => {
                let current = profile::load(&name)?;
                if let Some((_, p)) =
                    crate::form::run_form(terminal, Some(&name), &current, &[], theme)?
                {
                    profile::save(&name, &p)?;
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
