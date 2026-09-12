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

- [ ] Seconds round-trip against `format_unix_seconds_utc`, covering **epoch, a leap day, and a
      pair where string comparison and correct arithmetic disagree**.
- [ ] Exactly at `max_age_days` is **not** expired (§1). **Ablation:** `>` → `>=`; this test
      alone fails.
- [ ] `Expired` keeps its bytes and still reports `has_retained_bytes()`.
- [ ] Every expiry decision takes `now` as a parameter — **no `now_utc()` inside any of them**
      (D7). Grep the new functions; a clock call is a defect even if the test passes.
- [ ] Nothing deletes in this slice.

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
