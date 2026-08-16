---
title: "RFC-022: Adapter Spawn and the Command Approval Surface - Acceptance / QA Checklist"
rfc: "RFC-022"
rfc_file: "../../proposed/022-adapter-spawn-and-command-approval-surface.md"
status: "Open"
target_milestone: "M11"
created: "2026-08-16"
---

# Acceptance / QA Checklist

Every unchecked line at closeout carries a stated reason.

## The reference adapter (PR-022-B)

- [ ] Speaks the real protocol against the real socket and coordinator, not a mock.
- [ ] Full round trip proven; both approve and reject exercised.
- [ ] Token read from the environment as a real child would.
- [ ] Missing/wrong token behaviour defined and tested.
- [ ] Documented as a test-and-proof artifact, not a product feature.

## Spawn and token delivery (PR-022-C)

- [ ] Spawn path distinct from `spawn_shell`, inside RFC-009's boundary.
- [ ] `.env_clear()` preserved; token set, nothing inherited.
- [ ] `ExplicitAllowlist` still rejected, pinned by a test.
- [ ] `inject_token_into_environment` has exactly one production call site, enumerated.
- [ ] A real spawned adapter completes a real round trip, headless.
- [ ] Transcript capture exercised for the first time in production; what it proves stated.

## AgentRun creation and route (PR-022-D)

- [ ] `launch_agent_run_with_runtime` has a production caller.
- [ ] A user can start a run from the GUI; binding checked mechanically, no `Reserved`
      collision.
- [ ] A selected-run concept exists and its shape is stated.
- [ ] Resource limit enforced in core, not at the call site.
- [ ] Refusal is a typed error the shell renders.

## The dialog (PR-022-E)

- [ ] Proposed command escaped at the widget; raw bytes survive the model.
- [ ] **Falsifiable claim tested**: a bidi override in `argv` renders visibly.
- [ ] No double-escaping.
- [ ] The cooperative limit appears on the surface; wording quoted and justified.
- [ ] No claim that rejection prevents execution.
- [ ] Modal exclusivity proven under a live positive control.
- [ ] **No decision recorded for an undeliverable proposal**; the dialog says the
      connection is gone rather than silently recording one.
- [ ] Proven against a real exited adapter, not a synthesised closed socket; ablated.
- [ ] Owner's answer to open question 3 (mid-edit interruption) incorporated.
- [ ] `command_approval` has its first real producer.

## Honesty (PR-022-F)

- [ ] Claim statement checked against RFC-022's own text.
- [ ] No claim of enforcement.
- [ ] No claim that real AI CLIs are supported.
- [ ] No claim that the token is a security boundary.
- [ ] What this unblocks for RFC-020 stated precisely.
- [ ] `future-work.md`'s adapter-spawn theme updated in the same commit.
- [ ] Every unchecked line above carries a stated reason.

## Final Acceptance Decision

- [ ] Accepted
- [ ] Accepted with required follow-up
- [ ] Rejected

Reviewer notes:
