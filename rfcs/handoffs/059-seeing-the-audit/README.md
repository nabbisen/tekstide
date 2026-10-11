---
title: "RFC-059 handoff — Seeing The Audit"
status: "**Implemented and closed 2026-10-12** — `0.35.0` candidate, not yet published. All three slices and the Whole-RFC checklist closed."
source_rfc: "../../done/059-seeing-the-audit.md"
source_rfc_status: "Implemented and closed 2026-10-12; `0.35.0` candidate, not yet published."
target_release: "0.35.0"
---

# RFC-059 — Seeing The Audit

Read [the RFC](../../done/059-seeing-the-audit.md) first. D1–D7 are decided; this pack is how
the work is cut, what it must not do, and what it is checked against.

## The short version

**Twelve audit families record. No surface shows one record, and no in-app action purges one.**
Both capabilities already exist in `tekstide-core`, tested, with **no production call site**:

- `AuditStore::query(&AuditQuery) -> AuditRecordPage` — filters by project, family, outcome and
  operation; pages by `before_sequence`; bounded by `limit`.
- `AuditStore::purge_all_records` and the per-project purge, returning an `AuditPurgeReceipt`.

So the release is mostly a surface over finished storage. **That is the reason to believe its size,
and also the reason to be suspicious**: a slice that turns out to need storage changes has found
something the scoping missed, and should say so rather than absorb it.

## Order is a decision, not a convenience

**PR-059-A lands before any view exists.** A view over a store that can silently fail to write is a
view that can be confidently wrong, and the two recorded defects it closes are both about writes
that do not happen and say nothing:

1. An audit write rejected by a SQL `CHECK` is swallowed by `append_observation`'s best-effort
   design. 52 constraints; **nothing asserts that a family and outcome actually lands**.
2. Two processes creating the store for the first time both see `exists() == false` and both run
   `CREATE_SCHEMA_V3`, which has no `IF NOT EXISTS`.

If A finds more dropped families than the one already known, **stop and report before building the
view**. What it finds changes what the view can honestly claim.

## Slices

| Slice | What it is | Done when |
| --- | --- | --- |
| **PR-059-A** | The store can be believed. Enumerate every family-and-outcome the Rust side can produce, write each to a real store, assert each lands. Fix the first-creation race. No UI. | Every producible combination is asserted to land, by enumeration rather than a hand-written list; two real processes creating the store concurrently both succeed |
| **PR-059-B** | The view. A top-level destination from the Project Board (D7) over `AuditStore::query`, with D4's filters and D3's bounded-count wording. | A real store's records render; filters narrow; a bounded page says what it left out |
| **PR-059-C** | Purge, reachable. Per-project and global, receipt shown. | `REQ-SEC-025` met for audit data; the deleted count comes from the receipt, not from a count taken before |

## Conventions

- Evidence under `evidence/pr-059-<slice>/`, path given relative to the repository root.
- Screenshots show **throwaway state only** — a `mktemp -d` project and a `mktemp -d`
  `XDG_STATE_HOME`. A capture is taken `wl-copy --clear` → `niri msg action screenshot-window --id
  <id>` → confirm `wl-paste --list-types` prints `image/png` → paste. **`wl-paste` returns the
  previous clipboard contents when a capture does not land**, which cost the `0.34.0` gate a
  near-miss defect report; two captures that should differ must have different checksums.
- The gate is `rfcs/handoffs/release-checklist.md`. Read it before the candidate, not after.

## What this pack does not decide

The RFC's non-goals are not open questions. Redaction of transcript content is **RFC-068**. Audit
retention and audit export are neither in scope nor refused — they are unasked, and a slice that
finds it needs either has found a scoping error worth reporting.
