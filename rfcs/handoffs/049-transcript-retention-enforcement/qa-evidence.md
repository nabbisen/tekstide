---
title: "RFC-049 — QA evidence"
rfc: "RFC-049"
rfc_file: "../../done/049-transcript-retention-enforcement.md"
source_rfc_status: "Implemented and closed 2026-09-16 — M12"
target_milestone: "M12"
created: "2026-09-12"
---

# Evidence

## PR-049-A — arithmetic and marking

`tekstide-core` only. **Nothing deletes**, and nothing in production calls any of it — the
greps for both are below.

### D9: the inverse, defined by round-trip rather than by a second validator

`DomainTimestamp` wraps a formatted UTC string and offered no way back to seconds.
`DomainTimestamp::unix_seconds()` is the inverse of `format_unix_seconds_utc`, and its last line is
the design:

```rust
(format_unix_seconds_utc(seconds) == value).then_some(seconds)
```

Parsing the six fields is easy; deciding whether they name a real instant is where a hand-written
check would have to know month lengths and leap years — and would then be **a second, independent
opinion about the calendar, able to disagree with the formatter**. Computing the seconds and
formatting them back makes the formatter the only authority. `"2026-13-45T99:99:99Z"` is rejected
without this function knowing what a month is.

That matters more than tidiness here, because `DomainTimestamp::from_utc_string` validates **shape
only** — twenty bytes with separators in the right places — so month 13 is a value this crate will
hold and a persisted store can hand back. `days_from_civil` (Hinnant's, the companion of the
`civil_from_days` already in the file) does the arithmetic and validates nothing, deliberately.

**`None` is a real answer**, and §1 settles what a caller does with it: *not expired*.

### §1 applied to arithmetic, four ways

Every ambiguity resolves toward keeping, because the eventual consequence is deleting private local
data with no undo:

| Case | Answer | Why |
| --- | --- | --- |
| Exactly at `max_age_days` | **not** expired | strictly greater; a transcript survives its last day |
| A timestamp naming no instant | **not** expired | an age that cannot be computed is not an age |
| `now` before the transcript's own time | **not** expired | the elapsed span is unknown, not negative |
| `last_write_at` earlier than `created_at` | the **later** instant governs | a corrupt pair must not shorten a life |

Age is measured from the **most recent evidence of activity** — `last_write_at` when present,
`created_at` otherwise, the later of the two when both exist. A transcript written yesterday is not
thirty days old because it was created thirty-one days ago, and this is also the reading that keeps
more. It matches D8's definition of "oldest" for PR-049-B's ordering, so one notion of age serves
both.

### The flagged decision: `transcript_retention_days = 0`

**`max_age_days == 0` enforces nothing rather than everything, and the reviewer should overrule this
cheaply if it is wrong.**

RFC-045's parser accepts a configured `0` (response 378 noted it deliberately: semantically the
tightest *reduce*). `TranscriptRetentionLimits::is_bounded()` — which predates this RFC — already
treats `0` as **unbounded**. Two readings follow, and they are opposites:

- **Zero means delete everything immediately.** Literal, and the most destructive available reading
  of a value a user could plausibly type by accident or leave behind while experimenting.
- **Zero means the limit is not bounded, so nothing expires.** Defers to the existing validity
  check that the rest of the crate already uses.

§1 decides it: *"A transcript kept past its limit is a bounded disappointment. A transcript deleted
before its limit is gone."* `is_transcript_expired` returns `false` whenever `!limits.is_bounded()`,
and `limits_that_are_not_bounded_expire_nothing` pins it. **The cost is that a user who genuinely
wants zero retention gets none**, which is a disappointment rather than a loss — and if the RFC
wants the other reading, the honest place to fix it is RFC-045's parser refusing `0`, not this
function deleting on it.

### D3: `Expired` marks, and keeps the bytes

