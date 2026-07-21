# RFC-011: Transcript Retention and Local Data Policy - Developer Handoff Pack

Source RFC: [RFC-011](../../done/011-transcript-retention-and-local-data-policy.md)
Target milestone: **M6**
Source RFC status: **Implemented with documented limitations**

## Files

- `implementation-handoff.md` - developer-facing transcript retention, path, privacy, and purge guidance.
- `task-breakdown-pr-plan.md` - recommended implementation slices and review gates.
- `acceptance-qa-checklist.md` - acceptance traceability, QA checklist, and evidence requirements.
- `qa-evidence.md` - placeholder for observed implementation gates, security notes, and known limitations.

This handoff inherits the source RFC lifecycle state. RFC-011 implementation is accepted with documented limitations.

## Source Summary

RFC-011 defines bounded local transcript capture for Tekstide-created AgentRuns. It turns RFC-010's metadata-only transcript boundary into a local-only, bounded, purgeable, opt-out-capable data policy with path containment outside project roots.

RFC-011 does not implement durable audit storage, final GUI transcript/review panes, generated-change review UI, command approval, provider cloud integration, or redaction guarantees.
