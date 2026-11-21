# RFC-079: An Honest Audit Outcome For A Close With Nothing To Confirm

Status: **Accepted 2026-11-22 by the owner**; proposed the same day at the owner's authorisation of
the `0.43.0` scoping ([`release-0.43.0-scoping.md`](../handoffs/release-0.43.0-scoping.md),
candidate 1). Number taken from the tree's own maximum at the moment the file was created, per
RFC-058's own 2026-10-22 correction. **D1–D4 as written, and the three acceptance-time decisions
decided by the architect on acceptance** — the owner accepted without ruling them, which is the
RFC-043/RFC-053 precedent ("decided by the architect on acceptance and recorded in this file before
implementation begins"). See *Decided on acceptance* below; say otherwise and any of them reverses
at no cost, because nothing is implemented yet.

**Read *Corrected on acceptance* before starting work.** This document's own estimate of the cost
was wrong when it was written, and the correction is larger than the estimate.

Written by the reviewer, not the owner, and not a proposal the owner originated — the owner
authorised the slot this closes, and the defect was found by this reviewer while scoping it.

## Summary

A project close where **no terminal session was ever enumerated** is durably recorded as
`AuditOutcome::Failed` — the same arm as a close whose session was checked and could not be
confirmed empty. The product already distinguishes those two states internally, deliberately, and
then throws the distinction away one call before the record is written. A user filtering the audit
view by `Failed` is shown closes that did not fail.

This is not a new capability. The three-state value exists, the honest-variant precedent exists in
the very enum that needs the arm, and both lists a new variant must join are compiler-guarded. What
makes it an RFC rather than a chore is the **load path**: one decision has to be taken about what an
older build does when it meets a record written by a newer one.

## What is actually true today, measured

Each step verified in the source at `5e74bbe`, not recalled:

1. **The three states exist and are deliberate.** `TerminalSessionConfirmation`
   (`crates/tekstide/src/shell.rs:6179`) is `NothingToCheck | Confirmed | NotConfirmed`. Its own doc
   records why it replaced a bool: the old accumulator's hopeful `true` default read as "checked,
   and confirmed empty" for a close that checked nothing at all.
2. **`NothingToCheck` is the answer for a project with no live terminal.**
   `terminate_project_live_work` (`shell.rs:6260`) collects live terminal ids, and its own comment
   names the reachable case: *"a project whose only live work is an active agent run with no
   terminal of its own ... opens this function with `live_terminal_ids` empty, so the loop below
   never runs."*
3. **The distinction is collapsed to a bool one call later.** `for_audit_record()`
   (`shell.rs:6214`) is `matches!(self, Self::Confirmed)` — correct as far as it goes, and
   deliberately so (review 695 Required 2 made it `false` rather than a masked `true`) — and the
   call site passes `for_audit_record() && closed` into
   `SafeCloseDecision::Closed { terminal_session_confirmed_empty }` (`shell.rs:6054`).
4. **The bool becomes an outcome.** `safe_close_decision_record`
   (`crates/tekstide-core/src/audit/integration.rs:1377`) maps `true` → `AuditOutcome::Applied`,
   `false` → `AuditOutcome::Failed`. There is no third branch, because the input cannot express a
   third state.
5. **So `NothingToCheck` with a close that went through is recorded `Failed`.** Nothing in the
   record or the schema says a check never ran.

**What this is *not*, checked rather than assumed.** On the other branch — a close refused because
the assessment blocked it for more than running processes — `Failed` is **correct**, and this RFC
does not touch it. `SafeCloseDecision::Blocked`'s own doc reserves that arm for an assessment that
blocked the close *"before any confirmation was ever offered"*, and states that `Closed`'s `Failed`
outcome *"implies a close was attempted and did not finish"*, which is exactly that branch. The
defect is only the vacuous case.

### Why this is the product saying something untrue, not only an imprecise field

RFC-059's audit view carries an **outcome filter the user cycles** (`AuditViewCycleOutcomePressed`,
`audit_view::outcome_cycle_index`, `AuditViewState::filter_outcome`). The wrong arm is therefore not
an internal detail of a stored row: a user who filters for failures is shown ordinary closes, and a
user who filters for `Applied` does not see them. By the delivery plan's own ordering rule — *a
thing the product says that is not true is fixed before a thing the product does not yet do* — that
is what puts this ahead of the rest of `0.43.0`'s candidates.