`mark_transcript_expired_if_due` sets `TranscriptLifecycleState::Expired` and touches neither
`byte_count` nor `storage_path`. `record_lifecycle_state` only zeroes the byte count for a state
whose `has_retained_bytes()` is false, and `Expired`'s is **true** — so no existing caller changes
meaning, and a transcript is visibly eligible before it is removed, which is what D2's
"not hidden from the user" requires in practice.

It returns whether this call changed anything, so PR-049-C's §4 rule — *a cleanup that deleted
nothing writes nothing* — has an honest thing to count. A transcript with no bytes left (`Purged`,
`DisabledByOptOut`, `CaptureFailed`) is not marked: moving it to `Expired` would claim it has bytes.

### Required tests, each ablated separately

| Box | Ablation | Result |
| --- | --- | --- |
| Round-trip at the boundaries | — | epoch, both ends of a leap day, the day after, and **past 2100** (the century rule a naive every-fourth-year implementation gets wrong) |
| String vs seconds ordering disagree | — | see below |
| Exactly at the limit is not expired | `>` → `>=` | **fails alone** |
| `Expired` keeps its bytes | — | byte count, storage path, and `has_retained_bytes()` all asserted |
| Shape-valid impossible instants | remove the round-trip check | **fails two tests**, both being "a value naming no instant has no age", at the timestamp layer and at its transcript-level consequence |
| Corrupt write-before-creation pair | take `last_write_at` unconditionally | **fails alone** |
| A purged transcript is not marked | remove the retained-bytes guard | **fails alone** |

**The string-ordering test is the one D9 asked for by name.** It uses years either side of the
formatter's four-digit width: `"10000-01-01T00:00:00Z"` sorts **before** `"2026-01-01T00:00:00Z"`
because `'1' < '2'`, while naming an instant nearly eight thousand years later. It asserts the
disagreement exists, then that the wide year has **no age at all** (the formatter cannot produce it,
so §1 keeps), and that the ordinary one reads back exactly. Anyone who later replaces the seconds
comparison with `a.as_str() < b.as_str()` fails here, and the failure says why.

### Test fixtures use literal instants, on purpose

Every timestamp in these tests is a **literal string**, never one computed from seconds by a
helper. A first draft had such a helper and it had to reimplement the civil-date arithmetic the
tests exist to check — a second opinion about the calendar, the same defect the round-trip
definition exists to avoid, moved into the test file. Ten days after `2026-01-01` is `2026-01-11`,
and a reader can confirm that without running anything.

### D7 and "nothing deletes", both as greps rather than as claims

```
$ grep -n "now_utc" crates/tekstide-core/src/transcript/retention.rs
8:/// There is no clock abstraction in this crate — `DomainTimestamp::now_utc`
12:/// that do. Production passes `now_utc()` at RFC-049 D2's two trigger

$ grep -nE "remove_file|remove_dir|mark_purged|purge" crates/tekstide-core/src/transcript/retention.rs
(no matches)
```

Both `now_utc` hits are doc comments. **No expiry decision can reach the wall clock**, and
`a_fixed_now_gives_the_same_answer_every_run` states it as a test as well — a hundred calls, one
answer, guaranteed by the signature rather than by tolerance.

No production caller exists: `is_transcript_expired`, `mark_transcript_expired_if_due` and
`most_recent_activity_seconds` are called only from `tekstide-core`'s own tests this slice. Nothing
user-facing says transcripts are removed (§6).

### Gate

`fmt`, `clippy --workspace --all-targets -D warnings`, `git diff --check`, `rfc_docs_invariants`:
clean. Three consecutive full-workspace runs, output redirected to files: **507 + 4 + 773, green
every time** (+15 tests).

## PR-049-B — selection and cleanup

`tekstide-core` only. **No production caller** — grepped; every hit outside tests is a definition,
an export, or the method's own internal use. Nothing user-facing changed, so §6 holds.

Built under **D8′**, which replaced D8 after this slice first stopped (request 382): liveness is the
transcript's `AgentRun.status`, not `lifecycle_state`, which production never moves off `Active`.

