---
title: "RFC-046 — acceptance and QA checklist"
rfc: "RFC-046"
rfc_file: "../../accepted/046-managed-agentrun-audit-trail.md"
source_rfc_status: "Accepted 2026-09-06 — M12"
target_milestone: "M12"
created: "2026-09-06"
---

# Acceptance and QA checklist

Every box is a property, not a task. **A box whose plan assigns it to a different slice must be left
unticked with the contradiction named** — that happened on RFC-047 PR-047-A and the checklist was
wrong, not the implementer.

## PR-046-A — the producer is safe to call

- [x] A launch **succeeds against a genuinely degraded audit store**. **Ablation: revert
      `append_observation` to `append_required` and watch this specific test fail.** Done —
      `managed_launch_still_creates_a_process_when_authorization_cannot_persist`, ablated exactly
      this way, fails immediately (`Err(RequiredAuditUnavailable(..))` before the launch runs).
      **Deviation from "not a mock", flagged for review**: used `RecordingWriter::fail_on(1)` (a
      real `AuditRecordWriter` impl, this file's own established write-failure technique) rather
      than PR-047-B's store-won't-open fixtures, since those produce the wrong shape — a store that
      never opens, not one that opens and then refuses one write. See `qa-evidence.md` for the
      reasoning and where a genuinely unopenable store belongs instead (PR-046-C).
- [x] No `Started` record is attempted when its `Authorized` did not persist, **in its own test
      under its own name** (response 367 R1). Split into `no_started_write_is_attempted_when_its_
      own_authorized_did_not_persist`. Forcing the `Started` write unconditionally now fails **only**
      this test, not the "still creates a process" one — confirmed by ablating both directions.
      Splitting surfaced a second entanglement: `audit_status`/`health.status()` are themselves
      sensitive to whether `Started` is attempted (a wrongly-attempted `Started` write succeeds
      under `fail_on(1)`, which would flip both), so those assertions moved here too rather than
      staying split across both tests. The store's own `MissingAuthorization` rejection is the
      backstop, never the mechanism — `writer.attempt_count == 1` and `writer.records.is_empty()`
      prove the write was never attempted, not merely that it would have been rejected.
- [x] `Plain` is still rejected, with the reason commented. Unaffected by D1 — rejection happens
      before any write is attempted (`writer.attempt_count == 0`,
      `plain_agent_launch_is_not_relabelled_as_durably_authorized`).
- [x] Renamed to `launch_audited_agent_run`, with **no alias** for the old name. Grepped the tree
      for the old name after the rename — the only hits were a stale doc-comment cross-reference on
      a different method, fixed alongside a second stale claim in the same comment
      (`append_required` "for its first phase", no longer true).
- [x] The producer's doc comment states what the trail answers and what it does not (§5). Both
      sentences from the risk document, verbatim, in the function's own doc comment.

## PR-046-B — nothing is lost by becoming audited

- [x] `AuditedAgentLaunch` carries the approval endpoint, and the audited path returns everything
      `launch_agent_run_with_runtime` returns. `prepare_agent_run_launch`'s own return value,
      previously discarded outright, is now captured and threaded through. `Clone`/`Eq`/`PartialEq`
      dropped from `AuditedAgentLaunch` (`ApprovalChannelEndpoint` holds a live `UnixListener` and
      implements neither) — grepped the tree first; nothing outside this producer's own tests used
      either.
- [x] A **`Managed`** launch returns `Some(endpoint)` — the only assertion that fails on all three
      losses: field deleted, hardcoded `None`, dropped in the plumbing.
      `managed_launch_carries_a_real_bound_approval_endpoint`, following §4's own recipe (`Managed`
      level, `structured_action_approval = true`, `.with_approval_channel(<short path>)` — a short
      path, per §4's own `SocketPathTooLong` trap). **Ablated**: hardcoded `approval_endpoint: None`
      in the struct literal (the "silently dropped in the plumbing" defect, response 227's own
      historical shape) — failed only this test; `supervised_launch_does_not_bind_an_approval_
      endpoint` stayed green, since `None` is also its own correct answer.
      **Verified independently by the reviewer (response 370):** hardcoding
      `approval_endpoint: None` — response 227's own historical shape — fails this test
      alone, with the Supervised sibling correctly staying green.
- [x] A **Supervised** launch returns `None`, kept alongside it: Supervised must not bind a channel.
      `supervised_launch_does_not_bind_an_approval_endpoint` (renamed from `managed_launch_carries_
      the_approval_endpoint_field_through`, which the name no longer fit once its sibling existed).
- [x] The field's presence is a **compile-time** dependency of a test, not merely a runtime one.
      Either test above reads the field, so **ablating by deleting the field itself** fails the
      build (`E0609` at either assertion, `E0560` at the struct literal).

## PR-046-C — the trail exists in production

- [x] A real launch through the production path writes `Authorized` then `Started`, **read back from
      the store**, not asserted at the call site.
      `attempt_agent_run_launch_with_profile_writes_authorized_then_started_to_a_real_store` —
      launches through production's own real entry point, reopens the store, queries it. **Ablated**:
      reverted to the old unaudited call — zero matching records. Restored: passes.
- [x] The same launch with a **degraded** store still launches, still registers the approval channel,
      and writes no records. This is the box that proves D1 reached production.
      `attempt_agent_run_launch_with_profile_still_launches_and_registers_with_an_unopenable_store` —
      EVIDENCE-2's own unopenable-store fixture, a real `Managed` launch with a configured approval
      channel. **Ablated**: same revert — `audit_health` stayed `Healthy` (the unaudited path never
      touches it). Restored: passes.
- [x] The generated-change baseline is still captured and still keyed to the launched run. Unchanged
      code path; existing regression tests (`agent_run_change_baselines` keying) stayed green
      unmodified.
- [x] Restricted Mode still records its blocked launch; run-limit, validation and plan-transition
      still record nothing (D5). Unchanged code path (the refusal branches sit above the launch call
      this slice touched); existing regression tests stayed green unmodified.
- [x] Every post-launch step of the current path still happens, in the same order. **Substitution,
      not rewrite.** Only the launch call and its return shape changed; baseline insertion,
      terminal-id re-read, approval-channel registration, and pane creation are byte-for-byte the
      same code, confirmed by the unmodified regression tests above passing unchanged.

## Whole-RFC

- [x] `cargo fmt`, `clippy --workspace --all-targets -D warnings`, `git diff --check`,
      `rfc_docs_invariants` clean.
- [x] Three consecutive full-workspace runs, green, with any recurring flake given a dated row in
      `test-process-leak.md`. **485 + 4 + 746, fully green** every time — no flake.
- [x] **The trail's boundary is documented where a reader meets it** — not only in this pack. The new
      call site in `shell.rs` points to `launch_audited_agent_run`'s own doc comment explicitly,
      rather than leaving a production-code reader to find the boundary only in this handoff.
- [x] `crates/tekstide-core/README.md` does not claim more than the trail delivers. RFC-036 corrected
      it once for exactly this producer; check it again rather than assuming that correction still
      fits. **Rewritten**: no longer says "no production caller" (it does now); states best-effort,
      never a precondition, and the D3/D5 boundaries (no termination record, no new refusal record)
      explicitly rather than leaving them to be inferred.

- [ ] **Live evidence not captured, documented rather than silently dropped.** RFC-047 PR-047-C
      already spent six rounds establishing `wtype` does not reliably reach this application in this
      environment; the same route (`Ctrl+Alt+A`) is needed here. Not re-litigated — the task
      breakdown's own words: "the store read-back is the load-bearing evidence here, not a
      screenshot," and the two required tests above provide exactly that, through production's real
      entry point.
