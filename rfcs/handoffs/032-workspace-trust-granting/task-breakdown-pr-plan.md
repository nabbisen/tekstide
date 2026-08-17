---
title: "RFC-032: Workspace Trust Granting — Task Breakdown / PR Plan"
rfc: "RFC-032"
rfc_file: "../../proposed/032-workspace-trust-granting.md"
status: "Ready for implementation"
target_milestone: "M11"
created: "2026-08-17"
---

# Task Breakdown

Five slices. **[`what-the-trust-dialog-must-say.md`](./what-the-trust-dialog-must-say.md) is
required reading before any of them.**

## PR-032-A — Design and handoff acceptance

Granted 2026-08-17 with the pack. Both of RFC-032's open questions are answered in
`docs/src/contributors/security-decisions.md`; raise a disagreement with evidence rather than
implementing around one.

## PR-032-B — Persistence, bound to the canonical path

Core only. No GUI.

Review gate:

- **Trust is recorded against the canonical path**, and a project reopened at a path whose
  canonical resolution differs is **not** trusted. Proven against a **real symlink** redirected
  between sessions, not a synthesised path string.
- **A legitimately unchanged project is still trusted on reopen** — the positive control. Without
  it, a test proving "not trusted after redirect" would also pass if nothing were ever trusted.
- **Revocation clears the persisted state**, not only the in-memory one. Prove it survives a
  reopen.
- **`RecentProject`'s existing `root_path`/`canonical_root_path` pair is reused**, not joined by
  a third notion of project location.
- Ablate: record against the literal path instead, show a redirected symlink inheriting trust.

## PR-032-C — Grant and revoke, with a route

Review gate:

- **`AuditCoordinator::grant_project_trust` gains its first production caller**, and
  `revoke_trust` likewise. Enumerate both call sites so a second fails by name.
- **The audit records are asserted, not implied** — query the store and check `TrustGrant`
  authorization *and* application, the way RFC-022's `command_approval` assertion did after
  finding two records rather than one.
- **The route uses `ProjectOpenSurface::TrustSettings`**, already declared and dormant. This is
  the second real `open_surface`-conditional dispatch after RFC-022's `ApprovalHistory` — reuse
  the shared `surface_renders_editor` predicate rather than adding a parallel match.
- **Granting and revoking are comparably reachable.** State the action count for each; if
  revoking is materially harder, that is a finding.
- **The board reflects trust state**, via the existing `ProjectBoardRow::trust_label`.

## PR-032-D — The dialog

The security surface. Do not fold it into C.

Review gate:

- **The path is escaped at the widget.** Falsifiable claim: a project directory whose name
  contains a bidi override renders it visibly as an escape marker. Ablated.
- **No double-escaping**, shown against literal marker-shaped text.
- **The canonical path is what is shown**, and both are shown when they differ.
- **Focus defaults to the non-granting action**; granting needs focus movement *and*
  activation. Prove both halves.
- **The canonical sentence is used** — the one in the decisions page — and the "present and
  future" consequence is stated.
- **The nine features are not enumerated** in the dialog.
- **None of the three forbidden claims appears**: that trusting is safe, that Tekstide polices
  what runs, or that revoking undoes what already ran.
- Modal exclusivity per RFC-018.

## PR-032-E — Closeout

Review gate:

- Claim statement checked **against RFC-032's own text and the decisions page**, not only the
  evidence file.
- **No claim that trust makes a project safe.**
- **What this unblocks, precisely**: agent runs become launchable in a trusted project; that is
  not the same as RFC-020's surfaces being done.
- The three requirements confirmed as shipped, not intended.
- `rfcs/future-work.md`'s trust entry and the reachability audit's row updated in the same
  commit.
- **Every document in this pack's own front matter updated** — RFC-022's closeout found four of
  five stale, including the evidence file containing the closeout.

## Sequencing

```
A ─→ B ─→ C ─→ D ─→ E
```

**B first** because persistence and binding determine the data model; a dialog built before
them would be written against a shape that then changes. **D last** because the security
surface should be built on a path already proven.
