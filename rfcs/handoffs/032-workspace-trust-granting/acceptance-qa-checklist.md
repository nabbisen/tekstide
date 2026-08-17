---
title: "RFC-032: Workspace Trust Granting - Acceptance / QA Checklist"
rfc: "RFC-032"
rfc_file: "../../proposed/032-workspace-trust-granting.md"
status: "Open"
target_milestone: "M11"
created: "2026-08-17"
---

# Acceptance / QA Checklist

Every unchecked line at closeout carries a stated reason.

## Persistence and binding (PR-032-B)

- [ ] Trust recorded against the canonical path.
- [ ] A redirected **real symlink** leaves the project untrusted on reopen.
- [ ] Positive control: an unchanged project is still trusted on reopen.
- [ ] Revocation clears persisted state, proven across a reopen.
- [ ] Existing `root_path`/`canonical_root_path` reused; no third notion of location.
- [ ] Ablated: literal-path binding shows a redirected symlink inheriting trust.

## Grant, revoke, route (PR-032-C)

- [ ] `grant_project_trust` and `revoke_trust` each have their first production caller.
- [ ] Both call sites enumerated; a second fails by name.
- [ ] Audit records queried and asserted, not implied.
- [ ] Route uses the dormant `TrustSettings` variant and the shared predicate.
- [ ] Granting and revoking comparably reachable; action counts stated.
- [ ] Board reflects trust state.

## The dialog (PR-032-D)

- [ ] Path escaped at the widget; bidi-override claim tested and ablated.
- [ ] No double-escaping.
- [ ] Canonical path shown; both shown when they differ.
- [ ] Focus defaults to not-granting; granting needs two deliberate acts.
- [ ] The canonical sentence used; "present and future" stated.
- [ ] Nine features not enumerated.
- [ ] None of the three forbidden claims present.
- [ ] Modal exclusivity holds.

## Honesty (PR-032-E)

- [ ] Claim statement checked against the RFC **and** the decisions page.
- [ ] No claim that trust makes a project safe.
- [ ] What this unblocks stated precisely.
- [ ] The three requirements confirmed shipped, not intended.
- [ ] `future-work.md` and the audit row updated in the same commit.
- [ ] Every pack document's front matter updated.
- [ ] Every unchecked line above carries a stated reason.

## Final Acceptance Decision

- [ ] Accepted
- [ ] Accepted with required follow-up
- [ ] Rejected

Reviewer notes:
