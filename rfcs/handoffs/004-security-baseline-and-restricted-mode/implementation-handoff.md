# RFC-004 Implementation Handoff

Source RFC: [RFC-004](../../proposed/004-security-baseline-and-restricted-mode.md)
Target: security baseline and Restricted Mode

## Purpose

Set non-negotiable security boundaries before terminal, AgentRun, workspace trust, transcript, or plugin-related implementation begins.

## Decisions To Preserve

- Unknown projects are treated as Restricted.
- Restricted Mode blocks workspace-local automation, env loading, profiles, plugins, LSP startup, tasks, hooks, and workspace-sourced command palette entries.
- User-facing AI labels are `Plain`, `Supervised`, and `Managed`.
- Command approval is guaranteed only for Managed adapter-supported actions.
- Supervised and Plain sessions must not be shown as command-approved.
- Transcript capture is local, bounded, purgeable, visible, and default-on for Tekstide-created AgentRuns with per-run opt-out.
- Redaction claims are narrow: structured metadata/audit summaries only.
- MVP does not provide VM/container-grade isolation, full shell sandboxing, or arbitrary output/transcript redaction.

## Implementation Guidance

- Implement Restricted Mode as policy functions, not scattered UI conditionals.
- Make compatibility labels available to every terminal/AgentRun surface.
- Add audit hooks for trust, launch, approval, transcript purge, and safe-close decisions, even if storage is initially a no-op seam.
- Do not persist transcripts until bounded retention and purge behavior are represented.
- Treat terminal output, Git display data, LSP diagnostics, and AI output as untrusted display content.

## Stop Conditions

Pause for RFC amendment if implementation needs automatic workspace execution in Restricted Mode, unbounded transcript retention, unsupported command-approval claims, broad secret redaction claims, or OS sandboxing claims.