### The precedent is inside the same enum

`AuditOutcome::Anomaly`'s doc comment, added for RFC-021: *"an informational observation, not a
gate. Deliberately distinct from `Blocked` ... Reusing `Blocked` would overstate what happened."*
This RFC asks for the same judgement one variant along: reusing `Failed` overstates what happened.

### What is already guarded, so this document does not pretend otherwise

- **Writer and reader cannot drift.** `AuditOutcome`'s wire codes come from one `impl_code!`
  invocation (`record.rs:594`) that generates `as_code` and `from_code` from a single list. The
  enumerated-set hazard this project usually warns about — a deserializer nobody lists — does not
  apply here.
- **The UI's enumeration cannot drift either.** `outcome_cycle_index` is an exhaustive `match`, so a
  new variant is a compile error, and `shell/tests.rs:23251` asserts `ALL_OUTCOMES[i]`'s index is
  `i` for every `i`.

**The one unguarded direction is downgrade.** `from_code` returns `None` for an unrecognised code
and `decode_row_inner` (`crates/tekstide-core/src/audit/store.rs:607`) turns that into a decode
failure, so a row written by a build that knows the new variant is **unreadable** by one that does
not. `AUDIT_RECORD_SCHEMA_VERSION` is `1`.

## Options considered

1. **Leave it.** The record keeps saying `Failed` for closes that did not fail. Rejected: it is a
   false statement in a surface a user filters, and this project fixes those first.
2. **Reuse an existing arm — `Completed` or `Applied`.** `Applied` claims a session was confirmed
   empty when none was checked, which is the hopeful default review 490 and review 695 both
   rejected. `Completed` is in use by other families for a finished operation and would make
   "confirmed empty" and "nothing to confirm" indistinguishable in the other direction. Rejected for
   the same reason `Anomaly` was not folded into `Blocked`.
3. **Record it outside the outcome** — a `reason_code` on a `Failed` record, say. Rejected: the
   outcome is what the filter reads and what a reader sees first, so the honest value belongs there.
   A reason code qualifying a wrong outcome leaves the wrong outcome.
4. **Add an arm for "there was nothing to confirm", and stop collapsing the three-state value.**
   Proposed below.

## D1 — add an `AuditOutcome` arm for "nothing to confirm" (proposed)

A tenth variant, `NothingToConfirm` (name is an acceptance-time decision — see below), wire code
`nothing_to_confirm`, documented in the `Anomaly` style: what it means, and which neighbouring arm
it exists *not* to overstate.

## D2 — carry the three states to the producer, rather than a bool (proposed)

The bool is the mechanism, not just the symptom: as long as `SafeCloseDecision::Closed` takes
`terminal_session_confirmed_empty: bool`, the collapse can be reintroduced by anyone who adds a
caller. Replace that field with a three-state value owned by `tekstide-core` (the GUI's
`TerminalSessionConfirmation` lives in `crates/tekstide`, so core needs its own), and let
`safe_close_decision_record` map three inputs to three outcomes.

**`&& closed` must not be lost in the move.** Today the conjunction is what keeps a refused close
from being recorded as confirmed. With three states the mapping has to state explicitly what a
`Confirmed` check on a close that did **not** go through records — which is a case the bool
currently answers by accident rather than by design.

## D3 — the downgrade decision (proposed: state it, do not solve it)

An older build meeting the new code fails to decode that row. The options are (a) accept it and say
so in the record's own documentation, (b) make an unknown outcome decode to a reserved
`Unrecognised` value so old builds keep reading the store, (c) bump
`AUDIT_RECORD_SCHEMA_VERSION` to 2 and have readers refuse the store as a whole.

