# Using the TUI

Launching a profile opens the ratatui interface: a status bar (registration, MWI),
the call list, and a mode line. Below are the keybindings per mode and overlay.

## Profile picker

| Key | Action |
|-----|--------|
| `Enter` | Start selected profile |
| `Ctrl+N` | Create new profile |
| `Ctrl+E` | Edit selected profile |
| `Ctrl+Y` | Clone selected profile |
| `Ctrl+D` | Delete selected profile (confirmation) |
| `↑` / `↓` | Navigate (wrap-around) |
| `PgUp` / `PgDn` | Move by one screenful |
| `Home` / `End` | First / last profile |
| `Esc` | Quit |

Typing filters the list. Every word has to appear somewhere in the row — in the
profile name or in the subtitle next to it — in any order, so `dev sipgate` finds
a profile named `[DEV] Channel 01` registered at sipgate.

A `●` marks a profile that already has a session running. Starting a second one
registers another contact with the provider, and the provider then decides which
of them a call reaches. A profile whose file cannot be read is marked
`⚠ unreadable` instead of failing only once you start it. `Ctrl+E` on such a
profile opens `profile.toml` in `$EDITOR` rather than the form — a form cannot
be filled from a file that does not parse, and the text is what needs fixing.
Starting it is refused with a note above the list.

If `ringo.toml` itself could not be loaded, that is shown there too. It is worth
saying in the picker specifically: the picker runs before the log file is opened,
so the warning would otherwise reach nobody.

## Normal mode (default)

| Key | Action |
|-----|--------|
| `d` | Enter Dial mode |
| `a` | Accept incoming call |
| `s` | Silence the ringtone (the call keeps ringing) |
| `b` / `Del` | Hang up |
| `h` | Hold / resume the selected call (toggle) |
| `m` | Toggle microphone mute |
| `M` | Toggle deafen (microphone and speaker) |
| `t` / `T` | Blind / attended transfer |
| `0-9` `*` `#` | DTMF tones (during a call) |
| `f` / `Tab` | Open contacts (`Tab` switches calls when several are active) |
| `e` / `l` / `c` | Event log / baresip log / call history |
| `Ctrl+R` | Fuzzy-search dial history |
| `Ctrl+E` | Edit profile (no active call) |
| `Ctrl+P` | Switch profile (back to picker) |
| `:` | Open the command bar |
| `q` / `Ctrl+C` | Quit (with / without confirmation) |

## Dial mode

| Key | Action |
|-----|--------|
| `Enter` | Dial and return to Normal mode |
| `Esc` | Cancel |
| `Backspace` | Delete character / exit when empty |
| `←` `→` / `Home` `End` | Move / jump the cursor |
| `↑` / `↓` | Navigate dial history |
| `Tab` | Open contacts |
| `Ctrl+R` | Fuzzy-search dial history |

## Transfer mode

| Key | Action |
|-----|--------|
| `Enter` | Execute the transfer |
| `Tab` | Open contacts |
| `↑` / `↓` / `Ctrl+R` | Dial-history navigation / search |
| `Esc` | Cancel |

## Contacts overlay

| Key | Action |
|-----|--------|
| `↑` / `↓`, `g` / `G` | Navigate / jump to top-bottom |
| `Enter` | Select number (dial or transfer) |
| `/` | Search |
| `a` / `e` / `d` | Add / edit / delete contact |
| `E` | Open contacts in `$EDITOR` |
| `f` / `Esc` | Close |

## Command bar

Open with `:`. Tab-completes commands; `Enter` runs, `Esc` closes.

Commands: `dial <n>`, `hangup`, `accept`, `hold`, `resume`, `mute`,
`dtmf <digits>`, `transfer <uri>`, `contacts`, `events`, `log`, `history`, `edit`,
`switch`, `help`, `quit`.

## Call history / log views

| Key | Action |
|-----|--------|
| `↑` / `↓`, `g` / `G` | Navigate / jump |
| `Enter` | (history) copy peer to dial input — redial |
| `/` | (history) search |
| `d` / `D` | (history) delete entry / clear all |
| `e` / `l` / `c` | Switch between event log / baresip log / call history |
| `Esc` | Close |
