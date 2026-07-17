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

Status: ready for implementation review.

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

## Known Limitations

- PR-010-C does not launch an AgentRun process.
- PR-010-C does not start a `TerminalLaunchSpec`.
- PR-010-C does not map runtime lifecycle events.
- PR-010-C does not implement active-file safety.
- No transcript retention, durable audit storage, GUI launch/review surfaces, general command approval, provider-specific cloud integration, full watcher behavior, or multi-document conflict UI is claimed by RFC-010 design acceptance.
