# Configuration

Tekstide reads `$XDG_CONFIG_HOME/tekstide/config.toml` (or `~/.config/tekstide/config.toml`) at
startup.

**A missing file is normal.** An invalid one still starts the application with built-in defaults
— the Project Board then says the file was ignored and names the key that broke it.
`Ctrl+Alt+C` re-reads the file without restarting.

## The settings that take effect

```toml
[agent]
default_profile = "my-cli"          # which AI CLI the launch button runs
transcript_retention_days = 30      # recorded on each run's policy; see the caveat below

[agent.profile.my-cli]
display_name = "My AI CLI"
command = "/usr/local/bin/my-cli"   # absolute path, or a bare name found on the system path

[resources]
agent_run_limit = 3                 # per project; omit for no limit

[keybindings]
open_help = "Ctrl+Alt+J"            # rebind a shortcut; see below
```

## Keybindings

`[keybindings]` takes one entry per action you want to move: the action's name on the left, the
chord on the right, **spelled the way the Help window (`Ctrl+Alt+K`) and `tekstide --help` print
it**. Case does not matter (`ctrl+alt+j` works) and the chord is read back in the canonical
spelling. `Ctrl+Alt+C` applies a change without a restart.

What a chord can be: `Ctrl` and/or `Alt`, optionally `Shift`, then one letter `A`–`Z` or digit
`0`–`9`. **It needs `Ctrl` or `Alt`** — a bare letter would take that key away from typing in the
terminal and the editor — and `Shift` with a digit is refused, because `Shift+1` is a different
character on different keyboard layouts and would never reliably match. `Tab`, arrows, `Enter` and
the function keys are not rebindable.

**A bad entry falls back on its own, and the Project Board says which and why.** The rest of the
file still applies. The default stands whenever an entry is refused:

- the chord is not one Tekstide understands, or the value is not a string;
- the action is **reserved** (`open_command_palette`, held for a command palette that does not
  exist) or **has no key at all** (below);
- the chord is **`Ctrl+Shift+P`**, which is reserved;
- the chord **already reaches another action**. There is no "last one wins": if two entries ask
  for the same chord, **both** are refused, so what happens never depends on the order you wrote
  them in. Swapping two actions' chords is fine.

Nothing about *how* a key is handled changes: a global shortcut still wins over a focused
terminal, and a dialog still swallows every key but its own.

| Action (the name to use) | Default chord |
| --- | --- |
| `open_project_board` | `Ctrl+Alt+P` |
| `open_project_entry_field` | `Ctrl+Alt+O` |
| `switch_active_project` | `Ctrl+Alt+N` |
| `toggle_project_mode` | `Ctrl+Alt+M` |
| `launch_terminal` | `Ctrl+Alt+T` |
| `paste_into_terminal` | `Ctrl+Shift+V` |
| `save_active_document` | `Ctrl+S` |
| `launch_agent_run` | `Ctrl+Alt+A` |
| `open_current_agent_run_detail` | `Ctrl+Alt+R` |
| `open_approval_history` | `Ctrl+Alt+H` |
| `open_trust_settings` | `Ctrl+Alt+U` |
| `open_diff_review` | `Ctrl+Alt+D` |
| `open_help` | `Ctrl+Alt+K` |
| `open_folder_browser` | `Ctrl+Alt+B` |
| `reload_configuration` | `Ctrl+Alt+C` |

**Two actions have no chord and cannot be rebound**, and neither is an oversight:
`cycle_visible_terminal_session` (there is no handler — a new terminal becomes the one that receives
keystrokes, and no existing one can be brought back), and `open_safe_close_dialog` (the
close-project dialog is reached from a project tab's close button and from `Delete` on a focused
tab; a third route was not built). Naming either in the file is refused with that explanation.

**A configuration file inside a project is never read.** The file above is the only one, and it is
in *your* configuration directory: a repository cannot rebind a key.

## Why the file is this narrow

**Every other key is refused by name, and the file is reported as ignored.** A much larger schema
was designed — fonts, theme, scrollback, concurrency limits — and none of those settings has code
that reads them yet (keybindings do; see above). **A key the file accepts and the product ignores is a lie you
read**, so each is refused until the feature that honours it exists, and each returns to the file
in the same change as that feature.

A key Tekstide has simply never heard of — a typo, or one from a newer version — loads with a
**warning** instead, named on the board, because then nothing is wrong with the file itself. The
difference is deliberate: a refusal means *this key was real and was withdrawn*; a warning means
*this key was never real here*.

## Nothing a configuration file defines runs without a deliberate act

The first launch of a configuration-defined AI CLI asks first, and the confirmation names the
**executable path it resolved to** and the file that defined it — not the `display_name`, which
is text the file itself supplies. Once per session, not once per launch.

A reload that **weakens** a security-relevant setting asks the same way. One that **tightens**
applies immediately.

## Three things configuration can never do

The file is refused if it asks for any of them:

- grant workspace trust — that is a per-project act, `Ctrl+Alt+U`;
- disable multiline-paste confirmation;
- disable destructive-command approval.

## Caveat on `transcript_retention_days`

The value is the age at which a transcript is deleted, measured from its last write. Removal
happens when you open the project or launch an AI CLI run in it, and at no other time — there is no
timer. **The first run after upgrading deletes every transcript already older than this value**,
which for a default 30-day setting can be every transcript from more than a month ago.

**`0` is refused.** It is not a retention period: it would have meant *keep nothing*, and the
product would have enforced the opposite — no age limit at all. To keep no transcripts for a
project, decline transcript capture for it in Trust Settings. A file containing `0` starts
Tekstide with built-in defaults and a diagnostic naming the key, like any other invalid value.

The only purge is the manual, per-project one on the Trust Settings surface — see
[Local data and privacy](./local-data-and-privacy.md).
