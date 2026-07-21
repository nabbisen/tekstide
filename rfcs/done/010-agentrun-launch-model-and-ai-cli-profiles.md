# RFC-010: AgentRun Launch Model and AI CLI Profiles

Status: Implemented with documented limitations
Target milestone: M5
Date: 2026-07-17

Related baseline documents:

- `tekstide-requirements-v0.md`
- `tekstide-external-design-v0.md`
- `tekstide-security-threat-model-v0.md`
- `tekstide-roadmap-milestones-v0.md`
- [`ROADMAP.md`](../../ROADMAP.md)

Depends on:

- [RFC-002](../done/002-core-domain-model-projectsession-terminalsession-agentrun-auditevent.md)
- [RFC-004](../done/004-security-baseline-and-restricted-mode.md)
- [RFC-006](../done/006-projectsession-state-and-file-explorer-editor-basics.md)
- [RFC-008](../done/008-terminalsession-process-lifecycle.md)
- [RFC-009](../done/009-terminal-security-boundary.md)

Blocks:

- AgentRun launch from Tekstide-owned project context;
- AI CLI profile configuration and validation;
- AgentRun-to-TerminalSession attachment behavior;
- active-document safety checks while AgentRuns are active;
- later transcript retention, generated-change review, durable audit, and GUI AgentRun surfaces.

## Summary

RFC-010 defines the first AgentRun launch model on top of the terminal/process foundation. It introduces AI CLI profiles as reviewed launch specifications, validates project/trust/cwd/environment/transcript policy before starting a run, launches an AgentRun through a project-owned TerminalSession, and keeps Plain/Supervised/Managed compatibility labels honest.

This RFC also brings forward the minimum active-file safety required for M5: when an AgentRun is launched or active, Tekstide must surface dirty or externally changed active-document state instead of silently letting agent-driven file writes collide with the user's current buffer.

RFC-010 does not implement transcript byte retention, durable audit storage, final GUI dialogs, general command approval, or provider-specific AI cloud integration.

## Motivation

RFC-002 created AgentRun vocabulary. RFC-008 made project-owned terminals real processes. RFC-009 established terminal output/input security boundaries. Tekstide can now design AgentRun launch without pretending that a placeholder terminal, transcript store, or command approval system exists.

AgentRun launch is a trust boundary. A local AI CLI can read project files, write files, run tools, emit terminal output, and create the impression that Tekstide is supervising it. The design must therefore answer:

- which executable and arguments are launched;
- which project and cwd own the process;
- which environment values are passed;
- which compatibility label is shown;
- whether workspace-local profile/env/executable/config loading is allowed;
- how AgentRun lifecycle follows terminal/process lifecycle;
- how active text buffers are protected from unnoticed external writes.

## Goals

- Define a first-class AI CLI profile model.
- Validate AgentRun launch context before any process is started.
- Launch only project-owned AgentRuns attached to project-owned TerminalSessions.
- Derive AgentRun lifecycle from real terminal/process observations where possible.
- Preserve honest Plain, Supervised, and Managed labels.
- Keep environment policy explicit and bounded.
- Block workspace-local AI profile, prompt, environment, executable, and implicit CLI config/tool discovery in Restricted Mode.
- Use metadata-only transcript policy until RFC-011 defines bounded transcript retention.
- Surface active-document dirty/external-change/conflict state before and during AgentRun execution.
- Provide implementation slices and QA evidence requirements for M5.

## Non-Goals

- Transcript byte capture, retention, purge, or transcript storage paths.
- Durable audit persistence or audit migrations.
- Final GUI launch dialogs, approval dialogs, transcript panes, or review panes.
- General command approval for arbitrary terminal commands.
- Managed command approval unless a specific adapter capability proves structured action interception.
- AI provider API integration.
- Shell command semantic safety analysis.
- VM/container sandboxing.
- File watcher completion for all open files.
- Multi-document conflict workflow.
- Loading arbitrary workspace-provided profiles, executables, wrappers, or auto-discovered CLI config in Restricted Mode.

## Design Principles

1. **Profiles are launch contracts.** A profile is not just a display name; it defines executable provenance, argv shape, compatibility label, cwd policy, environment policy, transcript policy, and CLI auto-configuration behavior.
2. **Project ownership is mandatory.** Every AgentRun belongs to exactly one ProjectSession and exactly one project-owned TerminalSession when launched.
3. **Launch fails before side effects.** Invalid project, cwd, trust, profile, environment, or transcript policy must reject launch before process start.
4. **Labels describe proven behavior.** Plain and Supervised runs must not imply managed command approval. Managed labels require adapter capability evidence.
5. **TerminalSession remains process truth.** AgentRun state follows terminal/runtime events instead of becoming a second process supervisor.
6. **File safety is visible early.** Dirty or externally changed active documents must be surfaced before launch and refreshed while AgentRuns are active.

