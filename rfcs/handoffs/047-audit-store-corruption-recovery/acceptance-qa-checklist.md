---
title: "RFC-047 acceptance and QA checklist"
rfc: "RFC-047"
rfc_file: "../../done/047-audit-store-corruption-recovery.md"
source_rfc_status: "Implemented and closed 2026-09-09 — RFC-047 is in rfcs/done/"
target_milestone: "M12"
created: "2026-08-28"
---

# Acceptance and QA checklist

## The claim this RFC exists to be able to make

- [x] **A user can find out that the audit store is broken.** Proven against a real corrupted
      store in a scratch state root — the same reproduction RFC-036 PR-036-C used, which produced
      a screen indistinguishable from a healthy one. Now: *"Audit: the previous audit file could
      not be read. It was moved to `<path>` and a new one was started."* (`EVIDENCE-1`). D4's own
      per-action confirmations are a further, separate claim (PR-047-C, not yet built) — this box
      is about the claim in its own right, which stands on D1–D3 alone.

## PR-047-A — the seam

- [x] `open_audit_store` distinguishes its failure reasons; `RecoveryIncomplete` is separable.
      `AuditStoreOpenFailure::Store(AuditStoreErrorReason)` carries the real reason;
      `AuditStoreOpenFailure::Environment` for the two failures that happen before
      `AuditStore::open` is ever reached.
- [x] `AuditHealth` is stored on `State` and **accumulates across a session**. All fourteen former
      construction sites
      accounted for, each checked rather than assumed — twelve already had `&mut State`, one
      (`record_new_project_added`) widened from `&State` after checking its three callers, two
      (`main.rs`'s `boot()`, `State::new`'s own demo-panes launch) thread a real value through
      from before `State` exists rather than starting fresh. Read by the four tests calling
      `status()`/`failure_count()`/`last_failure()`; the **first production reader is the board
      indicator in PR-047-B**.
      *(Corrected 2026-08-28. This box said "and **read**", contradicting the task breakdown, which
      assigns the only production reader to B — A could not meet it as written. The implementer's
      own annotation named the diagnostic line as the first reader; it is not: the `eprintln!` reads
      the local `reason` before `record_failure`, never the accumulated health. Left explicit so B
      does not lose its own box to a reader that was never there.)*
- [x] A failure to open leaves a trace a technical user can find. One `eprintln!` line,
      unconditional (not gated behind `cfg!(debug_assertions)`), confirmed live against both
      corruption shapes with the release binary.

## D1 / D2 — recovery

- [x] `RecoveryIncomplete` resumes once per session. `resume_and_reopen`; "once per session" falls
      out for free from a successful recovery leaving a genuinely working store on disk.
- [x] Any other open failure recovers. `recover_and_reopen`; `recover()`'s own diagnostic guard
      safely refuses anything not actually diagnosed corrupt.
- [x] **In both cases the `AuditStoreRecovery` record is read back out of the store**, not inferred
      from a return value. `open_audit_store_recording_failure_resumes_and_records_the_recovery`
      queries the reopened store directly.
- [x] **The quarantined file still exists**, and its **path is what the product reports**. This is
      the condition D2 rests on — without it the decision was wrong.
      `open_audit_store_recording_failure_recovers_a_corrupt_store_and_reports_the_quarantine_path`,
      ablated (a fake reported path made the file-existence check fail correctly).
- [x] Recovery that itself fails leaves `AuditHealth` degraded, not reporting success.
      `..._leaves_health_degraded_when_recovery_itself_fails`, against a real refusal (a symlinked
      `recovery` directory), not simulated.
- [x] **A recovery that succeeds but cannot confirm its own record write still discloses the
      quarantine path, and the board does not say "not recording".** *(Added 2026-09-02, response
      358 required R1/R2 -- the original box above tested a different arm, `Failed`, not
      `recovery_event_recorded: false` on a successful recovery, which is where R1's defect
      actually lived.)* `AuditHealth::record_recovery` is now disclosure-only; a new
      `clear_degraded()` carries the status reset separately (§3.1 of the risk document).
      `apply_recovery_outcome_stays_degraded_and_still_discloses_when_the_record_is_unconfirmed`,
      against a real recovery driven through the `test-support` seam
      `recover_and_reopen_forcing_unrecorded_event_for_test` (the one input a black-box test
      cannot trigger organically, documented there as simulated). Ablated twice, independently:
      reverting either half of the fix fails a distinct assertion.
