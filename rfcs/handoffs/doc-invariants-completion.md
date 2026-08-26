---
title: "Doc invariants completion — implementation handoff"
rfc: "none"
source_rfc_status: "No RFC. Two mechanical checks and five one-line corrections; nothing to decide."
target_milestone: "M12"
created: "2026-08-26"
---

# Two more checks, and the five things they would have caught

**No RFC, deliberately.** An RFC exists to settle a decision an implementer must not make alone.
There is nothing to decide here: `rfcs/README.md`, `rfcs/delivery-plan.md` and each RFC's folder
already agree on what is true, and this slice makes two of those agreements mechanical instead of
remembered. Scoped as the third theme for `0.15.0` because it is small and because the gap it
closes has now recurred three times.

## Why

`crates/tekstide/tests/rfc_docs_invariants.rs` holds two checks today: every handoff pack's
`source_rfc_status` agrees with its RFC's folder, and every relative link in `rfcs/` resolves.
Both were written after a reviewer broke the thing they now guard, and both have caught real
breakage since.

Two gaps remain, and each has a real incident behind it.

### Gap 1 — an RFC's own `Status:` line is never checked against its folder

The existing test computes `claims_unfinished` from
`status.contains("Proposed") || status.starts_with("Accepted")` and points it **only** at the
handoff pack's `source_rfc_status`. The RFC file's own `Status:` line — the first thing a human
reads — is unchecked.

Five RFCs currently sit in `rfcs/done/` asserting a state they left:

| RFC | folder | its own `Status:` says |
| --- | --- | --- |
| 020 | `done/` | Accepted 2026-08-12 |
| 035 | `done/` | Accepted by the human owner 2026-08-18 |
| 038 | `done/` | Accepted by the human owner 2026-08-24 |
| 039 | `done/` | **Proposed** 2026-08-24 |
| 040 | `done/` | **Proposed** 2026-08-25 |

RFC-041 was the sixth until its closeout fixed it by hand.

**This is not a policy violation** — RFC-000 says the folder is the source of truth and the
Status field is not. But RFC-037's own central argument was that a folder/status divergence "is
the policy's central invariant being violated, in the direction that most misleads someone
deciding what to pick up," and two shipped RFCs whose first line says **Proposed** mislead in
exactly that direction. The predicate that catches it is already written; it is aimed at the
wrong file.

### Gap 2 — an RFC can be accepted and never enter the delivery queue

RFC-034, RFC-035 and RFC-036 were accepted on 2026-08-18 and **none of them appeared in
`rfcs/delivery-plan.md`** until 2026-08-25, when a reviewer noticed while scoping something else
and added all three retroactively. `delivery-plan.md` is the file that answers "what is startable
work"; an accepted RFC missing from it is invisible to the person looking for work.

That was the **third** occurrence of a gap the plan itself records. The first two were fixed by
remembering. This is what remembering is worth.

## Scope

1. **Extend `every_pack_status_field_agrees_with_its_rfc_folder`, or add a sibling**, to check
   each RFC file's own `Status:` line against its folder, using the same `claims_unfinished`
   predicate rather than a second, differently-worded one.
2. **A new check: every RFC in `accepted/` or `done/` has a row in `rfcs/delivery-plan.md`.**
   Keyed on the RFC number. `proposed/` is deliberately exempt — an RFC under review has not been
   scheduled and should not be in the queue.
3. **Correct the five Status lines above**, in the same commit as the check that would have
   caught them, so the check never lands red.

## What to be careful about

- **Both new checks must skip loudly when `rfcs/` is absent**, exactly as the existing two do.
  The crate is published; `rfcs/` is not in either archive, so a consumer running `cargo test`
  from the packaged crate must not see a failure. This is already solved in the file — copy the
  existing mechanism, do not invent a second.
- **Correcting a Status line is not the same as rewriting history.** These five RFCs shipped; the
  correction records that, and should keep the original acceptance date visible rather than
  overwrite it. RFC-041's own closeout shows the shape: *"Implemented and closed 2026-08-26.
  Proposed and accepted 2026-08-25 …"*
- **Do not extend either check to `archive/`** without deciding what a withdrawn RFC's Status
  should say. That is a real question and it is not this slice's.
- **`delivery-plan.md` rows are prose, not a schema.** Match on the RFC number in the first
  column and nothing else; anything stricter will break the next time a row is reworded.

## Acceptance

- Each new check **ablated**: break the property, watch that specific check fail, restore. For
  the delivery-plan check that means temporarily removing a real row, not adding a fake RFC —
  the unit is the design decision, not the line.
- The five corrections land in the same commit as the check.
- Full suite, three consecutive runs under default parallelism, logged to files rather than
  filtered live.

## Not in scope

The README keyboard-table check (README lists keybindings; `KeybindingPolicy::advertised_bindings`
is the source of truth; nothing compares them). Same family, genuinely useful, and a separate
slice — it needs a decision about whether the README is generated or merely checked, which these
two do not.
