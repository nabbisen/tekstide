# Contributing to Tekstide

This page is for working **in** the repository. Using Tekstide is covered by
[the book](https://nabbisen.github.io/tekstide/).

## How work is decided

Anything larger than a fix goes through an RFC. [rfcs/README.md](rfcs/README.md) is the index, and
it follows [RFC-000](rfcs/done/000-rfc-lifecycle-policy.md) in the five-folder variant adopted by
[RFC-037](rfcs/done/037-five-folder-rfc-lifecycle.md): `proposed/` → `accepted/` → `done/`, with
`archive/` for what was withdrawn. **The folder is the source of truth for an RFC's state.** If a
document's status and its folder disagree, the folder wins, and a test enforces it.

An accepted RFC is implemented from its handoff pack under `rfcs/handoffs/`: a task breakdown, an
acceptance checklist, and a risk document to read before writing code. Evidence goes in the pack's
`qa-evidence.md`.

## The gate

Run this before asking for review. Every step must pass:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace > test-run.log 2>&1   # three consecutive runs, each to a file
```

- **Three consecutive runs, with output redirected to a file.** Several tests launch real processes,
  PTYs and sockets and are sensitive to load. A run whose output is filtered cannot show a flake.
- **Every intermittent failure you see gets a dated entry** in
  [rfcs/handoffs/test-process-leak.md](rfcs/handoffs/test-process-leak.md), even if it is already
  known. A flake mentioned and not recorded is lost.
- **Stage explicit paths, then check whitespace on what you staged:** `git diff --cached --check`.
  Plain `git diff --check` never sees a new, untracked file.
- `cargo test --workspace` includes `rfc_docs_invariants`, which checks RFC status against folders,
  relative links in the RFC tree, the book source and this file, that `README.md` has no relative
  links, and that its book links name real pages.
- If you changed `docs/`, also run `mdbook build docs`.

## Evidence

[ARCHITECTURE.md](ARCHITECTURE.md#evidence-conventions) sets out the evidence conventions every RFC
inherits. The two that most often catch people:

- **A test is evidence only if removing the property it covers makes it fail.** Ablate it, and say
  what failed.
- **A committed screenshot shows throwaway state only**: a `mktemp -d` fixture, never a path under
  your home directory.

## What is not published

- **`.git-exclude/` is local only** and git-ignored. Review correspondence and working notes live
  there; nothing in it is committed or published.
- **`docs/book/` is build output**, also git-ignored. The book is built and published by
  `.github/workflows/docs.yml` on every push to `main`.
- Each published crate carries only its own directory. `README.md` is the `tekstide` crate's
  crates.io page, so **every link in it must be absolute**.
