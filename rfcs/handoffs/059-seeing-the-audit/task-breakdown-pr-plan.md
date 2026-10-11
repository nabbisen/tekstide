---
title: "RFC-059 task breakdown and PR plan"
source_rfc: "../../done/059-seeing-the-audit.md"
---

# RFC-059 — task breakdown and PR plan

Three slices, in this order. **The order is D6 and is not a convenience** — see the pack README.

## PR-059-A — the store can be believed

**No UI in this slice.**

1. **Enumerate, do not list.** Every `(AuditEventFamily, AuditOutcome, AuditActionKind)` the Rust
   side can actually produce, written to a **real** store on disk, each asserted to land and to
   come back from `query`. The pattern already in this codebase is
   `enumeration_confirms_only_the_closed_list_reads_full_file_content` (`project/diff/tests.rs`):
   enumerate the thing, assert the property over all of it. `record.rs` holds the enumeration —
   69 family-to-string arms.

   **A hand-written list of combinations is not this.** The defect being closed is that a
   combination nobody thought of fails silently; a test enumerating only combinations someone
   thought of reproduces the defect in the test suite.

2. **Prove it can fail.** Before fixing anything, run the enumeration against the tree as it
   stands and record which combinations do not land. The one already known is
   `safe_close_decision` + `blocked` (review 503). **If that is the only one, say so with the
   output; if there are others, stop and report before PR-059-B.**

3. **The first-creation race.** Two real processes creating the store concurrently must both
   succeed. Reproduce it first — two processes, overlapping, against one real sqlite path — then
   fix it. `CREATE TABLE IF NOT EXISTS` is the ordinary shape; a file lock across the `exists()`
   check and the create is the other. Either is fine; the reproduction is not optional.

   **The processes must overlap.** Two sequential processes do not race, and a test that starts
   one after the other has proven nothing. (This is the correction RFC-058's own breakdown needed
   at review 513 — it is written here so it does not need making twice.)

## PR-059-B — the view

1. A top-level destination reached from the Project Board (**D7**). A keybinding, named in the
   `Ctrl+Alt+K` list and in `--help`, like every other destination.
2. Reads `AuditStore::query`. Filters are exactly `AuditQuery`'s own — project, family, outcome,
   operation (**D4**). **No free-text search**; there is no free text to search.
3. Paging uses `before_sequence`, the keyset cursor that already exists. Do not add an offset
   paging path beside it.
4. A bounded page says what it left out, in `count_text`'s own words (**D3**).
5. `AuditHealth` is surfaced here (**§6**): a store that could not be opened, or a write that was
   rejected, is visible rather than silent.
6. **The wording guard asserts what the view composes**, not a string assembled beside it. RFC-058
   review 518 had to factor `project_board_lines(state)` out of `content_area` for exactly this
   reason; do the same here from the start.

## PR-059-C — purge, reachable

1. Per-project and global purge, from the view, each behind a confirmation that names its scope.
2. The confirmation says the act is irreversible, consistent with
   `docs/src/users/local-data-and-privacy.md`.
3. **The number shown afterwards is `AuditPurgeReceipt::deleted_record_count`** (**§4**).
4. `REQ-SEC-025` is met for audit data by this slice. Say so in the changelog entry; it is the
   first time any plan row has named that requirement.

## The book, as part of the work and not after it

`docs/src/users/local-data-and-privacy.md` currently says *"There is no in-app command to purge the
audit store. Delete the `audit/` directory to reset it."* **PR-059-C makes that false.** It is
corrected in the same slice that makes it false, not in the candidate sweep — and the correction
says what a reader who relied on the old sentence should now do, per the release checklist's own
*Corrections* rule.

The same section documents what each family records and what its schema makes impossible. The view
must not contradict it, and if the view shows something that section does not describe, the
section grows rather than the claim being left to the screen.
