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

  **Discharged 2026-08-16 by RFC-011 Amendment 2 (PR-A2-A through C, responses 211/212/213).**
  The writer now lives inside `TerminalReader`'s own thread (D1), write happens before send
  with the ordering proven, not assumed (D2), mid-stream failure has a real, tested policy
  per capture mode (D3), and the disk-backpressure coupling this produces is recorded as
  shipped behaviour (D4). `read_available_bounded_for` is untouched and still serves the
  agent-run subsystem's own separate call site — this closes the *terminal-pane-reader* gap
  named above, not a removal of the older path. **Adapter-spawn is still not built** — this
  discharges the one named prerequisite blocking it, nothing more; see the adapter-spawn
  entry below for what remains.
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
  **Cause found 2026-08-16** (review request 212): `Child::drop` does not kill the process, so **any test that panics before reaching its own cleanup leaks a shell**. That explains why the count varies between runs — it is a function of how many tests *failed*, not how many ran (the dev team measured 1,192 leaked while chasing a flake through repeated failing runs). The fix is cleanup that survives a panic, not a tidier happy path. Belongs to general test hygiene; no RFC owns it today, which is why it is here.
  **A second test flaking under the same pressure, found 2026-08-17** (review response 230
  asked whether a `tekstide-core` suite run it saw fail once in ten was the known socket
  flake; response 213's own rate was 10/200 — one measurement, not a fixed constant). Sampled
  150 full-suite runs on the dev machine: 3 failures. Two were the already-known
  `approval::tests::channel::bind_recovers_from_a_stale_socket_file` (identical panic each
  time: `second bind must clear the stale file and succeed: ApprovalChannelError { reason: Io,
  source: None }`, `channel.rs:168`) — confirms it is still live, not fixed by anything since
  response 213. The third was a **different** test failing the same way under the same kind
  of load: `approval::tests::coordinator::agent_run_queue_limit_is_enforced_and_only_counts_live_entries`
  (RFC-022 PR-022-E, commit `375d256` — predates this file's own most recent RFC-022 work, not
  introduced by it). That test also depends on a real socket's liveness transition completing
  within an expected window under concurrent suite pressure (`drop(first_peer)` closes one
  half of a real `UnixStream` pair from `AcceptedProposal::for_test`, then a later
  `receive_proposal` call must observe the other half as no-longer-open via
  `is_connection_still_open`'s real `recv(MSG_PEEK|MSG_DONTWAIT)`). Neither reproduced in 40
  isolated single-test reruns — both need concurrent suite pressure to surface, the same shape
  as the diagnosed fork-window mechanism. **Not confirmed to be the same root cause**, but
  consistent with one: both are real-socket/fd state-transition timing under load, and RFC-022
  has been adding more real-socket tests test-by-test (PR-022-B through E), which plausibly
  raises exposure rather than lowering it. Not root-caused or fixed in this pass — recording
  the second failure mode here so it is not lost the way response 213 warned "the count varies
  between runs" could hide before this file's cause-found note existed.
  **The `fd exhaustion` hypothesis is disconfirmed for the `bind_recovers_from_a_stale_socket_file`
  half, found 2026-08-17** (review response 231 named `EMFILE`/`ENFILE` in the preserved errno
  as the thing that would confirm it). `ApprovalChannelError`'s `Io`-reason `bind()` failure
  had one call site (`clear_stale_socket`'s catch-all `Err(_)` branch, `channel.rs`) still
  using the non-source-preserving `ApprovalChannelError::new` instead of `::io`, discarding
  the real `io::Error` at the exact site the flake's panic comes from -- fixed to preserve it
  (`ApprovalChannelError::io(error)`). Re-ran the sweep with temporary diagnostics in every
  branch of `clear_stale_socket` (180 further runs, 2 more reproductions): **the errno was
  never populated because no `io::Error` occurs at all** -- the diagnostic that fired
  immediately before the panic both times was `"connect unexpectedly succeeded"`
  (`UnixStream::connect(bind_path)` returning `Ok` when the abandoned raw listener was
  expected to have already made it fail with `ConnectionRefused`). That is a **connection
  succeeding**, not a resource-exhaustion failure -- `EMFILE`/`ENFILE` cannot be the mechanism
  for this specific flake, since there is no failed syscall carrying either errno anywhere in
  the reproduced path. The real, permanent fix (preserving the errno for the one case that
  genuinely can produce one) is committed regardless, since a future *different* failure mode
  through this same branch would otherwise still lose its errno.
  **The queue-limit test's own reproduction is the same shape from the other side**: its
  panic (`an expired entry must not continue occupying the live budget`,
  `coordinator.rs:468`) means `is_connection_still_open`'s `recv(MSG_PEEK|MSG_DONTWAIT)` probe
  did not observe an already-`drop`ped peer's closure. Both reproductions are therefore the
  same shape -- a real, synchronous, same-thread `close()` not being observed as closed by a
  liveness check moments later, under concurrent suite load -- with no confirmed kernel-level
  mechanism yet. Genuinely stranger than ordinary fork-window pressure: `close()` on a Unix
  domain socket is not supposed to leave a window where a fresh `connect()`/`recv()` can still
  observe the old, torn-down state as live. Further root-causing would need OS-level syscall
  tracing with precise timestamps under load, a materially bigger investigation than fits a
  review-response cycle -- left here rather than attempted further.
  **The mechanism, named 2026-08-17** (review response 232): `fork()`, not scheduling.
  `Command::spawn` on Linux is fork-then-exec whenever it sets environment variables or a
  working directory (every adapter/terminal spawn here does), and between the fork and the
  exec the child holds a duplicate of every fd open in the *whole parent process* --
  `CLOEXEC` closes fds at exec, not at fork, so the window exists regardless of it. A socket
  another thread has just `close()`d is still open in a forked-not-yet-exec'd child, so the
  kernel has not torn the connection down from that child's point of view: `connect()` to the
  abandoned listener's path succeeds (case 1) or `recv(MSG_PEEK|MSG_DONTWAIT)` on the
  surviving half of a closed pair returns `EAGAIN` instead of `0`/EOF (case 2) -- both of this
  file's two reproductions, explained by one mechanism, no failed syscall anywhere, and
  load-dependent in exactly the measured way (more concurrent real-process spawns means more
  fork windows overlapping socket teardown). Neither reproducing in isolation (40 runs each)
  is the confirming experiment already run, in hindsight: no concurrent forks, no window. This
  is also almost certainly what RFC-021's own originally-diagnosed "fork window" flake
  actually was -- the diagnosis was right, what was missing was that the mechanism is fd
  inheritance, not scheduling contention as such.
  **Mitigation applied and re-measured, same day.** `RealProcessLimiter`
  (`runtime::terminal::reader::tests`, response 212's own cap, previously scoped to that one
  file) lifted to a shared `crate::test_support` module and applied to every real-process
  spawn in `approval::tests::channel`/`approval::tests::reference_adapter`, so the cap is
  process-wide rather than two independent per-module pools. Re-sampled 150 further full-suite
  runs (matching the original sample size) under the shared limiter: **2 failures, versus 3 in
  the original 150** -- a small decrease, directionally consistent with the fd-inheritance
  hypothesis (fewer concurrent real spawns should mean fewer fork windows), but **not
  statistically distinguishable from noise at this sample size** (2 vs. 3 events is not a
  reliable signal either way). Reporting it as inconclusive-but-consistent rather than
  claiming confirmation a sample this small cannot support. A materially larger sample (several
  hundred runs) would be needed to detect a difference this small reliably, and was judged not
  worth the wall-clock time against the size of the effect being measured -- the mechanism
  itself is the valuable, load-bearing finding; the mitigation's exact magnitude is not.
  **Production implication, recorded rather than dismissed as test-only**: the same mechanism
  is not purely a test artifact -- spawning a terminal or adapter (a fork) while an unrelated
  approval socket is mid-teardown could transiently keep that socket alive in the forked
  child, in production too. Impact assessed as low (the window is microseconds, the listener
  is genuinely gone once the child execs, and `bind`'s own stale-file recovery exists
  precisely for leftovers like this), but not impossible, and not something a future closeout
  should describe as inapplicable to the shipped product.
  Not pursued further per its own scoping (extend the limiter, re-measure, record -- then
  move on): no OS-level syscall tracing attempted, no further mitigation attempted beyond the
  shared limiter.
- **Terminal spawn latency as a function of already-open terminals: never measured, plausibly not constant.** Surfaced 2026-08-16 while diagnosing a test-concurrency flake whose mechanism is `fork()` cost scaling with the forking process's thread count. Tekstide now runs **one reader thread per terminal** (RFC-017 Amendment 1) and `terminal_session_limit` is **6**, so launching the sixth terminal may be measurably slower than the first. RFC-017 Amendment 1 PR-A1-D's N-pane benchmark measured **throughput**, not **spawn latency** — it drained existing panes, it did not time creating them. Deliberately not measured in the slice that found it.
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

  **Discharged 2026-08-17 by RFC-022 (PR-022-B through F) — and the prediction above about
  *why* was wrong, kept rather than rewritten.** The pathway this theme described as missing
  now exists and is proven end to end: `spawn_adapter` (`runtime/terminal/launch.rs`) is the
  real, distinct spawn path for a `Managed` adapter; `AdapterApprovalConfig`/
  `inject_token_into_environment` deliver the per-run capability token to it; the arrival
  model (bounded queue, promotion, expiry) and the `ApprovalHistory` surface answer real
  proposals over a real socket, decided through the real coordinator, with `command_approval`
  audited for the first time.

  **The blocker this theme predicted never had to be resolved, because RFC-022 never took
  that path.** `ExplicitAllowlist` is still rejected today, untouched, exactly as this theme
  said it was — pinned by its own test (`agent/tests.rs`). Token delivery instead goes
  through the dedicated `AdapterApprovalConfig` mechanism above, independent of the general
  `TerminalEnvironmentPolicy` system entirely. The boundary-change decision this theme said
  an owner would need to make was never needed. Recorded here, not silently corrected away,
  because it is the second time a wrong prediction about which prerequisite actually blocks
  something has had to be unwound in this project's history (the first: the delivery plan's
  own 2026-08-16 correction) — a pattern worth a future reader seeing, not just this one
  instance of it.

  **What is still true, and is the corrected public statement**: `Managed` — and therefore
  command approval — can only ever be exercised by the reference adapter, a test artifact.
  No shipping AI CLI speaks RFC-021's protocol (RFC-022's own open question 1, answered by
  the architect 2026-08-16). A real user gets an AgentRun at `Plain` or `Supervised`: a real
  AI CLI in a project-owned terminal, with transcript capture and audit, and no approval
  protocol involved. Command approval becomes reachable *the day a real adapter exists*,
  which is not RFC-022's to produce and remains unscheduled, the same "resolve themes as they
  come" model this theme was always tracked under.

- **Workspace trust is a one-state machine: no project in the shipped app can ever leave
  `Restricted`.** Found 2026-08-16 by RFC-022 PR-022-D — the first thing that ever tried to
  pass through the trust gate (review request 219, response 219).
  `AuditCoordinator::grant_project_trust` (`audit/integration.rs`) is correct and fully
  audited, recording both `TrustGrant` authorization and application — and has **zero
  production callers**. `ProjectSession::grant_trust` beneath it is `pub(crate)` to
  `tekstide-core`. `crates/tekstide` contains no trust-granting anything. Every project
  defaults to `Restricted` at `ProjectSession::new` and stays there for the life of the
  installation.

  **The consequence is not limited to agent runs.** *Every* capability gated on workspace
  trust is permanently unreachable, and has been since the GUI existed — RFC-004's Restricted
  Mode is not a mode, it is the only state. PR-022-D surfaced it because a `Supervised`
  Claude Code profile honestly declares `MayDiscoverWorkspaceFiles`, which
  `validate_workspace_discovery_policy` refuses in a `Restricted` project. The slice
  correctly refused to weaken that declaration to route around the gate.

  **Needs its own design, not a button**: what the user is told they are authorising, whether
  it persists across sessions, whether it is revocable, and what its scope is. A security
  dialog with real consequences.

- **The systemic pattern behind four separate findings: core capability, no GUI route,
  nothing fails.** Recorded 2026-08-16 rather than logging a fourth instance. Within RFC-022
  alone: no shipping AI CLI speaks RFC-021's protocol (response 218), no code-defined
  `AiCliProfile` exists though the delivery plan describes profiles as "code-defined only"
  (218), and no trust route exists (219). Before it, RFC-020's two surfaces were scheduled
  against models nothing populates (response 200).

  **The shape is identical every time**: `tekstide-core` holds a correct, reviewed, tested
  capability; `crates/tekstide` has no path to it; no test fails, because nothing ever tried.
  Each was invisible until the previous one was cleared — a queue of prerequisites discovered
  one at a time, each by whichever slice was unlucky enough to reach it first.

  `ARCHITECTURE.md` §Evidence conventions now carries *reachability comes before
  correctness*, added after the RFC-020 instance. That convention governs **new** work. It
  does not surface the backlog of already-built capabilities with no route, which is what
  keeps being discovered. A deliberate audit — enumerate `tekstide-core`'s public
  capabilities, mark which have a production caller in `crates/tekstide` — would find the
  rest at once instead of one slice at a time. **Done 2026-08-17 — see "The reachability
  audit: results" below.**

- **The Tekstide state root lives on the transcript subsystem, and other subsystems have to
  reach through it.** Found 2026-08-16 by RFC-022 PR-022-C (review request 216, response
  216). `prepare_adapter_approval` needs somewhere to bind an approval socket, and the only
  available answer was `spec.transcript_capture.state_root` — so the approval channel now
  depends on transcript configuration for a value that has nothing to do with transcripts.

  **The immediate consequence** (required fix in that slice): `without_transcript_capture()`
  clears `transcript_state_root`, and `prepare_adapter_approval` requires it, so a `Managed`
  run that opts out of transcript capture cannot launch. It fails **closed**, which is the
  correct direction, and there is a defensible argument that an approved run should have a
  record — **but nobody made that argument.** The policy emerged from one field serving two
  purposes, and RFC-011 offers per-run opt-out as a documented privacy control.

  **The structural point outlives the fix.** The state root is conceptually *Tekstide's*,
  not the transcript subsystem's. The next subsystem needing app-level state will face the
  same reach-through, and the one after that. Whoever restructures it should expect the
  transcript field to have accumulated dependents by then.

- **RFC-021's approval protocol has no client surface, and no adapter can be written
  against it.** Found 2026-08-16 by RFC-022 PR-022-B — the first thing ever to speak the
  protocol from outside `approval::channel` (review request 215, response 215).
  `WireCommandProposal`/`WireCommandDecision` are **private to `channel.rs`**, so the
  reference adapter — a `[[bin]]` in the same *package*, which is still a separate crate for
  privacy — had to **hand-mirror their fields**, cited against the real definitions in a doc
  comment. The round trip catches renamed or newly-required fields, because deserialisation
  fails loudly; it does **not** catch a newly-added *optional* field, which the server
  defaults and the adapter then silently stops exercising.

  **Why this matters beyond the duplication.** RFC-022 scope item 6 exists precisely because
  no shipping AI CLI speaks this protocol. If speaking it requires reading `tekstide-core`'s
  private structs, none ever will — there is no specification, no client library, and no
  stable surface to implement against. This is a prerequisite for the adapter ecosystem
  RFC-022 assumes, not a detail of the test artifact that exposed it. Two shapes would
  resolve it: a published wire-format specification, or a small client module exposed for
  adapter authors. Both are RFC-021's territory.

- **A rejected adapter cannot tell why it was rejected.** Same origin (response 215).
  `approval::channel` is fail-closed **without an error frame**: on a token mismatch the
  server observes `TokenMismatch` and simply closes the connection. From the adapter's side
  that is indistinguishable from the server crashing or the socket dropping — the reference
  adapter exits `3` on EOF for both. **The security reasoning is sound** (an error frame is
  a probing oracle, and this project's fail-closed discipline is deliberate), so this is
  recorded as a **tradeoff to revisit when the protocol is specified**, not a defect to fix
  reflexively. A real cooperating adapter that cannot distinguish a bad token from a crashed
  server will retry the wrong thing.

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
- **The reachability audit: results.** Scheduled 2026-08-17 (`rfcs/handoffs/reachability-audit.md`) after RFC-022 found two real shipped defects the moment `open_surface` got its first reader — dormant state is not merely untested, it is actively corrupting, because nothing audits its writers until something finally reads it.

  **Methodology, per the handoff's own instruction, not a grep.** `#[deprecated(note = "reachability-audit")]` on 132 candidate state-changing `pub fn`s across `tekstide-core` (accessors and constructors excluded — "not every `pub fn`, only operations that create, mutate, decide, grant, launch, or persist"), then `cargo build -p tekstide` (library + binary only, test code never compiles, so a test caller can never count). Every deprecation warning names a real call site; markers reverted afterward, `git diff --stat` confirmed empty. **Caught two bugs in this method while running it** (worth recording so a repeat does not relearn them): a leading-whitespace bug in an early name-extraction pass produced false zero-caller results across the board, and a glob-exclusion bug (`*/tests/*` does not match multi-level paths like `audit/tests/integration.rs` — needs `**/tests/**`) inflated a secondary heuristic check with test-file matches before being caught and corrected.

  **28 of 132 reachable** — a genuine production call site in `crates/tekstide/src`, mostly `ApplicationShell`'s and `AppState`'s own facade methods plus RFC-022's approval/terminal plumbing. Not listed individually here; they are the ones already known to work.

  **104 of 132 dormant** — zero call sites in `crates/tekstide/src`. Of those, a secondary (heuristic, not compiler-verified) check found **30 with zero callers anywhere in `tekstide-core` either** — genuinely unreachable at every layer, not merely wrapped behind an already-reachable higher-level function. **30 is a lower bound on true orphans, not a count**: the other 74 are called by *something* else in `tekstide-core`, and a function called only by another dormant function is still dormant — this pass did not trace those 74 call chains to their own root, so some unknown number of them may turn out to be orphaned two (or more) layers deep rather than reachable (most are plausibly legitimate internal layering — e.g. `ProjectSession::launch_agent_run_with_runtime` is dormant-by-direct-call only because `AppState::launch_agent_run_with_runtime`, which *is* reachable, delegates to it — not a gap — but this pass did not confirm that reasoning for all 74). **Not traced further in this pass — a real limit of what one audit sitting can verify, disclosed rather than guessed at.** Read the 30 below as a floor, not the whole answer.

  **The 30 true orphans, with what a user consequently cannot do:**

  | Capability | Module | Consequence |
  |---|---|---|
  | `grant_project_trust` / `revoke_project_trust` | `audit::integration` | Already known (2026-08-16): no project can ever leave `Restricted` mode. |
  | `switch_active_project` | `app` | Already known (RFC-022): no way to switch which project is active in a session. |
  | `add_detected_generated_change_set` | `project::session` | Already known (item 5, re-verified): a generated change is never attached to a project session in production. |
  | `set_resource_limits` | `project::session` | **New.** No code path, anywhere, can change a project's resource limits after creation — every limit this project has spent real design effort tuning (`approval_request_limit`, `agent_run_approval_limit`, `approval_history_limit`, `terminal_session_limit`, and the rest) is fixed forever at whatever `ProjectResourceLimits::default()` produces. Not user-configurable, not project-specific, not overridable by anything shipped. |
  | `runtime::terminal::launch::resize` | `runtime::terminal::launch` | **Discharged 2026-08-17 (`rfcs/handoffs/terminal-resize.md`, accepted, review request 244).** At the time of this audit, no code anywhere called terminal resize -- `ROWS`/`COLS` were fixed constants shared by the spawned PTY's dimensions and the rendered grid, so every terminal was permanently 24×80 regardless of window size. `resize`/`resize_master` now have real callers on both paths that matter: `shell.rs` subscribes to `iced::window::resize_events()` for live drags, and separately queries the real window size once via `iced::window::open_events()`/`iced::window::size` and re-applies it from both production pane-launch call sites, so a pane launched before the user ever drags the window (the common case) is sized immediately rather than stuck at the default -- the gap the first review pass (request 243) found and the second (request 244) closed. Every trigger computes rows/cols the same way and applies them through the single `TerminalPane::resize` that updates the PTY, the emulator grid, and the pane's own stored size together. See the handoff's own closeout section for the full review-gate evidence and the one disclosed, reading-verified-not-run seam (`WindowOpened` → `window::size` → `WindowResized`). |
  | `runtime::terminal::termination::request_terminate` | `runtime::terminal::termination` | **New.** No code explicitly requests a terminal's termination through this API — whatever *does* end a terminal session goes through process exit or `Drop`, not an explicit terminate request; there may be no way to kill a hung terminal from the GUI. Worth checking against `SafeCloseDialog`'s own termination path specifically, not assumed. |
  | `purge_project_records` / `purge_all_records` | `audit::integration` | **New.** No user can ever purge audit records, at any granularity. RFC-013's own purge machinery has no GUI trigger. |
  | `purge_agent_run_transcripts` / `purge_project_transcripts` | `project::session` | **New.** Same shape, narrower granularity — RFC-011's retention/purge model has no route at the agent-run or project transcript level either. |
  | `resume` / `recover` (audit recovery) | `audit::recovery` | **New.** Interrupted-migration recovery has no GUI trigger — if the audit database is left mid-migration, nothing in the shipped app resumes or offers to resume it. |
  | `set_viewport` | `content::document` | **New.** No GUI scrolling mechanism ever sets a document's viewport — the editor, as shipped, has no scrolling. |
  | `set_git_summary` | `project::session` | **New, and the getter (`git_summary()`) also has zero readers anywhere in `crates/tekstide/src`, tests included** — the `open_surface` shape exactly. A project's git status is never actually populated for display; `ProjectBoardRow.branch_status` is plausibly always its placeholder value, never real git state, matching what `board.rs`'s own tests already hint at ("no process probe") without this audit's confirmation that the data path is entirely absent. |
  | `set_warning_state` | `project::session` | **New, partial.** Only `warning_state.has_risk_warning()` leaks into `runtime_summary` (which *is* reachable, via `project_board.rs`); the rest of `ProjectWarningState` is never read anywhere in `crates/tekstide/src`. |
  | `decide_with_edited_argv` | `approval::coordinator` | **New.** The approval dialog only ever offers Approve-once or Reject — RFC-021's protocol supports an edited-and-approved decision, but nothing in the shipped GUI can produce one. A disclosed scope choice already (the dialog's own `ApprovalDialogButton` has two variants), now confirmed as the reason by the audit rather than assumed. |
  | `accept_proposal` | `approval::channel` | Superseded, not orphaned: production exclusively uses `serve_concurrently`'s own internal accept loop. The single-connection method predates it and is retained for tests. Not a capability gap. |
  | `shutdown` (explicit) | `approval::channel` | Superseded, not orphaned: production relies on `ServeShutdown`'s own `Drop`, never an explicit non-drop shutdown call. Not a capability gap. |
  | `launch_managed_agent_run` / `apply_managed_agent_terminal_outcome` | `audit::integration` | **New.** A second, fully-audited "launch a Managed run" entry point exists alongside the one actually used (`AppState`/`ProjectSession::launch_agent_run_with_runtime`) — parallel APIs, only one wired. Worth understanding why two exist before treating this as simple dead code. |
  | `open_project_text_document` / `save_project_text_document` | `audit::integration` | **New.** Same parallel-API shape as the launch pair above, for document open/save. |
  | `add_agent_run` | `project::session` | **New.** Real launches attach an `AgentRun` via a direct internal push, not this public method — an orphaned, parallel attachment path. |
  | `add_transcript` | `project::session` | **New.** Same parallel-attachment shape as `add_agent_run`. |
  | `record_terminal_transcript_write_summary` | `project::session` | **New.** A neighbor of the (dormant-from-GUI-but-internally-called) `record_transcript_write_summary` — not traced further to confirm whether the neighbor's own reachability differs. |
  | `transition_change_set_review_state` | `project::session` | **New.** No GUI lets a user mark a generated change set reviewed, accepted, or rejected — RFC-012's review workflow has no route, consistent with `add_detected_generated_change_set` also being unreachable (nothing populates the list this would act on, either). |
  | `add_audit_event` | `project::session` | **New, likely benign.** A generic audit-event attachment path; production goes through `AuditCoordinator`'s own dedicated `record_*` methods instead. Plausibly an unused generic entry point rather than a gap, not confirmed either way. |
  | `add_approval` | `domain::agent` (`AgentRun`) | **New, likely benign.** A domain-level attachment method; approvals plausibly reach `AgentRun` bookkeeping through `ProjectSession`'s own route instead. Not confirmed either way. |
  | `set_runtime_summary` | `project::session` | **Audit candidate-list error, not a finding.** `#[cfg(test)]`-gated — not part of the real production API at all, so "dormant" is meaningless for it. Included here only so the correction is on the record rather than silently dropped. |

  **Not done in this pass, and why**: fixing any of the above (out of scope — "do not fix anything you find," per the handoff); tracing the 74 non-true-orphan dormant functions' own call chains to their root (a second pass's work, not this one's); auditing `crates/tekstide`'s own public surface against `tekstide-core`'s consumption of it (the handoff's own stated scope was one direction only).

  **Priority, per response 240's own re-ordering** (not simply "highest consequence first" — weighted by what a real user can reach today):

  1. **`resize`** — **discharged 2026-08-17**, see the row above. Was small and well-defined
     once the severity was pinned to "permanently 24×80," not a corruption risk; is now a real
     caller at both launch and live-resize, accepted with full review-gate evidence.
  2. **Trust granting** (`grant_project_trust`) — unchanged as the RFC-sized priority ahead of
     everything this audit found. It remains the single blocker on the whole agent-run chain.
  3. **`set_resource_limits`** — fold into RFC-023 (Configuration System) rather than fixing
     standalone, since a real configuration surface is where this setter belongs anyway, and
     confirms RFC-023's own scope is larger than "a file format and a parser": the setter it
     would need to drive has never been wired at all. Any doc comment elsewhere claiming a
     limit is already per-project configurable is currently false and worth a grep when this
     is picked up.

  Purge and audit-recovery are already disclosed elsewhere as absent (known gaps, not
  surprises), so they can wait behind the three above.

## Milestone Roadmap

See [`../ROADMAP.md`](../ROADMAP.md) for the milestone schedule, and [`delivery-plan.md`](./delivery-plan.md) for the ordered RFC queue, requirements gap analysis, and developer pick-up workflow.
