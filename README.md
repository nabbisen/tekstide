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

It is not yet the full RFC-001 `v0.1.0` AI CLI MVP. The desktop GUI, PTY terminal runtime, AgentRun launch, AI CLI workflow, transcript/review flow, file watcher, overwrite-confirmation UI, and durable audit storage are deferred.

## Quick Start

```sh
cargo run -p tekstide
```

Open one or more local project paths from the command line:

```sh
cargo run -p tekstide -- /path/to/project
```

## RFCs

Implemented foundation RFCs live under [`rfcs/done/`](rfcs/done/). The broader product-scope RFC remains under [`rfcs/proposed/`](rfcs/proposed/) until the release scope is finalized.
