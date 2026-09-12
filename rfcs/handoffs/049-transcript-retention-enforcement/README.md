---
title: "RFC-049: Transcript Retention Enforcement — implementation handoff"
rfc: "RFC-049"
rfc_file: "../../accepted/049-transcript-retention-enforcement.md"
source_rfc_status: "Accepted 2026-09-12 — M12"
target_milestone: "M12"
created: "2026-09-12"
---

# The first slice here that deletes a user's data

Source RFC: [RFC-049](../../accepted/049-transcript-retention-enforcement.md)

## What this is

RFC-011 names four retention budgets. **One is enforced.** The per-project and app-wide byte
budgets are computed into `TranscriptLocalDataSummary::budget_pressure` and read by nothing;
`max_age_days` is read only by an `is_bounded()` sanity check and RFC-045's config write; and
`TranscriptRetentionState::Expired` exists with **no producer**.

RFC-011 already specified the cleanup — oldest-inactive-first, never a live writer,
`RequiredLocalBounded` failing preflight on exhaustion. This slice builds it.

## Read these first, in this order

1. [`what-deleting-a-transcript-must-not-do.md`](./what-deleting-a-transcript-must-not-do.md) —
   **required before writing code.** Six rules. §1 is the one that makes this RFC different from
   every slice before it.
2. The RFC's D1–D9, especially **D6–D9 under "Decided on acceptance"** — found by reading the code
   rather than the RFC, and each one closes a question you would otherwise have to invent an answer
   to.

## The trap this slice sets

**Every previous slice in this area disclosed. This one acts.** RFC-033 built a purge the user
asks for. RFC-045 recorded a retention value and said four times that nothing enforces it. This
slice is the first that removes a user's data **because a policy said so**, and the user will not
be watching when it happens.

That asymmetry governs every judgement call here. When a rule could be read two ways, take the one
that deletes **less**, later, and more visibly. A transcript kept a day too long is a bounded
disappointment; one deleted a day early is gone.

## Sequencing

**A → B → C**, each landable alone, and **nothing deletes until C**.

- **A** — `tekstide-core`: the timestamp arithmetic D9 needs, and expiry *marking*. No deletion.
- **B** — `tekstide-core`: budget selection and the cleanup itself, driven by an explicit call.
  Deletion exists here but nothing in production calls it.
- **C** — the two triggers, the audit record, the preflight refusal, and D5's summary fix.

Depends on nothing open. RFC-045 is closed; RFC-033's purge is the path B reuses.

## What is not in this pack

- **Background automation.** D2 settles it: no timer, no watcher, no idle sweep.
- **A second deletion path.** D3 routes through RFC-033's purge; one function in this product
  deletes transcript bytes and it stays one.
- **Configurable byte budgets.** They become *possible* here and are not done here — RFC-045's D3′
  rule says a key returns with the code that reads it, and a key nobody asked for is not that.
- **Any audit schema change.** D6: the pairing already exists and is already valid.