### What was added

- **`agent_run_may_still_be_writing(AgentRunStatus)`** in `transcript/retention.rs` — a **new**
  predicate, reusing neither `agent_run_status_is_active` nor
  `agent_run_status_blocks_strong_association`. It matches **every variant by name**: only
  `Completed | Failed | Cancelled` are not live; `Detached` is live.
- **`ProjectSession::apply_transcript_retention(limits, retained_bytes_in_other_projects, now)`** —
  expiry pass, then byte-budget pass, deleting **only** through RFC-033's `purge_transcript_at`.
- **`TranscriptRetentionCleanup`** — the result PR-049-C's triggers will read: `marked_expired`,
  `expired` and `budget` purge summaries kept apart so the record can say **why**, `failures`, and
  `project_budget_exhausted` / `app_budget_exhausted` for D4's `RequiredLocalBounded` refusal.

### How liveness is driven in the tests, checked by grep

The checklist forbids assigning `lifecycle_state`, because that is the defect D8′ removes. Runs
reach `Running` through `AgentRun::transition_to` — the validated transition the real launch path
uses — and leave it through `ProjectSession::apply_agent_terminal_outcome`, the call production makes
when a terminal exits (`Exited { 0 }` → `Completed`, `OrphanedUnknown` → `Detached`).

```
lifecycle_state assignments in the new tests:           none
record_active_write / record_truncated_write / ...:     none
```

Transcripts are **production-shaped**: `byte_count` stays `0` and `last_write_at` stays `None`. One
test sets `last_write_at`, to state an **age** for the oldest-first case, and says so in a comment.

### Decisions the slice had to make, each resolved toward keeping (§1) and flagged

1. **App-wide pressure is relieved from the triggering project's transcripts only.** RFC-011 says
   app-wide cleanup processes *"inactive transcripts first, oldest first"* and does not say across
   which projects. Deleting project B's transcripts because the user acted in project A is the less
   predictable reading of D2, and `AppState::app_wide_retained_transcript_bytes` sums only **open**
   projects, so a cross-project "oldest" would be chosen from a partial list. **Cost:** an app-wide
   budget can read exhausted while an older transcript sits in another project. Cheap to overrule —
   selection is one private method over a candidate list.
2. **A failed deletion stops the budget pass, but not the expiry pass.** Continuing the budget pass
   would delete a *newer* transcript only because an older one could not be removed; one transcript
   failing to delete makes no other one less expired. Neither aborts the call.
3. **Liveness that cannot be determined is live.** A transcript naming no run, or a run the session
   does not hold, is never touched. Production always has both — which is exactly why the unknown
   case must not be the one that deletes.
4. **Limits that are not bounded clean nothing**, the same reading of `is_bounded()` PR-049-A's
   expiry already takes and response 381 accepted.
5. **A previously marked `Expired` transcript that is no longer due** (the configured age was raised
   since) is **kept**: the expiry pass re-checks age against the limits in force rather than trusting
   the mark. **This follows from the code, not from a test** — no test in this slice constructs a
   stale mark. Stated so it is not read as covered.

### A checklist box that cannot be satisfied as written

> *Expiry runs before byte-budget selection, asserted on **which transcript survived**.*

**With one shared notion of age, the two orders leave the same survivors in every case.** Expiry
measures age from most recent activity; budget selection orders oldest-first by the same measure. So
every expired transcript is older than every non-expired one, and a budget pass reaches the expired
ones first anyway. I modelled both orders exhaustively before claiming this:

```
ages 1..4, sizes 1..3, live or not, 1..4 transcripts, budgets 0..9, age limits 0..4
17,310,000 configurations, 0 differ
```

**Expiry-first is implemented as specified.** What the order observably changes is **attribution**:
a transcript past its age is reported under `expired`, not `budget` — the distinction RFC-011 line
150 asks the record to carry. That is what
`a_transcript_past_its_age_is_removed_by_expiry_not_by_the_budget` asserts, and swapping the passes
fails it alone. **The box stays unticked with the contradiction named**, per the checklist preamble.

