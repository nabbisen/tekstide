# Roadmap

Tekstide `0.1.0` is the foundation release: core domain model, Project Board state, navigation/mode policy, Restricted Mode policy, root-bound file access, bounded explorer state, UTF-8 text document buffers, safe save/external-change detection, and shell-visible Content Mode evidence.

This roadmap tracks the next implementation themes after `0.1.0`. It is intentionally milestone-based rather than calendar-based. Each milestone should receive its own RFC, handoff pack, implementation review, and closeout evidence before release.

## Milestone Schedule

| Milestone | Working Target | Theme | Primary Outcome |
| --- | --- | --- | --- |
| M4 | `0.2.x` | Runtime Feasibility + Terminal / PTY Foundation | GUI/TUI substrate decision, PTY render/input spike, then project-scoped local terminal lifecycle foundation. |
| M5 | `0.3.x` | AgentRun Launch + Active File Safety | Executable AI CLI profiles, AgentRun launch, and active-document external-change detection while agents run. |
| M6 | `0.4.x` | Transcript And Review Foundations | Bounded transcript capture, retention controls, and generated-change review models/harnesses. |
| M7 | `0.5.x` | Durable Audit | Local durable audit storage for trust, launch, approval, blocked access, and destructive decisions. |
| M8 | `0.6.x` | Desktop GUI Runtime + Terminal Surface | First real desktop shell, terminal/agent-immersion surface, and security/review UI surfaces. |
| M9 | `0.7.x` | File Workflow Follow-Up | Watcher, overwrite confirmation, multi-document model, and richer editor internals where needed. |
| M10 | `0.8.x`-`0.9.x` | Integration Hardening | Cross-feature UX, release automation, portability checks, and beta stabilization. |
| M11 | `1.0.0` candidate | Public Product Release | Coherent GUI AI CLI workbench with runtime, review, audit, and safety claims aligned. |

Versions are planning targets, not promises. A milestone can split into multiple patch/minor releases if review shows the scope is too large.

Dependency notes:

- M4 starts with a feasibility gate before larger terminal work: choose GUI-vs-TUI direction and the concrete primary crate, prove a PTY -> render -> keystroke loop, and measure typing/terminal latency against the performance targets.
- M4-M7 can deliver runtime, storage, policy, and headless/model evidence. Rendered panes, approval dialogs, paste dialogs, review surfaces, and audit viewer acceptance belong to the first milestone that can show those surfaces.
- Terminal rendering is explicit product scope: the terminal/agent-immersion surface must own the safe ANSI/VT subset, escape-sanitization boundary, and visual separation from approval/security dialogs.
- Active-document external-change detection moves forward to M5 because AgentRun launch increases external file write pressure. Full watcher, multi-document conflict workflows, and overwrite-confirmation UI remain M9 scope.
- Cross-platform support remains a requirement, but Linux is the primary early smoke target. Cheap Windows/macOS PTY and watcher checks should be added as soon as the relevant abstractions exist.

## M4: Runtime Feasibility + Terminal / PTY Foundation

Purpose:

- De-risk the interactive substrate first, then turn Terminal / Agent Immersion Mode from shell-visible policy into a real project-scoped terminal runtime foundation.

Scope:

- Decide the first GUI/TUI substrate direction and primary crate for the next implementation phase.
- Prove PTY output rendering and keyboard input in a small spike before committing deeper runtime work.
- Measure basic text input and terminal-loop latency against the performance targets.
- Start local shell sessions per ProjectSession.
- Preserve running terminal sessions across Content Mode / Terminal Mode switches.
- Maintain terminal lifecycle states: starting, running, exited, failed, terminating, orphaned/unknown.
- Model visible pane policy: at most two visible terminal panes once a rendered surface exists.
- Keep hidden terminal sessions running and visible through project summaries.
- Add paste-protection policy design and model-level enforcement points.
- Add safe-close assessment for real running terminal processes.

Review gates:

- RFC for substrate decision, terminal/process lifecycle, and PTY abstraction.
- Security review for Restricted Mode terminal behavior, paste protection, and terminal-rendering threat boundaries.
- Tests for cross-project isolation, lifecycle transitions, safe-close summaries, and no workspace automation startup.
- Manual smoke for PTY lifecycle on Linux primary target.
- Spike evidence for PTY -> render -> keystroke loop and latency measurement.

## M5: AgentRun Launch + Active File Safety

Purpose:

- Build AgentRun execution on top of the terminal/process foundation, with minimum active-file safety for agent-driven writes.

Scope:

- Define executable local AI CLI profiles.
- Validate launch context: project, trust state, working directory, environment policy, transcript policy.
- Launch AgentRuns from profile and project context.
- Attach AgentRuns to TerminalSessions.
- Track runtime lifecycle through real process state.
- Preserve managed/supervised/plain labels without overclaiming command interception.
- Add command approval model only where a managed adapter can actually support it; rendered approval dialogs remain surface milestone scope.
- Detect external changes for the active document while AgentRuns are active.

