---
title: "Release 0.20.0: the record that survives, and the trust state that survives with it"
status: "**Scoped 2026-09-16 by the architect**, after RFC-048 and RFC-051 closed. The candidate is the dev team's; the publish and the tag are the owner's."
rfc_file: "none — release slice. Scope is RFC-048 and RFC-051, both closed."
target_milestone: "M12"
created: "2026-09-16"
---

# Release 0.20.0

## Scope — what is in it

- **RFC-048, AgentRun Termination Records.** The trail said a run was launched and could not say
  whether it ended. It now records the ending — exited, terminated, or a runtime failure after the
  start — and records **nothing** for a run Tekstide stopped supervising, because an ending nobody
  observed must not be stated.
- **RFC-051, Recovering the Recent-Project List.** A `recent-projects.json` that could not be read was
  replaced by an empty list **saved over it**, taking the project ids — and with them the trust
  decisions those ids match, and the ownership of transcripts. Now it is quarantined, recovered from a
  previous-good copy, and the board says which happened.
- **One breaking change to `tekstide-core`'s API**: `RecentProjectStore::load` and the
  `CorruptState` error variant are gone, superseded by `load_or_recover`. Already in the changelog.

## The candidate — dev team

1. **Version** `0.19.0` → `0.20.0` in the workspace manifest, `Cargo.lock` updated by a real build.
2. **Changelog**: promote `Unreleased` to a `0.20.0` heading with a name in this project's style. The
   entries are already written and reviewed; **do not rewrite them**, and keep the Removed entry's
   "what a dependent does instead".
3. **Re-measure the Rust minimums rather than carrying them.** `0.19.0` declared `tekstide-core`
   1.89 and `tekstide` 1.90, the second because of a dependency. Both are claims about *this* tree, so
   build both crates on the declared toolchains again and say what happened. If a dependency moved,
   the numbers move with it.
4. **Gates, as at `0.19.0`**, output to `.git-exclude/release-evidence/0.20.0/`: `git status --short`,
   `git diff --check`, `cargo fmt --check`, `clippy --workspace --all-targets --all-features
   -D warnings`, `cargo test --all-targets`, `cargo build --release --locked`, `cargo package` both
   crates, `cargo publish --workspace --dry-run --locked`, package smoke (LICENSE, NOTICE and README
   in both archives; NOTICE naming the shipped SQLite; no `.git`, `.git-exclude` or `target`),
   build-and-test from the unpacked package, `cargo audit` reconciled against
   `dependency-advisories.md`, and **three consecutive full-workspace runs with `--no-fail-fast`**.
5. **Run It — this release's own headline.** A cold start, and **the recovery**: a real project, the
   state file corrupted, a restart, the project back with its id, and the board saying it was restored.
   The RFC-051 evidence capture proves the feature; **the release needs it from the release binary**.
   Throwaway paths only.
6. **Known limitations, re-read rather than copied.** `0.19.0`'s list stands except where these two
   RFCs changed it. What is new and must be said:
   - **A detached run's ending is not recorded**, and its trail stops at `Started`. Deliberate.
   - **A loss that happened before `0.20.0` is not repairable** — there was no backup to recover from.
   - **One previous-good copy is not a history**: two consecutive bad saves lose the good one.
   - **Project ids are still not derivable from the folder**, so a reset with no backup still orphans
     transcripts (RFC-050 D6′ is unchanged).

## Mine, not yours

Re-running every gate independently, reading both archives with `tar tzf`, checking the rendered
crates.io README after the publish, and post-publish artifact verification.

## The owner's

`cargo publish --workspace --locked`, the `0.20.0` tag on the reviewed release commit, and the word
that starts either.
