# RFC-027 acceptance and QA checklist

Tick a box only when the thing it names has been **run**, not reasoned about. Each slice's boxes are
its own; the Whole-RFC section is checked once at the end.

## PR-027-A — the marker

- [x] A real `SIGKILL` of a real process leaves a marker whose pid is not alive; a clean exit leaves
      none. Not a `simulate_crash()` helper (D6, §4 row 15). `qa-evidence.md`:
      `a_real_sigkill_leaves_a_marker_a_later_startup_detects_as_a_crash`,
      `a_clean_exit_leaves_no_marker_behind`.
- [x] **Two concurrent instances do not make each other look crashed** (D12). Start a second while
      the first runs; neither reports a crash. `a_concurrent_sibling_instance_is_not_reported_as_a_crash`.
- [x] The liveness check is production code. `test_support` is not imported by the product.
      `recovery::instance::pid_is_alive`, not `#[cfg(test)]`.
- [x] Pid reuse is disclosed in the write-up, with its direction of failure named. `qa-evidence.md`'s
      own "Pid reuse is disclosed, not fixed" section; direction: not offered, never falsely offered.
- [x] Nothing user-facing, and no buffer content written anywhere.
      `a_first_run_detects_nothing_and_writes_only_its_own_marker` asserts the only file on disk
      is the marker itself.

### Required at review 481

The slice is correct and I verified the whole of it against the real binary, including the thing
you disclosed as unverified. One required item, and it is required because of what PR-027-B does
next, not because of anything it costs today.

- [ ] **Validate the pid parsed from a marker filename before casting it.** `parse::<u32>()` feeds
  `pid as libc::pid_t` unbounded. I planted three markers and ran the real binary: `999999999` was
  detected and removed correctly, but **`0` and `4294967295` survived the scan and always will** —
  `kill(0, 0)` probes the caller's own process group, and `4294967295` casts to `-1`, which probes
  every process the caller may signal. Both answer "alive", so neither marker can ever be cleaned.
  Today that is two stray files and it fails in the safe direction. **It stops being harmless in
  PR-027-B**: a record whose lifecycle is tied to a marker that can never be removed is user content
  that outlives its reason, which is §2 row 7 of the risk document. Reject `0` and anything above
  `i32::MAX` at parse time, so a marker is always either live or removable.

**Verified live against `target/debug/tekstide`, not from the report** — these are not asks, they
are what I ran:

