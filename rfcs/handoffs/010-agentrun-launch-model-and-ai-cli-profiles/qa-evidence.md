# RFC-010: AgentRun Launch Model and AI CLI Profiles — QA Evidence

Status: Proposed
Date opened: 2026-07-17
Date accepted: Pending

## Scope

RFC-010 defines AgentRun launch, AI CLI profiles, launch validation, AgentRun-to-TerminalSession attachment, lifecycle mapping, and minimum active-file safety for M5.

Evidence in this file must not be used to claim transcript retention, durable audit storage, final GUI launch/review surfaces, general command approval, provider-specific cloud integration, full file watcher behavior, or multi-document conflict UI unless later reviewed implementation explicitly supports those claims.

## Design Review

Review request 064 requested changes on 2026-07-17 in `.git-exclude/reviewed/tekstide-review-request-064-rfc010-agentrun-launch-design-response.md`.

Blocking review finding:

- Restricted Mode needed explicit executable provenance and implicit AI CLI workspace-config discovery launch gates.

Focused re-review request 065 accepted the amendment with notes on 2026-07-17 in `.git-exclude/reviewed/tekstide-review-request-065-rfc010-restricted-mode-executable-provenance-rereview-response.md`.

Accepted design carry-forward requirements:

- PR-010-B starts implementation with the AI CLI profile model and launch validation.
- Restricted Mode must reject workspace-local executables, wrappers, shims, symlink targets, and project-local `PATH` resolution.
- Built-in/user-global AI CLI profiles must document implicit workspace-local config/tool/prompt/plugin/instruction discovery behavior.
- Restricted Mode must disable that discovery through reviewed flags/environment or reject launch when it cannot be disabled or bounded.
- Implementation reviews must include targeted tests for executable provenance, wrapper/shim/symlink rejection, project-local `PATH` rejection, and implicit workspace-discovery blocking.

## Implementation Evidence

### PR-010-B — AI CLI Profile Model and Launch Validation

Status: accepted with notes.

Implementation:

- Added `crates/tekstide-core/src/agent.rs`.
- Added `crates/tekstide-core/src/agent/profile.rs`.
- Added `crates/tekstide-core/src/agent/launch.rs`.
- Added `crates/tekstide-core/src/agent/tests.rs`.
- Exported the new `agent` module from `crates/tekstide-core/src/lib.rs`.
- Added AI CLI profile vocabulary:
  - `AiCliProfile`
  - `AiCliProfileSource`
  - `AiCliExecutable`
  - `AiCliExecutableProvenance`
  - `ExecutableLookupPath`
  - `AiCliPromptPolicy`
  - `AiCliEnvironmentPolicy`
  - `AiCliWorkspaceDiscoveryPolicy`
  - `AiCliAdapterCapabilities`
- Added launch validation vocabulary:
  - `AgentRunLaunchRequest`
  - `AgentRunLaunchValidation`
  - `AgentRunLaunchValidationError`
  - `AgentRunLaunchValidator`
  - `AgentLaunchSummary`

Implemented validation gates:

- project id must match the ProjectSession;
- profile id must match the selected profile;
- project root and cwd must canonicalize to directories;
- cwd must stay inside the canonical project root;
- workspace-local profiles are blocked in Restricted Mode;
- workspace-local prompt templates are blocked in Restricted Mode;
- workspace-local environment files are blocked in Restricted Mode;
- workspace-local executables are blocked in Restricted Mode;
- executable symlinks or wrappers resolving inside the project root are blocked in Restricted Mode;
- project-local reviewed `PATH` lookup entries are blocked in Restricted Mode;
- implicit CLI workspace discovery is blocked in Restricted Mode unless the profile declares reviewed disabling/no-discovery evidence;
- Managed profiles require structured action approval capability;
- transcript byte persistence remains blocked pending RFC-011.

Security/privacy notes:

- Launch errors use bounded `AgentLaunchSummary`.
- Environment summaries list policy and variable names only, not values.
- The slice does not launch a process, attach AgentRuns to TerminalSessions, retain transcript bytes, persist durable audit, or implement GUI surfaces.
- The slice does not claim Managed command approval without `structured_action_approval` capability.

Observed gates on 2026-07-17:

- `cargo fmt --all --check` passed.
- `cargo test -p tekstide-core agent` passed; 28 tests passed, 0 failed.
- `cargo test -p tekstide-core` passed; 235 tests passed, 0 failed; doc tests had 0 tests.
- `cargo check --workspace` passed.
- `git diff --check` passed.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` passed.

Review follow-up:

- `.git-exclude/reviewed/tekstide-review-request-066-rfc010-pr010b-profile-launch-validation-response.md` requested changes because Restricted Mode `PATH` lookup rejection trusted caller-provided `ExecutableLookupPath::project_local` metadata without validating the lookup directory against the project root.
- `resolve_executable` now validates each lookup directory in Restricted Mode by canonicalizing the lookup directory and rejecting it when it is inside the canonical project root, regardless of the caller-provided metadata.
- Added regression tests for:
  - `ExecutableLookupPath::reviewed_system(project_root.join("bin"))` containing a project-local executable;
  - `ExecutableLookupPath::reviewed_system(project_root.join("bin"))` containing a symlink to an outside executable, proving project-local lookup is rejected before symlink target resolution.
- `.git-exclude/reviewed/tekstide-review-request-067-rfc010-pr010b-path-lookup-rereview-response.md` accepted PR-010-B with notes on 2026-07-18.
- Carry forward before runtime-backed launch: transcript policy must stay metadata-only unless RFC-011 defines retention/purge behavior; PR-010-B does not justify transcript persistence, GUI readiness, durable audit readiness, or runtime launch claims.

### PR-010-C — AgentRun Launch Spec and Terminal Attachment

Status: ready for focused implementation re-review.

Implementation:

- Added `AgentRunLaunchSpec`.
- Added `AgentRunLaunchPlan`.
- `AgentRunLaunchPlan::from_validation` turns validated profile/context into:
  - an `AgentRun` in `Ready` status;
  - a matching `TerminalLaunchSpec`;
  - retained launch metadata including executable provenance, environment summary, terminal environment policy, and workspace-discovery summary.
- Added `ProjectSession::attach_agent_launch_plan`.
- Added `ProjectAgentLaunchError`.
- ProjectSession attachment orchestration:
  - validates plan, terminal launch spec, AgentRun, and TerminalSession project ownership;
  - rejects duplicate terminal or AgentRun references before mutation;
  - rejects TerminalSession metadata that does not match the plan's TerminalLaunchSpec;
  - attaches the AgentRun to the TerminalSession;
  - records terminal environment policy metadata on the TerminalSession;
  - updates ProjectSession terminal/AgentRun collections and runtime summaries together.

Security/privacy notes:

- PR-010-C constructs metadata/specs only; it does not start a process or create runtime handles.
- AgentRun remains in `Ready`; runtime-backed `Preparing` / `Running` lifecycle transitions are deferred to PR-010-D.
- TerminalSession remains process truth for later runtime slices.
- No transcript bytes, terminal output, environment values, process ids, durable audit records, GUI surfaces, or command approval behavior are introduced.

Observed gates on 2026-07-18:

- `cargo fmt --all --check` passed.
- `cargo test -p tekstide-core agent` passed; 34 tests passed, 0 failed.
- `cargo test -p tekstide-core` passed; 241 tests passed, 0 failed; doc tests had 0 tests.
- `cargo check --workspace` passed.
- `git diff --check` passed.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` passed.

Review follow-up:

- `.git-exclude/reviewed/tekstide-review-request-068-rfc010-pr010c-launch-spec-attachment-response.md` requested changes because `AiCliEnvironmentPolicy::ExplicitAllowlist(Vec<String>)` was accepted by validation but collapsed to a generic `TerminalEnvironmentPolicy::Named("explicit allowlist")` before reaching `TerminalLaunchSpec`.
- Added `TerminalEnvironmentPolicy::ExplicitAllowlist(Vec<String>)`.
- `AgentRunLaunchSpec` and `TerminalLaunchSpec` now preserve explicit allowlist names structurally.
- ProjectSession terminal metadata records an explicit allowlist summary while leaving the structured launch policy available for PR-010-D runtime behavior.
- Added regression tests proving:
  - distinct allowlists such as `["PATH"]` and `["PATH", "HOME"]` remain structurally distinct in `TerminalLaunchSpec`;
  - attached TerminalSession metadata records explicit allowlist names.
