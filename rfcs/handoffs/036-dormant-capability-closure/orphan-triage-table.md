---
title: "RFC-036 PR-036-A: the orphan triage table"
rfc: "RFC-036"
rfc_file: "../../done/036-dormant-capability-closure.md"
source_rfc_status: "Implemented and closed 2026-08-28 — RFC-036 is in rfcs/done/"
target_milestone: "M12"
created: "2026-08-27"
---

# The orphan triage table

**No code change in this document.** Per the task breakdown: the table is the deliverable, and
nothing is deleted or wired on the strength of a table nobody has reviewed.

## Measurement method, stated once, used for every row below

Same method the original audit used (`reachability-audit.md`), re-run fresh against today's tree
rather than trusted from the RFC's own ten-day-old list, per D0:

1. Mark the candidate `pub fn` (or `pub struct`, for `ConfigStore`) with
   `#[deprecated(note = "reachability-audit")]`.
2. `cargo build -p tekstide` — library **and** binary, not `--all-targets`, so test code never
   compiles and cannot count as a caller. Every warning names a real call site; the absence of one
   means zero callers in either crate's non-test code.
3. This single build necessarily compiles `tekstide-core` first, so one run gives **both**
   production-in-`tekstide` and `tekstide-core`-internal counts at once — a call from one marked
   item into another marked item does not always suppress the warning (confirmed directly: see
   `record_transcript_write_summary` below), so the internal count is real, not inferred.
4. For every item with zero warnings, `grep -rn '\.fn_name(\|::fn_name(' crates/tekstide-core/src
   crates/tekstide/src --include='*.rs'` restricted to `tests.rs`/`/tests/` paths gives the
   test-only count.
5. Revert the markers (`git checkout -- <files>`); `git diff --stat` confirmed empty before writing
   this document.

35 items marked across 12 files, in one build. Full warning list retained in this slice's own
session record; not reproduced verbatim here since the table below states, per row, exactly what
each warning did or did not show.

**Counts below are always stated as `production in tekstide / tekstide-core-internal /
test-only`.** A `0` in the first two positions and non-zero in the third is not "no callers" — see
§3 of the risk document.

## D0 — corrections to the RFC's own list, found by re-verifying rather than trusting it

The RFC's own D0 section (ten days newer than the audit, still nine days old by the time this
table was built) had its own count already drift:

- **`request_terminate`: 1, not 3.** The RFC's D0 claims three production callers (RFC-039/043).
  Measured today: the core function (`runtime::terminal::termination::request_terminate`) has
  exactly **one** call site anywhere in either crate's non-test code —
  `crates/tekstide/src/surface/terminal.rs:499`, itself inside `TerminalPane::request_terminate`
  (a same-named wrapper in the `tekstide` crate), which itself has exactly **one** caller:
  `crates/tekstide/src/shell.rs:4136`, the project-close path. `RunningTerminal`'s own `Drop`-path
  cleanup (`runtime/terminal/launch.rs`) is explicitly documented there as *not* going through
  `request_terminate` — a deliberate, different, bounded escalation. Still not an orphan (1 > 0),
  so no verdict changes — the count itself is what is corrected here, in the same spirit D0 already
  corrected the audit's own list.
- **`shutdown` (`approval::channel`): 0/0/3, not "1."** The RFC's D0 table lists `shutdown: 1`
  without naming which of the two functions named `shutdown` it means. `TerminalReader::shutdown`
  (a function outside the original 30-orphan list entirely) has exactly one production caller
  (`surface/terminal.rs:497`) and one internal `Drop`-path caller — that is very plausibly what D0
  actually counted. `approval::channel::shutdown` (the one the *original* audit named, and the one
  this triage is actually about) has **zero** callers anywhere outside its own three tests. Its
  classification is unchanged either way: see below.
- **`switch_active_project`: wired, and not listed as assigned.** The RFC's own "What is already
  assigned" section does not mention it, yet it has a real production caller —
  `crates/tekstide/src/shell.rs:3740`, wired by **RFC-039 PR-039-B** ("real project tabs — switch
  by click or keyboard"), confirmed via `git log -S`. Not an orphan; an omission in the RFC's own
  bookkeeping, corrected here rather than re-triaged.
- **`transition_change_set_review_state`: wired. This entry was wrong, corrected at review 354.**
  The original text here concluded "0 production callers" from finding only one internal caller
  named `app.rs:464`, and stopped without checking whether *that* caller itself had a production
  caller. It does: the internal caller is `AppState::transition_active_project_change_set_review_state`
  (`app.rs:452`) — a wrapper RFC-034 added under a **different name** than the core function it
  calls, not a reuse of the core function's own name — and it is called for real from
  `record_change_review_decision` (`crates/tekstide/src/shell.rs:8783`), the actual "Mark
  accepted / Mark rejected" handler. **The count (0 production callers *of the exact core
  symbol*) was correct; the conclusion drawn from it — "therefore unreached" — was not**, because
  a differently-named intermediate wrapper is exactly what a bare compiler-warning count cannot
  see past on its own; it takes one more grep, of the wrapper's own name, which this row skipped.
  Every other row with an internal-only caller in this table's first pass had that same wrapper
  independently marked `#[deprecated]` too (`switch_active_project`, `purge_project_transcripts`),
  so its own production reachability was checked directly rather than assumed — re-verified after
  this correction was found, and confirmed this was the only row with the gap. RFC-034's assignment
  stands, now correctly: it *did* wire this function, through a wrapper it built for the purpose,
  not through a separate ephemeral mechanism as first (wrongly) concluded here.

