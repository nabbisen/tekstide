---
title: "RFC-047: Audit Store Corruption Recovery — QA evidence"
rfc: "RFC-047"
rfc_file: "../../accepted/047-audit-store-corruption-recovery.md"
source_rfc_status: "Accepted 2026-08-28 — M12"
target_milestone: "M12"
created: "2026-08-28"
---

# Evidence

## PR-047-A — stop collapsing the failures, and make the degradation observable

**No recovery yet, per the task breakdown's own scope.** This slice is the seam: distinguish why
the store did not open, accumulate that onto one `AuditHealth` for the whole session instead of a
fresh, dropped-immediately instance, and write a diagnostic a technical user can find. `resume()`/
`recover()` are not called here — that is PR-047-B.

### The seam: `AuditStoreOpenFailure`

`open_real_audit_store`/`open_audit_store` (`shell.rs`) now return
`Result<AuditStore, AuditStoreOpenFailure>` instead of `Option<AuditStore>`. `AuditStoreOpenFailure`
carries either `Environment` (no `HOME`/`XDG_STATE_HOME`, or the directory could not be created --
failures before `AuditStore::open` is ever reached) or `Store(AuditStoreErrorReason)` (the real
reason `AuditStore::open` itself returned). `open_real_audit_store` itself stays fail-silent, per
its own doc comment and §6 of the risk document ("do not make `open_real_audit_store` noisy") --
callers decide what to do with the failure.

