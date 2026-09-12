# RFC-049: Transcript Retention Enforcement

Status: **Accepted by the human owner 2026-09-12.** **D1–D9 decided by the architect on acceptance** — see "Decided on acceptance" at the end, which adds **D6 (the audit record RFC-013 already reserved a slot for)**, D7 (the clock seam), D8 (what "oldest" and "inactive" mean) and D9 (age arithmetic on a string timestamp). Proposed the same day. Scoped after `0.18.0` shipped a configuration key whose value
nothing enforces. Reserved by no earlier RFC — the `future-work.md` row opened at response 377 said
"RFC-011 data-model change", and the measurement below shows it is larger than a data model.
Target milestone: **M12**
Date: 2026-09-12

Related RFCs:

- [RFC-011](../done/011-transcript-retention-and-local-data-policy.md) — owns transcript retention.
  **Its Goals and Default Retention specify everything below; its Acceptance Criteria do not, which
  is how it closed without them.**
- [RFC-033](../done/033-transcript-lifecycle-controls.md) — owns the manual purge this reuses.
- [RFC-045](../done/045-configuration-reachability.md) — shipped `transcript_retention_days`,
  recorded and unenforced, with every user-facing sentence saying so.

## Why

`0.18.0` lets a user write `transcript_retention_days = 7`, accepts it, records it on every run's
policy, and **never removes anything**. We said so in four places rather than imply otherwise, which
was right and is not a substitute for the behaviour.

**The measurement is worse than the changelog admits, and that is why this is an RFC rather than a
sweep.** RFC-011 names four retention budgets. Only one is enforced:

| Budget | State today |
| --- | --- |
| `max_bytes_per_transcript` | **Enforced.** The writer caps and marks `Truncated`. |
| `max_bytes_per_project` | **Computed, never acted on.** `TranscriptLocalDataSummary::new` sets `budget_pressure`; nothing reads it outside tests. |
| `max_bytes_app_wide` | **Computed, never acted on.** Same field, same fate. |
| `max_age_days` | **Neither computed nor acted on.** Read only by `is_bounded()`'s `> 0` check and RFC-045's config write. |

And `TranscriptRetentionState::Expired` **exists and nothing produces it** — RFC-036's shape, in the
vocabulary rather than the API.

RFC-011's Default Retention already says what should happen: *"cleanup should process inactive
transcripts first, oldest first by retention metadata… If no inactive transcript bytes can be
removed and the budget is exhausted, `LocalBounded` capture should truncate or disable further
writes with metadata, while `RequiredLocalBounded` should reject launch or fail preflight before
process start."* **None of that exists.** This RFC does not re-decide it; it builds it.

## D1 — What "bounded" was allowed to mean, and what it means now

RFC-011's acceptance criterion reads: *"Retention is bounded per transcript, per project, and
app-wide, **with metadata-only accounting** of total retained transcript bytes."* Two readings —
*enforced* and *accounted for* — and the implementation took the second. The criterion permitted it.

**Decided: bounded means enforced. Accounting is necessary and not sufficient.** RFC-011's row is
corrected to say which of its four budgets shipped enforced, rather than being reworded to have
always meant the weaker thing. A closed RFC's criteria are evidence of what was accepted; this one
was ambiguous and the ambiguity is the finding.

## D2 — Cleanup runs at moments a user can predict, never on a timer

RFC-011: *"age-based expiration processed by explicit cleanup/harness paths first, **not background
automation hidden from the user**."*

**Decided: two triggers, both already moments the user caused.**

- **Agent-run launch preflight**, where retention limits are consulted already.
- **Project open**, where the local-data summary is already computed.

No timer, no watcher, no idle sweep. A user who opens a project and launches nothing has nothing
deleted while they read. **This is a deliberate weakening of promptness in favour of predictability**,
and it is RFC-011's own choice, not a shortcut: transcripts are private local data, and a deletion
nobody can attribute to an action they took is worse than a late one.

## D3 — `Expired` marks; cleanup deletes; the tombstone survives

The vocabulary has both `Expired` and `Purged`, and RFC-011 requires purge to *"preserve a
content-free tombstone transcript reference by default"*.

**Decided:**

- **`Expired`** = past `max_age_days`, **bytes still present**, eligible for cleanup. This keeps
  `TranscriptRetentionState::has_retained_bytes()` correct as written — it already returns `true`
  for `Expired` — so no existing caller changes meaning.
