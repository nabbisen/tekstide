# Roadmap

Tekstide `0.3.0` completed the headless core: project/session state, root-bound file access, Linux PTY terminal runtime, terminal security boundary, AgentRun launch, transcript retention, generated-change review, and durable audit storage.

Everything from M8 onward builds the product surface. This roadmap is milestone-based rather than calendar-based. Each milestone gets its own RFC (or amendment), handoff pack, implementation review, and closeout evidence before release.

**Milestone-to-version mapping follows actual releases.** A milestone may share a release with others (as M5-M7 did in `0.3.0`), and version targets for unreleased milestones may shift.

## Milestone Schedule

| Milestone | Working Target | Theme | Primary Outcome |
| --- | --- | --- | --- |
| M4 | `0.2.0` (released 2026-07-17) | Runtime Feasibility + Terminal / PTY Foundation | PTY render/input spike, then project-scoped local terminal lifecycle foundation. |
| M5 | `0.3.0` (released 2026-07-28) | AgentRun Launch + Active File Safety | Executable AI CLI profiles, AgentRun launch, active-document external-change detection. |
| M6 | `0.3.0` (released 2026-07-28) | Transcript And Review Foundations | Bounded transcript capture, retention controls, generated-change review models. |
| M7 | `0.3.0` (released 2026-07-28) | Durable Audit | Local durable audit storage for trust decisions, managed process launches, blocked root/symlink access, and recovery outcomes. |
| M8 | `0.4.0` (released 2026-08-01) + `0.4.1` (release candidate) | GUI Foundation | Substrate decision, application shell, layout and focus model, i18n scaffolding, Project Board surface, mode switching, focus indicator. |
| M9 | `0.5.x` | Terminal Surface | Terminal renderer honoring the RFC-009 boundary, immersion mode, split policy, rendered paste protection, trusted-UI evidence. |
| M10 | `0.6.x` | Content Surfaces | Editor, file explorer tree, diff/review surface, AgentRun report surface. |
| M11 | `0.7.x` | Approval And Safety Dialogs | Command approval model and dialog, trust/safe-close/destructive dialogs, remaining security audit producers. |
| M12 | `0.8.x` | Configuration And Integrations | Configuration system, keybindings/theme/profiles, Git integration, notifications. |
| M13 | `0.9.x` | File Workflow | File watcher, multi-document model, overwrite confirmation, crash recovery. |
| M14 | `1.0.0` candidate | Release Readiness | Cross-platform support, documentation, CI, NFR verification, end-to-end QA, final security review. |

Versions are planning targets, not promises. A milestone can split into multiple releases if review shows the scope is too large.

## Why M8 Was Split

The previous roadmap carried a single `M8: Desktop GUI Runtime + Terminal Surface` milestone covering substrate selection, terminal rendering, Project Board, editor, explorer, every security dialog, audit producer wiring, accessibility, i18n, and performance verification.

A requirements gap analysis on 2026-07-28 found that is four-to-six milestones of work presented as one. M8-M13 above is that same scope, decomposed so each milestone can release, be reviewed, and fail visibly — rather than accumulating unreleased work, which is the failure mode that produced three unreleased milestones before `0.3.0`.

## Parallel Tracks

Several milestones contain **headless work that does not depend on the GUI substrate** and can proceed in parallel if GUI work blocks:

| Headless item | Nominally in | Can start |
| --- | --- | --- |
| Command approval policy, adapter capability model, risk classification | M11 | Any time — only the dialog needs the GUI |
| Configuration system, schema, validation, hot-reload policy | M12 | Any time |
| Git integration (status, branch, per-file state) | M12 | Any time |
| Notifications domain model | M12 | Any time |
| Crash recovery / unsaved buffer persistence | M13 | Any time |

Pulling these forward is a scheduling decision, not a scope change. Their rendered surfaces stay in their own milestone.

## Dependency Notes

- M8 begins with the RFC-014 substrate decision. **No product code may depend on a GUI substrate until that decision record is accepted.**
- Terminal rendering is explicit product scope: the terminal surface owns the safe ANSI/VT subset, the escape-sanitization boundary, and visual separation from approval and security dialogs.
- i18n scaffolding lands in M8, not later. Retrofitting string extraction across a built UI costs far more than designing it in.
- Command approval (M11) is the product's central safety promise. Until it lands, README and release notes must keep stating that it does not exist.
- Cross-platform remains a requirement; Linux is the primary target through M13. Windows/macOS evidence is M14 and must be produced per platform, never inferred.

