---
title: "RFC-020: Diff Review and AgentRun Report Surfaces — handoff pack"
rfc: "RFC-020"
rfc_file: "../../accepted/020-diff-review-and-agentrun-report.md"
status: "Ready for implementation — accepted 2026-08-12, both prerequisites landed"
target_milestone: "M10"
created: "2026-08-15"
---

# Start here

RFC-020 completes M10. It renders two surfaces that already have reviewed models behind
them and no user-visible existence: the change review surface over RFC-024's diff content,
and the AgentRun report over RFC-011 Amendment 1's transcript reader.

**`0.7.0` shipped RFC-024's content access with no surface at all.** That capability is
dark until this lands. It is the reason this slice is `0.8.0`'s spine rather than one item
among several.

## Reading order

1. **[`the-window-boundary.md`](./the-window-boundary.md)** — required before any code.
   The transcript reader's window boundary falls outside the property RFC-017's filter was
   proven against, and the escaping asymmetry gains a third position here. Both are
   security-critical and neither is obvious from the RFC alone.
2. [`implementation-handoff.md`](./implementation-handoff.md) — per-surface instructions.
3. [`task-breakdown-pr-plan.md`](./task-breakdown-pr-plan.md) — the slices and their gates.
4. [`acceptance-qa-checklist.md`](./acceptance-qa-checklist.md) — tick at closeout.
5. [`qa-evidence.md`](./qa-evidence.md) — record results as you go.

## Both prerequisites are landed — this is no longer blocked

RFC-020 §Correction sized two prerequisites and warned they were not the same size. Both
are now done:

- **RFC-024 (Diff Preview Policy)** — authored as its own RFC because it was new state and
  new I/O, not an accessor. Implemented, closed, shipped in `0.7.0`.
- **RFC-011 Amendment 1 (bounded transcript reader)** — genuinely amendment-shaped, since
  RFC-011 had already decided capture mode, retention, budget and purge. Authorised
  2026-08-12. **The reader does not exist yet; building it is PR-020-B's first task.**

## The four open questions, answered

RFC-020 §Open questions left four. They are resolved here so implementation does not have
to guess, and each answer is a decision you may push back on with evidence.

**1. Option A or B?** **B**, decided by the owner. Model work sequenced first, as
amendments to the RFCs that own it. Both landed; nothing here re-opens it.

**2. How is a diff bounded?** **It already is — do not add a second bound.** RFC-024
measured 4 MiB per side against a real RSS sweep and refuses above it rather than
truncating. RFC-020 renders what that policy hands it. If a rendering-side limit seems
necessary (a viewport, a line cap), that is a *display* concern and must be named as one,
never as a second content bound. Two bounds in two crates disagreeing is how a surface
starts silently showing less than the model allowed.

**3. Does the change surface offer any action — accept, revert, stage?** **No. Read-only**,
like the explorer. RFC-012's foundations are detection-only; accept/revert needs a model
that does not exist. A read-only surface that shows the truth is worth more than an
actionable one built on a model that cannot support the action. **Say this on the surface**
if a user might otherwise expect a button.

**4. Should `DiffContent` stay owned or become lifetime-bound?** **Leave it owned, and
carry the limitation forward honestly.** RFC-024's `DiffContent` derives neither `Clone`
nor `Serialize`, which blocks two specific storage paths — a `Clone` state struct and an
audit producer. It does **not** prevent a consumer destructuring it and retaining the
inner bytes. RFC-020 must not do that, and must not describe `DiffContent` as
non-retainable, because it is narrower than that. Changing the type is out of scope here;
it belongs to whoever revisits RFC-024.

## What this RFC must not claim

Three overclaims are live risks, each with a gate against it:

- **That it renders a diff for a modified file.** It cannot. RFC-024 §Correction is
  definitive: `ReviewBaselineEntry` is metadata-only by RFC-012's stated principle, so the
  before-bytes were never captured and are gone by preview time. The surface shows
  *current content, explicitly not a diff* — **and says so where the user reads it**, not
  only in a closeout.
- **That the change set is complete.** RFC-012's detection is metadata-only and
  conservative. A surface implying "these are all the changes" overclaims what detection
  can see. The limitation goes **on the surface**.
- **That a rendered change is safe to accept.** Nothing here evaluates a change. The
  surface presents; the user decides.

## One claim that must be checkable

**A bidi override introduced by a generated change is visible in the diff surface.**

State it as a claim that could be false, and test it. It is the strongest argument for
escaping this surface: a reviewer deciding whether to accept AI-generated code most needs
to see `U+202E` precisely when it is there, and a surface rendering it faithfully hides
the most dangerous thing it could contain.
