---
title: "Release record — 0.42.0"
status: "Published 2026-11-21. Tagged `0.42.0` at `31a648a` and published from that commit."
---

# `0.42.0` — Closing The Recall Gap

macOS's containment sweep re-checked a pid's liveness directly, bypassing its own bulk
`proc_listallpids` scan, in exactly one case: the session leader. Every other member was found, or
not, purely through that bulk scan — so a live, non-leader member the scan happened to omit was
read as departed, with nothing to re-check it against. This release closes that gap. Nothing about
it is visible to a user, and the CHANGELOG says so in its own second line.

## What this release is

**No user-visible change.** Two internal changes, both named in the CHANGELOG: RFC-078's
session-member recall, and the retirement of the explorer's macOS breadth-ratio assertion. A release
that improves a guarantee nobody can see is still worth shipping; claiming more for it than that is
the failure mode this project keeps closing.

**RFC-078 D1 — session-member recall.** Each `RunningTerminal` now remembers which pids its own most
recent successful enumeration reported as members; a later enumeration in the same close that no
longer names one of them re-checks it directly, through `live_member_of_session` — the same strict,
session-identity-checking predicate every other member is checked with, never the leader's
membership-blind fallback. A pid reused by an unrelated process on this shared machine therefore
still cannot be recovered and signalled by mistake, which is what that bullet exists for.

**The macOS breadth-ratio assertion is retired, not re-raised.** Review 706's pre-registered
condition fired: an ablation crossing of 3.93x arrived *below* the bound itself, so on that run the
macOS bound could not have detected the regression class it guards. Noise has reached 2.95 and
signal has fallen to 3.93 — the two populations overlap, and no single number separates two
overlapping distributions. The assertion is now Linux-conditional, where the separation is 3.35x;
macOS keeps measuring and printing through the standing `--nocapture` steps, and the constant names
what would reopen it. The scan under test is platform-independent code, which is why asserting on
one platform loses nothing real.

## Published

`cargo publish --workspace --locked`, core first: **`tekstide-core 0.42.0`** (crates.io,
`2026-11-21T13:42:56.961691Z`) then **`tekstide 0.42.0`** (`13:43:01.199943Z`, four seconds later).
Both confirmed present on the crates.io API directly
(`https://crates.io/api/v1/crates/<name>/0.42.0`, `yanked: false`), not inferred from `cargo`'s own
exit status.

Tagged **`0.42.0` at `31a648a`** (annotated, signed — `git tag -v` reports a good signature), pushed
before publishing, and published from that same commit so `.cargo_vcs_info.json` and the tag agree.

`31a648a` is the reviewed candidate `eacea6c` plus three documentation commits: the two checklist
edits from review 716's own findings (`c5837d5`), a correction to one of them (`657f292` — the
quoted harness dependency path was five levels up where the manifest says four, and five resolves to
nothing, which defeats the edit's purpose of letting a reader verify the dependency is by path), and
RFC-078's Final Acceptance Decision written into its own file before tagging, per the checklist's
review-526 item (`31a648a`). Both `ci` and `docs` were green on `31a648a` before the tag was created.

## Who authorised and who executed

**The owner authorised; this assistant — the reviewer — executed.** `git tag -a 0.42.0 … 31a648a`,
`git push origin 0.42.0` and `cargo publish --workspace --locked` were all run by this assistant, on
the owner's explicit per-release authorisation ("Authorized. You may do the tag and the publish."),
not a standing one.

Recorded this way because the checklist's own 2026-11-14 item requires it, and the previous record's
dated corrections were read first as that item also requires — `release-0.41.0.md` carries the
correction history of this exact misattribution (`0.39.0` and `0.40.0` both stated these were the
owner's own actions) and its own 2026-11-21 correction about a stale claim that reached four
documents.

## Review

Candidate reviewed across rounds 713, 714, 715 and 716. **No Required item was found in the
candidate itself** — the one defect in this release's work was found earlier, at review 713, and
fixed in the commit that was found in: `assert_session_is_empty` was passed a fresh, empty recall
list instead of the real one, making the leak guard strictly less able to confirm membership than
the production enumeration it certifies.

Review 716 raised two findings, both about **gates rather than this release's content**, and both
landed as checklist edits before the tag:

- **A path-dependency harness's stale lock is silent where `--locked` would be loud.** The
  `052-file-explorer` harness's lock read `tekstide-core 0.40.0` through the entire `0.41.0` cycle,
  including the commit whose own message said it was re-running that harness. Because the dependency
  is by path, the build always compiled the real local source and `0.41.0`'s numbers stand — but
  `cargo run --release --locked` would have *refused* on that lock, and the checklist says that
  refusal is itself the finding. It did not refuse, so the flag was not used.
- **A Run It capture names no build.** `0.42.0`'s first-screen capture is byte-identical to
  `0.41.0`'s. That is expected when nothing visible changed, and it was disclosed rather than left
  to be noticed — but nothing in a capture identifies the binary that produced it, so a fresh
  capture and a copied file are indistinguishable artifacts. The checklist now requires the launched
  binary's own `sha256` beside the capture.

## Gate evidence

`.git-exclude/release-evidence/0.42.0/` holds the record — the Run It capture and its procedure, the
containment Standing Watch's fresh red on both platforms, and the gate table. Untracked by design.

The one worth naming here, because a green run of it proves nothing:

- **Both platforms' containment guards were shown able to fail, this cycle, on real CI.** The
  `SIGKILL` fallback was suppressed (`2582dbc`), confirmed red on `gate` and on `macos-measurement`
  in run `44902051905` — read from that run's own failed-test list, including the macOS-only
  `a_real_close_kills_a_live_hup_immune_backgrounded_job` — and reverted **byte-identically across
  the whole `crates/` tree** (`022d155`; `git diff 2582dbc~1 HEAD -- crates/` empty). The
  byte-identity is the evidence, not the green run that followed: a partial revert would ship a
  suppressed guard.

## Known limitations, deferred rather than fixed

- **The containment Standing Watch stays open** on its own harder question: what makes a
  bulk-visible session member stop matching its own session while still alive. This release closes
  the consequence precautionarily; it does not explain the cause. The diagnostic that will answer
  its next occurrence — the enumeration log's pids — has been in place since `de4e0e3`, 2026-11-16.
- **Recall can reduce confirmations, not only find survivors.** An ambiguous liveness answer
  (`EPERM` rather than `ESRCH`) on a recalled pid fails the whole enumeration, so a session that
  would otherwise have been confirmed empty can become one that cannot be confirmed. Deliberate: the
  alternative is the unearned confidence `session_confirmed_empty`'s own doc comment forbids. This
  is the "refuse emptiness more often" cost the owner's acceptance weighed explicitly, now real
  rather than hypothetical.
- **The audit record still has no honest `AuditOutcome` arm for "there was nothing to confirm."** It
  needs a schema change and a load path, and it is the residual of the containment product decision
  whose part 1 shipped in `0.41.0`.
- The scrollbar contrast gap (`derived-contrast-pairs.md`) and widening the iced form-control
  denylist beyond `checkbox` remain recorded and unscheduled; neither was in this slot's scope.
- macOS: `0.39.0`'s disclosure (RFC-028 *Acceptance* option (2), no Apple hardware available to this
  project) covers the Run It gate for this release too, unchanged.

