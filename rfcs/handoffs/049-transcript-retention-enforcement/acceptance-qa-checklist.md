---
title: "RFC-049 — acceptance and QA checklist"
rfc: "RFC-049"
rfc_file: "../../accepted/049-transcript-retention-enforcement.md"
source_rfc_status: "Accepted 2026-09-12 — M12"
target_milestone: "M12"
created: "2026-09-12"
---

# Acceptance and QA checklist

Every box is a property. A box whose plan assigns it elsewhere stays unticked with the
contradiction named — the reviewer's error to fix, not the implementer's to paper over.

## PR-049-A — arithmetic and marking

- [x] Seconds round-trip against `format_unix_seconds_utc`, covering **epoch, a leap day, and a
      pair where string comparison and correct arithmetic disagree**.
      Both ends of a leap day, the day after, and past 2100 — the century rule a naive
      every-fourth-year implementation gets wrong. The disagreement test uses years either side of
      the formatter's four-digit width (`"10000-…"` sorts before `"2026-…"`), asserts the
      disagreement exists, and then that the wide year has **no age at all**. **The inverse is
      defined by round-trip** rather than by its own validator, so it cannot hold a second opinion
      about the calendar; ablating that check fails two tests.
- [x] Exactly at `max_age_days` is **not** expired (§1). **Ablation:** `>` → `>=`; this test
      alone fails. Done exactly that — fails alone.
- [x] `Expired` keeps its bytes and still reports `has_retained_bytes()`. Byte count, storage path
      and `has_retained_bytes()` all asserted; `record_lifecycle_state` only zeroes bytes for
      states whose `has_retained_bytes()` is false, and `Expired`'s is true.
- [x] Every expiry decision takes `now` as a parameter — **no `now_utc()` inside any of them**
      (D7). Grep the new functions; a clock call is a defect even if the test passes.
      Grepped: the only `now_utc` hits in `retention.rs` are doc comments. Also stated as a test
      (a hundred calls, one answer), though the signature is what guarantees it.
- [x] Nothing deletes in this slice. Grepped `remove_file|remove_dir|mark_purged|purge` in
      `retention.rs`: no matches. No production caller exists either — the three new functions are
      called only from `tekstide-core`'s own tests, and no user-facing text says transcripts are
      removed (§6).

**One decision flagged for review, and it is a §1 call.** `max_age_days == 0` enforces **nothing**
rather than everything: RFC-045's parser accepts a configured `0`, and `is_bounded()` already
treats `0` as unbounded, so `is_transcript_expired` defers to it. The literal reading — delete
everything immediately — is the most destructive available reading of a value a user could type by
accident. If the RFC wants that reading instead, the honest place to fix it is RFC-045's parser
refusing `0`, not this function deleting on it. See `qa-evidence.md`.

## PR-049-B — selection and cleanup

- [x] **A live writer is never selected** — including when it is the only candidate and the app-wide
      budget is exhausted (§2). **Ablation:** remove the liveness filter; this test alone fails.
      **Liveness is driven through a real `AgentRun` status**, not by assigning `lifecycle_state`.
- [x] The liveness predicate is **new**, matches `AgentRunStatus` **exhaustively**, and names only
      `Completed | Failed | Cancelled` as not-live (D8′). **Ablation:** add a variant to
      `AgentRunStatus` and confirm the crate **fails to compile** rather than defaulting it to
      deletable.
- [x] **`Detached` is not selected**, and `Completed` is — one test each.
- [x] A **live** transcript is not marked `Expired` even past its age limit (D8′-a). **Ablation:**
      move the liveness check after the marking.
- [x] Byte selection uses `real_retained_transcript_bytes()`, **never `Transcript.byte_count`**
      (D8′-b). Grep the slice for `byte_count`.
- [x] Oldest-first by `last_write_at` falling back to `created_at`, proven with a pair that
      inverts under the wrong field.
