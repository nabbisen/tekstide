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

**Speaks the real protocol against the real socket and coordinator, not a mock.** The
reference adapter is a genuine, separately-compiled binary
(`crates/tekstide-core/src/bin/reference_adapter.rs`) — a `[[bin]]` target in the same
package as the library, not a function inlined into a test. Its own review-gate test file
(`approval/tests/reference_adapter.rs`) spawns the *compiled artifact* as a real
`std::process::Child` and drives it against `ApprovalChannelEndpoint::bind`/`accept_proposal`
and `ApprovalCoordinator::receive_proposal`/`decide` — the same, unmodified production types
`approval::channel`/`approval::coordinator` export, not a stand-in.

**The wire encoding is not reused, and that is disclosed rather than hidden.**
`WireCommandProposal`/`WireCommandDecision` (`channel.rs`) are private to that module; a
`[[bin]]` target in the same package is a separate crate for privacy purposes and cannot
import them regardless of where it lives. The adapter's own `ProposalWire`/`DecisionWire`
mirror those private types' field names and serde shapes by hand, cited in the adapter's own
doc comment against `channel.rs`'s definitions at the time of writing. This is a client
speaking a documented wire protocol, not a reimplementation of the server's decoder — and
the round-trip tests below are exactly what proves that hand-matching didn't drift: a real
process built from `ProposalWire` had to be understood and answered by the real,
unmodified server code, or every one of these tests would have failed at the first frame.

**A full round trip proven, both decisions exercised.**
`a_real_adapter_process_completes_a_full_approve_round_trip` and
`a_real_adapter_process_completes_a_full_reject_round_trip`: a real bound endpoint, a real
spawned adapter process proposing over a real socket, a real coordinator classifying and
deciding, the decision travelling back over the same real connection to the same real
process — proven by that process's own exit code (`0`/`1`) and by what it printed after
parsing the decision it actually received, not by inspecting anything on the server side
alone.

**The token is read from the environment**, exactly as a real child process would —
`TEKSTIDE_APPROVAL_TOKEN` (`approval::APPROVAL_TOKEN_ENV_VAR`, the one constant this
program imports from the library rather than hand-copying, since getting the variable
*name* wrong would be a silent, hard-to-notice failure mode distinct from the wire-shape
question above).

**Missing and wrong token behaviour, both defined and tested, not left to whatever the
socket does:**
- Missing (`a_real_adapter_process_refuses_to_run_without_a_token`): the adapter refuses to
  connect at all and exits `2`, naming the missing variable on stderr. Defined before any
  socket I/O happens, not discovered as a connection failure.
- Wrong (`a_real_adapter_process_exits_distinctly_on_a_rejected_token`): the real server
  independently observes `ApprovalChannelErrorReason::TokenMismatch` (proving the rejection
  is real, not assumed), and the adapter — which receives no error frame at all, per
  `approval::channel`'s deliberate fail-closed-without-a-dialog design; the server just
  closes the connection — detects the resulting EOF on its decision read and exits `3`
  rather than hanging. `DECISION_READ_TIMEOUT` (30s) exists as a second-line defence for
  the same reason but is not what fires in this specific test; the closed connection is
  read as EOF well before that bound is reached.

**Named and documented as a test-and-proof artifact, not a product feature.** The binary's
own module doc comment states this in its first paragraph, citing
`what-the-dialog-must-not-lie-about.md` §4 directly, before the usage/exit-code reference
that follows it.

**One incidental discovery, disclosed:** building and binding a *real* socket (not
`AcceptedProposal::for_test`'s in-memory pair, which `approval::coordinator`'s own tests
use and which never touches a `sun_path` at all) surfaced that a naive, descriptive temp
directory name for the state root — timestamp plus nanoseconds plus a long label, the kind
of name this project's other test helpers use freely — blows a Unix socket's ~107-byte
`sun_path` budget once `/approval/<agent-run-id>.sock` is appended.
`ApprovalChannelEndpoint::bind` already checks and fails this closed
(`SocketPathTooLong`) rather than binding somewhere unreachable, so nothing was silently
broken by this — it surfaced immediately, as a bind error, the first time this suite ever
exercised a real bind. Fixed by shortening this test file's own naming scheme; documented
in `unique_temp_dir`'s doc comment as a caution for anything else that binds a real
approval-channel socket in a test.

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
