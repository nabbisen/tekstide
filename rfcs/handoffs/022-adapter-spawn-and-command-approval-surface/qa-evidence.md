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

**A spawn path distinct from `spawn_shell`, inside RFC-009's boundary.** `spawn_adapter`
(`runtime/terminal/launch.rs`) is a new function, not a branch inside `spawn_shell` itself —
`spawn_shell` is untouched at the call-site level; only its shared PTY/session-setup
mechanics (fd duplication, `setsid`/`TIOCSCTTY`, spawn, cleanup) were extracted into a new
`spawn_pty_child` helper both functions call, a pure extraction verified by the full existing
test suite passing unchanged. `launch_project_adapter` (new, on `LinuxTerminalRuntime`) is a
**duplicate** of `launch_project_shell`'s orchestration shape, not a refactor of it — the only
two differences are an approval-config check and which `spawn_*` function runs, and
`launch_project_shell` is already-reviewed, security-adjacent code this slice had no reason
to touch. `validate_launch_spec` itself **is** shared and unmodified, so RFC-009's existing
gates (cross-project, cwd containment, executable-is-file, environment policy) apply to the
adapter path exactly as they do to the shell path, by construction, not by copying the
checks — proven, not assumed:
`runtime_launch_rejects_non_minimal_environment_policy_for_the_adapter_path_too` re-runs the
pre-existing `ExplicitAllowlist`-rejection test's exact shape through `launch_project_adapter`
specifically and gets the identical rejection.