- **Cleanup deletes the bytes and transitions to `Purged`**, reusing RFC-033's purge path rather
  than writing a second deletion. One code path deletes transcript bytes in this product, and it
  stays one.
- A transcript may therefore be **visibly eligible before it is removed** — *corrected at
  response 385: only when its deletion fails.* PR-049-B marks and purges in the same pass at the
  same trigger, so normally a transcript goes `Active → Expired → Purged` inside one call and
  nobody sees it eligible. That is the specified behaviour, not a slice defect; what it changes is
  **where "not hidden from the user" is discharged** — not by a visible `Expired` state, but by
  PR-049-C telling the user afterwards that transcripts were removed by policy, on a surface they
  actually read. The audit record is not that surface.

## D4 — Budget exhaustion: RFC-011 decided it, this RFC implements it

Not re-opened. Oldest-inactive-first, never a transcript with a live writer, and on exhaustion
`LocalBounded` stops writing with metadata while `RequiredLocalBounded` fails preflight before
process start. The last clause is the one with teeth: **a launch that cannot honour its own
retention contract must not start a process.** `rejects_launch_when_unavailable()` already exists
and already gates launch validation; this gives it the budget case it was written for.

## D5 — The summary must be computed against the limits in force

`transcript_local_data_summary_for` passes `TranscriptRetentionLimits::agent_run_default()` — the
**compiled** defaults. Since `0.18.0` a user can configure `transcript_retention_days`, so the
number shown to them is computed against a limit they did not set.

**Decided: the summary takes the session's configured limits.** A budget figure computed against a
different budget than the one that governs is §4.1's shape in arithmetic rather than prose.

## What this RFC must not become

- **Background automation.** D2 settles it. No timer, no watcher, no idle sweep — if that is ever
  wanted it is a separate decision with its own disclosure.
- **A second deletion path.** D3 routes through RFC-033's purge.
- **Configurable byte budgets.** `transcript_retention_days` is configurable because RFC-045 made it
  so; the three byte budgets stay compiled constants here. Making them configurable is a
  configuration-key decision under RFC-045's own D3′ rule — it returns with the code that reads it,
  and this RFC is that code, so it becomes *possible* here and is not *done* here.
- **Cross-device or remote retention.** RFC-011 non-goal, unchanged.

## Risks

- **This RFC deletes user data on a schedule the user set, and deletion is not reversible.** Every
  other slice in this area has been about disclosure; this one acts. D2's predictable triggers and
  D3's visible-before-removed state are the mitigations, and they are the reason the design is
  slower than a sweep would be.
- **`RequiredLocalBounded` failing preflight turns a full disk into a refused launch.** That is
  RFC-011's decision and it is correct — a run whose transcript cannot be retained is a run whose
  record will not exist — but it is the first case where a *retention* budget can stop work, and the
  refusal must say so in the terms RFC-047 §5 established.
- **The age default is 30 days and nothing has ever expired.** The first release carrying this will
  delete every transcript older than the configured age **on its first qualifying trigger**. That is
  correct behaviour and a surprising first run; the changelog must say it plainly, and this is the
  one item in the RFC most likely to be under-disclosed.

## Decided on acceptance (2026-09-12)

Four things an implementer would otherwise have had to invent, all found by reading the code the
RFC talks about rather than the RFC.

### D6 — Policy cleanup is audited, and RFC-013 reserved the exact pairing for it

`valid_transcript_purge` permits **three** actor/source pairings:

```
(User,      TrustedUi | AppCommand)      -- RFC-033's manual purge
(AppPolicy, ExplicitCleanup)             -- nothing produces this
```

**`AuditActionSource::ExplicitCleanup` exists in the frozen vocabulary, paired with `AppPolicy`,
for this family, and has no producer anywhere.** Its name is RFC-011's own phrase — *"explicit
cleanup/harness paths"*. RFC-013 reserved a slot for this RFC's cleanup before this RFC existed.

**Decided: policy cleanup writes a `TranscriptPurge` record as `(AppPolicy, ExplicitCleanup)`.**
Not as `User`: the product is deleting a user's data on its own initiative, and a trail that cannot
tell that from a purge the user clicked is worse than no trail. `transcript_purge_record` gains the
pairing as a parameter rather than a second near-identical constructor.

**This is the one place this RFC touches the audit schema, and it touches nothing**: the pairing is
already legal, already tested by the validator, and needs no schema change. If an implementation
finds itself wanting a new field, a new outcome, or a new reason code, it has left this decision.

