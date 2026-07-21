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

Pending implementation.

## Known Limitations

- Pending implementation.
