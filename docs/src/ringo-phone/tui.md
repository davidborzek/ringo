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
| `b` / `Del` | Hang up, or reject a ringing incoming call (`reject_code`, default 486) |
| `h` | Hold / resume the selected call (toggle) |
| `m` | Toggle microphone mute |
| `M` | Toggle deafen (microphone and speaker) |
| `t` / `T` | Blind / attended transfer; `t` on a ringing incoming call deflects it (302) |
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

Also used to deflect a ringing call: the caller gets a `302 Moved Temporarily`
with the target as `Contact` and your account as `Diversion`. Whether the call
actually follows is up to your provider.

| Key | Action |
|-----|--------|
| `Enter` | Execute the transfer / deflection |
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
| `f` / `Esc` | Close |

## Command bar

Open with `:`. Tab-completes commands; `Enter` runs, `Esc` closes.

Commands: `dial <n>`, `hangup`, `accept`, `hold`, `resume`, `mute`,
`dtmf <digits>`, `transfer <uri>`, `contacts`, `events`, `log`, `history`, `edit`,
`switch`, `mouse`, `help`, `quit`.

## Call history / log views

| Key | Action |
|-----|--------|
| `↑` / `↓`, `g` / `G` | Navigate / jump |
| `Enter` | (history) copy peer to dial input — redial |
| `/` | (history) search |
| `d` / `D` | (history) delete entry / clear all |
| `e` / `l` / `c` | Switch between event log / baresip log / call history |
| `Esc` | Close |

## Mouse

Mouse support is off by default; enable it with `mouse = true` under `[ui]` (see
[Configuration](configuration.md#mouse)) or toggle it with `:mouse`. A click does
what the matching key would, so it behaves the same in every context — `b` still
rejects a ringing call and hangs up an active one.

| Where | Click | Wheel |
|-------|-------|-------|
| Phone, hint bar | Runs the hint (`d dial`, `b reject`, …) | — |
| Phone, call list | Selects the call (as `Tab` does) | — |
| Phone, status bar | Registration indicator re-registers when it is failed or unregistered | — |
| Phone, dial / DTMF line | Opens the dial pad (see below) | — |
| Call history, contacts, dial-history search | Selects the row; double-click presses `Enter` | Scrolls |
| Event log, baresip log | Hints only; rows are not selectable | Scrolls |
| Confirmation popups | `Cancel` / confirm button | — |
| Profile picker | Row selects, double-click starts it; hints run their key | Moves the selection |
| Profile form | Tab labels switch the tab; fields take focus; toggles and selects change; submenus open; `Save` / `Cancel` | Moves the focus |
| Form submenu: headers | Row (key or value half) takes focus; double-click does the same | Moves the focus |
| Form submenu: codecs | Row takes focus; double-click toggles it on or off | Moves the focus |

The dial pad is mouse-only. Click the dial line: while idle it opens the dial
input and the pad together, while dialling it adds the pad to the number typed so
far, and during a call it sends DTMF (the line shows the digits sent). The pad
adds `+`, `⌫` and `Call` while dialling. It closes with `Esc` or a click beside it,
keeping the number, and on its own once the call is placed or ends.

A click beside an overlay (help, logs, call history, contacts, call details,
dial-history search) or a form submenu (headers, codecs, rename) closes it, as
`Esc` would. Confirmations and the contact form only close through their own
buttons or keys. A click anywhere outside the open command bar leaves it, without
also acting on what was clicked.

While a text input is open (dial, transfer, command bar) only the hints for
`Enter`, `Esc`, `Tab` and `Ctrl+R` are clickable; clicking a character hint would
otherwise type it.

A click acts when the button is released, so dragging over a hint or row selects
text instead of pressing it.

Inside tmux, mouse events only reach ringo with `set -g mouse on` in `tmux.conf`.

### Selecting and copying

Drag with the left button to select text on ringo's screen (in the phone, the
picker, the profile form and its popups). The selection runs from where you
pressed to where you are, wrapping across lines like a terminal's. On release the
text is copied to the system clipboard and stays highlighted until the next key,
click or resize; the phone also shows `Copied N chars` in the bottom line.
Trailing spaces are trimmed and at most 100 KB are copied.

Copying uses the OSC 52 escape sequence, so it needs a terminal that supports it
(most do; some VTE-based terminals ignore it and receive nothing). Inside tmux
add `set -g set-clipboard on` to `tmux.conf`. Where OSC 52 is not available,
hold `Shift` (usually `Option` on macOS terminals) while dragging to use the
terminal's own selection instead.