- [x] **Expiry-first is observable as attribution, not as survivors** (corrected at response 385):
      a transcript past its age is reported under `expired`, never `budget`; swapping the two
      passes fails `a_transcript_past_its_age_is_removed_by_expiry_not_by_the_budget` alone.
      This box originally required the order to be "asserted on which transcript survived",
      which cannot be satisfied: expiry and budget selection share one age measure, so both
      orders leave identical survivors. PR-049-B modelled 17,310,000 configurations with 0
      differences and left the box unticked with the contradiction named, as the preamble asks.
      Ticked by the reviewer against the corrected wording, after swapping the passes and
      watching that test fail alone.
- [x] Cleanup routes through RFC-033's purge — the **tombstone and `Purged` state** are asserted,
      which a raw deletion would not produce (§3).
- [x] No production caller exists yet.

## PR-049-C — triggers, record, refusal

**Partly paused, 2026-09-13 (request 387).** A session knows only the transcripts launched in the
current process, so the triggers, the removal disclosure, D4′'s exhaustion path and §6's changelog
sentence would act on almost nothing. They wait for loading transcripts from disk, to be scoped as
its own RFC — [RFC-050](../../done/050-transcripts-from-earlier-runs.md). **After RFC-050's
PR-050-C, as PR-049-C's first commit** (order set at response 389; response 387 had said "land
now"): refusing `transcript_retention_days = 0`; the actor/source pairing and no record for an
empty cleanup; stale `Expired` marks re-checked and cleared; "a deletion failed" reported
separately; and D5 as restated below. The purge-defect disclosure landed on its own at `c2f5092`.

