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

**Third correction (review 486's own two notes, neither required, both applied).** The caveat fired
on a proxy (`ratio.abs() > N * 5.0`) rather than the direct signal -- a moderately noisy run could
produce a plausible-looking ratio while still resting on one-document noise. Fixed to fire on the
direct signal: the one-document spread itself straddling zero. And, since `dirty_documents` was
already a loop parameter, a fourth condition was added -- **twenty dirty documents, the open set's
own bound (D4), measured directly rather than only extrapolated to**:

```
median one-dirty-document cost over the five rounds: +0.016 ms/tick (spread -0.040 .. +0.472)
median 10-dirty-document cost over the five rounds: +12.336 ms/tick (spread +11.746 .. +16.195)
median 20-dirty-document cost over the five rounds: +24.608 ms/tick (spread +23.841 .. +32.003)
ratio (10-document cost / one-document cost): 755.9x -- 10x would match linear scaling
ratio (20-document cost / one-document cost): 1508.0x -- 20x would match linear scaling
the one-document delta straddles zero -- at or below this harness's own resolution
per-document delivery cost, derived from the ten-document median alone (+12.336 / 10): 1.234 ms/tick
per-document delivery cost, derived from the twenty-document median alone (+24.608 / 20): 1.230 ms/tick -- measured directly at the open set's own bound, not extrapolated
```

**This is a materially stronger result than the extrapolation it replaces.** The ten-document and
twenty-document per-document rates agree to within 0.3% (1.234 vs 1.230 ms/document) -- two
independently measured points on the same line, not one point and an assumption. The one-document
signal is still noise (spread straddles zero, consistent with every prior run), so the rate is
confirmed **between ten and twenty**, not from one, and the twenty-document figure quoted from here
on is **measured, not extrapolated**: ~24.6 ms/tick at the open set's own bound, all dirty.

Reproduce: `CARGO_PROFILE_RELEASE_DEBUG_ASSERTIONS=true cargo test --release -p tekstide
editor_typing_latency_under_a_recovery_persist_tick -- --ignored --nocapture` (the release-mode
`()`-renderer requirement review 469 already found for the sibling measurement).

### Measurement 4, per-document cost at one and ten, twenty extrapolated and labelled (D8)

The same run above *is* this measurement -- D7's cadence question and D8's per-document question
are the same number, read two ways. D8 asks for the cost "at one and at ten dirty documents, with
twenty extrapolated and labelled as an extrapolation" (§4 row 14); review 486's own second note
asked for twenty to be measured directly instead, which this harness now does. The honest answer:
**ten and twenty documents both cost real, consistent amounts (~1.23 ms/document at both points,
~24.6 ms/tick at twenty, measured, not extrapolated); one document's own cost is below this
harness's resolution, not measured to a number worth publishing as one.** Twenty is no longer an
extrapolation at all -- review 486's own second note is why.

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

## Review 486: a `## 0.31.0` changelog section, and the measurement strengthened further

Required item, fixed: `CHANGELOG.md` gets a `## 0.31.0 - The Crash Is Detected, Not Guessed`
entry, written incrementally (RFC-065's own established shape), covering what PR-027-A and
PR-027-B actually ship -- the marker, the record, the purge, the figure, the setting -- with the
per-document cost stated carrying the same caveat it carries here, not as a bare number.

Both optional notes applied too (see "Measurement 3" above for the full third correction): the
caveat now fires on the direct signal (the one-document spread straddling zero) rather than a
proxy ratio threshold, and a twenty-document condition was added and measured directly at the open
set's own bound rather than only extrapolated to -- cheap, since `dirty_documents` was already a
loop parameter. The result is materially stronger than what it replaces: ten and twenty documents
now agree on a per-document rate to within 0.3% (1.234 vs 1.230 ms/document), two independently
measured points rather than one point and an assumption. `RECOVERY_PERSIST_INTERVAL`'s own doc
comment updated to match.

### Gate, review 486's fix

- `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`: clean.
- `cargo test --test rfc_docs_invariants`: 16 passed, 0 failed.
- **Three consecutive full-workspace runs, `--no-fail-fast`, fresh short `TMPDIR` each run**
  (`/dev/shm/g487{1,2,3}`): `744 + 16 + 1107` (+ `0+1+1` doctests), 0 failed, 0 fixture entries left
  in each run's own `TMPDIR` afterward.

## Review 487: the published figure was the lowest-load run of several

Required item. `CHANGELOG.md` stated "about 24.6 ms, measured" at the twenty-document bound as if
it were *the* cost, when it was the lowest of several runs taken across this whole measurement
campaign. The within-run comparison (ten against twenty, same run, same load) is sound and
unaffected; what was missing is the other axis -- the absolute cost moves with this measuring
machine's own load between runs, which `qa-evidence.md` had already said in words (line 332 of the
original write-up) without the published figure reflecting it.

**Every ten-document median recorded in this file, across every run of this harness, in order:**
`+16.687`, `+20.442` (both before the twenty-document condition existed), `+12.336`, and two more
runs taken for this response, `+12.276` and `+12.005` ms/tick -- range **12.0 to 20.4 ms/tick**,
about 1.7x between the lowest and highest observed.

**Every twenty-document median recorded, all three from the same harness version (measured
directly, not extrapolated):** `+24.608`, `+25.003`, `+24.171` ms/tick -- a tight range, **24.2 to
25.0 ms/tick**, taken back-to-back under what is apparently this machine's own current typical
load. These three do not by themselves carry the load variability the five-run ten-document series
shows, because all three happened close together under similar conditions.

**The honest combined figure uses both.** The per-document rate (~1.2 ms/document) is consistent
and well-confirmed between ten and twenty within any one run; what varies between runs is the
*absolute* cost, scaling together. Applying the highest ten-document rate actually observed
(`+20.442 / 10` = 2.044 ms/document) to twenty documents gives `40.9 ms/tick` -- already recorded
in this file at the time, just not carried into the twenty-document figure once that condition was
added. **Stated range for the changelog: 24–41 ms/tick at the open set's own bound**, not a single
"measured" number -- the low end is what three consecutive runs on this machine actually showed
just now; the high end is what this same machine showed earlier in the same campaign, under
whatever load it was under then. `RECOVERY_PERSIST_INTERVAL` is unaffected: 2 seconds was chosen to
keep this cost rare regardless of where in that range any one tick lands.

### Gate, review 487's fix

- `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`: clean.
- **Three consecutive full-workspace runs, `--no-fail-fast`, fresh short `TMPDIR` each run**
  (`/dev/shm/g488{1,2,3}`): `744 + 16 + 1107` (+ `0+1+1` doctests), 0 failed, 0 fixture entries left
  in each run's own `TMPDIR` afterward.

## Review 488: the changelog's own heading claimed both ends of the range were measured

Required item, a wording fix only -- the range itself (`24-41 ms/tick`) was already right, and no
code or number changed. `CHANGELOG.md`'s own paragraph opened "measured at both ends," which a
reader combines with "24 to 41 ms" into "both 24 and 41 were measured at twenty documents" -- false:
only `24.608`/`25.003`/`24.171` were ever measured at twenty. `41` is `20.4` (the worst *ten*-document
rate this campaign observed) carried to twenty at the confirmed ~1.2 ms/document rate, the same
derivation `qa-evidence.md` itself already stated in words. Reworded to say so explicitly, the same
"extrapolates to roughly X ms" clause `0.30.0`'s own changelog entry already uses for the identical
shape of claim.

### Gate, review 488's fix

- `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`: clean.
- **Three consecutive full-workspace runs, `--no-fail-fast`, fresh short `TMPDIR` each run**
  (`/dev/shm/g489{1,2,3}`): `744 + 16 + 1107` (+ `0+1+1` doctests), 0 failed, 0 fixture entries left
  in each run's own `TMPDIR` afterward.

## PR-027-C — the offer

### Provenance disclosure, before anything else

Part of this slice's own core scaffolding (`TextDocument::recover`, `ProjectContentWorkspace::
recover_text_document`, `RecoveryFileSnapshot::modified_at`) was first written by a research-only
fork I dispatched to answer three narrow questions about existing conflict-resolution and
document-insertion mechanics. It was explicitly told not to write any code; it exceeded that
directive and wrote directly to this shared working tree, disclosing the overstep itself when it
stopped. I did not take it on trust: I read every line, ran the gate myself (`cargo clippy` failed
on first run -- `TextDocument::recover` had 8 positional arguments, over the lint's limit, proving
the fork's own claim that nothing had been gated), fixed the gate failure by bundling the
record-derived fields into a new `RecoveredBufferInit` struct rather than stacking a second
`#[allow(clippy::too_many_arguments)]`, and wrote the first tests either of these functions had.
From that point on the design and every line after it is mine, reviewed and tested the same as any
other code in this slice -- recorded here because the standing instruction is to disclose
deviations honestly, and an unauthorized write to a tree other agents may also touch is one.

### The trigger: presence of records, never the marker (Amendment 1)

`offer_recovery_for_opened_project` (`shell.rs`): reads `recovery::read_project_recovery_records`
for the project that just became open and, if non-empty, opens `ModalContent::RecoveryOffer` --
gated on `state.modal.is_some()` (the same discipline every other modal-opening function here
already uses), never on whether an `InstanceMarker` exists or what it says. Called at each of the
three mid-session project-open sites (`reopen_recent_project`, `attempt_open_project_from_path_field`,
`choose_current_browsed_directory`) -- the same duplicated-per-site shape
`trigger_git_summary_refresh`/`load_earlier_transcripts_for_opened_project` already use, for the
documented reason there is no single point every newly-opened project passes through -- and once,
inside `State::new` (`offer_recovery_for_open_projects`), for the CLI-argument path, mirroring
RFC-049 D2's own precedent (`run_transcript_retention_for_open_projects`/
`reconcile_project_watches_for_open_projects`): a command-line project is open before `State`
exists, so its own offer has to run from the one place every boot-time project is already in hand,
not a `main.rs` call site a future edit could delete with nothing noticing.

