---
title: "RFC-022: Adapter Spawn and the Command Approval Surface - Acceptance / QA Checklist"
rfc: "RFC-022"
rfc_file: "../../done/022-adapter-spawn-and-command-approval-surface.md"
status: "Open"
target_milestone: "M11"
created: "2026-08-16"
---

# Acceptance / QA Checklist

Every unchecked line at closeout carries a stated reason.

## The reference adapter (PR-022-B)

- [x] Speaks the real protocol against the real socket and coordinator, not a mock.
- [x] Full round trip proven; both approve and reject exercised.
- [x] Token read from the environment as a real child would.
- [x] Missing/wrong token behaviour defined and tested.
- [x] Documented as a test-and-proof artifact, not a product feature.

## Spawn and token delivery (PR-022-C)

- [x] Spawn path distinct from `spawn_shell`, inside RFC-009's boundary.
- [x] `.env_clear()` preserved; token set, nothing inherited.
- [x] `ExplicitAllowlist` still rejected, pinned by a test.
- [x] `inject_token_into_environment` has exactly one production call site, enumerated.
- [x] A real spawned adapter completes a real round trip, headless.
- [x] Transcript capture exercised for the first time in production; what it proves stated
      (`launch_real_managed_agent_run`'s real GUI launches run with transcript capture
      enabled by default via `with_local_bounded_transcript`; `a_managed_launch_can_bind_its_approval_channel_without_transcript_capture`
      and `a_managed_launch_still_fails_closed_with_no_state_root_configured_at_all` prove
      the opted-out and neither-configured cases separately, headless).

## AgentRun creation and route (PR-022-D)

- [x] `launch_agent_run_with_runtime` has a production caller.
- [x] A user can start a run from the GUI; binding checked mechanically, no `Reserved`
      collision.
- [ ] A selected-run concept exists and its shape is stated. **Reason**: deliberately
      deferred to RFC-020, not decided by this RFC. `task-breakdown-pr-plan.md` itself frames
      this as RFC-020's own question ("the selected-run concept from D — RFC-020's report
      surface needs it"). PR-022-E built `ApprovalHistory` alongside the still-open
      `AgentRunDetail` surface without deciding its shape (responses 233/234 both name this
      explicitly as out of scope) -- `ApprovalHistory` carries no selected-run id at all,
      which is exactly why it did not need this question answered to ship.
- [x] Resource limit enforced in core, not at the call site.
- [x] Refusal is a typed error the shell renders.

## The dialog (PR-022-E)

- [x] Proposed command escaped at the widget; raw bytes survive the model.
- [x] **Falsifiable claim tested**: a bidi override in `argv` renders visibly.
- [x] No double-escaping.
- [x] The cooperative limit appears on the surface; wording quoted and justified.
- [x] No claim that rejection prevents execution.
- [x] Modal exclusivity proven under a live positive control.
- [x] **No decision recorded for an undeliverable proposal**; the dialog says the
      connection is gone rather than silently recording one.
- [x] Proven against a real exited adapter, not a synthesised closed socket; ablated.
- [x] Owner's answer to open question 3 (mid-edit interruption) incorporated.
- [x] `command_approval` has its first real producer.

## Honesty (PR-022-F)

- [x] Claim statement checked against RFC-022's own text.
- [x] No claim of enforcement.
- [x] No claim that real AI CLIs are supported.
- [x] No claim that the token is a security boundary.
- [x] What this unblocks for RFC-020 stated precisely.
- [x] `future-work.md`'s adapter-spawn theme updated in the same commit.
- [x] Every unchecked line above carries a stated reason.

## Final Acceptance Decision

- [ ] Accepted
- [ ] Accepted with required follow-up
- [ ] Rejected

Reviewer notes:
