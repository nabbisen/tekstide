# Tekstide Future Work Themes

This file tracks deferred themes after the `0.1.0` foundation release scope. It is not a substitute for detailed RFCs or issues; it is the durable index that prevents deferred work from disappearing.

## Post-0.1.0 Product Themes

### Terminal / PTY Runtime

Status: implemented by RFC-007/RFC-008/RFC-009/RFC-017/RFC-018 with documented limitations; remaining items below are launch/close UX polish, not product UI or GUI evidence gaps.

- Linux project-owned PTY shell lifecycle foundation is implemented by RFC-008 with documented limitations.
- Background terminal sessions, mode-switch preservation, visible-slot policy, and project-close assessment are implemented at the core/project layer.
- Terminal output containment, conservative ANSI/VT/OSC policy, paste input policy, and model-level trusted UI/spoofing boundaries are implemented by RFC-009 with documented limitations.
- Add app/UI commands for launching, selecting, and closing terminals.
- Add app-wide close aggregation for running terminals.
- Add terminate/keep confirmation actions and visible terminal consequence text.
- Real clipboard paste is wired to app/UI paste events with a rendered confirmation
  dialog, implemented by RFC-018 with documented limitations (`0.5.1`).
- Safe GUI terminal rendering and trusted-UI evidence are implemented by RFC-017/RFC-018
  with documented limitations (`0.5.0`/`0.5.1`) — the dialog is distinguishable from
  terminal-output imitation by keystroke suppression under a live positive control, not
  by whether it visibly occupies chrome, which is content-dependent and disclosed rather
  than relied on.
- **A background scrim behind the paste-confirmation dialog — recommended, decided, and
  implemented 2026-08-12 as RFC-018 PR-018-G.** RFC-018 PR-018-E found the dialog's
  original "occludes chrome" evidence angle was content-dependent (an attacker who keeps a
  paste short can keep the dialog entirely inside the terminal's own pane, response 175),
  and named a background dimming/scrim as the fix for that specific weakness: unlike the
  spatial tell, a scrim is **content-independent** — it does not depend on what the
  attacker pastes. RFC-018's own task breakdown said explicitly, twice (PR-018-E's entry
  and PR-018-F's own scope), that **PR-018-F should decide whether to recommend it**.
  **It never did** — `qa-evidence.md`'s PR-018-F section covers every other carried-forward
  item (both audit-observability gaps, the spatial-property limitation, the RFC-022 note)
  but contains no mention of dimming or a scrim at all, found by grep while following this
  handoff after `0.6.0` shipped. Not implemented in RFC-018 (deliberately — response 173
  explicitly told PR-018-E not to add background-dimming or any other visual change while
  it was still gathering evidence, so evidence work would not also change what it
  evidenced) and not decided at closeout either, so the recommendation sat unactioned from
  2026-08-10 until this handoff packaged it into one concrete ask, accepted by the owner
  2026-08-11 (`pr-018-g-background-scrim.md`).

  **Outcome**: yes, built. A full-window dimming layer (`crate::theme::Theme::scrim`,
  `shell.rs`'s `modal_scrim_style`) reusing the existing modal layer every `ModalContent`
  variant already shares — one `.style(...)` call added to the container `opaque` already
  wrapped, not a second widget or a second input-capturing surface.
  `SubscriptionMode::for_modal` plus the `is_none()` guard remain the one mechanism that
  protects the user; the scrim is additive cosmetics, verified not to consume input (no
  `mouse_area`/`on_press` added anywhere) and live-verified not to dismiss the dialog on
  click. Content-independence demonstrated at both ends of the range that broke the
  original spatial claim: a short 2-line paste and a longer 3-line paste both show chrome
  outside the dialog dimmed, regardless of how the dialog's own size changes with pasted
  content. PR-018-E's suppression positive control re-run and still passing. Full evidence,
  screenshots, and an ablation naming the exact test that catches its own removal:
  `qa-evidence.md`'s PR-018-G section. **Still not claimed**: that the scrim makes the
  dialog unspoofable (it raises the cost of a convincing imitation; it does not eliminate
  one) or that the original spatial property is now sound (it was replaced, not repaired —
  RFC-018's own disclosed limitation stands unchanged).
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

**A third motivation, added 2026-08-08 by the terminal-launch-UX handoff, and the most user-facing of the three.** `ProjectResourceLimits::default().terminal_session_limit` is `Some(3)` — not chosen for genuine multi-tasking headroom, but because `Message::TerminalPollTick` polls every live pane sequentially and each `poll()` carries the same 10 ms sleep: measured linear at ~10.1 ms/pane against the 50 ms tick, saturating at 5 panes, leaving 3 as the largest count with real headroom (~20 ms) today. The other two motivations are invisible to a user — a latency number nobody sees, a throughput ceiling only heavy output hits. **This one is not**: a user with a build running, a log tailing, and a shell open is already at the limit, and a fourth terminal is refused, by design, because of this same sleep. **The limit is expected to rise once readiness-driven I/O removes the per-poll sleep it is currently a function of** — raising it without that fix would reopen the saturation risk this default exists to prevent.

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
- Content mode is real end to end as of RFC-019 (`0.6.x`, PR-019-B through E): a real
  explorer tree (untrusted names/status escaped) in the sidebar; a real, cursor-aware
  editor (text area raw, chrome escaped, `RFC-006 Amendment 1`'s cursor-forwarding
  accessor wired) in the main area; real save with a real, distinguishing conflict
  dialog (`ProjectContentStatus::Conflict` covers both a genuine dirty conflict and a
  clean externally-changed document — the dialog's wording now reads each correctly).
  Undo history remains out of scope (RFC-019 non-goal, inherited from RFC-006). Diff
  review and the AgentRun report are RFC-020, M10's second half.
- **`ProjectContentWorkspace::save_active_document`'s error mapping does not distinguish
  a genuine conflict from a clean, externally-changed document — a real `tekstide-core`
  defect, found during RFC-019 PR-019-E closeout (response 184), not RFC-019's to fix
  since it renders core state rather than owning it.** `project/content.rs:174` maps
  `SaveDecision::BlockedExternalChange` to `ProjectContentStatus::Conflict`
  unconditionally, regardless of whether the buffer was actually dirty. This disagrees
  with `refresh_active_document` in the same file (lines ~224–227), which correctly
  distinguishes `ExternalChangeDecision::ExternalChanged` from `::Conflict`. The two
  variants `ProjectContentStatus` already has prove the coarseness at line 174 is a
  defect, not a deliberate simplification. **Consequence**: `workspace.status()` reports
  `Conflict` for a clean external change, so any future consumer of `status()` — not only
  the shell's own conflict modal, which now reads the more authoritative
  `document.state()` instead and no longer depends on this mapping — gets the wrong
  answer; `render_text()` also renders this status, so `tekstide-core`'s own pre-GUI
  harness reports "conflict" for a save that lost nothing. Fix belongs in
  `project::content::save_active_document`: read `document.state()` (or the document's
  own dirty flag at the point of the error) the same way the shell-side fix now does,
  rather than collapsing both cases before the caller ever sees them.
- **The project board tells the user terminals are not implemented, in a build where they
  are.** Found in PR-018-G's own baseline screenshot (`00-baseline-no-modal.png`, response
  195, 2026-08-12): the board row renders `terminals: not implemented` and
  `agent runs: not implemented`, while `05` in the same pack shows a terminal running in
  the same build. `RuntimeSummary::default()` sets `terminal_count: None`
  (`project/runtime.rs:25`); `refresh_runtime_summary_from_collections` only raises it to
  `Some(..)` when a collection actually mutates (`project/session.rs:1181`); and
  `active_session_row`'s `.unwrap_or(CountDisplay::NotImplemented)`
  (`project_board.rs:192-195`) renders that `None` as **"not implemented."** So `None`
  carries two incompatible meanings — *the feature does not exist* and *nothing has
  happened yet* — and the label asserts the first when the truth is the second. A
  freshly-opened project claims the feature is absent; launching one terminal silently
  flips the same line to `terminals: 1`. **Same shape as the `content.rs:174` entry
  above** — a status mapped unconditionally where the truth is conditional — and it is a
  false statement in trusted chrome, the category RFC-018 exists to defend. The fix is
  not to default the count to `Some(0)` at construction (that just moves the guess): it is
  to stop overloading `None`, so "unknown" and "not implemented" are separate states and
  the board can say `terminals: 0` when it means zero. `agent_run_count` has the identical
  defect on the same two lines, and `recent_project_row` (`project_board.rs:245-250`)
  hardcodes `NotImplemented` for five fields where the honest answer is "no open session."
- **No `NavigationAction` reaches `AppCommand::OpenActiveProjectWorkspace` directly.**
  Found during RFC-019 PR-019-C's GUI evidence work (response 181, 2026-08-11):
  `SwitchActiveProject`'s own keybinding is `None`/`Configurable`, already disclosed as
  such in `navigation.rs`. The `ActiveProjectWorkspace` route is only reached as a side
  effect of `Ctrl+Alt+M` (`ToggleActiveProjectMode`) or `Ctrl+Alt+T` (`LaunchTerminal`)
  succeeding — a real gap, not a documented non-goal, and one a future keybinding pass
  (RFC-023) should close directly rather than leaving every workspace entry point to
  borrow a side effect from an unrelated command.
- **The `no_count_display_or_attention_label_is_called_anywhere_in_the_crate` scan matches
  only the literal substring `.label()`, so it cannot catch a hardcoded-English *free
  function* (one not called as a method).** Raised at RFC-019's own design stage (its
  handoff named the four free functions this exact gap would miss, before implementation
  started, so review caught none of them) and again at PR-019-E closeout (response 182):
  **raised here, not absorbed into RFC-019** — the scan lives in `i18n::enforcement`,
  which is nobody's territory to widen under a rendering RFC. Whoever next touches
  `i18n::enforcement` should decide whether to broaden the scan to match free-function
  calls generally, or accept that every future producer of this shape needs naming in
  its own RFC the way RFC-019 named its four.
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

### CLI Entry Point

Status: recorded 2026-08-08 during `0.5.0`'s post-publish verification — found by running the actual `cargo install`ed binary, not by inspection.

**`tekstide --version` does not work**, and the failure is actively misleading rather than merely absent. `main.rs`'s `boot()` treats every argument as a project path (`std::env::args_os().skip(1)`, feeding each straight to `add_project_from_path`), so the most reflexive command after installing a CLI tool produces `folder does not exist: --version` — telling the user their flag was read as a path, a worse first impression than an "unrecognised option" error would be. Not a regression; the binary has behaved this way since it existed, and nothing in `0.5.0` changed it. But `0.5.0` is the first release whose headline feature (`Ctrl+Alt+T`) invites people to actually install and run it, so the gap is newly worth fixing rather than newly introduced.

**One change closes two gaps.** `i18n.rs`'s `LocalePreference::cli_flag` has existed since RFC-016 PR-016-D with no production caller — the doc comment says so plainly ("no CLI flag parsing exists yet in `main.rs`"). Real argument parsing in `boot()`/`main()` (distinguishing `--version`/`--help`/a future `--locale`/`--config` from positional project paths) is the same piece of work `cli_flag` has been waiting for and what `--version` needs, and RFC-023's configuration system will need argument parsing regardless. Do them together rather than adding a one-off `--version` special case ahead of the real parser.

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
