# RFC-002 Acceptance / QA Checklist

Source RFC: [RFC-002](../../proposed/002-core-domain-model-projectsession-terminalsession-agentrun-auditevent.md)

## Model Checks

- [ ] Core entities exist or are represented in an accepted implementation plan.
- [ ] Each persisted entity has a stable ID strategy.
- [ ] Project ownership is explicit for project-scoped entities.
- [ ] Runtime-only handles are not serialized.
- [ ] AgentRun states use the canonical vocabulary.
- [ ] `WaitingForApproval` does not appear as a persisted/user-facing state.

## Tests / Evidence Required

- [ ] State transition tests or transition table review.
- [ ] Cross-project isolation tests or design evidence.
- [ ] Serialization/persistence fixture plan, or explicit no-persistence-yet note.
- [ ] Security note for ownership and audit linkage.