Review gates:

- RFC for AI CLI profile model and AgentRun launch lifecycle.
- Handoff for first supported supervised/plain profile flow.
- Security review for environment policy and compatibility labels.
- Tests for lifecycle, cross-project references, launch rejection, honest safety labels, and active-document external-change detection.

## M6: Transcript And Review

Purpose:

- Make generated work inspectable without silently retaining unbounded private output.

Scope:

- Capture bounded transcript/output for Tekstide-created AgentRuns.
- Define transcript storage path policy.
- Add retention and purge controls with model/harness evidence before rendered UI exists.
- Add per-run opt-out before launch.
- Link generated diffs/artifacts to AgentRuns when detectable.
- Add first review models or harnesses for transcript and generated changes; rendered review surfaces remain surface milestone scope.

Review gates:

- RFC for transcript retention, purge, and privacy policy.
- Tests for bounded storage, purge behavior, retention metadata, and no accidental content leakage in summaries.
- Review evidence showing transcript paths, limits, and purge behavior are inspectable.

## M7: Durable Audit

Purpose:

- Persist security-relevant decisions locally so user trust and release evidence are not only in memory.

Scope:

- Durable audit store.
- Persist trust decisions, process launches, approvals, paste blocks, safe-close decisions, blocked root/symlink access, and destructive confirmations.
- Keep audit records local.
- Avoid storing unnecessary file contents, transcript bytes, or private output in audit summaries.
- Add migration/versioning policy for audit data.

Review gates:

- RFC for audit persistence schema and retention.
- Migration/fixture tests.
- Security review for audit privacy boundaries.
- Recovery tests for corrupt/missing audit state.

## M8: Desktop GUI Runtime

Purpose:

- Replace shell-visible evidence with real desktop surfaces.

Scope:

- Select and implement the desktop GUI runtime.
- Build real Project Board surface.
- Build active Project Workspace shell.
- Build terminal/agent-immersion surface with the safe ANSI/VT subset and escape-sanitization policy.
- Build real file tree and editor surface backed by RFC-006 core models.
- Build rendered approval, paste-protection, transcript/review, and audit/security-event surfaces.
- Add keyboard/mouse/focus/dialog behavior.
- Preserve mode policy and no-overclaim safety labels.

Review gates:

- GUI runtime decision record or RFC, unless completed by the M4 feasibility gate.
- Security review for terminal rendering, approval-spoofing boundaries, paste dialogs, and audit/review surfaces.
- Accessibility and layout review for primary screens.
- NFR performance evidence for text input and terminal rendering.
- Screenshot/manual QA evidence.
- Regression tests for core state independent of GUI.

## M9: File Workflow Follow-Up

Purpose:

- Mature the file/editor layer after terminal and GUI foundations are clearer.

Scope:

- File watcher integration.
- Overwrite-confirmation UI for externally changed files.
- Multi-document tabs or another explicit multi-document model.
- Richer editor internals if `String`-backed buffers become limiting.
- Optional syntax highlighting if it does not weaken correctness.

Review gates:

- RFC or amendment for multi-document behavior.
- Tests for watcher/conflict/confirmation behavior.
- UX review for destructive or conflict-resolution flows.

## M10: Integration Hardening

Purpose:

- Stabilize cross-feature workflows before any `1.0.0` candidate.

Scope:

- Release automation/checklist hardening.
- Packaging and install smoke automation.
- Cross-platform portability checks.
- End-to-end Project Board -> terminal -> AgentRun -> transcript/review workflows.
- Real AI CLI dogfooding for supported managed/supervised/plain profile flows.
- Documentation pass for public claims.

Review gates:

- Release process review.
- Cross-feature scenario QA.
- Security/privacy review across runtime, transcript, audit, and GUI surfaces.

## M11: 1.0.0 Candidate

Purpose:

- Ship the coherent public product, not just foundations.

Minimum expectations:

- Desktop GUI workbench.
- Project-scoped terminal runtime.
- Safe terminal/agent-immersion renderer.
- AgentRun launch and lifecycle.
- Transcript/review workflow.
- Durable audit storage.
- Safe-close behavior for real running work.
- Honest managed/supervised/plain safety labels.
- Documentation and package metadata aligned with implemented behavior.

## Tracking Rules

- Every milestone starts with an RFC or explicit RFC amendment.
- Planned traceability starts at RFC-007 for M4 and continues with one RFC or amendment per milestone unless review approves a different split.
- Every implementation slice gets a review request before closeout.
- Deferred safety/security claims must remain visible in README, changelog, roadmap, or RFCs.
- No milestone is considered complete until tests, manual evidence where needed, and release notes are updated.
- The roadmap can change, but changes should be reviewed like product-scope changes.
