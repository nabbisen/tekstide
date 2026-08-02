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
- Content ↔ Terminal mode switching for an active project, with sidebar and
  main-area scaffolding that RFC-017 (terminal) and RFC-019 (editor/explorer) render
  real content into.

It is not yet the full AI CLI workbench. There is no rendered terminal surface, no
editor, no rendered paste/approval/trust dialogs, no adapter-spawn pathway that would
make command approval reachable, no Git-based change detection, file watcher, or
overwrite-confirmation UI, and no cross-platform evidence beyond Linux. There is
**no screen-reader support** — not limited, not planned, absent for the life of the
`iced` substrate decision (RFC-014). Command approval (below) remains implemented but
unreachable.

Durable audit currently records trust decisions, managed AgentRun lifecycle, blocked
root/symlink access, and audit-store recovery outcomes. Paste, restricted-feature,
safe-close, configuration-change, transcript-purge, project-added, and plain-terminal
producers are defined in the audit schema but not yet wired. The command-approval
producers are wired and tested but produce nothing, because nothing calls them — see
below.

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
| `Tab` / `Shift+Tab` | Cycle keyboard focus between shell zones |

`Ctrl+Shift+P` is reserved for a command palette that does not exist yet — it is
bound in the keybinding policy but currently does nothing.

`Esc` (dismiss) and `Enter` (activate) work on the shell's modal layer, but nothing
in the shipped application can currently open one — the only modal today is
developer-only scaffolding gated behind an environment variable. Real dialogs
arrive with RFC-022; these bindings currently do nothing a user can reach.

## Local Data and Privacy

Tekstide is local-first: it does not send project data anywhere. Today, the only
local state the desktop application creates is the **recent-projects list**, at
`$XDG_STATE_HOME/tekstide/recent-projects.json`
(`~/.local/state/tekstide/recent-projects.json` if `XDG_STATE_HOME` is unset) —
the paths of projects you have opened, used to restore the Project Board across
sessions. There is no in-app command to clear it yet; delete the file to reset it.

RFC-013's durable audit store and RFC-011's transcript retention are implemented
and tested, but **neither is wired into the desktop application yet** — running
`tekstide` today does not create an audit database or retain any transcripts.
See [`rfcs/done/013-durable-audit-store-and-local-data-policy.md`](rfcs/done/013-durable-audit-store-and-local-data-policy.md)
and [`rfcs/done/011-transcript-retention-and-local-data-policy.md`](rfcs/done/011-transcript-retention-and-local-data-policy.md)
for the retention and purge policy that applies once they are.

For a consolidated list of what else is missing or deferred, see
[`rfcs/future-work.md`](rfcs/future-work.md).

## RFCs

Implemented foundation RFCs live under [`rfcs/done/`](rfcs/done/).

Release scope and deferred work are tracked in [`rfcs/done/001-product-scope-mvp-and-non-goals.md`](rfcs/done/001-product-scope-mvp-and-non-goals.md), [`CHANGELOG.md`](CHANGELOG.md), [`ROADMAP.md`](ROADMAP.md), and [`rfcs/future-work.md`](rfcs/future-work.md).