- [x] Both triggers fire; **no timer, watcher, or idle sweep exists anywhere** (D2). Grep for one.
      *Second commit. Production callers of `run_transcript_retention_cleanup`: the launch preflight
      and the project open — and, after the live walkthrough found it missing, a project already open
      when `State` is built (a command-line open, which never reaches the GUI's `Added` arm). Grepped
      for timers/watchers/sweeps touching retention: none. B1 and B2 each fail their trigger's test
      alone; the boot case has its own test.*
      *Unticked at response 397 (U2): the project-open trigger for a project already open when
      `State` is built is held by a test that calls `run_transcript_retention_for_open_projects`
      directly. Deleting the call in `boot()` failed nothing. Hold the call site — best by running the
      trigger inside `State::new`, so no call site can be forgotten (the walkthrough found this class
      once already).*
      *Third commit, taking the structural option: **the trigger runs inside `State::new`**, and
      `boot()` has no line to forget. The test now asserts that *constructing* `State` is enough, and
      V2 (skip the trigger in the constructor) fails it alone.*
- [x] A policy cleanup writes `TranscriptPurge` as **`(AppPolicy, ExplicitCleanup)`**, and a user
      purge in the same file writes `(User, TrustedUi)` — **both read back from a real store**, so
      the distinction §4 requires is visible in one place.
      *First commit: `record_transcript_policy_cleanup`, and
      `a_policy_cleanup_and_a_user_purge_are_recorded_as_different_actors` writes both into one real
      store and reads them back. A4 (record as the user) fails it alone. **No production caller yet**
      — the triggers that call it are the next commit.*
- [x] **A cleanup that removed nothing because every deletion failed records `Failed`** (response 396).
      Today it records nothing: the producer returns `None` whenever nothing was purged, so a policy
      deletion that tried and could not is invisible. §4's "deleted nothing writes no record" is for
      *nothing to do*. **Ablation:** drop the failure arm from the condition; the test fails alone.
      *`removed_anything() || a_deletion_failed()`;
      `a_policy_cleanup_whose_every_deletion_failed_still_records_failed`. B12 fails it alone.*
- [x] A cleanup that deleted nothing writes **no record**. **Ablation:** record unconditionally.
      *`removed_anything()` counts deletions only, so a pass that merely marked or cleared writes
      nothing. A3 fails `a_policy_cleanup_that_removed_nothing_writes_no_record` alone.*
- [x] **The exhaustion check reads a fresh scan** (RFC-050, response 393). The GUI's app-wide figure is
      a cache, refreshed at boot, at each load and after each purge, so it misses bytes written since.
      The launch check scans `transcripts/` at preflight and never reads that cache.
      *The cleanup takes its app-wide figure from `transcript_disk_usage_for(&state.app_shell)`, a
      scan; the cached figure lives on `State`, which that function cannot reach.
      `the_cleanup_scans_the_app_wide_figure_rather_than_reading_the_cache` asserts the difference
      directly — bytes written after the cache was filled are invisible to it and visible to the
      scan. **Disclosed:** no test moves the exhaustion verdict itself, because the byte budgets are
      compiled constants (256 MiB / 1 GiB); the live walkthrough reaches it with a 300 MiB sparse
      file instead.*
      *Unticked at response 397 (U1): the test asserts that a scan sees bytes the cache does not — it
      never calls the cleanup. Pointing the cleanup at the cached figure failed nothing. Assert the
      cleanup's own decision changes with the figure it reads.*
      *Third commit: `the_cleanup_decides_exhaustion_from_a_scan_not_from_the_cache` makes the two
      figures **disagree about exhaustion** — a 2 GiB sparse file in another project's directory
      against a zeroed cache — and asserts the cleanup's own verdict. Sparse, so it costs no blocks
      and still moves the figure the scan reads. V1 (read the cache) fails it alone.*
- [x] **D4′: launch cleanup leaves a budget exhausted → the run starts with capture disabled.** Assert
      the process started **and** no transcript file or `Transcript` record exists for it.
      **Ablation:** capture anyway; the test fails alone.
      *`a_launch_whose_budget_is_exhausted_starts_without_capture`: the process started, no record, no
      reference, and no file at the path a captured run would have used. B7 fails it alone. The
      decision is made in the core from a fact the caller supplies, so the two modes cannot drift
      apart at a call site.*
- [x] **The launch confirmation says so — present when exhausted, absent otherwise**, each its own
      assertion, ablated separately. Wording meets RFC-047 §5: states the fact and why, implies no
      danger, implies no fix available from there.
      *B3 and B4 each fail their own test alone; captured live. **The premise is not quite true of the
      product, and this is for you:** `ConfiguredProfileFirstUse` exists only for a *configured*
      profile, on its first use in a session. A launch on the compiled default profile, or a second
      launch of a confirmed one, opens no dialog at all, so those launches have no pre-click surface —
      the run's detail is their disclosure. Implemented as: the notice wherever the confirmation
      exists, the detail always.*
      *Unticked at response 397: D4′'s premise was mine and wrong. `ConfiguredProfileFirstUse` exists
      only for a configured profile's first use, so a `Ctrl+Alt+A` launch on the compiled default has
      no dialog. Put the line beside the Launch button in Trust Settings too, where RFC-047's
      "will not be recorded" notice already lives, and state the remaining limit in the RFC.*
      *Third commit: `agent_run_launch_transcript_budget_notice` renders beside the Launch button, one
      line below RFC-047 D4's own. It states what is true at render time — a launch cleans up first,
      so the limit may be relieved by the click — rather than promising the next run goes unsaved. V3
      and V4 each fail their own test alone. A launch from a keybinding elsewhere still has no
      pre-click notice, exactly where RFC-047 leaves the audit one.*
- [x] **What the confirmation shows is what the launch applies** — the decision is carried into the
      launch, not recomputed after the click. Test: bytes freed between the two do not produce a
      transcript the user was told would not exist.
      *The modal carries the preflight verdict and `confirm_and_launch_configured_profile` applies
      **that** value. `the_launch_applies_the_budget_decision_the_confirmation_was_opened_with` opens
      the dialog as exhausted in a fixture where nothing is exhausted, so a recomputing launch would
      capture. B5 fails it (plus register row 1's socket flake, unrelated and disclosed).*
- [x] **The run's detail says why it has no transcript**, distinct from opt-out. Neither
      `DisabledByOptOut` nor `CaptureFailed` is written for this case — neither is true of it.
      *`agent_run_detail_unavailable_line` matches exhaustively on `TranscriptAbsence`, so a future
      reason cannot inherit another's words. Three tests: the budget case, RFC-050's lock case (which
      had no rendering at all until now), and the fall-back for a run with no recorded reason. B10
      fails the first two — one mechanism, two renderings, disclosed as a composition.*
- [x] `RequiredLocalBounded` on an unfreeable budget is still refused in core and **no process
      starts**, unit-tested through the builder and **labelled unreachable from the product**. No
      production caller added (RFC-011: *"must not… be used by an unreviewed workflow"*).
      *`validate_transcript_policy` refuses with `RequiredTranscriptBudgetExhausted`, before any
      process starts — the same layer the mode's other refusals live in.
      `a_required_local_bounded_launch_is_refused_when_the_budget_is_exhausted`, reached through the
      builder and labelled unreachable. B6 fails it alone.*
- [x] RFC-011's row states the project and app-wide budgets are enforced **at launch**, with the
      overshoot bound (capturing runs × per-transcript limit) — D1 without overstating it.
      *Corrected in place under RFC-011's own Acceptance Criteria, dated and marked "corrected, not
      reworded": what was enforced when it closed, what is enforced now, at which two moments, and
      the overshoot bound with its reason.*
- [x] `tests/retention.rs`'s message *"so PR-049-C can refuse a RequiredLocalBounded launch"* says D4′.
      *It already reads "so PR-049-C can start the run without capture and say so (RFC-049 D4′)" —
      updated when D4′ replaced the refusal; verified against the file rather than assumed.*
- [x] **RFC-045's parser refuses `transcript_retention_days = 0`**, naming the transcript
      opt-out as the way to express what the user probably meant. Required at response 381:
      `is_bounded()` already treats `0` as *unbounded* while response 378 called it the tightest
      *reduce* — two opposite semantics for one value. PR-049-A resolved it toward keeping (§1),
      correctly; but a **value** the file accepts and the product ignores is RFC-045 D3′'s own
      failure one level down, and `transcript_capture_declined` already expresses zero retention.
      *`take_retention_days` refuses it with a message naming the capture opt-out. A1 (accept 0
      again) fails `a_zero_transcript_retention_period_is_refused_and_names_the_capture_opt_out`
      alone, and `a_one_day_…_is_accepted` holds the boundary. The book's caveat and a changelog
      entry say so.*
- [x] D5: the summary uses the session's configured limits — **stated as unobservable today**:
      `budget_pressure` reads only byte budgets and none is configurable, so the original ablation
      (restore `agent_run_default()`) changes nothing a test can see (request 387).
      *One `configured_retention_limits` helper now serves both the launch and the summary, which is
      why they had drifted apart. **A6 ran that exact ablation and failed nothing** — 816 core tests
      green with the compiled defaults restored. Recorded as measurement, not as a test.*
- [x] **The purge defect is disclosed before it is fixed**: the book's privacy page, `README.md`,
      and `CHANGELOG.md` Unreleased say that after a restart or reopen, Trust Settings and purge cover
      only transcripts from runs since the project was opened, and that earlier ones stay in
      `transcripts/` until that directory is deleted.
      *`c2f5092` (request 389): README, the book's privacy page, and a `CHANGELOG.md` Unreleased
      entry. The reviewer built the book, and checked the text on the live site.*
- [x] **Policy removal is told to the user, on a surface they read** (response 385). B marks and
      purges in one pass, so nothing is ever visibly `Expired` unless its deletion fails — the
      audit record is therefore the *only* trace, and nobody reads the audit store. Absent when
      nothing was removed. **Ablation:** suppress the disclosure; its test fails alone.
      *The project board, the same surface the recent-list reset notice uses and the one read at every
      project open. B8 fails its presence test alone; B11 fails the "no notice from a cleanup that did
      nothing" test alone. Captured live: "Transcript retention removed 2 transcripts (66 bytes)…".*
- [x] **"A deletion failed" is reported distinctly from "nothing is deletable"** when the budget
      stays exhausted (response 385). A failed candidate stops the budget pass on every trigger,
      so one undeletable file can disable capture for every new run indefinitely (D4′); the
      disclosure must name the failure, which has a remedy the other case does not.
      *Second commit: the board gives the failure its own line, in its own words — it says the file
      stays and that the cleanup will stop at it again, which "nothing could be freed" says of
      neither. B9 fails that test alone.*
      *After the first commit: the **core distinction** exists and is held both ways
      (`a_budget_left_exhausted_by_a_failed_deletion_says_a_deletion_failed`, and the live-writer
      case as its own test; A5 fails the first alone). The box asks for a **disclosure**, which needs
      the triggers — next commit, with the removal disclosure it belongs beside.*
- [x] **A stale `Expired` mark is re-checked, not trusted — and cleared**, with a test: a transcript
      whose deletion failed stays marked; raise the configured age; at the next trigger it survives
      **and is no longer `Expired`**. PR-049-B's decision 5 (survival) follows from the code and was
      stated as untested. The clearing is new: nothing in production moves a transcript out of
      `Expired` (reviewer's grep, response 385), so today a saved transcript keeps a durable state
      that is no longer true.
      *`clear_stale_expired_mark`, called first in the expiry pass.
      `a_stale_expired_mark_is_cleared_when_the_limit_is_raised` fails the deletion, raises the
      limit, and asserts the transcript survives **and** is `Active` again. A2 fails it alone. The
      mark is restored to `Truncated` rather than `Active` when the bytes were truncated.*

## Whole-RFC

- [x] `cargo fmt`, `clippy --workspace --all-targets -D warnings`, `git diff --check`,
      `rfc_docs_invariants` clean.
      *All clean. Clippy caught three launch wrappers the restructure left unused by production; they
      are `#[cfg(test)]` now rather than kept alive by a call that exists to satisfy the lint.*
- [x] Three consecutive full-workspace runs, **output redirected to a file**, green; any recurring
      flake gets a dated row in `test-process-leak.md`.
      *535 + 9 + 819, green every time. Two dated rows added this slice: a flake I wrote into an
      RFC-050 test, and a recurrence of register row 1 under ablation B5.*
- [x] `cargo audit` still reconciles against `dependency-advisories.md`.
      *Three allowed warnings — `paste`, `ttf-parser`, `lru` — all three already rows in that file,
      with no new advisory.*
- [x] **The changelog says the first run after upgrading deletes every transcript already older than
      the configured age** (§6). This is the sentence most likely to be left out, and RFC-045's
      changelog earned a required fix for exactly this class.
      *In bold, in its own paragraph, with the default spelled out as "every transcript from more than
      a month ago". The book's configuration caveat says it too.*
- [x] **No text anywhere claims more than is enforced.** RFC-011's row and `crates/tekstide-core/README.md`
      both currently say retention is bounded per project and app-wide; after this slice that is true
      for the first time, and before it lands neither may be reworded to pretend it already was.
      *Corrected in this commit, not before it: `crates/tekstide-core/README.md`, the root `README.md`,
      the book's privacy and configuration pages, and RFC-011's own criterion. Each now says what is
      enforced, at which two moments, and that the byte budgets are not a hard ceiling.*
- [x] RFC-011's Acceptance Criteria re-read against the result, and its row corrected to say which
      budgets shipped enforced (D1).
      *Re-read in full. The criterion that needed correcting was the retention-bounds one; the others
      hold as written. The correction is dated, appended under the original, and says it is a
      correction rather than a rewording.*

## Final Acceptance Decision

*(Section present from the start this time — RFC-046's and RFC-045's packs both shipped without one.)*

- [ ] Accepted.
- [ ] Accepted with required follow-up.
- [ ] Requires re-review after changes.

Reviewer notes:

```text
Pending review.
```
