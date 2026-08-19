---
title: "What purge must remove — required reading before RFC-033 code"
status: "Required reading"
rfc_file: "../../proposed/033-transcript-lifecycle-controls.md"
target_milestone: "M11"
created: "2026-08-19"
---

# What purge must remove

**Read this before writing purge.** Deleting a user's data is irreversible, and the failure
that matters is not deleting too much — it is deleting *less than the UI then claims*.

## The failure mode, stated first

**A purge that removes the metadata record and leaves the bytes on disk is worse than no
purge**, because the UI will afterwards report zero retained bytes while the transcript is
still there. A user who checks, sees zero, and moves on has been told something false about
their own data.

So the gate is not "did purge return `Ok`." It is **"are the bytes gone from the filesystem,"**
asserted against the real path.

## What already exists, and is already right

`purge_transcript_at` → `remove_transcript_file` calls `fs::remove_file` on the real
`storage_path` and returns the byte count it removed. It is not a metadata-only operation
today. Do not rebuild it; wire it.

Two properties it already has, which you inherit rather than implement:

- **It refuses to delete anything inside a project root.** `transcript_path_is_project_local`
  returns an `UnsafeProjectPath` error if the storage path resolves inside the project's own
  canonical root. Transcripts live under `$XDG_STATE_HOME`, so real ones proceed — but a
  transcript whose path had been redirected into a project would be refused rather than
  deleting a user's source file. **Do not weaken this**, and do not treat the refusal as a bug
  when a test hits it.
- **It preserves a tombstone.** The purged transcript's record is marked, not erased. That is
  what lets the product distinguish *"this run's transcript was deleted"* from *"this run never
  had one"* — two different facts, and the same distinction `0.11.0` already refused to collapse
  for truncated-versus-clean change detection.

## What the surface may not claim

- **Not that purge removes every trace.** A tombstone remains, by design, and the audit record
  this RFC adds is itself durable. Say what is removed: the transcript bytes.
- **Not that purge covers other projects.** Scope is per the decision in the task breakdown;
  whatever it is, the confirmation must name it. "Delete transcripts" without a scope is the
  kind of wording a user will read as narrower than it is.
- **Not that opting out removes what already exists.** Declining capture for future runs and
  deleting past transcripts are two separate acts, and a surface that offers both must not let
  either read as the other. This is the same shape as RFC-032's *"revoking stops it loading
  again; it does not undo anything that has already run."*

## The audit record

`transcript_purge` is a frozen RFC-013 family with no producer. Wiring it means a deletion is
recorded — which is the point of an audit trail — while also meaning **the store retains a
record of a privacy action the user took to remove data.** That trade is real and belongs in
the closeout, stated, rather than discovered by a user later.

Record that a purge occurred and its scope. **Never a path, never a byte count** that would let
a reader reconstruct what was removed. `DurableAuditRecordV1` has no free-text field and
`AuditReference` rejects `/`, so most of this is structural — but check the family's own
`valid_*` function before assuming which fields it even permits, the way PR-023-D found
`valid_config_change` had already settled a question that looked open.