Proof that the marker plays no gating role: every `shell::tests` fixture that exercises the offer
constructs `State` via `state_with`, which calls `State::new(..., None)` -- no marker is ever
constructed or passed anywhere in that test file. The offer still opens. This is proof by
construction, not a dedicated "no marker" test, since there was nothing to construct a marker from
even if one were wanted.

**Disclosed scope decision, not dictated by the amendment's own text:** the trigger is
**per-project**, fired when that specific project is opened, not a single cross-project scan at
boot listing every project anywhere with records. The task-breakdown's own wording ("on restart...
shown what can be recovered, per project and path") is compatible with either reading. A true
cross-project boot scan would need to open or at least name a project the user has not yet
navigated to, which nothing else in this product does today, and D5/D4's own two measurements
(list + decline) hold identically either way. If the architect wants the global scan instead, the
per-project trigger is this slice's own judgment call to revisit, not a defect to find later.

**Disclosed scope cut:** the marker's own remaining role under Amendment 1 -- "may colour the
offer's own wording" (crash vs. ordinary-quit framing) -- is **not implemented**. The offer's copy
(`recovery-offer-title`/`recovery-offer-row`) does not distinguish the two. Amendment 1 says the
marker *may* colour the wording, not that it must, and nothing in D4 or measurement 1 depends on
it; recorded here as a deliberate cut so it is not mistaken for an oversight.

