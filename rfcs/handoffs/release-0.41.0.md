---
title: "Release record — 0.41.0"
status: "Published 2026-11-21. Tagged `0.41.0` at `67aec25` and published from that commit."
---

# `0.41.0` — Honest About Containment

The close dialog has promised, before every confirmed close with a running process, that *"anything
started from these terminals ends too, including a backgrounded job."* Until this release, nothing
told the user when that promise could not be verified afterward: the containment check ran, its
result reached the audit record, and the close completed either way. The status bar now carries a
notice when a closed project's terminal session could not be confirmed empty, cleared again on the
next clean close.

## What this release is

Two user-visible changes, both in the same area and both found by the same work.

**The honest close notice.** `ContainmentNotice`, raised in the close-confirmation path only when a
session was actually checked and could not be confirmed empty, rendered in the status bar — which
renders on every route, because by the time this is known the project tab that raised it is gone.
It names no pid and no command: process-table text is untrusted input, and the fact needs none of
it. It says what is known ("could not be confirmed empty"), never "a process is still running",
which the enumeration failing to confirm does not establish.

**`TerminalPane::launch` now actually validates its root.** Its own doc comment had claimed for
some time that the root was "already validated to exist" while validating nothing. It now runs
`ProjectRootValidator` for real, with the symlink policy threaded per caller so the user-facing
route keeps `FailClosed` and only the two scratch-directory callers (demo panes, the measurement
harness) take `AllowCanonicalTarget`.

Internally, RFC-077's capability split: `tekstide-core`'s `project/` module held eight unrelated
capabilities bundled with its orchestration code and now holds only that orchestration. Every
module path is unchanged through a compatibility façade; **three public item shapes are not**, and
the CHANGELOG names them. Both of the user-visible fixes above were surfaced *by* that split, which
is what its own rationale predicted ("a hub is where safety invariants stop being locatable").

## Published

`cargo publish --workspace --locked`, core first: **`tekstide-core 0.41.0`** (crates.io,
`2026-11-21T08:39:49.146186Z`) then **`tekstide 0.41.0`** (`08:39:53.160737Z`, four seconds later).
Both confirmed present on the crates.io index directly
(`https://crates.io/api/v1/crates/<name>/0.41.0`, `yanked: false`), not inferred from `cargo`'s own
exit status.

Tagged **`0.41.0` at `67aec25`** (annotated, signed), pushed before publishing, and published from
that same commit so `.cargo_vcs_info.json` and the tag agree — the Post-Publish Verification
section's own (b) note, acted on in advance rather than discovered afterward.

`67aec25` is the reviewed candidate `d81b8d8` plus one documentation commit: RFC-077's Final
Acceptance Decision, written into its handoff pack before tagging, per the checklist's own review-526
item. CI was green on both `ci` and `docs` for `67aec25` before the tag was created.

## Who authorised and who executed

**The owner authorised; this assistant — the reviewer — executed.** `git tag -a 0.41.0 -m … 67aec25`,
`git push origin 0.41.0`, and `cargo publish --workspace --locked` were all run by this assistant,
on the owner's explicit per-release authorisation ("Authorized. You may do the tag and the
publish."), not a standing one.

Recorded this way because the checklist's own 2026-11-14 item requires it and because the previous
record's dated corrections were read first, as that item also requires: `release-0.39.0.md` and
`release-0.40.0.md` each originally stated these were "the owner's own actions" when in both cases
the owner authorised and the reviewer executed; the first was corrected on 2026-11-12 and the
correction did not reach the second. Authorisation and execution are different facts.

## Gate evidence

`.git-exclude/release-evidence/0.41.0/` holds the full record — the Run It capture and its
procedure, the containment Standing Watch's fresh red on both platforms, and the gate table.
Untracked by design: a release's gate record is for the owner and the team, and it is the document
most likely to carry a real path.

Two things in it are worth naming here, because they are the two this release's own history made
hardest:

- **Both platforms' containment guards were shown able to fail, this cycle, on real CI.** The
  `signal_session_survivors` call was suppressed, pushed (`39c7a66`), confirmed red on `gate` and
  `macos-measurement` (run `44880500171`), and reverted byte-identically across `crates/`
  (`09d2b9f`, run `44880979390` green). This is the one gate whose entire point is that a green run
  proves nothing, and the checklist requires it every release rather than once.
- **The Run It gate was performed on the shipped release binary** with a fresh `XDG_STATE_HOME`,
  via a capture route that never touches the configured screenshot directory, and the capture was
  looked at by the reviewer rather than taken on description.

## Review

Candidate reviewed across rounds 710, 711 and 712. Two Required items were found and fixed before
the tag: the CHANGELOG claimed no published API had moved when three public item shapes had, and
the per-platform containment red had not been re-done this cycle. The release-evidence directory
was missing and was created. RFC-077's premature move to `rfcs/done/` at the candidate cut was
caught by `rfc_docs_invariants` itself — the fourth time this project has reached for that move and
the first time it did not survive to a commit.

