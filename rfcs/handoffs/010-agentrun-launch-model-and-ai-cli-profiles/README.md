# RFC-010: AgentRun Launch Model and AI CLI Profiles — Developer Handoff Pack

Source RFC: [RFC-010](../../proposed/010-agentrun-launch-model-and-ai-cli-profiles.md)
Target milestone: **M5**
Source RFC status: **Proposed**

## Files

- `implementation-handoff.md` — developer-facing launch/profile/security guidance.
- `task-breakdown-pr-plan.md` — recommended implementation slices and review gates.
- `acceptance-qa-checklist.md` — acceptance traceability, QA checklist, and evidence requirements.
- `qa-evidence.md` — placeholder for observed implementation gates, security notes, and known limitations.

This handoff inherits the source RFC lifecycle state. RFC-010 is proposed and must be reviewed before implementation starts.

Review disposition:

- RFC-010 design review requested changes on 2026-07-17. See `.git-exclude/reviewed/tekstide-review-request-064-rfc010-agentrun-launch-design-response.md`.
- Focused re-review accepted the Restricted Mode executable-provenance amendment on 2026-07-17. See `.git-exclude/reviewed/tekstide-review-request-065-rfc010-restricted-mode-executable-provenance-rereview-response.md`.
- RFC-010 is accepted for implementation to begin with PR-010-B: AI CLI profile model and launch validation.

## Source Summary

RFC-010 defines AgentRun launch on top of the RFC-008 terminal/process foundation and RFC-009 terminal security boundary. It introduces AI CLI profiles as reviewed launch contracts, validates project/trust/cwd/environment/transcript policy before process start, attaches launched AgentRuns to project-owned TerminalSessions, and requires minimum active-document safety while AgentRuns are active.

RFC-010 does not implement transcript retention, durable audit persistence, final GUI launch/review surfaces, general command approval, or provider-specific cloud integration.