**`.env_clear()` preserved; the token is one additional `.env(...)`.** `spawn_adapter` sets
the same five fixed variables `spawn_shell` sets (RFC-022's own text: token delivery is "a
sixth" call on top of the existing set, not a redesigned environment), then
`inject_token_into_environment` (a sixth) and `APPROVAL_SOCKET_PATH_ENV_VAR` (a seventh, see
below). Nothing inherited. **`ExplicitAllowlist` stays rejected**, pinned by the test named
above — a genuine re-proof, not an assumption that the shared check must still apply.

**`inject_token_into_environment` gains its first production caller**, and the enumeration
proving it is exact:
`inject_token_into_environment_has_exactly_one_production_call_site`
(`approval/tests/channel.rs`) names `runtime/terminal/launch.rs`'s one call site. The needle
is `"inject_token_into_environment(&mut command"`, not the bare function name — found live,
by running the test before choosing the needle: the bare form also matched the function's
own multi-line definition (`command: &mut std::process::Command`, name before type — the
reverse of a call site's `&mut command`), which would have miscounted this file's own
definition as a second call site. Ablated for real: added a second call, watched the
assertion fail with the exact count (`[("runtime/terminal/launch.rs", 2)]`), reverted.

**The socket path needed a delivery decision RFC-022's own text does not make**, and this
slice makes it, disclosed rather than left implicit: `APPROVAL_SOCKET_PATH_ENV_VAR`
(`TEKSTIDE_APPROVAL_SOCKET_PATH`), delivered the same way as the token — one more `.env(...)`
call, nothing inherited — rather than a CLI argument. Reasoning stated on the constant's own
doc comment: everything this spawn path delivers to the adapter goes through the same
mechanism, and a value with the same lifecycle as the token (generated fresh per bind,
meaningless once the endpoint is gone) gains nothing from a second delivery *class*. This
changed the reference adapter's own contract from PR-022-B's `<socket-path>` CLI argument to
this env var — PR-022-B's own tests were updated to match (a mechanical, disclosed interface
change anticipated by that slice's own doc comment: "RFC-022 does not yet define how a
spawned adapter learns the socket's path in production -- that is PR-022-C's job"), and a
new `a_real_adapter_process_refuses_to_run_without_a_socket_path` test gives the missing case
the same defined-not-guessed treatment PR-022-B already gave a missing token.

**A real spawned adapter completes a real approval round trip, end to end, headless.**
`a_real_adapter_completes_a_real_approval_round_trip_through_the_production_spawn_path`
(`agent/tests.rs`) is the slice's central proof: the reference adapter launched through the
*full* production chain (`AgentRunLaunchValidator::validate` → `AgentRunLaunchPlan::from_validation`
→ `prepare_agent_run_launch` → `launch_prepared_agent_run_with_runtime` →
`launch_project_adapter` → `spawn_adapter`), not the bare `Command` PR-022-B's own tests use
and not a hand-built `TerminalLaunchSpec` bypassing validation. The decision travels back
over the **PTY**, not a piped `Stdio` — `spawn_adapter` goes through the same PTY machinery
`spawn_shell` does — so the test reads it via `runtime.spawn_output_reader`, the same
consumer path a real terminal pane uses, and asserts on what the real adapter process
actually printed after parsing the decision it received over the wire.

**Transcript capture, exercised for the first time by a production `Managed` `AgentRun`.**
`prepare_transcript_capture` (already non-test code before this slice, just never reached
outside a test) is called via the same `prepare_agent_run_launch` this slice's headless test
drives for real. **What this proves**: the mechanism (RFC-011 Amendment 2's writer-in-the-
reader-thread design) reaches this specific, real, production-shaped spawn path — the
transcript file is read back from disk after the run and shown to contain the same
`approved_once` text the channel carried, not merely configured and assumed to work.
**What this does not prove**: RFC-011 Amendment 2's own byte-identical, ordering, and
failure-policy guarantees — those are that amendment's own evidence (RFC-011 Amendment 2
PR-A2-A/B), re-exercised here for the first time in a production context, not re-proven from
scratch by this one test.

**A design decision the review gate did not resolve, disclosed:** `prepare_adapter_approval`
(`AgentRunLaunchPlan`) reuses `AgentRunLaunchRequest`'s existing `transcript_state_root`
field as the approval channel's own state root too, rather than adding a second,
separately-configured root. Both name the same conceptual "Tekstide state root" for a run; a
`Managed` launch with no state root configured has nowhere to put either the transcript or
the approval socket, so `StateRootMissing` covers both honestly. This discharges the review
gate's implicit assumption that a Managed launch is always configured with a real state
root — the pre-existing `project_session_launches_validated_managed_agent_run_through_terminal_runtime`
test (PR-021-era, predating this slice) needed exactly this fixed, since it previously
launched a Managed profile with no state root at all; updated to supply one, a
removal-driven change this slice's own new requirement made necessary, not an unrelated bent
assertion.

**Correction (response 216, required change): the reuse above created a policy nobody
decided, and it is fixed, not merely disclosed.** `AgentRunLaunchRequest::without_transcript_capture()`
sets `transcript_state_root = None`, and the reused-field design above made
`prepare_adapter_approval` require that same field to be `Some` — so a `Managed` run that
opted out of RFC-011's documented per-run transcript retention control could not launch at
all, and the error it hit (`StateRootMissing`) named a mechanism, not the policy that had
accidentally coupled two unrelated RFCs. **Fixed by decoupling**, per the response's own
framing of the choice ("either the error names it... or the state root becomes separable...
State which and why"): `AgentRunLaunchRequest` gained `approval_state_root` (set via a new
`with_approval_channel` builder), independent of transcript configuration.
`prepare_adapter_approval` now reads `approval_state_root`, falling back to
`transcript_capture.state_root` only when the former is unset — preserving the common case
(both artifacts in the same place, one field to set) while making the two controls
genuinely independent. Chosen over the alternative (an explicit but still-blocking error)
because the reviewer's own framing made the deeper problem clear: a documented privacy
opt-out should not silently forfeit command approval, and there is no reason it must.

Proven both directions, not just the one this response asked for:
`a_managed_launch_can_bind_its_approval_channel_without_transcript_capture` launches a real
adapter through the full production chain with transcript capture explicitly disabled and
an explicit approval channel configured — no transcript file, a real bound endpoint, a real
completed round trip.
`a_managed_launch_still_fails_closed_with_no_state_root_configured_at_all` confirms
decoupling did not weaken the fail-closed behaviour: with neither route configured, the
launch still refuses with the same `StateRootMissing`, now honestly meaning "neither route
configured a location" rather than "the wrong field was empty."

**One more real socket-bind-length finding, same class as PR-022-B's, found again
independently.** The pre-existing Managed-launch test's own `test_root` helper (shared
across many unrelated tests in this file) produces a name too long once combined with a
*real* (UUID-based, 46-character) `agent_run.id` and the real socket suffix — worse than
PR-022-B's own finding, since a production `AgentRunId` is far longer than the short
`for_test` form used elsewhere. Fixed the same way: a short, purpose-built temp directory for
this one test rather than widening `test_root` itself (shared by many other tests with no
reason to shrink their own names).

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