### Ablations — each run from a clean tree, restoration verified by sha256

| Box | Ablation | Result |
| --- | --- | --- |
| §2: a live writer is never selected, only candidate, app budget exhausted | remove the liveness filter from budget selection | `a_live_writer_…` **fails alone** |
| Exhaustive predicate (D8′) | add `AgentRunStatus::Paused` | **fails to compile**, exactly once: `retention.rs:153: error[E0004]: non-exhaustive patterns: AgentRunStatus::Paused not covered` |
| `Detached` is live | map `Detached` to not-live | `a_detached_runs_transcript_is_not_selected` **fails alone** |
| D8′-a: live not marked `Expired` | move the liveness check after the marking | `a_live_transcript_past_its_age_limit_…` **fails alone** |
| Oldest-first by most recent activity | order by `created_at` only | `budget_selection_is_oldest_first_…` **fails alone** |
| Expiry before budget (attribution) | swap the two passes | `a_transcript_past_its_age_is_removed_by_expiry_…` **fails alone** |
| Cleanup routes through RFC-033's purge | a raw `fs::remove_file` that still reports the deletion | `cleanup_goes_through_the_purge_and_leaves_a_tombstone` **fails alone** |
| Unknown liveness is live | an unlinked transcript counts as not live | `a_transcript_with_no_agent_run_is_never_selected` **fails alone** |
| A failed budget deletion stops the pass | remove the `break` | `a_failed_budget_deletion_stops_…` **fails alone** |
| D8′-b: bytes from the files | read `Transcript.byte_count` instead | **three tests** — see below |

**The byte-source ablation is a disclosed composition, and the ablation corrected my disclosure.**
Every budget test is production-shaped and every real `byte_count` is `0`, so with the tracked
count no budget is ever over and budget cleanup never runs: the real-bytes test, the oldest-first
test, and the stop-on-failure test all fail. The test's doc comment first named only **two** of
those; the ablation found the third and the comment now names all three. Isolating it would mean
giving the other tests a `byte_count` production never writes.

The raw-deletion ablation deliberately **still reports** the deletion in the summary — the realistic
shape of a second deletion loop — so the only thing it lacks is RFC-033's tombstone, and the only
test that fails is the one asserting it.

### The first ablation run was invalid, and left the tree corrupted

**The Bash tool here runs zsh**, which does not word-split an unquoted `$FILES`. The first script's
backup, every restore, and every sha256 check acted on one nonexistent path and failed **silently**,
so the ablations **stacked**: each ran on top of every earlier one. Only its first result — the §2
ablation, run on a clean tree — was valid; failure counts climbing run over run exposed the rest.

Recovery was **not** by reversing ten stacked edits. The corrupted files were saved to the
scratchpad, restored from `HEAD` (`domain/agent.rs` had no slice changes, confirmed from
`git status` before the run), and the slice re-applied from its original edits. **The rebuild was
then proven identical to the pre-ablation slice**: diffing the corrupted copies against it, every
hunk is one of the ablations and nothing else.

The valid run above wraps the script in `bash`, uses an array, and **refuses to ablate unless the
baseline checksum file holds exactly three lines** — the check that would have stopped the first run
before it touched anything. A verification that can fail silently is not one.

### Greps the checklist asks for

```
byte_count in apply_transcript_retention and its helpers:  none
byte_count in transcript/retention.rs:                     none
production callers of apply_transcript_retention:          none
```

### Gate

`fmt`, `clippy --workspace --all-targets -D warnings`, `git diff --check`: clean. **Three
consecutive full-workspace runs, output redirected to files: 507 + 6 + 783, green every time**
(+10 tests). No flake.

## PR-049-C, first commit — the independent list

