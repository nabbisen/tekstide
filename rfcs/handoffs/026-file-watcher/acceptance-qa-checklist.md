---
title: "RFC-026 — acceptance and QA checklist"
rfc: "RFC-026"
rfc_file: "../../accepted/026-file-watcher-and-multi-document-model.md"
source_rfc_status: "Accepted 2026-09-30 — M13"
target_milestone: "M13"
created: "2026-09-30"
---

# Acceptance and QA checklist

Tick a box when the evidence is in `qa-evidence.md`, not when the code looks right. A box naming a
measurement with no number in the evidence is not ticked.

## PR-026-A — the batching

- [x] A simulated burst of **1,000 events into one directory yields one scan request per window**.
      Both numbers written down: scan requests, **and git subprocesses**.
      *(1 scan request, 2 git subprocesses; five windows: 5 and 10. `qa-evidence.md` § PR-026-A.)*
- [x] Ablated: remove the batching and the count becomes 1,000.
      *(`ablate.sh`: 1,000 scan requests and 2,000 git subprocesses for both streams.)*
- [x] The window is a stated constant (`SCAN_WINDOW = 250 ms`), with its reasoning in the doc
      comment. **The book half moved to C at review 450** — nothing feeds the batcher yet, so no user
      can observe the window, and naming it in the book would describe behaviour the product does not
      have. The implementer declined to tick it and said why; that was right.
      *(The constant is stated in code: `SCAN_WINDOW = 250 ms`. **Not yet in the book**, deliberately:
      it is an internal number until a watcher exists to make it visible. The book names it in the slice
      that makes watching user-visible — B or C. Left unticked until then, rather than documenting
      behaviour the product does not yet have.)*
- [x] Events for different directories do not silently merge.
      *(`events_for_different_directories_do_not_silently_merge`.)*
- [x] No dependency added in this slice, and no watcher wired.

### Required at review 450

- [x] **`record` cannot be handed a non-directory.** It derives `path.parent().unwrap_or(Path::new(""))`,
      so `/` or a bare relative name silently becomes the empty path and then
      `ScanRequest { directory: "" }`. Unreachable from a real watcher, but B is about to feed it real
      events. **Make it unrepresentable** — take the directory B computes anyway, rather than deriving
      one and needing a fallback.
      *(`record(directory: PathBuf, at)` derives nothing. Residual, stated in `qa-evidence.md`: a
      `PathBuf` can still hold `""`; the batcher invents none, and B's caller must not hand it one.
      A stricter newtype needs B's root context.)*
- [x] **Verify `GIT_SUBPROCESSES_PER_SCAN = 2` against a real scan.** Its provenance is recorded
      (measurement 8, review 431) but it describes something nobody has run in A. If the real figure
      differs, **A's evidence numbers are corrected**, not left standing.
      *(Measured through the explorer's own scan seam: **2 on every warm scan, 3 on the first scan in a
      process** (`git --version`, cached per process). Steady state is 2, as the batching counts assume;
      the one-time extra is stated in the constant's doc comment and in `qa-evidence.md`. The
      measurement is `measured_git_subprocesses_per_real_scan`, `#[ignore]`d, run by hand.)*

## PR-026-B — the watcher and its dependency

- [x] **D4's evaluation recorded before adoption**, including `notify`'s MSRV, its **CC0-1.0** licence
      (unlike every other direct dependency), its last stable date, and the answer to **what it does
      when the kernel refuses another watch**, read from what it does.
      *(Recorded at review 452 with `Cargo.toml` untouched, then adopted at 452. The last-stable date
      was settled by the architect from crates.io: `8.2.0`, 2025-08-03, with the 9.0 release candidates
      as the re-check row — `d4-notify-evaluation.md`, `qa-evidence.md` § Slice B.)*
- [x] **C1 (review 452): the wrapper does not depend on which error it gets.** Any non-success from
      `watch()` — any variant, and a caught panic per H1 — means *watching stopped*; the variant only
      refines the message. *(`WatchScope::reconcile` never inspects a refusal; a test proves two
      different refusals give the same state. A panic is caught in `guard_watch_call` and tested with a
      real panic.)*
- [x] **C1′ (review 453, correcting C1): the desired set contains only directories that exist.**
      *(`WatchScope::reconcile` filters `desired` through `WatchBackend::directory_exists` before the
      platform is asked. Proved with a fake (`a_directory_that_has_gone…`, and the deleted-later case) and
      on the real filesystem (`a_real_directory_removed_before_reconciling…`). The ablation (filter
      removed) fails both fake tests. The residual race — exists when checked, gone when watched — is
      named in `scope.rs` and in `qa-evidence.md`, not engineered away.)*
      `reconcile` hands `desired` straight to `notify.watch()` with no existence check, so a directory
      deleted before it is watched becomes a `WatchRefusal` and stops **all** watching — and that is
      the normal event a watcher exists to observe, not a rare race. A path that is gone is not a
      refusal; there is nothing to watch. Check existence **ourselves**, so the branch is on a fact we
      establish and never on a library's error variant — which is what C1 was protecting. The
      residual race (exists when checked, gone when watched) is **named**, not engineered away.
- [x] **C2 (review 452): no path in the sidebar line.** *(The sentence is Option C, verbatim, as the
      trusted catalog messages `watch-stopped-sidebar` and `watch-stopped-sidebar-action`, with no
      arguments: no untrusted value can reach it, so the rule holds by construction. Width-tested with the
      ignore-rule sentences: `the_watch_stopped_sentence_fits_the_sidebar`. Its on-screen display is B2.)*
- [x] **The sentence, chosen at review 454: C, verbatim.** *"Folders are no longer updating."* (31)
      and *"Reopen the project to resume."* (29) — the only candidate whose **both** lines fit
      `SIDEBAR_COLUMNS = 32`. The implementer's pick (A) had a 68-character second line that would
      have clipped the action away, and B missed by one character; the existing
      `the_ignore_rule_sentences_fit_the_sidebar` would have caught either. C is also the only one
      whose first line says *what* is stale, so a reader can tell the editor is unaffected.
      `watch-stopped-sidebar`, **trusted text with no arguments**, so C2 holds by construction —
      say so in the catalog comment, so nobody later adds a `$path` to be helpful.
- [x] **No positive-state line** (ruled at 454): when watching works the sidebar says nothing about
      it. The sidebar already spends four reserved lines; only the stopped state speaks.
- [x] **C3 (review 452): do not echo notify's error string as product text.** *(The refusal's detail is
      for logs only, enforced by its type; the sentence says the consequence first and names no cause.
      Its on-screen display is B2.)*
