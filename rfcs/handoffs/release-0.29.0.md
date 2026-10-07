---
title: "Release 0.29.0: the explorer keeps up on its own"
status: "**Scoped 2026-10-07 by the architect**, after the split point was decided. Three items complete RFC-026; the candidate is the dev team's; the publish, the tag and the post-publish checks are the architect's."
rfc_file: "../accepted/026-file-watcher.md — moved to done/ by the closure commit"
target_milestone: "M13"
created: "2026-10-07"
---

# Release 0.29.0

## The split, and what it means for this release

**RFC-026 is now the watcher alone.** The multi-document model is **RFC-065**, shipping as `0.30.0`.
The reasoning is in `delivery-plan.md` under *The split point, decided 2026-10-07*; the short form is
that the watcher is finished and measured, the document model has not started and has a design
question in front of it, and an RFC spanning two releases would trip the released-RFC invariant that
RFC-053's late closure bought us.

**Nothing is reordered.** RFC-027 recovers *buffers*, plural, so it always followed the document
model. `027`, `058`, `059` and `060` each move down one.

## What completes `0.29.0`

Three items from slice C, and one carried fix that is not RFC-026's:

1. **The book names the batching window.** By this release a user can observe it, so it describes real
   behaviour rather than an internal constant — which is why it moved out of slice A at review 450.
2. **Undo and a user-driven reload.** A reload constructs a fresh `TextDocument`, so the undo history
   goes with the document that went; the product must not pretend otherwise. RFC-057 PR-057-D proved
   the mechanism; this is the user-driven path over it.
3. **Captures use a fresh `XDG_STATE_HOME`.** Already adopted from review 461; it holds for the
   release captures too.
4. **The change-review ratio fix**, scheduled into `0.29.0` after a fourth lost gate attempt. Assert a
   **ratio against a reference workload timed in the same process** rather than an absolute 500 ms.
   Two worked examples are in reach: `editor_typing_latency_baseline_100_000_lines` for *publish the
   number and assert nothing*, and RFC-026's own paired control for *the comparison lives inside one
   run*. **It is not RFC-026's subject** — say so in the changelog, the way `0.27.0` carried the pin.

## What the changelog owns

- `REQ-FILE-003`, `REQ-FILE-004` and `NFR-PERF-007` met, with the figures: a thousand-file burst
  becoming **3 scans**, keystroke p95 inside 16 ms with the watcher idle and under burst.
- **The first new dependency in many releases.** The gate record stops saying "No dependency added"
  and says what was added and why: `notify 8.2.0`, five crates on Linux, eighteen lock entries, CC0-1.0,
  and the 9.0 release-candidate line as the re-check row.
- **Limitations, in the product's own words:** watching is per project and stops entirely on a
  refusal, resuming only on a reopen; the real `ENOSPC` path is evidenced by reading, not by a test;
  a save costs a scan and a re-read of the file just written; and the document model is **not** in
  this release — one document at a time, still.

## The gate

Everything in `release-checklist.md`. Note for this one: **`cargo audit` and
`dependency-advisories.md` both change** — the five new crates get rows, with the 9.0 line as the
re-check. The `.rs` placement check and the workspace package step are unchanged from `0.28.0`.

## Mine, after the candidate

The independent gate, the package diff against published `0.28.0`, `cargo audit`, the publish, the
tag, and `post-publish-check.sh` for **`0.29.0` and `0.28.0`**.
