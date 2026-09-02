---
title: "RFC-047 acceptance and QA checklist"
rfc: "RFC-047"
rfc_file: "../../accepted/047-audit-store-corruption-recovery.md"
source_rfc_status: "Accepted 2026-08-28 — M12"
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
      there, and does not appear when healthy. `trust_grant_dialog_degraded_notice_does_not_imply_
      unsafe_or_fixable` / `agent_run_launch_audit_notice_does_not_imply_unsafe_or_fixable`, checked
      against the real catalog strings.
- [x] Present-when-degraded and absent-when-healthy are **separately** ablated. Four tests (one pair
      per surface), each direction ablated independently and run by me — see `qa-evidence.md`.

## Live GUI evidence

- [x] Against a **`mktemp -d` fixture with a fresh `XDG_STATE_HOME`**, using RFC-036 PR-036-C's own
      corruption method.
- [ ] **Not captured.** Blocked by the synthetic-input environment, not by the feature: `wtype`
      (this project's own documented tool) delivers correctly to a freshly spawned native Wayland
      client in this session (confirmed against `alacritty`) but no keybinding reached the Tekstide
      window across repeated attempts, including after explicit refocus and after relaunching
      through `niri msg action spawn-sh`; no mouse-input tool is available either. Full account in
      `qa-evidence.md`'s PR-047-C section. The degraded state itself is confirmed live and genuine
      (the D3 board line renders correctly against the real corrupted fixture); the D4 confirmations
      are proven only by the ablated unit tests.
- [x] Whether a real mouse click was sent is stated either way. Zero mouse clicks for EVIDENCE-1/2;
      the D4 capture attempt used zero mouse clicks too (none available) and produced no usable
      screenshot, as stated above.

## Gates

- [x] `fmt`, `clippy -D warnings`, `git diff --check`, `rfc_docs_invariants`. All clean.
- [x] Full workspace suite, **three consecutive runs**, each logged to a file; any flake given a
      **row** in the register, not a mention. **468 + 4 + 741, fully green** every time — no flake
      this pass. *(Re-run 2026-09-02 after response 358's R1/R2 fix, two new tests added:
      **470 + 4 + 741, fully green** every time — no flake this pass either. Re-run again same day
      after PR-047-C, six more new tests: **476 + 4 + 741, fully green** every time — no flake.)*

## The outcome this slice must not reach

- [ ] **PR-047-D is done.** `AuditHealth::status` is not a latch: each kind of failure is cleared
      by the success that cures it, open and write failures are distinguishable, `failure_count`/
      `last_failure` survive as session history, and the board renders present-tense and history as
      independent lines (§3.2). **Ablation:** a transient `record_failure` followed by a successful
      open must leave the board showing no present-tense "not recording" line, and must still show
      the history line — both directions checked separately, not only in combination.

- [x] **PR-047-C is done.** D1–D3 connected in A/B; D4's own confirmations built and tested here,
      not left as the promise D1–D3 alone would have felt like keeping.

## Final Acceptance Decision

- [ ] Accepted.
- [ ] Accepted with required follow-up.
- [ ] Requires re-review after changes.

Reviewer notes:

```text
Pending review.
```
