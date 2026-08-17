---
title: "RFC-032: Workspace Trust Granting - QA Evidence"
rfc: "RFC-032"
rfc_file: "../../proposed/032-workspace-trust-granting.md"
status: "Open - no slices implemented yet"
target_milestone: "M11"
created: "2026-08-17"
---

# QA Evidence

**This file holds results. The obligations live in
[`task-breakdown-pr-plan.md`](./task-breakdown-pr-plan.md).**

## Conventions

- **Ablation**: break the property, watch the *specific* named test fail, restore. A green
  ablation is a defect in the ablation.
- **Positive control**: prove the check reaches real data before asserting what it does not
  find. The trust tests need this more than most — "not trusted" passes trivially if nothing
  is ever trusted.
- **Real conditions**: a real symlink, redirected for real. Not a synthesised path string.
- State what each piece of evidence does **not** prove.

## Starting state, recorded before any change

- `AuditCoordinator::grant_project_trust` and `ProjectSession::revoke_trust`: correct,
  audited, **zero production callers**.
- Every project is `Restricted` from `ProjectSession::new` and stays there.
- `ProjectOpenSurface::TrustSettings` declared and dormant.
- `Ctrl+Alt+A` refuses for every real user with `WorkspaceDiscoveryBlocked`.

## PR-032-A - Design and handoff acceptance

Granted 2026-08-17. Both open questions answered by the owner; decisions and reasoning in
`docs/src/contributors/security-decisions.md`, which is canonical.

## PR-032-B - Persistence and binding

*Not started.*

## PR-032-C - Grant, revoke, route

*Not started.*

## PR-032-D - The dialog

*Not started.*

## PR-032-E - Closeout

*Not started.*

## Known limitations going in

- **Trust cannot be withdrawn from what already ran.** Revocation stops future loading only.
- **Canonical resolution is checked at open time** and cannot close the gap between check and
  use. Inherent to filesystems.
- **Trust expiry** (RFC-004's own open question) is not addressed.