### D7 — The cleanup takes `now` as a parameter; it does not call the clock

There is **no clock seam in this crate** — `DomainTimestamp::now_utc()` calls `SystemTime::now()`
directly, everywhere. A cleanup that calls it internally is a test that depends on wall-clock time,
and this project keeps a flake register largely populated by tests that depend on timing.

**Decided: every function that decides expiry takes `now: &DomainTimestamp` from its caller.**
Production passes `DomainTimestamp::now_utc()` at the two D2 trigger points. Tests pass a fixed
value and are deterministic by construction, not by tolerance. **No `#[cfg(test)]` clock, no
injectable trait** — a parameter is the whole seam, and it is the smallest one that works.

### D8 — "Oldest" and "inactive", named against real fields

RFC-011 says *"inactive transcripts first, oldest first by retention metadata"* without naming
either. `Transcript` carries `created_at` and `last_write_at: Option<DomainTimestamp>`.

**Decided:**

- **Inactive** = has no live writer. The existing `lifecycle_state` is the authority, not the
  presence of `last_write_at` — a transcript can be between writes and still have a running agent.
  **A transcript with a live writer is never selected, at any budget pressure**, which is RFC-011's
  *"running transcript writers must not be silently deleted underneath active AgentRuns"*.
- **Oldest** = by `last_write_at`, falling back to `created_at` when it is `None`. A transcript
  written yesterday is not older than one created yesterday and never written, and byte pressure is
  about what is still accumulating.

### D9 — Age arithmetic needs seconds, and `DomainTimestamp` is a string

`DomainTimestamp` wraps a formatted UTC **string**; it exposes `now_utc`, `from_utc_string`,
`as_str`, and **no way to get seconds back**. Age comparison needs arithmetic the type does not
offer.

**Decided: add the inverse of `format_unix_seconds_utc` to `domain::time`, and compare in
seconds.** Not by string ordering — that happens to work for this format and is a trap the moment
the format gains a suffix or a different width. Not by re-parsing with a date library; this crate
has none and this RFC is not the place to add one.

The new function is the first thing PR-049-A builds, with round-trip tests against
`format_unix_seconds_utc` including the boundaries that bite: epoch, a leap day, and a value where
the naive string comparison and the correct arithmetic disagree.

### D8′ — D8 named a field production never writes. The liveness authority is the **AgentRun**.

**Added 2026-09-13, response 382. D8 was wrong and PR-049-B was right to stop.**

D8 said *"the existing `lifecycle_state` is the authority"*. In the compiled product
`Transcript.lifecycle_state` is `Active` at creation and moved only by `mark_purged()` — its other
mutators, `record_active_write`/`record_truncated_write`, lost their only caller when **RFC-036
deleted `record_transcript_write_summary`**, whose verdict was *delete*, on the recorded ground that
*"a tracked counter is only ever correct prospectively."* So "inactive by `lifecycle_state`" selects
**nothing**: cleanup would be inert in production and green in tests, because a test sets the field
directly. RFC-036's own shape, and §4.1's. **I asserted a property of the code from its shape** —
the third time in this project, after D9's "consumer waiting" and `transcript_retention_days = 0`.

**Decided: liveness is the transcript's `AgentRun.status`, reached through
`Transcript.agent_run_id`** — `Some(..)` on every production transcript, because
`attach_agent_run_transcript` is the sole production caller of `Transcript::metadata`. The status is
driven from real call sites (`apply_agent_terminal_outcome`, reached at `shell.rs:2701` and `4690`).
The transcript does not know whether it is being written; the run does.

**Enumerate not-live, never live.** Ten `AgentRunStatus` variants exist. Retention's predicate
matches **exhaustively** on the not-live set:

```
Completed | Failed | Cancelled   →  not live, selectable
everything else                  →  live, never touched
```

A variant added later therefore **fails to compile** rather than silently becoming deletable. That
is §1 expressed in the type system, and it is the same construction as RFC-044's exhaustive mirror
and RFC-046's `plan_is_auditable`.

**`Detached` is live, and the cost is disclosed rather than designed away.** Two precedents in
`session.rs` disagree: `agent_run_status_is_active` excludes it, and
`agent_run_status_blocks_strong_association` includes it. Retention takes the conservative one,
because its question — *might a claim about this run's output be wrong?* — is retention's question,
while the other exists to count processes the close flow must terminate. `AgentRun.status`'s own
field comment says it is *"not proof of supervision after `Detached`"*, so a detached process may
still be writing and we cannot know.

**The consequence: a detached run's transcript is never reclaimed, and its bytes occupy the budget
permanently.** Under §1 that is the right trade — but taken far enough, an app-wide budget full of
detached transcripts would refuse launches forever under D4. **Reserved as a `future-work.md` row**,
not solved here: solving it needs to know whether a process is gone, which we cannot, and the
available heuristic is a file-mtime threshold — the ambient read and magic number PR-049-B correctly
flagged.

**Write a new predicate; do not reuse either existing one.** Their names and purposes belong to the
close-confirmation flow and to change association. A future adjustment for RFC-020's reasons would
change deletion safety silently — the "reusing a field for a different meaning" defect response 224
already caught here once.

### D8′-a — Liveness gates marking as well as selection

PR-049-B found the second half: `last_write_at` is `None` on every production transcript, for the
same deleted recorder, so PR-049-A's age falls back to `created_at` — **a run writing for forty days
under a thirty-day limit reads as expired while it is being written.**

**Decided: liveness is checked before any retention action, not just before selection.** A live
transcript is neither marked `Expired` nor selected. PR-049-A's arithmetic is correct as written and
stays; what changes is that nothing calls it for a live transcript. Marking a record `Expired` while
it is being appended to would be a false state on a durable record — §4.1 again.

### D8′-b — Bytes come from the filesystem, never from `byte_count`

`Transcript.byte_count` is `0` in production, same cause. **Use
`ProjectSession::real_retained_transcript_bytes()`**, RFC-033's existing `fs::metadata` route — the
same source `remove_transcript_file` uses at delete time, so the figure selected on and the figure
deleted agree by construction. RFC-036 recorded this precedent; PR-049-B found it independently.

**And D7's seam survives intact**, which is why no mtime is needed: liveness comes from in-memory
`AgentRun.status` and age from in-memory `created_at`, so **byte accounting is the only filesystem
read in the slice**, through a method production already calls. PR-049-B's concern about a second
ambient input arriving behind D7 is real and is answered by not needing it.

### D8′-c — The deleted recorder stays deleted

Wiring `record_transcript_write_summary` would make D8 true as originally written. **It is not an
option:** RFC-036 is closed, its verdict was *delete*, and its reasoning — a counter is correct only
prospectively, so every transcript written before the wiring reads `0` — applies unchanged. Nothing
above needs it.

### Settled details

- **Trigger order at launch preflight**: expiry first, then byte budgets. Expiring a transcript may
  free enough bytes that no budget cleanup is needed, and doing it the other way can delete a
  transcript the user still wanted while an expired one sits next to it.
- **A cleanup that deletes nothing writes no record.** The trail says what happened, and "nothing
  happened" is not an event. This matters because the triggers are frequent.
- **Cleanup failure never fails the thing that triggered it**, except the `RequiredLocalBounded`
  preflight refusal D4 already specifies. A failed deletion is a degraded state to report, not a
  reason a project will not open.

### Decided at PR-049-B review (2026-09-13, response 385)

**App-wide pressure is relieved from the triggering project only** (PR-049-B decision 1, accepted).
Deleting project B's transcripts because the user acted in project A is the less predictable reading
of D2, and `AppState::app_wide_retained_transcript_bytes` sums only **open** projects — its own doc
comment says so — so a cross-project "oldest" would be chosen from a partial list. The cost: the
app-wide figure can read exhausted while an older transcript sits in another open project, and that
refuses a `RequiredLocalBounded` launch under D4 rather than deleting someone else's data. That is §1.

**A failed deletion stops the budget pass and not the expiry pass** (decision 2, accepted). A budget
pass that continued would delete a newer transcript only because an older one could not be removed;
an expired transcript is independently past its limit whatever happens to its neighbours. **The
consequence is carried into PR-049-C:** a candidate whose deletion failed stays a candidate, is reached
again on every trigger, and stops the budget pass each time — so one undeletable file can keep
`project_budget_exhausted` true indefinitely and D4 refuses launches. C's refusal must distinguish *a
deletion failed* from *nothing is deletable*: two facts, two remedies.

**Expiry-before-budgets is observable as attribution, not as survivors.** Expiry and budget selection
share one age measure, so among transcripts that are not live every expired one is older than every
non-expired one, and both orders delete the same set — **when no deletion fails**; PR-049-B's model
did not vary failures. The order decides whether a removal is reported as `expired` or `budget` — the
distinction RFC-011's aggregate accounting asks for.
