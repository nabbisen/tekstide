---
title: "Release 0.34.0: a project held by one process"
status: "**Published 2026-10-11.** Authorised by the owner; published, tagged and verified by the architect. The release whose own candidate claimed it had already shipped, and that now has a check which can say otherwise."
rfc_file: "../done/058-a-project-held-by-one-process.md"
target_milestone: "M13"
created: "2026-10-11"
---

# Release 0.34.0

## What this release is

**Two Tekstide windows could hold the same project and neither knew.** RFC-027's recovery records
are keyed by project id and a hash of the document's path, and `AppState::open_project` reuses the
same project id for the same canonical root — which is what lets recovery survive a restart, and
also what let two windows on one project write the same record file. Edit the same file differently
in each and the last write won, with nothing saying the other edit had ever existed.

A project root is now held by **one process at a time, with a real OS `flock`** at
`$XDG_STATE_HOME/tekstide/project-locks/<project-id>.lock` — the same primitive this project's own
transcript writer already uses, not an in-memory flag. A second attempt is refused rather than
opened as a duplicate, and the window that holds it is asked for your attention over a small local
socket beside the lock.

**If the mechanism cannot tell who holds a project, opening proceeds unprotected rather than
refusing.** Only `WouldBlock` means held; every other error means *cannot decide*. A text editor
that will not open because of a lock file has chosen the wrong failure.

## Verified before publishing, not after

- **Two real processes of the release binary**, not a test harness: A held a fixture project, B was
  refused, B's board carried *"This project is already open in another Tekstide window. It has been
  asked for your attention."* verbatim from `crates/tekstide/locales/en.ftl:385`, and the project
  was not opened as a duplicate. A's lock and attention socket were on disk, the lock holding A's
  pid. Evidence: `.git-exclude/release-evidence/0.34.0/`.
- **The first screen of a first-time user**, release binary, fresh `XDG_STATE_HOME`, no arguments:
  names only actions that exist.
- Gate: `fmt` clean, `clippy --workspace --all-targets --all-features -D warnings` 0 diagnostics,
  `763 + 22 + 1132` with 0 failed, 0 fixture entries in a fresh short `TMPDIR`, the owner's real
  state directory at 0 before and after, `cargo test --doc --workspace` clean.
- `cargo audit`: 0 vulnerabilities; the three allowed warnings (`paste`, `ttf-parser`, `lru`) match
  `dependency-advisories.md` exactly, none missing and none new.
- Unpacked-package test, **all `[[bin]]`s built first**: 1131 passed, 1 failed, that one being the
  documented `real_repository_filesystem_scan_cost_headless_benchmark`, which refuses to report a
  number for something that is not a repository.

## Published

`cargo publish --workspace --locked`, core first. Tagged **`0.34.0` at `293d981`**, and published
*from* that commit — so `.cargo_vcs_info.json` in both published archives names the tagged commit
rather than one past it, which is the first release where those two agree.

Post-publish: published and local archives differ in exactly the two documented places. The
published `tekstide` lockfile names `tekstide-core 0.34.0` from the registry with checksum
`9e5dba67…`, and the published core `.crate` hashes to that same value. `post-publish-check.sh`
**PASS** for `0.34.0` and for `0.33.0`, both including the `cargo install` build.

## What this release cost, and what it bought

RFC-058 took **fourteen review rounds, 513 to 526**. Five of them (522–526) were spent not on the
lock but on the release machinery around it, and that is the part worth recording.

**A fix introduced the leak it was reviewed next to.** PR-058-C's own CLI path resolved the state
root independently, so every CLI test wrote a real lock and a real `AF_UNIX` socket into the
owner's real state directory — 206 files. The reviewer's diagnosis blamed the GUI paths and was
wrong; the implementer corrected it, and removed the leftovers only after reading each lock's
recorded pid and confirming with `kill(pid, 0)` that none was live.

**The candidate claimed it had already been released.** Seven documents said *released
2026-10-11* with no tag, no publish record and nothing on crates.io, and the whole invariant
network was green because its ground truth for "released" was the changelog's own sentence. There
is now a record only a publish writes — this file — and
`the_newest_release_status_matches_whether_it_was_actually_published` reads the changelog against
it **in both directions**: a claim with no record, and a record with no claim. The second direction
exists because the reviewer asked for only the first and had to be told so.

**Three checks written during this release could not have failed.** A reviewer's `grep` item that
matched six of seven false sites and missed the changelog. A test asserting a corrected message on
the function's return value while the stale wording stayed in the panic header a human actually
reads. A checklist step naming one helper binary after a second had been added. Each was found by
running the check against the defect it was written for, rather than by reading it.

## Deferred, recorded rather than fixed

- The audit store's first-creation **TOCTOU** — app-wide, not project-scoped, so a per-project lock
  could never have guarded it. `rfcs/future-work.md`.
- **Four of the seven release-state claim sites** still have no check against the changelog, and
  both ground truths scrape every `RFC-` a changelog section *mentions* rather than the one it
  *releases*. One item, to be decided together. `rfcs/future-work.md`.

## Acceptance

`rfcs/handoffs/058-one-process-per-project/acceptance-qa-checklist.md`, `## Final Acceptance
Decision` — **Accepted**, with what the acceptance rests on written into the pack. That section had
been left blank for RFC-065, RFC-066, RFC-027 and RFC-067: four shipped releases whose acceptance
lived only in `.git-exclude/`, which `.gitignore` excludes. The release checklist now requires it
before tagging.
