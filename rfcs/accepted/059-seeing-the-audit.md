# RFC-059: Seeing The Audit

Status: **Accepted by the human owner 2026-10-11.** D1–D6 as written; the split accepted; **the
open question answered by the acceptance itself** — see *Decided on acceptance*. Proposed
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