**The new seam every production call site goes through**: `open_audit_store_recording_failure`
(`shell.rs`). Records the failure onto the caller's own `AuditHealth` (not a fresh instance),
writes one `eprintln!` line (unconditional, not gated behind `cfg!(debug_assertions)` the way
`i18n::log_missing_key` gates its own -- a release build is exactly where a real user hits this),
and returns `Option<AuditStore>` so every existing call site's own control-flow shape (`let
Some(store) = ... else { return }`, `if let Some(store) = ...`, `match ... { Some(...) => ...,
None => ... }`) compiles unchanged. Only what each site *records* changed, not how each is
structured.

### `AuditHealth` moves onto `State`, all fourteen sites accounted for

Fourteen former `AuditHealth::default()` construction sites, checked individually rather than
assumed shareable, per the README's own explicit warning:

- **Twelve** already took `state: &mut State` and now read/write `state.audit_health` directly.
- **One** (`record_new_project_added`) took `&State`; widened to `&mut State` -- checked all three
  of its own callers first, each already held `&mut State` at the call site.
- **Two** happen *before* `State` exists at all: `main.rs`'s `boot()` (the CLI-argument
  project-added producer, `record_project_added_if_possible`) and `State::new`'s own internal
  demo/measurement-panes launch (`TEKSTIDE_TERMINAL_DEMO`). Both now thread a real `AuditHealth`
  through instead of starting fresh -- `boot()` constructs it once, passes `&mut` through
  `open_cli_project_path_and_record`, then hands the accumulated value into `State::new(app_shell,
  catalog, audit_health)` as a new third parameter, which stores it rather than discarding it.

**One further downstream signature changed to make this possible without a double mutable
borrow**: `terminate_project_live_work` took `state: &mut State` *and* a separate `&mut
AuditHealth` parameter -- since `state.audit_health` and `state` overlap, its caller
(`apply_project_close_confirmation`) could not pass both. Dropped the separate parameter; the
function now reads `state.audit_health` directly, since it already had `state`.
`verify_restored_trust`'s own test-injection seam (`verify_restored_trust_against`, a bare
`FnOnce(&ApplicationShell) -> Option<AuditStore>` closure parameter, used by two of its own
tests to supply a real temp-dir-backed store) was left untouched -- `verify_restored_trust` itself
gained an `audit_health` parameter and routes through `open_audit_store_recording_failure` via a
closure that captures it, rather than changing the injected closure's own type.

### Something a technical user can find

The `eprintln!` line is the whole of it for this slice -- the on-screen board indicator is D3's
own PR-047-B work, not required yet per the task breakdown ("the on-screen indicator is not
required yet; the observability is").

### Required tests, each ablated for real

Four new tests (`shell/tests.rs`, "RFC-047 PR-047-A" section):

- `open_audit_store_recording_failure_distinguishes_recovery_incomplete` -- a bare recovery marker
  (no real store needed first, matching `recovery_is_active`'s own existence-only check) produces
  `AuditStoreErrorReason::RecoveryIncomplete` specifically. **Ablated**: changed `open_audit_store`
  to map every `AuditStore::open` failure to `Environment` regardless of the real reason (simulating
  the collapse this slice removes) -- failed, naming the wrong reason (`Path` instead of
  `RecoveryIncomplete`). Restored: passes.
- `open_audit_store_recording_failure_reports_a_different_reason_for_a_corrupted_file` -- the exact
  RFC-036 PR-036-C corruption method (a real store, then its database file overwritten with
  non-SQLite bytes) must **not** produce `RecoveryIncomplete`.
- `open_audit_store_recording_failure_accumulates_onto_the_same_health` -- the same `AuditHealth`
  value, reused across two failing calls, must show `failure_count() == 2`, not reset to 1 each
  time. **Ablated**: made `open_audit_store_recording_failure` construct a fresh `AuditHealth`
  internally instead of using the passed one (the literal pre-this-slice bug, reproduced on
  purpose) -- failed, `status()` stayed `Healthy` after a real failure. Restored: passes.
- `open_audit_store_recording_failure_leaves_health_healthy_on_success` -- a store that opens
  cleanly leaves `AuditHealth` exactly as it started (D3's "absent when healthy" principle, one
  layer down from the indicator itself).

### Live evidence: the exact RFC-036 PR-036-C reproduction, showing the difference

No on-screen indicator exists yet, so "showing the difference" is the new diagnostic line, not a
screenshot -- the full photographed evidence (indicator + confirmation wording) belongs at
PR-047-C's own close, once both exist. Release binary, fresh `mktemp -d` fixture, fresh `mktemp -d`
`XDG_STATE_HOME`, `WAYLAND_DISPLAY` unset (this project's own established convention).

Three real launches, stderr captured each time:

1. **A fresh state root**: store opens cleanly, empty stderr. (Precondition: confirms the
   diagnostic is silent when nothing is wrong.)
2. **The real `audit.sqlite3` overwritten with random bytes** (RFC-036 PR-036-C's own method,
   exactly): `[audit] the audit store did not open (Corrupt) -- this session's actions will not be
   recorded until it recovers`.
3. **A bare interrupted-migration marker** (`audit/recovery/active-recovery.json`, no real store
   needed first): `[audit] the audit store did not open (RecoveryIncomplete) -- this session's
   actions will not be recorded until it recovers`.

Both failure reasons are what a technical user watching stderr now sees; RFC-036 PR-036-C's own
screenshots of the same two corruptions showed nothing at all.

### Gate

`fmt`, `clippy --workspace --all-targets -D warnings`, `git diff --check`, `rfc_docs_invariants`
(4 tests): clean. Three consecutive full-workspace runs: **460 + 4 + 738** every time. One flake in
run 1 of the first pass, `approval::tests::channel::bind_recovers_from_a_stale_socket_file` --
already a known, extensively-documented ~2% baseline flake in `test-process-leak.md`, unrelated to
this slice (no `approval::channel` code touched); recorded as a dated recurrence there rather than
left unmentioned. Runs 2 and 3 of that same pass, and this evidence's own final confirmation pass,
all clean.

## PR-047-B — recover, and say what happened

### `tekstide-core`: two new orchestration methods, connecting what already existed

`AuditRecovery::recover_and_reopen`/`resume_and_reopen` (`audit/recovery.rs`) wrap the existing
`recover()`/`resume()` with the two things a caller actually needs afterward: a real, reopened
`AuditStore` (recovery itself leaves the store closed -- `finish_recovery`'s own internal open is
for the atomic-install step only) and, for `recover_and_reopen`, the quarantine directory the old
database went to. **Reconstructed from public data only**
(`storage_path.recovery_dir().join(receipt.recovery_id.as_str())`) -- confirmed against the same
path this module's own pre-existing test
(`recovery_quarantines_complete_artifact_set_and_records_fresh_event`) already proves `recover()`
itself writes to, not asserted independently. No schema change; nothing added to
`AuditRecoveryReceipt`.

**The `AuditStoreRecovery` durable record was already being written** -- found while designing
this slice, not assumed: `initialize_fresh_database` (called internally by both `recover()` and
`resume()`) already calls `store.append(&recovery_record(...))` and reports whether it succeeded
via `receipt.recovery_event_recorded`. This slice's own job was calling `recover()`/`resume()` at
all and reading that field back, not constructing the record itself.

`AuditHealth` gains `last_recovery: Option<AuditRecoveryDisclosure>` (`Resumed` or
`Recovered { quarantine_dir }`) and `record_recovery()`, which resets `status`/`failure_count`/
`last_failure` to healthy -- **only called after the durable record is confirmed written**, so a
recovery that leaves the disclosure unrecorded stays `Degraded` rather than reporting success (§4
of the risk document: "do not claim more than the record supports"). `last_recovery` itself is
never cleared by anything in this crate -- a durably-visible fact for the rest of the session, not
a toast.

**Cross-crate test access**: `corrupt_and_interrupt_recovery_for_test`
(`#[cfg(any(test, feature = "test-support"))]`) is the one way `tekstide`'s own tests can reach a
genuinely resumable state -- mirrors the private `manifest_write_failure_keeps_restart_guard_and_
can_resume` recipe exactly, since a hand-rolled marker/bundle pair would not match `resume()`'s own
internal format. Same gate `runtime::terminal::launch`'s leak guard and RFC-036's
`ProjectSession::add_transcript` already use to cross this exact boundary.

### `tekstide`: `open_audit_store_recording_failure` upgraded, zero call-site changes

