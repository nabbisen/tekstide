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

- **Selection** (D8): inactive by `lifecycle_state`; **a live writer is never a candidate**, at
  any pressure (§2). Order by `last_write_at`, falling back to `created_at`.
- **Cleanup** calls **RFC-033's existing purge** (§3, D3): bytes deleted, state `Purged`,
  content-free tombstone preserved. Expiry-driven and budget-driven cleanup share this.
- **Order**: expiry first, then byte budgets — expiring may free enough that no budget cleanup runs,
  and the reverse can delete a wanted transcript while an expired one sits beside it.
- Budget exhaustion behaviour per D4, which is RFC-011's decision, not a new one.

**Required tests, each ablated separately:**

- **A transcript with a live writer is not selected when it is the only candidate and the app-wide
  budget is exhausted.** This is §2's whole content; ablate by removing the liveness filter and
  watch this test alone fail.
- Oldest-first order, with a `last_write_at`/`created_at` pair that inverts under the wrong field.
- Expiry runs before byte selection: a case where running them the other way deletes a different
  transcript, asserted on **which** transcript survived.
- Cleanup routes through RFC-033's purge — assert the **tombstone and state**, which a raw
  `remove_file` would not produce.

## PR-049-C — the triggers, the record, the refusal

- **Two triggers** (D2): agent-run launch preflight, and project open. **No timer.**
- **Audit** (D6): `(AppPolicy, ExplicitCleanup)` on `TranscriptPurge`, via
  `transcript_purge_record` gaining the pairing as a parameter — not a second constructor.
  **A cleanup that deleted nothing writes nothing** (§4).
- **`RequiredLocalBounded` preflight refusal** when the budget cannot be freed: wording under
  RFC-047 §5 — state the fact, do not imply the user can fix it from there, do not imply danger.
- **D5**: `transcript_local_data_summary_for` takes the session's configured limits.

**Required tests:**

- A real launch after a real expiry writes a `TranscriptPurge` record **read back from a real
  store** with `AppPolicy`/`ExplicitCleanup` — and a user purge in the same test writes
  `User`/`TrustedUi`. **Both pairings, one test file, so the distinction is visible.**
- A cleanup that deletes nothing writes **no** record. Ablate by recording unconditionally.
- `RequiredLocalBounded` is refused, and **no process starts** — assert the absence of the
  process, not just the refusal value.
- D5: the summary's `budget_pressure` changes when the configured limit changes. Ablate by
  restoring `agent_run_default()` and watch it alone fail.

**Evidence:** unit-level, plus one live capture of the preflight refusal against a `mktemp -d`
fixture with `XDG_CONFIG_HOME` and `XDG_STATE_HOME` both throwaway. Per `ARCHITECTURE.md`, **try
`wtype` before assuming the documented capture gap** — it reached the application first try at
PR-045-C. Bounded-evidence rule applies: three rounds, then the store read-back carries it.

## Not in this plan

Everything the pack README lists. And **no user-facing text says transcripts are removed until C**
(§6) — A marks, B is unreachable from production.
