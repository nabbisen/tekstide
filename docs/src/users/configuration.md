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
```

## Why the file is this narrow

**Every other key is refused by name, and the file is reported as ignored.** A much larger schema
was designed — fonts, theme, keybindings, scrollback, concurrency limits — and none of those
settings has code that reads them. **A key the file accepts and the product ignores is a lie you
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

The value is recorded on each run's transcript policy and checked for validity, but **no
age-based purge reads it yet**. Transcripts are **not** kept for that many days and then removed.

The only purge is the manual, per-project one on the Trust Settings surface — see
[Local data and privacy](./local-data-and-privacy.md).
