---
title: "M14 security review, Part 3 — scope: the controls, the evidence that already exists, and the residue"
status: "Scope for Part 3 of the review opened in security-review-m14.md. Written 2026-11-15, after Parts 1 and 2. Not the verification itself."
rfc_file: "none — review planning"
target_milestone: "M14"
created: "2026-11-15"
---

# Part 3 — scope

Part 3 verifies the threat model's §8 controls (sixteen) and §12 acceptance criteria (seven groups)
against what the product does, rather than against what was decided. Parts 1 and 2 were about
decisions and their records; **this is the part that pays for the distinction**, and Part 1's F3 is
the reason it has to exist: "audit logs are purgeable" was a true statement about a design document
for two milestones during which no user could purge anything.

This file is the scope. It is not the verification.

## The reframing that makes this tractable

The obvious reading of "verify sixteen controls" is a from-scratch audit, which would be months and
would mostly re-derive things this project already knows. **That is the wrong shape, because the
evidence largely exists.**

Measured: `crates/tekstide-core` alone contains **274 distinct test functions** whose names
describe a refusal, a rejection, a block, a fail-closed path or a bound being enforced. Every one of
§8's control areas has some:

| Control area | Refusal/limit-shaped tests found |
| --- | --- |
| 8.1 Workspace opening | 3 |
| 8.2 Workspace trust prompt | 44 |
| 8.3 Command approval | 24 |
| 8.5 Transcript and prompt storage | 27 |
| 8.6 Environment and secret handling | 4 |
| 8.7 Terminal paste protection | 24 |
| 8.8 Terminal output sanitization | 6 |
| 8.12 Process lifecycle and safe close | 35 |
| 8.13 Resource exhaustion controls | 60 |
| 8.14 Local data retention and purge | 9 |
| 8.15 Configuration safety | 22 |

**Read that table as a map of where to look, not as a verdict.** It is a function-name heuristic
over one crate. A control with sixty matching tests can still lack a demonstration of the one
property that matters; a control with three can be complete because it has one property. The
numbers say where the mapping will be quick and where it will be work — nothing more. Anyone who
cites them as coverage has misused them.

**So Part 3's deliverable is the mapping, and its cost is the residue.** For each control: name the
evidence that already demonstrates it failing when it should, or record that none exists. The
controls with none are the implementation work, and there is no way to know how many there are
until the mapping is done.

## The standard, unchanged

**A control that has only ever been seen passing is not known to work.** This is the same rule that
produced RFC-074's per-stage ablation requirement and the release checklist's *"every containment
guard has been shown to FAIL, on every supported platform"* item — which is also the model for what
good looks like here: a named procedure, run per release, that makes the guard go red on demand.

A passing test is evidence about the property the test holds, not about the property its name
suggests. A grep is not evidence of absence.

## Two controls need a disposition, not a verification

- **§8.11, LSP/formatter/tool integration.** No LSP or formatter integration ships. What exists is
  `AutomaticLspStartup` as a *blocked capability* in restricted mode (`security.rs`) — the product
  knows the name and refuses the thing, and the thing does not exist. The honest record is
  "blocked and absent", with the blocking verified; it is not a control to test for correct
  behaviour it has never had.
- **§8.16, Plugin security baseline.** Deferred, per Part 1's Q9 and Part 2's F9, and carrying the
  same recommendation: close it deliberately or keep it open honestly.

## Who does what, and why the split falls where it does

- **The mapping is documentary and is the reviewer's.** It is the same work as Parts 1 and 2 —
  reading the record, naming what exists, refusing to assert what does not.
- **Writing the missing demonstrations is implementation** and belongs with whoever can make a
  control fail on demand. That work is sliced *after* the mapping, against a known residue, rather
  than pre-sliced into sixteen tickets most of which would turn out to be already satisfied.

## Sequencing

**Not before RFC-069's PR-069-A lands.** Two reasons, and the second matters more:

1. `0.41.0` is RFC-069's release and the dev team is on it.
2. **PR-069-A's measurement may change §8.3's and §6.5's dispositions.** It is a measurement of what
   a third-party AI CLI can be made to do, which is exactly the AI CLI boundary this review would
   otherwise be writing conclusions about from the outside. Writing those conclusions first and
   revising them afterwards is worse than waiting.

## What this scope deliberately does not do

Pre-judge how many controls lack evidence. Estimate the residue. Assert that any control holds.
Every one of those is the mapping's output, and claiming any of them now would be the shape of
failure this review exists to catch.
