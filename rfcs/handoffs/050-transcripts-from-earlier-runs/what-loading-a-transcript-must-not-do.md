---
title: "What loading a transcript must not do"
rfc: "RFC-050"
rfc_file: "../../accepted/050-transcripts-from-earlier-runs.md"
source_rfc_status: "Accepted 2026-09-13 — M12"
target_milestone: "M12"
created: "2026-09-13"
---

# What loading a transcript must not do

**Required reading before writing code.** Whatever the loader accepts, purge and retention can
delete. The failure that matters is not loading too little. It is loading something that was never a
transcript, or treating a file something is still writing as a leftover.

## §1 Nothing is loaded that this product did not write

- **Symlinks are refused at every level.** Use `symlink_metadata`, never `metadata`. A symlinked run
  directory pointing somewhere else would make purge delete a file the user owns outside the state
  root.
- **Only a regular file named `transcript.log`, exactly two levels under `transcripts/`.**
- **Both directory names must parse as UUIDs.** `ProjectId::from_persisted` already checks this;
  `AgentRunId` gains the same check.
- **Anything unrecognised is skipped and never deleted.** It is counted in the unclaimed figure, so
  the user can see it.
- RFC-033's containment checks and purge's project-local refusal stay exactly as they are.

## §2 A locked file is live, and a writer never writes unlocked

- The writer takes an exclusive `try_lock` when it creates the file and holds it while it writes.
  **If it cannot take the lock, it does not capture.** The run starts with capture disabled and its
  detail says why. A writer that wrote unlocked would look dead to every other process, which is how
  §2 of RFC-049's risk document gets broken by a different route.
- The loader probes each file **once, at load**, and drops the lock at once. A held lock means live:
  never marked, never selected, never purged.
- **User purge obeys this too.** Today's purge unlinks a file under a running writer. After this
  slice it skips that file, and the dialog names files still being written. When there are none, the
  dialog says nothing about them.
- The probe was measured before acceptance: a second handle in the same process gets `WouldBlock`
  while the writer holds the lock. **Test in-process; do not ship a test that needs a second binary to
  be meaningful.**

## §3 One deletion path

Loaded records are purged through `purge_transcript_at`, like launched ones. **No code walks the
directory to delete.** If a test is easier to write that way, the test is wrong.

## §4 A found record claims only what is known

- **No invented terminal or run ids.** The record carries its origin as an exhaustive type. RFC-049's
  liveness predicate matches on origin before it looks at any run.
- `last_write_at` comes from mtime, read at load. `created_at` holds a value no later than the true
  one, and **no surface may display it as a creation time.**
- **Counts are of retained transcripts.** Check whether the purge dialog's count includes purged
  tombstones today (`transcript_local_data_summary_for` uses `transcripts().len()`). This is a
  reading of the code, not a measurement: verify it before acting on it.

## §5 No test reads the real state root

The loader takes the state root as a parameter and is reached through the same `cfg(test)` split as
`resolve_agent_run_state_dir`. **A test that enumerates `~/.local/state/tekstide` reads the owner's
private transcripts.** Every fixture is a `mktemp -d`. Any committed screenshot shows only
throwaway state.

## §6 The disclosure leaves in the commit that makes it false

The disclosure added after request 387, in the book, `README.md` and the changelog, is removed in the
same commit that makes purge true. It goes neither earlier nor later. The changelog entry states the
defect plainly: `0.12.0` through `0.18.0`, what a user saw, and that deleting `transcripts/` was the
only complete removal.
