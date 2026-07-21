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

### PR-011-B - Bounded Transcript Writer

Status: accepted with notes.

Implementation:

- Added `crates/tekstide-core/src/transcript/writer.rs`.
- Exported bounded writer types from `crates/tekstide-core/src/transcript.rs`.
- Added writer tests to `crates/tekstide-core/src/transcript/tests.rs`.
- Renamed the successful project-inside-state-root path test for clarity while touching transcript tests.
- Added bounded transcript writer vocabulary:
  - `TranscriptWriterConfig`
  - `BoundedTranscriptWriter`
  - `TranscriptWriteSummary`
  - `TranscriptWriteError`
  - `TranscriptWriteErrorReason`

Implemented writer behavior:

- creates the resolved transcript directory;
- creates/truncates the resolved transcript file;
- appends terminal-output bytes provided by the caller;
- enforces the per-transcript byte limit from `TranscriptRetentionLimits`;
- writes only bytes that fit inside the remaining per-transcript budget;
- records `TranscriptRetentionState::Truncated { scope: TranscriptBudgetScope::Transcript }` when input exceeds the per-transcript budget;
- preserves byte count after additional appends once truncated;
- reports empty appends as no-op summaries;
- rejects unbounded retention before creating transcript directories or files;
- returns bounded open/write/flush errors with path, reason, and byte count only.

Review follow-up:

- `.git-exclude/reviewed/tekstide-review-request-078-rfc011-pr011b-bounded-transcript-writer-implementation-response.md` requested changes because public `TranscriptStoragePath` fields allowed callers to forge project-root transcript paths before writer filesystem side effects.
- Made `TranscriptStoragePath` fields private.
- Added read-only accessors for state root, project root, transcript directory, and transcript file.
- Added writer-side storage-path validation before `create_dir_all` or `open`.
- Added `TranscriptWriteErrorReason::InvalidStoragePath`.
- Added a forged project-root storage path regression proving the writer rejects before creating a project-root directory or transcript file.

Security/privacy notes:

- PR-011-B writes only caller-provided bytes through a local writer harness. It does not read terminal streams itself.
- Writer summaries and errors do not contain transcript snippets, prompt text, environment values, terminal output, or file contents.
- PR-011-B does not integrate with AgentRun launch, plain terminal capture, lifecycle state, purge, local-data aggregate cleanup, durable audit, GUI review surfaces, generated-change review, search indexing, secure deletion, or redaction.
- Path safety remains delegated to PR-011-A `TranscriptPathResolver`; the writer accepts a resolved `TranscriptStoragePath`.

Observed gates on 2026-07-21 before review request 078:

- `cargo fmt --all --check` passed.
- `cargo test -p tekstide-core transcript -- --quiet` passed; 22 tests passed, 0 failed.
- `cargo test -p tekstide-core -- --quiet` passed; 274 tests passed, 0 failed; doc tests had 0 tests.
- `cargo check --workspace` passed.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` passed.
- `git diff --check` passed.

Observed gates on 2026-07-21 after review request 078 fixes:

- `cargo fmt --all --check` passed.
- `cargo test -p tekstide-core transcript -- --quiet` passed; 23 tests passed, 0 failed.
- `cargo test -p tekstide-core -- --quiet` passed; 275 tests passed, 0 failed; doc tests had 0 tests.
- `cargo check --workspace` passed.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` passed.
- `git diff --check` passed.

Acceptance:

- `.git-exclude/reviewed/tekstide-review-request-079-rfc011-pr011b-bounded-transcript-writer-implementation-rereview-response.md` accepted PR-011-B on 2026-07-21.
- Carry forward to PR-011-C: apply opt-out, policy validation, and path preflight before process start.
- Carry forward to PR-011-C: ensure only resolved `TranscriptStoragePath` values reach the writer.
- Carry forward to PR-011-D: preserve tombstone references by default and keep purge/local-data summaries content-free.

### PR-011-C - AgentRun Launch Integration

Status: accepted with notes.

Implementation:

- Updated `crates/tekstide-core/src/agent/launch.rs`.
- Updated `crates/tekstide-core/src/agent.rs`.
- Updated `crates/tekstide-core/src/project/session.rs`.
- Updated `crates/tekstide-core/src/runtime/terminal/types.rs`.
- Updated `crates/tekstide-core/src/runtime/terminal/launch.rs`.
- Added AgentRun launch transcript capture request fields and helpers:
  - `transcript_capture_mode`
  - `transcript_state_root`
  - `transcript_retention_limits`
  - `without_transcript_capture`
  - `with_local_bounded_transcript`
  - `with_required_local_bounded_transcript`
  - `with_transcript_retention_limits`
