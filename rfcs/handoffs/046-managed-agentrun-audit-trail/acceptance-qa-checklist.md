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

- [ ] A launch **succeeds against a genuinely degraded audit store**, driven through a real degraded
      store (PR-047-B's fixtures), not a mock. **Ablation: revert `append_observation` to
      `append_required` and watch this specific test fail.** A green ablation is a defect in the
      ablation.
- [ ] No `Started` record is written when its `Authorized` did not persist. The store's own
      `MissingAuthorization` rejection is the backstop, never the mechanism.
- [ ] `Plain` is still rejected, with the reason commented.
- [ ] Renamed to `launch_audited_agent_run`, with **no alias** for the old name.
- [ ] The producer's doc comment states what the trail answers and what it does not (§5).

## PR-046-B — nothing is lost by becoming audited

- [ ] `AuditedAgentLaunch` carries the approval endpoint, and the audited path returns everything
      `launch_agent_run_with_runtime` returns.
- [ ] A test asserts the endpoint is **carried**, and **fails if the field is deleted** — verified by
      deleting it, not by inspection. Passing "because the launch succeeded" is not this test.

## PR-046-C — the trail exists in production

- [ ] A real launch through the production path writes `Authorized` then `Started`, **read back from
      the store**, not asserted at the call site.
- [ ] The same launch with a **degraded** store still launches, still registers the approval channel,
      and writes no records. This is the box that proves D1 reached production.
- [ ] The generated-change baseline is still captured and still keyed to the launched run.
- [ ] Restricted Mode still records its blocked launch; run-limit, validation and plan-transition
      still record nothing (D5).
- [ ] Every post-launch step of the current path still happens, in the same order. **Substitution,
      not rewrite.**

## Whole-RFC

- [ ] `cargo fmt`, `clippy --workspace --all-targets -D warnings`, `git diff --check`,
      `rfc_docs_invariants` clean.
- [ ] Three consecutive full-workspace runs, green, with any recurring flake given a dated row in
      `test-process-leak.md`.
- [ ] **The trail's boundary is documented where a reader meets it** — not only in this pack.
- [ ] `crates/tekstide-core/README.md` does not claim more than the trail delivers. RFC-036 corrected
      it once for exactly this producer; check it again rather than assuming that correction still
      fits.
