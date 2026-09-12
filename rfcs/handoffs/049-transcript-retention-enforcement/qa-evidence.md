---
title: "RFC-049 — QA evidence"
rfc: "RFC-049"
rfc_file: "../../accepted/049-transcript-retention-enforcement.md"
source_rfc_status: "Accepted 2026-09-12 — M12"
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