- [x] Nothing in this slice calls `fs::remove_*` on a user's audit data. `recover()`/`resume()`
      themselves are `fs::rename`, unmodified by this slice; the new code in `tekstide` only reads
      `AuditRecoveryOutcome` and never touches the filesystem directly.

## D3 — the indicator

- [x] Present when degraded. `project_board_audit_lines_shows_the_degraded_line_when_degraded`;
      confirmed live (`EVIDENCE-2`).
- [x] **Absent when healthy**, with its own test, ablated separately — deleting that assertion must
      fail on its own. `project_board_audit_lines_is_empty_when_healthy_and_never_recovered` and
      `..._shows_the_quarantine_path_when_recovered` (confirms the degraded line is specifically
      *absent* once a recovery succeeds) — both ablated together (forced the degraded line
      unconditionally) and both failed independently, not only in combination.
- [x] **The generic degraded line must not appear when a recovery has already been disclosed this
      session** (§3.1, added 2026-09-02, response 358 required R1) — that combination is false: a
      returned, working store is not "not recording". `project_board_audit_lines_shows_the_
      collision_line_not_the_generic_degraded_line`, ablated: reverting the wording fix fails both
      "must show the new line" and "must not show the generic line" independently.

## D4 — say it before the click

- [x] The agent-launch and trust-grant confirmations state the action will not be recorded, **while
      the control is live**. Trust grant: appended to `trust_grant_dialog_body`, rendered above the
      still-live Grant button. Agent-run launch: `agent_run_launch_audit_notice`, rendered above the
      still-live "Launch AI CLI Run" button in `trust_settings_view` (no confirmation modal exists
      for this control, unlike trust grant).
- [x] The wording does not imply the action is unsafe, does not imply the user can fix it from
      there, and does not appear when healthy, **and states the one fact D4 requires** (added
      2026-09-05, response 360 required R2 -- the original assertions were negative-only and let
      `"Audit note."` pass all six PR-047-C tests, confirmed by reproducing exactly that before
      fixing it). `trust_grant_dialog_degraded_notice_does_not_imply_unsafe_or_fixable` /
      `agent_run_launch_audit_notice_does_not_imply_unsafe_or_fixable`, checked against the real
      catalog strings, now asserting both the required content and the absence of the wrong content.
- [x] Present-when-degraded and absent-when-healthy are **separately** ablated. Four tests (one pair
      per surface), each direction ablated independently and run by me — see `qa-evidence.md`.

## Live GUI evidence

- [x] Against a **`mktemp -d` fixture with a fresh `XDG_STATE_HOME`**, using RFC-036 PR-036-C's own
      corruption method.
- [x] **Not captured — closed as a documented, investigated gap, per the reviewer's own bound**
      (2026-09-06, response 363, attempt 6/final). Reviewer identified the real remaining doubt: the
      screenshot pipeline goes through a clipboard (`niri msg action screenshot-window` returns
      `rc=0` while writing nothing), and no prior attempt had verified it returns a *fresh* frame
      rather than a stale one. Verified in order: (1) the clipboard does receive a real capture;
      (2) two captures around `wtype "ZZZ"` on the same window are byte-identical, matching the
      reviewer's own "stale" outcome; (3) — going one step further — two captures around a real,
      compositor-driven change (`fullscreen-window`, independent of `wtype`) **do differ**, proving
      the pipeline is not globally stuck; (4) re-running the actual D4 capture (corrupted-audit
      fixture, `Ctrl+Alt+U` then `Ctrl+Alt+T`, both large enough to be unmissable if they landed)
      against this now-proven-live pipeline still shows **no difference at all**. The stale-capture
      doubt is closed in the direction that confirms five attempts of prior readings, not the
      direction that overturns them. Per the reviewer's own explicit bound ("the next request either
      carries the capture or records option 3 as decided"): **option 3, decided.** D4 is accepted on
      the ablated unit-test evidence in this handoff; live GUI capture is a documented, thoroughly
      investigated, not-resolved gap, not a silently dropped requirement.
- [x] Whether a real mouse click was sent is stated either way. Zero mouse clicks for EVIDENCE-1/2;
      the D4 capture attempt used zero mouse clicks too (none available) and produced no usable
      screenshot, as stated above.

## Gates