## AI CLI Profile Model

Introduce a model equivalent to:

```text
AiCliProfile
├─ id
├─ display_name
├─ source
├─ executable
├─ argv_template
├─ compatibility_level
├─ cwd_policy
├─ environment_policy
├─ prompt_policy
├─ transcript_policy
├─ adapter_capabilities
└─ launch_label
```

The exact Rust names may differ, but the model must preserve these roles:

- `id`: stable profile reference stored on `AgentRun.profile_id`;
- `display_name`: user-facing profile label;
- `source`: built-in, user-global, or workspace-local;
- `executable`: absolute path or reviewed executable lookup result, including provenance;
- `argv_template`: bounded argument template with explicit prompt insertion behavior;
- `compatibility_level`: Plain, Supervised, or Managed;
- `cwd_policy`: project root by default; optional in-root subdirectory only after root-policy validation;
- `environment_policy`: minimal/default reviewed environment, named policy, or explicit allowlist;
- `prompt_policy`: whether a prompt is passed as argv, stdin, or omitted for interactive start;
- `transcript_policy`: metadata-only until RFC-011;
- `adapter_capabilities`: structured-action approval, lifecycle hints, transcript hints, if supported;
- `launch_label`: honest security/compatibility wording.

Initial profile sources:

- built-in profiles may be allowed in Restricted and Trusted projects only if implementation evidence documents executable provenance and whether the CLI performs workspace-local config, tool, profile, prompt, or plugin discovery;
- user-global profiles may be allowed if the project owner has configured them outside the workspace;
- workspace-local profiles are blocked in Restricted Mode by RFC-004 `WorkspaceAiProfileLoading`;
- workspace-local prompt templates and environment files are blocked in Restricted Mode by RFC-004 `WorkspaceAiPromptLoading` and `WorkspaceEnvironmentLoading`.

Executable provenance rules:

- resolved executables are classified as built-in-reviewed, user-global, system-path-reviewed, or workspace-local;
- in Restricted Mode, resolved executables must not be inside the project root or otherwise workspace-local unless a later reviewed policy explicitly allows that exact case;
- wrapper scripts, shims, symlinks, or resolved final targets inside the project root are workspace-local executables for Restricted Mode purposes;
- `PATH` lookup, if used, is a reviewed lookup result and must not prefer project-local directories in Restricted Mode;
- implementation evidence for each built-in profile must state how executable resolution avoids project-local binaries and how CLI workspace-local auto-discovery is disabled or blocked in Restricted Mode.

Implicit CLI workspace discovery:

- many AI CLIs can auto-discover project-local config, tool definitions, prompts, plugins, or agent instructions from the current working directory;
- a built-in or user-global profile must declare whether such discovery exists;
- in Restricted Mode, launch must disable discovery through reviewed flags/environment where the CLI supports it, or reject the launch when discovery cannot be disabled or bounded;
- documenting "no known workspace discovery" is acceptable only with profile-specific evidence in the implementation handoff or QA evidence.

## Compatibility Labels

Profile compatibility maps to existing AgentRun/terminal security vocabulary:

| Compatibility | Terminal kind | Claim |
| --- | --- | --- |
| Plain | `TerminalKind::Plain` | Tekstide starts or attaches to a plain terminal process. No managed approval or command interception claim. |
| Supervised | `TerminalKind::Supervised` | Tekstide owns lifecycle and may provide bounded warnings/read-models. No managed command approval claim. |
| Managed | `TerminalKind::Managed` | Managed command approval is eligible only for profile adapters with reviewed structured-action capability. |

Managed compatibility must be rejected or downgraded before launch if the selected adapter cannot prove the required capability. A Managed-looking process running as plain shell output is not a Managed AgentRun.

## Launch Context Validation

An AgentRun launch request contains:

- target `ProjectId`;
- selected AI CLI profile id;
- prompt summary and optional full prompt reference;
- requested cwd, if not default project root;
- terminal dimensions;
- transcript capture preference;
- active-document safety decision state.

Launch validation must reject before process start when:

