---
title: "RFC-029 handoff — Documentation, CI, and Release Automation"
status: "**Implemented and closed 2026-10-14** — `0.37.0` candidate, not yet published. All six slices and the Whole-RFC checklist closed."
source_rfc: "../../done/029-documentation-ci-and-release-automation.md"
source_rfc_status: "Implemented and closed 2026-10-14; `0.37.0` candidate, not yet published."
target_release: "0.37.0"
---

# RFC-029 — CI and distribution

Read [the RFC](../../done/029-documentation-ci-and-release-automation.md) first, then
[`what-the-installer-must-not-do.md`](./what-the-installer-must-not-do.md).

## What this is for

**A person who has never heard of Rust should be able to get this program running.** Today the only
install is `cargo install tekstide`, which asks for a toolchain and a compile. Everything in this
RFC exists for that one outcome; a slice that lands its mechanism and leaves that person no better
off has not finished.

## Two things decided before you start, both easy to get wrong

**The order is a prerequisite.** `test-flake-disposition.md`'s open verdicts land **first**
(PR-029-A). This project measures about **3 failures in 17 full-workspace runs**; a blocking CI job
at that rate is red roughly half the time at three runs. A gate that is red when nothing is wrong
destroys the only thing a gate is for, and this project's entire method depends on red meaning
something. Nothing in B–F starts until A is done.

**The one-liner is in, and the reviewer was wrong to resist it.** The argument is in the RFC under
*The installer*. The short version: both forms execute code from the same server, and the checksum
is served from the same release as the binary, so a four-line checksum ceremony recommended to a
beginner is a ritual resting on the same trust root as the thing it verifies — the exact shape
`what-the-dialog-must-not-lie-about.md` §2 forbids. What survived is two properties of the
**script**, now D10–D12, not an argument against offering it.

## Where the first artifact comes from — decided 2026-10-13 (owner, option B)

**`0.37.0`'s own tag produces it. There is no pre-emptive dispatch and no retrofit onto
`0.36.0`.** The release that introduces the distribution channel is the release that first uses
it: the candidate is cut, the architect tags, `release.yml` fires on that tag, and the artifact
exists.

This matters for how the slices land:

- **The install pages go into the candidate**, like the version bump and the changelog's own
  *"candidate, not yet published"* line. Between the candidate cut and the tag, `main` briefly
  describes an install whose artifact is minutes away — the same short, documented window every
  candidate already has. **No hold, and no branch**:
  `held/pr-029-e-install-rewrite` merges into `main` with the candidate and is then deleted.
- **The end-to-end install demonstration is a post-publish step**, not an acceptance item. It is in
  the release checklist's *Post-Publish Verification*, beside the `cargo install` check, because
  both need an artifact that acceptance precedes.
- **`release.yml`'s first live run happens at that tag.** If it fails there, the cost is bounded:
  it touches neither crates.io nor the release notes, so the fix is to repair the workflow and
  re-run it with `workflow_dispatch` against the same tag.

**How this came about is worth keeping**, since the fault was the architect's: RFC-029's slice
order put the documentation ahead of the channel it documents, which turned "the channel has no
first output yet" into an authorisation the owner had to grant, which blocked three slices, which
produced a parked branch, which broke the project's trunk-only workflow. Each step answered the one
before it. The order was the error.

## Slices

| Slice | What it is | Done when |
| --- | --- | --- |
| **PR-029-A** | The flake dispositions land. Not new work — the pass `test-flake-disposition.md` already defines. | Every live row has a verdict: fixed, quarantined with a reason, or accepted with a reason that is not "we have not got to it" |
| **PR-029-B** | The blocking CI job (D2, D3), Linux only. | It runs what the local gate runs, and **has been shown failing** on a deliberately broken commit before anyone trusts it |
| **PR-029-C** | Release artifacts and checksums (D5). | A tagged build publishes a Linux binary and its checksum |
| **PR-029-D** | The one-liner installer (D9–D12). | The script verifies the binary's checksum, **truncation safety is proven by serving a prefix**, and its scope is stated and kept |
| **PR-029-E** | AUR (D6) and the install rewrite (D7, D13, D14). | The pages lead with the one-liner; `cargo install` and the four-line form follow as equivalents |
| **PR-029-F** | The landing page (D8). | It carries the platform and screen-reader statements, and claims nothing the product does not do |

## Conventions

- **`.github/workflows/docs.yml` is the template, and its rules are not negotiable here**:
  first-party `actions/*` only, each pinned to a **commit SHA**; every download pinned by version
  **and** SHA-256; no `paths:` filter without the reasoning that file already gives. A new workflow
  that relaxes any of these is a regression, not a shortcut.
- Evidence under `evidence/pr-029-<slice>/`, paths relative to the repository root.
- Ablations needing isolation go in a disposable `git worktree` — `ablate.sh`'s header has the
  recipe and the cases the script cannot see.
- The gate is `rfcs/handoffs/release-checklist.md`, which this RFC **adds to** and does not shrink.

## What this pack does not decide

macOS and Windows artifacts are RFC-028; the Microsoft Store is RFC-071; the update check is
RFC-070; code signing needs an identity and is the owner's decision. A slice that finds itself
wanting any of them has found a scope error worth reporting.