- The marker is created named by the app's own pid; a real `SIGKILL` leaves it behind.
- A restart detects it, names the pid on stderr (*"the previous session (pid 2635125) did not exit
  cleanly"*) and removes it.
- **A real window close through the window manager drops `State` and removes the marker.** Your
  reading of `iced_winit` 0.14.1 was right, and it is no longer a structural claim — see below.
- Two concurrent real instances hold both markers; the second reports nothing and does not touch
  the first's.
- The only files under the state directory are `recent-projects.json`, its `.bak`, and the markers.
  **No buffer content is written anywhere**, checked on the filesystem rather than from the code.
- `test_support` is `#[cfg(test)]` at `lib.rs:31`, so production genuinely cannot reach it; the
  three remaining mentions in product source are doc comments.
- Gate reproduces: `739 + 16 + 1095`, 0 failures, 0 fixture entries left.

### Required at review 482

The fix itself is right: the range check sits at the parse, so no filename-derived value reaches
`libc::kill`'s cast. The gate reproduces (`739 + 16 + 1096`, 0 failures, 0 fixture entries). One
required item, and one correction to what I asked for at review 481.

- [ ] **The new test passes with the fix removed, so it is not evidence for it.** I ablated the
  `.filter(|pid| (1..=i32::MAX as u32).contains(pid))` line and ran
  `an_out_of_range_marker_filename_is_neither_a_crash_nor_a_live_sibling`: **it still passes.** It
  cannot do otherwise — `InstanceStartup` carries only `detected_crashes`, and `0` and `u32::MAX`
  produce no detected crash either way (pre-fix `kill` answers "alive"; post-fix the filter rejects
  them), leaving the file on disk in both cases. The observable outcome is identical, so no test
  over `detected_crashes` can distinguish the two. **Test the property the fix actually
  establishes**: lift the filename→pid step into its own named function and assert it directly —
  `"0"` and `"4294967295"` yield nothing, `"999999999"` yields `999999999`. That test fails when the
  filter goes.

**My correction, from review 481.** I justified the item with "so a marker is always either live or
removable". That was wrong, and this fix is right not to deliver it: `0` and `u32::MAX` stay on disk
forever, and they should. `local-data-and-privacy.md:112` already states the product's rule — **"a
file Tekstide did not write is never deleted"** — and a filename this module does not recognise is,
by definition, not a file we wrote. Leaving it untouched is the existing policy, not a gap in the
fix. I should have cited that line at 481 instead of inventing a property that contradicts it.

- [ ] **Carry the real constraint into PR-027-B instead.** What actually matters once records exist
  is narrower than what I wrote: **a recovery record's own cleanup must never depend on a marker
  filename the module does not recognise.** An ignored marker is fine; an ignored marker that strands
  a record of user content is §2 row 7. Prove it when records land.

### Closed at review 483 — PR-027-A accepted

Nothing required. The test now tests the property, and I confirmed it rather than taking the
report: ablating the range filter with `ablate.sh` fails
`marker_filename_to_pid_rejects_zero_and_anything_past_i32_max` immediately
(`left: Some(0), right: None`), and the tree restores clean. Gate reproduces on my own run:
`739 + 16 + 1096`, `0+1+1` doctests, 0 failures, 0 fixture entries left.

**Removing the old end-to-end test rather than keeping it alongside was the right call**, and worth
recording as a judgement rather than a tidy-up: a test that passes either way, sitting next to one
that does not, reads to the next person as two proofs where there is one. Deleting it is the honest
state.

Everything PR-027-A set out to prove is proved, most of it against the real binary at review 481:
the marker is created by pid and removed by a clean close through the window manager, a real
`SIGKILL` leaves it, a restart detects and names it, two concurrent instances leave each other
alone, and no buffer content is written anywhere.

**PR-027-B is next**, and carries review 482's forward constraint: a record's own cleanup must never
depend on a marker filename this module does not recognise.

## PR-027-B — the record, with its purge

- [x] A **dirty** document gets a record; a **clean** one does not (D2, §3 row 12). Proved by what
      is on disk, not by a count the code reports about itself.
      `a_dirty_document_gets_a_recovery_record_and_saving_removes_it`,
      `undoing_back_to_clean_removes_the_stale_record_on_the_next_tick` (`shell::tests`).
- [x] Records are `0600` in a `0700` directory (§2 row 6), checked by reading the mode.
      `a_written_record_has_the_right_permissions_and_reads_back_exactly` (`recovery::tests`).
- [x] No buffer content reaches the audit store (D13, §2 row 5). The record is a file under
      `recovery/records/`, never a row anywhere the audit coordinator writes — nothing in this
      slice's own code path touches `audit::AuditCoordinator` at all.
- [x] **Measurement 3 — the cadence**, chosen against `editor_baseline.rs`'s paired harness with the
      control carried inside the same run (D7). The window is justified by the number, not the
      number by the window. `qa-evidence.md`'s own "Measurement 3" section:
      `editor_typing_latency_under_a_recovery_persist_tick`.
- [x] **Measurement 4 — per-document cost** at one and at ten dirty documents, with twenty
      extrapolated and **labelled as an extrapolation** (D8, §4 row 14). Same run as measurement 3;
      `qa-evidence.md`'s own "Measurement 4" section.
- [x] **Measurement 5 — the bound** refuses a too-large buffer and names the buffer and the limit
      (D9, §1 row 4). `a_record_over_the_per_record_bound_is_refused_and_nothing_is_written`,
      `a_record_over_the_total_bound_is_refused_and_nothing_new_is_written` (`recovery::tests`);
      named to the user live, `recovery_persist_refusal_lines_names_each_path_and_reason`
      (`surface::editor::tests`).
- [x] **Measurement 6 — the record is gone** after a save, and after a close (D11, §2 row 7).
      `a_dirty_document_gets_a_recovery_record_and_saving_removes_it`,
      `closing_a_project_removes_its_recovery_records` (`shell::tests`) — see `qa-evidence.md`'s own
      disclosed finding about what "close" can and cannot mean for a genuinely dirty document in
      this product today.
- [x] The per-project purge removes recovery records; Trust Settings' *Retained locally* figure
      counts them (D10, D14, §2 row 8).
      `purging_a_projects_transcripts_also_purges_its_recovery_records` (`shell::tests`); a second,
      separate Fluent line (`trust-settings-retained-recovery-records`), not folded into the
      transcript figure.
- [x] `local-data-and-privacy.md` has its section, **and the sentence saying the retained figure
      counts transcripts only is corrected** — it is made false by this slice.
- [x] The setting exists and defaults on (D15). `[recovery] persist_unsaved_buffers`,
      `config/recovery.rs`, `RecoverySettings`'s own hand-written `Default` (not derived, which
      would give `false`).
