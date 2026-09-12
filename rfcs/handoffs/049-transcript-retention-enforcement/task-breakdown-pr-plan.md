---
title: "RFC-049 — task breakdown and PR plan"
rfc: "RFC-049"
rfc_file: "../../accepted/049-transcript-retention-enforcement.md"
source_rfc_status: "Accepted 2026-09-12 — M12"
target_milestone: "M12"
created: "2026-09-12"
---

# Task breakdown and PR plan

**A → B → C. Nothing deletes until C.**

## PR-049-A — arithmetic and marking (core only, no deletion)

**D9 first**, because everything else needs it. `DomainTimestamp` is a formatted UTC string with
`now_utc`, `from_utc_string`, `as_str` — and no way back to seconds.

- Add the inverse of `format_unix_seconds_utc` to `domain::time`. **Compare in seconds, never by
  string ordering**: it happens to work for this format and stops the moment the format changes.
- Expiry marking: a transcript past `max_age_days` becomes `TranscriptRetentionState::Expired`
  and **keeps its bytes** (D3). `has_retained_bytes()` already returns `true` for `Expired` and
  must keep doing so — no existing caller changes meaning.
- Every function deciding expiry takes `now: &DomainTimestamp` (D7). No internal clock call.

**Required tests, each ablated separately:**

- Round-trip against `format_unix_seconds_utc`, including **epoch, a leap day, and a pair where
  naive string comparison and correct arithmetic disagree** — that last one is the test that makes
  the string-ordering shortcut fail loudly if someone takes it later.
- Exactly at the limit is **not** expired (§1). Ablate by changing `>` to `>=`.
- `Expired` still reports retained bytes.
- A fixed `now` gives the same answer every run — the point of D7, and it should be obvious from
  the signature that wall-clock time cannot reach it.

## PR-049-B — selection and cleanup (core only, no production caller)

- **Liveness** (**D8′**, replacing D8 — read it before anything else in this slice): the authority
  is the transcript's `AgentRun.status` via `Transcript.agent_run_id`, **not** `lifecycle_state`,
  which production never moves off `Active`. A **new predicate**, matching **exhaustively** with
  only `Completed | Failed | Cancelled` not-live, so a future `AgentRunStatus` variant fails to
  compile rather than becoming deletable. **`Detached` is live.** Do not reuse
  `agent_run_status_is_active` or `agent_run_status_blocks_strong_association`.
- **Liveness gates marking too** (D8′-a): a live transcript is neither marked `Expired` nor
  selected. PR-049-A's arithmetic is unchanged; nothing calls it for a live transcript.
- **Selection**: **a live writer is never a candidate**, at any pressure (§2). Order by
  `last_write_at` falling back to `created_at` — in production always the latter, which is correct
  because a live run is never reached.
- **Bytes** (D8′-b): `ProjectSession::real_retained_transcript_bytes()`. **Never
  `Transcript.byte_count`**, which is `0` in production.
- **Cleanup** calls **RFC-033's existing purge** (§3, D3): bytes deleted, state `Purged`,
  content-free tombstone preserved. Expiry-driven and budget-driven cleanup share this.
- **Order**: expiry first, then byte budgets — expiring may free enough that no budget cleanup runs,
  and the reverse can delete a wanted transcript while an expired one sits beside it.
- Budget exhaustion behaviour per D4, which is RFC-011's decision, not a new one.

**Required tests, each ablated separately:**

- **A transcript with a live writer is not selected when it is the only candidate and the app-wide
  budget is exhausted.** This is §2's whole content; ablate by removing the liveness filter and
  watch this test alone fail. **Drive liveness through a real `AgentRun` whose status production
  actually sets** — a test that assigns `lifecycle_state` directly is the defect D8′ exists to
  remove, reproduced in the test file.
- **A `Detached` run's transcript is not selected** (D8′), and **a `Completed` one is** — one test
  each, so the boundary is visible rather than inferred.
- **A live transcript is not marked `Expired`** even when its age exceeds the limit (D8′-a). This is
  the forty-day-live-writer case; ablate by moving the liveness check after the marking.
- Oldest-first order, with a `last_write_at`/`created_at` pair that inverts under the wrong field.
- Expiry runs before byte selection: a case where running them the other way deletes a different
  transcript, asserted on **which** transcript survived.
- Cleanup routes through RFC-033's purge — assert the **tombstone and state**, which a raw
  `remove_file` would not produce.

## PR-049-C — the triggers, the record, the refusal

**Partly paused, 2026-09-13 (request 387).** A session knows only the transcripts launched in the
current process, so the triggers, the removal disclosure, D4′'s exhaustion path and §6's changelog
sentence would act on almost nothing. They wait for loading transcripts from disk, to be scoped as
its own RFC. **Land now:** refusing `transcript_retention_days = 0`; the actor/source pairing and
no record for an empty cleanup; stale `Expired` marks re-checked and cleared; "a deletion failed"
reported separately; D5 as restated below; and the disclosure of the purge defect.

- **Two triggers** (D2): agent-run launch preflight, and project open. **No timer.**
- **Audit** (D6): `(AppPolicy, ExplicitCleanup)` on `TranscriptPurge`, via
  `transcript_purge_record` gaining the pairing as a parameter — not a second constructor.
  **A cleanup that deleted nothing writes nothing** (§4).
- **D4′ (response 386), replacing the refusal:** when launch cleanup leaves a budget exhausted, the
  run starts with capture disabled; the existing launch confirmation says so, worded under RFC-047 §5;
  the decision shown is the decision applied; the run's detail says why it has no transcript,
  distinctly from opt-out. `RequiredLocalBounded`'s core refusal stays a unit test, labelled
  unreachable. **The parts that do not depend on D4′ may land first as their own commit.**
- **D5**: `transcript_local_data_summary_for` takes the session's configured limits.

**Required tests:**

- A real launch after a real expiry writes a `TranscriptPurge` record **read back from a real
  store** with `AppPolicy`/`ExplicitCleanup` — and a user purge in the same test writes
  `User`/`TrustedUi`. **Both pairings, one test file, so the distinction is visible.**
- A cleanup that deletes nothing writes **no** record. Ablate by recording unconditionally.
- Exhausted at launch: the process **starts** and **no transcript exists** for it — assert both.
- The confirmation's notice is present when exhausted and absent otherwise, each ablated alone.
- D5: implemented, **and stated as unobservable** — `budget_pressure` reads only the byte budgets,
  none of which is configurable, so no configured value can move it (request 387). No test pretends
  otherwise.

**Evidence:** unit-level, plus one live capture of the launch confirmation naming a run whose output
will not be kept (D4′; the refusal it replaced is unreachable), against a `mktemp -d`
fixture with `XDG_CONFIG_HOME` and `XDG_STATE_HOME` both throwaway. Per `ARCHITECTURE.md`, **try
`wtype` before assuming the documented capture gap** — it reached the application first try at
PR-045-C. Bounded-evidence rule applies: three rounds, then the store read-back carries it.

## Not in this plan

Everything the pack README lists. And **no user-facing text says transcripts are removed until C**
(§6) — A marks, B is unreachable from production.
