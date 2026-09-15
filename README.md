<img src="https://raw.githubusercontent.com/nabbisen/tekstide/main/assets/tekstide-logo.png" alt="Tekst IDE" width="160">

# Tekst IDE

![Status](https://img.shields.io/badge/status-early--implementation-orange)
[![license](https://img.shields.io/crates/l/tekstide.svg)](https://github.com/nabbisen/tekstide/blob/main/LICENSE)
[![crates.io](https://img.shields.io/crates/v/tekstide.svg?label=tekstide)](https://crates.io/crates/tekstide)
[![docs.rs](https://img.shields.io/docsrs/tekstide?version=latest)](https://docs.rs/tekstide)
[![Dependency Status](https://deps.rs/crate/tekstide/latest/status.svg)](https://deps.rs/crate/tekstide)
[![crates.io](https://img.shields.io/crates/v/tekstide-core.svg?label=core)](https://crates.io/crates/tekstide-core)
[![docs.rs](https://img.shields.io/docsrs/tekstide-core?version=latest)](https://docs.rs/tekstide-core)
[![Dependency Status](https://deps.rs/crate/tekstide-core/latest/status.svg)](https://deps.rs/crate/tekstide-core)

Tekst IDE (`tekstide`) is a local-first, multi-project workbench for supervising terminal-based AI development workflows.

**Documentation: [the Tekstide book](https://nabbisen.github.io/tekstide/).** Start with
[What works today](https://nabbisen.github.io/tekstide/users/what-works-today.html), which states the limits as plainly as the
capabilities.

## What it does

- **Several projects at once**, each with root-bound file access, a read-only explorer, a text
  editor that will not silently overwrite a file changed on disk, and its own tab.
- **Real terminals per project**, behind a conservative output-security policy, with a confirmation
  dialog for any multi-line paste.
- **AI CLI runs under workspace trust.** A project starts Restricted, and launching an AI CLI takes a
  deliberate grant. Each run's output is captured to a bounded local transcript.
- **Review what a run changed**: the files it touched, and their content as it is now.
- **A durable local audit trail** of trust decisions, terminal sessions, refused pastes, and more.

## What it does not do yet

It is early. Read these before relying on it:

- **Linux only.** There is no evidence for any other platform.
- **No screen-reader support**, for the life of the `iced` substrate decision.
- **The real Claude Code CLI has never been exercised by this project's tests.** Every automated
  proof uses a controlled test executable.
- **Command approval is built but unreachable.** No shipping AI CLI speaks its protocol, and it is
  cooperative, not enforced.
- **No before/after diff, and no undo in the editor.** Terminal input latency is not verified
  against its target.

The full list, with the reasons, is [What works today](https://nabbisen.github.io/tekstide/users/what-works-today.html). Deferred
work is tracked in [Deferred work](https://nabbisen.github.io/tekstide/contributors/future-work.html).

## Quick Start

```sh
cargo install tekstide
tekstide /path/to/project
```

`tekstide` with no argument opens an empty Project Board with a field to type or paste a path into,
and a **Browse...** button for choosing a folder. A project you opened before is remembered on the
board. You can open several at once:

```sh
tekstide /path/to/project /path/to/another
```

`tekstide --help` prints usage and every key binding. More in
[Getting started](https://nabbisen.github.io/tekstide/users/getting-started.html).

### Building from a checkout

```sh
cargo run -p tekstide
cargo run -p tekstide -- /path/to/project
```

## Keys worth knowing first

| Binding | Action |
| --- | --- |
| `Ctrl+Alt+P` | Open the Project Board |
| `Ctrl+Alt+T` | Launch a terminal in the active project |
| `Ctrl+Alt+M` | Toggle Content / Terminal mode |
| `Ctrl+Alt+U` | Open Workspace Trust for the active project |
| `Ctrl+Alt+K` | Open the keyboard reference, from anywhere |

Everything else is in `tekstide --help`, the in-app Help modal, and the
[Keyboard reference](https://nabbisen.github.io/tekstide/users/keyboard-reference.html).

## Local data and privacy

Tekstide is local-first: **it does not send project data anywhere.** It writes under
`$XDG_STATE_HOME/tekstide` (`~/.local/state/tekstide` if `XDG_STATE_HOME` is unset):

- **`recent-projects.json`**: the projects you opened, and a display-only hint of their trust state;
- **an audit store**, created the first time you open a terminal;
- **transcripts of AI CLI runs**, containing whatever the AI CLI printed — **including anything it
  quoted from your files.** Plain terminals are not recorded. Capture is bounded: 32 MiB per
  transcript, 256 MiB per project, 1 GiB overall.

Trust Settings (`Ctrl+Alt+U`) can decline capture for a project's future runs and purge its
transcripts, **including those from earlier runs**. Purge leaves two kinds of file in place: a
transcript that was still being written when the project opened, and transcripts whose project is no
longer in the recent list. **Deleting the `transcripts/` directory removes everything.** A retention
age can be configured, but **nothing removes a transcript because of its age yet**.

What each file can contain, and how to remove it:
[Local data and privacy](https://nabbisen.github.io/tekstide/users/local-data-and-privacy.html).

## Configuration

`$XDG_CONFIG_HOME/tekstide/config.toml` (or `~/.config/tekstide/config.toml`) is optional and
deliberately narrow: four settings take effect, and nothing a configuration file defines runs without
a deliberate act. See [Configuration](https://nabbisen.github.io/tekstide/users/configuration.html).

## Project

- [Changelog](https://github.com/nabbisen/tekstide/blob/main/CHANGELOG.md): what shipped in each release, and what each release does not do.
- [Roadmap](https://github.com/nabbisen/tekstide/blob/main/ROADMAP.md) and the [RFC index](https://github.com/nabbisen/tekstide/blob/main/rfcs/README.md).
- [Contributing](https://github.com/nabbisen/tekstide/blob/main/CONTRIBUTING.md): the gate, the RFC lifecycle, and the evidence conventions.

Licensed under the terms in [LICENSE](https://github.com/nabbisen/tekstide/blob/main/LICENSE); see also [NOTICE](https://github.com/nabbisen/tekstide/blob/main/NOTICE).