### The disk comparison: D5's three-way split, without reusing `FileSnapshot` equality directly

`TextDocument::recover` (`content/document.rs`) resolves the file at the record's own relative
path and classifies it exactly like the task-breakdown's own three words -- unchanged, changed,
gone -- but **does not** call the existing `refresh_external_state` unmodified against a freshly
constructed document, because that method's own equality check compares whole `FileSnapshot`s
including `content_hash`, a field `RecoveryFileSnapshot` was deliberately built without (see
PR-027-B's own doc on that type). A `None` read against a fresh `Some(_)` hash would report
"changed" even when the file is byte-identical, breaking the easiest and most common case. Instead,
`recover` compares only `modified_at`/`len` -- the same two fields `TextDocument::save`'s own
external-change check already treats as what changed -- and reuses the **existing state
vocabulary** for the result: unchanged restores `Dirty` (never `Clean`: a recovered buffer is
always local work the file does not have); changed or gone both restore `Conflict`, the same state
`record_external_change` already gives a dirty document whose file moved under it. No fourth state
exists anywhere in this slice's own code. "Gone" is told apart from "changed" the same way it
already is for a live, already-open document: a `Conflict`-state document whose
`target().canonical_path` no longer exists is `ExternalDeleted`, the exact check
`refresh_document_by_canonical_path` already performs.

`recover_with_unchanged_disk_file_restores_dirty_with_the_recorded_text`,
`recover_with_changed_disk_file_restores_as_conflict`,
`recover_with_missing_disk_file_restores_as_conflict` (`content::tests::recover`) prove the
three-way split directly, on real files, with no shell or project session involved.

### A found-and-fixed defect: the dedup switch silently discarded the conflict this slice just found

`recover_text_document` deliberately inserts into the open set without activating it (RFC-065 D1/
D2's own distinction between the open set and what is active); the caller makes it visible by
calling the ordinary `open_text_document` on the same path afterward, which hits that method's
existing dedup-switch (a path already open switches to it, no disk read) -- the literal reuse D5
asks for, and the one piece of forward-compatibility RFC-067 D8 cares about (the mode-switch this
slice inherits by chaining into `open_text_document` rather than copying its logic).

The dedup-switch, however, sets `ProjectContentWorkspace::status` to a plain `Opened`
**unconditionally** -- correct for every other caller, which is always switching to a document
nothing just found divergent, but wrong here: it silently overwrote the `Conflict`/`ExternalDeleted`
status the chrome's own Reload control (`editor::reload_button_is_shown`) reads, for exactly the
"changed"/"gone" rows this slice's own checklist requires to go through "the existing
`ExternalChanged`/conflict path." Found by writing
`activating_a_row_whose_file_has_changed_surfaces_the_reload_control` *before* trusting the chain
worked, not by assuming reuse of `open_text_document` was enough on its own -- the test failed
against the first version of this code (status read back as `Opened`), then passed once fixed.

Fixed in two parts, not one, because the first alone is not sufficient: (1) `recover`'s own
"changed" branch no longer stores the fresh disk read it just took as `last_known_snapshot` --
doing so would make a *later* refresh compare a read against itself and report `Unchanged`,
erasing the very divergence just found. It stores a snapshot with `content_hash: None` instead,
which a later fresh read (almost always `Some(_)`) can never equal, guaranteeing the divergence
stays visible. Proved directly: `recovering_a_changed_file_still_reports_changed_on_the_next_refresh`
(`content::tests::recover`) constructs the document, calls `refresh_external_state` on it exactly
as `recover_highlighted_offer_item` does and asserts `ExternalChangeDecision::Conflict` comes back,
not `Unchanged`. (2) `recover_highlighted_offer_item` (`shell.rs`) calls
`ProjectSession::refresh_active_text_document` immediately after `open_text_document` -- the
existing machinery that actually re-derives `ProjectContentStatus` from the now-correctly-mismatching
snapshot, through the identical mapping `refresh_document_by_canonical_path` already uses for a
live document's own external change. Proved end-to-end, through the real `Message::ModalActivate`
handler, by `activating_a_row_whose_file_has_changed_surfaces_the_reload_control` (status `Conflict`)
and `activating_a_row_whose_file_is_gone_surfaces_external_deleted` (status `ExternalDeleted`),
both in `shell::tests`.

### Measurement 1 — the offer, and declining

The offer lists every record for the project that just opened (project id + each record's own
relative path), proved by `a_project_with_recovery_records_is_offered_at_state_construction`
(`shell::tests`): writes a real record against a real file via the real `write_recovery_record`,
then constructs `State` and reads the resulting `RecoveryOfferModal`'s own `items` back.
`a_project_with_no_recovery_records_opens_no_modal` is the negative control.

Declining (`Message::ModalDismiss`, Escape) leaves every file and every record exactly as it was:
`dismissing_the_offer_leaves_every_record_on_disk_untouched` asserts both the record (via
`read_project_recovery_records`) and the real file's own bytes (`fs::read_to_string`) are unchanged
after dismissing.

### Measurement 2 — the real round trip

D6 requires the durability half proven against a real `SIGKILL`, not a `simulate_crash()` helper.
`a_real_sigkill_leaves_the_crashed_instances_own_recovery_record_intact`
(`recovery::tests`) extends PR-027-A's own established shape
(`a_real_sigkill_leaves_a_marker_a_later_startup_detects_as_a_crash`): a real `sleep` process
stands in for the crashed instance (the marker and the record are written in the test's own
process, since the stand-in process never ran Tekstide's own code -- the same reasoning that
test's own doc comment gives for the marker alone), genuinely `kill()`ed (`SIGKILL` on Unix) and
reaped, then the real `start_instance` and `read_project_recovery_records` are called against the
same root. Both the crash detection and the record -- byte-for-byte, via `assert_eq!` against the
record that was written -- survive the real kill intact.

The disk-comparison half of the round trip (what "recover" decides from unchanged/changed/gone)
needs no process at all to exercise correctly, and is proved directly in
`content::tests::recover` above against real files, not re-proved here with an unnecessary
process in the way.

### Gate, PR-027-C (so far)

- `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`: clean.
- `cargo test --doc --workspace`: clean (unchanged from PR-027-B's own run).
- **Three consecutive full-workspace runs, `--no-fail-fast`, fresh short `TMPDIR` each run**
  (`/dev/shm/t27cfinal{1,2,3}`): `750 + 16 + 1115` (+ `0+1+1` doctests), 0 failed, 0 fixture entries
  left in each run's own `TMPDIR` afterward.
- Not yet done: the live capture (changed-on-disk case), and the "no undo history" notice's own
  visibility proof -- both left for the next response, not silently assumed.

## Review 490: a recovered oversize file is saved over -- the conflict is set and never consulted

Required item 1, a real data-loss bug, demonstrated by the reviewer against the real code rather
than reasoned about: `recover`'s own "changed" branch stores `last_known_snapshot` with
`content_hash: None` on the stated assumption that a later fresh read is `Some(_)` "whenever the
file is within the policy's editable bound" -- true for every file this slice's own tests had
tried, and false for any file over `DEFAULT_MAX_EDITABLE_BYTES` (4 MiB), where a fresh read is
`content_hash: None` too (`file_snapshot_for_current_disk`'s own shape). The two `None`s compared
equal, `save`'s own guard (`current_snapshot != self.last_known_snapshot`) found no divergence, and
a tiny recovered buffer silently replaced a real 5 MiB file. Only recovery can reach this: the
ordinary open path refuses an oversize file outright (`TooLarge`), so an ordinary document's own
`last_known_snapshot` always carries a real hash; `recover` never reads the file's content at all,
which is exactly what let it admit a document for a file that could never have been opened by hand.

**Fixed structurally, per the review's own framing** ("the property required, not the
implementation... guarding `save` on the `Conflict` state does not depend on how a snapshot encodes
absence"), not by trying to encode "always different" more cleverly:

- `TextDocument::save` now refuses unconditionally while `self.state == Conflict`, checked as the
  very first thing, before any disk read at all -- no snapshot shape, at any file size, can bypass
  it. For an ordinary (non-recovered) document this is a no-op (the snapshot guard below it would
  already have blocked the save), so nothing about today's non-recovery behaviour changes.
- `TextDocument::refresh_external_state` carries the identical guard on its own `Unchanged` branch.
  Not required by the review (it is a display bug, not a data-loss one), but the same root cause: an
  oversize `Conflict` document's later refresh would otherwise report `Unchanged` too, and
  `refresh_document_by_canonical_path`'s own mapping reads that as `Edited`, hiding the Reload
  control for exactly the document that most needs it. Disclosed and fixed alongside the required
  item rather than left as a known residual gap, since the fix was one guard, not a redesign.

**Tested against the exact assumption the removed comment stated**, per the review's own
instruction ("every stated assumption in it is a test case"): both new tests build a real file over
the 4 MiB cap on disk, recover a record whose own length does not match it (D5's "changed" case),
and assert against the real result --
`recovering_an_oversize_changed_file_refuses_to_save_over_it` (save refused, `TextDocumentSaveError
::ExternalChange`, and the real file's own byte length on disk unchanged afterward) and
`refreshing_an_oversize_conflict_document_stays_conflict` (`ExternalChangeDecision::Conflict`, not
`Unchanged`) (`content::tests::recover`).

**Ablated, not merely passing**: `rfcs/handoffs/ablate.sh` against the `save` guard alone, on a
clean tree (commit `c07d7ff`) -- `recovering_an_oversize_changed_file_refuses_to_save_over_it` fails
without it (`Saved`, not an error), confirming the test is load-bearing, not accidentally green.

### Gate, review 490's fix

- `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`: clean.
- `cargo test --doc --workspace`: clean.
- **Three consecutive full-workspace runs, `--no-fail-fast`, fresh short `TMPDIR` each run**
  (`/dev/shm/t27r490g{1,2,3}`): `750 + 16 + 1117`, 0 failed, 0 fixture entries left each time.
- Not yet done, exactly as review 490's own remaining required items say: the "no undo history"
  notice's own visibility proof, and the live capture (held off at the owner's instruction).

## Review 491: the last required item before the live capture -- proving the notice renders

Required item 2 (review 490's own numbering carried forward; review 491 named it unchanged). The
i18n enforcement suite proves `recovery-offer-no-undo-notice` resolves to real text against the
real `en.ftl`; it never proves any view call site actually places that resolved text where the
user sees it, the same gap `qa-evidence.md` itself already disclosed rather than assumed closed.

Fixed the same way `external_change_dialog_body`/`paste_preview` already let their own dialogs be
tested: `recovery_offer_header_lines(catalog, project_display_name) -> [String; 2]` is factored out
of `recovery_offer_modal_view`, returning the exact two header lines the view pushes, in the same
order, with no condition between computing them and pushing them. A test resolves a **real**
`Catalog` against the shipped `en.ftl` (not a fixture string) and asserts the second line mentions
undo -- `recovery_offer_header_includes_the_no_undo_history_notice` (`shell::tests`).

**Ablated**: replacing the real catalog lookup with unrelated text (`review-491-notice-renders`)
fails the test. The first ablation attempt accidentally proved nothing -- the replacement text
("no mention of undo here") still contained the substring `"undo"`, so the test's own `contains`
check passed anyway; retried with text that genuinely has no occurrence of the word, which failed
as expected. Recorded here because it is exactly the kind of accidentally-green ablation
`ablation-summary-grep-must-include-errors`'s own lesson warns about, just one level up (a
substring match standing in for a compile error).

### Gate, review 491's fix

- `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`: clean.
- `cargo test --doc --workspace`: clean.
- **Three consecutive full-workspace runs, `--no-fail-fast`, fresh short `TMPDIR` each run**
  (`/dev/shm/t27r492g{1,2,3}`): `751 + 16 + 1117`, 0 failed, 0 fixture entries left each time.
- The only item left for PR-027-C to close: **the live capture**, held off at the owner's own
  instruction about the shared desktop.

## The live capture: review 490's own last two required items, closed together

Review 492's own finding sharpened what this capture had to prove: ablating the view's own push of
the no-undo-history line (not the factored-out function) left `recovery_offer_header_includes_the_
no_undo_history_notice` passing anyway, which is the gap every line-function test in this codebase
has against an `iced` view tree. The live capture is the only thing that closes it -- and, per the
task-breakdown's own requirement, had to be the changed-on-disk case, not the easy unchanged one.

Taken on the owner's own go-ahead, against the real binary, with a real `kill -9` (not
`simulate_crash()`) and a real external edit to the file while Tekstide was down -- full method and
all three images in `evidence/pr-027-c/README.md`. In order: the offer opens on restart with the
no-undo-history notice **on screen**, not only resolved in the catalog; accepting the row updates
its own text to "recovered (the file on disk has since changed)"; switching into the project shows
`notes.txt (conflict)` with a real **Reload** button in the chrome -- the exact control review 490
found missing for this exact scenario, now visibly present, holding the recovered text rather than
the file's own real, changed content.

A confirming detail recorded in the evidence README rather than left unexplained: the recovery
record for the recovered document was not gone afterward -- a *new* one existed, timestamped to the
external edit, because the recovered buffer is itself still dirty and the ordinary persist tick
protects it again, exactly as D11 says it should (a record is removed when *its own* reason ends,
and "still unsaved" is a reason that has not ended).

The real environment was left as found: the throwaway project's entry was removed from the real
`recent-projects.json` by hand (backed up first, then restored to 19 entries from 20), its
recovery record directory and both instance markers removed, and the fixture directory deleted.
None of this -- the fixture path, or any other project's own path shown in the Project Board behind
the modal -- is under `$HOME`; every path in every captured image reads `/tmp/...`.

### Gate, the live capture

No code changed for this response -- evidence only. The gate already reported above (review 491's
fix) is the current one: `751 + 16 + 1117`, 0 failed, three consecutive runs, `cargo fmt --check`/
`clippy --workspace --all-targets -D warnings`/`cargo test --doc --workspace` all clean.

**With this, every item review 490 and 492 named is closed.**

## Whole-RFC

**The two hedged requirements, and what their hedges were decided to mean.** `rfcs/delivery-plan.md`:
the "Session recovery" row now carries `REQ-RECOVER-002`/`005` alongside `001`/`003`/`004`, with
what each hedge was decided to mean rather than repeated back -- "where safe" (`002`) is D5's own
three-way disk comparison, never assumed; "where technically feasible" (`005`) is D6's own real-kill
proof, never `simulate_crash()`. The "Crash / unsaved buffer recovery" row (the "Not implemented"
table) is annotated rather than left stale -- which also surfaced that this row never carried `002`
at all since the plan was first written, a gap in the plan itself, disclosed rather than quietly
folded in as if it had always been there.

**The colour-alone, i18n completeness and internal-identifier scans.** RFC-027 adds no new
colour-coded state -- the offer's own rows use a text `>` highlight marker and plain outcome text,
never colour alone. The two concrete scans, `no_catalog_string_names_an_internal_identifier` and
`every_source_locale_key_resolves_in_every_shipped_locale` (`crates/tekstide/src/i18n/
enforcement.rs`), pass directly as part of the gate below, alongside the other six tests in that
module -- no separate invocation exists for either, the same as every prior slice in this RFC.

**The changelog, re-read against the finished set.** `## 0.31.0`'s own Status line still said the
live capture and the notice-visibility proof were outstanding -- both closed by review 494 and
review 492 respectively, before this re-read. Fixed, and reworded to say *why* the entry is written
incrementally rather than only restate that it is -- the RFC-065 review 479 lesson, named so it is
not quietly skipped the way this project's own history shows it can be. The four feature paragraphs
(the marker, the record, the purge/setting, the cost measurement, the offer) were each re-checked
against the fully finished behaviour, including the review-490 save-guard fix -- all four still read
correctly; nothing needed changing beyond the Status line.

**The book, read against the changelog in both directions.** Forward (changelog to book): a real
gap, not merely a staleness -- `what-works-today.md`'s "Projects, files, and editing" section
described every other document-state surface this RFC's own commits touch (external change,
conflict, deleted-on-disk, multi-document) but never mentioned the recovery offer at all. Added, in
the same place and the same voice as the surrounding paragraphs, cross-referencing the privacy page
the way every other retained-content feature on that page already does. Reverse (book to changelog):
a real staleness, the exact shape review 472 found at RFC-065 -- a capability's own meaning widened
(there, `[open]` from one document to a set; here, the offer from crash-only to record-presence) and
the sentence describing it kept the old meaning. `local-data-and-privacy.md` opened its own recovery
section "Tekstide can detect that it did not exit cleanly, and offer back what was unsaved **when it
crashed**" -- true when only PR-027-A existed, false since Amendment 1 (review 485): the offer is
driven by the record's own presence, never by whether a crash was detected. Fixed, naming the
amendment's own mechanism (a clean quit removes the marker and leaves the records) rather than just
asserting the corrected claim. The same page's "RFC-027... under `rfcs/accepted/` while it is still
being implemented" line was also stale (every slice is done; the RFC itself has not moved to
`rfcs/done/`, which is the release-cut step, not this one) and corrected to say so precisely, not by
claiming "closed" ahead of that step. Every other `recovery-*`/`editor-recovery-*`/
`trust-settings-retained-recovery-records` Fluent key's own wording was checked against both pages
and already agreed; none needed a change.

**Flakes.** None from this response's own three runs. The one review 494 found in its own run
(`runtime::terminal::reader::tests::drain_available_never_blocks_the_caller_even_under_sustained_
production`) is already registered at "New row, 2026-10-09 -- review 494" in `test-process-leak.md`
(`803dd0a`), not mine to add a second time.

**Core pin and commits.** Not a release cut: `tekstide-core`'s own pin does not move until the
candidate names a new version for it to move to -- a release-step item, explicitly left for that
step by the checklist's own wording. This response's own commits are pushed once its own gate is
green, the same as every prior one.

## Gate, Whole-RFC

- `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`: clean.
- `git diff --cached --check` after staging: clean.
- `cargo test --test rfc_docs_invariants`: 16 passed, 0 failed.
- `cargo test --doc --workspace`: clean.
- **Three consecutive full-workspace runs, `--no-fail-fast`, fresh short `TMPDIR` each run**
  (`/dev/shm/g4w{1,2,3}`): `751 + 16 + 1117`, 0 failed, 0 fixture entries left each time. (A first
  attempt under a longer literal, `/dev/shm/t27wholerfc{1,2,3}`, hit two real, deterministic
  `SocketPathTooLong` failures in `approval::tests::reference_adapter` -- the exact reason this
  project's own convention insists on a *short* fixed literal, not merely a fixed one. Re-run under
  a short literal, both pass; not a flake, not registered.)

## Review 495: three user-visible strings still said "against a crash"

Required item, found because the reverse book-vs-changelog read in the Whole-RFC response above
stopped at the documents -- it corrected `local-data-and-privacy.md`'s own "when it crashed"
framing but never ran the same check against `en.ftl`, the place a user actually reads the words.
`editor-recovery-persist-refusal-too-large`/`-total-bound`/`-io` (`crates/tekstide/locales/en.ftl`,
rendered through `surface/editor.rs`, so genuinely on screen) all scoped the refusal to "a crash".
After Amendment 1 a record protects unsaved work across an ordinary quit too -- the common case,
not the rare one -- so scoping the warning to "a crash" told the user less than was true about what
a refusal costs them.

Fixed by dropping the scoping clause rather than widening it to name every trigger: "too large to
protect"/"not protected"/"could not be protected", none of them naming a cause, since the record's
own job is to protect the edit regardless of *why* Tekstide stops.

**A fourth instance, found by applying the reviewer's own concrete instruction** ("grep the words
whose meaning changed in `en.ftl`") **one level further than asked** -- `grep "crash"` across the
whole `docs/src/users/` tree, not only `en.ftl`: `configuration.md`'s own example config block had
`persist_unsaved_buffers = true # protect dirty documents against a crash; default true`, the
identical understatement, in a page already touched by this same response's own earlier edits and
still missed on the first pass. Fixed the same way: "protect dirty documents from being lost",
naming no cause.

Nothing else matched `crash` as a live, user-reachable string after this pass: the remaining
occurrences in `en.ftl` are comments, and every other hit in `CHANGELOG.md`/the book is either
correctly scoped to the marker/detection mechanism specifically (which genuinely is crash-only by
design, D1) or already names both triggers (`what-works-today.md`'s own new paragraph).

### Gate, review 495's fix

- `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`: clean.
- `i18n::enforcement` (8/8), `rfc_docs_invariants` (16/16),
  `surface::editor::tests::recovery_persist_refusal_lines_names_each_path_and_reason`: all pass --
  none assert the refusal lines' exact prose, so the wording change needed no test change.
- `cargo test --doc --workspace`: clean.
- **Three consecutive full-workspace runs, `--no-fail-fast`, fresh short `TMPDIR` each run**
  (`/dev/shm/g5r{1,2,3}`): `751 + 16 + 1117`, 0 failed, 0 fixture entries left each time.

## Review 496: dropping the trigger must not also drop the object

Required item, and the reviewer's own share of it stated plainly: asking for "against a crash" to
go without also asking the *object* to stay invited the overcorrection. `could not be protected`
names no object at all -- a user reading it on its own, between the save-all notice and the Save
button with nothing else establishing the subject, cannot tell what was not protected, by what, or
from what.

Fixed by naming the thing instead of the trigger -- "too large to **keep a recovery copy**", "no
**recovery copy** kept", "could not **keep a recovery copy**" -- accurate about scope (no cause
named, so no scope to misstate) and comprehensible alone (the object is in the sentence itself, not
only implied by a neighbouring line).

### Gate, review 496's fix

- `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`: clean.
- `i18n::enforcement` (8/8), `rfc_docs_invariants` (16/16), and the editor refusal-line test: all
  pass unchanged -- still no test asserts the lines' exact prose (the RFC-063 gap review 492
  recorded; not closed here, per the reviewer's own note that this is not required).
- `cargo test --doc --workspace`: clean.
- **Three consecutive full-workspace runs** (`/dev/shm/g6r{1,2,3}`) was reported here as satisfying
  the gate with `751 + 16 + 1117` "in two of three runs" -- **wrong, per review 497**: the rule is
  three consecutive green runs, not three attempts, and every earlier response in this RFC (reviews
  478, 485, 495) redid the trio rather than counting a failed one. Corrected below, not left
  standing next to its own correction.

## Review 497: three consecutive green, not three attempts

Required item. The miscount above is mine: a run that failed on an already-registered, unrelated
intermittent (the review-478 PTY pair) was reported as "2 of 3" rather than redone, the exact thing
this RFC's own gate convention already names and three earlier responses in it already followed
correctly. The reviewer's own independent run found a *second*, different registered row fail on
their machine's own first attempt
(`closing_a_project_with_a_backgrounded_descendant_kills_it_through_a_real_close`) -- two
independent three-run gates, two different intermittents, confirming this is not specific to either
machine and, by the reviewer's own count across reviews 494-497, lands a three-run gate on its first
attempt only about half the time. Not a reason to count a failed run; a reason to expect to redo the
trio, which is what the convention already says.

No code changed for this response. Re-ran the trio clean on the first attempt:
**`751 + 16 + 1117`, 0 failed, 0 fixture entries left, three consecutive runs**
(`/dev/shm/g7a1r{1,2,3}`).

**Noted for later, not acted on here**: the reviewer has scheduled a disposition pass over the whole
`test-process-leak.md` register for after `0.31.0` ships, with my name against it -- written up at
"Disposition, 2026-10-09" in that file. Not a Whole-RFC item and not started now, per the reviewer's
own explicit "not before the release".

### Gate, review 497's fix

- No code changed; evidence only.
- **Three consecutive full-workspace runs, `--no-fail-fast`, fresh short `TMPDIR` each run**
  (`/dev/shm/g7a1r{1,2,3}`): `751 + 16 + 1117`, 0 failed, 0 fixture entries left each time -- clean
  on the first attempt at the trio.

**With this, review 496's required item closes.**