## Boundaries respected writing this record

Did not touch `packaging/aur/*`. The GitHub release workflow runs automatically on the pushed tag;
the AUR packages and the beginner-path install against the published artifact remain the owner's own
actions, under those checklist sections' own terms.

## Post-publish verification

Both published `.crate` files downloaded from `static.crates.io`, their `sha256` compared against
the checksums the crates.io API reports, extracted, and compared against the local package trees by
content:

- **`tekstide-core 0.42.0`: identical.** `diff -rq` reports nothing. Checksum
  `550246936db0c8b4d1e068951fbf86d15ce0c0ca1548715193eb45611ae05dcc`, matching the API.
- **`tekstide 0.42.0`: identical.** `diff -rq` reports nothing, including `Cargo.lock`. Checksum
  `27c79e201cfd76fe370330f3508f476458a6f8e97cd64d2dd4029b70c85eec19`, matching the API.

**Neither of the two expected differences occurred, and both have a named cause for why not** — the
same outcome as `0.41.0`, recorded again rather than cross-referenced, because a reader of this
record should not have to fetch another one to know the check was really run.

(a) The Post-Publish Verification section predicts the local `tekstide` archive's lockfile will name
the *previous* `tekstide-core`, because `cargo package -p tekstide` resolves it from crates.io before
the new version exists. Under `cargo publish --workspace` the core is published first and the
`tekstide` archive packaged after, so the local tree had already resolved `tekstide-core 0.42.0`
(`registry+https://github.com/rust-lang/crates.io-index`) and nothing was left to differ. The
check's substance holds: **the published `tekstide` lockfile names the version of `tekstide-core`
just published.**

(b) Both archives' `.cargo_vcs_info.json` name `31a648a2f0151e33c7a823da09eae3d98d788168` — the
tagged commit — because the publish was run from it with nothing landing in between. That section
says to publish from the tagged commit if provenance and tag are to agree; they do.

So *"two differences are expected, anything else is a defect"* was met with **zero**, which is the
stronger outcome and not a missed check.

## Release artifacts

`.github/workflows/release.yml` ran for the tag (`44910215067`) and **both** jobs succeeded —
`artifacts` (Linux) and `artifacts-macos` — checked per job rather than inferred from the run's own
conclusion or from the tag existing. The release is public: `draft=false`, `prerelease=false`.

Six assets, each binary with its published checksum:

| asset | size |
|---|---|
| `tekstide-x86_64-linux` + `.sha256` | 30 041 024 B |
| `tekstide-aarch64-macos` + `.sha256` | 17 151 360 B |
| `LICENSE`, `NOTICE` | — |

**Both checksums verified by downloading the artifacts and re-hashing them**, not read off the
workflow's own log:

- Linux `9597bb4738da5e587fbb2e3b9486c5c01f43e256371a3f2849184b5712505a84` — match.
- macOS `02550d0cc967bd30aca23fa8cea2a2df9dd0024e0461dc0376929ab000dcc7c1` — match.

The downloaded Linux artifact reports `tekstide 0.42.0` when run. The macOS artifact was
checksum-verified only; this project has no Apple hardware to run it on, which is the same RFC-028
*Acceptance* option (2) disclosure the rest of this record carries.

**Note for the next release, from review 716's second finding:** this record can state which binary
produced the Linux version string above because the artifact was downloaded and executed here. The
Run It capture in `.git-exclude/release-evidence/0.42.0/` cannot make the equivalent statement — the
checklist now requires the launched binary's own `sha256` beside it, starting next release.
