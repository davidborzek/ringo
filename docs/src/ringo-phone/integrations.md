# Integrations

## Shell completions

Completions are dynamic — profile names complete from your actual profiles under
`~/.config/ringo/profiles/`.

```fish
# fish — ~/.config/fish/config.fish
COMPLETE=fish ringo | source
```

```bash
# bash — ~/.bashrc
source <(COMPLETE=bash ringo)
```

```zsh
# zsh — ~/.zshrc
source <(COMPLETE=zsh ringo)
```

After sourcing, `ringo start <Tab>` completes profile names.

## rofi

```sh
cp scripts/ringo-rofi ~/.local/bin/

# sway / i3
bindsym $mod+p exec ringo-rofi
```

`ringo-rofi` uses `$TERMINAL` if set, otherwise tries `kitty`, `alacritty`,
`foot`, `wezterm`, `xterm`.

## tmux

```sh
cp scripts/ringo-tmux ~/.local/bin/
ringo-tmux
```

`ringo-tmux` uses `fzf` for multi-select profile picking and opens each selected
profile in its own pane within a `ringo` tmux session. Requires `tmux` and `fzf`.

## Call history format

One JSON object per line:

```json
{"ts":"2024-01-15 14:30:05","dir":"outgoing","peer":"sip:alice@example.com","duration":"02:05:13","duration_secs":7513}
```

```sh
cat ~/.config/ringo/profiles/<name>/call_history | jq .
```
