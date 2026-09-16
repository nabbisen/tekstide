---
title: "Release 0.19.0: purge reaches what a user actually has, and retention finally acts"
status: "**Scoped 2026-09-16 by the architect**, at the owner's word. The candidate is the dev team's; the publish and the tag are the owner's."
rfc_file: "none — release slice. Scope is RFC-049, RFC-050 and the documentation handoff, all closed."
target_milestone: "M12"
created: "2026-09-16"
---

# Release 0.19.0

**Why this release now.** crates.io still serves `0.18.0`'s README, which tells a reader that purge
removes *"every transcript retained for this project"*. That was false for every release from
`0.12.0`, and it is still the page a new user reads. **Only a publish replaces it.**

## Scope — what is in it

- **RFC-050, Transcripts From Earlier Runs.** A project knew only the transcripts launched since it
  was opened, so purge and the retained figures covered nothing older. They do now.
- **RFC-049, Transcript Retention Enforcement.** `transcript_retention_days` is enforced, at project
  open and agent-run launch preflight and at no other time. A live transcript is never deleted; when
  that leaves a budget exhausted, the next run starts without capture and says so.
- **The documentation slice.** `README.md` 561 → 123 lines, `CONTRIBUTING.md` written, the book
  published at `https://nabbisen.github.io/tekstide/` with six owned user chapters, and every README
  link absolute — which is what makes the crates.io page correct rather than merely shorter.

## The candidate — dev team

1. **Version** `0.18.0` → `0.19.0` in the workspace manifest, and `Cargo.lock` updated by a real
   build, not by hand.
2. **Changelog.** Promote `Unreleased` to a `0.19.0` heading with a name in this project's style, and
   **add the entry the Unreleased section is missing: the documentation slice.** A reader of the
   changelog should learn the book exists and where. The existing Fixed and Added entries stay as
   written; they were reviewed as they landed.
3. **The Rust minimum, measured before it is declared.** `File::try_lock` is stable from **1.89.0**
   (read from the std source at response 388), and nothing in the manifests says so. Install 1.89,
   build **both** crates with it, and then either declare `rust-version = "1.89"` or report what
   actually builds. A number nobody built against is not a claim this project makes.
4. **Gates, as at `0.18.0`**, with output to `.git-exclude/release-evidence/0.19.0/`:
   `git status --short`, `git diff --check`, `cargo fmt --check`, `clippy --workspace --all-targets
   --all-features -D warnings`, `cargo test --all-targets`, `cargo build --release --locked`,
   `cargo package` both crates, `cargo publish --workspace --dry-run --locked`, package smoke
   (LICENSE, NOTICE and README in both archives; NOTICE naming the shipped SQLite; no `.git`,
   `.git-exclude` or `target`), build-and-test from the unpacked package, `cargo audit` reconciled
   against `dependency-advisories.md`, and **three consecutive full-workspace runs with
   `--no-fail-fast`**.
5. **Run It.** A cold start against a throwaway `XDG_STATE_HOME`, and **one capture of this release's
   own headline**: the project board line naming what retention removed. Throwaway paths only.
6. **Known limitations, re-read rather than copied.** `0.18.0`'s list says
   `transcript_retention_days` is recorded and not enforced. **That line is now false and must go.**
   What stays: no screen-reader support; command approval exercisable only by the reference adapter;
   no before/after diff and no undo; terminal latency unverified; plain terminals unrecorded. What is
   new and must be said: the byte budgets gate new capture and are **not** a hard ceiling; a detached
   run's transcript is never reclaimed; transcripts belonging to no project are shown and not
   deletable in-app; a `recent-projects.json` that cannot be read loses that project's trust decisions.

## Mine, not yours

- Re-running every gate independently, and reading both archives with `tar tzf` rather than trusting
  the repository.
- **The crates.io rendered README, after the publish** — the check PR-DOC-C deferred to this release,
  and the reason the release exists.
- Post-publish artifact verification: extracted contents compared, and the published `tekstide`
  lockfile resolving the new core.

## The owner's

`cargo publish --workspace --locked`, the `0.19.0` tag on the reviewed release commit, and the word
that starts either.