- [x] Nothing in this slice writes to a path inside the project (§1 row 1). Every path this slice
      ever opens for writing is under `<state_root>/recovery/records/`, checked by the module's own
      doc comment and by every test in `recovery::tests` asserting against `records_dir`, never
      against anything under a project root.

### Required at review 485

The slice is substantial and mostly right: D10's ordering honoured (purge, figure, setting and the
privacy-page correction all in with the first content written), `0700`/`0600` asserted by reading
the mode, no buffer content in the audit store, a clean document never written, the record type
carrying no undo field at all, and the file-content-read guard found and named rather than
silenced. Catching review 469's tick-count confound **before** publishing was the right instinct.
Two required items.

- [ ] **The two medians contradict each other, and the published per-document figure follows only
  one of them.** Same run, same 3.3 MiB fixture: one dirty document costs **+0.001 ms/tick**, ten
  cost **+16.687 ms/tick**. That is **16,687× for 10× the work**, and the per-document figure
  (1.669 ms) comes from dividing the ten-document median by ten — which your own one-document
  measurement denies by a factor of 1,669. The 20-document extrapolation (33.374 ms) rests on the
  linearity those two numbers jointly disprove. The likely explanation is that the one-document
  delta is **below this harness's resolution**, in which case say that — `+0.001 ms/tick` printed
  as a cost is a noise floor wearing a number's clothes — and publish the per-round spread, not
  only the medians, so a reader can see it. **RFC-065 D7 got this right by publishing the ratio
  against what linearity predicts (9.2× where 10× was expected); computing that same ratio here
  would have surfaced this immediately.**
- [ ] **Records with no marker are ordinary, not a bug — and the rule that said otherwise was
  mine.** See **Amendment 1** on the RFC. Verified live: a clean window close removes the marker and
  **leaves the records**, and a restart then says nothing about them; there is no guard anywhere
  against quitting with unsaved work. Nothing in PR-027-B needs changing for this — the amendment
  changes **PR-027-C**, which must offer whatever records exist rather than gating on a detected
  crash. **It also means this RFC must not reach a release with B in and C out**: between them, a
  user who quits with unsaved work leaves content on disk that nothing offers back and nothing
  removes but a purge they have to go and find.

**On the close-path constraint you disclosed:** confirmed, pre-existing, and already a known
limitation in the product's own code — `apply_project_close_confirmation` says in as many words
that a refused close leaves the project open with the modal already dismissed. **It is a real
defect** (a dialog offering an action it then does not perform, with no explanation) but it is not
RFC-027's, and your test standing in with a stale record against a clean document is the right way
to work around it rather than through it. Your read of D11's consequence is right too: the close
trigger will only ever find a record that is stale for some other reason.

### Required at review 486

The measurement fix is right and PR-027-B's numbers now stand up. One required item, which is not
about this fix.

- [ ] **There is no `## 0.31.0` changelog section, and two slices have closed.** Review 471 ruled
  the incremental changelog mandatory *because* it is written as each slice closes, and this RFC's
  own Whole-RFC checklist already carries that line. PR-027-A was arguably nothing to tell a user
  about; **PR-027-B is not** — it ships a setting, a second Trust Settings figure, a privacy-page
  section, and the first content this product writes outside a project since transcripts. Start the
  section now, with B in it. At `0.30.0` the same omission produced a changelog that stated the
  opposite of the tree for three reviews running.
- [ ] **Whatever figure reaches that changelog carries the caveat it carries in the evidence.**
  `0.30.0`'s entry published an extrapolation and labelled it one; the per-document cost here is
  weaker than that — derived from the ten-document median alone, with the one-document point below
  resolution — and the changelog must say so rather than inheriting the bare number.

**Not required, worth one line each:**

- The caveat fires on `ratio.abs() > BURST_N * 5.0`, a proxy, when the direct signal is already
  computed two lines above: **the one-document spread straddling zero** is what makes it noise.
  A moderately noisy run (ratio ~30×) would skip the caveat and still deserve it.