**Proposed: (a), with the statement written down.** This product does not support downgrade
anywhere else, the audit store is per-installation, and (b) is a second honesty problem — a reader
that renders an unrecognised outcome as something it invents is the overstatement this RFC exists to
remove. What is not acceptable is leaving it unstated, which is what makes it a decision rather than
an implementation detail.

## D4 — one test per state, including the vacuous one (proposed)

The reachability of the defect is **asserted by a code comment and not by a test**: no test today
shows a close with `NothingToCheck` and a successful close producing a record. Whatever D1–D3
decide, this RFC is not complete until a test pins each of the three states to the outcome it
produces — the vacuous one first, because that is the one the current code gets wrong.

## Scope

- One new `AuditOutcome` variant, its wire code, and its doc comment.
- `SafeCloseDecision::Closed`'s field becomes three-state; `safe_close_decision_record` gains its
  third branch; the GUI call site stops calling `for_audit_record()`.
- The downgrade behaviour stated in `record.rs`'s own documentation.
- Three tests, one per state.
- The audit view's filter list, which the compiler will demand.

## Non-goals

- **The containment Standing Watch.** This RFC makes the record honest about whether a check ran; it
  says nothing about why a bulk-visible session member can stop matching its own session.
- **`Blocked` versus `Failed` for a refused close.** Verified correct as it stands, above.
- **Any change to what the user is told in the GUI at close time.** `ContainmentNotice` shipped in
  `0.41.0` and its condition (`should_notify`) is already the three-state value, read correctly.
- **A schema migration.** Nothing existing is rewritten; old rows keep their codes and their
  meanings.

## Risks

- **A new arm invites reuse by the next producer who wants a soft "nothing happened".** The doc
  comment has to say what it is *not* for, which is the discipline `Anomaly`'s own comment already
  models.
- **The three-state type crossing the crate boundary could duplicate the GUI's.** Two types with the
  same three names and no conversion between them is the two-copies defect RFC-058's own review
  found in `max_socket_path_len`. One of them must be defined in core and *used* by the GUI, not
  mirrored.
- **Old rows are not wrong, and must not be relabelled.** Every `Failed` already in a store stays
  `Failed`; this RFC changes what is written from now on, and a reader cannot retroactively tell
  which old `Failed` rows were vacuous. That limit belongs in the release note.

## Acceptance-time decisions, for the owner

1. **The variant's name.** `NothingToConfirm` is the proposal. `NothingToCheck` would match the
   GUI-side vocabulary; `NotApplicable` is shorter and vaguer.
2. **D3, the downgrade story** — (a) unsupported and stated, (b) a reserved unrecognised value, or
   (c) a schema-version bump.
3. **Whether D2 is in scope for this release or deferred.** D1 alone fixes the record; D2 is what
   stops the collapse being reintroduced. Doing D1 without D2 is coherent but leaves the mechanism
   in place.

## Decided on acceptance (2026-11-22, by the architect)

**1. The variant is `NothingToConfirm`, wire code `nothing_to_confirm`.** `AuditOutcome`'s arms are
family-neutral verbs for what happened — `Applied`, `Blocked`, `Cancelled`, `Anomaly` — and
`NothingToConfirm` sits in that register. Rejected `NothingToCheck` deliberately, although it is the
GUI-side vocabulary: that name describes a *code state* in `crates/tekstide`, and reusing it for a
durable record read by a user invites the two-vocabularies confusion across the crate boundary that
the Risks section already warns about for the type itself. Rejected `NotApplicable` as vague, which
is the property this RFC exists to remove.

**2. D3: downgrade is unsupported, and the record's own documentation says so.** Option (a).
Nothing else in this product supports downgrade, the audit store is per-installation, and option
(b) — a reserved `Unrecognised` value an old build renders — would have an old build *display* an
outcome it cannot name, which is the overstatement this RFC removes, one layer down. **Strengthened
by the correction below**: the schema migration means an older build meets a newer `user_version`
before it ever meets an unknown outcome code, so the unknown-code path is the second gate, not the
first. The implementation must confirm what the older build actually does at that first gate and
record it; it must not assume.