- [x] `fmt`, `clippy -D warnings`, `git diff --check`, `rfc_docs_invariants`. All clean.
- [x] Full workspace suite, **three consecutive runs**, each logged to a file; any flake given a
      **row** in the register, not a mention. **468 + 4 + 741, fully green** every time — no flake
      this pass. *(Re-run 2026-09-02 after response 358's R1/R2 fix, two new tests added:
      **470 + 4 + 741, fully green** every time — no flake this pass either. Re-run again same day
      after PR-047-C, six more new tests: **476 + 4 + 741, fully green** every time — no flake.
      Re-run 2026-09-09 after PR-047-D, six more new tests in `tekstide` and two in `tekstide-core`:
      **482 + 4 + 743, fully green** every time — no flake. Re-run again same day after response
      365's R1 wording fix, one more new test: **483 + 4 + 743, fully green** every time — no
      flake.)*

## The outcome this slice must not reach

- [x] **PR-047-D is done.** `AuditHealth::status` is not a latch: `open_status`/`write_status` are
      separate fields, each cleared only by the success that cures it (`clear_open_failure`/
      `clear_write_failure`); `failure_count`/`last_failure` survive as session history untouched by
      either; the board renders present-tense (`status()`) and history (`failure_count() > 0`) as
      independent lines (§3.2). **Ablated, each direction separately, all run by me**: removing the
      open-success clear fails only the "clears" test; zeroing history inside `clear_open_failure`
      fails only the "preserves history"/recovery-path tests; collapsing `clear_open_failure` into
      also clearing `write_status` (the literal naive fix) fails only the "write survives a
      successful open" test; removing the board's history-line block fails only the history-line
      test. See `qa-evidence.md`.
- [x] **The history line names what `failure_count` actually counts** (added 2026-09-09, response
      365 required R1, §4.1 of the risk document) — record write attempts, not user actions; several
      best-effort producers for one action (closing a project with several terminals) do not
      short-circuit each other, so an action-level word would overstate. `project-board-audit-history`
      says "record(s)... were not written". `project_board_audit_history_names_records_not_actions`,
      ablated: reverting to "action(s)... recorded" fails, naming the exact overclaim.

- [x] **PR-047-C is done.** D1–D3 connected in A/B; D4's own confirmations built and tested here,
      not left as the promise D1–D3 alone would have felt like keeping.

## Final Acceptance Decision

- [x] **Accepted.** 2026-09-09, by the architect.
- [ ] Accepted with required follow-up.
- [ ] Requires re-review after changes.

Reviewer notes:

```text
Accepted on the evidence, ablations and gate runs reproduced independently by the reviewer at
each of responses 357, 358, 359, 360, 361, 365 and 366 -- not on the implementer's report of
them. Final gate re-run on the closed tree: 483 + 4 + 743, green three times.

What this RFC actually delivered, against what it set out to do:

- The defect it was written for is fixed. A corrupted audit store no longer fails silently: it
  is detected, quarantined by rename rather than deleted, recovered, and the quarantine path is
  named on screen. RFC-036 PR-036-C reproduced this same corruption against the release binary
  and the interface reported "Calm".
- D4 held its line. A degraded audit store does not refuse an agent run; it says, before the
  click, that the run will not be recorded. Restricted Mode refuses what it cannot bound; an
  unrecorded action is not that.

Three things this RFC should be remembered for, none of them in its own D1-D4:

1. PR-047-D was not planned. It was found in PR-047-B's review as a deeper instance of the
   defect S3.1 had just fixed, and it was a real, reachable bug: a transient failure degraded a
   session permanently while recording worked fine.
2. The same class of defect recurred three times (S4.1) -- a sentence or number naming something
   adjacent to what was measured. Every instance passed its tests, because each test asserted a
   line was present, never that it was true. That is the lasting finding, and it is why S4.1 is
   written as a rule rather than a war story.
3. PR-047-C's live-capture evidence gap consumed four review rounds and was never resolved. The
   cause was the reviewer testing three hypotheses about the input path without once checking
   whether the measurement instrument was live. D4 is accepted on ablated unit tests, and the
   delivery plan now bounds evidence capture at three rounds because of it.

Reviewer error recorded rather than omitted: the PR-047-A checklist box contradicted the task
breakdown and was mine; the S3/S4 collision that produced PR-047-B R1 was an unresolved gap in
my own risk document; and the four-round capture chase above was mine to stop sooner.
```
