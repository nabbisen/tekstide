# RFC-059: Seeing The Audit

Status: **Implemented and closed 2026-10-12; released as `0.35.0` 2026-10-12.** See the
*Closed* section. **Accepted by the human owner 2026-10-11.** D1–D6 as written; the split accepted;
**the open question answered by the acceptance itself** — see *Decided on acceptance*. Proposed
2026-10-11. `0.35.0`, pre-1.0. `REQ-SEC-025` (untracked until now), `REQ-SEC-023` in part (see
*Non-goals*), and the gap that **nothing renders the audit store**. Reserved as number 059 on
2026-09-24 under the title *"Seeing The Audit, And Redacting It"*; the second half is now
**RFC-068**.

## Summary

Twelve audit families record. **No surface renders a single audit record**, and no in-app action
purges one. Both capabilities are already written and tested in `tekstide-core`, and neither has a
production call site. This release spends itself on reaching them, and on the two recorded defects
that decide whether what they show can be believed.

## What is already built and unreachable

Verified by reading the code, not the plan:

- **`AuditStore::query(&AuditQuery) -> AuditRecordPage`** — filters by project, family, outcome and
  operation, pages by `before_sequence` keyset, bounded by `limit`. Its only callers are tests.
- **`AuditStore::purge_all_records` / per-project purge**, returning an `AuditPurgeReceipt` with a
  deleted count and journal cleanup. **Zero production call sites** — so the book's *"There is no
  in-app command to purge the audit store"* is true, and `REQ-SEC-025` (*"Users must be able to
  purge local transcripts, logs, and audit data"*) is unmet for audit data. That requirement is
  **named nowhere in `rfcs/delivery-plan.md`**; this RFC is where it starts being tracked.
- **`local_data_summary`**, used today only for transcripts.

So this is a **surface** release over a storage layer that is finished. That is the main reason to
believe its size.

## Why the second half is being dropped, not deferred quietly

`REQ-SEC-023`: *"Environment variables with secret-like names must be redacted from logs and audit
views."* It has two targets and they are not alike.

**For audit views the requirement is already met, structurally, and a pattern set would weaken the
claim rather than strengthen it.** Every column of `audit_events` is an id, an enumerated symbol, a
timestamp, or a reference bounded to `[A-Za-z0-9_.:-]` and 128 bytes. There is **no free-text
column**, and `subject_ref` — the only field that looks open — is an `AuditReference` written for
three internal subject kinds (`app_resource`, `transcript`, `recovery_bundle`). Nothing anywhere
writes an environment variable, a command, a path or a value into an audit record, **because there
is no field that could hold one**. A view over this store cannot leak a secret. Adding a redaction
pass to it would be a claim that implies the opposite: that there is something in there worth
scanning for.

**For logs the requirement is real, hard, and does not belong in a viewer release.** A transcript
holds *"the terminal output of that AI session as it was produced — including anything it quoted
from your files"*, and the AgentRun Report exports the last 16 KiB of it to a file the user names.
If an AI CLI prints a secret, it is on disk verbatim. Meeting `REQ-SEC-023` there means scanning
raw output, and **a scanner that misses one secret is worse than no claim at all**, because the
claim is what makes a user stop checking. `RedactionClaimScope::StructuredMetadataOnly` having
exactly one variant is not an oversight — it is the honest marker of precisely this gap, and it
should not grow a second variant until something can keep the promise the second variant implies.

**Proposed: redaction becomes its own RFC**, whose single question is what claim about transcript
content this product can actually keep. Pairing it with a viewer would let the easy half carry the
hard one across the line.

## The two defects that decide whether the view can be believed

Both are recorded in `rfcs/future-work.md`, both were raised to the owner as pre-1.0, and both are
about the one store this product promises honesty about. **A viewer makes them worse**, because a
reader who is shown a list concludes that the list is what happened.

1. **An audit write can be rejected by SQLite and silently dropped.** 52 `CHECK` constraints, and
   nothing asserts that a given family and outcome actually lands. RFC-066 PR-066-B's own `blocked`
   write produced zero records, passed review, passed the Rust validator, and said nothing. The fix
   shape already exists in this codebase (`enumeration_confirms_only_the_closed_list_reads_full_file_content`):
   enumerate every family-and-outcome the Rust side can produce, write each to a real store, assert
   each lands.
2. **Two processes creating the store for the first time race, and the second loses.**
   `exists()` is read before either connection opens, and `CREATE_SCHEMA_V3` has no
   `IF NOT EXISTS`. Out of RFC-058's scope by construction; ordinary to fix.

## Decisions