The five items response 389 placed after RFC-050, and nothing else. **No trigger is added, so
nothing in production calls the cleanup yet**: the audit producer, the cleared marks and the two
budget predicates are all reachable only from tests. The triggers, D4′ and the disclosures follow in
the next commit.

### `transcript_retention_days = 0` is refused

The value meant two opposite things at once. `is_bounded()` reads `max_age_days == 0` as
**unbounded**, so expiry would enforce nothing; response 378 had called the same value the tightest
possible *reduce*, and RFC-045's sensitive-change machinery still treats lowering it as a tightening.
PR-049-A resolved it toward keeping — §1, because the other reading deletes everything immediately —
and that left a value the file accepts and the product ignores, which is RFC-045 D3′'s failure one
level down.

`take_retention_days` refuses it, and **the message names the control that expresses what the user
meant**: declining transcript capture for the project. A refusal costs one startup with a
diagnostic; silently ignoring the value costs the user the retention they believed they had set.
The book's caveat and a changelog entry say the same thing.

### A stale `Expired` mark is cleared

`clear_stale_expired_mark` runs **first in the expiry pass**, before the liveness check, because
clearing removes a false claim rather than making one and touches no bytes. The only way to hold a
stale mark is the one the test builds: a deletion that failed, then a limit raised past the
transcript's age.

**Restored to what the bytes say**, not blindly to `Active`: a transcript whose writer truncated it
returns to `Truncated`, read from `truncation_state`. Both states retain bytes, so `byte_count`
survives either direction.

### The audit pairing, and the record that is not written

`transcript_purge_record` takes the actor/source pairing as a parameter (D6: not a second
near-identical constructor — everything else about the record is identical, which is the argument
for one constructor). `record_transcript_policy_cleanup` writes `(AppPolicy, ExplicitCleanup)`, the
pairing RFC-013 reserved for this RFC and which had no producer until now.

**A cleanup that removed nothing returns `None` and writes nothing** (§4). `removed_anything()`
counts deletions only: marking and clearing are not removals.

**One decision the RFC does not make, flagged for review:** a pass that removed some transcripts and
failed on others records **`Failed`**. The record exists only because bytes were removed, and
`Completed` would claim the cleanup did what it set out to do. A test pins it so it can be
overturned deliberately. The alternative — two records — needs an event the frozen schema lacks.

### "A deletion failed" is not "nothing is deletable"

Two predicates, each with its own test: a budget left exhausted by an undeletable file, and one left
exhausted because the only candidate is a live writer. The first has a remedy and the second does
not, which is why one sentence for both would tell the first user nothing they can act on. **The
user-facing disclosure is not here** — it needs the triggers — so that checklist box stays unticked.

### D5, implemented and measured as unobservable

Both readers now share `configured_retention_limits`; deriving the same limits at two call sites is
how they came to disagree in the first place. **Ablation A6 restored `agent_run_default()` in the
summary and failed nothing**, 816 core tests green — which is the checklist's own expectation, and
is recorded as a measurement rather than dressed up as a test that would be asserting the compiled
constants.

### Ablations, each restored and hash-checked, `--no-fail-fast`

| | Ablation | Fails |
| --- | --- | --- |
| A1 | the parser accepts `0` again | the zero-refusal test **alone** |
| A2 | stale marks are never cleared | `a_stale_expired_mark_is_cleared_when_the_limit_is_raised` **alone** |
| A3 | the policy cleanup records unconditionally | `a_policy_cleanup_that_removed_nothing_writes_no_record` **alone** |
| A4 | the policy cleanup records as the user | `a_policy_cleanup_and_a_user_purge_are_recorded_as_different_actors` **alone** |
| A5 | a failed deletion is never reported | `a_budget_left_exhausted_by_a_failed_deletion_says_a_deletion_failed` **alone** |
| A6 | the summary reads compiled defaults again | **nothing — by design; see D5 above** |

**A5 failed two tests on its first run**, because the stale-mark test used `a_deletion_failed()` as
its precondition. That coupling is the test's, not the property's: the precondition now reads
`failures` directly, and A5 was re-run and fails alone.

