# RFC-004 Acceptance / QA Checklist

Source RFC: [RFC-004](../../done/004-security-baseline-and-restricted-mode.md)

## Current Status

RFC-004 is implemented for the foundation stage as the security policy/read-model baseline. Terminal launch, AgentRun launch, command approval execution, transcript byte storage, and GUI dialogs are explicitly deferred to later RFCs/slices.

Implemented evidence comes from:

- `b3d78da RFC-004: add security policy foundation`
- `3a588c5 RFC-004: expose security display summaries`

## Security Checks

| Check | Status | Evidence / note |
| --- | --- | --- |
| New/unknown projects start Restricted or equivalent. | Satisfied | `ProjectSession::new` initializes `WorkspaceTrust::Restricted`; covered by `new_project_initializes_rfc002_session_metadata_with_inert_provider_defaults` and `add_project_from_path_validates_and_restricts_before_display`. |
| Restricted Mode blocks workspace-local automation paths. | Satisfied at policy level | `RestrictedModeFeature::ALL` covers LSP startup, workspace AI profile/prompt loading, environment loading, plugin loading, task execution, Tekstide-initiated Git hooks, workspace command palette entries, and background project automation. Covered by `restricted_mode_blocks_workspace_local_automation_paths`. Runtime launch/configuration seams are not implemented yet. |
| Trust decisions are explicit and auditable. | Satisfied at domain level | `ProjectSession::grant_trust` and `ProjectSession::revoke_trust` append project-scoped audit events. Covered by `trust_decisions_are_explicit_and_audited` and `security_audit_constructors_create_project_scoped_events`. Persistence of audit storage is deferred. |
| Plain, Supervised, and Managed labels are visible where relevant. | Satisfied for policy/read-model surfaces | `AiSessionSecurityLevel` exposes policy labels and Project Board rows expose security mode. Covered by `compatibility_labels_do_not_overclaim_command_approval`, `project_rows_preserve_placeholder_field_shape_without_probing`, and `trusted_project_board_row_uses_security_policy_summary`. Future terminal/AgentRun surfaces must consume the same policy labels. |
| Managed command approval is not claimed for Supervised or Plain sessions. | Satisfied at policy level | `can_claim_managed_command_approval` returns true only for Managed; Plain/Supervised expose `Managed command approval not guaranteed`. Covered by `compatibility_labels_do_not_overclaim_command_approval`. Command approval execution is deferred. |
| Transcript persistence has bounded retention and purge behavior. | Deferred, guarded | `TranscriptPrivacyPolicy::permits_transcript_byte_persistence` blocks byte persistence until local-only storage, bounded retention, per-run opt-out, and purge support exist. Covered by `transcript_bytes_are_blocked_until_local_bounded_purgeable_policy_exists`. Actual transcript byte storage and purge backend are deferred. |
| Redaction claims exclude arbitrary terminal output and transcripts. | Satisfied at policy level | `RedactionClaimScope::StructuredMetadataOnly` is the only implemented redaction scope. Transcript policy tests verify the narrow scope. UI/docs must continue to avoid broad terminal-output or transcript redaction claims. |
| Safe close is required for running processes. | Satisfied for close assessment / shell removal | `assess_close` returns `NeedsConfirmation` for running processes and other active resources; `AppState::close_project` does not remove active projects unless safe. Covered by `active_resources_need_confirmation`, `active_project_with_resources_needs_confirmation_and_stays_open`, and `active_project_with_missing_close_provider_is_not_closed`. GUI close dialog is deferred. |

## Evidence Required

| Evidence | Status | Notes |
| --- | --- | --- |
| Restricted Mode policy tests or design evidence. | Satisfied | `restricted_mode_blocks_workspace_local_automation_paths`, `restricted_mode_summary_exposes_ui_ready_blocked_feature_labels`, and `trusted_workspace_allows_policy_checked_automation_paths`. |
| Compatibility label UI/test evidence. | Satisfied for current surfaces | `compatibility_labels_do_not_overclaim_command_approval`; Project Board and shell rendering expose security mode through `project_rows_preserve_placeholder_field_shape_without_probing`, `trusted_project_board_row_uses_security_policy_summary`, and `populated_project_board_renders_placeholder_branch_status_without_process_probe`. |
| Transcript retention and purge plan. | Satisfied as guardrail / implementation deferred | `TranscriptPrivacyPolicy` requires local, bounded, opt-out-capable, purgeable policy before byte persistence. Actual persistence and purge behavior must be implemented in a future transcript/storage slice. |
| Audit event plan for security transitions. | Satisfied at domain level | Audit classes and constructors exist for trust grant/revoke, approval request/decision, transcript purge, and safe-close decision. Covered by `security_audit_constructors_create_project_scoped_events`, `approval_audit_constructor_reflects_pending_and_decided_states`, and `approval_audit_decision_constructor_maps_approved_decisions`. |
| Security non-claims checked in UI/docs. | Partially satisfied | Policy names and tests avoid VM/container isolation, broad redaction, and universal command approval claims. Before release, user-facing docs and GUI copy must explicitly preserve these non-claims. |

## Gate Evidence

Observed after the RFC-004 policy and display-summary slices:

```text
cargo fmt --check
cargo test --all-targets
cargo clippy --all-targets --all-features -- -D warnings
```

Latest observed test result:

```text
101 passed; 0 failed
```

Targeted no-execution/probing scans over RFC-004 touched files returned no matches for process launch, command execution, network clients, transcript byte writes, or PTY APIs.

## Closeout Decision

- RFC-004 is closed as the policy/read-model baseline.
- No extra runtime-seam slice is required before closeout because the runtime surfaces do not exist yet.
- Future terminal/AgentRun launch code must call `assess_restricted_mode_feature` rather than duplicating Restricted Mode checks.
- Future transcript storage must not write bytes until `TranscriptPrivacyPolicy::permits_transcript_byte_persistence` is true.
- Future GUI/user docs must keep the RFC-004 non-claims visible: no VM/container-grade isolation, no full arbitrary shell sandboxing, no universal command approval, and no broad transcript/output redaction.
