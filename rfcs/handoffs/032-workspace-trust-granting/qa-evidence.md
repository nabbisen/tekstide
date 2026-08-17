---
title: "RFC-032: Workspace Trust Granting - QA Evidence"
rfc: "RFC-032"
rfc_file: "../../done/032-workspace-trust-granting.md"
status: "Closed 2026-08-17 -- all five PRs (A-E) implemented and evidenced"
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

**Done, in two increments: the audit-authority fix (responses 245/246), then grant/revoke's
production callers, the route, and the board (response 247's remaining scope, below).**

### The audit-authority fix (response 245)

Response 245 accepted PR-032-B and found a real gap: `recent-projects.json` is ordinary
user-writable state, so anything that can write it could mark a project `Trusted` with no
corresponding `TrustGrant` in the durable audit store -- an auditor reading the durable record
would see a project that was never granted trust, operating as trusted. The response's stated
preference: make the audit store authoritative, restoring trust only when a matching applied
`TrustGrant` genuinely exists, failing closed otherwise.

**What changed:**

- `AuditStore::has_applied_trust_grant(&self, project_id)` (`audit/store.rs`): queries the
  newest **`Applied`** `TrustChange` record for a project and confirms its `action_kind` is
  `TrustGrant` -- a later `TrustRevoke` correctly supersedes an earlier grant, since both share
  the one query ordered by sequence. **Corrected in response 246**: the first version queried
  the newest row of *any* outcome, which meant a later re-grant attempt interrupted after its
  `Authorized` write (a retry, a crash) but before its `Applied` one would silently demote a
  project that was still legitimately trusted from an earlier, completed grant. Filtering to
  `Applied` rows asks "what was the last *completed* decision," not "what was the last row
  written" -- dangling authorizations are ignored rather than treated as decisions.
- `ProjectSession::deny_unverified_trust` (`project/session.rs`, `pub`): demotes back to
  `Restricted`, pushing no `AuditEvent` -- the same reasoning as `restore_trust_state`, this is
  not a new decision, it is declining to honour one the durable record does not confirm.
- `verify_restored_trust`/`verify_restored_trust_against` (`crates/tekstide/src/shell.rs`):
  called once, inside `State::new`, for every boot-time project. Confirms each currently-`Trusted`
  project against the real store and demotes the ones it cannot confirm. **Deliberately not
  folded into `AppState::add_project_session`** (`tekstide-core`) -- that function stays
  synchronous and I/O-free, exercised by dozens of tests against synthetic paths a real
  `AuditStore::open` would fail against; verification instead happens once, at the one real
  boundary that already opens the audit store for every other trust-related operation.
- **Opens the audit store only when something is cached `Trusted`** -- preserves "ordinary use
  does not create this file" (README, `open_real_audit_store`'s own doc): a project can only be
  cached `Trusted` if `grant_project_trust` ran for it at some point, and that call already
  created the store itself, so this is never a *new* reason to create it.

- `only_one_production_call_site_ever_restores_a_projects_trust_state`
  (`app/tests.rs`): response 246's second requirement -- restoration and verification are two
  separate steps (`restore_trust_state` then, later, `verify_restored_trust`), safe today only
  because `AppState::add_project_session` is the one and only caller of the former. Pins that
  count by source-scan enumeration (the same shape
  `only_two_named_production_call_sites_ever_append_to_a_transcript_writer` already uses), so a
  second call site fails this test **by name** rather than silently restoring an unverified
  decision. Ablated manually: a throwaway second call site added, confirmed this test failed
  naming both files, removed.

**Evidence:**

- `crates/tekstide-core/src/audit/tests/store/trust.rs` (6 tests): `has_applied_trust_grant` is
  true after a real two-phase grant, false with no records, false after a later revoke
  supersedes an earlier grant, false for an authorization with no matching applied record (a
  grant that never completed), **true across a later interrupted re-grant** (response 246's own
  regression test -- a completed grant followed by a dangling authorization from an interrupted
  retry must not silently undo it), and correctly project-scoped (no cross-project leak).
- `crates/tekstide/src/shell/tests.rs` (4 tests, real temp-dir-backed `AuditStore`, not a mock):
  a real recorded grant keeps the cache-restored trust standing; a cache says `Trusted` with no
  matching record in the store and is demoted (**the fix's own regression test**); an unopenable
  store demotes every currently-`Trusted` project (fail-closed); and nothing cached as `Trusted`
  never even opens the store (proven with a closure that panics if called, not just by checking
  the end state).

`cargo test -p tekstide-core --lib audit::tests::store::trust::` -- 6/6.
`cargo test -p tekstide-core --lib app::tests::only_one_production_call_site_ever_restores_a_projects_trust_state`
-- 1/1.
`cargo test -p tekstide --bin tekstide shell::tests::verify_restored_trust` -- 4/4.

### Grant, revoke, the route, and the board (response 247's remaining scope)

Built together with PR-032-D's dialog (below) rather than as two separate commits -- response
247 required proving the full chain end to end "in this slice," which needs a real, working
confirmation dialog to exist, not only the mechanical plumbing around it.

**Route**: `NavigationAction::OpenTrustSettings` (new, `Configurable`/`None` binding, the same
shape every other navigation action without a default binding uses today) -> `AppCommand::OpenActiveProjectSurface(ProjectOpenSurface::TrustSettings)`
-> `content_mode_view` now matches `TrustSettings` before falling back to
`surface_renders_editor`'s boolean check, routing to a new `trust_settings_view` -- the second
real `open_surface`-conditional dispatch after `ApprovalHistory`, reusing the one predicate
rather than adding a parallel list.

**Grant**: `trust_settings_view`'s "Grant Trust…" button fires `Message::OpenTrustGrantDialog`,
which opens `ModalContent::TrustGrant` (focus defaulting to `Cancel`) -- guarded by the same
"never replace an open modal" rule `open_approval_history_entry` already uses.
`Message::ModalActivate`'s new arm only acts when focus is on `Grant`; any other focus, or
`ModalDismiss`, closes without granting -- the paste dialog's shape (one real decision, one
inert dismiss), not the approval dialog's (both buttons are decisions). The real grant goes
through `AuditCoordinator::grant_project_trust`, its first production caller.

**Revoke**: `trust_settings_view`'s "Revoke Trust" button fires `Message::RevokeWorkspaceTrust`
directly -- no confirmation dialog, per `what-the-trust-dialog-must-say.md` §5 ("revoking must be
as reachable as granting," not "as gated as granting" -- revocation is the safe direction).
`AuditCoordinator::revoke_project_trust`'s first production caller.

**Comparably reachable, stated precisely**: both controls live on the *same* `TrustSettings`
surface, so reaching either starts from the identical one action
(`NavigationAction::OpenTrustSettings`). From there: **Revoke is one further action** (click).
**Grant is three** (open the dialog, move focus to `Grant`, activate) -- a real, deliberate
asymmetry inside the security-sensitive action itself, not a difference in how deep either
control is buried. Never both offered at once (nothing to grant while already trusted, nothing
to revoke while not).

**Board**: `recent_project_row` (`project_board.rs`) no longer hardcodes `"Restricted"` --
reads the real cached `RecentProject.trust_state` (PR-032-B), except when `availability` is
`PathChanged`, where it correctly shows `Restricted` regardless of the cached value (a
canonical-path mismatch means reopening would not restore that cached trust either -- see
`AppState::add_project_session`'s own lookup). Disclosed, not silently assumed complete: this is
still the *cached* value for an unopened row, not one confirmed against the audit store the way
`verify_restored_trust` confirms an *open* project's -- a last-known snapshot, the same status
every other recent-only field already carries.

### Evidence

`crates/tekstide-core/src/project_board.rs`/`project_board/tests.rs`: the two stale fixtures
PR-032-B's own evidence flagged (constructing a `Trusted` recent entry, asserting the old
hardcoded `"Restricted"` label) now assert the real label; a new test proves the `PathChanged`
suppression specifically, with a real-directory fixture.

`crates/tekstide/src/shell/tests.rs` (23 new tests):

- **Enumeration, both directions**: `only_one_production_call_site_ever_grants_workspace_trust`/
  `..._revokes_workspace_trust` (response 246's own shape). **Ablated**: a throwaway second call
  site added to each, confirmed both failed naming both files, reverted.
- **The route**: `open_trust_settings_shell_input_routes_to_the_trust_settings_surface`.
- **Modal exclusivity**: `open_trust_grant_dialog_does_not_replace_an_already_open_modal`.
- **Focus defaults to `Cancel`; activating it grants nothing**:
  `trust_grant_dialog_defaults_focus_to_cancel_and_activating_it_grants_nothing`.
- **Granting needs both deliberate acts**:
  `trust_grant_dialog_requires_moving_focus_and_activating_to_grant`.
- **Audit records queried and asserted, not implied** (the gate's own words, "the way RFC-022's
  `command_approval` assertion did"): `granting_trust_through_the_real_route_records_both_audit_records`
  (Authorized then Applied, sharing one `operation_id`) and
  `revoking_trust_through_the_real_route_records_a_single_applied_record` (one `Applied` record,
  no `operation_id`).
- **Comparable reachability**: `trust_settings_surface_offers_grant_when_restricted_and_revoke_when_trusted`.
- **The dialog's own review gate, `what-the-trust-dialog-must-say.md` item by item**:
  - §1, escaping: `trust_grant_dialog_escapes_a_bidi_override_in_the_canonical_path` (falsifiable
    claim, **ablated** -- the real `quote_untrusted` call temporarily replaced with a raw one,
    confirmed this test fails with the raw override character in the panic output, reverted).
  - §1, no double-escaping: `trust_grant_dialog_body_does_not_double_escape_literal_marker_text`.
  - §1, canonical path shown, both shown when they differ:
    `trust_grant_dialog_paths_shows_both_when_root_and_canonical_differ`/
    `..._shows_only_the_canonical_path_when_they_match`.
  - §3, the canonical sentence verbatim:
    `trust_grant_dialog_body_contains_the_canonical_sentence_verbatim`.
  - §3, the nine features not enumerated:
    `trust_grant_dialog_body_does_not_enumerate_the_nine_restricted_features` (checks every
    `RestrictedModeFeature::ALL` label's absence, not a sample).
  - §4, present and future: `trust_grant_dialog_body_states_the_present_and_future_consequence`.
  - §6, none of the three forbidden claims:
    `trust_grant_dialog_body_makes_none_of_the_three_forbidden_claims`.
  - Modal exclusivity: covered by the same structural mechanism (`input::ModalAbsent`) every
    other modal in this crate already proves against, unchanged by this addition -- `ModalContent::TrustGrant`
    is one more arm inside the one type that mechanism gates on, not a parallel path.
- **The chain, not the link** (response 247's own required addition):
  `granting_trust_through_the_real_route_unblocks_a_real_agent_run_launch` -- refuses with
  `WorkspaceDiscoveryBlocked` before granting (against a custom profile reaching the identical
  gate `claude_code_linux_default` does), grants trust through the real dispatch chain (not
  `grant_trust` called directly), then the *same* profile launches for real: a real controlled
  test executable spawns, registers, and reaches `AgentRunStatus::Running` -- not merely that
  `trust_state()` changed.

`cargo test -p tekstide --bin tekstide shell::tests::` -- all passing (274 total in this crate).

### Response 248's required corrections: the surface was unreachable

Response 248 found that everything above was correct but sat on a surface no user could open --
`NavigationAction::OpenTrustSettings` was `Configurable`/`None`, and `Configurable`-with-`None`
"reads as pending, actually means dead" until RFC-023 (configuration/keybinding) exists.
`grant_project_trust` had a production caller no real key press could reach.

**Fixed:**

- **A real default binding**: `Ctrl+Alt+U`, `KeybindingStatus::Candidate` -- the same shape the
  six actually-reachable actions use, checked mechanically against every other rule
  (`open_trust_settings_shortcut_is_a_candidate_that_collides_with_no_other_rule`,
  `crates/tekstide-core/src/navigation/tests.rs`), not by inspection.
- **Keyboard navigation on the surface itself**: `handle_trust_settings_key`
  (`crates/tekstide/src/shell.rs`), wired as a fourth `FocusZone::MainArea` consumer alongside
  the editor/approval-history/(now) trust-settings key handlers, each still checking
  `open_surface` itself. No highlight index, unlike `handle_approval_history_key`'s list --
  `TrustSettings` shows exactly one control at a time (never both), so Enter always activates
  whichever one `trust_settings_view` is currently rendering, with nothing to move a cursor
  between.
- **Every test in this section rebuilt to start from a real key event**: a new helper,
  `press_trust_settings_action` (`Ctrl+Alt+U` via `shell_input_for_test`, then a real Enter via
  `send_main_area_key` -- the same helper `arrow_keys_move_the_approval_history_highlight`
  already established for that surface's own keyboard access), replaces every direct
  `Message::OpenTrustGrantDialog`/`Message::RevokeWorkspaceTrust` dispatch throughout this file,
  including the end-to-end chain proof. Two new tests cover `handle_trust_settings_key`'s own
  guard (`trust_settings_key_is_a_no_op_off_the_trust_settings_surface`,
  `trust_settings_key_ignores_keys_other_than_enter`).

**Also noted, not fixed here**: response 248 found `NavigationAction::OpenApprovalHistory` has
the identical `Configurable`/`None` gap (RFC-022, closed without catching it) -- the architect's
own record to correct separately; RFC-022's own consequence is smaller (`High`/`Destructive`
promotion still works without the history surface) but real. Out of this RFC's scope.

**The screenshot** (response 248's "one specific reason": escaping mangling legibility, not
rendering at all, is the actual risk for a dialog that is almost entirely a rendered path) --
`rfcs/handoffs/032-workspace-trust-granting/evidence/pr-032-d/trust-grant-dialog-bidi-override.png`.
A real, running Tekstide, launched against a real directory named
`safe-project<U+202E actual override, not the literal text>gpj`, navigated with real input the
whole way: `Ctrl+Alt+U` from the Project Board (the project opened via CLI arg becomes active by
default, and `OpenActiveProjectSurface` routes into the workspace on its own -- no separate
"enter workspace" step needed) lands on `TrustSettings` ("Current state: Restricted", one "Grant
Trust…" button); real Enter opens the dialog. The capture shows: the escaped path rendered as
`<U+202E>` inline and fully legible, not wrapped or truncated into something unreadable; focus
marked on `Cancel` (`> Cancel`, `Grant Trust` unmarked); the canonical sentence, the
present-and-future clause, and the "does not undo" clause all present and readable in full. Taken
via `niri msg action screenshot-window` + `wl-paste` (this niri config copies to the clipboard
rather than writing to disk); the scratch project directory and test process were removed after.

**What this capture does not prove** (response 249): the path fits on one line at the captured
window width. At a narrower width it would wrap, and a wrap landing inside `<U+202E>` would
split the marker across lines -- the same legibility failure this capture exists to rule out,
reappearing at a size not captured. Not fixed, not re-captured -- stated as what this evidence
covers and does not, the same convention every other capture in this codebase already follows.

### Gates run

`cargo fmt --all --check` clean. `cargo clippy --workspace --all-targets --all-features -- -D
warnings` clean. `cargo test --workspace --all-targets --all-features`: 884 passed, 0 failed, run
three times for stability. `git diff --check` clean. Orphaned test-spawned processes and scratch
directories cleaned up after each run.

## PR-032-D - The dialog

Built together with PR-032-C above -- see that section's evidence for the full,
item-by-item accounting against `what-the-trust-dialog-must-say.md`'s own review gate, and for
response 248's screenshot evidence specifically.

## PR-032-E - Closeout

### The claim statement, checked against the RFC and the decisions page

**What may be claimed**: workspace trust is grantable and revocable through a real, reachable
GUI route (`Ctrl+Alt+U` -> `TrustSettings` -> the confirmation dialog), proven end to end from a
real key event -- not from a dispatched `AppCommand`/`Message`, after response 248 found the
first version of this proof started one step after the step that did not exist. A profile
requiring workspace discovery, refused with `WorkspaceDiscoveryBlocked` in a fresh `Restricted`
project, launches for real once trust is granted through that route: a real controlled test
executable spawns, registers, and reaches `AgentRunStatus::Running`.

Persistence and binding match RFC-032's own two decisions, both the owner's, both reasoned in
`docs/src/contributors/security-decisions.md`: trust persists across sessions (restored on
reopen, bound to the *canonical* path -- proven against a real, redirected symlink, not a
synthesised path string) and does not follow a redirected symlink to a different real folder.
The audit store, not the user-writable recent-projects cache, is authoritative for what
"persists" actually means: a cache-restored `Trusted` is confirmed against a real, applied
`TrustGrant` before it is honoured (`verify_restored_trust`), and a completed grant survives an
interrupted later re-grant attempt rather than being silently undone by a dangling record.

**What may not be claimed, checked against §What the dialog may not claim and RFC-032's own
§Risks**:

- **Not that a trusted project is safe.** The dialog's own copy states what trust authorises
  ("Files inside the trusted folder may configure Tekstide and cause programs to run") and
  never characterises that as safe or acceptable -- whether it is depends on the folder, which
  Tekstide cannot assess, exactly as the RFC's own text says.
- **Not that Tekstide polices what runs.** Nothing here intercepts execution; granting removes
  a restriction, it does not add supervision. No claim to the contrary appears anywhere in the
  dialog or the surface.
- **Not that revoking undoes what already ran.** Stated as the negative explicitly, in the
  dialog's own body ("Revoking stops it from loading again; it does not undo anything that has
  already run") -- the one claim the dialog-copy handoff itself named as "easy to imply by
  omission and the one a user would most want to be true," so it is said plainly rather than
  left to silence.
- **Not that granting trust makes any other gated surface reachable.** RFC-022's
  `ApprovalHistory` surface has its own, entirely independent reachability gap (a stale
  `KeybindingStatus::Configurable`/`None` binding, found during this RFC's own review at
  response 248 and corrected in RFC-022's record separately, not here) -- granting workspace
  trust does nothing to that gap, and this pack does not imply otherwise.
- **Not that the real Claude Code CLI has been exercised.** Every real-process test in this
  slice, like every one before it in this codebase, uses a controlled test executable -- the
  live product needs interactive auth and makes real network calls, unsafe and unbounded for an
  automated test.

### The three requirements, confirmed shipped

Persistence is acceptable *because* of these (RFC-032's own framing) -- each is a requirement,
not an intention, and each is shipped:

1. **Revoking is always available.** One direct action from the `TrustSettings` surface, no
   confirmation dialog -- revocation is never gated behind the grant dialog's own two-act
   requirement.
2. **Trust state is visible on the project board.** `active_project_row` reads the live
   `trust_state()`; `recent_project_row` reads the real cached value (fixed this slice, was
   hardcoded `"Restricted"`), suppressed back to `Restricted` specifically when the canonical
   path has changed since it was cached.
3. **The dialog says the folder's contents, present and future.** `trust-grant-dialog-body`
   states explicitly that the grant covers files not yet written, including an AI agent run's
   own output, across every future session until revoked.

### What this unblocks, stated precisely (per response 249's own emphasis)

Trust becomes grantable, so `validate_workspace_discovery_policy` stops refusing a profile that
honestly declares it may discover workspace files, so an agent run using such a profile can
launch in a trusted project -- proven from a real key event, not merely that the trust flag
changed. **This is not the same as RFC-020's surfaces being reachable**, and this project has
made exactly that overstatement before (RFC-022's own closeout, corrected afterward). RFC-032
unblocks one gate in the agent-run launch validator; it does not build, wire, or make reachable
anything downstream of a launched run.

### Gates run

`cargo fmt --all --check` clean. `cargo clippy --workspace --all-targets --all-features -- -D
warnings` clean. `cargo test --workspace --all-targets --all-features`: 884 passed, 0 failed,
run three times for stability across every slice in this RFC. `git diff --check` clean.

### Documents updated in this closeout

- `rfcs/proposed/032-workspace-trust-granting.md` -> moved to `rfcs/done/`; `Status` rewritten
  to the closed, precise claim above, matching RFC-021/022's own closed-status convention.
- `rfcs/README.md`: the stale Proposed-table row removed; a new Implemented-table row added.
- This pack's other four documents (`README.md`, `task-breakdown-pr-plan.md`,
  `what-the-trust-dialog-must-say.md`, `acceptance-qa-checklist.md`): `status`/`rfc_file` front
  matter all updated -- `rfc_file` was pointing at `proposed/`, now stale after the move to
  `done/`, in every one of them (RFC-022's own closeout found four of five stale; checked
  directly here rather than assumed fixed by precedent).
- `acceptance-qa-checklist.md`: 27 substantive items ticked by the implementer, matching the
  precedent RFC-022's closeout established (response 237); "Final Acceptance Decision" and
  "Reviewer notes" left for the architect.
- `rfcs/future-work.md`: the "Workspace trust is a one-state machine" theme marked discharged;
  the reachability audit's own `grant_project_trust`/`revoke_project_trust` row and priority
  section updated to match (see below).

### A lesson carried forward, not re-learned

This RFC's own review cycle found the same failure class twice, at two different layers: the
dialog and mechanics were correct but the *route* to them did not exist (response 248), and
separately, RFC-022's `ApprovalHistory` surface has had the identical gap since its own closeout,
missed until this RFC's review found it by comparison. Both times the actual defect was
`KeybindingStatus::Configurable` with a `None` binding, which *reads* as "a user can bind this"
and *means* "dead until RFC-023 exists." `future-work.md` now names this as a category error
independent of either RFC, so a future closeout does not have to rediscover it a third time.

## Known limitations going in

- **Trust cannot be withdrawn from what already ran.** Revocation stops future loading only.
- **Canonical resolution is checked at open time** and cannot close the gap between check and
  use. Inherent to filesystems.
- **Trust expiry** (RFC-004's own open question) is not addressed.
