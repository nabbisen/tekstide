---
title: "RFC-032: Workspace Trust Granting - Acceptance / QA Checklist"
rfc: "RFC-032"
rfc_file: "../../done/032-workspace-trust-granting.md"
status: "Substantive items ticked by the implementer 2026-08-17; Final Acceptance Decision reserved for the reviewer"
target_milestone: "M11"
created: "2026-08-17"
---

# Acceptance / QA Checklist

Every unchecked line at closeout carries a stated reason.

## Persistence and binding (PR-032-B)

- [x] Trust recorded against the canonical path.
- [x] A redirected **real symlink** leaves the project untrusted on reopen.
- [x] Positive control: an unchanged project is still trusted on reopen.
- [x] Revocation clears persisted state, proven across a reopen.
- [x] Existing `root_path`/`canonical_root_path` reused; no third notion of location.
- [x] Ablated: literal-path binding shows a redirected symlink inheriting trust.

## Grant, revoke, route (PR-032-C)

- [x] `grant_project_trust` and `revoke_trust` each have their first production caller.
      (`AuditCoordinator::grant_project_trust`/`revoke_project_trust`, `shell.rs`; the
      checklist's `revoke_trust` is the `pub(crate)` `ProjectSession` method the coordinator
      calls beneath its own `revoke_project_trust`.)
- [x] Both call sites enumerated; a second fails by name.
- [x] Audit records queried and asserted, not implied.
- [x] Route uses the dormant `TrustSettings` variant and the shared predicate.
- [x] Granting and revoking comparably reachable; action counts stated.
- [x] Board reflects trust state.

## The dialog (PR-032-D)

- [x] Path escaped at the widget; bidi-override claim tested and ablated.
- [x] No double-escaping.
- [x] Canonical path shown; both shown when they differ.
- [x] Focus defaults to not-granting; granting needs two deliberate acts.
- [x] The canonical sentence used; "present and future" stated.
- [x] Nine features not enumerated.
- [x] None of the three forbidden claims present.
- [x] Modal exclusivity holds.

## Honesty (PR-032-E)

- [x] Claim statement checked against the RFC **and** the decisions page.
- [x] No claim that trust makes a project safe.
- [x] What this unblocks stated precisely.
- [x] The three requirements confirmed shipped, not intended.
- [x] `future-work.md` and the audit row updated in the same commit.
- [x] Every pack document's front matter updated.
- [x] Every unchecked line above carries a stated reason.

## Final Acceptance Decision

- [ ] Accepted
- [ ] Accepted with required follow-up
- [ ] Rejected

Reviewer notes:
