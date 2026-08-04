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

#### Readiness-driven terminal I/O ("Option B") — owns `NFR-PERF-004`

Recorded 2026-08-04 at RFC-017's closeout, because everything below otherwise lives only inside a closed RFC's evidence file and would not be found by whoever picks this up.

**`NFR-PERF-004` (terminal input latency p95 ≤ 16 ms) is recorded as not met** by RFC-017 PR-017-G. RFC-014 never verified it either ("Not verified — see R1"), so this is the criterion's first evidenced verdict rather than a regression.

**Why the current architecture cannot meet it.** `terminal_demo_subscription`'s **50 ms** poll tick is the only path by which PTY bytes reach the grid, so poll-wait alone contributes an expected p95 near **47.5 ms** — roughly 3× the budget, before any PTY, VTE, layout or paint cost. This is arithmetic over a fixed, code-visible interval, not a measurement artifact. A headless benchmark confirms the update loop is *not* saturating (`poll()` costs ~10.3 ms against the 50 ms period, 21% duty), so the ceiling is the tick interval itself.

**The fix is readiness-driven I/O**: wake on PTY readability rather than polling — an async or dedicated-thread reader that blocks at the OS level and pushes a message the instant bytes arrive. This removes the interval/cost tradeoff rather than tuning it. **Shortening the tick is not an acceptable substitute**: it narrows a structural ceiling by tuning a constant whose permanent idle-CPU cost, across every open terminal pane, is unquantified.

**Scope warning.** This changes the shape of the one ingress path RFC-017 PR-017-B/C's **P1 (single ingress)** and **P2 (no side channels)** were enumerated and ablated against. It needs the same re-enumeration and re-ablation treatment those got — sized as its own slice, plausibly an RFC-009/RFC-017 amendment, not a patch.

**Two coupled `tekstide-core` defects that must be fixed *in the same change*:**

1. **The 10 ms `WouldBlock` sleep.** `read_available_bounded_for` (`crates/tekstide-core/src/runtime/terminal/launch.rs:147-150`) sleeps a hardcoded 10 ms against a caller-supplied 5 ms bound, so an idle `poll()` blocks **twice its own budget** and returns having read nothing — synchronously, on `iced`'s single update thread. It also **caps real terminal output throughput at roughly 374 KB/s** (measured): the reader sustains ~69 MB/s while actually reading, but spends about 0.5% of each tick doing so. A verbose build or a `cat` of a large log will hit that ceiling and block on write.

2. **The 64 KiB per-poll cap's truncation policy.** `read_available_bounded_for` truncates a read chunk at an arbitrary byte once `output` reaches `max_buffered_bytes`, discards the remainder, and keeps reading — feeding the emulator a byte stream **with a hole in it** — while `TerminalPane::poll()` discards the `TerminalOutputSummary` carrying `dropped_bytes`, so nothing reports that it happened.

**Why they are one change and not two.** Today `dropped_bytes` is always `0` **only because the sleep starves the reader** — about 18.7 KB accumulates per poll, nowhere near the cap. Fix the sleep alone and a 5 ms window would offer roughly 104 KB against a 64 KiB cap (two independent measurements agree on that figure), and the truncation becomes live. **P4 (stream-position independence) does not cover this**: P4 proves classification is stable across arbitrary *chunking*, where every byte still arrives. Dropped bytes are a different property and were never proven. A hole landing mid-escape-sequence leaves the parser consuming later output as that sequence's parameters.

So the cap needs a real decision — block, grow, or drop-with-a-reported-count — not the current silent truncation with the event discarded. **Fixing the sleep in isolation trades a throughput cap for a stream-corruption bug.**

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
- **The adapter-spawn pathway — what makes command approval reachable.** Named as a standing theme 2026-08-01, deliberately not scheduled: the owner's model is to resolve themes and issues as they come, not to fix a milestone for this.

  **What it is, in plain terms.** RFC-021's approval model works like this: an AI CLI runs as a *cooperating adapter*, and before executing a command it asks Tekstide over a Unix socket — "may I run this?" Tekstide classifies the risk, shows the user a dialog, and sends the answer back. All of that exists and is tested.

  What does not exist is the step that starts the adapter in the first place. `runtime::terminal::spawn_shell` only ever launches a plain interactive shell with a fixed, hard-coded environment. Nothing launches an AI CLI *as an adapter*, and nothing delivers the per-run capability token to it — `inject_token_into_environment` is built and tested with no production caller. So the protocol is complete and nobody speaks it.

  **Why it is not just scheduling.** Delivering the token requires `TerminalEnvironmentPolicy::ExplicitAllowlist`, which `launch.rs` currently *rejects as unsupported* — and that rejection is tested behaviour inside the RFC-009/RFC-010 terminal security boundary. Enabling it is a boundary change, so it needs an owner decision whenever it is picked up, not just a slot.

  **Until then**, the honest public statement stands unchanged: command approval is implemented, unreachable, and cooperative rather than enforced.

- **Audit-schema migration guide.** Owner decision 2026-08-01, reaffirmed the same day: breaking audit-schema changes are accepted **for a while** — no end date set, and recorded as a future event rather than a pending decision. A migration guide becomes required when that changes. Belongs with RFC-029 (documentation, M14) unless the end condition above arrives sooner — if breaking changes stop being acceptable before M14, the guide is needed at that point, not at M14. See RFC-013 §Schema Versioning and Migration.

- Accessibility: visible focus indicators now render at the shell-chrome level (`0.4.1`, three independent channels — border colour, border width, textual marker). Screen-reader support remains out of scope for the life of the `iced` substrate decision (RFC-014 R2, owner-accepted).

  **Recorded as a standing theme with an explicit trigger, so it is revisited rather than forgotten.** It is not "hard" — `iced` 0.14 has *no* accessibility bridge at all (grepped for `accesskit`/`a11y`: zero matches in `iced`/`iced_winit`), so no amount of effort inside Tekstide produces it. Two things would change that, and either should reopen the question:

  1. **`iced` gains an `accesskit` integration upstream.** Check at each substrate-version bump; it is a one-command check (`cargo tree -p tekstide | grep -i accesskit`).
  2. **A substrate change is contemplated for any other reason.** RFC-014's comparison recorded that `egui` ships `accesskit` as a required dependency, so accessibility posture is a live input to any future substrate decision rather than a settled one.

  Until then the honest public statement is the one already in the README: Tekstide has no screen-reader support. It must not be softened to "limited" or "planned".

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
