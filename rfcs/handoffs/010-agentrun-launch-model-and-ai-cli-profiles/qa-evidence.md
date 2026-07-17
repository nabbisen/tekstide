# RFC-010: AgentRun Launch Model and AI CLI Profiles — QA Evidence

Status: Proposed
Date opened: 2026-07-17
Date accepted: Pending

## Scope

RFC-010 defines AgentRun launch, AI CLI profiles, launch validation, AgentRun-to-TerminalSession attachment, lifecycle mapping, and minimum active-file safety for M5.

Evidence in this file must not be used to claim transcript retention, durable audit storage, final GUI launch/review surfaces, general command approval, provider-specific cloud integration, full file watcher behavior, or multi-document conflict UI unless later reviewed implementation explicitly supports those claims.

## Design Review

Review request 064 requested changes on 2026-07-17 in `.git-exclude/reviewed/tekstide-review-request-064-rfc010-agentrun-launch-design-response.md`.

Blocking review finding:

- Restricted Mode needed explicit executable provenance and implicit AI CLI workspace-config discovery launch gates.

Focused re-review request 065 accepted the amendment with notes on 2026-07-17 in `.git-exclude/reviewed/tekstide-review-request-065-rfc010-restricted-mode-executable-provenance-rereview-response.md`.

Accepted design carry-forward requirements:

- PR-010-B starts implementation with the AI CLI profile model and launch validation.
- Restricted Mode must reject workspace-local executables, wrappers, shims, symlink targets, and project-local `PATH` resolution.
- Built-in/user-global AI CLI profiles must document implicit workspace-local config/tool/prompt/plugin/instruction discovery behavior.
- Restricted Mode must disable that discovery through reviewed flags/environment or reject launch when it cannot be disabled or bounded.
- Implementation reviews must include targeted tests for executable provenance, wrapper/shim/symlink rejection, project-local `PATH` rejection, and implicit workspace-discovery blocking.

## Implementation Evidence

Pending.

## Known Limitations

- Implementation is pending.
- No transcript retention, durable audit storage, GUI launch/review surfaces, general command approval, provider-specific cloud integration, full watcher behavior, or multi-document conflict UI is claimed by RFC-010 design acceptance.