### A flake I wrote, and fixed

`the_reset_notice_keeps_the_boot_figure_after_the_live_one_changes` (from the RFC-050 follow-up)
failed in run 3 of this slice's first gate. Its negative assertion was `!lines[1].contains("12")`,
and the same line carries the state root — that run's temporary directory was
`/tmp/tekstide-run-554612-245/…`, whose pid contains `12`. Fixed by stripping Fluent's isolate marks
and comparing `777 bytes` / `12 bytes`, which a path cannot contain. Dated row in
`test-process-leak.md`, with the approval-queue recurrence from the same run.

### Gate

`cargo fmt --all --check`, `clippy --workspace --all-targets -D warnings`: clean.
`rfc_docs_invariants`: 9 passed. `mdbook build docs`: clean. **Five consecutive full-workspace runs
with `--no-fail-fast`, output redirected to files: 519 + 9 + 816, green every time** — five rather
than three because the first gate exposed the flake above. `git diff --cached --check` after
staging: clean.

## PR-049-C, second commit — the triggers, D4′, and the disclosures

**This is the commit that deletes a user's data without being asked.** Everything before it marked,
selected or measured; nothing in production called any of it.

### The two triggers, and nothing else (D2)

```
production callers of run_transcript_retention_cleanup:  shell.rs:3453 (launch preflight)
                                                         shell.rs:9202 (project open)
timers / watchers / idle sweeps touching retention:      none
```

The project-open trigger runs **after** the load, not before: before it, a session knows only the
transcripts it launched itself, so the cleanup would measure a fraction of what the project holds and
expire nothing an earlier run left — which is the case this slice exists for.

### D4′, decided in the core rather than at the call site

The launch request carries one new fact — `transcript_budget_exhausted` — and the core decides what
it costs, so the two modes cannot drift apart at a call site:

- **`LocalBounded`** starts the run with capture disabled (`prepare_transcript_capture`), and the run
  records `TranscriptAbsence::BudgetExhausted`.
- **`RequiredLocalBounded`** is refused by `validate_transcript_policy` **before any process starts**,
  which is RFC-011's own sentence. Unreachable from the product; reached through the builder, as the
  locked-file refusal already is.

**The measurement is the caller's, the policy is the core's.** Putting the verdict in the request is
also what makes "the decision shown is the decision applied" mechanical rather than a convention: the
same boolean is rendered and applied.

### The confirmation, and a gap in D4′'s premise

`attempt_agent_run_launch` runs the preflight **once**, before either branch — including the branch
that only opens a dialog, since the dialog must state what the launch will do. The modal carries the
verdict; `confirm_and_launch_configured_profile` applies **that** value and never re-measures.

**D4′ says "the existing launch confirmation says so", and the product does not have one at every
launch.** `ConfiguredProfileFirstUse` appears only for a *configured* profile, only on its first use
in a session. A `Ctrl+Alt+A` launch on the compiled default profile, or a second launch of a
confirmed one, shows no dialog at all — so for those launches there is no pre-click surface, and the
run's own detail is the only disclosure. Implemented as: the notice wherever the confirmation exists,
and the run detail always. **Named here rather than papered over.**

### What the user is told

| Where | When |
| --- | --- |
| Project board | what retention removed, and — separately — that a deletion **failed** |
| Launch confirmation | this run's output will not be kept (when the budget is exhausted) |
| AgentRun Report | why this run has no transcript: an exhausted budget, or a lock another Tekstide held (RFC-050 D3, which had no rendering until now) |

The two board lines are separate because the remedies are: a file that cannot be deleted can be
looked at, and it stops the budget pass again at every trigger until it is.

### Ablations, each restored and hash-checked, `--no-fail-fast`

