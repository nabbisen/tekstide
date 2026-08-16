---
title: "RFC-022: Adapter Spawn and the Command Approval Surface - QA Evidence"
rfc: "RFC-022"
rfc_file: "../../proposed/022-adapter-spawn-and-command-approval-surface.md"
status: "Open - no slices implemented yet"
target_milestone: "M11"
created: "2026-08-16"
---

# QA Evidence

**This file holds results. The obligations live in
[`task-breakdown-pr-plan.md`](./task-breakdown-pr-plan.md).** Four obligations in this
project have been lost to that gap.

## Conventions

- **Ablation**: break the property, watch the *specific* named test fail, restore. **A green
  ablation is a defect in the ablation, not a pass.**
- **Positive control**: prove the check reaches real data before asserting what it does not
  find.
- **Reachability**: for each slice, state the path a user takes to reach what it built. If
  the answer is "once a later slice lands," say so rather than letting it read as done.
- **GUI evidence**: `niri msg action screenshot-window`; `env -u WAYLAND_DISPLAY`,
  `xdotool windowfocus`, always `--clearmodifiers`. One window geometry per comparison.
- State what each piece of evidence **does not** prove.

## Starting state, recorded before any change

- No production caller of `launch_agent_run_with_runtime` or `add_agent_run`.
- No production caller of `inject_token_into_environment`.
- `spawn_shell` launches a plain interactive shell only; `.env_clear()` plus five fixed
  variables (`runtime/terminal/launch.rs:482-487`).
- `NavigationAction::OpenCurrentAgentRunDetail` and `OpenDiffReview` both map to `None`.
- `validate_compatibility` (`agent/launch.rs:651-658`) rejects `Managed` without declared
  `structured_action_approval`; no profile declares it.
- The `command_approval` audit family is wired and produces nothing.

## PR-022-A - Design and handoff acceptance

Accepted 2026-08-16. Open question 1 answered in RFC-022 itself: no shipping AI CLI speaks
this protocol, so the first adapter is ours (scope item 6). Questions 2 and 3 remain the
owner's, not blocking until PR-022-E.

## PR-022-B - The reference adapter

*Not started.*

## PR-022-C - Spawn path and token delivery

*Not started.*

## PR-022-D - AgentRun creation and route

*Not started.*

## PR-022-E - The approval dialog

*Not started.*

## PR-022-F - Closeout

*Not started.*

## Known limitations going in

- **Approval is cooperative, not enforced.** Nothing intercepts execution; a rejected
  adapter can run the command anyway. RFC-021's own limit, unlifted.
- **The token is not a security boundary** — it authenticates which run is asking, not that
  the asker is trustworthy, and is worthless against a hostile same-user process.
- **The reference adapter proves the pathway, not the ecosystem.** No real AI CLI speaks
  this protocol.
