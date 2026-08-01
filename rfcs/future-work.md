# Tekstide Future Work Themes

This file tracks deferred themes after the `0.1.0` foundation release scope. It is not a substitute for detailed RFCs or issues; it is the durable index that prevents deferred work from disappearing.

## Post-0.1.0 Product Themes

### Terminal / PTY Runtime

Status: partially implemented by RFC-007/RFC-008/RFC-009; product UI and GUI evidence remain.

- Linux project-owned PTY shell lifecycle foundation is implemented by RFC-008 with documented limitations.
- Background terminal sessions, mode-switch preservation, visible-slot policy, and project-close assessment are implemented at the core/project layer.
- Terminal output containment, conservative ANSI/VT/OSC policy, paste input policy, and model-level trusted UI/spoofing boundaries are implemented by RFC-009 with documented limitations.
- Add app/UI commands for launching, selecting, and closing terminals.
- Add app-wide close aggregation for running terminals.
- Add terminate/keep confirmation actions and visible terminal consequence text.
- Wire paste protection into real app/UI paste events and rendered confirmation dialogs.
- Implement safe GUI terminal rendering and screenshot-backed spoofing evidence in the GUI milestone.
- Add macOS/Windows terminal runtime evidence before claiming cross-platform terminal support.

### AgentRun And AI CLI Execution

Status: partially implemented by RFC-010; GUI launch/review surfaces and command approval remain.

- Executable AI CLI profile launch, AgentRun attachment, lifecycle tracking, active-file safety, and compatibility labels are implemented by RFC-010 with documented limitations.
- Add command approval only where an adapter can actually support it.

### Transcript And Review Workflow

Status: transcript retention is implemented by RFC-011; generated-change review foundations are implemented by RFC-012 with documented limitations.

- Bounded transcript/output capture, retention metadata, opt-out, explicit purge scopes, and metadata-only local-data summaries are implemented by RFC-011 with documented limitations.
- Link generated diffs/artifacts to AgentRuns when detectable; RFC-012 implements conservative metadata-only association foundations.
- Add Git-backed detection only after its safety evidence is reviewed.
- Add rendered review surfaces for transcript and generated changes in the GUI milestone.

### Durable Audit Storage

Status: implemented by RFC-013 with documented limitations; three of twelve v1 event families have a wired producer.

- Durable local SQLite store, schema identity, migration harness, corruption diagnostics, restart-safe recovery, and explicit project/global purge are implemented by RFC-013 with documented limitations.
- Wired producers: trust decisions, managed AgentRun lifecycle, and blocked root/symlink access.
- Remaining work — producers represented in the v1 schema but not yet wired: command approval, terminal paste blocks, restricted-feature blocks, safe-close/destructive decisions, sensitive configuration changes, transcript purge, project added, and plain-terminal lifecycle observation.
- Command approval, safe-close, and configuration-change producers require rendered dialogs and move to M11, not M8 — `0.4.0` delivered the application shell and Project Board only, with no dialog surface yet. Paste blocks and project-added producers are feasible headlessly and remain available for wiring before then.
- Keep audit records local and avoid storing unnecessary file contents or private output.

### Desktop GUI Runtime

Status: substrate decided, application shell, and mode switching implemented by RFC-014/RFC-015 (`0.4.0`/`0.4.1`, RFC-015 now closed); remaining product surfaces deferred.

- Desktop GUI substrate selected (`iced`, RFC-014) and application shell implemented: window/chrome/content/modal layer composition, keyboard focus and input routing, i18n-backed text, a compiled theme, and a Project Board surface rendering real `ApplicationShell` state with untrusted names and paths escaped (RFC-015 `0.4.0`).
- Content ↔ Terminal mode switching, a visible chrome-level focus indicator, and the `NFR-PERF-002` mode-switch latency measurement all implemented in `0.4.1` (RFC-015 PR-015-E) — both against Content/Terminal-mode placeholders, since neither RFC-017's terminal grid nor RFC-019's editor exists yet. `NFR-PERF-002` needs re-checking once either does (RFC-017's own handoff carries this obligation).
- Replace the sidebar/main-area scaffolding and Project Board placeholder content with real file tree and editor surfaces (later GUI milestones, M9/M10).
- Add dialog and confirmation flows (trust, safe-close, destructive, configuration change) — M11.
- Validate responsive layout and visual polish beyond the Project Board's current row-based rendering.
- Accessibility: visible focus indicators now render at the shell-chrome level (`0.4.1`, three independent channels — border colour, border width, textual marker). Screen-reader support remains out of scope for the life of the `iced` substrate decision (RFC-014 R2, owner-accepted).

### File Workflow Follow-Up

Status: deferred after `0.1.0`.

- File watcher integration.
- Overwrite-confirmation UI for externally changed files.
- Multi-document tabs or another explicit multi-document model.
- Richer editor internals if `String`-backed buffers become limiting.

### Release Process

Status: active after `0.1.0`.

- Keep the release checklist current.
- Add release build, package, and package smoke evidence before each release.
- Decide whether future releases need scripts, `xtask`, or CI gates.
- Keep the changelog aligned with implemented and deferred scope.
- **`NOTICE` and third-party dependency trees (`iced` and similar): not owed today, becomes owed at RFC-029.** A `cargo publish` tarball redistributes only this project's own sources — confirmed for `0.4.0` by inspecting `cargo package --list`, which shows no third-party files in either crate. The Apache-2.0 §4(d) notice-propagation obligation attaches to redistributing a work, and `cargo publish` does not redistribute `iced`'s sources, only a dependency reference resolved separately by Cargo. This becomes live the day a prebuilt binary ships (RFC-029: documentation, CI, release automation, M14) — audit `iced`'s dependency tree for upstream `NOTICE` obligations then, not at every source-only release before it.

## Milestone Roadmap

See [`../ROADMAP.md`](../ROADMAP.md) for the milestone schedule, and [`delivery-plan.md`](./delivery-plan.md) for the ordered RFC queue, requirements gap analysis, and developer pick-up workflow.
