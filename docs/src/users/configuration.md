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

[theme]
background = "#0B1F2A"              # colours as #RRGGBB; see "Colours and text" below

[font]
family = "DejaVu Serif"             # the name of an installed font family, never a file
body_size = 16                      # pixels, 8 to 32

[terminal]
scrollback_lines = 5000             # lines of history a terminal keeps; at most 12,000

[explorer]
show_ignored = false                # draw the entries Git says are ignored; default false
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

## Colours and text

`[theme]` restyles the window and `[font]` sets the type. `Ctrl+Alt+C` applies both without a
restart, and removing a line and reloading puts the default back.

```toml
[theme]
background       = "#0B1F2A"   # the window
foreground       = "#F5F0E1"   # text
surface_elevated = "#12303F"   # the bar, dialogs and cards text sits on
accent           = "#FFB000"   # the border of a dialog
border_default   = "#7A8A93"
border_focused   = "#FFB000"
scrim            = "#00000099" # the dimming behind a dialog; the only colour that can be transparent

[font]
family       = "DejaVu Serif"  # an installed family, by name
body_size    = 16              # ordinary text
heading_size = 20
status_size  = 13              # badges and the status bar
```

A colour is `#` and six hex digits (`#RRGGBB`; eight for the scrim, `#RRGGBBAA`). Nothing else —
no colour names, no `rgb()`.

**You cannot configure the window into something you cannot read or navigate.** Text has to be
readable on the two surfaces it sits on: `foreground` against `background`, and `foreground` against
`surface_elevated`, each at **4.5:1** (WCAG AA). And **the focus border has to be visible** — the
border that shows which part of the window has the keyboard — so `border_focused` is held to **3:1**
(WCAG's minimum for interface components) against both surfaces. A colour that would take either below that is not
used, the default stands, and the Project Board says **the ratio it measured** — *"Its contrast with
theme.foreground is 1.25:1, below the 4.5:1 that keeps text readable."* A colour you did not set is
measured at its default, so changing only `background` to white is measured against the default
`foreground`, and fails. When you set `background` and `foreground` for a light theme, **set `surface_elevated` and
`border_focused` too**, or the default dark surface and the default blue focus border will fail
against your light colours — and if taking one colour back makes another fail, that one is taken
back as well, each named.

**What is not measured, and why.** `accent` and `border_default` are decoration: whatever they
are, the words are still there and the focus border is measured, so you may choose them freely.
The scrim is not measured for contrast but **is capped in opacity**: it is the dimming layer
behind a dialog, and a fully opaque one would look like a window this application did not draw,
so anything above 90 % opaque is reduced to 90 % and the board says so. (The number is a
judgement: the reason for it is that a dialog's backdrop must let the window behind it show
through faintly, or the dialog stops being distinguishable from a screen this application did not
draw.) Buttons are drawn from
these same colours (`surface_elevated`, `foreground`, the borders), so they follow your theme.

**A size outside 8–32 pixels is not used.** The three sizes fall back independently.

**A font is a name, not a file.** `family` is compared with the family names of the fonts installed
on this machine — the same list the window is drawn from — and nothing you write is ever opened as
a font file. A path (`/usr/share/fonts/…`, anything with a slash) is refused as "not a name"; a name
no installed font has is refused as "not installed"; either way the default face stands and the
board says so. Case does not matter (`dejavu serif` finds `DejaVu Serif`). The family applies to
the interface's own text, including the editor's; **the terminal and the file tree keep a
fixed-width face**, because a column of code has to line up whatever you read prose in.

Every one of these falls back **on its own**: one bad colour does not cost you the rest of the
file.

The word `[ui]` in an older draft of this schema is not read; its settings are `[theme]` and
`[font]`.

## Terminal scrollback

`[terminal] scrollback_lines` is how many lines of history each terminal keeps above what is on
screen. The default is 2,000 lines. `Ctrl+Alt+C` applies a change to terminals that are already
open — history above the new number is dropped from the oldest end — as well as to ones opened
later.

The most it can be is **12,000 lines**, and a bigger number is reduced to that with a note on the
Project Board. The reason is memory: a terminal keeps its scrollback in memory, a terminal in
another project keeps filling while you are not looking at it, and one terminal should not be able
to take more than **64 MiB**. The cap was measured rather than chosen:

| A terminal this wide | holds this much at 12,000 lines |
| --- | --- |
| 80 columns | 22.6 MiB |
| 200 columns | 55.7 MiB |
| 400 columns | 110.8 MiB — so it keeps fewer, about 5,000 lines |

Memory is per column, so a **wider terminal keeps fewer lines** whenever the number you asked for
would pass 64 MiB at that width — and it gets them back when you narrow it. At 200 columns (a
full-width terminal on an ordinary display) the full 12,000 fit. These figures are for ordinary
output. **A 12,000-line terminal at 200 columns can reach roughly 200 MB if every cell carries a
combining mark**, and the 64 MiB is not a promise about that.

A value that is not a whole number of lines (`-1`, `2000.5`, `"lots"`) is not used, and the
default stands.

## The file explorer

`[explorer] show_ignored` is `false` by default. Inside a Git repository the explorer asks Git which entries are
ignored and, with this off, **does not draw ignored files and says how many it left out** (*2 ignored files hidden*).
**An ignored folder keeps its row** — collapsed, marked `[ignored]`, and still openable — because hiding a place would
take away the way in. `true` draws the ignored files as well, each with an `[ignored]` word. It governs **ignored
entries only** — `.env`, `.gitignore` and every other dotfile are ordinary rows whichever way it is set. A value that
is not `true` or `false` is not used, the default stands, and the Project Board says so. `Ctrl+Alt+C` applies a
change to projects that are already open.

**What "ignored" means here is Git's answer**, not Tekstide's reading of a `.gitignore`: it is asked about the
entries of each folder as it is read, so the marks are exactly as old as that read. Git is run **without your
personal Git configuration** — the same restriction that keeps a repository from making Tekstide run a program — so
a global ignore file named by `core.excludesFile` is not applied. A repository whose own configuration names
something Tekstide does not vouch for (a hook-like setting such as `core.fsmonitor`) is not asked at all, and the
built-in list decides; so does a repository rooted at your home directory, because a dotfiles repository that
ignores everything would otherwise hide every project under it.

## Why the file is this narrow

**Every other key is refused by name, and the file is reported as ignored.** A much larger schema
was designed — scrollback, concurrency limits, and more — and settings without code that reads
them yet are refused (keybindings, colours and fonts have it; see above). **A key the file accepts and the product ignores is a lie you
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
