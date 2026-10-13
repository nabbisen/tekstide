---
title: "RFC-029 acceptance and QA checklist"
source_rfc: "../../done/029-documentation-ci-and-release-automation.md"
source_rfc_status: "Implemented and closed 2026-10-14; `0.37.0` candidate, not yet published."
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

- [ ] The first thing the README shows a reader is an install that needs no Rust toolchain, and
      every claim on that page is true of the release being cut. **The end-to-end demonstration
      itself is NOT a candidate-acceptance item** — it needs a published artifact, which only
      exists once this release is tagged. It lives in the release checklist's *Post-Publish
      Verification*, beside the `cargo install` check that is already done after publishing rather
      than before. Corrected 2026-10-13: the original wording required an artifact that acceptance
      itself precedes.
- [ ] No artifact, page or script implies a platform that does not exist, or a guarantee the
      checksum does not give.
- [ ] The release checklist gained steps and lost none; the Run It gate and the book sweep are
      still human (D4).
- [ ] `rfcs/handoffs/release-checklist.md` run in full for the candidate.

## Final Acceptance Decision

- [ ] Accepted.
- [x] Accepted with required follow-up.
- [ ] Requires re-review after changes.

Reviewer notes:

```text
Reviewer: the high-capability reviewer. Review rounds 530-555; the candidate itself
was 552, with 553, 554 and 555 carrying required corrections.

ACCEPTED WITH REQUIRED FOLLOW-UP, at 3fee6d8.

What the acceptance rests on, measured rather than reported:
- Three consecutive full-workspace runs at the candidate commit, mine not the
  implementer's, TMPDIR=/dev/shm: 0 failed, 0 fixture entries each. The gate was
  re-run from scratch once, because row 1 of the flake register failed run 2 of
  an earlier attempt at 9037aee -- three consecutive green, not three attempts.
- fmt, clippy --workspace --all-targets --all-features -D warnings, doc tests,
  rfc_docs_invariants 22/22.
- cargo audit: zero vulnerabilities, and the three unmaintained/unsound warnings
  match dependency-advisories.md's three rows exactly -- no new row, none retired.
- Scope: workspace 0.37.0 with the tekstide-core pin moved with it; crates/
  tekstide-core/README.md read against this release and needs nothing, since no
  core production code changed; no status line anywhere claims 0.37.0 as released.
- The book-against-the-changelog sweep, both directions. No locale string changed
  this release, verified from the diff rather than assumed, so the en.ftl half of
  that sweep had no target. introduction.md's platform and no-screen-reader
  statements are word-for-word identical to README.md's, as the entry claims.

Defects found during the candidate's own review, and fixed:
- The README advertised a 404: the install pages described a prebuilt binary
  before any tag produced one (review 552, fixed by D15 and a sentence true on
  both sides of the tag).
- D1's second sentence -- a quarantined test still runs, in a separate job that
  is read -- was never built by any slice. Found at 553 after this RFC had
  already closed. Now stated in CHANGELOG.md, this RFC's own Closed section,
  both registers and ci.yml's header, and reserved as RFC-072.
- The flake register blamed "real machine contention" for the quarantined knock
  test. Ablating the #[ignore] away in a throwaway worktree passed 3 of 3 full
  workspace runs at the load the row blamed; the trigger is unidentified. The
  verdict (Quarantine) stands on the record, not on a named cause.
- Two overclaims corrected: "rules out load" (failing to reproduce is not ruling
  out) and the changelog's opening "all implemented" while PR-029-A's own
  checklist box was unmet. Both were partly the reviewer's own wording.
- RFC-072 was first reserved in a second table below the one rfcs/README.md's
  heading tells an author to check, and with Headless = no for a CI job.
- README said "the four-line verified form" above a five-line block -- the
  review-549 fix added the line to the code and left the sentence. Fixed by
  dropping the count, since the count is the part that drifts.

Deferred rather than fixed, deliberately:
- RFC-072. D1's non-blocking job needs a convention separating the three
  quarantined rows from the nine measurements and diagnostics that -- --ignored
  also selects; a job reading all twelve would be red for non-findings on day
  one, which is D1's own failure mode.
- Row 1 of the flake register (bind_recovers_from_a_stale_socket_file) recurred
  on this candidate's gate, and its Accept's "no actionable fix" premise no
  longer holds: source: None narrows it to the two arms of clear_stale_socket
  that response 231 left indistinguishable. Recorded in test-process-leak.md
  with the ruling-out work; the fix and the re-verdict are the slice after this
  release, since the flake predates it and nothing here touches that code.
- Branch protection on main, which makes ci.yml's job actually blocking, is a
  repository setting only the owner can make. The job ships advisory and says so.
- The one-liner run against the real published artifact, on a machine without
  cargo on PATH, cannot run before the tag exists. It is in the release
  checklist's Post-Publish Verification, not here.
```
