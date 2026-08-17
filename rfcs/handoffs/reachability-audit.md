---
title: "The reachability audit: implementation handoff"
owning_rfcs: "none — infrastructure, like minimal-user-documentation"
status: "Scheduled 2026-08-17 by the owner"
created: "2026-08-17"
---

# Find every capability with no route, at once

## Why

`tekstide-core` holds correct, reviewed, tested capabilities that `crates/tekstide` has no
production path to. Each one has been found individually, by whichever slice was unlucky
enough to reach it:

1. No shipping AI CLI speaks RFC-021's protocol (response 218)
2. No code-defined `AiCliProfile` exists (218)
3. No trust route — `grant_project_trust` has zero production callers (219)
4. `launch_agent_run_with_runtime` had none, until RFC-022 (200)
5. `add_detected_generated_change_set` still has none (200, re-verified 2026-08-17)
6. `switch_active_project` has none (219)
7. `ProjectSession::open_surface()` had no reader at all (233)

**The seventh is why this is scheduled rather than tracked.** Building the first reader of
`open_surface` surfaced **two real shipped defects** — `open_surface` clobbering, which had
silently broken `OpenCurrentAgentRunDetail` since PR-022-D, and an editor keystroke leak into
a hidden document. Neither was findable by inspection.

**Dormant state is not merely untested. It is actively corrupting**, because nothing audits
its writers until something finally reads it. Seven instances produced two real bugs the
moment one woke up, and nobody knows how many remain.

## The methodology, and the trap I already fell into

**Do not grep.** I tried, and produced a list of 65 "dormant" capabilities in
`project/session.rs` alone — most of them false. The reason is worth knowing before you
repeat it:

`#[cfg(test)]` appears at **line 336 of `shell.rs`, a 4127-line file** — an inline attribute
mid-file, not the trailing test module. Any script that strips tests by truncating at the
first `#[cfg(test)]` discards 92% of that file, and every symbol called only in the discarded
part reads as dormant. `dispatch` is called at lines 778, 1082 and 2247. It looked unused.

A scan that produces a plausible answer for the wrong reason is worse than no scan — this
one would have sent you chasing sixty-five non-problems.

**Use the compiler instead.** It already knows the answer:

1. Mark every candidate `pub fn` in `tekstide-core` with
   `#[deprecated(note = "reachability-audit")]`.
2. Run **`cargo build -p tekstide`** — note: `build`, **not** `--all-targets`. That compiles
   the library and binary only, so test code is not compiled and cannot count as a caller.
3. Every deprecation warning names a **real production call site**. The complement — every
   marked item that produced no warning — is the dormant set.
4. Revert the markers.

One build, no false positives from test callers, and no parsing of module boundaries.

## What counts as a capability

Not every `pub fn`. Accessors and `Display` helpers are not interesting; a dormant getter
harms nobody.

**Report on state-changing operations and anything a user should be able to invoke** —
functions that create, mutate, decide, grant, launch, or persist. `grant_project_trust` is
the archetype: correct, audited, and unreachable.

## Output

A table in `rfcs/future-work.md`: capability, module, has-production-caller, and — where it
is absent — **one line on what the user consequently cannot do**. That last column is what
turns a list into a priority order.

**Flag separately any dormant state that has writers but no readers.** That is the
`open_surface` shape, and it is the one that produces bugs rather than mere absence.

## Scope

- **`tekstide-core` public API against `crates/tekstide` production code.** Not internal
  `pub(crate)` items, not the reverse direction.
- **Do not fix anything you find.** This is a survey. Fixes are separate, prioritised work,
  and the trust route is already scheduled ahead of whatever else this turns up.
- **Do not touch `rfcs/delivery-plan.md`.**

## Expected finding, stated in advance so it can be wrong

I expect this to find more than seven, and I expect at least one more of the
writers-without-readers shape. If it finds nothing beyond what is already listed, that is a
genuinely useful result too — it would mean the pattern is exhausted rather than ongoing, and
the audit stops being worth repeating.
