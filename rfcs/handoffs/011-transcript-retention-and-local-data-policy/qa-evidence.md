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

Pending.

## Known Limitations

Pending design review.