**All 17 call sites already funneled through this one seam (PR-047-A's own design, confirmed by
the reviewer at response 357)** -- so connecting recovery needed no changes to any of them, only to
what happens inside the seam itself. `AuditStoreErrorReason::RecoveryIncomplete` routes to
`resume_and_reopen` (D1); every other reason routes to `recover_and_reopen` (D2) --
`recover()`'s own diagnostic guard safely refuses anything not actually diagnosed corrupt, so
attempting it for a transient failure costs one extra, safely-refused call rather than doing
anything wrong. "Once per session" (D1's own phrasing) falls out for free: a successful recovery
leaves a genuinely working store on disk, so the next call's own first `AuditStore::open` attempt
just succeeds -- no separate "already tried" flag to maintain.

`AuditStoreOpenFailure::Store` now carries the `AuditStoragePath` alongside the reason (needed to
retry) -- the one signature change in this slice, contained entirely inside `open_audit_store`/
`open_real_audit_store`'s own return type, invisible to every caller of the seam above them.

### Required tests, each read back rather than inferred from a return value

- `open_audit_store_recording_failure_resumes_and_records_the_recovery` -- a resumable marker
  resumes, and the `AuditStoreRecovery` record is **queried back out of the reopened store**.
- `open_audit_store_recording_failure_recovers_a_corrupt_store_and_reports_the_quarantine_path` --
  a corrupt store (RFC-036 PR-036-C's own method) recovers, **the old file still exists at the
  reported path** (`quarantine_dir.join("audit.sqlite3").is_file()`), and the record is read back.
  **Ablated**: replaced the reported `quarantine_dir` with a fake path -- failed, the file check
  correctly found nothing there. Restored: passes.
- `open_audit_store_recording_failure_produces_no_recovery_disclosure_for_a_healthy_store` -- a
  healthy store leaves `AuditHealth` exactly as it started.
- `open_audit_store_recording_failure_leaves_health_degraded_when_recovery_itself_fails` -- a
  recovery this project's own path validation refuses (`recovery_dir` replaced with a symlink,
  which `validate_for_recovery` rejects the same way `AuditStore::open` itself would) leaves
  `AuditHealth` `Degraded`, not reporting success, and claims no disclosure.
- Response 357's own required strengthening, applied: `..._reports_a_different_reason_for_a_
  corrupted_file` now asserts the *identity* of the outcome (`Recovered`, not merely "not
  `RecoveryIncomplete`"), and a new sibling test
  (`..._discloses_resumed_not_recovered_for_an_interrupted_migration`) proves the other half of the
  same contrast independently rather than by omission.

**Ablation, per the task breakdown's own required pair**: removed the `RecoveryIncomplete`
special-case branch (routing everything through `recover_and_reopen`) -- both the resume-specific
tests failed correctly, naming the exact wrong behavior. Restored: passes.

### D3: the degraded indicator, on the project board

`project_board_audit_lines(state) -> Vec<String>` composes the extra line(s) in `shell.rs`, kept
**out of** `surface::board::row_lines` deliberately -- that module's own doc comment already states
its architecture: it renders only what `ProjectBoardViewModel` hands it, and `AuditHealth` is a
session-wide concept a `tekstide-core` API change would be needed to thread through a per-project
row type, out of this slice's scope. Composed in `content_area`'s own `AppRoute::ProjectBoard` arm
instead, the same "shell.rs supplies data, the surface renders" split already used for
`terminal_launch_notice` and friends.

**Present when degraded** (`project-board-audit-degraded`, generic wording -- the technical
`AuditStoreErrorReason` stays in the `[audit]` stderr line only). **The one-time recovery
disclosure is separate from the ongoing indicator**: `last_recovery` renders even once `status()`
is healthy again, since D2's own disclosure must survive the moment `record_recovery` resets
`status` -- confirmed by its own test that the degraded line is specifically *absent* once
recovered. Quarantine paths are filesystem-derived and routed through
`text_safety::quote_untrusted` before reaching the catalog, the same discipline every other
filesystem-derived string on this surface already follows -- confirmed directly in the ablation
below (the isolate marks appear around the path in the assertion failure's own printed line).

**Required tests, ablated separately per the checklist's own explicit requirement** (deleting one
assertion must fail on its own, not only in combination):

- `project_board_audit_lines_is_empty_when_healthy_and_never_recovered`
- `project_board_audit_lines_shows_the_degraded_line_when_degraded`
- `project_board_audit_lines_shows_the_quarantine_path_when_recovered`

**Ablated**: made the function unconditionally push the degraded line. The "healthy" test failed
(a permanent line where none should be) and the "recovered" test failed too (the degraded line
appeared alongside the recovery disclosure, which must not happen for a session that is currently
healthy again). Restored: both, and the third, pass.

**Scope, stated rather than left implicit**: the indicator lives on the `ProjectBoard` route only,
matching the RFC's own D3 text verbatim ("the project board already carries a runtime summary...
a degraded-audit line belongs there"). A user deep in `ActiveProjectWorkspace` mode when
degradation happens will not see it until returning to the board -- D4/PR-047-C's own per-action
confirmations (not yet built) are what cover that surface instead, by design, not by oversight.

### Live evidence

Release binary, fresh `mktemp -d` fixture and state root, `WAYLAND_DISPLAY` unset. Two real
launches, both screenshotted:

1. **A genuinely corrupted store** (RFC-036 PR-036-C's own method): recovers automatically, and the
   project board now reads *"Audit: the previous audit file could not be read. It was moved to
   `<path>` and a new one was started."* (`EVIDENCE-1`). RFC-036 PR-036-C's own screenshot of this
   exact corruption showed nothing at all.
2. **An unrecoverable failure** (`recovery` replaced with a symlink, so both the initial open and
   the recovery attempt are refused by this project's own path validation): the board reads
   *"Audit: not recording. Recent actions may be missing from the record."* (`EVIDENCE-2`).

Neither shows a path under `$HOME`, a real project name, or another project on screen.

### Gate

`fmt`, `clippy --workspace --all-targets -D warnings`, `git diff --check`, `rfc_docs_invariants`
(4 tests): clean. Three consecutive full-workspace runs: **468 + 4 + 741, fully green** every
time -- no flake this pass.

## PR-047-B — response 358 required follow-up (R1, R2)

Response 358 accepted PR-047-B with two required changes. Both addressed here, before PR-047-C.

### R1: the recovered-but-unrecorded branch dropped the quarantine path

`open_audit_store_recording_failure`'s `Recovered`/`Resumed` arms called `health.record_failure`
only when `recovery_event_recorded` was `false`, never `health.record_recovery` -- so a real
quarantine that happened on disk was never disclosed, and the board showed only the generic
`project-board-audit-degraded` line (*"Audit: not recording..."*), which is false there: the store
works and is recording, what failed is the recovery's own record of itself.

Fixed at the root cause the reviewer named: §3 (surface the path) and §4 (a failed record-write is
degraded) govern different fields, not one. `AuditHealth::record_recovery` (`integration.rs`) is
now disclosure-only -- sets `last_recovery`, nothing else -- and a new `AuditHealth::clear_degraded`
carries the `status`/`failure_count`/`last_failure` reset that used to be bundled into it.
`open_audit_store_recording_failure`'s `Resumed`/`Recovered` arms now call `record_recovery`
**unconditionally** (the resume/recovery really happened, regardless of whether recording that it
happened also succeeded), then branch only on `clear_degraded()` vs `record_failure(reason)`.

The wording half of R1: `project_board_audit_lines` (`shell.rs`) previously showed the generic
degraded line whenever `status() == Degraded`, without checking whether a recovery had also been
disclosed this session -- exactly the collision §3.1 (now in the risk document, added by the
reviewer at `f59b5c5`) describes. Now checks both facts and picks the accurate line: the collision
(`status() == Degraded` **and** `last_recovery().is_some()`) renders a new catalog key,
`project-board-audit-recovery-not-confirmed` (*"Audit: recording again, but this recovery could
not confirm its own record was written."*), instead of the generic degraded line -- shown
*alongside* the existing `Resumed`/`Recovered` disclosure line, not replacing it, since §3's
requirement to name the path does not go away just because §4 also applies.

**New tests, each ablated separately:**

- `project_board_audit_lines_shows_the_collision_line_not_the_generic_degraded_line` -- `AuditHealth`
  driven directly (`record_recovery` then `record_failure`) to the collision state; asserts the new
  line appears, the generic degraded line does not, and the quarantine path still reaches the
  screen. **Ablated**: reverted the rendering fix to always show the generic degraded line whenever
  degraded -- failed, both the "must not show degraded" and "must show the new line" assertions
  caught it independently.
- `apply_recovery_outcome_stays_degraded_and_still_discloses_when_the_record_is_unconfirmed` -- see
  R2 below; also exercises the collision rendering end-to-end.

### R2: no test constructed `recovery_event_recorded: false` on a successful recovery

The cited test (`..._leaves_health_degraded_when_recovery_itself_fails`) proves the `Failed` arm (a
real refusal via a symlinked `recovery` directory), not the arm R1's defect actually lived in: a
recovery that **succeeds** (a real store comes back) but whose own `AuditStoreRecovery` record
write could not be confirmed. Grep confirmed no test in the suite constructed that case at all.

**Why it can't be triggered organically**: `initialize_fresh_database`'s `store.append(...)` writes
a record to a database this same call just created, schema'd, and validated -- there is no
reliable, portable filesystem trick to make that one write fail (SQLite runs in WAL mode there)
without also risking `prepare_for_atomic_install` or the post-write diagnostics check right after
it, which would take down the whole recovery instead of leaving it successful-but-unrecorded.

**Fix**: `open_audit_store_recording_failure`'s outcome-handling half was split into a new function,
`apply_recovery_outcome(outcome, reason, health) -> Option<AuditStore>` (`shell.rs`) -- the exact
same match arms, just callable with an outcome built anywhere, not only from the real
`AuditRecovery::recover_and_reopen`. A new `test-support` function,
`recover_and_reopen_forcing_unrecorded_event_for_test` (`tekstide-core/src/audit/recovery.rs`, same
`#[cfg(any(test, feature = "test-support"))]` gate as `corrupt_and_interrupt_recovery_for_test`),
runs the real quarantine/move/reopen path (`recover_with_move_and_initializer`, the same seam
`initialize_fresh_database` is itself plugged into) with a substitute initializer that does
everything the real one does -- real store, real schema, real `prepare_for_atomic_install` -- except
it discards the append's own result and always reports `Ok(false)`. Documented there as simulated,
not organic, the same honesty `corrupt_and_interrupt_recovery_for_test` already holds itself to.

`apply_recovery_outcome_stays_degraded_and_still_discloses_when_the_record_is_unconfirmed`
(`shell/tests.rs`): a real corrupted store, recovered via the forcing seam, fed into the real
`apply_recovery_outcome`. Asserts: a real, usable store comes back; `health.status()` is `Degraded`;
`health.last_recovery()` is `Some(Recovered { quarantine_dir })` with the file really present at
that path (`quarantine_dir.join("audit.sqlite3").is_file()`); and `project_board_audit_lines`
renders the new collision line, not the generic one, with the real quarantine path still present.

**Ablated twice, independently:**

- Reverted R1's `AuditHealth`-side fix (moved `record_recovery` inside the `if recovery_event_
  recorded` branch, reproducing the original bug) -- failed at the `last_recovery` assertion:
  *"§3.1: failing to attest the rename must not suppress disclosing it..."*.
- Reverted R1's wording fix only (`project_board_audit_lines` back to unconditional-on-degraded) --
  failed at the "collision case needs its own, accurate line" assertion, alongside the new
  `project_board_audit_lines_*` test above -- both caught it, not only in combination.

Restored: both ablations pass.

### No live screenshot of the collision state itself

EVIDENCE-1/EVIDENCE-2 above are real, organically-triggered corruptions. The R1/R2 collision
(`recovery_event_recorded: false` on a *successful* recovery) is, per R2's own finding, not
reliably constructible outside the `test-support` seam described there -- a live human run cannot
currently produce it. The rendering is proven by the ablated unit tests above instead of a
screenshot; noting this explicitly rather than presenting a test-support-forced state as an
organic live-evidence capture.

### Gate

`fmt`, `clippy --workspace --all-targets -D warnings`, `git diff --check`, `rfc_docs_invariants`
(4 tests): clean. Three consecutive full-workspace runs: **470 + 4 + 741, fully green** every
time -- no flake this pass.

## PR-047-C — say it before the click (D4)

D4: the agent-launch and trust-grant confirmations state, while the control is still live, that the
action will not be recorded. Two separate controls, two separate mechanisms.

### Trust grant: appended to the still-live confirmation body

`trust_grant_dialog_body` (`shell.rs`) gains a `degraded: bool` parameter. When true, appends
`trust-grant-dialog-degraded-notice` (*"This grant will not be recorded while the audit store is
degraded."*) as a final paragraph, after the canonical sentence and future-consequence text
`what-the-trust-dialog-must-say.md` already required -- this adds one more fact, not a replacement
for any of them. `trust_grant_dialog_view` passes `state.audit_health.status() ==
AuditHealthStatus::Degraded`. The whole body, notice included, renders above the still-live Grant
button -- read before the click, not after.

### Agent-run launch: an inline notice above the button, since no modal exists

Unlike trust grant, launching an agent run has no confirmation modal -- `LaunchAgentRunButtonPressed`
commits directly. A new `agent_run_launch_audit_notice(state) -> Option<String>` (`shell.rs`,
factored the same way D3's `project_board_audit_lines` is -- returns text, not an `Element`, so
presence/absence and wording are asserted directly rather than walked out of a widget tree) is
rendered in `trust_settings_view` immediately above the "Launch AI CLI Run" button, only while
`AuditHealth::status()` is `Degraded`. Catalog key: `trust-settings-launch-agent-run-degraded-notice`
(*"This run will not be recorded while the audit store is degraded."*).

### Wording, checked against the actual catalog strings

§5 of the risk document: must not imply the action is unsafe, must not imply the user can fix it
from here, must not appear when healthy. Both notices echo the RFC's own D4 text almost verbatim
("this action will not be recorded while the audit store is degraded") rather than inventing new
language, and state only that recording will not happen -- no "unsafe"/"danger"/"warning"/"risk",
no "fix"/"repair"/"resolve"/"try again". Checked directly against the catalog strings by
`trust_grant_dialog_degraded_notice_does_not_imply_unsafe_or_fixable` and
`agent_run_launch_audit_notice_does_not_imply_unsafe_or_fixable`, not by inspection alone.

### Required tests, present/absent ablated separately per surface

- `trust_grant_dialog_body_shows_the_degraded_notice_when_degraded` /
  `..._omits_the_degraded_notice_when_healthy` -- **ablated in both directions, run by me**:
  reverting the append (always omit) fails only the "shows" test; forcing an unconditional append
  fails only the "omits" test -- confirmed independently, not only in combination. Each is its own
  function per the task breakdown's explicit "ablated separately" requirement.
- `agent_run_launch_audit_notice_present_when_degraded` / `..._absent_when_healthy` -- same pair,
  same ablation discipline, for the agent-launch half. **Ablated both directions, run by me**:
  hard-coding `None` fails only "present when degraded"; hard-coding `Some(...)` fails only "absent
  when healthy" -- confirmed independently, not only in combination.

### Live GUI evidence: blocked by the synthetic-input environment, not by the feature

EVIDENCE-1/EVIDENCE-2 (PR-047-A/B) needed only a screenshot of whatever the app was already
rendering at boot. D4's evidence needs the app to actually *navigate* -- into `TrustSettings`, and
into the `TrustGrant` modal -- which needs synthetic keyboard input.

Built the fixture the same way as EVIDENCE-1/2 (`mktemp -d` project, `mktemp -d` `XDG_STATE_HOME`,
the EVIDENCE-2 "unrecoverable failure" method -- corrupt `audit.sqlite3`, `recovery` replaced with a
symlink -- so the session stays `Degraded` for the life of the process, not self-heal via automatic
recovery the way a plain corrupted file would). Launched the release binary against it, confirmed
via `niri msg windows`/`screenshot-window` that it is a real, live window rendering real state (the
D3 board line, *"Audit: not recording..."*, appears against the real corrupted fixture -- proving
the fixture and the running binary are both genuine).

**Could not drive it further.** `wtype` (this project's own documented synthetic-input tool,
`ARCHITECTURE.md`) delivers correctly to a native Wayland client in this same session -- confirmed
independently against a freshly spawned `alacritty`, typed text appeared -- but every keybinding
sent to the Tekstide window (`Ctrl+Alt+U` for Trust Settings, `Ctrl+Alt+P`, `Ctrl+Alt+O`, `Ctrl+Alt+T`,
even a bare unmodified key) produced no visible change across repeated attempts, including after
`niri msg action focus-window` and after relaunching the binary through `niri msg action spawn-sh`
(ruling out "launched via a background shell job never got an activation token" as the cause, since
Alacritty launched the identical way received input fine). No mouse-input tool (`ydotool`, `wlrctl`,
`dotool`) is available in this environment to try the alternative the task breakdown's own README
explicitly allows ("state whether a real mouse click was sent either way").

**Not claiming this as done.** The D3 board line's own correctness under this exact fixture is real,
live evidence that the degraded state is genuine; the D4 confirmations themselves are proven only
by the unit tests above, ablated in both directions. Flagged to the reviewer rather than presented
as captured -- consistent with this project's own rule that a screenshot states what it proves and
what it does not, and a convention nobody can execute in a given environment is worse than admitting
it plainly.

### Gate

`fmt`, `clippy --workspace --all-targets -D warnings`, `git diff --check`, `rfc_docs_invariants`
(4 tests): clean. Three consecutive full-workspace runs: **476 + 4 + 741, fully green** every
time -- no flake this pass.

## PR-047-C — response 360 required follow-up (R1, R2)

### R2: the wording tests forbid the wrong words but did not require the right meaning, fixed

`..._does_not_imply_unsafe_or_fixable` was negative-only -- the shows/omits pair is not vacuous (a
degenerate string breaks "omits"), but nothing asserted the notice actually **says** the action will
not be recorded. Confirmed exactly as described: swapped `trust-grant-dialog-degraded-notice` to
`"Audit note."` and all six PR-047-C tests stayed green.

Fixed by adding a positive assertion to both wording tests --
`notice.contains("will not be recorded") && notice.contains("degraded")` -- before the existing
negative checks. Re-ran the same ablation: `"Audit note."` now fails
`trust_grant_dialog_degraded_notice_does_not_imply_unsafe_or_fixable` at the new assertion, with the
two presence/absence tests (which only check whether the string appears at all, not its content)
unaffected, as expected. Restored the real string; all tests pass.

### R1: three attempts at the live capture, all with the same negative result

Re-verified against the release binary before attempting anything, per `ARCHITECTURE.md`'s updated
guidance (`09d5cff`): confirmed `wtype "PROBE-PLAIN"` and `wtype -M ctrl -M alt b -m alt -m ctrl`
both work against a freshly spawned window in this session in principle -- but every attempt to
reproduce that specifically against Tekstide failed the same way.

**Attempt 1** repeated the original method (spawn via `niri msg action spawn-sh`, fresh `mktemp -d`
`XDG_STATE_HOME`, plain-text probe first). The path field showed unrelated leftover text
(`123456789`) that was never typed by this session -- inconclusive on its own, but enough to stop
and flag it rather than proceed, since it meant keyboard input was landing somewhere unaccounted
for.

**Attempt 2**, after checking `niri msg focused-window` immediately beforehand and finding the
user's own editor focused (not the test window) -- explaining attempt 1's stray input as reasonable
focus contention on a shared desktop, not an environment defect. Relaunched, confirmed via
`niri msg focused-window` that the fresh Tekstide window (not the editor) held focus at the moment
of sending, then sent `Ctrl+Alt+B`. No folder browser opened; screen unchanged.

**Attempt 3**, same as attempt 2 but checking focus **both immediately before and immediately
after** sending the chord, closing the window in which the result could be explained by a focus
change mid-send. Both checks confirmed the Tekstide window held focus throughout. `Ctrl+Alt+B` still
produced no visible change.

**Conclusion**: this rules out focus contention as the explanation for at least the last attempt.
The `wtype` virtual-keyboard input is not reaching this specific application in this environment for
a reason not yet identified -- confirmed under the exact controlled conditions the reviewer's own
positive-control fix asks for, not merely repeating the original untested assumption. Leaving this
as an open item rather than a fourth blind retry; see review request 361.

### Gate (unchanged by R2's test-only fix)

`fmt`, `clippy --workspace --all-targets -D warnings`, `git diff --check`, `rfc_docs_invariants`
(4 tests): clean. Three consecutive full-workspace runs: **476 + 4 + 741, fully green** every
time -- no flake this pass (test count unchanged; R2 strengthened two existing tests rather than
adding new ones).

## PR-047-C — response 361 required follow-up (R1, attempt 4)

Response 361 reproduced `Ctrl+Alt+B` working against this exact commit, and diagnosed what attempts
2/3 got wrong: focus verification is not a positive control, since it confirms which window the
compositor considers active, not that a given `wtype` invocation actually reached it. Prescribed
sending plain text and the chord **in the same attempt** and reporting both outcomes, since only
that combination can distinguish "the chord doesn't work" from "no input reached the app at all."

**Attempt 4, run exactly as prescribed.** Fresh `mktemp -d` `XDG_STATE_HOME`, spawned via
`niri msg action spawn-sh`, empty board:

```
niri msg focused-window          -> Window ID 119, "Tekstide"
wtype "PROBE-PLAIN"
niri msg focused-window          -> Window ID 119, still
                                  -> path field shows "123456789" -- not what was typed
wtype -M ctrl -M alt b -m alt -m ctrl
niri msg focused-window          -> Window ID 119, still
                                  -> field unchanged ("123456789"); folder browser did not open
```

**Neither input landed.** Per response 361's own framework, this is case 2 -- "your input path, not
the app." Something in how this session's `wtype` reaches the compositor is the variable now, not
Tekstide's own keybinding handling.

**One further, unexplained detail worth recording rather than omitting**: the string `123456789`
that appeared in the path field instead of `PROBE-PLAIN` is not new to this attempt -- the exact
same nine-digit string appeared, verbatim, in an earlier attempt this same day against a completely
independent fresh window, fresh `mktemp -d` state directory, and different window ID. Two
independent fresh launches producing the identical, specific, non-random string in the same field
is not consistent with ordinary focus contention (a person retyping the same nine digits twice, at
exactly the moments this session happened to probe, on both occasions) -- it looks like a fixed
value from some other, unidentified source landing in this field, or a client-side artifact this
session cannot explain from the outside. Not investigated further per this project's own "avoid
rabbit holes" discipline and the explicit request to report rather than retry a fourth time without
a new variable; recorded here as a fact for whoever investigates the "which seat/session" question
response 361 named as the next real variable.

No screenshots committed or retained from this attempt (empty-board fixture only, no real paths
shown; deleted after inspection per this project's own screenshot-retention discipline for
inconclusive captures).

Gate unchanged -- no code or test changed by this attempt.

## PR-047-C — response 362 required follow-up (R1, attempt 5)

Response 362 identified `123456789` as informative rather than noise: a fixed nine-digit string
appearing identically across independent fresh launches fits a client decoding raw keycodes under
the wrong keymap before `wtype`'s own temporary keymap is adopted (consecutive slots landing on the
number row, `1 2 3 4 5 6 7 8 9 0`), though the exact string doesn't fully match that theory's own
prediction for `PROBE-PLAIN`'s length. The reviewer's own control on the same commit showed
`wtype "ZZZ"` landing as `ZZZ`. Prescribed one discriminating command -- `wtype "ZZZ"` -- with three
named outcomes (lands correctly; a single repeated digit, confirming keymap-slot substitution;
`123456789` again, falsifying both hypotheses).

**Attempt 5, run exactly as prescribed.** Fresh `mktemp -d` `XDG_STATE_HOME`, spawned via
`niri msg action spawn-sh`, empty board, focus confirmed on the new window beforehand:

```
wtype "ZZZ"   -> path field: empty. Not "ZZZ", not a repeated digit, not "123456789".
```

**A fourth outcome, not one of the three named.** Tried the reviewer's own suggested mitigation for
the keymap-timing case next, in case the field being empty was the same underlying problem in a more
severe form (events dropped entirely rather than merely misdecoded): `wtype -s 200 "ZZZ"` against
the same window. Also empty -- no visible change at all.

Not guessing further at what a fourth, unpredicted outcome means. Reporting the literal result
rather than fitting it to either standing hypothesis; see review request 363.

No screenshots committed or retained (empty-board fixture only; deleted after inspection).

Gate unchanged -- no code or test changed by this attempt.

## PR-047-C — response 363 required follow-up (R1, attempt 6, final)

Response 363 identified the real gap: every attempt 1-5 assumed `niri msg action screenshot-window`
+ `wl-paste` returns the *current* frame, and that was never verified. `screenshot-window` returns
`rc=0` while writing nothing -- the frame reaches the clipboard, and a stale clipboard read would
explain every anomaly seen so far (`123456789` identical across independent windows; `empty` at
attempt 5 unmoved by `-s 200`) without implicating `wtype` at all. Prescribed two checks: does the
clipboard receive a capture at all, and do two captures taken around a real change actually differ.

**Check 1 (clipboard receives a capture):** `wl-copy --clear`, `screenshot-window -d true`,
`wl-paste` -- a real, non-empty PNG (41697 bytes) came back. Confirmed.

**Check 2 (do two captures differ around a change), run exactly as prescribed:** capture, `wtype
"ZZZ"`, capture again. **Identical** (`cmp` exit 0, both 41697 bytes) -- matching the reviewer's own
"stale capture" outcome exactly.

**Went one step further before accepting that conclusion**, since accepting it meant declaring five
rounds of negative evidence artifacts of measurement rather than fact, and that deserved its own
check: is the capture pipeline ever capable of showing a real difference for this window, or is it
globally stuck? Captured, then triggered a real, compositor-driven, wtype-independent change
(`niri msg action fullscreen-window --id <id>`), captured again. **These two differed** (different
byte count, `cmp` found the first differing byte at offset 19). So the pipeline is not globally
stuck -- it picks up a real change when the compositor itself causes one.

**Then ran the actual, real D4 capture attempt against this proven-live pipeline.** Fresh corrupted-
audit fixture (EVIDENCE-2's method, `Degraded` confirmed via the D3 board line rendering correctly),
capture, `Ctrl+Alt+U` (Trust Settings -- would change the whole route and layout), capture again:
**identical.** Repeated with `Ctrl+Alt+T` (launch terminal -- an even larger layout change, a new
pane): **also identical.**

**Conclusion.** The measurement is proven live (the fullscreen test differs); the same measurement,
around two keybindings each large enough to be unmissable if they landed, shows no difference at
all. This is no longer explainable as a stale-capture artifact -- the one remaining doubt response
363 raised is now closed, in the direction that confirms rather than overturns five attempts' worth
of readings. The input is not reaching this application's keybinding handling in this environment,
under the exact verification response 363 asked for.

Six attempts, the reviewer's own three input-side hypotheses and one measurement-side hypothesis all
tested and none holding, is where this closes per the reviewer's own explicit bound ("the next
request either carries the capture or records option 3 as decided"). Recording option 3: D4 is
accepted on the ablated unit-test evidence in this handoff (present/absent, wording-content, all
independently ablated in both directions), with live GUI capture left as a documented, investigated,
and not-resolved gap rather than a silently dropped requirement.

No screenshots committed or retained from this attempt (fixture-only paths, no real `$HOME`;
deleted after inspection).

Gate unchanged -- no code or test changed by this attempt.

## PR-047-D — `status` is not a latch (§3.2)

### `AuditHealth` splits one shared flag into two independent capabilities

`status: AuditHealthStatus` was a single field, set by both the shell seam's open-failure path and
`AuditCoordinator::append_required`/`append_observation`'s write-failure path, cleared only by
`clear_degraded()` on the recovery path -- nothing cleared it on an ordinary successful open, which
is the measured defect (`record_failure(Busy)` then an ordinary open left `status=Degraded`
permanently).

Split into `open_status`/`write_status` (`integration.rs`), each its own
`record_open_failure(reason)`/`record_write_failure(reason)` and
`clear_open_failure()`/`clear_write_failure()`. `status()` is now computed --
`Degraded` if either is currently down, `Healthy` only if both are up -- rather than stored, so there
is no way for a caller to set it directly and no way for the two capabilities to be conflated by
construction. `failure_count`/`last_failure` are untouched by either `clear_*` method (§3.2 rule 2):
session history that survives capability returning, on every path including recovery, where the old
`clear_degraded()` used to zero it.

### Call sites, by kind

- **Open**: `open_audit_store_recording_failure`'s `Ok(store)` arm (previously untouched --
  the actual defect) now calls `clear_open_failure()`. Its `Environment` failure and
  `apply_recovery_outcome`'s `Failed` outcome call `record_open_failure(reason)`. `apply_recovery_
  outcome`'s `Resumed`/`Recovered` arms call `clear_open_failure()` unconditionally (the store did
  reopen) and then `clear_write_failure()`/`record_write_failure(reason)` depending on whether the
  recovery's own record write succeeded -- reclassified from "open" to "write" per §3.2, since by
  this point the open itself is not in question, only the record.
- **Write**: `append_required`/`append_observation` (`integration.rs`) call `record_write_failure`/
  `clear_write_failure` around their own write attempt -- the successful-write branch previously
  touched `AuditHealth` at all.

### The board: two independent lines (§3.2 rule 3)

`project_board_audit_lines` gains a history line, `project-board-audit-history` (pluralized,
`$count`), rendered whenever `failure_count() > 0` -- independent of and in addition to the existing
present-tense line, which still renders only while `status()` is `Degraded`. The two can and do
render together (a currently-degraded session that also has history), and the history line alone can
render once capability returns. Rule 3 is what makes rule 1 safe: without it, a cleared `status`
would hide that an action this session went unrecorded.

### Required tests, each ablated separately, all run by me

- `open_audit_store_recording_failure_clears_a_transient_open_failure_on_a_successful_open` /
  `..._preserves_history_through_a_transient_open_failure_clearing` -- the required pair.
  **Ablated**: removing `clear_open_failure()` from the `Ok(store)` arm fails only the first;
  making `clear_open_failure()` also zero `failure_count`/`last_failure` fails only the second (and
  the recovery-path test below, not this pair's own "clears" half).
- `open_audit_store_recording_failure_does_not_clear_a_write_failure_on_a_successful_open` -- the
  test that catches the naive one-flag fix. **Ablated**: made `clear_open_failure()` also clear
  `write_status` (the literal naive fix the risk document names) -- failed only this test; the
  "clears a transient open failure" test above stayed green, since it is testing the direction the
  naive fix gets right.
- `open_audit_store_recording_failure_recovery_path_preserves_failure_count` -- §3.2 rule 2 on the
  specific path most likely to have real history. **Ablated** together with the "preserves history"
  test above (same underlying defect: `clear_open_failure` zeroing history) -- both failed, the
  "clears" tests did not.
- `project_board_audit_lines_omits_the_present_tense_line_once_a_transient_open_failure_clears` /
  `..._still_shows_the_history_line_once_a_transient_open_failure_clears` -- D3's own required pair,
  one layer up. **Ablated**: removing the history-line block from `project_board_audit_lines`
  entirely failed only the second; the first (already covered by the pre-existing collision test's
  own sibling assertions) was unaffected.
- `a_successful_write_clears_a_prior_write_failure` / `a_successful_write_does_not_clear_a_prior_
  open_failure` (`tekstide-core`, `audit/tests/integration.rs`) -- the write-side symmetry, not
  explicitly required by the task breakdown's four items but the same rule 1 guarantee in the other
  direction, checked for completeness against a genuine successful write
  (`record_safe_close_authorized`) rather than only the open side.

### Evidence

Per the delivery plan's bounded-evidence rule and the task breakdown's own note ("unit-level is
sufficient and expected... this slice changes when the D3 lines appear, not what they look like"),
no live capture for this slice -- EVIDENCE-1/EVIDENCE-2 already show the lines' appearance live, and
nothing about their rendered appearance changed here, only when each one is present.

### Gate

`fmt`, `clippy --workspace --all-targets -D warnings`, `git diff --check`, `rfc_docs_invariants`
(4 tests): clean. Three consecutive full-workspace runs: **482 + 4 + 743, fully green** every
time -- no flake this pass (six new tests in `tekstide`, two new in `tekstide-core`).

## PR-047-D — response 365 required follow-up (R1)

Accepted with one required fix: the history line named a quantity it does not hold. `failure_count`
is incremented once per *record write attempt* (`record_open_failure`/`record_write_failure`), and
several best-effort producers do not short-circuit each other -- closing one project with three
terminals writes five records (`record_safe_close_authorized`, `record_plain_terminal_terminated`
once per terminal, `record_safe_close_decision`), so a degraded store during that close would have
read *"5 actions this session were not recorded"* for one action. `grant_project_trust` doesn't show
this because its `append_required` short-circuits before a second write -- which is why the path most
tests exercise never surfaced it.

This is the third instance in this RFC of the same shape (§4.1, added by the reviewer): a number or
sentence naming something adjacent to what was actually measured, caught in review each time because
every test asserted the line was *present*, never that it was *true*.

**Fix**: `project-board-audit-history` now says "record(s)... were not written" rather than
"action(s)... were not recorded" -- naming the quantity `failure_count` actually holds, not inventing
an action-level counter (explicitly not asked for -- "action" has no definition in `AuditHealth`
today, and inventing one inside a wording fix would be a second, unrequested design).

**New test**: `project_board_audit_history_names_records_not_actions` -- checks the real catalog
string (both plural forms) for "record" and the absence of "action". **Ablated**: reverted to the
original "action(s)... recorded" wording -- failed, naming exactly the overclaim response 365
described. Restored: passes.

### Gate

`fmt`, `clippy --workspace --all-targets -D warnings`, `git diff --check`, `rfc_docs_invariants`
(4 tests): clean. Three consecutive full-workspace runs: **483 + 4 + 743, fully green** every
time -- no flake this pass (one new test).
