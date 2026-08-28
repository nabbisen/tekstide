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

## PR-047-C

Not started. D4 -- the agent-launch and trust-grant confirmations naming the unrecorded state
before the click -- is the reason this RFC exists and is not optional; landing A and B and calling
it done is the exact outcome the task breakdown's own "what this RFC must not become" section
warns against.