- project id is unknown or mismatched;
- project root is missing;
- cwd is missing or escapes the canonical project root;
- selected profile is unknown;
- profile executable is unavailable or not executable;
- resolved executable provenance is workspace-local in Restricted Mode;
- reviewed `PATH` lookup would prefer a project-local executable in Restricted Mode;
- profile source is blocked by current trust state;
- workspace-local prompt/env loading is blocked by Restricted Mode;
- the selected CLI can implicitly auto-load workspace-local config, tool, profile, prompt, plugin, or instruction files and the launch cannot disable or block that behavior in Restricted Mode;
- requested Managed compatibility lacks adapter capability evidence;
- transcript byte capture is requested before RFC-011 policy exists;
- active-document state requires a user decision and no decision has been recorded.

Validation errors must be bounded summaries. They must not dump environment values, prompts, terminal output, shell history, or file contents.

## Launch Flow

The first production launch flow should be:

1. Resolve project and profile.
2. Refresh active-document external state if an active text document is open.
3. Produce an active-file safety assessment.
4. Validate trust, cwd, executable provenance, argv, implicit CLI config discovery, environment, compatibility, and transcript policy.
5. Build an `AgentRunLaunchSpec` and `TerminalLaunchSpec`.
6. Create an `AgentRun` in `Draft`, then transition to `Ready` only after validation succeeds.
7. Start the terminal runtime with the launch spec.
8. Add the returned `TerminalSession` to the ProjectSession.
9. Attach the AgentRun to the TerminalSession.
10. Transition AgentRun through `Preparing` to `Running` only after runtime launch/process-start evidence.
11. Map terminal exit/failure/termination events into AgentRun `Completed`, `Failed`, `Cancelled`, or `Detached` according to observed process state.

If terminal launch fails, Tekstide must not leave an optimistic running AgentRun. The failed launch may produce a failed metadata entry only if it is explicitly marked as failed and contains no private process output or environment dump.

## AgentRun and TerminalSession Attachment

Rules:

- one launched AgentRun attaches to one TerminalSession;
- the TerminalSession must belong to the same ProjectSession;
- duplicate attachment is idempotent only when it references the same terminal;
- attaching a second terminal to the same AgentRun is rejected;
- attaching an AgentRun to a terminal from another project is rejected;
- deleting or closing the terminal must leave AgentRun lifecycle in a truthful terminal-derived state.

The TerminalSession is the runtime/process owner. AgentRun stores references and lifecycle summary, not PTY handles, process ids, environment data, or output bytes.

## Environment Policy

The default policy is minimal and explicit. It may include only the values required for the chosen executable to run and for common terminal behavior, with names documented in implementation evidence.

Rules:

- no automatic `.env` loading in Restricted Mode;
- no workspace scripts, hooks, profile files, prompt templates, tool definitions, plugins, agent instruction files, wrappers, shims, or executables in Restricted Mode;
- `PATH` lookup must be deterministic and reviewed; it must not resolve through project-local directories in Restricted Mode;
- user-global profile configuration must be outside the project root;
- environment summaries may list variable names and policy source, not values;
- launch diagnostics must never print full environment maps;
- named environment policies must be reviewed before use by AgentRun launch.

## Transcript Policy

RFC-010 uses `TranscriptPrivacyPolicy::metadata_only_until_retention_ready()` semantics.

Rules:

- no transcript bytes are persisted by RFC-010;
- no terminal scrollback is promoted to transcript storage;
- AgentRun metadata may record profile id, prompt summary, terminal reference, lifecycle status, and bounded launch errors;
- full prompt storage remains a reference only unless a later RFC defines storage policy;
- RFC-011 owns bounded transcript retention, opt-out behavior, purge, and local paths.

## Active-File Safety

AgentRun launch increases the chance that an external process changes a file currently open in Tekstide. RFC-010 therefore requires minimum active-document safety.

Before launch:

- if there is an active text document, refresh its external state using the existing RFC-006 safe-open/root policy;
- if the active document is clean and unchanged, launch can proceed;
- if the active document is dirty, externally changed, or in conflict, launch must surface that state before process start;
- if policy requires a decision, launch must be blocked until the decision is recorded.

While an AgentRun is active:

- Tekstide must provide a model path to refresh active-document external state;
- external-change/conflict state must remain visible in project/content summaries;
- save behavior must continue to block overwrite on external-change conflicts;
- active-file safety evidence may be model/harness-based before the final GUI exists.

RFC-010 does not require a full file watcher or multi-document conflict UI. Those remain M9 scope unless review decides to split active-file safety into a follow-up RFC.

## Security and Privacy

