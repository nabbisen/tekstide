---
title: "0.42.0 scoping"
status: "**Authorized by the owner 2026-11-21.** The recommended scope stands: RFC-078's implementation slice plus the enumeration-log pid diagnostic, with the two one-liners as candidates. Read as answering question 1 (0.42.0 is the right slot for RFC-078); **questions 2 (scope RFC-075 this cycle?) and 3 (a reserved number for the accessibility question) are not answered by it** and stay open."
target_milestone: "M14"
created: "2026-11-21"
---

# Scoping 0.42.0

Written against the measured state after `0.41.0` published, not against the shape of the backlog.

## What is actually available

**Accepted and startable** (`rfcs/accepted/`, which is what that folder means):

| RFC | State | Size |
|---|---|---|
| **078** Session-member recall | Accepted 2026-11-18, D1 as written, **no slice begun**, unscheduled by the owner's own decision | Scope section names 3 items: per-session state, `decide_processes_in_session`'s extension, one new test |
| **069** An adapter for a real AI CLI | Accepted 2026-11-15, **D1 withdrawn the same day** — no bridge is shipped. D2–D7 are a contract and an honesty rule, already satisfied | Nothing to build |

**Reserved, not scoped** — each would cost a scoping cycle before it could cost an implementation
one: **068** redaction (hard: "a scanner that misses one secret is worse than no claim"), **070**
check for updates (this product's first outbound request), **071** Windows and the Microsoft Store
(blocked on RFC-028's Windows half), **072** a non-blocking job for quarantined tests (RFC-029's own
D1, shipped and never built), **075** sending the attention knock (RFC-058's closeout claims
something the code does not do).

**Recorded findings, unscheduled**, from this cycle's own reviews: the containment enumeration log
prints `members.len()` and discards the pids; the scrollbar contrast gap
(`derived-contrast-pairs.md`); the iced form-control denylist covers `checkbox` only; the audit
record has no honest `AuditOutcome` arm for "there was nothing to confirm"; `project`'s three
single-module dependency subtrees (`crate-organization-status.md` — assessed, no demonstrated
beneficiary, deliberately not proposed here).

## Recommendation: finish the containment thread

**0.42.0 = RFC-078 plus the diagnostic the open watch has been waiting on.** One theme, one
accepted RFC, one small change, and a release whose own closeout can state what is now known that
was not known before.

**1. RFC-078's implementation slice.** It is the only accepted, unscheduled, buildable work in the
tree, and `accepted/` is the folder that means "startable". Its two decision bullets bind the
implementation rather than leaving it open — the recall re-check is `live_member_of_session`, never
a membership-blind oracle, and the per-session set updates after *every* enumeration that returns
`Some(members)`. The RFC's own Risks section names the failure mode its review must check
specifically: a cache updated inconsistently still compiles and still passes a test written against
the easy case.

**2. ~~The enumeration log's pids.~~ Struck 2026-11-21, review 713: already shipped.** This item
was wrong when written. The pids have been printed since `de4e0e3`, 2026-11-16 — the reviewer read a
stale present-tense paragraph in the release checklist as current state instead of checking
`launch.rs`, and the dev team found it by reading the tree. **0.42.0's scope is therefore RFC-078's
slice alone**, with the two one-liners still candidates. The watch stays open on its own unexplained
question, and the diagnostic that will answer its next occurrence is already in place.

**3. Two one-liners already recorded, if they fit**: widen the form-control denylist from `checkbox`
to the other five iced widgets that register no focus (two independent reasons now, and only one is
written down), and the scrollbar contrast gap if the owner wants it — that one is a real change to
`Theme`, not a one-liner, so it is a candidate rather than a given.

**What this release does *not* have: a user-visible change.** `0.41.0` had two; this would have
none unless the scrollbar work lands. That is acceptable — `0.40.0`'s RFC-074 slice was internal
too — but it should be said in the CHANGELOG rather than dressed up. A release that improves a
containment guarantee nobody can see is still worth shipping; claiming otherwise is the failure
mode this project keeps fixing.

## What I am not proposing, and why

- **RFC-075 (the attention knock)** is the strongest *next* candidate and I am deliberately not
  putting it in this slot. It is honesty debt of exactly the class `0.41.0` just paid twice —
  RFC-058's closeout claims the lock holder is addressed "over a real `AF_UNIX` knock" and nothing
  in any real open path calls it. But it carries three open questions (where the call belongs, what
  the user is told when a knock fails, whether to attempt one where the socket cannot bind), and
  answering those is a scoping cycle, not an implementation one. **Worth scoping during 0.42.0
  without committing it to 0.42.0.**
- **RFC-072** is also real debt — a decision that shipped in RFC-029 and was never built — but its
  own text says the hard part is a convention separating quarantined flakes from measurements and
  diagnostics, which `-- --ignored` currently mixes. Same answer: scope it, do not schedule it
  blind.
- **The crate-level work** from `crate-organization-status.md`: no demonstrated beneficiary, and the
  prize has not been measured. It should not enter a release on structural tidiness alone.
- **Anything touching RFC-069's withdrawn D1.** The owner withdrew it with reasons; re-opening is
  theirs to initiate, not mine to schedule.

## Three questions that are the owner's

1. **Is 0.42.0 the right slot for RFC-078?** It was accepted as a *precautionary* closure and left
   unscheduled deliberately. Nothing has made it more urgent since; nothing has made it less so
   either. If the answer is "not yet", 0.42.0 has no accepted work left in it and the honest options
   are a scoping-only cycle (075 and 072) or no release at all for now.
2. **Should RFC-075 be scoped this cycle?** It is the oldest live false claim in a shipped RFC, and
   this project has now twice found that such claims are where real defects hide
   (`ValidProjectRoot`, `TerminalPane::launch`).
3. **Accessibility has no reserved number.** The Standing Watch asks one question every release —
   has `iced` gained a bridge — and the answer has always been no. Nothing anywhere asks the second
   question: *what this product does if it never gains one.* The README, the release notes and the
   landing page all state plainly that there is no screen-reader support, which is honest, and
   honesty is not a plan. If the owner wants that question to exist, it needs a reserved number;
   if the answer is "nothing, deliberately, and we keep saying so", that is a decision worth
   recording rather than leaving as the absence of one.

## Standing debts that outlive this slot

Recorded so they are not rediscovered: the containment watch's unexplained question and its
product decision's own residual (no honest `AuditOutcome` arm for "nothing to confirm", which needs
a schema change and a load path); the explorer's per-pull cost class, guarded only by a manual
measurement harness; RFC-068's redaction question; and RFC-071's Windows half, which is where
RFC-028's remaining cost sits.
