# RFC-002 Acceptance / QA Checklist

Source RFC: [RFC-002](../../done/002-core-domain-model-projectsession-terminalsession-agentrun-auditevent.md)

## Model Checks

- [x] Core entities exist or are represented in an accepted implementation plan.
- [x] Each persisted entity has a stable ID strategy.
- [x] Project ownership is explicit for project-scoped entities.
- [x] Runtime-only handles are not serialized.
- [x] AgentRun states use the canonical vocabulary.
- [x] `WaitingForApproval` does not appear as a persisted/user-facing state.

## Tests / Evidence Required

- [x] State transition tests or transition table review.
- [x] Cross-project isolation tests or design evidence.
- [x] Serialization/persistence fixture plan, or explicit no-persistence-yet note.
- [x] Security note for ownership and audit linkage.

## Implementation Evidence

- Implemented in commit `3432298 RFC-002: implement core domain and ProjectSession model`.
- Core types are exposed through `tekstide-core` modules `domain` and `project`.
- Project-scoped entities carry explicit `ProjectId` ownership and reject cross-project attachment.
- `ProjectSession` keeps runtime process handles out of the persisted model and uses metadata summaries until later persistence/runtime RFCs define storage.
- AgentRun lifecycle uses `AwaitingApproval`; no `WaitingForApproval` state is part of the implemented vocabulary.
- Validation evidence observed before closeout: `cargo fmt --check`, `cargo test --all-targets`, and `cargo clippy --all-targets --all-features -- -D warnings`.
