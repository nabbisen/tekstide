---
title: "RFC-036 task breakdown and PR plan"
rfc: "RFC-036"
rfc_file: "../../accepted/036-dormant-capability-closure.md"
source_rfc_status: "Accepted 2026-08-18, D0–D4 decided 2026-08-27 — M12"
target_milestone: "M12"
created: "2026-08-27"
---

# Three slices

**The table comes before any code.** A decision made while the deletion is already half-written is
a decision made by the deletion.

## PR-036-A — the table

**No code change at all.** A document, and it is the deliverable this RFC exists for.

One row per orphan, each carrying:

1. **The measured caller counts**, split three ways — production in `tekstide`,
   `tekstide-core`-internal, test-only — with the method stated once and used for every row (§3 of
   the risk document).
2. **The verdict**: wire / delete / keep-with-named-RFC / own-RFC.
3. **A one-line reason**, including what the capability was *for* — not just that nothing calls
   it (§2).
4. **Per D4: the search shape that would have found it.**

**Start by re-verifying, per D0.** `request_terminate` has three production callers and `shutdown`
one; if either appears as an orphan in your table, the document was triaged instead of the tree.

**Expect rows to change verdict as you measure.** That is the slice working.

**Gate:** the table is reviewed before PR-036-B starts. Nothing is deleted on the strength of a
table nobody has read.

## PR-036-B — wire and delete, in one release

Only what PR-036-A's reviewed table says.

- **Deletions batched**, per D1, targeting `0.16.0` — already owed for RFC-044.
- **`CHANGELOG.md` names every removed public item individually.** A consumer's build breaking is
  the loudest thing this project can do to someone, and a list is the least it can offer in
  return. **Write it in the `0.16.0` entry, at release time** — not by editing a released entry,
  which review 352 had to correct.
- **Anything the table marked "wire"** gets a real caller and a test that the caller reaches it —
  the same bar RFC-041 held for `read_diff_content`, which had been correct, tested and unreached
  for six releases.

**Ablations:** for each wired item, remove the new call site and watch its test fail.

## PR-036-C — the two that left the triage

`recover` and `purge_all_records`, per D3. **A defect slice, not a triage row.**

The question is not wire/delete/document — it is *why can a user's corrupted audit store not be
recovered by the application that built the recovery?*

- Establish what a user experiences today when the store is corrupt. **Reproduce it** — corrupt a
  store in a scratch `XDG_STATE_HOME` and run the release binary — rather than reasoning from the
  code.
- Then decide what should happen, and say whether that is this slice or its own RFC.

**This one is allowed to end in an RFC recommendation rather than a fix.** It is a product
question about recovery behaviour, and §6 of the risk document says so explicitly.

## Not in this plan

- The 74 untraced call chains. A second audit pass, its own unit of work.
- Authoring RFC-045. Reserved by D2, unauthored, and the rows depending on it say so.
- Any mechanical sweep for the categories D4 identifies. The output is the specification; building
  it is later work.
