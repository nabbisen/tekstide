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

## Review 481: an unvalidated pid cast two ways to always read as "alive"

Verified live against the real binary, not only in the test suite -- the review planted three
marker filenames and ran `target/debug/tekstide` against them: `999999999` was detected and
removed correctly; `0` and `4294967295` both survived the scan, because `libc::kill` treats `0` as
"my own process group" (always answers "alive") and a `u32` cast of anything past `i32::MAX` wraps
to a negative `pid_t`, where `-1` means "every process I may signal" (also always "alive"). Neither
could ever be cleaned.

**Not harmful today** -- `InstanceMarker::create` only ever writes a real `std::process::id()`, so
this is defense against a malformed or adversarial filename, not a case this module's own writer
produces, and the two stray files it leaves behind are empty and inert. **The reason it matters
regardless**: PR-027-B ties real user content (a dirty buffer's own record) to a marker's own
lifecycle, and a marker that is *wrongly* readable as "a live instance" forever would let a record
outlive its reason the same way -- §2 row 7 of the risk document, in the slice whose whole job is
not doing that.

**Fixed**: the pid parsed from a marker filename is range-checked (`1..=i32::MAX as u32`) before it
ever reaches `pid_is_alive`'s own cast -- rejected at parse time, the same bucket a non-numeric
filename already falls into (ignored, not deleted, not reported). `0` and `u32::MAX` are now never
classified as a live sibling and never classified as a crash; they are simply not a pid this module
recognises.

`an_out_of_range_marker_filename_is_neither_a_crash_nor_a_live_sibling`
(`crates/tekstide-core/src/recovery/tests.rs`) reproduced the review's own three-row table at the
`start_instance` level: `999999999` still detected and removed; `0` and `u32::MAX` both left on
disk, neither reported. **Superseded by review 482's own finding below** -- that test passed
whether or not the fix was actually present, so it is not kept as a second, weaker copy.

### Gate, review 481's fix

- `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`: clean.
- **Three consecutive full-workspace runs, `--no-fail-fast`, fresh short `TMPDIR` each run**
  (`/dev/shm/g482{1,2,3}`): `739 + 16 + 1096` (+ `0+1+1` doctests), 0 failed, 0 fixture entries left
  in each run's own `TMPDIR` afterward.

## Review 482: the fix was right, its test proved nothing, and my own justification was wrong

**The test passed without the fix** -- the review's own ablation, not mine: `InstanceStartup`
carries only `detected_crashes`, and `0`/`u32::MAX` produce no detected crash either way, before
the fix because `pid_is_alive` answers "alive" and after it because the parse-time filter rejects
the filename -- the same observable outcome either way.

**Fixed by testing the property the fix actually establishes, not its downstream effect.** The
filename-to-pid step is its own named function now, `marker_filename_to_pid`
(`crates/tekstide-core/src/recovery/instance.rs`), asserted directly:
`marker_filename_to_pid_rejects_zero_and_anything_past_i32_max`
(`recovery::instance::tests`) asserts `"0"` and `"4294967295"` both yield `None`, `"999999999"`
and `i32::MAX`'s own string yield `Some` of themselves, and a non-numeric name yields `None` too.
**Ablated against this response's own commit**, `.filter(|pid| (1..=i32::MAX as u32).contains(pid))`
removed: this test fails immediately, the moment the filter goes -- verified before trusting it as
the fix's own proof, not assumed from reading the code.

**Review 481's own justification was wrong, and the fix is right not to have delivered it.** The
required item was framed as "a marker is always either live or removable"; `0` and `u32::MAX`
never become removable under this fix, and they should not --
`local-data-and-privacy.md`'s own published rule is "a file Tekstide did not write is never
deleted," and a filename this module does not recognise is, by definition, one we did not write.
The actual property this fix protects is narrower, and belongs to PR-027-B: a recovery record's
own cleanup must never depend on a marker filename this module does not recognise, since an
ignored marker is harmless on its own but an ignored marker that stops a record of real user
content from ever being cleaned is §2 row 7.

### Gate, review 482's fix

- `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`: clean.
- **Three consecutive full-workspace runs, `--no-fail-fast`, fresh short `TMPDIR` each run**
  (`/dev/shm/g483{1,2,3}`): `739 + 16 + 1096` (+ `0+1+1` doctests), 0 failed, 0 fixture entries left
  in each run's own `TMPDIR` afterward.

## PR-027-B — the record, with its purge

The slice that creates user content, and therefore the slice that ships its own purge, its own
`Retained locally` figure and its own `local-data-and-privacy.md` section in the same response --
D10's own ordering constraint, not a later one.

### The record: text, cursor, viewport, the snapshot it was opened against -- never undo (D2, D3)

`tekstide_core::recovery::record` (new): `RecoveryRecord` -- `relative_path`, `text`, cursor,
viewport, and `RecoveryFileSnapshot` (`canonical_path`/`modified_at` at full `SystemTime`
precision/`len`, deliberately **not** `content::FileSnapshot`'s own `content_hash`, which is
documented "not persisted, not a durable file identity" -- D5's own three-way disk comparison,
PR-027-C's job, only ever needs path/mtime/len, the same fields `TextDocument::save`'s existing
external-change check already treats as what changed). No `undo_stack`/`redo_stack` field exists on
the type at all -- not merely unpopulated, structurally absent, so there is no undo history for a
future slice to accidentally start persisting.

One file per record (`<pid-free, path-hash>.json`, a deterministic, non-cryptographic hash of the
document's own relative path -- the same document always lands on the same file, so a later write
replaces rather than accumulates), in its own project-scoped directory,
`<state_root>/recovery/records/<project_id>/`, `0600` in a `0700` directory (D13, §2 row 6, checked
by reading the real mode bits), written atomically (temp file, `create_new`, synced, renamed,
directory synced) -- the identical durability shape `write_run_record` already has.

`record_file_name_is_stable_and_distinct_per_path`, `record_round_trips_through_json_exactly`,
`file_snapshot_round_trips_sub_second_precision` (`recovery::record::tests`);
`a_written_record_has_the_right_permissions_and_reads_back_exactly`,
`writing_the_same_document_twice_replaces_its_own_record`
(`recovery::tests`) prove the write path directly, on real files, with real mode bits.

### Two byte bounds, each refused before anything reaches disk, each named (D9, measurement 5)

`RecoveryRetentionLimits { max_bytes_per_record, max_bytes_total }` (8 MiB / 192 MiB compiled
defaults -- the per-record bound is headroom over the 4 MiB editable-open cap for the JSON
envelope; the total is twenty records at that bound with headroom, matching the open set's own
D4 bound). Both checked **before** any byte reaches disk: the per-record bound against the
record's own serialized size; the total bound against every project's own on-disk bytes
(`<state_root>/recovery/records/*/`, summed), **excluding** the document's own previous record so
replacing it at the same size always fits even at an exact-total limit.

`a_record_over_the_per_record_bound_is_refused_and_nothing_is_written`,
`a_record_over_the_total_bound_is_refused_and_nothing_new_is_written` (`recovery::tests`) prove
both refusals and that nothing is written on either. The refusal is named to the user at the
moment it happens, not only logged: `RecoveryPersistRefusal`/`RecoveryPersistRefusalReason` (moved
into `tekstide-core` so the render layer can depend on it without crossing into the shell crate --
`RecoveryRecordWriteError` itself holds a non-`Clone` `io::Error`, unfit for a notice held across
frames) renders as a new line per refusal in the editor's own chrome
(`editor-recovery-persist-refusal-too-large`/`-total-bound`/`-io`), proved directly against the
catalog (`recovery_persist_refusal_lines_names_each_path_and_reason`,
`surface::editor::tests`, including the untrusted-path escaping `save_all_notice_lines`'s own
rows already get).

### Deleted the moment its own reason ends (D11, measurement 6)

Three real triggers, not one: **save** (`attempt_save_active_document`/
`save_all_documents_button_pressed`, removed the instant `SaveDecision::Saved` comes back -- a
clean document counts, RFC-065 review 477's own lesson, and costs one `remove_recovery_record`
call that is a no-op when there was never a record to begin with); **a tick finding the document
clean again** (undo back to the text it was opened with, the periodic tick's own job, since D2's
principle -- a clean document's content is already on disk -- applies the moment it becomes true,
not only when it starts); and **the project closing** (every document it held has just stopped
being open).

`a_dirty_document_gets_a_recovery_record_and_saving_removes_it`,
`undoing_back_to_clean_removes_the_stale_record_on_the_next_tick`,
`closing_a_project_removes_its_recovery_records` (`shell::tests`) prove each trigger on real
files.

**A real product constraint found while writing the close test, disclosed rather than worked
around**: this product's close assessment reads a project's own *live* dirty-document count, not
disk, and confirming the close-project dialog does not discard a text document's own dirty state
the way it terminates a live terminal session -- a genuinely dirty document cannot be closed
through the ordinary confirm path at all today. The close test therefore writes its own stale
record directly (standing in for one a crash, or an earlier dirty period already saved over, left
behind) against an otherwise-clean document, which closes through the immediate `SafeToClose`
path. This is a real, disclosed product behaviour, not a gap in this slice's own test coverage --
and it means D11's "close" trigger will, in practice, only ever find a record to remove when that
record is already stale for some other reason (a crash, or a save that happened to race a close
attempt), never when the close itself is what makes the document stop being dirty.

### The purge, the figure, and the privacy page -- in this same response (D10)

`purge_project_recovery_records` is a new primitive in `tekstide-core`, shared by two real
triggers rather than opening a second purge control (D14): the Trust Settings purge button
(`apply_transcript_purge`, extended) and a project closing (above) both call it. Byte/record
counting (`project_recovery_record_bytes`) mirrors `run_record_bytes`'s own "never a directory,
never a symlink, never a stranger's file" discipline, proved against a stranger file planted
directly in a project's own records directory
(`an_unrecognised_file_in_the_records_directory_is_skipped_and_left_alone`) and against a second
project's own records surviving the first's purge
(`purging_one_project_leaves_another_projects_records_untouched`).

`purging_a_projects_transcripts_also_purges_its_recovery_records` (`shell::tests`) proves the real
trigger end to end. Trust Settings gets a second line, `trust-settings-retained-recovery-records`,
beside the transcript figure -- not folded into it (two content types collapsed into one count is
itself the §4.1 pattern this project's own review history keeps finding).
`docs/src/users/local-data-and-privacy.md` gets its own section, and the sentence that said the
retained figure "counts transcripts only" is corrected to say a second figure exists beside it.

### The setting, default on (D15)

`[recovery] persist_unsaved_buffers`, `config/recovery.rs`, mirroring `config/explorer.rs`'s own
shape exactly except for the default direction -- `RecoverySettings` cannot derive `Default`
(derived `Default` on a `bool` gives `false`), so `Default` is implemented by hand to give `true`.
Forward-only, the same shape transcript capture's own decline already has: `false` stops new
records; it does not delete ones that already exist. `docs/src/users/configuration.md` gets its own
`[recovery]` table row and a `## Crash recovery` section, linking to the privacy page's own new
section.

### Measurement 3, the cadence (D7)

`editor_typing_latency_under_a_recovery_persist_tick`
(`crates/tekstide/src/shell/tests/editor_baseline.rs`), a new sibling to
`editor_typing_latency_under_a_watched_burst` reusing the same fixture, the same real-`update`
keystroke primitives and the same paired-round methodology -- not grafted into the watcher's own
condition set, since a periodic persist tick and an external file-change burst are different
mechanisms and intermixing them risked destabilising the existing, carefully-tuned RFC-026/RFC-065
measurement. A real `Message::RecoveryPersistTick` is delivered through the real `update`
whenever the interval has elapsed, driving the identical production code
(`persist_recovery_records`), not a stand-in.

Three conditions (idle/one dirty document/ten dirty documents, the fixture's own 100,000-line,
3.3 MiB text, the largest realistic document size), five rounds, orders rotated. **Corrected
twice**, both times before trusting the number, not after:

**First correction (before review 485).** The first pass reported total delivery over a fixed
keystroke budget, which conflates cost-per-tick with tick-*count* (a slower condition leaves less
wall-clock time per keystroke budget for later ticks to land in, so it fires more of them) -- the
same shape of error review 469 found in RFC-065's own D7 measurement. Fixed to report milliseconds
**per tick**.

**Second correction (review 485's own required item).** The per-tick fix produced
`median one-dirty-document cost: +0.001 ms/tick` and `median ten-document cost: +16.687 ms/tick`
-- a ratio of **16,687x for 10x the work**, which the published "per-document" figure (the
ten-document median divided by ten) silently assumed was linear. The code's own guard against
dividing by a near-zero one-document figure suppressed the ratio printout instead of surfacing it,
which is exactly backwards: a ratio that explodes **is** the finding. Fixed to publish the ratio
unconditionally (the same thing RFC-065 D7 already does, where it came back 9.2x against a 10x
expectation) and the per-round spread beside each median, re-run twice for a real number rather
than reasoned about:

```
median one-dirty-document cost over the five rounds: -0.030 ms/tick (spread -0.166 .. +0.248)
median 10-dirty-document cost over the five rounds: +20.442 ms/tick (spread +19.260 .. +23.810)
ratio (10-document cost / one-document cost): -671.9x -- 10x would match linear scaling
the one-document delta (-0.030 ms/tick, spread -0.166 .. +0.248) is at or below this harness's own resolution
per-document delivery cost, derived from the ten-document median alone (+20.442 / 10): 2.044 ms/tick
extrapolated cost at the open set's own bound of 20 documents, all dirty: 40.884 ms/tick (extrapolation, not measured, assumes the ten-document rate holds)
```

A first re-run (before fixing the suppression) gave ten-document +16.7 ms/tick and the original
run's own `+0.001`; this one gives +20.4 and `-0.030` -- both one-document spreads straddle zero
across two independent runs, the signature of round-to-round noise rather than a measured
per-document cost, on this measuring machine's own load. **The ten-document cost is real and
consistently an order of magnitude above idle across both runs (+16.7 to +20.4 ms/tick); the
one-document cost is not independently confirmed by this harness at all.** The per-document figure
and the twenty-document extrapolation both rest on the ten-document measurement alone, dividing by
ten and scaling by two -- an assumption of linearity, not a demonstration of it, stated as such
rather than dressed as a confirmed rate.

**This does not change the interval decision**, because that decision only ever needed the
worst-case number, which is the one real signal here: ten large documents, all dirty, cost
+16.7 to +20.4 ms/tick, a real cost comparable to the keystroke latency budget itself.
**`RECOVERY_PERSIST_INTERVAL = 2` seconds** keeps that measured worst case rare rather than
per-keystroke-adjacent -- double `RUN_RECORD_INTERVAL`'s own 1 second for a much heavier write --
regardless of what the true per-document rate turns out to be between one document and ten.

Reproduce: `CARGO_PROFILE_RELEASE_DEBUG_ASSERTIONS=true cargo test --release -p tekstide
editor_typing_latency_under_a_recovery_persist_tick -- --ignored --nocapture` (the release-mode
`()`-renderer requirement review 469 already found for the sibling measurement).

### Measurement 4, per-document cost at one and ten, twenty extrapolated and labelled (D8)

The same run above *is* this measurement -- D7's cadence question and D8's per-document question
are the same number, read two ways. D8 asks for the cost "at one and at ten dirty documents, with
twenty extrapolated and labelled as an extrapolation" (§4 row 14); this harness measures exactly
that shape, and review 485's own correction is precisely about not overstating what the
one-document half of it actually shows. The honest answer: **ten documents cost +16.7 to +20.4
ms/tick, measured directly; one document's own cost is below this harness's resolution, not
measured to a number worth publishing as one; twenty is an extrapolation from the ten-document
figure alone**, labelled as such in the test's own output and here.

### An unrelated file-content-read guard, found and fixed

`project::diff::tests::enumeration_confirms_only_the_closed_list_reads_full_file_content` failed
on the first full-workspace run after this slice's own files existed: `recovery/record.rs`'s
`read_recovery_record_file` (`file.read_to_end`) is a new full-file-content read, and this
project's own enumeration test requires every such call site to be named, deliberately, in
`FILES_ALLOWED_TO_READ_FULL_FILE_CONTENT`, not left to pass silently. Added, with a one-line
reason (the same shape `run_record.rs`'s own entry already has: a record's own small envelope,
read whole, never a path inside the project).

### Flake disclosed, not counted

One intermittent in the first full-workspace run:
`surface::terminal::tests::resize_makes_the_pty_the_emulator_and_the_render_path_agree` --
unrelated to this response (nothing it touches is anywhere near `surface::terminal`), already a
known, registered PTY-timing flake (`test-process-leak.md` row 787, now a third occurrence),
passed on immediate rerun in isolation. Recorded, gate redone rather than counted, per the
register's own convention.

## Gate, PR-027-B

- `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`: clean.
- `cargo test --doc --workspace`: `0 + 1 + 1`, 0 failed.
- **Three consecutive full-workspace runs, `--no-fail-fast`, fresh short `TMPDIR` each run**
  (`/dev/shm/g484{1,2,3}`): `744 + 16 + 1107` (+ `0+1+1` doctests), 0 failed, 6 ignored (the new
  measurement test among them), 0 fixture entries left in each run's own `TMPDIR` afterward.

## Review 485: the measurement's own ratio, suppressed instead of surfaced

Required item 1, fixed -- see "Measurement 3, the cadence (D7)" above for the full correction and
the re-measured numbers. Summary: the published "1.67 ms/document" figure was the ten-document
median divided by ten, silently assuming linearity the one-document measurement (median
`+0.001 ms/tick`) did not confirm -- the ratio between the two is 16,687x against a 10x
expectation, and the code's own near-zero guard suppressed the ratio printout rather than showing
it. Fixed to publish the ratio unconditionally (RFC-065 D7's own precedent) and the per-round
spread beside each median; re-run twice, both runs show the one-document spread straddling zero
(noise), and the ten-document cost consistently an order of magnitude above idle (+16.7 to
+20.4 ms/tick across the two runs). `RECOVERY_PERSIST_INTERVAL` is unchanged at 2 seconds: the
interval decision only ever depended on the real, worst-case ten-document number, not on the
per-document rate the one-document measurement cannot actually supply.

Required item 2 is **Amendment 1 to the RFC itself**, not a PR-027-B change: closing a window
cleanly removes the crash marker but leaves any recovery records behind, with nothing today (no
PR-027-C yet) that offers them back or removes them except a manual purge -- "edit, don't save,
quit" is exactly the case a user most wants their work back from, and it is not crash-scoped.
Nothing in this response changes as a result; the review's own ruling is that the offer (PR-027-C)
must be driven by the *presence* of records rather than gated by the marker, and that this RFC
must not reach a release with B shipped and C not.

### Gate, review 485's fix

- `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`: clean.
- **Three consecutive full-workspace runs, `--no-fail-fast`, fresh short `TMPDIR` each run**
  (`/dev/shm/g486{1,2,3}`): `744 + 16 + 1107` (+ `0+1+1` doctests), 0 failed, 0 fixture entries left
  in each run's own `TMPDIR` afterward.
