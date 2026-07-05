# RFC-004 Acceptance / QA Checklist

Source RFC: [RFC-004](../../proposed/004-security-baseline-and-restricted-mode.md)

## Security Checks

- [ ] New/unknown projects start Restricted or equivalent.
- [ ] Restricted Mode blocks workspace-local automation paths.
- [ ] Trust decisions are explicit and auditable.
- [ ] Plain, Supervised, and Managed labels are visible where relevant.
- [ ] Managed command approval is not claimed for Supervised or Plain sessions.
- [ ] Transcript persistence has bounded retention and purge behavior.
- [ ] Redaction claims exclude arbitrary terminal output and transcripts.
- [ ] Safe close is required for running processes.

## Evidence Required

- [ ] Restricted Mode policy tests or design evidence.
- [ ] Compatibility label UI/test evidence.
- [ ] Transcript retention and purge plan.
- [ ] Audit event plan for security transitions.
- [ ] Security non-claims checked in UI/docs.