**3. D2 is in scope, and not as a preference — it is entailed.** `safe_close_decision_record` must
now produce three outcomes, and a `bool` cannot tell it which of two states a `false` means. Leaving
the bool would force the producer to *infer* the third state, which is precisely the information
that was lost. So `SafeCloseDecision::Closed`'s field becomes a three-state value owned by
`tekstide-core`, and the GUI stops calling `for_audit_record()`.

## Corrected on acceptance: the cost is a schema migration, not one variant

**The *Summary* above calls this "not a new capability" and the scoping document called it "small,
bounded". That estimate was made without reading the storage layer, and it is wrong.** Recorded here
rather than quietly fixed, because the dev team would otherwise start from a scope that does not
exist.

A new `AuditOutcome` variant must join **four** lists, not two. The RFC as proposed named the two
that are guarded and missed the two that are not:

1. `impl_code!` (`record.rs:594`) — one list, both directions. **Guarded.**
2. `outcome_cycle_index` + `ALL_OUTCOMES` (`shell.rs:14698`, `shell/tests.rs:23251`) — exhaustive
   match plus an index test. **Guarded.**
3. **`valid_safe_close()`'s own `match self.outcome`** (`record.rs`) ends in **`_ => false`**. A
   record carrying a new outcome is therefore *invalid*, and `AuditStore::append` validates before
   writing (`store.rs:130`), so the close's audit record would **not be written at all** — a missing
   record, which is worse than a wrong one. No compile error warns of this. This is the
   "unreachable" wildcard arm this project's own standing lesson names as the tell.
4. **The SQL `CHECK (outcome IN (…))`** in the `audit_events_v3_table_ddl!` DDL macro
   (`schema.rs`), plus the per-family row `CHECK` for `family = 'safe_close_decision'`. SQLite bakes
   a `CHECK` into the table definition, so an **existing** store cannot accept the new code without
   a migration step; a fresh install would work and an upgraded one would not.

**The precedent is exact and it is in the tree**: `RFC_066_SAFE_CLOSE_BLOCKED_MIGRATION`
(`migration.rs:124`) exists because the *previous* new outcome for *this same family* —
`safe_close_decision` + `blocked` — was accepted by `validate()` and rejected by the schema's own
`CHECK`. Review 503 found it; RFC-066 fixed it with a named `MigrationStep`. This work is that work
again, with a different code.

### A finding about the test written to catch exactly this

`crates/tekstide-core/src/audit/tests/every_family_outcome_lands.rs` is the enumeration written
after review 503 so the next such mistake could not land. Its module doc claims it "guards every
`(family, outcome, action_kind)` combination the type itself can produce, **not only the ones a
hand-written list happens to include**".

**Only one of its two axes is derived from the type.** `chains_for_family` dispatches on
`AuditEventFamily` with no wildcard arm, so a new *family* is a compile error — that part of
the claim is true. The *outcome* axis is a hand-written list of `AuditOutcome::…` values per family.
A
new outcome variant is therefore not enumerated by it until someone adds it by hand, which is the
condition the doc says does not apply.

**Required, as part of this RFC rather than after it:** add the new combination to that enumeration,
and either make its outcome axis derive from `ALL_OUTCOMES` the way its family axis derives from the
type, or correct the module doc to state which axis is derived. A test whose doc claims a stronger
property than it has is the shape this project keeps paying for; this one is now documented, so it
is no longer free to leave.

### Revised scope

To the *Scope* section above, add: a `MigrationStep` for the `CHECK` constraint; the
`valid_safe_close` arm and what it requires of `operation_id`; the new combination in
`every_family_outcome_lands`; and whatever the older-build-meets-newer-`user_version` check turns
out to do. **Still one release's work, and no longer "one variant, one mapping".**
