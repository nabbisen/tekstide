---
title: "RFC-022: Adapter Spawn and the Command Approval Surface - QA Evidence"
rfc: "RFC-022"
rfc_file: "../../done/022-adapter-spawn-and-command-approval-surface.md"
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

**Response 220 (four rounds late, by a relay failure the reviewer owned, not a decision
delay): open question 3 is answered.** Full reasoning in RFC-022 §"The arrival model" and the
rewritten PR-022-E gate in `task-breakdown-pr-plan.md`. Summary: interrupt-versus-notify was
the wrong framing -- every design needs a queue (an arriving proposal must never replace an
open modal), so the real decision is *when a queued proposal promotes itself to a modal*.
`High`/`Destructive` promote (active project only, no modal open); `Low`/`Medium` stay
queued; focus defaults to Reject; a promoted dialog briefly ignores input; expiry is a
connection property, not a decision outcome (`ApprovalDecision` stays `Pending`, no audit
schema change). This section covers the first piece built against that answer -- the
undeliverable-decision requirement, which the gate already asked for independent of the
queue/promotion machinery. The queue, promotion, and GUI wiring are separate, later
increments within this same slice.

**The undeliverable-decision requirement (`task-breakdown-pr-plan.md`'s own "a decision that
can no longer be delivered is not recorded as if it were"), built and proven against a real,
actually-exited adapter process, not a synthesised closed socket.**

`AcceptedProposal::is_connection_still_open` (`approval/channel.rs`) -- a non-blocking,
non-consuming liveness probe via raw `libc::recv(..., MSG_PEEK | MSG_DONTWAIT)`, not
`UnixStream::peek` (not yet stable in `std`: `unix_socket_peek`, rust-lang issue #76923; this
file already reaches into `libc` directly elsewhere for comparable reasons). `Ok(0)`/EOF or
any I/O error other than `EAGAIN`/`EWOULDBLOCK` reads as "gone," fail-safe.

`ApprovalCoordinator::decide`/`decide_with_edited_argv` both gained a guard, checked *before*
`audit.authorize_command_decision` and before `send_decision` is ever attempted -- not
discovered afterward via `Decided.sent` being `Err`, which remains the correct, unchanged
handling for a genuine race (the connection was alive at check-time and died between the
check and the send; RFC-021's own reasoning for why that case's decision stays final is
untouched). `DecideOutcome::Undeliverable` is the new outcome: nothing authorized, nothing
recorded, `ApprovalDecision` stays `Pending`. `ApprovalCoordinator::is_still_answerable` is
the same check exposed standalone, for a queue view to ask without attempting a decision.

**Proven against a real process, per the gate's own requirement:**

- `deciding_a_proposal_whose_real_adapter_process_has_already_exited_is_undeliverable`
  (`approval::tests::reference_adapter`) -- spawns the real compiled `reference_adapter`
  binary, accepts its real proposal, then `SIGKILL`s and reaps the process (a genuine exit,
  not a dropped `UnixStream` or a half-close simulated from the test's own side) *before*
  calling `decide`. No 30-second wait for the adapter's own read timeout: killing the process
  is itself a real exit, and the kernel tears down its socket as part of that regardless of
  whether the timeout would eventually have fired too. Asserts `Undeliverable`, the stored
  request still `Pending`, and no `CommandApprove` record of any outcome in the real audit
  store. **Ablated**: temporarily disabled the guard (`if false && !...`), reran -- the
  decision succeeded (`Decided { decision: ApprovedOnce, sent: Err(BrokenPipe), .. }`),
  exactly the false record the gate describes ("an approval is written... for a command
  nothing ran"). Restored, reran clean.
- `is_still_answerable_reflects_the_real_connection_state` and
  `is_still_answerable_is_false_for_unknown_and_already_decided_requests`
  (`approval::tests::coordinator`) -- the lighter, synthetic-socket-pair unit tests isolating
  `is_still_answerable`'s own logic (connection open/closed, request unknown, request already
  decided) from the full real-process integration test above.
- A real bug caught mid-build, not hypothetical: the first version of the real-process test
  used `AgentRunId::for_test(5)` and failed query with `AuditStoreError { reason: DecodeFailed }`
  -- the exact, already-documented gotcha `coordinator.rs`'s own
  `receive_and_approve_persist_the_expected_audit_records` explains (`from_persisted`, the
  decode path every query row goes through, requires a real `<prefix>-<uuid>` shape; `for_test`'s
  short sequence-based ids don't have it). Fixed by switching to `AgentRunId::new_uuid()`,
  the same fix that test already carries as a documented precedent I didn't apply on the
  first pass.

**Gates.** `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --all-features
-- -D warnings`, full workspace suite, `git diff --check` -- all clean.

- `tekstide-core`: 581 (up from 578 -- the three tests named above).
- `tekstide`: 220 (unchanged -- no GUI changes this round).

**Response 224 reviewed the plan above and corrected one of four structural decisions before
more code was built on it** -- full exchange in
`.git-exclude/review-request/224-rfc022-pr-022e-undeliverable-decision-and-arrival-model-plan.md`
and `.git-exclude/reviewed/tekstide-review-request-224-arrival-model-plan-response.md`.
Accepted as planned: the coordinator living on the GUI's `State`, not `ProjectSession`
(`TerminalPane`'s own precedent); "app-wide ceiling" read as per-project (no cross-project
limits infrastructure exists anywhere, and this is not the RFC to invent it); the queue as a
view over `ProjectSession.approval_requests`, not new parallel bookkeeping. **Corrected**:
`approval_request_limit` stays **per-project**, matching its container and its two sibling
fields (`terminal_session_limit`, `agent_run_limit`) -- reading it as per-`AgentRun`, which I
had proposed, would have repeated response 216's PR-022-C defect (reusing a field for a
different scope than its container because the name happened to fit). A new, explicitly
per-run field was added instead. The reviewer also supplied the stronger justification for
the app-wide/per-project ceiling I had reasoned to a weaker one: every pending proposal holds
a live file descriptor (`AcceptedProposal.stream`), so the real thing being protected is
process fd exhaustion, which takes down PTYs, the audit store, and transcript writers with
it, not merely approvals themselves.

**The queue bound, both tiers, built and proven in `tekstide-core` alone -- no GUI needed to
test either.**

`ProjectResourceLimits` gained `agent_run_approval_limit: Option<u32>` (new, per-run, default
`Some(20)`) alongside the existing `approval_request_limit: Option<u32>` (now documented as
per-project, default raised from `None` to `Some(50)` since this slice gives it real
enforcement for the first time) -- both reasoned from the fd-exhaustion rationale above, not
measured, since this bounds simultaneous open descriptors, not throughput.

`ApprovalCoordinator::receive_proposal` gained an `ApprovalQueueLimits { per_agent_run,
per_project }` parameter, sourced by the caller from those two fields. **"Live" only**: each
bound counts entries that are `Pending` *and* still connected
(`AcceptedProposal::is_connection_still_open`, built for the undeliverable-decision fix
above) -- an expired entry holds no file descriptor and does not count against either bound,
matching the fd-exhaustion rationale exactly. `ReceiveOutcome::QueueLimitExceeded { scope,
limit }` is the new refusal, mirroring `DuplicateRejected`'s shape (nothing stored, the
connection dropped).

**Proven, including the cross-project guard response 224 required explicitly:**

- `agent_run_queue_limit_is_enforced_and_only_counts_live_entries` -- two live proposals
  admitted at a limit of two, a third refused naming the real limit; expiring one (dropping
  its peer) frees the slot for a fourth. **Ablated**: disabled the guard, reran -- the third
  proposal was wrongly admitted. Restored, reran clean.
- `project_wide_queue_limit_is_enforced_across_agent_runs` -- the same shape, shared across
  two different `AgentRun`s within one project, proving the budget is project-wide, not
  per-run.
- `queue_limits_do_not_cross_project_boundaries` -- **response 224's required guard**: project
  A held at its own per-project ceiling, project B's proposal (different `ProjectId`) still
  admitted normally. **Ablated**: replaced the per-project filter with "count everything
  regardless of project," reran -- project B's proposal was wrongly refused by project A's
  own pressure (`QueueLimitExceeded { scope: PerProject, limit: 1 }`). Restored, reran clean.

**`ProjectSession`'s own retention/expiry tracking -- decision 3's open question, answered.**
Response 224 asked directly: are expired entries removed from `approval_requests`, or
retained and excluded from the count? **Retained, per the arrival model's own disclosure
requirement** -- but bounded, since "retained forever" is exactly the unbounded growth the
reviewer flagged. `expired_approval_ids: HashSet<ApprovalId>` (a new `ProjectSession` field)
tracks expiry separately from `ApprovalRequest.decision`, which stays `Pending` by design
(nobody decided). `mark_approval_expired` sets it; `pending_approvals`'s computation now
excludes ids in the set. `add_approval_request` reuses `approval_request_limit` (the same
field bounding the coordinator's live queue, since it names the same real quantity for a
project) to bound total retained history: at capacity, the **oldest terminal** (decided or
expired) entry is evicted to make room, never a still-live entry -- silently dropping an
answerable request from view would be worse than the audit trail's own "the absence is the
record" principle, which is about decided requests, not deleted live ones. If nothing is
evictable (every retained entry is genuinely still live), the new one is refused
(`ProjectApprovalError::RetentionLimitExceeded`) -- a backstop, since the coordinator's own
live-queue bound already prevents that many simultaneously-live proposals from existing.

**Proven:**

- `mark_approval_expired_excludes_it_from_pending_approvals_without_changing_its_decision` --
  against the real `pending_approvals` field, and confirms `decision` stays `Pending`.
  **Ablated**: removed the exclusion filter, reran -- the expired request kept counting.
  Restored, reran clean.
- `approval_request_retention_limit_evicts_the_oldest_terminal_entry` -- at capacity with one
  expired, one live entry retained, a new arrival evicts the expired one and is admitted; the
  live entry survives untouched.
- `approval_request_retention_limit_refuses_when_nothing_is_evictable` -- the backstop case,
  a single still-live entry at a limit of one refuses the second with the typed error.
  **Both ablated together**: disabled the eviction guard, reran both -- the first grew past
  its limit (3 retained where 2 was the cap) and the second wrongly succeeded (`Ok(())` where
  `RetentionLimitExceeded` was expected). Restored, reran clean.

**Gates.** `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --all-features
-- -D warnings`, full workspace suite, `git diff --check` -- all clean.

- `tekstide-core`: 587 (up from 581 -- the six tests named above).
- `tekstide`: 220 (unchanged -- no GUI changes this round).

**Response 225, one required change: `approval_request_limit` was bounding two different
quantities.** Full exchange in
`.git-exclude/review-request/225-rfc022-pr-022e-bounded-queue-and-expiry.md` and
`.git-exclude/reviewed/tekstide-review-request-225-bounded-queue-response.md`. I had reused
the field to bound `ProjectSession.approval_requests`'s retained history too, on the stated
reasoning "the same real quantity, for the same project." The reviewer's correction: it is
the same *project*, not the same *quantity* -- an expired entry holds no file descriptor
(established in this same slice), so the fd-exhaustion rationale that justifies the field's
*value* does not apply to retained-history bloat, which is a memory/disclosure cost instead.
This was the third occurrence of the identical shape this session found (PR-022-C's
state-root reuse, request 224's own per-run misreading of this same field, then this) --
recorded because the reviewer named the general check that catches all three: when reusing a
field, state the reason its current value was chosen, then ask whether that reason travels to
the new use.

**Fixed with a genuinely new field**: `approval_history_limit: Option<u32>` (default
`Some(100)`), reasoned from disclosure/memory, not fds -- "how much of a project's approval
history is worth keeping in memory for a user to review." `approval_request_limit` now bounds
only the coordinator's live queue, as originally intended; `add_approval_request` reads
`approval_history_limit` instead. All eight `ProjectResourceLimits` construction sites across
the crate updated; the two retention tests
(`approval_request_retention_limit_evicts_the_oldest_terminal_entry`/
`..._refuses_when_nothing_is_evictable`) now set `approval_history_limit` rather than
`approval_request_limit`, and their continued passing is itself confirmation the fix took
effect (`approval_request_limit` is `None` in both, so if the code still read the old field
the guard would never fire and both would fail).

**Also addressed: the "unmeasured" note.** The reviewer asked to ground the fd numbers or
disclose plainly that they are unmeasured and why. Checked `ulimit -n` on the reference dev
machine: soft and hard `RLIMIT_NOFILE` both report 1,048,576 -- not the "1024 on most
distributions" the original doc comment claimed without checking, which is itself now
corrected to say so explicitly and not generalize from either number (containers and minimal
distros commonly still cap at 1024 or lower; Tekstide has no way to discover the real limit
its own process runs under). Reasoned a baseline Tekstide fd count from the code rather than a
live instrumented measurement (no running GUI instance was available in this environment):
`terminal_session_limit`'s own 6 sessions each hold a PTY master plus two `eventfd`s
(`runtime::terminal::reader`) -- already ~18 -- plus a handful for the audit store's WAL-mode
sqlite files and any open transcript writers, putting steady-state usage around 20-30 fds.
`approval_request_limit`'s `50` brings the project comfortably under 100 total, under 10% of
even the constrained 1024 floor. Recorded in the field's own doc comment as reasoned, not
benchmarked -- unlike `terminal_session_limit`'s real N-pane throughput measurement, there is
no throughput to measure here, only a ceiling to stay well clear of.

**Also addressed: eviction is real disclosure loss.** The reviewer's point stands as a
carry-forward requirement, not something fixable in this slice's own code: whatever future
surface renders `ProjectSession::approval_requests()` must say it is showing the most recent
`approval_history_limit` entries, not imply the list is the project's complete approval
history. Recorded on the accessor's own doc comment and the new field's doc comment so the
GUI-wiring increment inherits the requirement rather than rediscovering it.

Re-verified after the fix: `tekstide-core` 587 (unchanged -- no tests added, two tests'
resource-limit setup corrected to the right field), `tekstide` 220, all gates clean.

**The promotion decision, built as its own pure function** (`approval::should_promote_to_modal`,
new `approval/arrival.rs` module, named after RFC-022's own "the arrival model" section) --
`High`/`Destructive`, no modal open, belongs to the active project; `Low`/`Medium` never
promote (habituation). Deliberately in `tekstide-core`, not the GUI crate: this is security
policy directly derived from `RiskLevel`, the same reasoning that already puts
`approval::risk::classify` here rather than in `crates/tekstide`.

**Proven exhaustively, not spot-checked**: `promotion_requires_high_or_destructive_no_modal_and_the_active_project`
sweeps all 4×2×2 = 16 combinations of risk level, modal state, and project membership against
the same boolean expression the function itself computes, so no corner of this three-input
policy is untested. Two further tests name the two properties the gate calls out explicitly
(the cross-project guard; the open-modal guard) as their own, separately-labelled tests, not
only rows of the sweep. **Ablated three times**: removed the modal/project guards together
(two of the four tests failed, including the exhaustive sweep); removed only the
`belongs_to_active_project` check (the cross-project test failed on its own). All three
restored, reran clean. `tekstide-core`: 591 (up from 587).

**A real defect found while planning the endpoint's GUI-side lifecycle, not yet fixed.**
Tracing how a bound `ApprovalChannelEndpoint` would reach a long-lived GUI-side
`ApprovalCoordinator` surfaced that `ProjectSession::launch_agent_run_with_runtime`
(`session.rs:425-432`, the exact production entry point PR-022-D's `attempt_agent_run_launch`
calls) discards it:

```rust
pub fn launch_agent_run_with_runtime(...) -> Result<(AgentRunId, Vec<TerminalRuntimeEvent>), ...> {
    self.prepare_agent_run_launch(&mut plan)?;   // <- Option<ApprovalChannelEndpoint> dropped here
    self.launch_prepared_agent_run_with_runtime(plan, runtime)
}
```

`prepare_agent_run_launch` returns `Result<Option<ApprovalChannelEndpoint>, ...>` specifically
so its own caller can decide where the endpoint lives (its own doc comment says so). This
convenience wrapper calls it in statement position (`?;`) -- propagating the error, discarding
the `Ok` value -- so for a real `Managed` launch through this one-shot path, the socket is
bound and then immediately dropped before anything can `accept_proposal`/`serve_concurrently`
on it. **PR-022-D's own real caller is unaffected** (`claude_code_linux_default` is
`Supervised`, which never binds an endpoint at all), which is why this shipped without
tripping any existing test -- every test that exercises a real `Managed` round trip
(PR-022-C's own suite) calls `prepare_agent_run_launch`/`launch_prepared_agent_run_with_runtime`
as two separate steps precisely to keep the endpoint alive itself, never through this
convenience wrapper. Not a design ambiguity -- a mechanical fix (widen the return type to
include the endpoint) -- but recorded here rather than silently folded into the next commit,
since it is a real gap in already-reviewed PR-022-C/D code, not new work this slice invented.

**Not yet built, and next**: the endpoint-discarding fix above; the four non-optional UI
constraints (no bulk approval, visibly-unanswerable expired entries, focus-defaults-to-Reject,
the post-promotion input-ignore window); the classifier-limitation disclosure; and the entire
GUI wiring -- a long-lived `ApprovalCoordinator`/endpoint ownership on `State` (currently
nowhere in production, the same "wired with no caller" shape response 219 found for
`launch_agent_run_with_runtime`), the poll/subscription path, `ModalContent::Approval`, the
new `ProjectOpenSurface` variant and its queue-viewing surface, and the `command_approval`
audit family's first real producer.

### The GUI wiring itself -- the arrival model made real (response 227's explicit go-ahead)

Built the full production path per response 227's "the forks that needed raising have been
raised, and the remaining work is execution against a settled shape": `State` gained a real
`approval_coordinator: ApprovalCoordinator` (one, flat, keyed by the coordinator's own
globally-unique `AgentRunId`s -- same shape as `state.terminal_panes`, not per-project, for
the same reason `AcceptedProposal` holding a live `UnixStream` rules out storing it on
`ProjectSession`: that type derives `Clone`/`PartialEq`, a `UnixStream` cannot), a
`Vec<ApprovalChannelServing>` (one per live-served `Managed` run), and an
`approval_proposal_ids: HashMap<ApprovalId, ProposalId>` bridge -- disclosed as a real, small,
never-explicitly-cleaned-up leak relative to `ProjectSession`'s own eviction policy, closing it
scoped as a follow-up rather than blocking this slice.

`Message::ApprovalPollTick` (`iced::time::every`, 250ms, a new `APPROVAL_POLL_INTERVAL` const)
drains every open channel non-blockingly, feeds accepted proposals through the real
coordinator (`ApprovalQueueLimits` built from the owning project's real `resource_limits()`),
mirrors into `ProjectSession` via the now-de-gated `AppState::project_mut`, sweeps expiry
(`ApprovalCoordinator::is_still_answerable` against every still-`Pending` request), then calls
`should_promote_to_modal`'s real call site for the first time: `evaluate_promotion` -- no-op if
a modal is already open, otherwise the active project's oldest qualifying `Pending` proposal
(re-confirmed live via `is_still_answerable`, not trusting the last sweep alone) promotes,
focus defaulting to Reject, with the post-promotion input-ignore window
(`APPROVAL_DIALOG_INPUT_IGNORE_WINDOW`, 400ms) set. `ModalContent::Approval(Box<ApprovalDialog>)`
is now genuinely constructed and dispatched (`view()`, `trusted_ui_state` -- resolving that
accessor's own long-standing "third contributor" doc-comment note); `ModalActivate` computes a
real `SimpleDecision` from focus for both buttons (unlike Paste/ExternalChange, dismissing this
dialog is never a no-op); both `ModalActivate` and `ModalDismiss` re-run `evaluate_promotion`
afterward, since freeing the one modal slot is itself a promotion trigger, per response 227's
correction that promotion is not only an arrival-time check.

**A real defect found and fixed while building this: the endpoint was still being dropped, one
layer higher than PR-022-C/D's discarding bug.** `ApprovalChannelEndpoint::serve_concurrently`
(`self: Arc<Self>`) deliberately drops its own strong `Arc` internally and keeps only a `Weak`
in its accept-loop thread -- its own doc comment says a caller-retained clone is required for
the endpoint to stay alive. The first version of `register_approval_channel` wrote
`Arc::new(endpoint).serve_concurrently()` with no clone retained anywhere: the temporary
`Arc`'s strong count hit zero the instant the call returned, running
`ApprovalChannelEndpoint::drop` immediately -- closing the listener and removing the real
socket special file -- before the accept-loop thread's first `accept()` call ever ran. Found
via the first real GUI-level end-to-end test
(`a_real_low_risk_proposal_is_received_mirrored_and_stays_queued_without_promoting`): the real
reference adapter subprocess connected to a socket path that reported `ENOENT`
(`connect to <path> failed: No such file or directory (os error 2)`) and exited with its own
defined connect-failure code (3), confirmed via a `ps aux` check that no `reference_adapter`
process was still alive ~200ms after launch. Traced by reading `ApprovalChannelEndpoint::bind`
and `serve_concurrently` directly rather than trusting an earlier, wrong hypothesis (that a
re-resolved `state_root` used only for GUI-side risk-classification context had somehow
diverged from the real bind path -- it never touches the bind path at all, which is determined
entirely inside `prepare_adapter_approval`). Fixed by storing the retained `Arc` on
`ApprovalChannelServing` itself (`endpoint: Arc<ApprovalChannelEndpoint>`, `#[allow(dead_code)]`
-- never explicitly read, held purely to keep the socket alive for the serving's lifetime,
same shape as the pre-existing `shutdown: ServeShutdown` field). **Ablated**: temporarily
reverted `register_approval_channel` to the exact original single-`Arc`-with-no-retained-clone
shape (removing the `endpoint` field entirely) and reran the same test -- it failed identically
(`the real adapter should send its default proposal within the poll window`, the coordinator
never receiving anything within the 4s poll window), confirming the fix is what makes the real
end-to-end path work, not an artifact of test setup. Restored and reran clean.

**Six new tests, all against the real pathway** (no mock adapter, no synthetic socket -- the
actual `reference_adapter` binary, spawned through the production `Managed` launch path):
`a_real_low_risk_proposal_is_received_mirrored_and_stays_queued_without_promoting`,
`a_destructive_risk_level_promotes_with_focus_defaulting_to_reject`,
`deciding_the_promoted_dialog_sends_a_real_decision_and_updates_the_stored_request`,
`a_destructive_proposal_for_a_background_project_does_not_promote`,
`re_evaluation_promotes_a_queued_destructive_proposal_once_a_different_modal_closes`,
`modal_input_is_ignored_within_the_post_promotion_window`. Since the reference adapter's real
argv is not controllable through the production launch path (no CLI-arg-injection mechanism
exists for `AiCliPromptPolicy::Argument`), every real proposal received this way is `Low` risk
by the adapter's own unconfigurable default; tests needing `High`/`Destructive` risk override
only the GUI-mirrored copy's `risk_level` via `replace_approval_request`, leaving the real wire
connection and the coordinator's own liveness tracking untouched -- documented directly in the
test helper (`launch_real_managed_agent_run`) as the methodology, not asserted implicitly.

**A second, unrelated bug found and fixed while writing these tests**: the cross-project test
(`a_destructive_proposal_for_a_background_project_does_not_promote`) assumed
`add_project_from_path` makes the newly added project active. It does not --
`AppState::add_project_session` only auto-activates a project when none was active yet
(`app.rs:136-138`); a second project added while one is already active leaves the first one
active unless `switch_active_project` is called explicitly, which is exactly the function
already disclosed (PR-022-E's promotion-decision evidence, above) as having no production
caller anywhere in the GUI crate. This was a test-setup defect, not a production one -- fixed
by having the test call `switch_active_project` explicitly rather than assuming it happens
implicitly. Recorded here since it is a second, independent confirmation of the same disclosed
gap (no GUI feature exists yet to switch a project's active status interactively), not a new
one.

**A pre-existing, unrelated doc-comment defect found and fixed while documenting the retained-`Arc`
fix**: `risk_level_symbol`'s own doc comment (explaining its `trusted_symbol` Fluent-lookup
division of labour) had become orphaned onto `register_approval_channel` -- a leftover
merge artifact from earlier editing in this same file, with no function signature separating
the two doc blocks, so rustdoc silently attached both to whichever item followed. Moved back to
its own function.

Full gate clean after this increment: `cargo fmt --all --check`, `cargo clippy --workspace
--all-targets --all-features -- -D warnings`, `cargo test --workspace --all-targets
--all-features` (`tekstide` 226, up from 220 -- the six new tests; `tekstide-core` 593, up from
591 -- unrelated to this increment's own tests, reflecting the workspace's current state),
`git diff --check`.

**Still not built, disclosed and carried forward**: the four non-optional UI constraints beyond
what this increment covers (no-bulk-approval is satisfied by omission -- no multi-select UI
exists to violate it -- but not explicitly tested; visibly-unanswerable expired entries needs
an actual queue-viewing surface, which does not exist yet); the classifier-limitation
disclosure copy; the new `ProjectOpenSurface` variant, its queue-viewing surface, and
`NavigationAction::OpenPendingApproval` (still mapped to `None` in `app_command_for`); an
explicit GUI-level test asserting the `command_approval` audit family produces real durable
records through this pipeline (implied by `receive_approval_proposal`/`decide_approval`
passing a real `AuditCoordinator`, not yet directly queried and asserted in a test); wiring the
"active-project-change" re-evaluation trigger for real, once project-switching exists anywhere
in the GUI (the logic itself, `evaluate_promotion`, is unconditionally correct regardless of
caller -- this is a one-line addition once a real call site exists, not a design question);
closing the `approval_proposal_ids` bridge's own small leak.

### Response 228's two required items

**Required 1: a real, unmirrored `Destructive` proposal, classified and promoted end to end.**
Every existing promotion test overrode only the GUI-mirrored `ApprovalRequest.risk_level`,
since the production launch path has no mechanism to inject a custom argv into the adapter's
own command line (`spawn_adapter` calls `Command::new(&spec.shell)` with no `.arg`/`.args` at
all). The reviewer's fix: the profile's *executable* is test-controlled even though its argv
is not. New test `a_genuinely_destructive_real_proposal_is_classified_and_promoted_end_to_end`
launches a profile pointed at a tiny generated `#!/bin/sh` wrapper
(`destructive_reference_adapter_wrapper_path`) that hardcodes `exec <reference_adapter>
rm -rf /nonexistent/tekstide-test-destructive-marker` -- the reference adapter only ever
proposes and prints a decision, never executes the argv it sends, so nothing destructive
actually runs. This reaches the real `approval::risk::classify` with a real `rm -rf` argv over
the real socket, and asserts the *received, unoverridden* `ApprovalRequest.risk_level` is
`Destructive` before calling `evaluate_promotion` on it. **Verified the test is not vacuously
true**: temporarily swapped the wrapper's hardcoded argv for `echo hello-ablation-check` and
reran -- failed with the real classifier correctly reporting `Low` instead
(`assertion left == right failed ... left: Low, right: Destructive`), confirming the assertion
tracks the real argv rather than passing regardless. Restored and reran clean. The existing
override-based tests are unchanged, as the reviewer said they could remain -- this is the one
test proving the seam itself, not a replacement for the rest.

**Required 2: `approval_proposal_ids` is pruned rather than left to leak.** `ProjectSession::add_approval_request`
now returns `Result<Option<ApprovalId>, ProjectApprovalError>` -- `Some(evicted_id)` when
`approval_history_limit` eviction removed an entry to make room (`evict_oldest_terminal_approval_request`
now returns the evicted `ApprovalId` instead of a bare `bool`). `receive_approval_proposal`
removes that id from the bridge on eviction; `decide_approval` removes its own entry
immediately on a real `Decided` outcome (nothing ever looks up a decided request's
`ProposalId` again). Expiry deliberately does **not** prune -- an expired request stays
`Pending` and retained until it is later decided or evicted, so pruning at expiry time would
desync the bridge from a request `sweep_expired_approvals` might still need to look up before
that happens; this is recorded directly on the field's own doc comment, not left implicit.

**Both routes ablated independently, since a decided entry is already pruned before it could
ever reach eviction** -- the new eviction test therefore marks its first entry **expired**, not
decided (`mark_approval_expired` directly, matching `approval_proposal_ids`'s own documented
reason expiry doesn't already remove it), specifically so the eviction-side branch is the only
thing that can account for its later removal. Reverting `receive_approval_proposal`'s
eviction-prune line reproduced the expected failure (`the evicted (first) entry's bridge
mapping must not reappear or persist`); reverting `decide_approval`'s decide-time removal line
reproduced its own expected failure on the pre-existing decide test, now extended with this
exact assertion. Both restored, reran clean.

Full gate clean: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets
--all-features -- -D warnings`, full workspace suite (`tekstide` 228, up from 226; `tekstide-core`
593, unchanged -- the two new assertions extend existing tests rather than adding new ones),
`git diff --check`.

### Response 229's suggestion, and priority item 2

**Response 229's suggestion (not blocking, taken)**: `a_genuinely_destructive_real_proposal_is_classified_and_promoted_end_to_end`'s
safety depends on the reference adapter never executing the argv it proposes -- a property
that was, until now, unstated and unpinned in code. New test
`reference_adapter_binary_never_executes_the_argv_it_proposes`
(`crates/tekstide-core/src/approval/tests/reference_adapter.rs`) source-scans
`reference_adapter.rs` for the concrete Rust APIs that would actually spawn a process
(`std::process::Command`/`Command::new`/`.exec(`/`execvp`/`execv`/`execve`/`execl`/
`posix_spawn`/`libc::fork`) and fails by name if any appear -- narrow enough not to false-fire
on unrelated future code, wide enough to catch every ordinary way this file could grow a real
spawn path. Cross-referenced directly from the GUI test's own doc comment, so the dependency is
visible from either direction. **Ablated**: added a throwaway
`std::process::Command::new("true")` call inside `reference_adapter.rs`, reran -- failed
naming the exact forbidden string found. Reverted, reran clean. Also added the transitive-bound
note response 229 asked for to `approval_proposal_ids`'s own doc comment: the map is not
bounded by anything of its own, only through `approval_history_limit` via eviction pruning.

**Priority item 2: the `command_approval` audit family's first real producer, queried and
asserted rather than implied.** New test
`command_approval_family_produces_real_durable_audit_records_through_the_pipeline` runs a real
receive (through the real reference adapter) followed by a real `ApprovedOnce` decision
(through real `update()`/`ModalActivate` routing), then opens the real audit store and queries
it, filtering by this run's own `AgentRunId`. Asserts, by exact `action_kind`/`outcome`/
`approval_id` rather than "a record exists somewhere":

- A `CommandRequest` record with `outcome: Requested` and `approval_id` matching the received
  request (`AuditCoordinator::record_command_request`, called from `receive_approval_proposal`).
- **Two** `CommandApprove` records, not one -- `authorize_command_decision` writes an
  `Authorized` record first (the real authorization gate: its own doc comment says a failure
  here must block the decision entirely), then `record_command_decision_outcome` writes a
  second, best-effort record confirming whether the decision was actually delivered back to the
  adapter (`Applied`) or not (`Failed`). Asserting both, not just "a `CommandApprove` record
  exists," is what proves the real socket delivery happened, not only that the decision was
  authorized in principle -- this was found by reading `audit/integration.rs` directly rather
  than assumed, after an initial draft asserted only a single `Authorized` record and would
  have been fragile to which of the two `query()` returned first.
- Both records' `approval_id` matches the request's own id, tying every record unambiguously
  to the one proposal this test actually drove through the real pipeline.

**Ablated**: temporarily replaced `decide_approval`'s body with an unconditional early return
(no decision sent, nothing recorded), reran -- failed at the test's own precondition check
(`stored.decision` stayed `Pending` instead of reaching `ApprovedOnce`), confirming the test
depends on the real pipeline actually running rather than passing regardless. Restored, reran
clean.

Full gate clean again after both: `tekstide` 229 (up from 228), `tekstide-core` 594 (up from
593, the source-scan guard).

### The flake investigation (responses 230/231), and a real fix found along the way

Response 230 reported one `tekstide-core` failure in ten runs and asked it be characterized
before closeout rather than left unmentioned. Sampled 150 full-suite runs: 3 failures (2%).
Two were the already-known `approval::tests::channel::bind_recovers_from_a_stale_socket_file`
(RFC-021, response 213's own flake, confirmed still live). The third was a **different**,
pre-existing test failing the same way under the same kind of load:
`approval::tests::coordinator::agent_run_queue_limit_is_enforced_and_only_counts_live_entries`
(commit `375d256`, predates this window). Neither reproduced across 40 isolated single-test
reruns -- both need concurrent suite pressure, matching the known fork-window shape. Recorded
in `rfcs/future-work.md`'s existing socket-flake entry rather than opened as a new one.

Response 231 pointed out the panic's own `ApprovalChannelError { reason: Io, source: None }`
was destroyed evidence, not missing evidence: `clear_stale_socket`'s catch-all `Err(_)` branch
used the non-source-preserving `ApprovalChannelError::new` where every other `Io`-reason site
in `bind()`'s call chain already uses the source-preserving `::io`. **Fixed**
(`ApprovalChannelError::io(error)`), a real, permanent correctness improvement independent of
whether it explains this specific flake.

**Re-ran the sweep with temporary per-branch diagnostics (180 further runs, 2 more
reproductions) to find out.** Both times, the diagnostic that fired immediately before the
panic was `"connect unexpectedly succeeded"` -- `UnixStream::connect` returning `Ok` where an
abandoned listener was expected to produce `ConnectionRefused`. **This disconfirms the
fd-exhaustion (`EMFILE`/`ENFILE`) hypothesis for this specific flake**: a successful connect
carries no errno to preserve, so the errno fix could not and did not surface one -- correctly,
since none exists on this path. Diagnostics removed after use; the real fix stays. Full detail
and the queue-limit test's own matching-shape reproduction (`is_connection_still_open`'s
`recv` probe not observing an already-`drop`ped peer's closure) recorded in
`rfcs/future-work.md`, since a genuine kernel-level root cause is a materially bigger
investigation than fits inside this review cycle.

Full gate clean after the fix: `cargo fmt --all --check`, `cargo clippy --workspace
--all-targets --all-features -- -D warnings`, full workspace suite (`tekstide` 229, unchanged;
`tekstide-core` 594, unchanged -- a correctness fix to an existing error path, not a new test),
`git diff --check`.

### The flake's named mechanism, and the mitigation's measured (inconclusive) effect

Response 232 named the mechanism: `fork()`, not scheduling. `Command::spawn` on Linux is
fork-then-exec whenever it sets env vars or a cwd (every spawn in this codebase does), and
between fork and exec the child holds a duplicate of every fd open in the whole parent
process. A socket another thread has just closed is still open in a forked-not-yet-exec'd
child, so a `connect()`/`recv()` against it observes the old, live state -- explaining both
reproductions (a stale-listener `connect()` unexpectedly succeeding; an already-closed peer's
`recv()` not observing EOF) with one mechanism and no failed syscall anywhere. Full reasoning
in `rfcs/future-work.md`'s socket-flake entry, not duplicated here.

**Mitigation**: `RealProcessLimiter` (previously private to `runtime::terminal::reader::tests`,
response 212's own measured cap) lifted to a new shared `crate::test_support` module
(`crates/tekstide-core/src/test_support.rs`) so the cap is genuinely process-wide. Applied to
every real-process spawn in `approval::tests::channel`
(`inject_token_into_environment_sets_the_sanctioned_variable_on_a_real_child_process`,
`cross_process_impersonation_with_wrong_token_is_rejected`) and all six real-adapter-spawning
tests in `approval::tests::reference_adapter`.

**Re-measured**: 150 further full-suite runs (matching the original sample size) under the
shared limiter -- 2 failures, versus 3 in the original 150. A small decrease, directionally
consistent with the hypothesis, but not statistically distinguishable from noise at this
sample size (2 vs. 3 events). Reported as inconclusive-but-consistent, not confirmed -- a
claim this sample cannot support was not made. Not pursued further (no syscall tracing, no
further mitigation) per the review response's own scoping: extend the limiter, re-measure,
record, then move on.

Full gate clean: `tekstide` 229 (unchanged), `tekstide-core` 594 (unchanged -- existing tests
moved/wrapped, none added or removed).

### Priority item 3: the queue-viewing surface, `ApprovalHistory`

Response 233 approved the plan surfaced in review request 233, with three changes and one
answered design question -- all applied as directed.

**A real, pre-existing gap found before writing any code**: `ProjectSession::open_surface()`
had zero readers anywhere in the GUI crate outside tests -- `view()` never branched on it, for
any of `ProjectOpenSurface`'s seven variants, including `AgentRunDetail` despite
`OpenCurrentAgentRunDetail` having had a real `AppCommand` dispatch since PR-022-D. This is the
seventh instance of "wired with no reader" this RFC has found. Confirmed by direct search
before building anything, not assumed.

**Three changes from response 233, all applied**:
- Named `ProjectOpenSurface::ApprovalHistory` (not `PendingApprovals`) -- the surface renders
  every retained request, decided and expired included, and a name promising a "pending"
  subset would be the same defect class this project has already been bitten by twice
  (`only_one_call_site_...`, `StateRootMissing`).
- Reused the existing, previously-dead `NavigationAction::OpenPendingApproval` rather than
  adding a second action beside it -- renamed to `OpenApprovalHistory` for the same naming
  reason as the surface, and given its first real `app_command_for` arm
  (`AppCommand::OpenActiveProjectSurface(ProjectOpenSurface::ApprovalHistory)`).
- `content_mode_view` is the first real `open_surface`-conditional dispatch this crate has
  had, and it is exhaustive -- all eight variants named explicitly, the six still-dormant ones
  (`AgentRunDetail` included) falling to today's unconditional editor view by their own named
  arm, not a `_ =>` catch-all. Building only `ApprovalHistory`'s real arm, not six more
  surfaces to prove the mechanism works for one -- `AgentRunDetail`'s own "selected-run
  concept" question stays RFC-020's, untouched.

**A second real, pre-existing defect found by the first real test of this dispatch, not by
inspection**: `ensure_explorer_scanned`'s incidental, background explorer-cache priming (the
first scan when a project enters Content mode for any reason) went through
`ProjectSession::scan_content_explorer_directory`, which also sets `open_surface` to
`TextEditor` as a side effect -- appropriate for `handle_explorer_key`'s own explicit rescan,
wrong for an incidental background call. Every `OpenActiveProjectSurface(surface)` for any
surface other than `TextEditor` was silently overwritten back to `TextEditor` one line after
`dispatch` set it correctly, whenever the active project's explorer had not been scanned yet.
Invisible until today because nothing ever read `open_surface` to notice. **Fixed**:
`ensure_explorer_scanned` now captures `open_surface` before the incidental scan and restores
it afterward; `handle_explorer_key`'s own direct call site is untouched, so its legitimate
"browsing the tree opens the editor" behavior is unaffected. **Ablated**: reverted the
restore, reran `opening_approval_history_from_navigation_sets_the_open_surface_and_forces_content_mode`
-- failed identically (`open_surface` reported `TextEditor` instead of `ApprovalHistory`).
Restored, reran clean.

**The design question, answered and implemented as directed**: manually opening a live entry
from the surface (`Message::OpenApprovalHistoryEntry` -> `open_approval_history_entry`) does
not consult `should_promote_to_modal` (the active-project/severity guards exist to constrain
*automatic interruption*, and both are structurally satisfied already by the user looking at
that project's own history) but does still refuse to replace an already-open modal (a
correctness rule, not a promotion guard -- the user's place in another decision is not this
surface's to discard). Reuses the exact same `ApprovalDialog` construction
`evaluate_promotion` uses, not a second inline decision UI, so "one decision, one command,
read individually" holds regardless of how the dialog was reached.

**Content**: renders every retained `ApprovalRequest` for the active project (decided and
expired included), both non-optional disclosures above the list unconditionally (the
retention-limit caveat and the classifier-limitation caveat -- response 233's priority item 4,
folded into this response since the surface itself is where that copy belongs), and a real
open control on each still-live entry only -- no bulk approval, no multi-select, matching
RFC-022's own explicit constraint. First use of `iced::widget::button`/`scrollable` in this
crate; disclosed rather than silent, since every prior interactive control in this crate has
been keyboard-driven (Tab/focus-marker/Enter) -- the history list is mouse-only for now, a
known accessibility gap, not yet a keyboard-navigable focus zone.

**Tests, all real**: `approval_history_entry_body_escapes_a_bidi_override_in_the_cwd` (the
same escaping property already proven for the dialog, proven independently for this surface's
own render function -- a second `ApprovalRequest.cwd` consumer, not assumed safe by
association), `approval_history_entry_body_distinguishes_answerable_from_expired_pending`,
`approval_history_entry_body_renders_every_decision_state_distinguishably`,
`opening_approval_history_from_navigation_sets_the_open_surface_and_forces_content_mode` (the
real navigation path, proven starting from `TerminalImmersion` so success is not an accident
of already being in the right mode),
`manually_opening_a_low_risk_live_entry_bypasses_the_promotion_predicate` (real launch, real
receive, a genuinely `Low`-risk entry that `evaluate_promotion` would never touch, opened
manually and decided for real through the same coordinator),
`manually_opening_an_entry_does_not_replace_an_already_open_modal`.

**Not built, disclosed rather than dropped**: an explicit enumeration test asserting
no-bulk-approval (priority item 5 -- satisfied by omission today, no multi-select UI exists to
violate it, but nothing fails by name if one is added later); the active-project-change
promotion-re-evaluation trigger (priority item 6, still blocked on project-switching existing
at all anywhere in the GUI).

Full gate clean: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets
--all-features -- -D warnings`, full workspace suite (`tekstide` 235, up from 229 -- six new
tests; `tekstide-core` 594, unchanged), `git diff --check`.

### Keyboard access for the history list (response 234, not blocking but required before closeout)

Response 234 accepted the surface but named the mouse-only list as more than an accessibility
note: every other interactive control in this crate is keyboard-driven (Tab/focus-marker/
Enter), so a mouse-only history list made every non-promoted proposal unanswerable *in
principle* for a keyboard user -- silently re-imposing the relabel-as-history design the owner
had already rejected (response 231: "some genuinely are answerable" assumed a working
interaction model), for most of this application's own interaction model. Called a precedent
decision, not a detail, and required before PR-022-F could close.

**Built**: `handle_approval_history_key`, the same Up/Down-moves-highlight, Enter-activates
shape `handle_explorer_key` already establishes for the sidebar's own list. A new
`state.approval_history_highlight: usize` field (the direct analogue of `explorer_highlight`,
kept separate since the two lists live in different zones with unrelated row counts). Wired
into `FocusZone::MainArea`'s existing `RoutedInput::Surface` handling alongside
`handle_editor_key`. `approval_history_entry_view` renders the same `focus_marker` convention
(`"> "`/`"  "`) every other keyboard-navigable list and modal in this crate already uses on the
currently-highlighted row.

**A second real defect found while wiring this, before it could ship**: `handle_editor_key`
had no way to know any surface other than the editor existed (`open_surface` had no real
reader before response 233's own work), so a document left open from an earlier `TextEditor`
visit kept silently absorbing keystrokes after switching to `ApprovalHistory` -- a key not
handled by the history list's own Up/Down/Enter (an ordinary character, say) would fall
through and edit the hidden document instead of doing nothing. **Fixed**: `handle_editor_key`
now returns immediately when `open_surface() == ApprovalHistory`, written as an explicit
exclusion (not "require `TextEditor`") so the six still-dormant surfaces keep falling through
to the editor exactly as `content_mode_view`'s own exhaustive match already treats them.
**Ablated**: reverted the guard, reran
`switching_to_approval_history_stops_the_hidden_document_from_absorbing_keystrokes` -- failed
with the document text becoming `"!hello"` instead of staying `"hello"`, confirming the exact
leak. Restored, reran clean.

**Four new tests, all real routing** (`crate::input::surface_input_for_test`, not
`apply_edit_key`/`handle_approval_history_key` called directly): `arrow_keys_move_the_approval_history_highlight`
(two real, retained requests -- one alone would leave nothing for Down to move to; also proves
clamping at both ends), `enter_on_the_highlighted_live_entry_opens_the_real_dialog` (the
keyboard equivalent of the mouse control, same real dialog and coordinator),
`enter_on_a_decided_highlighted_entry_does_nothing` (Enter must not act on a request with
nothing left to decide, the same property the mouse control's own conditional rendering
already enforces), `switching_to_approval_history_stops_the_hidden_document_from_absorbing_keystrokes`
(the leak fix above, ablated as described).

Full gate clean: `tekstide` 239 (up from 235), `tekstide-core` 594 (unchanged).

### The two-readers-that-can-disagree fix (response 235's non-blocking suggestion, taken)

Response 235 accepted the keyboard-access work and pointed out a structural gap it left
behind: `content_mode_view`'s exhaustive match decided which surface renders; the leak fix's
own exclusion in `handle_editor_key` separately decided which surfaces the editor absorbs keys
for. Nothing kept the two in agreement -- exactly the shape response 235 itself named as the
recurring cost of dormant state (two real bugs this response alone found from one reader
waking up), now reproduced one level deeper: two independent decision points instead of one.

**Fixed**: factored `surface_renders_editor(surface: ProjectOpenSurface) -> bool`, a single
exhaustive match (`ApprovalHistory => false`, every other variant => `true`, no `_ =>`), used
by both `content_mode_view` (to pick its render arm) and `handle_editor_key` (to decide
whether to absorb a keystroke). `content_mode_view`'s own match simplified to branch on this
predicate rather than naming all seven dormant variants itself -- the exhaustiveness guarantee
moved into the one shared function rather than being duplicated per call site. A ninth
`ProjectOpenSurface` variant now fails to compile in exactly one place until someone decides
which side of the predicate it falls on, and both call sites inherit that decision
automatically.

No new tests: the existing real-routing tests (`opening_approval_history_from_navigation_...`,
`switching_to_approval_history_stops_the_hidden_document_from_absorbing_keystrokes`) already
exercise both call sites' real behavior through both branches of the predicate; the
exhaustiveness property itself is a compiler guarantee (non-exhaustive match with no
wildcard), not something a runtime test would add confidence to.

Full gate clean, unchanged: `tekstide` 239, `tekstide-core` 594.

### Priority item 5: no-bulk-approval, enumerated

New test `no_bulk_approval_or_multi_select_construct_exists_anywhere_in_the_crate`
source-scans the whole `tekstide` crate (`scannable_source_files()`, the same helper
`no_raw_color_construction_anywhere_in_the_crate` uses) for the concrete building blocks a
bulk-decide surface would plausibly reach for first: a `checkbox` widget, a
`Vec<ApprovalId>`/`&[ApprovalId]`-shaped decide entry point, or (scanning `en.ftl` separately)
an "approve all"/"select all"/"decide all" catalog key. Fails by name if any appear.

**Disclosed as a denylist, not proof of absence** -- the same limitation already disclosed for
`reference_adapter_binary_never_executes_the_argv_it_proposes`'s own scan (response 230's
convention). It cannot prove no bulk mechanism could ever be built by some other shape
entirely; it can and does fail loudly on the obvious ones.

**Ablated three times, one per denylist entry**: added a `// ABLATION PROBE: Vec<ApprovalId>`
comment to `shell.rs` (source-scans see comment text, not only code -- the same shape
`no_raw_color_construction_anywhere_in_the_crate`'s own scan already relies on), reran --
failed naming the exact string. Reverted. Added `approval-history-approve-all = Approve All`
to `en.ftl`, reran -- failed naming `"approve-all"`. Reverted, reran clean. (The checkbox
branch was not separately ablated -- same mechanical shape as the other two, and the string
match is unconditional regardless of which of the three literals is present.)

Full gate clean: `tekstide` 240 (up from 239), `tekstide-core` 594 (unchanged).

## PR-022-F - Closeout

### What this delivered, versus what RFC-022 originally promised

Quoting RFC-022's own corrected summary (response 218, 2026-08-16) rather than paraphrasing
it, since a paraphrase is exactly where a claim this project has already gotten wrong once
would drift back up:

> This section originally read *"Make command approval reachable."* **It does not, for a real
> user, and cannot.** ... So **`Managed` — and therefore command approval — can only ever be
> exercised by the reference adapter**, which is a test artifact. What a real user gets from
> this RFC is an AgentRun at `Plain` or `Supervised`: a real AI CLI in a project-owned
> terminal, with transcript capture and audit, and no approval protocol involved.
>
> **What this RFC delivers, stated honestly:** the approval pathway exists and is proven end
> to end; AgentRuns become reachable; command approval becomes reachable *the day a real
> adapter exists*, which is not this RFC's to produce.

Everything built across PR-022-B through E matches that corrected promise, not the original
one: a real reference adapter speaking RFC-021's protocol against the real socket and
coordinator; a real, distinct spawn path (`spawn_adapter`) delivering a real per-run
capability token; a real `AgentRun` route a user can actually launch from the GUI; the full
arrival model (bounded queue, severity-gated promotion, connection-based expiry, the
`ApprovalHistory` surface with real keyboard and mouse access); and `command_approval`'s first
real, queried-and-asserted audit producer. All of it proven against real production code --
no mock adapter, no synthesised socket, anywhere in this slice's own tests.

**The four non-claims, held throughout:**

- **No claim of enforcement.** Nothing intercepts execution; a rejected adapter can run the
  command anyway. `approval-dialog-cooperative-notice`'s own wording states this to the user
  directly, not only in documentation, and is tested for exactly that (response 221/222,
  `what-the-dialog-must-not-lie-about.md`).
- **No claim that real AI CLIs are supported.** The reference adapter proves the pathway, not
  the ecosystem -- its own module doc says so, and it is never presented as evidence that a
  shipping AI CLI speaks this protocol.
- **No claim that the token is a security boundary.** It authenticates which run is asking,
  not that the asker is trustworthy, and is worthless against a hostile same-user process.
- **What this unblocks for RFC-020, stated precisely**: its two surfaces (diff review,
  AgentRun report) become *reachable* -- a real `AgentRun` route exists to build against --
  which is not the same as *done*. `AgentRunDetail`'s own "selected-run concept" question is
  RFC-020's to answer, deliberately not decided here (see the acceptance checklist's own
  stated reason).

### Two real, shipped defects found and fixed by building the first reader of dormant state

Both surfaced only because `ApprovalHistory` gave `ProjectSession::open_surface()` its first
real reader anywhere in the GUI crate -- confirmed by direct search before any of this slice's
GUI-wiring work began (`grep -rn "open_surface()" crates/tekstide/src`, no non-test matches).

- **`open_surface` clobbering, silently breaking `OpenCurrentAgentRunDetail` since PR-022-D.**
  `ensure_explorer_scanned`'s incidental, background explorer-cache priming set `open_surface`
  back to `TextEditor` as an unrelated side effect of `scan_content_explorer_directory`, so
  every `OpenActiveProjectSurface(surface)` for any surface other than `TextEditor` was
  overwritten one line after `dispatch` set it correctly, whenever a project's explorer had
  not been scanned yet. This means `OpenCurrentAgentRunDetail` has been broken by this exact
  mechanism the entire time it has existed, invisibly, because nothing ever read `open_surface`
  to notice. Fixed (`ensure_explorer_scanned` now restores the surface it found), ablated,
  and the render/absorb decision this defect lived in was further unified into one shared
  predicate (`surface_renders_editor`) so the same two-readers-can-disagree shape cannot
  recur silently for a future surface.
- **The editor keystroke leak.** A document left open from an earlier `TextEditor` visit kept
  silently absorbing keystrokes aimed at `ApprovalHistory` after switching surfaces, since
  `handle_editor_key` had no way to know any other surface existed before this slice.
  Ablated to the exact wrong value (`"!hello"` against `"hello"`) before being fixed.

Both are recorded here directly, not only cross-referenced to a review response, because they
are the most useful thing this slice learned about the reachability pattern: seven instances
of `tekstide-core` state or capability with no GUI reader had, until this slice, only ever
been found as *dormant* -- untested, but inert. These two are the first confirmation that
dormant state is not merely untested; it is actively corrupting, because nothing audits its
writers until something finally reads it.

### The flake: named mechanism, mitigation applied, effect not proven at this sample size

The RFC-021 socket flake (`bind_recovers_from_a_stale_socket_file`, response 213) and a second
test sharing its shape (`agent_run_queue_limit_is_enforced_and_only_counts_live_entries`,
found by this slice) are both explained by one mechanism, named this slice: `Command::spawn`
on Linux is fork-then-exec whenever it sets environment variables or a working directory
(every spawn in this codebase does), and between fork and exec a child holds a duplicate of
every fd open in the whole parent process -- so a socket another thread has just closed can
still appear open to a probe running moments later, in a forked-not-yet-exec'd child
elsewhere in the same test binary. An initial fd-exhaustion (`EMFILE`/`ENFILE`) hypothesis was
tested directly and **disconfirmed** (both reproductions showed a *successful* syscall, not a
failed one). The mitigation (`RealProcessLimiter` lifted to a shared, process-wide
`crate::test_support` module, applied to every real-process spawn in the approval test
modules) was applied and re-measured against the original 150-run sample: 2 failures versus 3.
**Directionally consistent with the mechanism, not statistically proven at this sample size**
-- reported as such, not overclaimed. Full reasoning in `rfcs/future-work.md`'s socket-flake
entry.

### Item 6: the active-project-change re-evaluation trigger, disclosed as blocked

RFC-022's promotion re-evaluation (`evaluate_promotion`) is wired for two of its three named
triggers -- a new arrival, and a modal closing -- but not the third, an active-project change,
because nothing in the shipped GUI switches which project is active during a session at all
(`AppState::switch_active_project` has zero production callers anywhere in this crate;
`NavigationAction::SwitchActiveProject` itself maps to no `AppCommand`). The re-evaluation
logic itself is unconditionally correct regardless of what calls it, so wiring the real
trigger once project-switching exists anywhere in the GUI is a one-line addition, not a
design question left open by this RFC. Not built here because there is nowhere in the shipped
product to build it against yet -- the same reachability-pattern shape this closeout's own
"two real defects" section describes, found a sixth and seventh time across this slice's
history (responses 233/234 both independently re-confirmed `switch_active_project`'s own
absence of a caller).

### Gates

`cargo fmt --all --check`, `cargo clippy --workspace --all-targets --all-features -- -D
warnings`, full workspace suite, `git diff --check` -- all clean. `tekstide`: 240.
`tekstide-core`: 594.

## Known limitations going in

- **Approval is cooperative, not enforced.** Nothing intercepts execution; a rejected
  adapter can run the command anyway. RFC-021's own limit, unlifted.
- **The token is not a security boundary** — it authenticates which run is asking, not that
  the asker is trustworthy, and is worthless against a hostile same-user process.
- **The reference adapter proves the pathway, not the ecosystem.** No real AI CLI speaks
  this protocol.
