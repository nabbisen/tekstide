# Tekstide

Tekstide is a local-first, multi-project workbench for supervising terminal-based AI development workflows.

## Quick Start

```sh
cargo run -p tekstide_app
```

Open one or more local project paths from the command line:

```sh
cargo run -p tekstide_app -- /path/to/project
```

The current implementation is the first RFC-005 application-shell slice. It renders the first-run Project Board and keeps the core shell state buildable and testable while the desktop GUI layer is selected.
