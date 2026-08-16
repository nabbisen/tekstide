---
title: "RFC-022: Adapter Spawn and the Command Approval Surface - QA Evidence"
rfc: "RFC-022"
rfc_file: "../../proposed/022-adapter-spawn-and-command-approval-surface.md"
status: "In progress - PR-022-A through PR-022-D implemented, PR-022-D pending review"
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

**Scope was redirected before any code, by review request 218 / response 218.** The pack's
own gate ("a keybinding launches an AgentRun," "`launch_agent_run_with_runtime` gains its
first production caller") did not say what a real keybinding could honestly launch, and
research before writing code found the only concrete answer the pack implied — the reference
adapter — was structurally impossible to ship (a `[[bin]]` of `tekstide-core`, a library
dependency `cargo install tekstide` does not install). Response 218 redirected the slice: a
real, code-defined profile for a genuinely installed AI CLI, `Plain`/`Supervised`, no adapter
protocol needed. Full research and the fork are in
`.git-exclude/review-request/218-rfc022-pr-022d-profile-source-question.md`; the redirection
is `.git-exclude/reviewed/tekstide-review-request-218-profile-source-response.md`. Everything
below implements that redirection, not the pack's original wording.

**The unaffected half of the gate, done first, exactly as scoped.** Response 217/218 both
state resource limits, the selected-run concept, and the keybinding collision check are
unchanged by the profile-source question. Built in that order, before the profile:

- `ProjectSession.selected_agent_run: Option<AgentRunId>` — an explicit field, not derived at
  render time the way `active_terminal_focus` derives "which terminal" from
  `VisibleSlot::Primary`. There is no slot system for agent runs and RFC-022 does not build
  one, so there is nothing to derive from. Set on every successful `attach_agent_launch_plan`,
  moving to the most recently launched run — proven by
  `attach_agent_launch_plan_selects_the_just_launched_run`, which launches two runs in
  sequence and checks selection moves with the second, not just that it is set at all.
- `ProjectAgentLaunchError::AgentRunLimitExceeded { limit }`, enforced inside
  `attach_agent_launch_plan` itself — the same shape and the same reasoning
  `ProjectTerminalError::SessionLimitExceeded`/`add_terminal_session` already established
  (a limit enforced at the call site is one the next caller forgets), restated because
  response 217 flagged this as the first slice where a user action can spawn a real,
  transcript-capturing, audited process. **Ablated**: replaced the real check with a
  structurally-similar but inert one (`if false && ...`), ran
  `agent_run_limit_is_enforced_with_a_typed_refusal` — it failed with `Ok(AgentRunId(...))`
  where `Err(AgentRunLimitExceeded { limit: 1 })` was expected, i.e. the second, over-limit
  launch silently succeeded. Restored, reran clean.
- `NavigationAction::LaunchAgentRun`, `Ctrl+Alt+A` (the established `Ctrl+Alt+<letter>`
  shape — `P`, `M`, `T` already taken, `A` for Agent), `Candidate` status. Checked
  mechanically, not by inspection, the same way every other candidate binding in this file
  is: `launch_agent_run_shortcut_is_a_candidate_that_collides_with_no_other_rule` enumerates
  every *other* rule's binding and asserts none match, so a future `Reserved` addition would
  be caught here too, not only a hand-picked one.
- `launch_agent_run_with_runtime` de-gated: `#[cfg(test)] pub(crate)` → `pub`. This alone
  does not satisfy "first production caller" — the internal tests already called it under
  `#[cfg(test)]` visibility before this slice. What makes the claim true is the new,
  non-test caller below.

**The profile: `AiCliProfile::claude_code_linux_default()`, pointed at a real, genuinely
installed AI CLI.** `claude` (Claude Code), confirmed installed on the reference dev machine
at `~/.local/bin/claude` (a symlink) — not found via bare `which`/`command -v` in the
sandboxed test shell's own `PATH`, confirming `~/.local/bin` needed to be an explicit
`ExecutableLookupPath`, not assumed reachable via inherited `PATH` search (which this
profile type deliberately does not do at all — `AiCliExecutable::PathLookup` walks an
explicit, reviewed list, never `$PATH`). Lookup order: `$HOME/.local/bin`, `/usr/local/bin`,
`/usr/bin`; `$HOME` unavailable degrades to the last two only, never substituting anything
silently — both shapes proven by
`claude_code_profile_lookup_paths_prefer_home_local_bin_when_home_is_set` and
`claude_code_profile_falls_back_to_system_paths_only_without_home`.

`compatibility_level: Supervised` — no adapter protocol needed, matching response 218's own
reasoning (`Managed`/command approval remains reachable only through the reference adapter;
this profile does not attempt to change that, and RFC-022's own claim-narrowing consequence
is the architect's to record, not this slice's to solve).

`workspace_discovery_policy: MayDiscoverWorkspaceFiles`, not `NoKnownWorkspaceDiscovery` —
the honest choice, not the convenient one: Claude Code genuinely reads project files as part
of normal operation, and profile.rs's `evidence()`/`validate_workspace_discovery_policy`
(`agent/launch.rs`) treat this as a real declaration, not free-text description. Restated
here because it has a real, load-bearing consequence below.

`transcript_policy` left at `AiCliProfile::new`'s default — confirmed by reading
`validate_transcript_policy` (`agent/launch.rs`) that this field is purely descriptive and
never consulted by launch validation; only `AgentRunLaunchRequest`'s own
`transcript_capture_mode`/`transcript_state_root` matter, and those are set by the GUI's
launch call, not the profile.

**A finding response 218 did not anticipate, found while trying to test the "no AI CLI
found" case it named as the common first-run state.** `MayDiscoverWorkspaceFiles`'s honest
consequence is that `validate_workspace_discovery_policy` refuses in any `Restricted`
project — and **every project defaults to `Restricted`** (`WorkspaceTrust::Restricted`,
`ProjectSession::new`), and **nothing in the shipped GUI, anywhere, grants trust.**
`grep`-confirmed: no call to `grant_trust`/`revoke_trust`/any construction of
`WorkspaceTrust::Trusted` exists in `crates/tekstide/src` at all. `grant_trust` itself is
`pub(crate)` to `tekstide-core`, unreachable even from this crate's own tests.

The consequence: **`Ctrl+Alt+A`, as built, refuses with `WorkspaceDiscoveryBlocked` for
every real user, every time, regardless of whether Claude Code is installed.** "No AI CLI
found" — the message response 218 called "the common, honest first-run state" — is
currently unreachable through the real keybinding, not common. This is not a bug in the
executable-resolution logic (proven separately below, with a controlled test profile) and
not something this slice should route around by weakening the profile's honest disclosure —
that would repeat exactly the mistake response 218 corrected in the pack's own Option B
framing, dishonesty dressed as a workaround. It is a real gap one RFC boundary over from this
one (RFC-014's trust system has no GUI-reachable grant path yet, a fact this slice's own
launch profile is the first thing to make load-bearing), and I'm recording it rather than
silently building around it, the same discipline response 218 itself modelled when it
recorded RFC-022's own claim-narrowing consequence rather than quietly absorbing it.

Proven, not asserted:
`agent_run_launch_shell_input_switches_to_terminal_immersion_and_shows_the_real_trust_refusal`
drives the real `Ctrl+Alt+A` dispatch path (`update` → `app_command_for` →
`attempt_agent_run_launch`, the real, hardcoded-to-`claude_code_linux_default` production
function) against a freshly opened, never-trusted project and gets exactly
`WorkspaceDiscoveryBlocked` — the actual, current, disclosed behaviour of this keybinding
today, not a gap in the test.

**A related, smaller, non-blocking gap in the same family:** `agent_run_limit` is enforced
in core now, but `ProjectResourceLimits` has no setter reachable from the GUI crate either —
a real user cannot configure a finite limit, only a code-level default (`None`, unlimited)
applies. Same shape `terminal_session_limit`'s already-shipped default of 6 has always had
(not user-configurable either); not a regression this slice introduces, just newly visible
because this slice is the first to give `agent_run_limit` a reader at all.

**The production spawn plumbing itself, proven correct against a controlled test profile —
deliberately never against the real, live Claude Code CLI.** Spawning the real product in an
automated test would mean real interactive auth, real network calls, and an unbounded hang
waiting on stdin; unsafe and unbounded, not merely slow. Every real-process test in this
slice (both crates) instead points a profile built the same way `built_in_profile` already
does at a short, in-repo shell script — the established "real spawn machinery, controlled
test artifact" shape this whole RFC's test suite already uses for the reference adapter.

- `a_trusted_project_launches_a_real_claude_code_profile_through_the_production_spawn_path`
  (`tekstide-core`): a **trusted** project (`grant_trust`, reachable inside `tekstide-core`'s
  own tests only), a `claude_code_from_env` profile pointed at a fake `$HOME/.local/bin/claude`
  script, validated and launched through the de-gated, now-`pub`
  `ProjectSession::launch_agent_run_with_runtime` — the exact entry point the GUI's
  production caller below also calls. Confirms the profile, once trusted, genuinely resolves
  and launches: `Running` status, a real terminal id, `selected_agent_run` set.
- `attempt_agent_run_launch_with_profile_spawns_registers_and_selects_a_real_run`
  (`tekstide`): the GUI-side counterpart, proving `attempt_agent_run_launch`'s full
  downstream chain (`AppState::launch_agent_run_with_runtime` → `TerminalPane::from_launched`
  → pane registration) with a fake, `NoKnownWorkspaceDiscovery`-policy profile (bypassing the
  trust gate proven separately above, since this crate cannot grant trust at all) pointed at
  a controlled executable. **Ablated twice**: (1) disabled the `not-found` refusal-symbol
  mapping — `agent_run_launch_refusal_text_renders_the_not_found_reason_honestly` failed,
  rendering the generic "Couldn't start an agent run" instead of "No AI CLI found"; restored.
  (2) removed the `state.terminal_panes.push(pane)` call —
  `attempt_agent_run_launch_with_profile_spawns_registers_and_selects_a_real_run` failed,
  `terminal_panes.len()` `0` where `1` was expected; restored.

**`TerminalPane::from_launched`, not a second construction path.** Rather than build a
parallel agent-run pane/rendering/subscription pipeline, `launch()` was refactored to share
its tail (spawn the reader thread, build the emulator fields) with a new `from_launched`
constructor that wraps an *already-launched* runtime/handle. This means an agent run's
terminal is picked up by the exact same `state.terminal_panes`/wake-subscription/
`handle_terminal_woke` machinery a plain terminal already uses, with no new subscription code
— which also closes a real correctness concern, not just an implementation-convenience one:
an undrained `TerminalReader` channel (bounded, capacity 8) would otherwise eventually block
the reader thread and, via PTY backpressure, stall the agent's own process. Reusing the
existing wake-driven drain path means this happens "for free," the same way it already does
for every plain terminal pane.

**Route.** `AppCommand::LaunchAgentRun` reuses `open_active_project_terminal_workspace()` —
the same `TerminalImmersion` landing `LaunchTerminal` uses — since an agent run is a real PTY
session the user should be able to see, not a new route. `NavigationAction::OpenCurrentAgentRunDetail`
now maps to `AppCommand::OpenActiveProjectSurface(ProjectOpenSurface::AgentRunDetail)`
(previously `None`, i.e. wired to nothing at all) — the route half of the gate's "reach a
run's detail" requirement; no detail *view* content is built, confirmed out of scope for this
slice by the pack's own text.

**Refusal type.** `AgentRunLaunchRefusal` mirrors `TerminalLaunchRefusal`'s established shape
exactly: a private enum, a `_symbol` fn to a compile-time literal, a `_text` fn doing one
Fluent lookup (`agent-run-launch-refused` in `en.ftl`), a `State` field cleared at the start
of every attempt, rendered conditionally in `terminal_workspace_view` alongside the existing
terminal/paste notices. `not-found` and `workspace-blocked` get their own select arms because
those are the two outcomes response 218's own text asked to be distinguishable from a
generic failure; everything else in `AgentRunLaunchValidationError` falls to the same
`*[error]` arm `TerminalLaunchRefusal` already uses for its own rarer cases.

**State root.** Agent-run transcript capture reuses `AppStatePathProvider::linux_default()`
— the exact resolution `open_real_audit_store` already uses (RFC-013's own "one resolution,
N consumers" convention) — as a third consumer, rather than inventing new XDG resolution
logic in the GUI crate. Degrades to "no transcript capture for this launch" on failure, not a
launch refusal: the default `TranscriptCaptureMode::LocalBounded`
(`AgentRunLaunchRequest::new`'s default) does not reject launch when unavailable, only
`RequiredLocalBounded` does, and this slice does not ask for that.

**Gates.** `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --all-features
-- -D warnings`, full workspace suite, `git diff --check` — all clean.

- `tekstide-core`: 578 (up from 573 — the five tests named above).
- `tekstide`: 215 (up from 212 — the three tests named above).

**What this slice does not claim.** It does not make `Ctrl+Alt+A` actually launch anything
for a real user today — see the trust-gating finding above. It does not build any
trust-granting UI (RFC-014's territory, undecided whether it belongs to this RFC's own
closeout or a separate one). It does not change `Managed`/command-approval reachability,
which response 218 already recorded as the architect's own consequence to carry, not this
slice's.

## PR-022-E - The approval dialog

**In progress, deliberately partial.** Two research questions were raised before any dialog
code (review requests 220, 221) -- 221 is answered (response 221); 220 (open question 3:
does this dialog interrupt whatever the user is doing) is still open. Everything below is the
half response 221 unblocked -- the escaping widget, proven correct -- and none of it depends
on 220's answer. **Not built yet, and waiting on 220**: `state.modal`'s new variant, the
poll/subscription path that detects an inbound proposal, the `OpenPendingApproval` dispatch
wiring, the undeliverable-decision handling, and the `command_approval` audit family's first
real producer (recording a decision is exactly where that producer call belongs, and it has
nowhere to go until the dialog can actually be reached).

**Response 221 corrected `what-the-dialog-must-not-lie-about.md` §1, and the correction
changes what this slice escapes.** Full trace in
`.git-exclude/review-request/221-rfc022-pr-022e-escaping-source-question.md` and
`.git-exclude/reviewed/tekstide-review-request-221-escaping-source-response.md`; the doc
itself was amended in place (commit `54bdb30`, not mine). Summary: `ApprovalRequest.display_command`
(the argv) is already escaped **in the model** by `approval::coordinator::display_argv`/
`display_entry` (RFC-021, response 114/115's own ten-probe suite) -- re-escaping it at the
widget would be redundant, not additionally safe, since `text_safety::escape_untrusted_chars`
only touches control/format characters and none survive in an already-escaped string.
`ApprovalRequest.cwd`, by contrast, is **raw**, straight from the adapter's proposal, and
nothing had ever escaped it -- the actual live attack surface, and arguably the sharper one:
a user reads the command carefully but skims the directory to confirm context, exactly what a
rendering attack targets. `environment_summary` was checked and found dead: no writer exists
anywhere in the codebase (`ApprovalRequest::pending` sets it to `None`, nothing since RFC-021
has ever set it to `Some`) -- nothing adapter-derived is in it today, so nothing was built to
render it.

**What was built**: `approval_dialog_body` (`shell.rs`) -- isolation-wraps `display_command`
(citing RFC-021's escaping, not re-proving it) and escapes `cwd` for the first time via
`text_safety::quote_untrusted`, both fed through `CatalogArgs::untrusted` into the
`approval-dialog-body` Fluent template (`en.ftl`), the same `.untrusted(...)`/`DisplayText`
division of labour every other untrusted-text render site in this crate already uses
(`explorer.rs`, `editor.rs`, `board.rs`, `external_change_dialog_body`). `risk_level_symbol`
renders `RiskLevel` (Tekstide's own classification output, never adapter text) through a
`trusted_symbol` select expression, no escaping needed. `ApprovalDialogButton`
(`ApproveOnce`/`Reject` -- no edit-argv button; the gate names neither an edit flow nor does
`decide_with_edited_argv` have a caller from this slice) and `ApprovalDialog` (holding the
full `ApprovalRequest` plus focus) exist as real types with a real `approval_dialog_view`,
`#[allow(dead_code)]`'d rather than half-wired into `ModalContent` -- see the type's own doc
comment for why building the render layer now, ahead of the trigger wiring, is deliberate
rather than premature.

**Proven, not asserted, with the same rigor RFC-021's own escaping work used:**

- `approval_dialog_body_escapes_a_bidi_override_in_the_cwd` -- the falsifiable claim §1 asks
  for, now aimed at the field that actually needed it. **Ablated**: temporarily replaced the
  real `quote_untrusted(&request.cwd...)` call with an empty escaped value plus the raw cwd
  string appended after the templated body (the same "swap the escaping call for a raw
  `format!`" shape the pre-existing `external_change_dialog_body_escapes_a_bidi_override_in_the_path`
  test's own doc comment documents) -- failed with the real `\u{202e}` override character
  present in the panic's own printed body text. Restored, reran clean.
- `approval_dialog_body_does_not_double_escape_literal_marker_text_in_the_cwd` -- a `cwd`
  containing the literal text `<U+202E>` (no real override character anywhere in it) survives
  unmangled, proving `escape_untrusted_chars`'s idempotency claim rather than assuming it.
- `approval_dialog_body_does_not_mangle_argvs_already_escaped_marker` -- the argv side of the
  same property: a marker `display_argv` itself already produced survives the widget's
  isolation-wrapping unchanged, so citing RFC-021's escaping instead of re-running it is
  provably harmless, not merely argued to be.
- `approval_dialog_body_renders_each_risk_level_distinguishably` -- all four `RiskLevel`
  variants render as four distinct words; a `Destructive` proposal cannot read as `Low`
  because a selector arm was missed.
- `approval_dialog_cooperative_notice_states_both_required_non_claims` -- asserts the actual
  rendered copy states both non-claims §2 requires (a decision here does not stop execution;
  approving does not make the command safe), not merely that some text exists at that key.

**The cooperative-limit wording, chosen and justified, per §2's own instruction to do both:**
"This choice is advisory, not a safeguard: Tekstide sends it to the AI CLI, but the AI CLI
decides whether to actually run the command. Approving does not make the command safe, and
rejecting cannot stop the AI CLI from running it anyway." Rendered as its own line in the
dialog, not folded into the command/cwd body text, so it cannot be missed by a reader skimming
for the command and directory alone -- the same reasoning that keeps it out of documentation-only
per §2's own complaint about dialogs that "look like a security control while being an honour
system." Does not restate the third non-claim (that the shown command is all the adapter will
do) in the notice itself, since that is a property of the interaction pattern (one dialog per
proposal) rather than a sentence about this one decision; worth a second look once 220's
answer determines whether proposals can queue.

**`i18n::enforcement` tests required two additions, not zero**: `every_source_locale_key_resolves_in_every_shipped_locale`
failed on `approval-dialog-body` until `generic_args()` (`i18n/enforcement.rs`) gained
`command`/`cwd`/`risk` fixture values -- a real gap the test caught immediately, not a
formality passed by construction.

**Gates.** `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --all-features
-- -D warnings`, full workspace suite, `git diff --check` -- all clean.

- `tekstide-core`: 578 (unchanged -- no core changes this round).
- `tekstide`: 220 (up from 215 -- the five tests named above).

**Response 222, two required fixes, both cheap, neither blocked on 220:**

- **The Escape hint committed to a state 220 has not decided exists.** `approval-dialog-hint`
  read "Escape leaves this request pending" -- a specific outcome for the interrupt-timing
  question the pack itself says is not mine to decide, and a diverging one: RFC-018's paste
  dialog says "Escape always cancels," which a user has already learned means "get out, no
  consequence." Leaving an adapter's proposal pending (practically: waiting out its own
  30-second timeout) is a different outcome reached by the same trained key, undisclosed.
  Fixed by matching RFC-018's own wording instead of inventing a new one, with a code comment
  explicitly marking it provisional -- do not extend this hint to describe outcomes it does
  not yet know about once 220 answers.
- **The third non-claim ("this is one proposal among however many the adapter may make") was
  deferred, then added** once the reviewer pointed out it does not depend on 220's answer --
  it describes what a single dialog's authority covers, true regardless of how or when that
  dialog was reached. `approval_dialog_cooperative_notice_states_all_three_required_non_claims`
  (renamed from `..._both_required_non_claims`) now asserts all three.
- **`#[allow(dead_code)]` on `ApprovalDialog`/`ApprovalDialogButton` is a condition, not a
  resting state**, per the reviewer's flag -- correct while 220 is open, wrong at a release.
  Recorded directly on both types' own doc comments: if a release is cut before 220 is
  answered, they must be wired into `ModalContent` for real or removed, not shipped dead with
  the lint silenced. Also recorded here as a standing release-gating note, not only in code,
  since a closeout is exactly where this kind of thing gets missed if it lives in only one
  place.

Re-verified after the fixes: `tekstide-core` 578, `tekstide` 220 (same counts -- one test
renamed and extended, none added), all gates clean.

## PR-022-F - Closeout

*Not started.*

## Known limitations going in

- **Approval is cooperative, not enforced.** Nothing intercepts execution; a rejected
  adapter can run the command anyway. RFC-021's own limit, unlifted.
- **The token is not a security boundary** — it authenticates which run is asking, not that
  the asker is trustworthy, and is worthless against a hostile same-user process.
- **The reference adapter proves the pathway, not the ecosystem.** No real AI CLI speaks
  this protocol.
