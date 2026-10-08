# RFC-027 — QA evidence

## PR-027-A — the marker

### A crash is detected, not guessed (D1/D12)

`tekstide_core::recovery::start_instance` (`crates/tekstide-core/src/recovery/instance.rs`):
scans `<state_root>/recovery/instances/` for marker files left by a previous run **before**
writing this run's own, so a freshly created marker can never be mistaken for evidence against
itself. Each marker is named by the pid that wrote it (`InstanceMarker::create`, named by
`std::process::id()`) -- `pid_is_alive`, the production liveness check, decides what happens to
each one found:

- **still alive** -- a concurrent sibling instance (D12). Left untouched: not removed, not
  reported.
- **not alive** -- the instance that wrote it crashed. Removed and reported as a
  [`DetectedCrash`], since a startup scan is the only consumer of the fact in this slice -- there
  is no recovery record yet (PR-027-B) to tie a longer lifecycle to.

### The liveness check is production code (checklist row 3)

`pid_is_alive` (`recovery/instance.rs`) is a real, non-`#[cfg(test)]` function: `libc::kill(pid,
0)`, `ESRCH` the only outcome distinguished from "still there" -- the same technique
`test_support::process_is_alive` already used, mirrored rather than imported, since
`test_support` is `#[cfg(test)]` and production code could never reach it regardless. D12 asked
for the real implementation, not a weakened gate around the test one; this is that.

### Proved against real processes, not `simulate_crash()` (D6, §4 row 15; checklist row 1)

Four core-level tests (`crates/tekstide-core/src/recovery/tests.rs`):

- `a_first_run_detects_nothing_and_writes_only_its_own_marker` -- a fresh state root: no crash to
  detect, and the only file `start_instance` writes is the new marker itself (§1 row 1: no buffer
  content anywhere).
- `a_clean_exit_leaves_no_marker_behind` -- `start_instance`, then `drop` the returned marker (the
  same thing a normal application exit does to `shell::State`'s own field): the marker file is
  gone.
- `a_real_sigkill_leaves_a_marker_a_later_startup_detects_as_a_crash` -- a real `sleep` child
  process is spawned for its real, kernel-assigned pid; a marker is written under that pid (the
  marker a real instance with that pid would have written); the child is `kill()`ed and `wait()`ed
  so it is genuinely reaped, not merely signalled. A fresh `start_instance` call against the same
  directory reports exactly that pid as a detected crash and removes its stale marker. No
  `simulate_crash()` helper anywhere in this path -- a real process, really killed, really reaped.
- `a_concurrent_sibling_instance_is_not_reported_as_a_crash` (D12, checklist row 2) -- the same
  real-process technique, this time left running: a live sibling's own marker is neither reported
  as a crash nor removed, and the new instance's own marker is confirmed to be a distinct file.

Plus two inline unit tests for the liveness primitive itself
(`recovery::instance::tests::pid_is_alive_is_true_for_this_very_process`,
`...is_false_for_a_pid_that_does_not_exist`, against `i32::MAX` as a pid no real system has
allocated).

### Pid reuse is disclosed, not fixed (checklist row 4)

Named in `start_instance`'s own doc comment, in this file, and here: a stale marker whose pid
number has since been reused by an unrelated live process reads as "still running" --
indistinguishable from a genuine concurrent sibling (D12's own case) -- so it is neither detected
nor removed. **That is the direction this must fail in**: not offering recovery costs the user a
prompt; falsely offering stale text as theirs is the failure this RFC exists to prevent. No
cleverer check (an extra identifying field inside the marker, a start-time comparison, etc.) is
built for it in this slice, per the RFC's own instruction not to.

### Nothing user-facing, no buffer content written anywhere (checklist row 5)

`start_instance` touches nothing under `crates/tekstide-core/src/content/`; the only I/O in this
slice is the marker file itself (empty, created and removed), proved directly by
`a_first_run_detects_nothing_and_writes_only_its_own_marker`'s own directory-listing assertion.
Detected crashes are logged to `stderr` in `boot()` (`crates/tekstide/src/main.rs`) --
"internally only," per the slice's own scope, since there is nothing to offer back yet.

### Wired into the real application, not only testable in isolation

`boot()` resolves `AppStatePathProvider::linux_default()` a second time (the same independent-
resolution shape `resolve_audit_state_dir` already uses for the audit store -- a cheap, stateless
env-var read, not a second source of truth) and calls `start_instance` before constructing
`ApplicationShell`'s own recent-project store. The returned marker is threaded through
`State::new`'s new parameter into a new `_instance_marker` field on `shell::State`, held for the
whole application lifetime so a normal exit's own `Drop` removes it. The leading `_` is
deliberate: the field is never read after construction by design, the same "held only for its
`Drop`" shape `test_support::RealProcessSlot`'s own callers already spell with `let
_real_process_slot = ...`, here on a struct field instead of a local -- not a lint workaround.

`state_holds_the_instance_marker_for_its_whole_lifetime_so_a_normal_exit_removes_it`
(`crates/tekstide/src/shell/tests.rs`): a real marker on disk, a real `State` built with it
(`state exists` -> marker still on disk), a real `drop(state)`, and a real check after -- proving
the wiring, not only the lower-level marker logic `recovery::tests` already covers.

**Not independently verified: that `iced`'s own event loop really drops `State` (and therefore
this field) on an ordinary window close**, as opposed to some `std::process::exit`-style shortcut.
Read directly from `iced_winit` 0.14.1's own source (`window_manager.is_empty()` on the last
window's `Destroyed` event sends `Control::Exit`, which breaks `process_event`'s own loop and
returns normally -- no `process::exit` anywhere in that path) rather than assumed, but not proved
by a live capture of closing the real window and checking the marker is gone, since that would
need driving a real window manager the way live-capture evidence elsewhere in this project does,
and this slice has no user-facing surface yet to capture. **Disclosed as reasoned, not measured**,
the same category review 477 named for a different claim this session: a structural fact checked
once against the code, not inferred from first principles alone, but short of the real-process
proof the slice's other claims get. The two `std::process::exit` calls already in `shell.rs`
(`Message::MeasurementTick`/`Message::MeasurementFrame`) are the one confirmed exception, reachable
only under `TEKSTIDE_*` measurement env vars, never in ordinary use.

## Gate, PR-027-A

- `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`: clean.
- **Three consecutive full-workspace runs, `--no-fail-fast`, fresh short `TMPDIR` each run**
  (`/dev/shm/g481{1,2,3}`): `739 + 16 + 1095` (+ `0+1+1` doctests), 0 failed, 0 fixture entries left
  in each run's own `TMPDIR` afterward.
