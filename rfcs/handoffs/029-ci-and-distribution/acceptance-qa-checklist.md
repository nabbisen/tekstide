---
title: "RFC-029 acceptance and QA checklist"
source_rfc: "../../accepted/029-documentation-ci-and-release-automation.md"
source_rfc_status: "Accepted by the human owner 2026-10-13"
---

# RFC-029 — acceptance and QA checklist

Every box stays unchecked in this file: it is a per-slice template, not a record. A slice's run is
recorded in its review request and in `qa-evidence.md`.

## PR-029-A — the dispositions

- [ ] Every live row in `test-process-leak.md` has a verdict, including the two added by this RFC's
      own review rounds (527, 538).
- [ ] Quarantined rows run somewhere non-blocking and say why they are there.
- [ ] **The failure rate was re-measured after the pass**, not assumed to have improved, and the
      number is stated.

## PR-029-B — the CI job

- [ ] Runs `fmt`, `clippy --workspace --all-targets --all-features -D warnings`,
      `cargo test --doc --workspace`, the full workspace with `--all-targets --no-fail-fast`.
- [ ] Short fixed `TMPDIR`; asserts **0 fixture entries** left.
- [ ] **Shown red on a deliberately broken commit**, with the output, before being trusted.
- [ ] First-party actions only, each pinned to a commit SHA; every download pinned by version and
      SHA-256.
- [ ] The workflow header states what it does not cover: Run It, the book sweep, anything needing a
      display.

## PR-029-C — artifacts

- [ ] A tag produces a Linux `x86_64` binary and its SHA-256, published on the release.
- [ ] Built `--locked` from the tag.
- [ ] Whether the binary is byte-reproducible from a fresh clone is **stated either way**.

## PR-029-D — the installer

- [ ] The script verifies the binary against its published checksum before installing.
- [ ] **Truncation proven**: a served prefix installs nothing. The evidence shows the attempt.
- [ ] Installs exactly one file to one location; touches no shell rc, adds no repository, needs no
      `sudo` surprise.
- [ ] Says where it installed to, and whether that location is on `PATH`.
- [ ] Wording: *verified against its published checksum*. **Not** *secure*, *safe*, *signed*, or
      *verified* unqualified.
- [ ] The scope is stated where the command is offered, not only in the script.

## PR-029-E — AUR and the install rewrite

- [ ] The AUR package installs a working binary; a packaged binary never rewrites itself.
- [ ] `README.md` and `getting-started.md` lead with the one-liner; `cargo install` and the
      four-line form follow as equivalents, not as the safer option.
- [ ] `getting-started.md` still says the product runs on Linux with no cross-platform evidence.

## PR-029-F — the landing page

- [ ] Carries the platform statement and the no-screen-reader statement.
- [ ] Says nothing about command approval that `REQ-AGENT-013` forbids.
- [ ] **Added to the release checklist's book-against-the-changelog sweep in the same slice.**

## Whole-RFC

- [ ] A person without a Rust toolchain can install and run Tekstide by following the first thing
      the README shows them. **Demonstrated, on a machine without `cargo` on `PATH`.**
- [ ] No artifact, page or script implies a platform that does not exist, or a guarantee the
      checksum does not give.
- [ ] The release checklist gained steps and lost none; the Run It gate and the book sweep are
      still human (D4).
- [ ] `rfcs/handoffs/release-checklist.md` run in full for the candidate.

## Final Acceptance Decision

- [ ] Accepted.
- [ ] Accepted with required follow-up.
- [ ] Requires re-review after changes.

Reviewer notes:

```text
```