- **D1. The view is a new surface, not an extension of Approval History.** Approval History is
  in-memory, bounded by `approval_history_limit`, and **evicted**; the audit store is durable. Two
  lists that disagree about the same events, one lossy and one complete, is the misunderstanding
  this product's philosophy exists to prevent. The audit view reads the store and says so.
- **D2. Approval History stays as it is.** It answers *"what is live right now"*; the audit view
  answers *"what happened"*. Neither is retired by this RFC, and each says which it is.
- **D3. Counts reuse the existing vocabulary.** `agent/report.rs`'s `count_text` already prints
  `"{kept} listed, {omitted} more not recorded"`. A bounded audit page says what it left out in the
  same words, rather than inventing a second idiom for the same honesty.
- **D4. Filtering is exactly what `AuditQuery` already supports** — project, family, outcome,
  operation — and nothing more. No free-text search: there is no free text to search.
- **D5. Purge is reachable, per-project and global, and returns its receipt to the user.** The
  deleted count is shown. `REQ-SEC-025` is the requirement; the machinery exists.
- **D6. Both defects above are in scope for this release.** A view over a store that can silently
  fail to write, or silently fail to be created, is a view that can be confidently wrong.

## Non-goals

- Redaction of transcript or log content (`REQ-SEC-023`'s hard half) — its own RFC.
- A second variant of `RedactionClaimScope`.
- Audit retention or automatic pruning. The store grows until purged; that is today's behaviour and
  this RFC does not change it. Worth its own decision later, not a rider here.
- Export of audit records to a file. The AgentRun Report already exports a run's own counts; a
  general audit export is a separate question with its own disclosure problems.

## Slices

- **PR-059-A — the store can be believed.** The enumeration test over every family-and-outcome, and
  the first-creation race. No UI. This lands first on purpose: if it finds more dropped writes, the
  view's design is affected before it is built.
- **PR-059-B — the view.** A surface over `AuditStore::query`, with D3's bounded-count wording and
  D4's filters.
- **PR-059-C — purge, reachable.** Per-project and global, with the receipt shown.

## Open question as it was put to the owner

**Where does the audit view live, and who can reach it?**

The audit store is **app-wide**, not per-project, and it records events for projects that are not
currently open — including `project_added` and `trust_change` for projects since removed from the
board. Three options, and I do not think the choice is obvious:

1. **A top-level view, reached from the Project Board** (`Ctrl+Alt+?`), showing everything with a
   project filter. Matches the store's real shape. Costs a new top-level destination.
2. **Inside Trust Settings**, where trust grants and purge already live. Cheapest to reach and
   thematically close, but Trust Settings is per-project and the store is not, so it would show
   either less than the truth or more than its own title implies.
3. **Both** — a project-scoped list in Trust Settings, and an app-wide one from the board.
   Complete, and the largest surface added in a release whose argument is that it adds little.

My recommendation is **(1)**: it is the only one whose scope matches the data's scope, and under
the standing criterion that a user must not be able to misunderstand, a per-project framing over an
app-wide store is the option most likely to mislead.

## Decided on acceptance (2026-10-11)

**How the open question was answered matters, and is recorded rather than smoothed over.** The
owner replied *"Accepted."* without naming an option. The only reading that leaves nothing
undecided is that the recommendation stands, so:

- **D7. The audit view is a top-level destination reached from the Project Board** — option (1).
  Decided by acceptance of the recommendation, not by a separate instruction. If the owner meant
  otherwise, this is the line to correct, and correcting it before PR-059-B starts costs nothing.

**The split is accepted.** `REQ-SEC-023`'s log half leaves this RFC and becomes **RFC-068, Redacting
What A Run Prints**, reserved 2026-10-11. This RFC keeps the part that is already true
structurally, and claims nothing about transcript content.

**`REQ-SEC-025` is now tracked**, for the first time, by this RFC's D5.

**D6 stands and is load-bearing.** Both recorded store defects — the silently dropped write and the
first-creation race — are in scope, and PR-059-A lands before any view is built. The order is the
decision: a view over a store that can silently fail to write is a view that can be confidently
wrong, and finding more dropped writes after designing the view would be the expensive order.

## Closed (2026-10-12)

All three slices and the Whole-RFC checklist are done. **Released as `0.35.0` 2026-10-12** — see `rfcs/handoffs/release-0.35.0.md`, the publish record the invariant reads the changelog against;
see `CHANGELOG.md`'s own status line, which does not claim "released" ahead of the actual publish.

**PR-059-A (the store can be believed)** proved every `(AuditEventFamily, AuditOutcome,
AuditActionKind)` combination the type can produce actually lands in a real on-disk store and comes
back from `query` — derived from the type via an exhaustive `chains_for_family` match (no
wildcard), not a hand-written list, so a reviewer could break it by adding a family arm and watch
the test fail to compile. The one previously-known gap (`safe_close_decision` + `blocked`, review
503) was confirmed already closed by an earlier migration; no other gap was found. A second,
independent defect was reproduced and fixed alongside it: two real processes racing to *create* the
audit database for the first time could livelock, not merely lose — SQLite's own `busy_timeout`
retry schedule is fixed, not jittered, so two processes released close together can retry in
lockstep and both exhaust it. Four distinct failure shapes were eventually catalogued for this one
race, the fourth (`UnsupportedApplication`, a torn store read as foreign) found only by the
reviewer's own independent worktree ablation. Fixed with a real blocking OS `flock`
(`lock_creation_section`) across the whole create-or-open decision, the same primitive RFC-058's
`project_lock` already established, held function-local through the migration branch rather than
narrowed — narrowing it would hand a concurrent migration the identical race one step later.

**PR-059-B (the view)** is a new top-level destination, `AppRoute::AuditView` (`Ctrl+Alt+V`, or the
top bar's own always-shown "Audit" button) — not a `ProjectOpenSurface`, since every one of those
needs an active project and the audit store does not. Filters are exactly `AuditQuery`'s own fields
(project, family, outcome, operation) with no free-text search; family and outcome cycle through a
fixed list on a button click, the project filter cycles through currently-open projects, and the
operation filter is reached only by clicking a shown record's own "View full operation chain"
button, which clears the other three filters at the same time so an operation's full chain is never
silently half-hidden. The bounded-count notice reproduces `count_text`'s own existing wording
rather than inventing a second idiom; `AuditHealth` is reused from the Project Board's own producer
for open/write failures, and a distinct notice covers a *query* failure against an otherwise
healthy store, which `AuditHealth` has no slot for. The view leads with a disclosure naming which
question it answers — added at review 530 after the first draft said nothing about itself,
leaving a reader who compared it against Approval History with no way to know the two would
legitimately disagree. The family/outcome filter-cycle lists were a disclosed, non-compile-time-
enforced gap in the first draft; closed at review 531 with `family_cycle_index`/
`outcome_cycle_index`, exhaustive matches a 13th variant fails to compile against, which
`cycle_copy` now navigates by directly rather than a runtime scan.

**PR-059-C (purge, reachable)** wires the storage layer's own already-finished
`AuditStore::purge_project_records`/`purge_all_records` to a new confirmation dialog, mirroring
`TranscriptPurgeModal`'s own shape (RFC-033): scope named two ways (the title, and the body's own
"other projects are not affected" for project scope), the real pre-purge count, and "this cannot be
undone." The receipt shown afterwards is `AuditPurgeReceipt::deleted_record_count`, read off the
real `DELETE`, never a count taken before it; a failed purge is shown as a failure, with no
best-effort fallback available, since the audit store is both what is being mutated and what would
record the mutation. **A real consequence was found while building the confirmation, not assumed**:
purging a project's audit records also revokes its trust, because `has_applied_trust_grant`
decides trust from the exact rows a purge deletes, and `verify_restored_trust_against` demotes a
project whose row is gone on its next open. Closed at review 532/533 by naming the consequence in
the dialog itself, before the user presses Purge, and proving the whole mechanism end to end
against a real store: grant, purge, confirm the already-open session stays trusted, re-verify,
confirm the demotion. The book's own *"There is no in-app command to purge the audit store"* was
corrected in the same slice that made it false.

**`REQ-SEC-025`** is tracked and met for audit data for the first time by this release — named in
no plan row before this RFC. **`REQ-SEC-023`** is met structurally for audit views only: every
`audit_events` column is an id, an enumerated symbol, or a bounded reference, so there is no field
that could hold free text to redact. The log-content half is unmet and is **RFC-068**, reserved and
not started; no entry anywhere in this release implies transcript or log content is redacted.

**Live-captured** against a real project, a real trust grant, and real audit records, throwaway
project and throwaway `XDG_STATE_HOME` under the session scratchpad:
`rfcs/handoffs/059-seeing-the-audit/evidence/pr-059-b/` and `evidence/pr-059-c/`. The purge-opening
and filter-cycle buttons are ordinary `BackgroundControl`s with no keyboard route (the same
classification several existing controls elsewhere in this crate already have), so clicking
through to a purge confirmation or a filtered page could not be captured live with the keyboard-
only tooling available this round — covered instead by real-store end-to-end tests dispatching the
exact `Message`s each button sends.
