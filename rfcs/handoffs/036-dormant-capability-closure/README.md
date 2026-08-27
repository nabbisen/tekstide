---
title: "RFC-036: Dormant Capability Closure — implementation handoff"
rfc: "RFC-036"
rfc_file: "../../accepted/036-dormant-capability-closure.md"
source_rfc_status: "Accepted 2026-08-18, D0–D4 decided 2026-08-27 — M12"
target_milestone: "M12"
created: "2026-08-27"
---

# Decide each orphan once, with evidence

Source RFC: [RFC-036](../../accepted/036-dormant-capability-closure.md)

| # | Read | Why |
| --- | --- | --- |
| 1 | [RFC-036](../../accepted/036-dormant-capability-closure.md) | **Read "Decided 2026-08-27" first.** D0–D4 are settled; the RFC body above them still lists rows that have since changed |
| 2 | [`what-a-triage-must-not-become.md`](./what-a-triage-must-not-become.md) | **Required.** This slice deletes published API and can quietly launder gaps |
| 3 | [`reachability-audit.md`](../reachability-audit.md) | The 2026-08-17 audit that produced the list. **Its counts are stale** — see D0 |
| 4 | [`task-breakdown-pr-plan.md`](./task-breakdown-pr-plan.md) | Three slices |
| 5 | [`acceptance-qa-checklist.md`](./acceptance-qa-checklist.md) | What must be true and evidenced |

## The one-sentence version

Every remaining orphan gets one of three decisions with a cited measurement behind it, deletions
land together in `0.16.0`, and two rows leave the triage because they are defects rather than
dormancy.

## The deliverable is a table, not a refactor

This RFC's own stated risk is *"triage becomes a rewrite."* The output is a **decision table**:
one row per orphan, with a decision, a one-line reason, the measured caller count, and — per D4 —
the search shape that would have found it.

Anything that turns out to need real design **gets its own RFC and a row saying so**. It does not
get absorbed into this slice because it was already open in the editor.

## What is already decided, so you do not re-litigate it

- **D0 — re-verify every row before deciding it.** The list is ten days old. `request_terminate`
  has **three** production callers now (RFC-039/043); `shutdown` has one. Deciding from the
  written list would produce decisions about capabilities that are no longer dormant.
- **D1 — deletion is on the table, batched into `0.16.0`.** That bump is already owed for
  RFC-044, so it costs nothing extra. A "delete" row must cite what was searched, not assert
  dormancy.
- **D2 — "core-only" needs a named consumer that exists as a file**, cited by RFC number.
  **RFC-045 (Configuration Reachability) is reserved** for the four configuration-conditioned rows
  precisely so this rule has no carve-out: `set_resource_limits`, `ConfigStore`,
  `to_ai_cli_profile`, and the `sensitive_config_changed` producer are all "keep, consumer:
  RFC-045."
- **D3 — `recover` and `purge_all_records` are not triage rows.** They are a recovery path and a
  data-deletion path that have never run from the application. Their own slice, as a defect.
- **D4 — each row names the search that would have found it**, in one line. The shape, not the
  check. This is a specification for the next mechanical sweep, not a mandate to build one.

## Measuring a caller count, so every row means the same thing

State the method once in `qa-evidence.md` and use it for every row. At minimum a row's count
distinguishes:

- **production callers in `tekstide`** — the thing the audit actually counted;
- **callers in `tekstide-core` itself** — an item used internally is not an orphan in the same
  sense;
- **test-only callers** — `set_resource_limits` has one (RFC-031's discrimination test), which is
  why "delete it" is no longer free for that row, and the RFC says so.

A row that says "0 callers" without saying *which* of these it counted is not a measurement.

## Traps

- **`shutdown` and `request_terminate` are already wired.** If your table still lists them as
  orphans, you triaged the document instead of the tree.
- **"Keep, documented" is the comfortable answer.** D2 exists because it is indistinguishable from
  "dead" without a named consumer. A row that cannot cite an RFC number is not eligible for it.
- **Deleting a public item is a breaking change**, and `tekstide-core 0.15.0` is published. That
  is fine and decided — but the removals go in one release, and `CHANGELOG.md` names them
  individually, because a consumer's build breaking is the loudest thing this project can do to
  someone.

## Deferrals to state, not to solve

- **The 74 untraced call chains.** A second audit pass and its own unit of work; this triages what
  the first pass confirmed.
- **RFC-045 itself.** Reserved, unauthored, and the rows that depend on it say so.
