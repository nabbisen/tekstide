---
title: "RFC-050 — acceptance and QA checklist"
rfc: "RFC-050"
rfc_file: "../../accepted/050-transcripts-from-earlier-runs.md"
source_rfc_status: "Accepted 2026-09-13 — M12"
target_milestone: "M12"
created: "2026-09-13"
---

# Acceptance and QA checklist

Every box is a property. A box whose plan assigns it elsewhere, or that cannot be satisfied as
written, stays unticked with the contradiction named. That is the reviewer's error to fix, not the
implementer's to paper over.

## PR-050-A — the lock, the id, the origin

- [ ] The writer holds an exclusive lock for its lifetime. A second handle gets `WouldBlock` while
      the writer lives and succeeds after it drops. **Ablation:** remove the lock; that assertion fails
      alone.
- [ ] **A writer that cannot lock does not write.** The launch proceeds without capture, and the
      run's reason is distinct from opt-out.
- [ ] `AgentRunId` rejects a non-UUID string.
- [ ] `Transcript`'s origin is an exhaustive type. A found record names no terminal or run the
      session lacks. **Grep:** no id is fabricated to satisfy an ownership check.
- [ ] The liveness predicate matches on origin first. **Ablation:** add an origin variant; the compile
      error appears only in the predicate.
- [ ] Nothing is loaded from disk yet.

## PR-050-B — load, count, purge

- [ ] **A real restart** (a fresh `AppState` restored from saved recent-project state): the earlier
      transcript is counted, and purge removes **the file on disk**. **Ablation:** skip loading; the
      test fails alone.
- [ ] A file whose lock another handle holds loads as live, survives purge, and is reported.
      **Ablation:** remove the probe; the test fails alone.
- [ ] The probe drops its lock at once. Assert that a later `try_lock` succeeds.
- [ ] Skipped **and still present**: a symlinked run directory (pointing outside the state root), a
      non-UUID name, a stray file, and a `transcript.log` at the wrong depth.
- [ ] Loaded records are purged through `purge_transcript_at` only. **Grep:** no other
      `remove_file` reaches `transcripts/`.
- [ ] `last_write_at` comes from mtime, read at load. **Grep:** no filesystem read inside retention
      selection (RFC-049 D7).
- [ ] The app-wide figure includes a closed project. Unclaimed bytes are computed separately.
- [ ] A directory whose id is not in the recent list is not loaded, not deleted, and counted as
      unclaimed.
- [ ] The dialog's count is of retained transcripts, not tombstones. Verify the reading first; if it
      was wrong, say so here.
- [ ] **No test enumerates the real state root.** Every fixture is `mktemp -d`, reached through the
      `cfg(test)` split.

## PR-050-C — say it, and stop saying the old thing

- [ ] Trust Settings shows unclaimed bytes and where they are when there are some, and nothing when
      there are none. Each case is ablated alone. The wording fits the reset case (D6′): it does not
      imply the user did something, or that anything is dangerous.
- [ ] The purge dialog names files still being written; absent when none. Each case is ablated alone.
- [ ] The request-387 disclosure is removed from the book, `README.md` and the changelog **in the
      same commit** that makes purge true.
- [ ] The changelog defect entry names `0.12.0` through `0.18.0`, what a user saw, and that deleting
      `transcripts/` was the only complete removal.
- [ ] Live walkthrough of launch, quit, restart, the count shown, purge, and the file gone, against
      throwaway state only.

## Whole-RFC

- [ ] `cargo fmt`, `clippy --workspace --all-targets -D warnings`, `git diff --check`,
      `rfc_docs_invariants`, and three consecutive full-workspace runs with output redirected to a
      file.
- [ ] Every new intermittent failure has a row in `test-process-leak.md`.
- [ ] Commits are pushed once the gate is green.

## Final Acceptance Decision

- [ ] Accepted.
- [ ] Accepted with required follow-up.
- [ ] Requires re-review after changes.

Reviewer notes:

```text
Pending review.
```
