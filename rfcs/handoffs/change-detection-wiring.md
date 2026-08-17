---
title: "Change detection wiring — giving RFC-020 something to render"
status: "Scheduled 2026-08-17, awaiting implementation"
rfc_file: "none — this is a wiring slice, not an RFC (see 'Why this is not an RFC')"
target_milestone: "M11"
created: "2026-08-17"
---

# Change detection wiring

## Why this exists

`0.10.0` made AgentRun launch reachable. A user can now grant trust, press `Ctrl+Alt+A`,
and have a real Claude Code session run in their project. **They cannot see what it
changed.**

RFC-020's two surfaces — diff review and the AgentRun report — have been blocked since
response 200 for one reason that has never moved: `add_detected_generated_change_set` has
**zero production callers**. Nothing runs change detection, so no `ChangeSet` can exist,
so both surfaces would render nothing forever.

That is the last structural gap between "you can launch an AI CLI" and "you can supervise
one," which is the product's entire premise.

## Why this is not an RFC

Everything it needs is built, reviewed and tested in `tekstide-core`, and dormant:

| Capability | Where | State |
| --- | --- | --- |
| `GeneratedChangeDetector::capture_agent_run_filesystem_baseline` | `project/change_detection.rs` | implemented, no production caller |
| `GeneratedChangeDetector::detect_filesystem_changes` | `project/change_detection.rs` | implemented, no production caller |
| `ProjectSession::add_detected_generated_change_set` | `project/session.rs:895` | implemented, no production caller |
| `ProjectSession::transition_change_set_review_state` | `project/session.rs` | implemented, no production caller |

No new domain model, no new policy, no security-boundary change. This is the same shape as
terminal resize and RFC-032's grant wiring: correct core, no route.

**But do not read "wiring" as "small."** The finding below is why.

## The finding that decides this slice — read it before planning

**`scan_filesystem` has no ignore model at all.** `scan_directory`
(`project/change_detection.rs`) walks every entry under the project root: `.git/`,
`target/`, `node_modules/`, everything. It has no exclusion list, no `.gitignore`
handling, no hidden-file rule.

**The explorer, in the same crate, already has one.** `FileExplorerScanPolicy::linux_mvp`
(`project/root/explorer.rs:21`) collapses exactly `[".git", "node_modules", "target"]`.

Two sibling directory scanners, one that knows what to skip and one that does not.

### What that means if you wire it as-is

`GeneratedChangeDetectionPolicy::default()` sets `max_entries: 4096`
(`DEFAULT_CHANGE_DETECTOR_ENTRY_LIMIT`). A real project blows through that in `.git/`
alone; a Rust project's `target/` does it many times over.

And when the cap is hit, `detect_filesystem_changes` does this:

```rust
let changed_paths = if status == ChangeDetectionStatus::Complete {
    changed_paths_between(&baseline.entries, &scan.entries)
} else {
    Vec::new()          // <-- truncated scan yields an EMPTY change list
};
```

So on any real project, wiring this today produces: a slow recursive walk over build
artifacts, truncation almost immediately, and **an empty change list** — reported as
`Partial`, not as an error. The diff surface would render nothing, forever, which is the
exact failure RFC-020 already has, reached by a longer route and with more machinery to
maintain.

**Emptying the list on truncation is correct**, incidentally — a partial scan genuinely
cannot distinguish "unchanged" from "not looked at," and returning a partial diff as if it
were complete would be worse. The defect is not that behaviour. The defect is scanning
things nobody wants diffed, and then presenting the result as if it were an answer.

### Positive control before you believe any of the above

Do not take this section on faith. Before changing anything, run detection against a real
project with a `.git/` and a populated `target/`, and confirm you observe
`ChangeDetectionStatus::Partial` and an empty `changed_paths` **after** editing a real
source file. If you see `Complete`, the analysis above is wrong and you should say so
rather than proceeding on it.

## Required decisions

### D1. The ignore model — reuse, do not invent

Change detection must not scan what the explorer already knows to skip. If the two lists
disagree, the user gets an explorer that hides `target/` and a change list full of it.

**Recommended**: give `GeneratedChangeDetectionPolicy` the same collapsed-directory
concept the explorer already has, sourced from one shared definition rather than a second
literal list. A second hardcoded `[".git", "node_modules", "target"]` is a defect the day
someone edits one of them.

**Not in scope**: real `.gitignore` parsing. That is a genuine feature with genuine
subtleties (negation, precedence, nested files) and it belongs with RFC-030 (Git
Integration), not here. Say so in the evidence rather than leaving a reader to assume
`.gitignore` is honoured — it will not be.

### D2. What the surface says when the scan truncates

Whatever the limits, some project will exceed them. `ChangeDetectionStatus::Partial` must
never render as "no changes." It must say the scan was truncated and the result is
incomplete.

This is the same honesty rule as `0.9.0`'s conflict-vs-external-change dialog: two
different states that happen to share an empty-ish presentation must not collapse into one
message. Assume a user who will act on what the surface says.

### D3. When detection runs

Baseline capture at launch is obvious. The completion trigger is not, and **the GUI has no
agent-run exit detection today** — `grep` for `AgentRunStatus` in `crates/tekstide/src`
returns nothing outside tests. Terminals have exit detection; agent runs do not.

So this slice either adds that trigger or picks a different one. State which, and why, and
note that an interactive Claude Code session may run for a long time — a user may well
want to see changes before it exits.

### D4. Cost, measured rather than assumed

A synchronous recursive walk at launch and again at completion, on the UI thread. Once
`.git/` and `target/` are excluded the cost drops enormously, but "enormously" is not a
number.

**This is a case where the number decides the design** — which is the project's own test
for when to measure rather than estimate (`ARCHITECTURE.md`). If a scan of a realistic
project blocks the UI thread perceptibly, it belongs off the UI thread, and that is a
different slice shape. Measure against a real repository with a real `.git/`, report the
number, and let it decide. Do not assume it is fine because the exclusions helped.

## Slices

**A — the ignore model (D1).** One shared definition of what a project scan skips, used by
both scanners. Ablation: remove an entry, watch a specific test fail naming the directory
that reappeared.

**B — measurement (D4).** Baseline and detect timings against a real repository, before
and after A. Report the numbers; if they force detection off the UI thread, stop and say
so before building C.

**C — the wiring (D3).** Baseline at launch, detection at completion,
`add_detected_generated_change_set` called for real. The first production `ChangeSet`.

**D — truncation honesty (D2)** and closeout.

Order matters: A before B (measuring the unexcluded walk tells you nothing you will ship),
B before C (the number may change C's shape).

## The gate

- **A real `ChangeSet` exists in production**, created from a real key press, and the test
  proving it starts from that key press — not from a dispatched command. This is response
  248's lesson and it is not optional.
- **The reachability question answered before the work, not after**: name the user's path
  to seeing the result. If the answer is "there is no surface yet," that is fine and
  expected — RFC-020 builds it — but say it, and do **not** describe this slice as making
  diff review reachable. It makes diff review *buildable*. This project has overstated
  exactly this before, twice.
- Truncation renders as truncation (D2), with a test.
- The `.gitignore` non-goal stated explicitly (D1).
- Measurement reported as a number, with the machine and project it was taken on (D4).
- Every claim about what is now reachable checked against the code the day it is written.

## What this does not do

It does not build diff review, the AgentRun report, or any surface. It produces the input
those surfaces need and have never had. RFC-020 remains blocked on its own work after
this; it stops being blocked on this one.
