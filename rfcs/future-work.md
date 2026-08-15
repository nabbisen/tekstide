# Tekstide Future Work Themes

This file tracks deferred themes after the `0.1.0` foundation release scope. It is not a substitute for detailed RFCs or issues; it is the durable index that prevents deferred work from disappearing.

## Post-0.1.0 Product Themes

### Terminal / PTY Runtime

Status: implemented by RFC-007/RFC-008/RFC-009/RFC-017/RFC-017 Amendment 1/RFC-018 with documented limitations; remaining items below are launch/close UX polish, not product UI or GUI evidence gaps.

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

#### Readiness-driven terminal I/O ("Option B") — RFC-017 Amendment 1, closed 2026-08-15

Recorded 2026-08-04 at RFC-017's closeout; **superseded 2026-08-15 by RFC-017 Amendment 1's own closeout** (`rfcs/handoffs/017-amendment-1-readiness-driven-terminal-io/qa-evidence.md`). The tick, the 10ms sleep, and the truncation-on-the-terminal-pane-path this section originally described are all gone. What follows is what that amendment actually found and what's still open, replacing the analysis below rather than sitting alongside it.

**`NFR-PERF-004` (terminal input latency p95 ≤ 16 ms): structural cause removed, criterion unverified end-to-end.** Not "met." The old "not met" verdict was proven by an arithmetic *lower* bound (the 50ms tick's poll-wait floor); "met" requires an *upper* bound on the true end-to-end path, which includes compositor/GPU present cost — this project has **no non-perturbing, in-process way to bound present latency**, so the criterion cannot be discharged under the current measurement discipline. *(Narrowed by response 210 from an earlier claim that no criterion could produce that bound "on any machine, by design." The original reason for avoiding `iced::window::frames()` is that it **forces continuous redraw once subscribed** (RFC-015 §R1) — a perturbation argument, not a proof that presentation time is unknowable. External approaches are not ruled out; they are unavailable here and not worth building. The distinction matters because the stronger phrasing made `NFR-PERF-004` permanently undischargeable by definition, which is a decision about a requirement rather than a finding about a measurement.)* **Open for the owner**: a criterion that cannot be verified under our own discipline should either be restated in terms we can bound — RFC-015 already defines and measures **input-to-state-change** without touching `frames()` — or accepted as permanently unverified. Not decided here. A headless, GUI-free benchmark proves the specific old cause is gone (sub-microsecond typical wake-to-`poll()` cost, ~500,000 real wakes/sec sustained, zero backlog). Three live GUI attempts across the amendment's own PR-A1-D were all confounded by the same shared-machine swap pressure PR-017-G's responses 155/156 first diagnosed, and were disclosed rather than reported as clean numbers, capped at three attempts per explicit review instruction.

**Terminal output throughput**: was capped at ~374 KB/s by the sleep; now ~17.4-18 MB/s, matching the flood test script's own standalone production rate — roughly a 47x improvement, and no longer an architectural ceiling independent of what's actually being written.

**`terminal_session_limit`**: raised from `Some(3)` to `Some(6)`, from a real headless N-pane measurement (not carried forward by assumption) — see `ProjectResourceLimits::default`'s own doc comment (`crates/tekstide-core/src/project/metadata.rs`) for the full per-N figures.

**Three new findings from PR-A1-D, none fixed there, all still open:**

1. **`FLOOD_SCRIPT`'s own character changed without anyone choosing it.** Under the old tick it could never exceed one drain per 50ms, so its intensity was irrelevant; it now drives ~250,000-500,000 wakes/sec headlessly — a **saturating** producer, not "bounded background output" as `NFR-PERF-004`'s own text names the test condition. The criterion has never actually been evaluated against a realistic *bounded* background load, on either architecture. Worth deciding, not urgent: whether to add a genuinely rate-limited producer for a future measurement slice, or accept the saturating script as the intentional worst case.

2. **Per-event instrumentation designed under the old tick-throttled assumption can become the bottleneck it's trying to measure**, now that wake rates are two to three orders of magnitude higher. Found directly: PR-A1-D's own `check_echo_visible` called a real `O(grid)` cost (`TerminalPane::rendered_text`) unconditionally on every wake, which was itself enough to starve a headless benchmark down to 2 of 200 samples completing in 25s at ~500,000 wakes/sec. Fixed there (a wall-clock-throttled, gated check), but this project's other per-event instruments were built against the same now-invalid assumption and have not been individually audited for the same risk.

3. **A real, reproducible property of this environment's PTY canonical-mode echo**: sending the same character repeatedly with no `Enter`, past roughly 20 accumulated characters on one unterminated line, the terminal occasionally re-echoes the *entire current line* in one wake (traced directly: a grid occurrence count jumped from 20 to 41 in a single step). Defeats any echo-detection approach based on counting occurrences of a repeated character; does not defeat one based on a fresh, distinctive marker's substring presence (PR-A1-D's fix). Not investigated further (line length? a tty/line-discipline redraw timer? shell-specific?) — the fix sidesteps needing to know, but the underlying mechanism itself is unexplained and could matter for anything else that relies on repeated-character PTY echo for detection.

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
- **`ProjectContentWorkspace::save_active_document`'s error mapping conflated a genuine
  conflict with a clean, externally-changed document — fixed 2026-08-15
  (`rfcs/handoffs/status-mapping-honesty-fixes.md`, Fix 2; response 196).** Found during
  RFC-019 PR-019-E closeout (response 184). `project/content.rs` mapped
  `SaveDecision::BlockedExternalChange` to `ProjectContentStatus::Conflict`
  unconditionally, disagreeing with `refresh_active_document` in the same file, which
  already distinguished `ExternalChangeDecision::ExternalChanged` from `::Conflict`.
  **Correction to this entry's own earlier claim**: the shell's conflict modal did *not*
  "no longer depend on this mapping" as originally written here — only its `had_local_edits`
  wording read `document.state()`; the decision to open the modal at all still gated on
  `ProjectContentStatus::Conflict` specifically, which is exactly why fixing this mapping
  broke that gate (found by the slice's own regression test, not shipped). **Fixed**:
  `save_active_document` now reads `document.state()` back, the same pattern
  `refresh_active_document` already used, producing `ProjectContentStatus::ExternalChanged`
  for a clean external change and `::Conflict` only when local edits would actually be
  lost. `render_text()` no longer renders the self-contradiction found live —
  `content status: conflict | document: external changed` on the same line for a save that
  lost nothing. **Scope amended mid-slice to authorise one shell change**:
  `attempt_save_active_document` (`crates/tekstide/src/shell.rs`) was coupled to the exact
  bug being fixed, re-reading `workspace.status()` after the save call rather than the
  `SaveDecision` the call already returned — so a correct, narrowly-scoped core fix broke
  it. Rewritten to read `error.decision()` directly off the returned
  `ProjectContentError::Save(error)`, removing the coupling rather than widening the guard
  to accommodate it (a `Conflict | ExternalChanged` match would have worked but left the
  next status refinement free to break it again, and RFC-020 is about to add statuses in
  this same area). Both the core mapping and the shell guard were ablated for real by
  reverting each independently and confirming the exact named test fails; the
  genuine-conflict case (dirty buffer, real external write) still opens the modal with
  `had_local_edits: true`, proven by a dedicated test that must not regress. Enumeration
  after the fix found no remaining `crates/tekstide` call site coupled to the
  `Conflict`/`ExternalChanged` distinction — the coupling is gone, not merely reduced.
- **The project board told the user terminals were not implemented, in a build where they
  were — fixed 2026-08-15 (`rfcs/handoffs/status-mapping-honesty-fixes.md`, Fix 1).**
  Found in PR-018-G's own baseline screenshot (`00-baseline-no-modal.png`, response 195,
  2026-08-12): the board row rendered `terminals: not implemented` and
  `agent runs: not implemented`, while `05` in the same pack showed a terminal running in
  the same build. `RuntimeSummary::default()` sets `terminal_count: None`
  (`project/runtime.rs:25`); `refresh_runtime_summary_from_collections` only raises it to
  `Some(..)` when a collection actually mutates (`project/session.rs:1181`); and
  `active_project_row`'s `.unwrap_or(CountDisplay::NotImplemented)`
  (`project_board.rs`) rendered that `None` as **"not implemented."** `None` carried two
  incompatible meanings — *the feature does not exist* and *nothing has happened yet* —
  and the label asserted the first when the truth was the second. **Fixed**: both
  `.unwrap_or(...)` calls now read `CountDisplay::Unknown` (already an existing, correctly
  labelled, but never-constructed variant), with a doc comment on `CountDisplay` itself
  recording the distinction so it cannot be re-conflated silently. `recent_project_row`'s
  five hardcoded `NotImplemented` fields (a recent, unopened project has no session to
  count from — "nothing has happened yet," the same shape, not a separate limitation) were
  fixed the same way, decided and stated rather than left ambiguous. Proven with a positive
  control before the negative (a project with a real terminal reports `KnownCount(1)`,
  *then* an empty project reports `Unknown`) and ablated for real: reverting either
  `unwrap_or` call back to `NotImplemented` fails both
  `project_rows_preserve_placeholder_field_shape_without_probing` and
  `a_project_with_a_terminal_reports_a_real_count_and_an_empty_one_reports_unknown` by
  name. No GUI change needed — `crates/tekstide`'s own `count_display_args` and the i18n
  enforcement scan's exempt-literal list already supported `CountDisplay::Unknown` before
  this fix landed.
- **BLOCKING PREREQUISITE ON ADAPTER-SPAWN: `TerminalReader` has no transcript capture, and
  the path that had it is no longer on the ingress.** Found 2026-08-15 by a scoping question
  from the dev team (review request 206, response 206), not by a failing test — nothing
  fails, and nothing will until it matters.
  `LinuxTerminalRuntime::read_available_bounded_for` (`runtime/terminal/launch.rs:115`) does
  two unrelated things in one loop: it returns a capped buffer to its caller, **and it
  appends every byte read to `session.transcript_writer` and flushes it** (`:131-136`,
  `:162-169`). Those are **the only non-test `.append(`/`.flush(` calls on a
  `BoundedTranscriptWriter` in the workspace** — it is the sole transcript producer.
  RFC-017 Amendment 1 PR-A1-A/B replaced the terminal ingress with `TerminalReader`, whose
  module contains the string "transcript" **zero times**. So the new path captures nothing.
  This is invisible today only because no production code creates an `AgentRun` (response
  200), so no transcript writer is ever configured. **Adapter-spawn is the work that makes
  `AgentRun`s real**, and whoever builds it will wire output through `TerminalReader`
  because that is the reviewed path — at which point RFC-011's whole retention design, and
  RFC-011 Amendment 1's bounded reader, read empty files forever with nothing failing to
  say so. **Do not start adapter-spawn without re-homing transcript capture first.** It
  needs a real decision, not a copied line: file I/O on the reader thread, what happens when
  a transcript write fails mid-stream, and how that interacts with the bounded channel's
  backpressure. RFC-011's territory, not a performance amendment's.
- **The `tekstide-core` test suite leaks real shell processes — roughly 87 orphaned
  `/bin/sh` per full run.** Found 2026-08-15 while diagnosing PR-A1-A's own (since-fixed)
  test flakiness, and disclosed rather than absorbed (review request 201, response 201).
  Each carries `PS1=tekstide$` and reparents to `systemd --user` after the run completes.
  **Not introduced by the reader work**: its four new tests leak zero in isolation,
  confirmed across repeated runs both alone and combined, and the ~87 come from the other
  547 tests. Whoever picks this up should check the connection to **the RFC-021 socket
  flake**, whose diagnosed mechanism was a fork window (10 failures/200 with forking tests,
  0/200 without) — a suite that leaves 87 live processes behind is a plausible source of
  the same pressure, and the two were investigated a fortnight apart without being linked.
  Belongs to general test hygiene; no RFC owns it today, which is why it is here.
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
