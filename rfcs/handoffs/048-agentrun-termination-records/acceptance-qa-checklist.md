---
title: "RFC-048 — acceptance and QA checklist"
rfc: "RFC-048"
rfc_file: "../../accepted/048-agentrun-termination-records.md"
source_rfc_status: "Accepted 2026-09-16 — M12"
target_milestone: "M12"
created: "2026-09-16"
---

# Acceptance and QA checklist

Every box is a property. A box whose plan assigns it elsewhere, or that cannot be satisfied as
written, stays unticked with the contradiction named — the reviewer's error to fix, not the
implementer's to paper over.

## PR-048-A — the producer and its one path

- [ ] A run that exits records `Terminated`/`ProcessExited` after its `Started`, **read back from a
      real store**, same operation id.
- [ ] A run killed through the close flow records `Terminated`/`ProcessTerminated`.
- [ ] A post-start runtime failure records `Terminated`/`RuntimeFailure`.
- [ ] **A detached run records nothing.** **Ablation:** record one anyway; the test fails alone.
- [ ] **The record's fields are asserted exhaustively**, so no exit status, signal, or
      `BoundedRuntimeSummary` text can have reached it (§2). **Ablation:** put the exit status in a
      field; the test fails alone.
- [ ] **A degraded store does not block termination**: the run still reaches its end status.
      **Ablation:** `append_required`; the test fails alone.
- [ ] **The store refuses a termination with no `started` phase** (D6), asserted against the store.
- [ ] **One path, by construction** (§4): production calls the coordinator, never
      `ProjectSession::apply_agent_terminal_outcome`. **Grep**, and **ablation:** delete the recording
      from the coordinator; a test fails alone.

## PR-048-B — the documentation

- [ ] `crates/tekstide-core/README.md`'s "no termination record exists" sentence is corrected **in the
      commit that makes it false**, and says what a detached run's trail does instead.
- [ ] The changelog says both halves in a user's words.
- [ ] The book's audit description states the limit.
- [ ] **No text claims the trail answers whether a run is still going.** It does not.

## Whole-RFC

- [ ] `cargo fmt`, `clippy --workspace --all-targets -D warnings`, `git diff --cached --check` after
      staging, `rfc_docs_invariants`, and **three consecutive full-workspace runs with
      `--no-fail-fast`**, output redirected to files.
- [ ] Every new intermittent failure has a dated row in `test-process-leak.md`.
- [ ] Commits are pushed once the gate is green.

## Final Acceptance Decision

- [ ] Accepted.
- [ ] Accepted with required follow-up.
- [ ] Requires re-review after changes.

Reviewer notes:

```text
Pending review.
```
