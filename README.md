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

The current implementation is a headless core through RFC-013: terminal runtime,
AgentRun launch, transcript retention, generated-change review, and durable audit
storage. It includes:

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
  corruption diagnostics, restart-safe recovery, and explicit purge.

It is not yet the full AI CLI workbench. The desktop GUI, rendered terminal surface,
app/UI terminal and launch commands, rendered paste/approval/trust dialogs, command
approval, Git-based change detection, file watcher, overwrite-confirmation UI, and
cross-platform evidence beyond Linux are deferred.

Durable audit currently records trust decisions, managed AgentRun lifecycle, blocked
root/symlink access, and audit-store recovery outcomes. Command approval, paste,
restricted-feature, safe-close, configuration-change, transcript-purge, project-added,
and plain-terminal producers are defined in the audit schema but not yet wired.

## Quick Start

```sh
cargo run -p tekstide
```

Open one or more local project paths from the command line:

```sh
cargo run -p tekstide -- /path/to/project
```

## RFCs

Implemented foundation RFCs live under [`rfcs/done/`](rfcs/done/).

Release scope and deferred work are tracked in [`rfcs/done/001-product-scope-mvp-and-non-goals.md`](rfcs/done/001-product-scope-mvp-and-non-goals.md), [`CHANGELOG.md`](CHANGELOG.md), [`ROADMAP.md`](ROADMAP.md), and [`rfcs/future-work.md`](rfcs/future-work.md).