| | Ablation | Fails |
| --- | --- | --- |
| B1 | no project-open trigger | the open-trigger test **alone** |
| B2 | no launch preflight trigger | the launch-trigger test **alone** |
| B3 | the confirmation notice never shows | its presence test **alone** |
| B4 | it always shows | its absence test **alone** |
| B5 | the click recomputes the verdict | `the_launch_applies_the_budget_decision_the_confirmation_was_opened_with` (plus register row 1's socket flake, unrelated) |
| B6 | `RequiredLocalBounded` is not refused | the refusal test **alone** |
| B7 | capture proceeds despite the budget | the starts-without-capture test **alone** |
| B8 | the board never says what was removed | its test **alone** |
| B9 | a failed deletion gets no line of its own | the failure test **alone** |
| B10 | the run detail ignores the recorded reason | **two tests** — the budget line and the lock line. One mechanism, two renderings; disclosed as a composition rather than split |
| B11 | a notice is built when nothing happened | `a_cleanup_that_did_nothing_produces_no_notice` **alone** |

### Two ablations that first reported nothing, and why that was my harness

**B5 and B11 both came back "fails nothing" on the first batch, and both results were false.**

- **B5 did not compile.** Substituting a call taking `&mut State` into an argument list that already
  borrowed `state` is a borrow error. My script counted `test result: FAILED` lines, so a build that
  never produced any read as a clean run. Re-run with the call hoisted into a local: it fails the
  carried-decision test, as it should.
- **B11 compiled and genuinely failed nothing**, which exposed a real gap: the only test of "absent
  when nothing happened" supplied the `None` itself instead of getting it from a cleanup, so the
  guard inside `TranscriptCleanupNotice::from_cleanup` was held by nothing.
  `a_cleanup_that_did_nothing_produces_no_notice` now holds it, and B11 fails it alone.

The harness lesson is the one this pack already learned once, in a different disguise: **a
verification that can fail silently is not one.** A compile failure must be reported as "not
evidence", never as an absence of failures. My first fix for that over-matched (`error: test failed`
is a test result, not a build failure) and had to be narrowed to `could not compile`.

### The live walkthrough found a real gap, and a grammar bug

Against `target/release/tekstide`, with config, state, project and the AI CLI each in its own
`mktemp -d`. Every image shows only those `/tmp` paths.

**Reaching an exhausted budget honestly.** The byte budgets are compiled constants (256 MiB per
project), so the walkthrough plants a **300 MiB sparse** `transcript.log` at the product's own path
and **holds its `flock`** from another process. The loader probes the lock, reads the file as live,
and the cleanup refuses to select it at any pressure (§2) — so the budget is genuinely unfreeable,
which is the only honest way to reach D4′. Confirmed in the run: the file was still there, at
314,572,800 bytes, after the open trigger.

| Step | What happened | Image |
| --- | --- | --- |
| 1 | `Ctrl+Alt+A`: the confirmation names the resolved executable **and** says *"This run's output will not be saved: the transcripts already on disk are at the limit…"* | `evidence/pr-049-c/01-launch-confirmation-budget-exhausted.png` |
| 2 | Confirmed the launch. **No transcript was written**: the only file under the project's directory is the planted 300 MiB one. `Ctrl+Alt+R` shows *"This run has no transcript: the transcripts already on disk were at the limit when it started…"* | `02-agent-run-report-no-transcript.png` |
| 3 | With the lock released and two **60-day-old** transcripts planted, a fresh start removes both and the board says *"Transcript retention removed 2 transcripts (66 bytes) that were past the limits in your settings."* | `03-board-says-what-retention-removed.png` |

**The gap: a command-line open had no trigger.** Step 3 failed the first time — the old transcript
survived. `main.rs` opens a project from the command line **before `State` exists**, so it never
reaches the GUI's `Added` arm where I had put the trigger; it loaded the earlier transcripts and
cleaned up nothing. Fixed with `run_transcript_retention_for_open_projects`, called from `boot()`
once `State` is built, and held by
`a_project_already_open_when_the_state_is_built_gets_its_cleanup_too`.

