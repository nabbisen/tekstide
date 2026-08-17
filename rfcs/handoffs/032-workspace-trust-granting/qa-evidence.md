---
title: "RFC-032: Workspace Trust Granting - QA Evidence"
rfc: "RFC-032"
rfc_file: "../../proposed/032-workspace-trust-granting.md"
status: "In progress -- PR-032-B done, PR-032-C/D/E remaining"
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

**Core only, no GUI. `crates/tekstide-core`.**

### What changed

- `WorkspaceTrust` (`project/metadata.rs`) gained `Serialize`/`Deserialize`/`Default`
  (`#[default]` on `Restricted`, the fail-closed choice) so it can be persisted directly.
- `RecentProject.last_trust_state_summary: String` (a write-only display label nothing ever
  read back) became `RecentProject.trust_state: WorkspaceTrust` (`project/recent/state.rs`), a
  real typed value. `#[serde(default)]`: a pre-RFC-032 on-disk record lacks the field entirely
  and defaults to `Restricted` -- correct, not just lenient, since nothing could have been
  trusted before `grant_project_trust` had a production caller.
- `ProjectSession::restore_trust_state` (`project/session.rs`, `pub(crate)`): sets `trust_state`
  directly with **no** `AuditEvent` push, unlike `grant_trust`/`revoke_trust` -- restoring a
  past decision on reopen is not a new one, and the original grant already has its own
  `TrustGrant` authorization record from when it actually happened.
- `AppState::add_project_session` (`app.rs`): after constructing a fresh `ProjectSession`,
  looks up `recent_trust_by_canonical_root` (the new helper, same key comparison
  `recent_project_id_by_canonical_root` already uses to reuse `ProjectId`) and calls
  `restore_trust_state` if found. **No third notion of project location** -- the existing
  `root_path`/`canonical_root_path` pair is the only key used, for both the id lookup and the
  new trust lookup.

### Review gate, evidence (`crates/tekstide-core/src/app/tests.rs`)

All five real, against `AppState`/`ProjectSession` directly -- no mocked lookup, no synthesized
path string standing in for a real one:

1. **Positive control** (`an_unredirected_symlinked_project_is_still_trusted_on_reopen`): a real
   symlink, unredirected, still trusted on reopen. Required *before* the negative case means
   anything -- without it, "not trusted after redirect" would pass equally well if nothing were
   ever trusted.
2. **The falsifiable claim** (`a_redirected_symlink_is_not_trusted_on_reopen`): a real symlink
   (`std::os::unix::fs::symlink`), redirected for real (removed and recreated pointing at a
   different real directory) between a grant and a reopen, is **not** trusted on reopen --
   `Restricted`, not `Trusted`.
3. **Ablated** (`ablation_binding_trust_to_the_literal_path_would_inherit_a_redirected_symlinks_trust`):
   the same real redirected fixture, looked up by `root_path` (the literal path -- the symlink
   itself, unchanged by the redirect) instead of `canonical_root_path`, **does** find the old
   `Trusted` grant -- the specific divergence canonical-path binding exists to prevent, shown
   rather than assumed.
4. **Revocation persists across a reopen** (`revoking_trust_persists_and_survives_a_reopen`):
   grant, snapshot (`Trusted`), revoke, snapshot again (not `Trusted` -- the in-memory half),
   then a genuinely fresh `AppState` restored from that second snapshot and reopened (not
   `Trusted` -- the half that actually crosses a session boundary).
5. **The existing-mechanism regression test flipped**
   (`restored_recent_project_id_is_reused_when_project_is_added_again`): this test predates
   PR-032-B and asserted the *old*, soon-to-be-wrong behaviour -- "display-only trust summary
   must not restore trust." Now asserts the opposite, correct behaviour, with the reason stated
   in the assertion message rather than silently inverted.

`cargo test -p tekstide-core --lib app::tests::` -- 26/26 passing.

### Not built here, and why

- **The dialog, the route, the board** -- PR-032-C/D. This slice is data-model only.
- **`recent_project_row`'s own `trust_label` hardcodes `"Restricted"`** regardless of the
  persisted value (`project_board.rs`) -- a real, separate gap this slice's tests deliberately
  do not paper over (see the two `project_board::tests` fixtures using
  `WorkspaceTrust::Trusted` while still asserting `trust_label == "Restricted"`, unchanged).
  `task-breakdown-pr-plan.md`'s PR-032-C item "the board reflects trust state" owns this.

### Gates run

`cargo fmt --all --check` clean. `cargo clippy --workspace --all-targets --all-features -- -D
warnings` clean. `cargo test --workspace --all-targets --all-features`: 851 passed, 0 failed,
run three times for stability. `git diff --check` clean.

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