- AI CLI output is terminal output and remains governed by RFC-009.
- Plain and Supervised AgentRuns do not claim command interception.
- Managed AgentRuns require adapter capability evidence before launch.
- Restricted Mode blocks workspace-local AI profiles, prompts, environment files, executables, wrappers, shims, auto-discovered CLI config/tool files, plugins, tasks, and background automation.
- Launch errors, environment summaries, and diagnostics are bounded and metadata-only.
- No transcript bytes or private terminal output are persisted by this RFC.
- Durable audit remains deferred to RFC-012; RFC-010 may create in-memory/domain audit metadata only where existing models require it.
- Terminal output cannot approve launches, mutate trust state, synthesize approvals, or dismiss trusted UI.

## Persistence

RFC-010 may add domain metadata needed for:

- AI CLI profiles;
- launch specs or launch validation results;
- AgentRun-to-TerminalSession attachment;
- active-file safety assessments;
- bounded launch failure summaries.

RFC-010 must not persist:

- runtime handles;
- process ids as durable truth;
- PTY bytes;
- terminal scrollback;
- transcript bytes;
- environment values;
- raw prompts unless a reviewed storage policy exists.

If local schema/state files change, the implementation must include migration or no-migration evidence.

## Test Plan

- Profile validation tests for known/unknown profiles, executable availability, executable provenance, argv/prompt policy, CLI auto-discovery policy, and compatibility labels.
- Restricted Mode tests blocking workspace-local AI profile, prompt, environment, executable, wrapper/shim, project-local `PATH`, and implicit CLI workspace-config loading.
- Environment policy tests proving summaries list policy/variable names without values.
- Launch validation tests for unknown project, wrong project, missing root, invalid cwd, cwd escaping root, and unsupported Managed capability.
- AgentRun lifecycle tests for validation success/failure, runtime launch success, process failure, terminal exit, cancellation, and detached/orphaned cases.
- Attachment tests for same-project success, cross-project rejection, duplicate idempotence, and duplicate-terminal rejection.
- Transcript policy tests proving no transcript byte persistence is enabled.
- Active-file safety tests for clean, dirty, externally changed, and conflict states before launch.
- Safe-save regression tests proving external-change conflicts still block overwrite while an AgentRun is active.
- Terminal security regression tests proving terminal output cannot mutate launch/trust/approval state.

## Acceptance Criteria

- AI CLI profiles are represented as reviewed launch contracts.
- AgentRun launch validates project, root, cwd, profile source, executable provenance, CLI auto-discovery behavior, environment, compatibility, transcript policy, and active-file safety before process start.
- Restricted Mode blocks workspace-local AI profile, prompt, environment, executable, wrapper/shim, project-local `PATH`, and implicit CLI workspace-config loading.
- AgentRuns launch through project-owned TerminalSessions.
- AgentRun lifecycle follows runtime/terminal observations and does not claim process truth independently.
- AgentRun-to-TerminalSession attachment rejects cross-project and duplicate-invalid references.
- Plain/Supervised/Managed labels remain honest and Managed launch requires capability evidence.
- Environment summaries avoid values and private data.
- Transcript behavior remains metadata-only pending RFC-011.
- Active-document dirty/external-change/conflict state is surfaced before launch and while AgentRuns are active.
- Tests and QA evidence cover launch rejection, lifecycle, attachment, Restricted Mode, transcript non-persistence, and active-file safety.

## Risks and Mitigations

- **Profiles can become hidden automation.** Treat profiles as launch contracts and block workspace-local profile/env/prompt loading, workspace-local executable provenance, project-local `PATH` resolution, and implicit CLI workspace-config discovery in Restricted Mode.
- **Managed labels can overclaim.** Require explicit adapter capability evidence before Managed launch.
- **Terminal lifecycle and AgentRun lifecycle can diverge.** Keep TerminalSession as process truth and map runtime observations into AgentRun summaries.
- **Environment diagnostics can leak secrets.** Store names/policies, not values, and bound all error summaries.
- **AgentRun writes can race the editor.** Refresh active document state before launch and preserve existing safe-save conflict blocking.
- **Scope can expand into transcripts/audit/UI.** Keep transcript retention, durable audit, and final GUI surfaces in later RFCs.

## Open Questions

- Should active-file safety remain fully inside RFC-010 implementation, or should review split the while-running refresh model into a small RFC-010 amendment?
- Which built-in AI CLI profile should be the first implementation target: a plain interactive CLI profile, a supervised profile with lifecycle labels, or both?
- Should user-global profiles be editable before the GUI milestone, or should the first implementation use code-defined/built-in profiles only?
- What minimum adapter capability evidence is sufficient to allow a Managed profile name with no command-approval UI? Default answer for implementation is no Managed label unless reviewed structured-action capability evidence exists.