## Known limitations, deferred rather than fixed

- **The containment Standing Watch stays open** on its own harder question: what makes a
  bulk-visible session member stop matching its own session while still alive. This release makes
  the product honest about the consequence; it does not explain the cause. The cheapest step toward
  it. **Correction, 2026-11-21, review 713:** this entry originally called printing the pids the
  enumeration log discards a step still untaken; it had landed in `de4e0e3` on 2026-11-16, before
  this release was cut, and the claim came from a stale paragraph in the release checklist.
- **RFC-078 (session-member recall) is accepted with no implementation slice begun.**
- The scrollbar contrast gap (`derived-contrast-pairs.md`, 2026-11-18) and widening the iced
  form-control denylist beyond `checkbox` are recorded and unscheduled.
- macOS: `0.39.0`'s disclosure (RFC-028 *Acceptance* option (2), no Apple hardware available to
  this project) covers the Run It gate for this release too, unchanged.

## Boundaries respected writing this record

Did not touch `packaging/aur/*`. The GitHub release workflow runs automatically on the pushed tag;
the AUR packages remain the owner's own action, under that checklist section's own terms.

## Post-publish verification

Both published `.crate` files downloaded from `static.crates.io`, extracted, and compared against
the local package trees by content:

- **`tekstide-core 0.41.0`: identical.** `diff -rq` reports nothing.
- **`tekstide 0.41.0`: identical.** `diff -rq` reports nothing — including `Cargo.lock`.

**Expected difference (a) did not occur, and that is correct rather than suspicious.** The
Post-Publish Verification section predicts the local `tekstide` archive's lockfile will name the
*previous* `tekstide-core`, because `cargo package -p tekstide` resolves it from crates.io before
the new one exists. Under `cargo publish --workspace` the core is published first and the
`tekstide` archive is packaged after, so the local tree already resolved `tekstide-core 0.41.0`
(`registry+https://github.com/rust-lang/crates.io-index`) and there was nothing left to differ. The
check's own substance holds: **the published `tekstide` lockfile names the version of
`tekstide-core` just published.**

**Expected difference (b) did not occur either, by design.** Both archives'
`.cargo_vcs_info.json` name `67aec25885451b1468cdfc6d4533f6a06272b97a` — the tagged commit — because
the publish was run from it with nothing landing in between. That section says to publish from the
tagged commit if provenance and tag are to agree; they do.

So the rule *"two differences are expected, anything else is a defect"* was met with **zero**
differences, which is the stronger outcome and not a missed check: each expected difference has a
named cause that did not apply this time.

Remaining post-publish items are the ones the checklist assigns elsewhere: the beginner-path
install against the published artifact, and the AUR packages, both the owner's.

## Release artifacts

`.github/workflows/release.yml` ran for the tag (`44885910047`) and **both** jobs succeeded —
`artifacts` (Linux) and `artifacts-macos` — checked in the Actions tab per job, not inferred from
the tag existing or from one job's green check. The release is public: `draft=false`,
`prerelease=false`.

Six assets, each binary with its published checksum:

| asset | size |
|---|---|
| `tekstide-x86_64-linux` + `.sha256` | 30 043 592 B |
| `tekstide-aarch64-macos` + `.sha256` | 17 148 560 B |
| `LICENSE`, `NOTICE` | — |

**Both checksums verified by downloading the artifacts and re-hashing**, not read off the
workflow's own log:

- Linux `053dbc18d5c91d6dfcc9861f5cdf93e6ee29c83c60689cd55859af12eba92e2e` — match.
- macOS `740f7ba5b8ee6a8fee478c1a3e75e8020a992a3a12ece483b7d37ba19c4720b5` — match.

The downloaded Linux artifact reports `tekstide 0.41.0`. The macOS artifact was checksum-verified
only; this project has no Apple hardware to run it on, which is the same RFC-028 *Acceptance*
option (2) disclosure the rest of this record carries.

## This record was destroyed and restored, 2026-11-21

**The reviewer truncated this file to one byte in `5036fab`** — the commit whose own message says
it added the artifacts section above — and the loss stood through `fda89fe` and `475ae1d` until
review 713 surfaced it. Restored here from `4eafeb7`, the last intact commit, with the artifacts
section rewritten.

**Cause**, named exactly so it is not repeated: a whitespace fix written as
`open(p,'w').write(open(p).read().rstrip('\n')+'\n')`. Python evaluates `open(p,'w')` **first**,
which truncates the file, and only then evaluates the argument — which reads the now-empty file. The
same fix written over two statements, reading into a variable before opening for write, is safe and
was used correctly in the commit immediately before this one.

**What did not catch it.** `rfc_docs_invariants` was green on the truncating commit and on every
commit after: `the_newest_release_status_matches_whether_it_was_actually_published` requires this
file to **exist**, not to say anything. A one-byte release record satisfies the invariant written to
guarantee a release has a record. Recorded as a gap in the gate, not only as a mistake in a
one-liner.
