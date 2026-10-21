---
title: "Reserved RFC numbers: what is claimed, by whom, and since when"
status: "Register. Opened 2026-10-22, after the owner asked whether the numbering was being consumed too fast."
rfc_file: "none — maintenance register, like `handoffs/test-process-leak.md`"
target_milestone: "M14"
created: "2026-10-22"
---

# Reserved RFC numbers

**Why this exists.** The lifecycle folders (`proposed/`, `accepted/`, `done/`, `archive/`) are the
source of truth for RFCs that *exist*. They say nothing about numbers that have been **claimed in
prose** — in a review response, a dated correction, or a handoff — and never written. On
2026-10-22 there were **ten** such numbers, plus four unexplained gaps, none of them visible to
anyone reading `rfcs/`. That is how two agents pick the same number, and how a reader of the tree
gets a false picture of what is planned.

**The rule, from today.** A number is reserved only if it has a row here. A claim made in a review
response, a commit message or a handoff and nowhere else is **not** a reservation, and the number
stays free. Releasing a reservation means deleting its row, not leaving it to rot.

## Held

| # | Working title | Held by | Since | Where it was claimed |
| --- | --- | --- | --- | --- |
| 074 | Capability Boundaries | architect | 2026-10-17 | `delivery-plan.md`'s own schedule row; goal definition in `tekstide-structure-goal-definition.md` |
| 075 | Wiring the attention knock | architect | 2026-10-19 | `done/058-a-project-held-by-one-process.md`'s dated correction |
| 076 | AUR Publishing From CI | architect | 2026-10-21 | `handoffs/aur-ci-publishing.md`, `delivery-plan.md` |

## Claimed in prose, unaccounted for — the dev team to confirm or release

`068`, `069`, `070`, `071`, `072`, `073`, `079` appear in prose with no document and no row. Each
needs one of: a row above (with a working title and a holder), or release. **`079` in particular is
beyond every other reservation**, which is the clearest sign that numbers were being taken ad hoc
rather than from a register.

## Gaps with no document and no claim

`061`, `062`, `063`, `064` exist in neither the lifecycle folders nor in prose. Either they were
reserved and abandoned, or they were skipped. Record which, here, rather than leaving four holes a
future reader has to guess about.
