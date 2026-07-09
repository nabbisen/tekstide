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

The current implementation is a core/shell foundation through RFC-006. It includes:

- Project Board and ProjectSession state;
- root-bound file access policy;
- bounded explorer read model;
- UTF-8 text document buffer;
- safe save and external-change detection;
- shell-visible Content Mode evidence.

It is not yet the full AI CLI workbench. The desktop GUI, PTY terminal runtime, AgentRun launch, AI CLI workflow, transcript/review flow, file watcher, overwrite-confirmation UI, and durable audit storage are deferred.

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
