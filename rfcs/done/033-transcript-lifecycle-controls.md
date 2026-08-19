# RFC-033: Transcript Lifecycle Controls

Status: **Implemented and closed 2026-08-19.** A user can decline transcript capture per
project, purge what exists, and see what is retained — all from the Trust Settings surface
(`Ctrl+Alt+U`), all proven from real key presses. The sentence `0.11.1` had to publish — *"there
is no in-app way to turn capture off or to purge it"* — is gone from both READMEs. **Does not
claim** that purge removes every trace: a tombstone remains, and the purge itself is now audited,
which is a trade stated in the closeout rather than discovered. **Does not claim** that declining
capture deletes anything already written; those are two acts and the surface keeps them
distinguishable. Accepted by the human owner 2026-08-18; see
[the handoff pack](../handoffs/033-transcript-lifecycle-controls/README.md) for the full
evidence. Original acceptance note: closes a limitation `0.11.1` had to publish on a privacy claim.
Target milestone: M11
Date: 2026-08-18

Related baseline documents:

- `tekstide-requirements-v0.md`
- `tekstide-security-threat-model-v0.md`

Depends on:

- [RFC-011](../done/011-transcript-retention-and-local-data-policy.md) — retention limits,
  capture mode, per-run opt-out and purge scope, all **designed and none reachable**.
- [RFC-013](../done/013-durable-audit-store-and-local-data-policy.md) — the audit store a
  purge must record into (`transcript_purge` is a defined, unwired family).
- [RFC-023](./023-configuration-system.md) — where a *default* capture setting would live, if
  one is wanted. This RFC is about per-run and after-the-fact control, not defaults.

## Summary

Let a user decline transcript capture for a run, and delete transcripts afterwards.

## Why this is scheduled

`0.10.0` made agent-run launch reachable, which made transcript capture reachable with it.
`0.11.1` corrected the documentation and, in doing so, had to publish this:

> **There is no in-app way to turn capture off or to purge it.** RFC-011 designs both a
> per-run opt-out and a purge scope, and neither has a user-facing route yet. To remove
> transcripts today, delete the `transcripts/` directory.

That sentence is accurate and it is not acceptable as a resting state. Tekstide writes the
output of a user's AI sessions — which may quote their files — to disk, bounded but
indefinite within those bounds, and offers no control except a filesystem operation performed
outside the application.

**The model is already built.** `AgentRunLaunchRequest::without_transcript_capture()` exists.
`ProjectSession::purge_project_transcripts` exists. Both are on the reachability audit's
orphan list. This is a routes-and-decisions RFC, not a model RFC.

## Scope

1. **Per-run opt-out**, exercised before the run starts, at the moment the user launches it.
2. **Purge**, with a stated scope — per run, per project, or application-wide.
3. **Visibility**: a user should be able to see that capture happened and how much is
   retained, without leaving the application. `transcript_local_data_summary` already
   computes this and has no caller.

## Non-goals

- Changing capture defaults. The owner decided 2026-08-18 that capture is intended; this RFC
  does not revisit that.
- A configurable default. That is RFC-023's, and this RFC should not pre-empt it.
- Rendering transcript *content*. That is RFC-020's AgentRun report surface.

## Decisions required

**D1 — where the opt-out lives.** `Ctrl+Alt+A` launches immediately today; there is no launch
dialog to put a checkbox in. Options: a confirmation step before launch (a new modal, and a
new interruption on the product's most-used action), a per-project setting, or a modifier
binding. **Recommend a per-project setting on the Trust Settings surface**, which already
exists, already carries project-scoped security state, and is already reachable
(`Ctrl+Alt+U`) — rather than adding a dialog in front of every run.

**D2 — purge scope and confirmation.** Deleting a transcript is irreversible and it is the
*safe* direction (less data retained), which argues for the same asymmetry RFC-032 chose:
revoking trust needs no confirmation, granting does. But a purge that silently removes a
record a user wanted is a different failure than a grant. **Recommend: confirmation for
project-wide and application-wide purge, none for a single run**, and the dialog states what
is removed and that it cannot be undone.

**D3 — does purge write an audit event?** `transcript_purge` is a defined but unwired family
in RFC-013's schema. Wiring it here means a deletion is recorded — which is the point of an
audit trail — but it also means the audit store retains a record of a privacy action the user
took to remove data. **Recommend wiring it**, recording only that a purge occurred and its
scope, never a path or a byte count that would reconstruct what was removed. State the trade
in the closeout rather than leaving it implicit.

## Risks

- **A purge that misses.** RFC-011's storage layout is per-project, per-run; a purge that
  removes the metadata record but leaves bytes on disk is worse than no purge, because the
  UI would then report zero retained. The gate must assert bytes are gone, not that the
  record is.
- **An opt-out that does not opt out.** `without_transcript_capture()` sets the request's
  state root to `None`, which — per the finding recorded in `future-work.md` — is also what
  a `Managed` profile's approval channel falls back to. Setting `approval_state_root`
  explicitly is a prerequisite for this RFC, not an afterthought.

## Open questions

1. Should an opt-out persist per project, or reset each session? Persisting is friendlier and
   is a security-relevant setting that survives restarts — the same shape as trust, which
   this project already decided should persist and be audited.
