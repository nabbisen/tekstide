# RFC-011: Transcript Retention and Local Data Policy - QA Evidence

Status: Proposed
Date opened: 2026-07-21
Date accepted: Pending

## Scope

RFC-011 defines bounded local transcript capture, retention, opt-out, purge, and local data path policy for Tekstide-created AgentRuns.

Evidence in this file must not be used to claim durable audit storage, GUI transcript/review panes, generated-change review UI, command approval, provider cloud integration, search indexing, cloud sync, secure deletion, or transcript redaction unless later reviewed implementation explicitly supports those claims.

## Design Review

Review request 076 was accepted with required amendments on 2026-07-21 in `.git-exclude/reviewed/tekstide-review-request-076-rfc011-transcript-retention-local-data-policy-design-response.md`.

Required amendments applied:

- Added aggregate transcript retention guardrails: 32 MiB per transcript, 256 MiB per project, 1 GiB app-wide, and 30 days retained.
- Added local-data accounting requirements for project/app retained transcript bytes and transcript counts.
- Defined deterministic aggregate cleanup ordering: inactive transcripts first, oldest first by retention metadata.
- Defined active-writer behavior when aggregate budget is exhausted.
- Made content-free tombstone transcript preservation the default purge/reference policy.
- Limited reference clearing to cases where tombstone preservation would retain sensitive path, content, or environment metadata.

## Implementation Evidence

### PR-011-A - Transcript Policy and Path Model

Status: accepted with notes.

Implementation:

- Added `crates/tekstide-core/src/transcript.rs`.
- Added `crates/tekstide-core/src/transcript/policy.rs`.
- Added `crates/tekstide-core/src/transcript/path.rs`.
- Added `crates/tekstide-core/src/transcript/tests.rs`.
- Exported the new `transcript` module from `crates/tekstide-core/src/lib.rs`.
- Added transcript capture policy vocabulary:
  - `TranscriptCaptureMode`
  - `TranscriptCapturePolicy`
  - `TranscriptRetentionLimits`
  - `TranscriptRetentionState`
  - `TranscriptBudgetScope`
  - `TranscriptLocalDataSummary`
- Added transcript storage path vocabulary:
  - `TranscriptPathRequest`
  - `TranscriptPathResolver`
  - `TranscriptStoragePath`
  - `TranscriptPathError`
  - `TranscriptPathErrorReason`

Implemented policy gates:

- default AgentRun capture policy is local bounded;
- default retention is 32 MiB per transcript, 256 MiB per project, 1 GiB app-wide, and 30 days;
- transcript byte persistence is rejected when aggregate limits are unbounded or inconsistently ordered;
- disabled capture never permits transcript byte persistence;
- required local bounded capture is modeled but explicitly reports that launch should reject when capture is unavailable;
- local-data summary exposes retained byte counts and transcript count without content fields;
- purged transcript state is represented as a tombstone.

Implemented path gates:

- state root must be absolute;
- project root must be absolute;
- state root must exist and canonicalize to a directory;
- project root must exist and canonicalize to a directory;
- state root inside the project root is rejected;
- generated transcript path is structurally under the canonical state root;
- generated transcript path is structurally outside the canonical project root;
- project id and AgentRun id are used only as sanitized path components;
- Unix symlinked state roots resolving inside a project root are rejected.

Security/privacy notes:

- PR-011-A adds model and path preflight only. It does not write transcript bytes.
- No writer, launch integration, purge operation, durable audit, GUI transcript/review surface, generated-change review, search indexing, secure deletion, or redaction claim is introduced.
- Local-data summaries expose counts and byte totals only; no transcript snippets, prompt text, environment values, terminal output, or file contents are modeled.
- Tombstone behavior is represented in policy state, but actual purge operations are deferred to PR-011-D.

Observed gates on 2026-07-21:

- `cargo fmt --all --check` passed.
- `cargo test -p tekstide-core transcript -- --quiet` passed; 16 tests passed, 0 failed.
- `cargo test -p tekstide-core -- --quiet` passed; 268 tests passed, 0 failed; doc tests had 0 tests.
- `cargo check --workspace` passed.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` passed.
- `git diff --check` passed.

Acceptance:

- `.git-exclude/reviewed/tekstide-review-request-077-rfc011-pr011a-transcript-policy-path-model-implementation-response.md` accepted PR-011-A on 2026-07-21.
- Carry forward to PR-011-B: enforce the per-transcript byte limit and produce truncation metadata without content snippets.
- Carry forward to PR-011-C: apply opt-out and path/policy preflight before process start.
- Carry forward to PR-011-D: preserve tombstone references by default and decide whether local-data summaries need multi-scope budget pressure.
- Minor cleanup opportunity: rename the successful project-inside-state-root path test for clarity in a later touch.

## Known Limitations

- PR-011-A does not implement the bounded transcript writer.
- PR-011-A does not integrate transcript capture with AgentRun launch.
- PR-011-A does not implement purge operations.
- PR-011-A does not create transcript files or directories.
- PR-011-A does not claim durable audit, GUI review surfaces, generated-change review, search indexing, secure deletion, or redaction.
