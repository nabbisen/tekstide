---
title: "RFC-025 — task breakdown and PR plan"
rfc: "RFC-025"
rfc_file: "../../accepted/025-notifications.md"
source_rfc_status: "Accepted 2026-09-22 — M12"
target_milestone: "M12"
created: "2026-09-22"
---

# Task breakdown and PR plan

**A then B.** A changes no visible text; B adds the status bar fields.

## PR-025-A — the model, and the four migrate onto it

- A `Notification`: scope (project or global), kind, rendered text, and **a lifetime from the closed
  set of two** (§3). The type makes a third impossible.
- **The four existing producers construct notifications** instead of returning strings: audit health,
  configuration diagnostics, the recent-list reset/recovery, retention cleanup. The board renders the
  model in a **deterministic order defined by kind** (D7).
- **No text changes.** If a sentence must change to fit the model, stop: that is a finding, not a
  chore.

**Required tests:** every existing absent-when-false test passes **unmodified** — that is the
migration's whole acceptance. Plus: the order is deterministic across renders; a notification cannot
be constructed with a lifetime outside the set (a compile-level property if the type allows it).

**Ablations:** give one migrated notice the other lifetime; drop a producer's condition. Each fails
its own existing test.

## PR-025-B — the status bar says what the requirements ask

- **REQ-NOTIFY-002 fields**: trust state, Git state, running and failed sessions, pending approvals.
  `status_bar_summary` shows a route, a project count and a key hint today; these are additions to
  it, not a rewrite of the board.
- **Git state reads "not available"** until RFC-030 produces it (D5). Do not fake it, and do not
  leave the field out.
- **REQ-NOTIFY-003 labels** from `ProjectRuntimeSummary`: `2 running`, `1 awaiting approval`,
  `1 failed`. Absent when zero, so the bar stays readable.
- **REQ-NOTIFY-004/005**: reachable by keyboard on surfaces already reachable; every state a word.

**Required tests:** each field present when its fact is true and absent when it is not, ablated
separately; a bare count appears nowhere; the Git field says "not available" while nothing produces
it.

**Evidence:** a live capture of a project with at least one running session and one pending approval,
against throwaway state, showing the labels as states rather than numbers.