## The two that already left the triage (D3) — reconfirmed, not re-decided

`recover` (0/0/3), `resume` (0/0/5), `purge_all_records` (0/0/6): all three zero in both
production and internal use today, matching D3's own finding. **`purge_project_records` (0/0/2)
folded into the same defect slice**, not given a separate wire/delete/document verdict: it is the
same file (`audit/purge.rs`), the same "data-deletion path that has never run from the
application" shape D3 already names for `purge_all_records`, differing only in scope (one project
vs. all) — treating the narrower one as a separate triage row while the RFC pulls the broader one
out entirely would be an arbitrary split of one finding into two. PR-036-C's own reproduction
(corrupt a store, run the release binary) should cover both purge paths and both recovery paths in
one pass, not four.

**Search shape (D4): none of these four would be found by "does anything call it" alone once
`AuditRecovery`/`AuditStore` are constructed at all** — they are reachable *types*, just never
invoked from application code. The shape that would find this category is "does a GUI action exist
for every `Result`-returning method on a type the GUI already constructs," which is closer to
D4's second named category (paths that never run) than the first.

## RFC-045-conditioned — D2 already decided, restated here for completeness

All four: **keep, documented, consumer RFC-045 (reserved).**

| Item | Counts | Search shape |
| --- | --- | --- |
| `set_resource_limits` | 0 / 0 / **8** test-only (`project/tests/metadata.rs`, `collections.rs` ×3, `agent/tests.rs`, `shell/tests.rs` ×2 in `tekstide-core`, plus 2 in `tekstide`'s own `shell/tests.rs`) | `grep -rn '\.set_resource_limits('` |
| `ConfigStore` (struct) | 0 / 0 / 11 test-only references | `grep -rn 'ConfigStore'` outside `tests/` |
| `to_ai_cli_profile` | 0 / 0 / 4 test-only | `grep -rn '\bto_ai_cli_profile('` |
| `record_sensitive_config_policy_increase` / `_reduce` | 0 / 0 / 2 each | `grep -rn '\.record_sensitive_config_policy_'` |

**Correction to the RFC's own text**: `set_resource_limits` was described as having "exactly one"
test caller (RFC-031's discrimination test). It now has eight across both crates — the general
rule in §2 of the risk document (a test-only caller is a reason to look harder, not discount the
count) applies more, not less, than when the RFC was written. Still conditioned on RFC-045, not
re-decided.

## Confirmed benign — superseded by a real, different, already-wired path

**Not "own RFC" rows** — each was checked directly, not merely assumed benign the way the audit's
own "likely benign" hedge left open. Verdict: **delete**, batched into `0.16.0` per D1, each with
what the real path is instead so the *why* survives the function's own removal (§2).

| Item | Counts | The real path instead | Search shape |
| --- | --- | --- | --- |
| `add_agent_run` (`project::session`) | 0 / 0 / 12 test-only | `ProjectSession::attach_agent_launch_plan` (`session.rs:506`) does the real `self.agent_runs.push` at the one live launch site | `grep -rn '\.add_agent_run('` |
| `add_transcript` (`project::session`) | 0 / 0 / 9 test-only (incl. 1 in `tekstide`'s own `shell/tests.rs`) | `ProjectSession::attach_agent_run_transcript` (private, `session.rs:920`) does the real push | `grep -rn '\.add_transcript('` |
| `add_audit_event` (`project::session`) | 0 / 0 / 6 test-only | `ProjectSession::grant_trust`/`revoke_trust` (`pub(crate)`, RFC-032) are the real writers of the same `audit_events` collection — checked directly: they do write to it, in production, today | `grep -rn '\.add_audit_event('` |
| `add_approval` (`domain::agent::AgentRun`) | 0 / 0 / 4 test-only | `ProjectSession::add_approval_request` (`session.rs:720`, real production caller at `shell.rs:7311`) is the real approval-attachment path. **Checked, not assumed**: `add_approval_request` does *not* call `AgentRun::add_approval` internally either — `AgentRun.approval_ids` is written only by its own dead method and read nowhere in either crate, so it is inert rather than actively wrong (no reader trusts stale data the way `open_surface` once did) | `grep -rn '\.add_approval\(' `, excluding `add_approval_request` |
| `shutdown` (`approval::channel`, explicit) | 0 / 0 / 3 test-only | `ServeShutdown`'s own `Drop` calls the identical private `shutdown_and_join` this method calls — checked directly: same function, not merely equivalent-looking. The explicit method exists so a caller can wait for shutdown at a chosen point rather than at scope exit; nothing in production needs that determinism | `grep -rn '\.shutdown\(\)'` on a `ServeShutdown`-typed value |
| `accept_proposal` (`approval::channel`) | 0 / 0 / 12 test-only | `serve_concurrently`'s own internal accept loop is what production uses; this single-connection method predates it | `grep -rn '\.accept_proposal('` |

`shutdown` and `accept_proposal` are not new findings — the original audit already called both
"superseded, not orphaned... not a capability gap," and D0's re-verification found nothing to
overturn that. Recorded here with fresh counts rather than re-litigated, since D0 asks every row to
be re-checked, not re-argued once confirmed.

**`record_terminal_transcript_write_summary` / `record_transcript_write_summary`** (`project::session`)
— 0 production / **1 internal** (the former calls the latter — confirmed directly by the warning
firing even though both are marked deprecated, so the "deprecated code may call deprecated code
silently" concern does not apply here) / 2 and 2 test-only respectively. **Verdict: delete, both
together** — not "own RFC," because the RFC's own body has already made the case against wiring
them: RFC-033 PR-033-C needed a real retained-bytes figure and *deliberately* went a different
route (`fs::metadata`, correct retrospectively, matching `remove_transcript_file`'s own source),
specifically because a tracked counter is only ever correct prospectively and every transcript
written before the wiring would show `0`. There is no future version of "wire it" that would not
repeat that same mistake. Search shape: `grep -rn '\.record_terminal_transcript_write_summary\('`.

**`set_runtime_summary`** (`project::session`) — **not a triage row.** `#[cfg(test)]`-gated,
re-confirmed still true today; the original audit's own correction ("candidate-list error, not a
finding") stands.

## Own RFC — real design, correctly not absorbed into this slice

Per §6 of the risk document, each of these is a **finished** row: a verdict, stated plainly, with
the number left blank because none is reserved (unlike the RFC-045 cluster above, which has one).

| Item | Counts | What it would take | Search shape (D4) |
| --- | --- | --- | --- |
| `purge_agent_run_transcripts` | 0 / 0 / 1 test-only | A UI entry point at the single-agent-run granularity; RFC-033 built the project-level one only. A narrow follow-up to RFC-033, not a redesign | `grep -rn '\.purge_agent_run_transcripts('` |
| `set_viewport` | 0 / 0 / 1 test-only | Real scroll-input handling (wheel and/or drag) wired to it — the editor, as shipped, has no scrolling at all. Deciding the input model is design, not glue | `grep -rn '\.set_viewport('` |
| `set_git_summary` (+ `git_summary()` getter, 0 non-test readers, re-confirmed) | 0 / 0 / 1 test-only | A real git-status probe (spawning `git`, or reading `.git` directly) — security-relevant given this project's own established caution around subprocess spawning and `.git/` supervision | `grep -rn '\.set_git_summary(\|\.git_summary('` |
| `set_warning_state` | 0 / 0 / 1 test-only | Deciding what actually populates `ProjectWarningState` in production; only its `has_risk_warning()` boolean currently leaks through `runtime_summary`, and nothing produces a real warning to leak | `grep -rn '\.set_warning_state('` |
| `decide_with_edited_argv` | 0 / 0 / 4 test-only | An "edit and approve" control in the approval dialog (currently Approve-once / Reject only) plus the protocol wiring RFC-021 already supports | `grep -rn '\.decide_with_edited_argv('` |

## The finding worth more than any single row: agent-run launches are not durably audited

**Confirmed a defect at review 354, not left as an open question.** The first pass here handed the
"is this a defect or a written-nowhere scope choice" question to whoever authors the recommended
RFC. The reviewer answered it directly and the answer is defect, on three independent grounds, none
of which needed new measurement beyond what this row already found:

1. **The asymmetry cannot be a choice.** This product audits a plain terminal start
   (`record_plain_terminal_started`). It does not audit launching an AI CLI agent — the action
   Restricted Mode, workspace trust, and the entire command-approval machinery exist to control.
2. **A scope choice would be written down.** This project records its deferrals compulsively
   (this document is itself an instance of that habit). No note anywhere says agent launches are
   deliberately unaudited.
3. **The documentation asserted the opposite, on a published crate.** `crates/tekstide-core/README.md`
   said durable audit "currently records … managed AgentRun lifecycle." False in the shipped
   product — recorded in tests only. **Corrected as of this response**, not deferred to whenever
   the recommended RFC below lands, since a false claim about an audit trail on a published crate
   is exactly the kind of statement this project's own convention says gets fixed the moment it is
   found, not the moment it is scheduled.

**This is the third category D4 asks to be named if found, and it is more consequential than
either of the first two.** RFC-040/044 built mechanical answers for "actions a user cannot take."
Nothing exists yet for "a fully-built, tested audit-writing path that production never calls" —
and unlike `recover`/`purge_all_records` (D3's "paths that never run"), this is not a recovery
mechanism sitting idle. It is the **audit trail for the thing this whole project exists to run.**

Checked directly, not inferred from the function names alone: `AuditCoordinator::launch_managed_agent_run`
(`audit/integration.rs:342`) does the real launch (`project.launch_prepared_agent_run_with_runtime`)
**and** writes real `DurableAuditRecordV1` records (`Authorized`, then `Started` or `Failed`) via
`append_required`/`append_observation` — the actual `AuditStore`, not an in-memory list. It has
zero callers anywhere outside its own six tests. Production launches a Managed-compatibility agent
run through a different route entirely (`AppState`/`ProjectSession::launch_agent_run_with_runtime`)
that writes no audit record for the launch at all.

| Item | Counts | Search shape |
| --- | --- | --- |
| `launch_managed_agent_run` | 0 / 0 / 6 test-only | `grep -rn '\.launch_managed_agent_run('` |
| `apply_managed_agent_terminal_outcome` | 0 / 0 / 4 test-only | `grep -rn '\.apply_managed_agent_terminal_outcome('` |
| `open_project_text_document` | 0 / 0 / 1 test-only | `grep -rn '\.open_project_text_document('` |
| `save_project_text_document` | 0 / 0 / 1 test-only | `grep -rn '\.save_project_text_document('` |

All four share the same shape: a fully-audited entry point exists, is tested, and is not the one
production calls. **Verdict: own RFC, no number reserved. Confirmed a defect, not an open
question** — see the three grounds above. Not wired here regardless: what to record and when
(`Authorized`, then `Started`/`Failed`, then terminated) is real design, and RFC-013's own
frozen-family discipline over the audit schema applies to it, so this triage surfaces the defect
and corrects the one false public claim it made (`tekstide-core/README.md`) rather than quietly
wiring four functions into a slice whose own stated risk is "triage becomes a rewrite."

**Search shape for the category itself, per D4**: nothing that searches for "does this function
have a caller" finds this, because the *feature* has a caller (agent runs launch constantly) — it
is the wrong function that gets called. The shape that would find it: for every `AuditCoordinator`
method that writes a `DurableAuditRecordV1`, does production call it, or does production call a
same-shaped method elsewhere that skips the write? That is a cross-reference this project does not
yet run mechanically, matching D4's own framing that the output here is a specification, not a
built check.

## Summary

- **4** rows leave the triage as D3's own defect slice (`recover`, `resume`, `purge_all_records`,
  `purge_project_records`), unchanged from the RFC's own decision.
- **4** rows are RFC-045-conditioned `keep, documented`, unchanged from the RFC's own D2 decision.
- **7** rows are **delete**: `add_agent_run`, `add_transcript`, `add_audit_event`, `add_approval`,
  `shutdown` (approval channel), `accept_proposal`, and the
  `record_terminal_transcript_write_summary` / `record_transcript_write_summary` pair counted as
  one row — **9 functions total**, all confirmed superseded by a real, different, already-wired
  production path, none deleting a capability nothing else provides. Two of the nine
  (`shutdown`, `accept_proposal`) are not new findings — the original audit already called both
  "not a capability gap," and re-verification found nothing to overturn that; the other seven are.
- **9** rows are **own RFC**: `purge_agent_run_transcripts`, `set_viewport`, `set_git_summary`,
  `set_warning_state`, `decide_with_edited_argv`, and the four-function managed-agent audit-trail
  cluster (`launch_managed_agent_run`, `apply_managed_agent_terminal_outcome`,
  `open_project_text_document`, `save_project_text_document`) — the last four sharing one
  recommended RFC, not four separate ones, and confirmed a defect rather than left as an open
  question (see above).
- **`set_runtime_summary`** is reconfirmed **not a real finding** — test-only by construction.
- **Four D0 corrections**, one of them wrong in its first draft and fixed at review 354:
  `request_terminate`'s true count (1, not 3), `switch_active_project`'s missing assignment,
  `shutdown`'s ambiguous count, and `transition_change_set_review_state` — **wired**, not
  unwired as first concluded; see the corrected entry above for what the mistake actually was.

No wire verdicts. Every candidate this triage found undesigned went to "own RFC" rather than being
wired here, matching the RFC's own stated risk (triage becomes a rewrite) and this slice's own
scope (a table, not a code change).
