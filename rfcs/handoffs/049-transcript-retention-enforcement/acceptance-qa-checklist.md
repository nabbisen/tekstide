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

- [ ] **A live writer is never selected** — including when it is the only candidate and the app-wide
      budget is exhausted (§2). **Ablation:** remove the liveness filter; this test alone fails.
- [ ] Oldest-first by `last_write_at` falling back to `created_at`, proven with a pair that
      inverts under the wrong field.
- [ ] Expiry runs before byte-budget selection, asserted on **which transcript survived**.
- [ ] Cleanup routes through RFC-033's purge — the **tombstone and `Purged` state** are asserted,
      which a raw deletion would not produce (§3).
- [ ] No production caller exists yet.

## PR-049-C — triggers, record, refusal

- [ ] Both triggers fire; **no timer, watcher, or idle sweep exists anywhere** (D2). Grep for one.
- [ ] A policy cleanup writes `TranscriptPurge` as **`(AppPolicy, ExplicitCleanup)`**, and a user
      purge in the same file writes `(User, TrustedUi)` — **both read back from a real store**, so
      the distinction §4 requires is visible in one place.
- [ ] A cleanup that deleted nothing writes **no record**. **Ablation:** record unconditionally.
- [ ] `RequiredLocalBounded` on an unfreeable budget is refused **and no process starts** — assert
      the absent process, not the refusal value.
- [ ] The refusal's wording meets RFC-047 §5: states the fact, implies no danger, implies no fix
      available from there.
- [ ] **RFC-045's parser refuses `transcript_retention_days = 0`**, naming the transcript
      opt-out as the way to express what the user probably meant. Required at response 381:
      `is_bounded()` already treats `0` as *unbounded* while response 378 called it the tightest
      *reduce* — two opposite semantics for one value. PR-049-A resolved it toward keeping (§1),
      correctly; but a **value** the file accepts and the product ignores is RFC-045 D3′'s own
      failure one level down, and `transcript_capture_declined` already expresses zero retention.
- [ ] D5: the summary uses the session's configured limits. **Ablation:** restore
      `agent_run_default()`.

## Whole-RFC

- [ ] `cargo fmt`, `clippy --workspace --all-targets -D warnings`, `git diff --check`,
      `rfc_docs_invariants` clean.
- [ ] Three consecutive full-workspace runs, **output redirected to a file**, green; any recurring
      flake gets a dated row in `test-process-leak.md`.
- [ ] `cargo audit` still reconciles against `dependency-advisories.md`.
- [ ] **The changelog says the first run after upgrading deletes every transcript already older than
      the configured age** (§6). This is the sentence most likely to be left out, and RFC-045's
      changelog earned a required fix for exactly this class.
- [ ] **No text anywhere claims more than is enforced.** RFC-011's row and `crates/tekstide-core/README.md`
      both currently say retention is bounded per project and app-wide; after this slice that is true
      for the first time, and before it lands neither may be reworded to pretend it already was.
- [ ] RFC-011's Acceptance Criteria re-read against the result, and its row corrected to say which
      budgets shipped enforced (D1).

## Final Acceptance Decision

*(Section present from the start this time — RFC-046's and RFC-045's packs both shipped without one.)*

- [ ] Accepted.
- [ ] Accepted with required follow-up.
- [ ] Requires re-review after changes.

Reviewer notes:

```text
Pending review.
```