## M8: GUI Foundation

Purpose:

- Choose the substrate and build the application shell that everything else renders into.

Scope:

- Desktop GUI substrate decision (RFC-014, accepted); terminal-rendering strategy is scoped in the decision record but implemented in M9, not M8.
- Application window, Content Mode / Terminal Mode layout, mode switching without animation.
- Keyboard focus model and routing; all shell navigation reachable without a mouse.
- Theme and typography primitives, with the font family/size seam plumbed through. **User-configurable** typography needs RFC-023 (M12) and is not an M8 deliverable — corrected 2026-07-30; the original wording overstated what M8 can deliver.
- **i18n scaffolding** — string catalog, locale loading, and the discipline that no user-facing string is hardcoded.
- Project Board surface rendering existing `tekstide-core` state.
- Accessibility baseline: visible focus indicators, no colour-only status, screen-reader labelling path identified.

Review gates:

- RFC-014 decision record accepted with maintainer sign-off before product code depends on the substrate.
- RFC for application shell and rendered surface model.
- RFC for i18n and localization.
- Accessibility and layout review for the shell.
- NFR evidence for warm startup (`NFR-PERF-001`) and input latency (RFC-014 R1) in `0.4.0`. **Mode switch (`NFR-PERF-002`) moves to `0.4.1`** with the mode-switching slice — corrected 2026-07-30: in M8 a mode switch toggles the Project Board against an empty content area, since Terminal Mode has no terminal until M9, so measuring it here measures scaffolding.

`0.4.0` ships the shell, input routing, text safety, and the Project Board. `0.4.1` ships mode switching and the Content-mode scaffolding for M9/M10 surfaces. See `rfcs/delivery-plan.md` §Release Cycle Tracking for the reasoning.

Explicitly not in M8: terminal rendering, editor, dialogs.

**Accessibility baseline delivered as of `0.4.1`.** No colour-only status holds (mechanically checked). Screen-reader support does not exist, stated rather than simulated (RFC-014 R2, owner-accepted) — a disclosed absence, not an M8 gap. Visible focus indicators render at the shell-chrome level since PR-015-E (`0.4.1`): border colour, border width, and a textual marker all change with `state.focus` across both `FocusZone` variants — corrected 2026-08-01, having been correctly unmet in `0.4.0` when the shell had only one focus zone.

## M9: Terminal Surface

Purpose:

- Turn the reviewed terminal security boundary into a real rendered surface.

Scope:

- Terminal renderer honoring the RFC-009 accepted-sequence policy, using the interposition strategy chosen in RFC-014.
- Terminal / Agent Immersion Mode with at most two visible panes.
- Split policy driven by real font metrics and DPI scaling (minimum columns per pane).
- Hidden session handling and a session bar with non-colour-reliant state labels.
- Paste protection wired to real paste events, with a rendered confirmation dialog.
- Trusted-UI separation demonstrated with screenshot evidence under adversarial terminal output.
- Audit producers: `paste_blocked`, `plain_terminal_observation`.

Review gates:

- RFC for terminal renderer and immersion mode.
- Security review for the rendered escape-sequence boundary and spoofing resistance.
- NFR evidence for terminal input latency under output flood (`NFR-PERF-004`).
- Screenshot-backed spoofing evidence, closing the RFC-009 deferral.

## M10: Content Surfaces

Purpose:

- Make the editing and review surfaces real.

Scope:

- Editor surface backed by RFC-006 document models; line numbers, cursor position, dirty state.
- File explorer tree backed by the bounded explorer model.
- Diff / review surface for generated changes, backed by RFC-012 models.
- AgentRun report surface: transcript view, changed files, lifecycle state.
- Syntax highlighting, only if it does not weaken correctness or latency.

Review gates:

- RFC for editor and explorer surfaces.
- RFC for diff review and AgentRun report surfaces.
- NFR evidence for typing latency in a large document (`NFR-PERF-003`).
- Review that transcript and diff content render as untrusted data.

## M11: Approval And Safety Dialogs

Purpose:

- Deliver the product's central safety promise — approve before risk — as a real workflow.

Scope:

- Command approval: adapter capability model, approval policy, risk classification, and the approval dialog showing exact command, cwd, environment policy, and risk labels.
- Trust dialog, safe-close dialog, destructive-confirmation dialog.
- Restricted Mode blocked-action surfacing.
- Audit producers: `command_approval`, `safe_close_decision`, `restricted_mode_blocked`, `project_added`.

Review gates:

- RFC for the command approval model and adapter capability contract.
- RFC for security dialogs and audit producer completion.
- Security review: approval cannot be synthesized by terminal output; denied commands do not execute.
- Evidence that Managed labels still require adapter capability evidence.

**This milestone closes the largest honesty gap in the product.**

## M12: Configuration And Integrations

Purpose:

- Give users the configuration surface the requirements promise, and the Git status integration the file explorer assumes.

Scope:

- Configuration file, schema, atomic validation, and diagnostics on invalid config (`REQ-CONFIG-001` to `007`).
- Hot reload for safe settings; security-sensitive settings never silently weakened.
- Keybindings, theme, fonts, terminal scrollback, resource limits, and AI CLI profiles as user configuration.
- Git integration: repository detection, branch, dirty state, per-file status (`REQ-GIT-001` to `007`).
- Notifications domain model and surface (`REQ-NOTIFY-001` to `005`).
- Audit producers: `sensitive_config_changed`, `transcript_purge`.

Review gates:

- RFC for the configuration system, including the security-sensitive reload policy.
- RFC for Git integration, honoring the subprocess safety rules already specified in RFC-012.
- RFC for notifications.
- Security review: config parsing cannot execute code; Git invocation avoids workspace hooks and credential helpers.

## M13: File Workflow

Purpose:

- Mature the file layer once the editor surface exists to expose it.

Scope:

- File watcher with debouncing and batching (`REQ-FILE-003`, `REQ-FILE-004`).
- Multi-document model, replacing the single-active-document limitation from RFC-006.
- Overwrite confirmation UI for externally changed files.
- Crash recovery for unsaved buffers (`REQ-RECOVER-005`).

Review gates:

- RFC for watcher and multi-document behavior.
- RFC or amendment for crash recovery.
- Tests for watcher storms, conflict resolution, and recovery correctness.
- UX review for conflict and destructive flows.

## M14: Release Readiness

Purpose:

- Ship a coherent public product, not a collection of milestones.

Scope:

- Windows and macOS support: PTY, file watching, process groups, config paths, clipboard.
- `docs/src` mdBook documentation organized by persona (new users, intermediate, maintainers).
- CI: gates, cross-platform build matrix, release automation.
- Full NFR verification against all performance and resource budgets.
- End-to-end scenario QA across Project Board → terminal → AgentRun → transcript → review → audit.
- Final security review and threat-model update.
- Real AI CLI dogfooding for supported profile flows.

Review gates:

- Cross-platform evidence per platform, not inferred.
- Documentation review against implemented behavior.
- Security and privacy review across every surface.
- Release process review.

## 1.0.0 Minimum Expectations

- Desktop GUI workbench with Project Board, Content Mode, and Terminal / Agent Immersion Mode.
- Project-scoped terminal runtime with a safe rendered terminal surface.
- AgentRun launch, transcript capture, and generated-change review.
- Command approval for adapter-supported workflows.
- Durable audit with producers for every security decision the UI can make.
- Configuration, Git status, notifications, file watching, and multi-document editing.
- Cross-platform support with per-platform evidence.
- Documentation and package metadata aligned with implemented behavior.

## Tracking Rules

- Every milestone starts with an RFC or explicit RFC amendment.
- RFC traceability continues from RFC-014; see [`rfcs/delivery-plan.md`](rfcs/delivery-plan.md) for the ordered RFC queue, scope, and dependencies.
- Every implementation slice gets a review request before closeout.
- Deferred safety/security claims must remain visible in README, changelog, roadmap, or RFCs.
- No milestone is considered complete until tests, manual evidence where needed, and release notes are updated.
- **Release at every milestone.** Three milestones accumulated unreleased before `0.3.0`; the decomposition above exists partly to prevent that recurring.
- The roadmap can change, but changes should be reviewed like product-scope changes.
