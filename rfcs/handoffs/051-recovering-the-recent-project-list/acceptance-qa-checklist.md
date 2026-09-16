---
title: "RFC-051 — acceptance and QA checklist"
rfc: "RFC-051"
rfc_file: "../../accepted/051-recovering-the-recent-project-list.md"
source_rfc_status: "Accepted 2026-09-16 — M12"
target_milestone: "M12"
created: "2026-09-16"
---

# Acceptance and QA checklist

Every box is a property. A box whose plan assigns it elsewhere, or that cannot be satisfied as
written, stays unticked with the contradiction named — the reviewer's error to fix, not the
implementer's to paper over.

## PR-051-A — the store

- [ ] A corrupt live file with a good backup **recovers the list**, and every project keeps its id.
      **Ablation:** save before recovering; the test fails alone.
- [ ] **An unreadable live file is quarantined, not overwritten**, and when the quarantine rename
      fails, **nothing is saved**. **Ablation:** skip the quarantine; the test fails alone.
- [ ] **No backup is written from a session that started empty after a failed load** (§2, D2′).
      **Ablation:** write it regardless; the test fails alone.
- [ ] A corrupt or unreadable **backup** yields the reset outcome. **Nothing is salvaged from a
      partial file** (§3), asserted.
- [ ] The outcome is **one typed value** naming where a bad file went. **Grep:** the shell does not
      sequence load/recover/save itself.

## PR-051-B — the product

- [ ] The board says **recovered from the last saved copy**, or **reset with nothing to recover**, and
      nothing on a normal start. Each case ablated alone.
- [ ] The reset form still says earlier transcripts remain on disk and where (RFC-050 PR-050-C).
- [ ] **A recovered project the audit store has no grant for is demoted** (§4). **Ablation:** skip
      `verify_restored_trust` for recovered lists; the test fails alone.
- [ ] The book's "a reset loses that project's trust decisions" sentence is corrected **in the commit
      that makes it false**, and says what happens now.
- [ ] Live walkthrough: real project, corrupted state file, restart, list back with the same project,
      board saying so. Throwaway paths only.

## Whole-RFC

- [ ] `cargo fmt`, `clippy --workspace --all-targets -D warnings`, `git diff --cached --check` after
      staging, `rfc_docs_invariants`, and **three consecutive full-workspace runs with
      `--no-fail-fast`**, output redirected to files.
- [ ] Every new intermittent failure has a dated row in `test-process-leak.md`.
- [ ] `future-work.md`'s reset row is updated: what this RFC fixed, and what remains.
- [ ] Commits are pushed once the gate is green.

## Final Acceptance Decision

- [ ] Accepted.
- [ ] Accepted with required follow-up.
- [ ] Requires re-review after changes.

Reviewer notes:

```text
Pending review.
```
