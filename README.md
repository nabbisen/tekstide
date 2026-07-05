# Tekstide

Tekstide is a local-first, multi-project workbench for supervising terminal-based AI development workflows.

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

Implemented foundation RFCs live under [`rfcs/done/`](rfcs/done/). The release-scope RFC remains under [`rfcs/proposed/`](rfcs/proposed/) until the amendment is reviewed and accepted under the RFC lifecycle.

Release scope and deferred work are tracked in [`rfcs/proposed/001-product-scope-mvp-and-non-goals.md`](rfcs/proposed/001-product-scope-mvp-and-non-goals.md), [`RELEASE_NOTES.md`](RELEASE_NOTES.md), and [`rfcs/future-work.md`](rfcs/future-work.md).
