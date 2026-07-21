# RFC-012: Generated Change Review Foundations - QA Evidence

Status: Proposed
Date opened: 2026-07-21
Date accepted: Pending

## Scope

RFC-012 defines headless generated-change review foundations for Tekstide-created AgentRuns: metadata-only changed-path detection, conservative AgentRun association, ChangeSet review state, and ProjectSession review counts.

Evidence in this file must not be used to claim rendered diff/review UI, durable audit storage, command approval, hunk-level patch application, rollback, search indexing, secure deletion, redaction, or file-content indexing unless later reviewed implementation explicitly supports those claims.

## Design Review

Review request 084 was accepted with required follow-up on 2026-07-21 in `.git-exclude/reviewed/tekstide-review-request-084-rfc012-generated-change-review-foundations-design-response.md`.

Required amendments applied:

- Removed `Detached` from the strong AgentRun association state list.
- Documented that detached/orphaned AgentRuns are weak or ambiguous unless later reviewed runtime evidence proves the process boundary is closed and ownership is unambiguous.
- Added Git detector safety policy for subprocess provenance, direct invocation without shell, alias avoidance, sanitized environment, project-local `PATH` avoidance, workspace hooks/config automation avoidance, bounded timeout/output, content-free diagnostics, and unavailable/unsupported fallback behavior.

## Implementation Evidence

### PR-012-B - ChangeSet Review Model

Implementation:

- Updated `crates/tekstide-core/src/domain/changeset.rs`.
- Updated `crates/tekstide-core/src/domain.rs`.
- Updated `crates/tekstide-core/src/domain/agent.rs`.
- Updated `crates/tekstide-core/src/project/session.rs`.
- Updated `crates/tekstide-core/src/project.rs`.
- Updated focused domain and ProjectSession tests.

Implemented model behavior:

- Added `ChangeDetectionSource`.
- Added `ChangeDetectionStatus`.
- Added `ChangeAssociationConfidence`.
- Added `ChangeSetSummary`.
- Added `ReviewStateTransitionError`.
- Added `ChangeSet::agent_run_detected`.
- Added `ChangeSet::with_detection`.
- Added `ChangeSet::with_association_confidence`.
- Added `ChangeSet::with_artifact_ref`.
- Added explicit `ChangeSet::transition_review_to` validation.
- Added bounded, content-free `ChangeSet::bounded_summary`.
- Added AgentRun ChangeSet attachment helper.
- Added ProjectSession review-state transition helper that refreshes review-ready counts.

Security/privacy notes:

- ChangeSet summaries include ids, project id, optional AgentRun id, changed path metadata, counts, detector state, association confidence, review state, and timestamps only.
- PR-012-B does not read project files, compute diffs, invoke Git, scan the filesystem, render GUI review surfaces, persist durable audit records, apply patches, rollback changes, search-index contents, secure-delete files, or claim redaction.
- Review state transitions are metadata-only and do not imply tests passed.
- ProjectSession attaches ChangeSet ids to AgentRuns only when ownership/reference checks pass.

Observed gates on 2026-07-21 before review request 085:

- `cargo fmt --all` passed.
- `cargo test -p tekstide-core domain::tests -- --quiet` passed; 32 tests passed, 0 failed.
- `cargo test -p tekstide-core project::tests::collections -- --quiet` passed; 11 tests passed, 0 failed.
- `cargo test -p tekstide-core -- --quiet` passed; 294 tests passed, 0 failed; doc tests had 0 tests.
- `cargo fmt --all --check` passed.
- `cargo check --workspace` passed.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` passed.
- `git diff --check` passed.

Review follow-up:

- `.git-exclude/reviewed/tekstide-review-request-085-rfc012-pr012b-changeset-review-model-response.md` accepted PR-012-B with required follow-up on 2026-07-21.
- Updated review-state transitions so `Accepted` and `Rejected` ChangeSets may transition to `Superseded`.
- Added explicit regression coverage for later detection superseding accepted and rejected ChangeSets.
- Recorded that `ChangeSet::agent_run_detected` is a model convenience constructor and does not enforce RFC-012 strong-association preconditions by itself.
- PR-012-D must gate `Strong` association on the RFC-012 AgentRun association preconditions.
- Replaced the summary privacy assertion with sentinels proving caller-supplied `ChangeSet.summary` and artifact reference strings do not appear in `ChangeSetSummary` debug output.

Observed gates on 2026-07-21 after review request 085 follow-up:

- `cargo fmt --all` passed.
- `cargo test -p tekstide-core domain::tests -- --quiet` passed; 33 tests passed, 0 failed.
- `cargo test -p tekstide-core project::tests::collections -- --quiet` passed; 11 tests passed, 0 failed.
- `cargo test -p tekstide-core -- --quiet` passed; 295 tests passed, 0 failed; doc tests had 0 tests.
- `cargo fmt --all --check` passed.
- `cargo check --workspace` passed.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` passed.
- `git diff --check` passed.

## Known Limitations

- PR-012-B is model-only.
- No baseline capture or changed-path detector is implemented yet.
- ChangeSet constructors do not validate project-relative root containment for `changed_files`; PR-012-C must enforce that in the detector.
- No Git subprocess or safe-library detector behavior is implemented yet.
- No AgentRun lifecycle integration is implemented yet.
- No rendered diff/review UI, durable audit persistence, hunk-level patch application, rollback, search indexing, secure deletion, or redaction is claimed.
