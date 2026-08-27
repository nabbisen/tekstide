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

## PR-047-B, PR-047-C

Not started.
