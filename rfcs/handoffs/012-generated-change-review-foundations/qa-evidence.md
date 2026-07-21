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

### PR-012-C - Baseline and Path Detector Harness

Implementation:

- Added `crates/tekstide-core/src/project/change_detection.rs`.
- Added `crates/tekstide-core/src/project/tests/change_detection.rs`.
- Updated `crates/tekstide-core/src/project.rs` exports.
- Updated `crates/tekstide-core/src/domain/changeset.rs` so `ChangeDetectionStatus::Failed` carries a bounded `ChangeDetectionFailureReason`.
- Updated `crates/tekstide-core/src/domain.rs` exports.

Implemented detector behavior:

- Added `GeneratedChangeDetector` and `GeneratedChangeDetectionPolicy`.
- Added metadata-only `ReviewBaseline` capture using filesystem snapshots.
- Added metadata-only changed-path comparison for created, modified, and deleted path evidence; renamed paths surface as a delete/create pair.
- Added project-relative changed-path validation for relative and absolute inputs.
- Added explicit changed-path kind metadata for files, directories, symlinks, deleted paths, and other filesystem entries.
- Added bounded entry/path limits with `ChangeDetectionStatus::Partial`.
- Added content-free failed states through `ChangeDetectionFailureReason`.
- Added explicit `GitStatus` detector unavailable/unsupported results for this slice; no Git subprocess or Git library behavior is claimed.

Security/privacy notes:

- Filesystem snapshots use `symlink_metadata`, file kind, file length, and modified timestamp metadata only.
- The detector does not read file contents, compute diffs, invoke Git, execute workspace automation, scan transcript bytes, persist durable audit records, apply patches, rollback changes, search-index contents, secure-delete files, or claim redaction.
- Absolute changed-path inputs must canonicalize under the ProjectSession root before becoming project-relative paths.
- Relative changed-path inputs reject `..` traversal before resolution.
- Escaping symlink paths are rejected by explicit validation, and recursive filesystem scanning records symlink entries without following their targets.
- Directory entries are tracked without using directory mtime as change evidence, avoiding noisy parent-directory changes from child edits.

Observed gates on 2026-07-21 before review request 086:

- `cargo fmt --all --check` passed.
- `cargo test -p tekstide-core project::tests::change_detection -- --quiet` passed; 7 tests passed, 0 failed.
- `cargo test -p tekstide-core -- --quiet` passed; 302 tests passed, 0 failed; doc tests had 0 tests.
- `cargo check --workspace` passed.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` passed.
- `git diff --check` passed.

Review follow-up:

- `.git-exclude/reviewed/tekstide-review-request-086-rfc012-pr012c-baseline-path-detector-harness-response.md` accepted PR-012-C with required follow-up on 2026-07-21.
- Updated changed-path validation to anchor below `canonical_root_path()` so symlinked ancestors above the project root do not reject ordinary in-root relative paths.
- Added a Unix regression test where `root_path` uses a symlinked ancestor and `canonical_root_path` differs.
- Updated degraded scan handling so `DetectedChanges.changed_paths` is empty for `Failed` and `Partial` statuses.
- Added regression coverage for changed-path limit partial scans and current-scan failure.
- Removed unused `ChangeDetectionFailureReason::TooManyEntries`.

Observed gates on 2026-07-21 after review request 086 follow-up:

- `cargo fmt --all --check` passed.
- `cargo test -p tekstide-core project::tests::change_detection -- --quiet` passed; 10 tests passed, 0 failed.
- `cargo test -p tekstide-core -- --quiet` passed; 305 tests passed, 0 failed; doc tests had 0 tests.
- `cargo check --workspace` passed.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` passed.

Re-review:

- `.git-exclude/reviewed/tekstide-review-request-087-rfc012-pr012c-baseline-path-detector-harness-rereview-response.md` accepted PR-012-C on 2026-07-21.

### PR-012-D - AgentRun Review Integration

Implementation:

- Updated `crates/tekstide-core/src/project/change_detection.rs`.
- Updated `crates/tekstide-core/src/project/session.rs`.
- Updated `crates/tekstide-core/src/domain/changeset.rs`.
- Updated `crates/tekstide-core/src/project/tests/change_detection.rs`.

Implemented integration behavior:

- `ReviewBaseline` can record the AgentRun id it was captured for.
- `GeneratedChangeDetector::capture_agent_run_filesystem_baseline` captures AgentRun-linked filesystem baselines.
- `ProjectSession::add_detected_generated_change_set` creates ChangeSets from detector output only when baseline and detection statuses are `Complete`.
- ProjectSession revalidates every detector path through `GeneratedChangeDetector::validate_changed_path` before ChangeSet creation.
- Matching baseline reference is required before ChangeSet creation.
- Strong AgentRun association requires a baseline captured for the same AgentRun, a closed or review-ready target AgentRun, and no other active, review-ready, or detached AgentRun blocking ownership clarity.
- Detached, non-closed, missing-baseline, concurrently active, or since-closed temporally overlapping AgentRun scenarios create unlinked ambiguous ChangeSets rather than attaching authorship to an AgentRun.
- Strongly associated ChangeSets attach to `AgentRun::change_set_ids`; ambiguous ChangeSets do not.
- Empty complete detections return `None` and create no ChangeSet.

Security/privacy notes:

- PR-012-D does not read file contents, compute diffs, invoke Git, execute workspace automation, scan transcript bytes, persist durable audit records, apply patches, rollback changes, search-index contents, secure-delete files, or claim redaction.
- Non-`Complete` detection status blocks ChangeSet creation.
- `ChangeSet::agent_run_detected` remains a convenience constructor; ProjectSession owns the strong-association gate for detector-created ChangeSets.
- AgentRun lifecycle process truth is preserved. PR-012-D does not rewrite completed, failed, cancelled, or detached runtime status to make review claims.

Observed gates on 2026-07-21 before review request 088:

- `cargo fmt --all --check` passed.
- `cargo test -p tekstide-core project::tests::change_detection -- --quiet` passed; 15 tests passed, 0 failed.
- `cargo test -p tekstide-core -- --quiet` passed; 310 tests passed, 0 failed; doc tests had 0 tests.
- `cargo check --workspace` passed.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` passed.
- `git diff --check` passed.

Review follow-up:

- `.git-exclude/reviewed/tekstide-review-request-088-rfc012-pr012d-agentrun-review-integration-response.md` accepted PR-012-D with required follow-up on 2026-07-21.
- Updated the strong-association gate so another run with `ended_at >= baseline.captured_at` blocks `Strong` association.
- Treating equality as overlap is intentional because `DomainTimestamp` is second-granularity.
- Added regression coverage for a since-closed overlapping run that forces an unlinked ambiguous ChangeSet.
- Updated evidence wording so "overlapping" includes since-closed temporal overlap, not only concurrently active runs.

Observed gates on 2026-07-21 after review request 088 follow-up:

- `cargo fmt --all --check` passed.
- `cargo test -p tekstide-core project::tests::change_detection -- --quiet` passed; 16 tests passed, 0 failed.
- `cargo test -p tekstide-core -- --quiet` passed; 311 tests passed, 0 failed; doc tests had 0 tests.
- `cargo check --workspace` passed.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` passed.
- `git diff --check` passed.

Re-review follow-up:

- `.git-exclude/reviewed/tekstide-review-request-089-rfc012-pr012d-agentrun-review-integration-rereview-response.md` required follow-up on 2026-07-21 because normal closed AgentRun lifecycle paths leave `ended_at` unset.
- Updated temporal overlap handling so closed bystander runs with missing `ended_at` block `Strong` association conservatively.
- Added regression coverage for a normal completed bystander with `ended_at == None` forcing an unlinked ambiguous ChangeSet.

Observed gates on 2026-07-21 after review request 089 follow-up:

- `cargo fmt --all --check` passed.
- `cargo test -p tekstide-core project::tests::change_detection -- --quiet` passed; 17 tests passed, 0 failed.
- `cargo test -p tekstide-core -- --quiet` passed; 312 tests passed, 0 failed; doc tests had 0 tests.
- `cargo check --workspace` passed.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` passed.
- `git diff --check` passed.

## Known Limitations

- Filesystem snapshot detection is implemented as metadata-only evidence.
- Git detection is explicitly unavailable/unsupported in PR-012-C; no Git subprocess or safe-library detector behavior is implemented yet.
- ChangeSet constructors do not validate project-relative root containment for `changed_files`; PR-012-D revalidates detector-created ChangeSets through the detector harness before adding them to ProjectSession.
- PR-012-D does not store baseline registries or prove wall-clock launch/scan ordering beyond the caller-provided AgentRun-linked baseline.
- No rendered diff/review UI, durable audit persistence, hunk-level patch application, rollback, search indexing, secure deletion, or redaction is claimed.