- If a twenty-document condition is cheap — `dirty_documents` is already a parameter and the enum
  has three arms — **measure at the bound instead of extrapolating to it.** Two measurable points
  would establish the rate D8 asks for rather than assuming it. Only if cheap; the interval
  decision does not need it.

**The guard that hid the finding is worth naming**, because it will recur in a different shape: a
numerical guard against a degenerate divisor suppressed the ratio *exactly* on the runs where the
ratio was the finding. That is the same family as this project's filtered-gate rule — a safety check
that removes the diagnostic in the one case it matters. You found it yourself; it is recorded here so
the next harness does not reinvent it.

### Required at review 487

The changelog is written and well caveated, and the twenty-document measurement is a genuine
improvement over the extrapolation it replaces: two independently measured points agreeing to 0.3%
on the per-document rate is a real confirmation of linearity, not an assumption. The
spread-straddles-zero trigger is the right signal, and *"not claimed to hold below ten"* is exactly
the sentence that number needed. One required item.

- [ ] **The changelog publishes one run of a visibly load-sensitive measurement as though it were
  the cost.** Your own `qa-evidence.md` records the same ten-document condition three times:
  **+20.442**, **+16.687** and **+12.336 ms/tick** — a **1.66×** between-run spread, and line 332
  already says the figure moves with "this measuring machine's own load". Line 326 even records a
  **40.9 ms** figure at the twenty-document bound. The changelog states **24.6 ms** at that bound,
  which is the lowest-load run of the three, with no mention that the same quantity has measured
  nearly twice that.

  **The within-run comparison is sound and should stay** — ten against twenty, same run, same load,
  is exactly the right way to establish the rate. What is missing is the other axis: the rate itself
  moves between runs. Give the changelog the range, or the worst observed, the same way it already
  gives the one-document caveat. A reader takes "about 24.6 ms, measured" as *the* number; on a busy
  machine it is closer to 41.

**Not required.** The status line is right to say the release cannot ship without PR-027-C, and
right to say why — that is Amendment 1's consequence stated where a user would meet it.

## PR-027-C — the offer

- [ ] **Measurement 1 — the offer** lists each recoverable buffer with its project and path, and
      **declining leaves every file on disk untouched**, proved on real files (D4, §1 row 3).
- [ ] **Measurement 2 — the real round trip**: edit without saving, `SIGKILL`, restart, recover,
      verified against what is on disk (D6).
- [ ] The **unchanged** disk file restores the buffer as dirty.
- [ ] The **changed** disk file goes through the existing `ExternalChanged`/conflict path, and
      **no new conflict vocabulary was minted** (D5, §1 row 2). If one seemed necessary, that is
      reported as a finding instead.
- [ ] The **deleted** disk file offers the text with the deleted state.
- [ ] The offer says recovered buffers come back **without undo history** (D3, §3 row 10).
- [ ] Recovery data with no marker is reported as a cleanup bug, not consumed as a crash
      (D1, §3 row 11).
- [ ] **Live capture**, including the changed-on-disk case, not only the easy one.

## Whole-RFC

- [ ] `REQ-RECOVER-002` and `REQ-RECOVER-005`'s coverage rows updated — and **the "where safe" and
      "where technically feasible" hedges are reported as *decided*, naming what they were decided
      to mean**, not repeated back.
- [ ] The colour-alone, i18n completeness and internal-identifier scans still pass.
- [ ] `cargo fmt`, `clippy --workspace --all-targets -D warnings`, `git diff --cached --check`
      **after staging**, `rfc_docs_invariants`, `cargo test --doc --workspace`, **three consecutive
      full-workspace runs with `--no-fail-fast`**, **0 fixture entries left** in a fresh short fixed
      `TMPDIR` — a literal, not `mktemp`.
- [ ] Every new intermittent has a dated row in `test-process-leak.md`.
- [ ] The changelog is written incrementally as slices close, **and re-read against the finished set
      at the candidate** — not against the last slice's diff (the lesson of RFC-065 review 479).
- [ ] The book is read against the changelog **in both directions**, including for any user-visible
      word this RFC's commits touch (`release-checklist.md`, "A word quietly widening").
- [ ] The core pin bumps with the version. *(Release-cut item; not a Whole-RFC item.)*
- [ ] Commits are pushed once the gate is green.

## Final Acceptance Decision

- [ ] Accepted.
- [ ] Accepted with required follow-up.
- [ ] Requires re-review after changes.

Reviewer notes:

```text
```