- Added `AgentRunTranscriptCapture`.
- Added `AgentRunTranscriptCaptureError`.
- Added runtime `TerminalLaunchSpec` transcript writer configuration.
- Added runtime-owned `BoundedTranscriptWriter` support for AgentRun terminal output reads.

Implemented launch/capture behavior:

- `LocalBounded` is the default AgentRun request capture mode.
- `LocalBounded` without a state root or with failed path preflight disables capture and still allows launch.
- `RequiredLocalBounded` rejects validation when state root is missing.
- `RequiredLocalBounded` rejects validation when retention policy is unbounded.
- `RequiredLocalBounded` rejects before runtime launch when path preflight fails.
- Transcript path preflight happens before process start in `ProjectSession::launch_agent_run_with_runtime`.
- Only resolver-produced `TranscriptStoragePath` values reach `TerminalLaunchSpec` writer configuration.
- Runtime creates the transcript writer before spawning the process.
- Runtime appends all PTY bytes read by `read_available_bounded_for` into the bounded transcript writer before applying the caller-visible UI buffer cap.
- Successful transcript-enabled AgentRun launch attaches transcript metadata to the AgentRun and TerminalSession.
- Per-run opt-out launches without transcript metadata or bytes.

Review follow-up:

- `.git-exclude/reviewed/tekstide-review-request-080-rfc011-pr011c-agentrun-transcript-launch-integration-implementation-response.md` requested changes because transcript capture wrote only the UI-buffered output subset and silently omitted PTY bytes dropped by `max_buffered_bytes`.
- Changed `read_available_bounded_for` to append each raw PTY read chunk to the transcript writer before applying the caller-visible output buffer cap.
- Kept caller-visible `TerminalOutputSummary` bounded by `max_buffered_bytes`.
- Added a transcript-enabled AgentRun regression where `TerminalOutputSummary` reports dropped bytes while the transcript file still contains bytes beyond the returned UI buffer.

Security/privacy notes:

- Transcript capture remains limited to Tekstide-created AgentRuns; plain terminal launch specs still default to no transcript writer.
- Transcript errors and summaries remain bounded and content-free.
- Runtime transcript writer state does not become process truth; TerminalSession/runtime lifecycle remains authoritative.
- PR-011-C does not implement purge operations, tombstone updates, aggregate cleanup, GUI transcript/review surfaces, generated-change review, search indexing, secure deletion, or redaction.
- PR-011-C captures output only when callers read terminal output through the existing bounded runtime read API. It does not add a hidden background capture loop.

Observed gates on 2026-07-21 before review request 080:

- `cargo fmt --all --check` passed.
- `cargo test -p tekstide-core agent -- --quiet` passed; 55 tests passed, 0 failed.
- `cargo test -p tekstide-core transcript -- --quiet` passed; 28 tests passed, 0 failed.
- `cargo test -p tekstide-core -- --quiet` passed; 280 tests passed, 0 failed; doc tests had 0 tests.
- `cargo check --workspace` passed.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` passed.
- `git diff --check` passed.

Observed gates on 2026-07-21 after review request 080 fixes:

- `cargo fmt --all --check` passed.
- `cargo test -p tekstide-core agent -- --quiet` passed; 56 tests passed, 0 failed.
- `cargo test -p tekstide-core transcript -- --quiet` passed; 29 tests passed, 0 failed.
- `cargo test -p tekstide-core -- --quiet` passed; 281 tests passed, 0 failed; doc tests had 0 tests.
- `cargo check --workspace` passed.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` passed.
- `git diff --check` passed.

Acceptance:

- `.git-exclude/reviewed/tekstide-review-request-081-rfc011-pr011c-agentrun-transcript-launch-integration-implementation-rereview-response.md` accepted PR-011-C on 2026-07-21.
- Carry forward to PR-011-D: preserve tombstone references by default.
- Carry forward to PR-011-D: keep purge/local-data summaries content-free.
- Carry forward to PR-011-D or closeout: reconcile stored transcript metadata with writer byte count, truncation state, last-write timestamp, and finalization behavior.

## Known Limitations

- PR-011-C does not implement purge operations or tombstone updates.
- PR-011-C does not implement aggregate cleanup for project/app budgets.
- PR-011-C does not add a hidden background terminal-output capture loop.
- PR-011-C does not claim durable audit, GUI review surfaces, generated-change review, search indexing, secure deletion, or redaction.