**No test would have caught it**, because every GUI test opens a project through the `Added` arm.
This is what the walkthrough is for, and it is the second time in this pack that a launch/open path
existed which nothing exercised (response 390 found the first).

**A grammar bug, also from the capture**: the removal line read *"removed 1 transcript … that
**were** past"*. The plural variant covered only the count phrase. The whole clause now varies, and
the re-capture shows the plural form.

### Clippy found what the restructure left behind

Hoisting the preflight out of the wrapper chain made three launch wrappers unused by production, and
`-D warnings` failed on it. They are now `#[cfg(test)]`, with a doc comment saying production enters
above them — rather than kept alive by a production call that exists only to satisfy the lint.

### Gate

`cargo fmt --all --check`, `clippy --workspace --all-targets -D warnings`: clean.
`rfc_docs_invariants`: 9 passed. `mdbook build docs`: clean. **Three consecutive full-workspace runs
with `--no-fail-fast`, output redirected to files: 535 + 9 + 819, green every time** (+16 shell, +3
core). `cargo audit`: three allowed warnings, all already rows in `dependency-advisories.md`.
`git diff --cached --check` after staging: clean.

## PR-049-C follow-up (response 397) — two tests that held nothing, and D4′'s surface

### U2 — the call site, removed rather than tested

The trigger now runs **inside `State::new`**. `boot()` has no line to call it, so there is no line to
delete: the reviewer's ablation has nothing to ablate, and the replacement — skipping the trigger in
the constructor — fails the test alone.

The old test called `run_transcript_retention_for_open_projects` directly, which held the helper and
not the call site. It now builds a `State` and asserts the file is gone.

**One consequence, found by an existing test.** With the trigger in the constructor, every `State`
touched the audit store — and `attempt_agent_run_launch_with_profile_still_launches_and_registers_with_an_unopenable_store`
asserts its store is untouched at that point. It was right to fail: a cleanup with nothing to record
should not **open** the store either, or "the policy looked and did nothing" could mark a session's
audit health degraded. The open is now behind the same condition as the record.

### U1 — a verdict, not a comparison

`the_cleanup_decides_exhaustion_from_a_scan_not_from_the_cache` makes the two figures disagree about
**exhaustion**: a 2 GiB sparse file in another project's directory, against a zeroed cache. It then
asserts the cleanup's own answer. Reading the cache yields "nothing on disk", so V1 fails it alone.

Sparse is what makes this affordable — `set_len` moves the size the scan reads without writing a
block, the same trick the live walkthrough used for 300 MiB.

### D4′'s surface, corrected

The line now also sits beside the **Trust Settings launch button**, one below RFC-047 D4's *"This run
will not be recorded…"* — the pre-click surface every launch has, rather than a dialog that exists
only for a configured profile's first use.

It states what is true when rendered, not a promise about the click: a launch runs the cleanup first,
so a limit shown here can be relieved before the run starts. **A launch from a keybinding elsewhere
still has no pre-click notice**, exactly where RFC-047 leaves the audit one, and the run's detail says
why afterwards.

### Ablations, each restored and hash-checked, `--no-fail-fast`

| | Ablation | Fails |
| --- | --- | --- |
| V1 | the cleanup reads the cached figure | the scan-not-cache test **alone** |
| V2 | `State::new` does not run the trigger | the already-open-project test **alone** |
| V3 | the Trust Settings budget notice never shows | its presence test **alone** |
| V4 | it always shows | its absence test **alone** |

### Gate

`cargo fmt --all --check`, `clippy --workspace --all-targets -D warnings`: clean — clippy asked for
the two audit-store conditions to be one `let`-chain, so they are. `rfc_docs_invariants`: 9 passed.
`mdbook build docs`: clean. **Three consecutive full-workspace runs with `--no-fail-fast`, output
redirected to files: 537 + 9 + 819, green every time** (+2 shell tests). `cargo audit`: three allowed
warnings, unchanged. `git diff --cached --check` after staging: clean.