- `.git-exclude/reviewed/tekstide-review-request-069-rfc010-pr010c-environment-policy-rereview-response.md` accepted PR-010-C with notes on 2026-07-18.
- Carry forward before broader UI or persistence claims: bound the human-readable `TerminalSession.environment_policy_ref` summary for explicit allowlists. Runtime application of `TerminalEnvironmentPolicy::ExplicitAllowlist` remains a PR-010-D gate.

### PR-010-D — Runtime-Backed AgentRun Launch Lifecycle

Status: ready for implementation review.

Implementation:

- `LinuxTerminalRuntime::launch_project_shell` now accepts AgentRun terminal kinds through the existing PTY launch path.
- `LinuxTerminalRuntime` rejects non-minimal `TerminalEnvironmentPolicy` values before process start instead of silently ignoring them.
- `TerminalLaunchSpec` now carries private launch-authority metadata so plain shell callers cannot mint AgentRun labels by mutating `kind`.
- Added `ProjectSession::launch_agent_run_with_runtime`.
- Added `ProjectSession::apply_agent_terminal_outcome`.
- Added `ProjectAgentRuntimeLaunchError`.
- Runtime-backed launch:
  - validates AgentRun launch ownership and duplicate AgentRun references before process start;
  - transitions the AgentRun from `Ready` to `Preparing`;
  - starts the matching `TerminalLaunchSpec` through `LinuxTerminalRuntime`;
  - transitions the AgentRun from `Preparing` to `Running`;
  - attaches the runtime-created TerminalSession and AgentRun to ProjectSession collections.
- Runtime summaries count an attached AgentRun and its owning active TerminalSession as one running process.
- Terminal outcomes map into AgentRun lifecycle summaries:
  - exit status `0` maps to AgentRun `Completed`;
  - nonzero exit maps to AgentRun `Failed`;
  - signal termination and timeout kill map to AgentRun `Cancelled`;
  - orphaned-unknown terminal state maps to AgentRun `Detached`;
  - terminal failure maps to AgentRun `Failed`.

Security/privacy notes:

- Unsupported environment policies fail closed before process spawn.
- Raw terminal launch specs cannot claim `TerminalKind::Supervised` or `TerminalKind::Managed`; those kinds require the validated AgentRun launch-authority path.
- Explicit allowlist variable names are preserved structurally, but the runtime does not apply or read environment values for that policy yet.
- Runtime handles and process identifiers stay runtime-only; they are not persisted into AgentRun or TerminalSession metadata.
- No transcript bytes, terminal output content, environment values, durable audit records, GUI surfaces, or command approval behavior are introduced.

Observed gates on 2026-07-18 before review request 070:

- `cargo fmt --all --check` passed.
- `cargo test -p tekstide-core agent -- --quiet` passed; 37 tests passed, 0 failed.
- `cargo test -p tekstide-core -- --quiet` passed; 244 tests passed, 0 failed; doc tests had 0 tests.
- `cargo check --workspace` passed.
- `git diff --check` passed.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` passed.

Review focus:

- Whether routing `TerminalKind::Supervised` and `TerminalKind::Managed` through the current Linux terminal runtime keeps the labels honest for PR-010-D.
- Whether pre-runtime validation plus runtime-created terminal metadata is sufficient to avoid ordinary partial attachment cases.
- Whether unsupported non-minimal environment policies should remain fail-closed until a reviewed runtime application model exists.
- Whether terminal outcome mapping is truthful enough for the M5 lifecycle boundary.
- Whether process handles and process identifiers remain outside persisted metadata.

Review follow-up:

- `.git-exclude/reviewed/tekstide-review-request-070-rfc010-pr010d-runtime-launch-lifecycle-response.md` requested changes because:
  - one runtime-backed AgentRun attached to one TerminalSession was double-counted as two running processes;
  - raw `TerminalLaunchSpec` mutation could launch `TerminalKind::Managed` without AgentRun profile capability evidence.
- Fixed runtime summary calculation so attached active AgentRuns are counted through their owning active TerminalSession only once.
- Added a regression proving one runtime-backed AgentRun reports one running process and one close resource.
- Added private `TerminalLaunchAuthority` metadata to `TerminalLaunchSpec`.
- Added crate-only `TerminalLaunchSpec::authorize_validated_agent_run`.
- `LinuxTerminalRuntime` now rejects raw non-Plain terminal specs whose launch authority is still `PlainShell`.
- `ProjectSession::validate_agent_launch_plan_before_runtime` now rejects launch plans whose terminal kind no longer matches the validated AgentRun compatibility level.
- Added regressions proving:
  - raw Managed terminal launch is rejected;
  - validated Managed AgentRun launch is accepted only through the AgentRun launch path with structured-action capability evidence;
  - `TerminatedBySignal` maps to AgentRun `Cancelled`;
  - `KilledAfterTimeout` maps to AgentRun `Cancelled`;
  - `OrphanedUnknown` maps to AgentRun `Detached`;
  - terminal `Failed` maps to AgentRun `Failed`.

Observed gates on 2026-07-18 after review request 070 fixes:

- `cargo fmt --all --check` passed.
- `cargo test -p tekstide-core agent -- --quiet` passed; 43 tests passed, 0 failed.
- `cargo test -p tekstide-core runtime::terminal -- --quiet` passed; 46 tests passed, 0 failed.
- `cargo test -p tekstide-core -- --quiet` passed; 250 tests passed, 0 failed; doc tests had 0 tests.
- `cargo check --workspace` passed.
- `git diff --check` passed.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` passed.

Second review follow-up:

- `.git-exclude/reviewed/tekstide-review-request-071-rfc010-pr010d-runtime-launch-lifecycle-rereview-response.md` requested changes because an already-authorized non-Managed `AgentRunLaunchPlan` still exposed mutable public fields that could be changed into a Managed terminal launch before runtime start.
- Made `AgentRunLaunchValidation`, `AgentRunLaunchSpec`, and `AgentRunLaunchPlan` internals private.
- Added read-only accessors for validation/spec/plan metadata.
- Kept runtime-only AgentRun status transitions and terminal launch-spec extraction crate-local to ProjectSession orchestration.
- Changed `TerminalLaunchAuthority::ValidatedAgentRun` to carry the validated `AgentCompatibilityLevel`.
- `LinuxTerminalRuntime` now checks requested terminal kind against the private validated compatibility embedded in the terminal launch authority.
- Added a regression proving an authorized Supervised terminal spec clone cannot be mutated into a Managed runtime launch.

Observed gates on 2026-07-21 after review request 071 fixes:

- `cargo fmt --all --check` passed.
- `cargo test -p tekstide-core agent -- --quiet` passed; 44 tests passed, 0 failed.
- `cargo test -p tekstide-core runtime::terminal -- --quiet` passed; 46 tests passed, 0 failed.
- `cargo test -p tekstide-core -- --quiet` passed; 251 tests passed, 0 failed; doc tests had 0 tests.
- `cargo check --workspace` passed.
- `git diff --check` passed.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` passed.

## Known Limitations

- PR-010-D launches through the Linux PTY runtime, but it does not implement active-file safety.
- PR-010-D does not apply `TerminalEnvironmentPolicy::ExplicitAllowlist` or named policies at runtime; those policies are rejected before process start.
- PR-010-D does not add post-spawn cleanup machinery for unexpected attachment-invariant failures after `LinuxTerminalRuntime` returns a fresh terminal.
- No transcript retention, durable audit storage, GUI launch/review surfaces, general command approval, provider-specific cloud integration, full watcher behavior, or multi-document conflict UI is claimed by RFC-010 design acceptance.
