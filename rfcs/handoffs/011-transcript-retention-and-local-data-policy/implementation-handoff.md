---
title: "RFC-011: Transcript Retention and Local Data Policy - Implementation Handoff"
rfc: "RFC-011"
rfc_file: "../../done/011-transcript-retention-and-local-data-policy.md"
status: "Implemented with documented limitations"
target_milestone: "M6"
created: "2026-07-21"
---

# RFC-011: Transcript Retention and Local Data Policy - Implementation Handoff

## Scope

Implement bounded local transcript retention for Tekstide-created AgentRuns according to the accepted RFC-011 policy and reviewed implementation slices.

The implementation must preserve RFC-010 launch validation and lifecycle truth. Transcript capture is storage behavior, not process supervision.

## Existing Anchors

- `crates/tekstide-core/src/domain/transcript.rs` already provides `Transcript` metadata and `TruncationState`.
- `crates/tekstide-core/src/security.rs` already provides `TranscriptPrivacyPolicy`, `BoundedTranscriptRetention`, `TranscriptStoragePolicy`, and related privacy vocabulary.
- `crates/tekstide-core/src/project/session.rs` already owns `ProjectSession::add_transcript` and reference validation.
- `crates/tekstide-core/src/agent/launch.rs` is the RFC-010 launch validation boundary that should receive transcript capture preferences.
- RFC-009 terminal output security rules continue to apply to retained bytes.

## Hard Requirements

- No transcript byte persistence unless storage is local-only, bounded, purgeable, and opt-out capable.
- No default-on transcript byte persistence without per-transcript, per-project, and app-wide aggregate retention accounting.
- No transcript paths inside a ProjectSession root.
- No workspace-local symlink or traversal path may redirect transcript storage into a project root.
- No raw transcript bytes, prompt text, environment values, terminal output, or file contents in summaries or errors.
- No plain terminal transcript retention by default.
- No search indexing by default.
- No redaction guarantee. Structured metadata only may be described as redacted/bounded.
- No durable audit persistence in RFC-011; RFC-012 owns durable audit.

## Model Guidance

Prefer extending existing transcript/security vocabulary before adding parallel concepts.

Expected additions:

- `TranscriptCaptureMode`: disabled, local bounded, required local bounded.
- retention state that can represent active, truncated, expired, disabled, failed, and purged outcomes.
- a path resolver that returns a reviewed local transcript reference.
- a bounded writer summary with byte count, truncation state, and bounded error category.
- local-data summary types that expose retained transcript byte totals without content.
- purge result types for transcript, AgentRun, and ProjectSession scopes.

Use domain-specific modules if files become large. Do not force a split before the boundaries are visible.

## Path Policy

Transcript storage should live under a Tekstide-managed state root, for example:

```text
<tekstide-state-root>/transcripts/<project-id>/<agent-run-id>/transcript.log
```

The resolver must prove the transcript path is under the state root and outside the project root. Treat this as a security boundary, not formatting.

## Runtime / Writer Guidance

The bounded writer should be append-only from the AgentRun terminal output stream.

It should:

- stop or truncate at the byte limit;
- update transcript metadata without storing content in summaries;
- preserve untrusted terminal bytes without interpreting them as UI;
- finalize metadata on terminal exit/failure/cancellation/detach;
- avoid background cleanup unless the behavior is explicit and testable.

## Purge Guidance

Purge should be idempotent and local. The default policy is to preserve content-free tombstone transcript metadata and keep AgentRun/TerminalSession transcript references pointing at that tombstone.

Required scopes:

- transcript id;
- AgentRun id;
- ProjectSession id.

Purge must never delete project files. References may be cleared only if the implementation cannot preserve a content-free tombstone without retaining sensitive path, content, or environment metadata.

## Review Notes

Every implementation review request for RFC-011 must reference:

- `rfcs/done/011-transcript-retention-and-local-data-policy.md`
- this handoff pack;
- relevant prior RFCs: RFC-009 and RFC-010 at minimum.

Implementation review request filenames should include `implementation`.
