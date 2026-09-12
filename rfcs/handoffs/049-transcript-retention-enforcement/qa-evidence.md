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
