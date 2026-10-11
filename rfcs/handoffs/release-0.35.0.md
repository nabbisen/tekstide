---
title: "Release 0.35.0: seeing the audit"
status: "**Published 2026-10-12.** Authorised by the owner; published, tagged and verified by the architect. The release that reached a storage layer finished two years' worth of slices ago, and found what the reaching was hiding."
rfc_file: "../done/059-seeing-the-audit.md"
target_milestone: "pre-1.0"
created: "2026-10-12"
---

# Release 0.35.0

## What this release is

**Twelve audit families recorded, and nothing could see or remove any of it.** `AuditStore::query`
— filtering by project, family, outcome and operation, paged by keyset cursor — and the whole
purge API with its receipt were already written and tested in `tekstide-core`, with **zero
production call sites**. A finished storage layer with no surface on it.

`Ctrl+Alt+V`, or the "Audit" button, now opens a top-level view over it, reachable with no project
open because the store is app-wide. Purge is reachable too, per-project and global, each behind a
confirmation naming its scope and its real count.

## What the reaching found

**A view makes a store's silence dangerous**, because a reader shown a list concludes the list is
what happened. So the release spent its first slice on whether the store could be believed at all:

- **Every family-and-outcome combination the type can produce** is now proven to land in a real
  store and come back from `query`, by a test derived from the type with no hand-written list. It
  found the one gap already known and no others.
- **Two processes creating the database for the first time could livelock.** Not merely lose the
  race — both retry in lockstep against SQLite's fixed busy-handler schedule and exhaust the
  timeout. Fixed with a blocking OS `flock` across the whole create-or-open decision.

**A fourth failure shape of that race turned up in the reviewer's own ablation, after the fix had
landed**: the losing process opens a database the winner created but has not yet stamped, and
reads it as foreign — `diagnostics.rs` would have told a user their audit database was *not a
Tekstide audit store at all*, about a file Tekstide was mid-write on. That is the clearest argument
for why the lock covers the whole `exists()`-through-pragmas sequence rather than the `CREATE`.

## The consequence nobody had noticed

**Purging audit records also revokes trust.** Trust is decided by querying `audit_events` for a
`TrustChange`/`Applied` row — the exact rows a purge deletes — and `verify_restored_trust_against`
demotes any project whose row is gone. The confirmation described the act as deleting a log.

It now says so, in both scopes and in the book, including the part that makes it worse: the
revocation is **delayed**. The open session keeps working, trusted, and the trust is gone at the
project's next open. The dialog names the mechanism, not just the outcome.

The behaviour itself is right — the evidence is gone, so the claim should be too. Only the silence
was wrong.

## `REQ-SEC-023`, and what this release does not claim

Met **structurally for audit views, and only for audit views**: every column in `audit_events` is
an id, an enumerated symbol, or a bounded reference, so there is no field that could hold free
text and no redaction logic was needed to make the claim true. Adding one would have implied the
opposite.

The requirement's log half — a transcript holds raw run output, and the AgentRun Report exports
16 KiB of it to a user-named path — is **unmet**, and is RFC-068, reserved and not started. A
content scanner that misses one secret is worse than no claim, because the claim is what makes
someone stop checking. `RedactionClaimScope` still has exactly one variant, which is the honest
marker of that gap.

`REQ-SEC-025` is tracked for the first time by this release. No plan row had ever named it.

## Published

`cargo publish --workspace --locked`, core first. Tagged **`0.35.0` at `6fc0555`**, and published
*from* that commit — `.cargo_vcs_info.json` in both published archives names the tag, the second
release running.

Post-publish: the two documented differences and nothing else. The published `tekstide` lockfile
names `tekstide-core 0.35.0` with checksum `c9593736…`, and the published core `.crate` hashes to
exactly that. `post-publish-check.sh` **PASS** for `0.35.0` and for `0.34.0`, both including the
`cargo install` build.

Run It, against the release binary: `Ctrl+Alt+V` opened the view in the shipped artifact, showing
its own disclosure line, the bounded count, all four filters, the purge control and one real
record. `.git-exclude/release-evidence/0.35.0/`.

## What review cost, and bought

Reviews **527–534**. Three of the required items were about *how work is checked* rather than what
it does, and all three were the same shape — a check that could not fail:

- A documented reproduction recipe that failed for an unrelated reason (`--lib` does not build
  `[[bin]]` targets), reading as success both before and after the fix.
- A corrected message asserted on a function's return value while the stale wording stayed in the
  panic header a human actually reads.
- Hand-maintained filter arrays of a thing the previous slice had just made compile-time
  exhaustive.

And one about where checking happens: a pre-fix proof taken on a hand-dirtied tree with a `git
checkout` restore — the practice `ablate.sh` exists to replace. Redone in a worktree, and the
worktree recipe is now in the script's own header, together with the cases the script cannot see.

## Deferred, recorded rather than fixed

- **RFC-068** — redacting what a run prints.
- The audit directory's permission modes: `0755`/`0644`, private only because `~/.local/state` is
  `0700`, which is the user's system's doing and not a decision this product has made. Relocating
  `XDG_STATE_HOME` somewhere shared makes the store world-readable.
- Several surfaces' controls cannot be reached from the keyboard. Raised to the owner as a
  product-wide question.
- A `proposed/` RFC's status claim is unchecked in every register.
