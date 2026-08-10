# Tekst IDE

![Status](https://img.shields.io/badge/status-early--implementation-orange)
[![license](https://img.shields.io/crates/l/tekstide.svg)](LICENSE)
[![crates.io](https://img.shields.io/crates/v/tekstide.svg?label=tekstide)](https://crates.io/crates/tekstide)
[![docs.rs](https://img.shields.io/docsrs/tekstide?version=latest)](https://docs.rs/tekstide)
[![Dependency Status](https://deps.rs/crate/tekstide/latest/status.svg)](https://deps.rs/crate/tekstide)
[![crates.io](https://img.shields.io/crates/v/tekstide-core.svg?label=core)](https://crates.io/crates/tekstide-core)
[![docs.rs](https://img.shields.io/docsrs/tekstide-core?version=latest)](https://docs.rs/tekstide-core)
[![Dependency Status](https://deps.rs/crate/tekstide-core/latest/status.svg)](https://deps.rs/crate/tekstide-core)

Tekst IDE (`tekstide`) is a local-first, multi-project workbench for supervising terminal-based AI development workflows.

## Current Status

The current implementation is a headless core through RFC-013, plus the headless
command-approval model of RFC-021 and the shared text-safety primitive of RFC-016
PR-016-C: terminal runtime, AgentRun launch, transcript retention, generated-change
review, and durable audit storage. As of RFC-015 (now closed, `rfcs/done/`), there is
also a real desktop GUI application shell with mode switching. It includes:

- Project Board and ProjectSession state, root-bound file access, bounded explorer,
  UTF-8 text buffers, and safe save with external-change detection;
- project-owned Linux PTY terminal lifecycle with bounded IO, resize, and
  process-group termination;
- conservative terminal output security policy, paste classification, and
  model-level trusted UI boundaries;
- AI CLI profiles as reviewed launch contracts, with Restricted Mode blocking
  workspace-local executables, wrappers, project-local `PATH`, and implicit CLI
  workspace-config discovery;
- AgentRun launch through project-owned terminals, with honest Plain/Supervised/
  Managed labels and active-file safety before process start;
- bounded local transcript capture with retention limits, per-run opt-out, and purge;
- metadata-only generated-change detection and review-state tracking;
- durable local SQLite audit storage with schema identity, migration harness,
  corruption diagnostics, restart-safe recovery, and explicit purge;
- a real `iced` desktop shell: window/chrome/content/modal layer composition, a
  keyboard-driven focus and input-routing model with a visible, non-colour-only focus
  indicator, i18n-backed text and a compiled theme, and a Project Board surface
  rendering live `ApplicationShell` state with untrusted project names and paths
  escaped, never trusted;
- Content ↔ Terminal mode switching for an active project, with a real,
  user-reachable terminal (`Ctrl+Alt+T`): a security-filtered PTY session
  rendering real output, with a bounded terminal-count limit and exit
  detection so the session bar reflects what is actually running rather than
  what was last launched. RFC-019 still owns the editor/explorer main-area
  content.
- Real clipboard paste into a focused terminal (`Ctrl+Shift+V`, RFC-018),
  routed through the same RFC-009 policy as everything else that reaches a
  PTY: single-line and empty pastes go through and control-containing pastes
  are blocked outright, both without a dialog. A multi-line paste opens a
  real, rendered confirmation dialog — the first trusted dialog this product
  has — showing an escaped preview of the pasted content; accepting is the
  only thing that writes it, and every other way to leave the dialog
  (Escape, or activating Cancel) leaves the terminal untouched.

It is not yet the full AI CLI workbench. There is no editor, no rendered
approval/trust dialogs beyond the paste confirmation above, no adapter-spawn
pathway that would make command approval reachable, no Git-based change
detection, file watcher, or overwrite-confirmation UI, and no cross-platform
evidence beyond Linux. There is **no screen-reader support** — not limited,
not planned, absent for the life of the `iced` substrate decision (RFC-014).
Command approval (below) remains implemented but unreachable. RFC-018's
trusted-UI evidence shows one checkable property distinguishing the real
paste dialog from terminal output imitating it: keystrokes typed while it
is open never reach the terminal, verified with a positive control proving
they were reaching the app. The terminal grid can never draw outside its
own pane, which is architecturally sound but does not mean the dialog
visibly uses that headroom — whether it does depends on pasted-content
width, which is attacker-influenced, so it is not claimed as something a
user can rely on. Nothing here claims an untrained user would notice
either property unprompted.

Durable audit currently records trust decisions, managed AgentRun lifecycle, blocked
root/symlink access, audit-store recovery outcomes, plain-terminal session starts and
terminations, and paste refusals. Restricted-feature, safe-close, configuration-change,
transcript-purge, and project-added producers are defined in the audit schema but not
yet wired. The command-approval producers are wired and tested but produce nothing,
because nothing calls them — see below.

### Command approval

Tekstide implements a command-approval protocol that a cooperating AI CLI adapter can
use: a versioned sideband channel over a per-run Unix domain socket, two-layer peer
authentication, a structural risk classifier, and single-use decisions recorded in the
durable audit trail.

**It is not yet available to users.** No code path spawns an adapter that could speak
the protocol, and there is no dialog to decide in, so nothing reaches it.

**It is cooperative, not enforced.** Approval works only if the adapter asks. Tekstide
does not intercept process execution and has no execution path of its own to withhold,
so an adapter that ignores a rejection — or never submits a proposal — runs its command
regardless. Tekstide does not approve commands, and does not control what an AI CLI can
run.

## Quick Start

Install from crates.io and run:

```sh
cargo install tekstide
tekstide
```

Open one or more local project paths from the command line:

```sh
tekstide /path/to/project
```

### Building from a checkout (contributors)

```sh
cargo run -p tekstide
cargo run -p tekstide -- /path/to/project
```

## Keyboard Reference

The shell is keyboard-navigable by design. These bindings exist today
(`crates/tekstide-core/src/navigation.rs`'s `KeybindingPolicy::linux_mvp()` and
`crates/tekstide/src/input.rs`):

| Binding | Action |
| --- | --- |
| `Ctrl+Alt+P` | Open the Project Board |
| `Ctrl+Alt+M` | Toggle Content / Terminal mode for the active project |
| `Ctrl+Alt+T` | Launch a real terminal in the active project (switches to Terminal mode) |
| `Ctrl+Shift+V` | Paste the clipboard into the focused terminal, subject to RFC-009's policy |
| `Tab` / `Shift+Tab` | Cycle keyboard focus between shell zones |

`Ctrl+Shift+P` is reserved for a command palette that does not exist yet — it is
bound in the keybinding policy but currently does nothing.

`Esc` (dismiss) and `Enter` (activate) work on the shell's modal layer, but nothing
in the shipped application can currently open one — the only modal today is
developer-only scaffolding gated behind an environment variable. Real dialogs
arrive with RFC-022; these bindings currently do nothing a user can reach.

## Local Data and Privacy

Tekstide is local-first: it does not send project data anywhere. On every launch,
the desktop application creates the **recent-projects list**, at
`$XDG_STATE_HOME/tekstide/recent-projects.json`
(`~/.local/state/tekstide/recent-projects.json` if `XDG_STATE_HOME` is unset) —
the paths of projects you have opened, used to restore the Project Board across
sessions. There is no in-app command to clear it yet; delete the file to reset it.
The only other local state it creates is the audit store described next, and only
once you actually open a terminal.

RFC-013's durable audit store is implemented, tested, and has a real producer
(RFC-017, wired end to end as of the terminal-launch-UX handoff):
**pressing `Ctrl+Alt+T` to open a terminal creates this database**, the first
time you do it. Each launch records a `plain_terminal_observation` `Started`
event; if that session later exits (typing `exit`, or the shell dying on its
own), a matching `Terminated` event is recorded too, naming only whether the
process exited or was signalled — never a command, its output, or a path.
This family's schema has no field for any of those at all, so none can ever be
recorded in it, launch or exit. The store lives at
`$XDG_STATE_HOME/tekstide/audit/audit.sqlite3`
(`~/.local/state/tekstide/audit/audit.sqlite3` if `XDG_STATE_HOME` is unset).
**This is no longer gated behind a developer-only flag** — `TEKSTIDE_TERMINAL_DEMO`
still exists for diagnostic use, but the real `Ctrl+Alt+T` binding is what
ordinary use reaches, and it opens the same store.

Pasting into a terminal (`Ctrl+Shift+V`, RFC-018) writes to the same store when the
paste is refused: a `paste_blocked` event, naming only that a paste was blocked and
which project/terminal it was aimed at — never the pasted content, the clipboard
text, or the command it would have produced. This family's schema has no field for
any of those either. A paste the policy *allows* is not audited at all; only
refusals are, a known and disclosed limitation of the schema rather than an
oversight — see the RFC for why.

There is no in-app command to purge the audit store yet; delete the `audit/`
directory to reset it, or see
[`rfcs/done/013-durable-audit-store-and-local-data-policy.md`](rfcs/done/013-durable-audit-store-and-local-data-policy.md)
for the store's full retention and purge policy.

RFC-011's transcript retention is implemented and tested, but **is not wired
into the desktop application yet** — running `tekstide` does not retain any
transcripts. See
[`rfcs/done/011-transcript-retention-and-local-data-policy.md`](rfcs/done/011-transcript-retention-and-local-data-policy.md)
for the retention and purge policy that applies once it is.

For a consolidated list of what else is missing or deferred, see
[`rfcs/future-work.md`](rfcs/future-work.md).

## RFCs

Implemented foundation RFCs live under [`rfcs/done/`](rfcs/done/).

Release scope and deferred work are tracked in [`rfcs/done/001-product-scope-mvp-and-non-goals.md`](rfcs/done/001-product-scope-mvp-and-non-goals.md), [`CHANGELOG.md`](CHANGELOG.md), [`ROADMAP.md`](ROADMAP.md), and [`rfcs/future-work.md`](rfcs/future-work.md).