- [x] `dependency-advisories.md` carries the new crate the way it carries the existing three.
      *(A section for the five Linux crates, the platform-only lock entries, and the 9.0 re-check row;
      `cargo audit`: zero vulnerabilities, the same three warnings.)*
- [ ] Watch scope is `expanded` + the root + open documents' folders. **Counted before and after**
      *(Step 1 (review 455): the session computes the admitted set from its own tree and document —
      `the_session_wires_the_expanded_folders_into_the_desired_set`, ablated. The app does not reconcile
      it yet: that needs the watcher's owner decided, see review 456.)*
      *(Step 2: reconciled on the triggers, and the owner exists per project. The live count across
      expand, collapse and close is not measured in the running app, so the box stays open.)*
      expanding, collapsing and closing a project — a scope that only grows is a defect.
      *(The policy is proven: `expanding_collapsing_and_closing_move_the_watched_count_exactly`. **Not
      yet wired** to the live explorer tree and the open-document set — that is the next step, so the
      box stays open.)*
- [ ] **Budget exhaustion forced in a test**: no crash, today's fallback behaviour, and a sentence on
      screen saying watching stopped and why.
      *(Forced and tested against a fake backend — `a_refused_watch_stops_watching_and_drops_every_watch_without_crashing`.
      **The sentence on screen is not written yet**, so this box stays open.)*
- [x] **The policy guarantee lands with the wiring, not with its test** (ruled at review 455):
      `desired_directories` is `root.join(relative)` with **no policy consulted**, is `pub`, and takes
      arbitrary paths. D7 is a guarantee, so step 1 takes paths the explorer's access policy has
      admitted — the hostile fixture below *proves* the property and must not be where it arrives.
      *(Step 2 (review 456): the live reconcile takes only `watched_directories()`, which admits every
      directory through `WatchedDirectory::admit`. Nothing else reaches the owner. Proved by
      `the_session_wires_the_expanded_folders_into_the_desired_set`, ablated at step 1.)*
- [ ] Hostile fixture: a symlink leaving the root is not watched; a loop does not recurse.
      *(Step 1 (review 455): the access policy is the only way into the scope — `WatchedDirectory` is
      constructible only through `admit` — and `the_access_policy_decides_what_the_scope_may_ever_hold`
      refuses a real escaping symlink and admits an in-root one. Non-recursion is by construction (single
      directories only). The end-to-end fixture, with a live tree and a live watch, is step 4: still open.)*
- [x] **Ownership is per project** (ruled at review 456): each open project owns its backend, scope
      and receiver. Measured against (b), one process-wide watcher: the watch budget is **per user**
      (524,288), so (b) shares nothing that is scarce; instances are 1,024, so (a) costs one per
      project. What (b) would share is the **failure** — a refusal in one project stopping them all,
      which contradicts *"Reopen the project to resume."* R6 holds by ownership, not by a cleanup step.
      *(Implemented at step 2: `ProjectWatcher` owns backend, scope and receiver; `State::project_watches`
      holds one per open project and close removes it. `dropping_the_owner_ends_its_event_stream`;
      `a_project_is_watched_from_open_and_its_owner_is_removed_on_close`, ablated: the close removal taken
      out fails it.)*
- [x] **Reconcile is trigger-driven and invents no trigger**: a toggle, a scan finishing, a document
      opening or closing — **and project open**, which the plan left out and which is when the root
      first becomes desired. It must **not** become an unconditional call in `update`: it costs a
      `directory_exists` syscall per desired directory, so a hundred expanded folders is a hundred
      stat calls per invocation. Say so where it lives.
      *(Step 2: six call sites, `grep -n reconcile_project_watch crates/tekstide/src/shell.rs`: project open
      (three arms), folder toggle, document open, scan finished. The cost is in `reconcile_project_watch`'s
      doc comment. Not per frame.)*
- [x] **At most one waiter on the event stream, enforced by the type** (required at review 457):
      `wait_for_event` holds the receiver's mutex across a blocking `recv()`, `WatchEvents` is
      clonable, and the method is `pub` — so a second caller blocks until the next filesystem event,
      which on a quiet project is indefinite. **Avoid `try_lock`**: on a subscription rebuild the new
      thread would fail the lock and return, leaving the project silently receiving nothing.
      *(Done at review 457: `WatchEvents` is not `Clone` and `wait_for_event` takes `&mut self`; the receiver
      is handed out once by `WatchEventsSlot::take`, and a second take gets `None` without blocking.
      `the_event_receiver_is_handed_out_once_so_only_one_waiter_can_exist`. The ablation is weak by nature:
      it shows the test sees a missing receiver, and the property rests on the type. `qa-evidence.md` § Review 457.)*
- [x] **Reconcile's cost on the render thread is measured before this box is ticked** (ruled at 457):
      cost **per expanded directory**, not one figure at a hundred, reported against
      `NFR-PERF-002`'s p95 ≤ 32 ms — the nearest stated budget, since no trigger is an editor
      keystroke.
      *(Measured: `measured_reconcile_cost_per_expanded_directory`, 1 to 500 expanded folders. Steady p95
      about 4.2 µs per expanded directory from 10 to 500, and 2.1 ms at 500 — within 32 ms at every count.
      Measured on tmpfs with 500 as the largest tree, not on disk or beyond. `qa-evidence.md` § Review 457.)*
- [x] Nothing new on the render thread; the subscription is the shape `explorer_scan_subscription`
      already has.
      *(Step 2: the event thread is off the render thread, and the subscription has the explorer's shape.
      Reconcile runs in `update` on its triggers, which is new render-thread work; its cost is measured and
      within budget (box above). Ticked on that measurement, not on the claim that the work is zero.)*

## PR-026-C — the change arrives

- [ ] **`REQ-FILE-003` captured live**: a file created in an expanded folder appears without the user
      reopening it.
- [ ] **`REQ-FILE-004` measured**: editor keystroke latency under a watched burst, against RFC-057's
      baseline harness.
- [ ] **The book names the batching window** (moved here from A at review 450): by this slice a user
      can observe it, so it describes real behaviour rather than an internal constant.
- [ ] Unsaved edits survive an external change; **no silent reload**; a deleted open file is a state
      the product can say.
- [ ] A reload takes the undo history with the document, and the product does not pretend otherwise.
- [ ] **The split point considered and answered here**, not at the candidate.

## PR-026-D — the open set

- [ ] `open_buffer_count()` and `dirty_file_count()` count the whole open set. **A test fails if
      either counts one when two are open.**
- [ ] A third counter, if one exists by then, is reported rather than absorbed.
- [ ] The open-document cap is stated when reached.
- [ ] The close dialog and save-all count every open document.

## Whole-RFC

- [ ] `REQ-FILE-003`, `REQ-FILE-004` and `NFR-PERF-007` move to met **with evidence a user can see**,
      not with the fields existing.
- [ ] The colour-alone, i18n completeness and internal-identifier scans still pass.
- [ ] `cargo audit` gains no vulnerability.
- [ ] `cargo fmt`, `clippy --workspace --all-targets -D warnings`, `git diff --cached --check` after
      staging, `rfc_docs_invariants`, **three consecutive full-workspace runs with `--no-fail-fast`**
      to files, **0 fixture entries left** in a fresh short `TMPDIR` (a **short fixed literal** —
      `mktemp` plus nested run dirs crosses the `AF_UNIX` 108-byte limit).
- [ ] Every new intermittent failure has a dated row in `test-process-leak.md`.
- [ ] The core pin bumps with the version.
- [ ] Commits are pushed once the gate is green.

### Carried into `0.29.0` at review 451 — not RFC-026's subject

- [ ] **`change_review_content_view_build_cost_by_line_count_measurement` stops measuring the
      machine.** Assert a **ratio against a reference workload timed in the same process** — ten
      times the work must cost under some multiple of the time — rather than an absolute 500 ms.
      Scheduled after a **fourth** episode: `0.26.0`, the `0.28.0` candidate, the architect's `0.28.0`
      verification, and PR-026-A's own gate. The implementer flagged that the count was high enough
      to schedule rather than record again, and was right. Worked examples for both halves:
      `editor_typing_latency_baseline_100_000_lines` (publish the number, assert nothing) and snora
      0.52.0's CI ratio.

## Final Acceptance Decision

- [ ] Accepted.
- [ ] Accepted with required follow-up.
- [ ] Requires re-review after changes.

Reviewer notes:

```text
```
