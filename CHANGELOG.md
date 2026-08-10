# Changelog

## 0.5.1 - Paste Protection and Trusted-UI Evidence

Status: release candidate, prepared 2026-08-10, pending review and the owner's signed tag.

Tekstide `0.5.1` completes milestone M9 with RFC-018, the second half of the terminal
work `0.5.0` began. **RFC-018 is closed** (`rfcs/done/`). The `0.5.0`/`0.5.1` split
follows the same shape as `0.4.0`/`0.4.1`: one milestone, two releases, because the
scope was too large for one.

Press **`Ctrl+Shift+V`** to paste into a focused terminal. What happens next is decided
by RFC-009's policy, not by the paste widget.

### Implemented

- **Real clipboard paste, classified before it reaches the shell (RFC-018).** Pasted
  bytes go through `TerminalInputPolicy::evaluate` before any PTY write. A single-line
  paste is allowed; a multi-line paste opens a confirmation dialog; a paste containing
  control characters is refused outright. Paste reaches the PTY through the same single,
  modal-gated ingress keystrokes already used — it did not get its own.
- **A real confirmation dialog**, built on the existing modal layer. Every dismissal path
  defaults to **not** pasting: Escape cancels regardless of which button is focused, and
  only an explicit accept writes anything.
- **The pasted content is shown, escaped.** The preview runs through the same
  untrusted-text path the Project Board uses, so a paste containing bidi-override or
  control characters renders as `<U+XXXX>` markers rather than reordering the dialog's
  own text. Newlines are escaped too, so pasted content cannot fabricate extra rows that
  imitate the dialog's controls.
- **Audit: `paste_blocked` has a producer.** A policy-refused paste is recorded. A
  sentinel test proves no pasted content, clipboard text, or command text reaches the
  durable store, checked against raw on-disk bytes.
- **Trusted-UI evidence**, nine screenshots against a real terminal running live output.

### What the trusted-UI evidence shows, and does not

One property distinguishes the genuine paste dialog from terminal output imitating it:
**while the dialog is open, keystrokes never reach the terminal.** That was demonstrated
live, with a positive control proving the keystrokes were reaching the application at the
time — so their absence is suppression, not non-delivery. An imitation drawn by terminal
output cannot suppress input.

The terminal grid can never render outside its own pane, so chrome is always authentic.
But whether the genuine dialog *visibly* uses that headroom depends on how wide its
preview is — which depends on the pasted content, which an attacker may influence. It is
therefore recorded as an architectural fact and **not** offered as something a user can
rely on seeing.

**This evidence shows an imitation cannot occupy chrome and cannot suppress input. It
does not show that a user would notice one that tries.**

### Dependencies

No new dependencies.

### Deferred

- **Pastes larger than 256 KiB are refused whole**, not truncated. Truncating before
  classification would let truncation change the classification and would silently write
  a prefix of what was copied.
- **The audit family records paste refusals only.** A paste the user *approves* has no
  valid encoding in the frozen v1 schema, and an over-cap refusal has none either. Both
  are recorded as known limitations rather than fixed by amending a frozen schema.
- **No semantic detection of dangerous pasted commands.** RFC-009 excludes it by design;
  a classifier that catches some dangerous pastes invites the belief that it catches all.
- **Nothing here improves terminal performance.** `NFR-PERF-004` remains not met, the
  three-terminal limit and the ~374 KB/s output ceiling are unchanged. All are downstream
  of the same poll defect and owned by readiness-driven terminal I/O
  (`rfcs/future-work.md`).
- **No screen-reader support.** Checked again this release
  (`cargo tree -p tekstide | grep -i accesskit`, empty).
- Linux only. No macOS or Windows terminal runtime evidence exists.

## 0.5.0 - Terminal Renderer, and a Terminal You Can Open

Status: release candidate, prepared 2026-08-08, pending review and the owner's signed tag.

Tekstide `0.5.0` delivers the first half of milestone M9: RFC-017's terminal renderer,
plus the launch UX that makes it reachable. **RFC-017 is closed** (`rfcs/done/`), accepted
with `NFR-PERF-004` recorded as **not met** — see Deferred. RFC-018 (rendered paste
protection and adversarial spoofing evidence) is M9's second half and is not in this
release.

Press **`Ctrl+Alt+T`** in an open project and you get a real, PTY-backed terminal with
RFC-009's accepted-sequence policy enforced in front of the emulator.

### Implemented

- **Terminal surface (RFC-017).** A real `alacritty_terminal` grid behind RFC-009's
  security filter, rendered as a surface under RFC-015's contract. The filter's four
  properties — single ingress, no side channels, classification parity, and
  stream-position independence under adversarially chunked input — were re-proven
  against product code rather than inherited from the RFC-014 spike, each independently
  ablated.
- **Terminal launch UX.** `Ctrl+Alt+T` launches a terminal in the active project and
  switches to Terminal Mode. Typing `exit` really closes it: exit detection transitions
  the session, frees its visible slot, and makes the slot reusable.
- **Immersion mode, split, and session bar.** At most two visible panes, with the split
  decided from real measured font metrics — a split that cannot give each pane a full
  grid width is refused rather than rendered clipped. Session state is distinguishable
  without colour (`NFR-UX-002`).
- **Audit: `plain_terminal_observation` has a producer.** The first audit write the
  desktop application has ever performed. Opening a terminal records that a session
  started; exiting records that it terminated. A sentinel test proves no command text,
  output, or path reaches the durable store, checked against raw on-disk bytes.
- **Bounded scrollback** at 2,000 lines, ablation-verified under sustained output.
- The RFC-014 and RFC-007 spike crates were deleted, their properties having product-code
  equivalents with their own tests.

### Local data

**Opening a terminal now creates an audit database** at
`$XDG_STATE_HOME/tekstide/audit/audit.sqlite3`. This is a behaviour change from `0.4.1`,
which created no such file. It records that a terminal session started and stopped, and
nothing else — the schema has no field for command text, output, or paths. Delete the
`audit/` directory to reset it; there is no in-app purge command yet.

### Dependencies

No new dependencies. Two workspace crates were removed (`tekstide-gui-spike`,
`tekstide-pty-spike`), both `publish = false` and neither reachable from a shipped crate.

### Deferred

- **`NFR-PERF-004` (terminal input latency p95 ≤ 16 ms) is NOT met**, and is recorded as
  such rather than redefined until it passed. PTY bytes reach the grid only on a 50 ms
  poll tick, so poll-wait alone contributes a p95 near 47.5 ms. The fix is readiness-driven
  terminal I/O, scheduled as follow-up (`rfcs/future-work.md`).
- **At most three concurrent terminals per project.** This is a consequence of the same
  poll defect, not a product decision: every live pane is polled sequentially each tick at
  roughly 10 ms per pane, and five panes would saturate the tick. The limit is expected to
  rise once readiness-driven I/O lands.
- **Terminal output throughput is capped near 374 KB/s**, again by the same defect.
- **No trusted-UI separation or spoofing-resistance claim.** Nothing in this release
  demonstrates that terminal content cannot imitate Tekstide's own chrome. That is RFC-018.
- **No paste path exists** — the terminal accepts keystrokes only, so RFC-009's paste
  policy has nothing to protect yet. Rendered paste protection is RFC-018.
- **No terminate-from-UI and no pane selection.** Close a terminal by typing `exit`; input
  goes to the `Primary` pane.
- `TextStream::to_pty_bytes` is a defined subset, not a complete VT100/xterm encoder.
- **No screen-reader support.** `iced` offers no accessibility bridge; checked again this
  release (`cargo tree -p tekstide | grep -i accesskit`, empty).
- Linux only. No macOS or Windows terminal runtime evidence exists.

## 0.4.1 - Mode Switching, Focus Indicator, and RFC-015 Closure

Status: release candidate, prepared 2026-08-01, pending review and the owner's signed tag.

Tekstide `0.4.1` completes milestone M8 (GUI Foundation) with RFC-015 PR-015-E: the
`0.4.0`/`0.4.1` split deferred mode switching and its latency measurement here because
M8 had no second mode to switch into until this slice built it. **RFC-015 is closed**
(`rfcs/done/`) as of this release — both risks the RFC-014 substrate decision carried
unverified are now discharged: **R1** (input latency) by `0.4.0`'s C2/C5 measurements
and this release's C4, and **R6** (the focus-trap property not transferring from the
spike) by PR-015-C's real test. The RFC-014 substrate decision record has no open
items remaining.

### Implemented

- RFC-015 PR-015-E mode switching and Content-mode scaffolding:
  - Content ↔ Terminal route switching, dispatched through a real `Ctrl+Alt+M`
    keybinding (`NavigationAction::ToggleProjectMode`, previously unbound) to the
    pre-existing `AppCommand::ToggleActiveProjectMode`; no animation or interpolation
    in the switch path;
  - sidebar and main-area scaffolding (`FocusZone::Sidebar`, still `#[non_exhaustive]`
    for RFC-017/019/020) that required no changes to the input-routing structure
    PR-015-C established;
  - a visible, non-colour-only focus indicator (`NFR-UX-002`): border colour, border
    width, and a textual `"> "` marker all change together with `state.focus` —
    `0.4.0` shipped without one, defensibly, because the shell had only one focus
    zone; this release adds the second zone the indicator was always meant for.
- RFC-014 R1 discharge, completed: C4 (`NFR-PERF-002`, mode-switch latency, budget
  p95 ≤ 32ms), reusing `0.4.0`'s measurement harness rather than a new mechanism.
  Decomposed input-to-state-change (p95 29µs) and view-build cost (p95 39µs) sum to
  68µs, met by roughly 470× — **measured against the Content/Terminal-mode
  placeholders this release ships** (single-line catalog text each), not against the
  real editor (RFC-019) or terminal grid (RFC-017) those placeholders stand in for.
  RFC-017's handoff carries the obligation to re-check `NFR-PERF-002` once Terminal
  Mode renders a real grid.

### Dependencies

No new dependencies; this release is entirely `crates/tekstide-core`/`crates/tekstide`
source changes (one new default keybinding, no new crates).

### Deferred

- Terminal rendering, editor, file explorer, and diff/review surfaces — M9/M10, RFC-017/019/020.
- Rendered security dialogs and an adapter-spawn pathway that would make command
  approval reachable — M11. Command approval remains implemented but unreachable.
- Screen-reader support — out of scope for the life of the `iced` substrate decision
  (RFC-014 R2, owner-accepted), unchanged.
- `NFR-PERF-002`'s re-check against real Content/Terminal-mode content once RFC-017
  and RFC-019 render it — the placeholder boundary above, not a new finding.

## 0.4.0 - Application Shell and Project Board

Status: released on 2026-08-01.

Tekstide `0.4.0` covers milestone M8 (GUI Foundation): RFC-014's substrate decision,
RFC-016 PR-016-B/C/D's i18n and text-safety foundations, and RFC-015 PR-015-B/C/D/F/G's
application shell and Project Board. Owner-approved `0.4.0`/`0.4.1` split (2026-07-30):
mode switching and its latency measurement move to `0.4.1` because M8 has no second
mode to switch into that isn't the Project Board against an empty placeholder. It
remains a GUI shell over the headless core, not the full AI CLI workbench.

### Implemented

- RFC-014 desktop GUI substrate decision:
  - `iced` approved as the substrate, with Option A terminal filtering;
  - spike evidence and findings R1-R9 recorded; R1 (latency unverified) and R6
    (focus-trap property) discharged by RFC-015; R2 (no screen-reader support) and R9
    (survivorship bias in confirmed-only percentiles) owner-accepted and carried
    forward as standing findings, not defects.
- RFC-016 i18n, locale, and text-safety foundations (PR-016-B/C/D):
  - string catalog, locale selection with fallback, and the discipline that no
    user-facing string is hardcoded;
  - a canonical shared text-safety primitive (escaping and bidi isolation for
    untrusted text) adopted by both the shell and `approval::coordinator::display_argv`,
    retiring the duplicate-escaping debt recorded in `rfcs/delivery-plan.md`;
  - `CatalogArgs`' typed `number`/`untrusted`/`trusted_symbol` interpolation API,
    closing the untrusted-text interpolation bypass structurally rather than by
    convention, plus pluralization support.
- RFC-015 application shell and rendered surface model (PR-015-B/C/D/F/G):
  - a real `iced` desktop application replacing the headless text harness: window,
    chrome/content/modal layer composition via `stack`/`opaque`, with surface code
    structurally unable to open, populate, or dismiss a modal or render trusted chrome;
  - a keyboard-driven focus and input-routing model (`ShellInput`/`SurfaceInput`/
    `TextStream` as distinct, module-private types) with modal exclusivity and
    input-class privacy enforced by the compiler, not a runtime check;
  - a Project Board surface rendering live `ApplicationShell` state, with untrusted
    project names and paths escaped and honest `CountDisplay` fidelity
    (`Unavailable`/`NotImplemented` never render as `0`);
  - app-internal latency measurement (behind an opt-in flag, proven non-contaminating
    by idle-CPU comparison) discharging RFC-014 R1: typing latency
    (`NFR-PERF-003`, an upper-bound proxy from the sum of two measured streams' p95s)
    clears its budget by roughly two orders of magnitude, and warm start
    (`NFR-PERF-001`) clears its budget comfortably, at about a fifth of it.

### Dependencies

- Added to `tekstide` only (`tekstide-core` gains no GUI dependency, mechanically
  checked via `cargo tree -p tekstide-core --edges normal | grep -i iced`): `iced 0.14`
  (`tokio`, `advanced` features), `fluent-bundle 0.16`, `unic-langid 0.9`,
  `sys-locale 0.3`.

### Deferred

- Mode switching between Content and Terminal views, and the `NFR-PERF-002`
  mode-switch latency measurement that depends on it — `0.4.1`, RFC-015 PR-015-E.
- Visible focus indicators at the shell-chrome level. Low-stakes today because the
  shell has a single focus zone, but required before PR-015-E adds a second one —
  tracked for `0.4.1`.
- Terminal rendering, editor, file explorer, and diff/review surfaces — M9/M10.
- Rendered security dialogs (trust, safe-close, destructive, configuration change) and
  an adapter-spawn pathway that would make command approval reachable — M11. Command
  approval remains implemented but unreachable, as in `0.3.0`.
- Screen-reader support — out of scope for the life of the `iced` substrate decision
  (RFC-014 R2, owner-accepted).
- Cross-platform terminal, storage, and GUI evidence beyond
  `x86_64-unknown-linux-gnu`.
- **RFC-015 is not closed by this release.** Per RFC-000, it stays in
  `rfcs/proposed/` until PR-015-E and `NFR-PERF-002` land in `0.4.1`.

## 0.3.0 - AgentRun, Transcript, Review, and Durable Audit

Status: released on 2026-07-28.

Tekstide `0.3.0` consolidates three milestones — M5 AgentRun launch, M6 transcript
and review foundations, and M7 durable audit — covering RFC-010 through RFC-013.
These milestones were developed sequentially but never separately released, so they
ship together here. It remains a headless core, not the full AI CLI workbench.

### Implemented

- RFC-010 AgentRun launch model and AI CLI profiles:
  - AI CLI profiles as reviewed launch contracts covering executable provenance,
    argv shape, compatibility level, cwd, environment, prompt, and transcript policy;
  - Restricted Mode rejection of workspace-local executables, wrappers, shims,
    symlink targets resolving into the project root, and project-local `PATH` entries;
  - implicit CLI workspace-config/tool/prompt/plugin discovery blocked or rejected
    before process start;
  - launch validation for project, root, cwd, profile source, environment, and
    compatibility before any process is created;
  - AgentRun launch through project-owned TerminalSessions, with lifecycle derived
    from runtime observation;
  - honest Plain/Supervised/Managed labels; Managed requires adapter capability evidence;
  - active-document dirty, external-change, conflict, and save-error states block
    launch before process start, and safe-save conflict blocking is preserved while
    AgentRuns are active.
- RFC-011 transcript retention and local data policy:
  - capture modes Disabled, LocalBounded, and RequiredLocalBounded;
  - default retention of 32 MiB per transcript, 256 MiB per project, 1 GiB app-wide,
    and 30 days, with aggregate accounting;
  - transcript paths resolved under Tekstide state, outside project roots, with
    symlinked state-root rejection;
  - bounded append-only writer with truncation state;
  - per-run opt-out before process start;
  - purge by transcript, AgentRun, and ProjectSession scope with content-free tombstones.
- RFC-012 generated change review foundations:
  - ChangeSet review model with detection source/status, association confidence,
    bounded content-free summaries, and validated review-state transitions;
  - filesystem baseline capture and metadata-only changed-path detection;
  - project-relative path validation rejecting absolute escapes, `..` traversal, and
    escaping symlinks; symlink entries recorded without following targets;
  - conservative AgentRun association — strong linkage requires a same-run baseline,
    a closed target run, and no overlapping run; ambiguous cases stay unlinked.
- RFC-013 durable audit store and local data policy:
  - versioned durable record with stable string codes and an exhaustive
    family/field validation matrix;
  - local SQLite store with CHECK constraints mirroring that matrix independently of
    Rust validation, transactional append, exact-retry idempotency, operation
    correlation, and phase cardinality;
  - bounded descending cursor queries;
  - schema identity, read-only probe before write-capable open, and a sequential
    migration harness with a statement allowlist;
  - explicit comprehensive diagnostics separate from the bounded startup probe;
  - corruption classification, exact-artifact quarantine with content-free manifests,
    atomic fresh-store installation, and restart-safe resume;
  - project and global purge with ephemeral receipts and local-data accounting;
  - security-event integration for trust grant/revoke, managed AgentRun lifecycle,
    and blocked root/symlink access.

Only three of the twelve audit-schema event families have a wired runtime producer
(trust decisions, managed AgentRun lifecycle, blocked root/symlink access). See
Deferred below and the security threat model's T-035 for why this distinction matters.

### Dependencies

- Added `rusqlite 0.39.0` with `default-features = false` and only `bundled` enabled,
  resolving `libsqlite3-sys 0.37.0` and bundled SQLite `3.51.3`. This is the first
  third-party native dependency; it compiles the SQLite C amalgamation during build.
  Third-party notices are recorded in `NOTICE`.

### Deferred

- Desktop GUI runtime, rendered terminal surface, and rendered paste/approval/trust
  dialogs.
- App/UI commands for launching, selecting, and closing terminals and AgentRuns.
- Command approval.
- Audit producers for command approval, terminal paste, restricted-feature blocks,
  safe-close and destructive decisions, sensitive configuration changes, transcript
  purge, project added, and plain-terminal lifecycle. These families exist in the
  audit schema but have no runtime producer. Wiring `paste_blocked` headlessly was
  considered for this release (RFC-009 already classifies paste without a GUI) and
  deferred to keep 0.3.0 reconciliation-only; it remains available for a future
  release alongside `project_added`, `plain_terminal_observation`, and
  `transcript_purge`.
- Git-based change detection; the RFC-012 detector reports Git as unavailable.
- File watcher, overwrite-confirmation UI, and multi-document conflict workflow.
- Cross-platform terminal, storage, and native build evidence beyond
  `x86_64-unknown-linux-gnu`.
- Encryption at rest, tamper-evident audit, secure deletion, and automatic retention.

### Release Gate Status

Completed on a clean, committed tree:

- `git status --short` clean; `git diff --check`;
- `cargo fmt --all --check`;
- `cargo test --workspace --all-targets --all-features` — 375 `tekstide-core` tests, 0 elsewhere, 0 failures;
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`;
- `cargo build --release --locked`;
- `cargo package -p tekstide-core --locked` (113 files, 872.3 KiB / 143.7 KiB compressed);
- `cargo publish --dry-run -p tekstide-core --locked`;
- `cargo package -p tekstide --locked` (6 files, 12.8 KiB / 4.7 KiB compressed);
- `cargo publish --dry-run -p tekstide --locked`;
- `cargo publish --workspace --dry-run --locked` (authoritative same-workspace pairing check; verifies `tekstide` against the local `tekstide-core 0.3.0`, not a stale registry version);
- package smoke test: `cargo build`/`cargo test` from the unpacked `tekstide-core-0.3.0` package artifact (not the working tree) — 375 tests passed;
- release tarball built via `git archive` at `tekstide-v0.3.0.tar`: no intermediate parent directory, `NOTICE` and `LICENSE` both at archive root, no `.git`/`.git-exclude`/`target`/local-agent-config paths present (249 entries).

Build-cost baseline (first captured; RFC-013 retained none, so there is no prior figure to compare against):

- Clean `cargo build --release --locked` on `x86_64-unknown-linux-gnu`, Rust 1.97.1: 27.3s wall-clock.
- `target/release/tekstide` binary size: 790,560 bytes unstripped, 605,368 bytes stripped.

Release-candidate review (request 104) found that both published READMEs undercounted wired audit producers (three instead of four — audit-store recovery was omitted) and that the ROADMAP M7 table row still listed producers the reconciled scope section had already moved to M8. Both were corrected in commit `1f5100b` before publishing; the gates above were re-run against the corrected tree and the release tarball and crate packages were rebuilt from that commit. The threat model's matching corrections live in `.git-exclude/specs/`, which is gitignored and carries no commit.

Post-publish verification on 2026-07-28:

- `cargo publish -p tekstide-core --locked` — published `tekstide-core 0.3.0` to crates.io.
- `cargo publish -p tekstide --locked` — published `tekstide 0.3.0` to crates.io, correctly resolved against the just-published `tekstide-core 0.3.0`.
- Tag `0.3.0` (signed) created at commit `1f5100b`, matching the `0.1.0`/`0.2.0` tagging convention.
- `crates.io` API confirms both `tekstide-core 0.3.0` and `tekstide 0.3.0` exist and are not yanked.

## 0.2.0 - Terminal Runtime Foundation

Status: released on 2026-07-17.

Tekstide `0.2.0` is scoped as an M4 terminal/runtime/security foundation release through RFC-009. It is not the full AI CLI workbench.

### Implemented

- RFC-007 Linux PTY feasibility evidence:
  - PTY-backed shell startup;
  - output capture/rendering in the spike harness;
  - scripted input;
  - resize observation;
  - foreground-child termination, timeout, and SIGKILL fallback observations;
  - output flood and latency evidence.
- RFC-008 TerminalSession/process lifecycle foundation:
  - project-owned Linux plain shell launch;
  - runtime boundary that keeps PTY/process handles out of persisted domain metadata;
  - bounded PTY output reads and dropped-byte accounting;
  - project-addressed input and resize routing;
  - process-group termination with SIGTERM, timeout, SIGKILL fallback, and honest unresolved cleanup outcomes;
  - ProjectSession terminal collection integration and visible-slot policy;
  - project close assessment for real running terminals.
- RFC-009 terminal security boundary:
  - conservative ANSI/VT/OSC parser/security boundary;
  - exact accepted and inert sequence-family policy;
  - inert/diagnostic OSC clipboard, title, hyperlink, host-integration, private-mode, query, reply, unsupported control, and invalid-byte behavior;
  - bounded diagnostics without raw private terminal output, OSC payloads, pasted text, shell output, or environment-like values;
  - typed-input vs paste-input classification before PTY write;
  - multiline paste confirmation decision before PTY write;
  - C0, DEL, and C1 control-containing paste blocking;
  - model-level trusted UI / terminal spoofing boundary;
  - honest Plain/Supervised/Managed labels without command-approval overclaim.

### Deferred

- Desktop GUI runtime and final terminal renderer.
- App/UI commands for launching, selecting, and closing terminals.
- App/UI paste-event wiring, rendered paste confirmation, paste queue, and replay behavior.
- Rendered trusted dialogs and screenshot-backed visual spoofing evidence.
- App-wide close aggregation.
- Cross-platform terminal runtime and GUI security evidence beyond Linux.
- AI CLI profile execution and AgentRun launch.
- Transcript capture, retention, purge, and review workflow.
- Durable audit storage.
- Command approval.
- File watcher and overwrite-confirmation UI.

### Release Gate Status

Completed before release:

- clean working tree;
- `git diff --check`;
- `cargo fmt --all --check`;
- `cargo test --workspace --all-targets`;
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`;
- `cargo build --release --locked`;
- `cargo package -p tekstide-core --locked`;
- `cargo publish --dry-run -p tekstide-core --locked`;
- `cargo package -p tekstide --locked`;
- `cargo publish --dry-run -p tekstide --locked`;
- `cargo publish --workspace --dry-run --locked`;
- package smoke test from generated package artifacts;
- release-candidate review package accepted.

## 0.1.0 - Foundation Release

Status: released on 2026-07-06.

Tekstide `0.1.0` is scoped as a core/shell foundation release through RFC-006. It is not the full AI CLI workbench.

### Implemented

- Project Board and ProjectSession state.
- Core domain vocabulary for ProjectSession, TerminalSession, AgentRun, approvals, transcripts, change sets, and audit events.
- Navigation/mode policy for Project Board, Content Mode, and Terminal / Agent Immersion Mode.
- Restricted Mode policy/read-model baseline.
- Root-bound project file access policy.
- Bounded explorer read model.
- UTF-8 text document buffer.
- Safe save and external-change detection.
- Dirty-state propagation to project/runtime summaries.
- Shell-visible Content Mode evidence.

### Deferred

- Desktop GUI runtime.
- PTY terminal runtime.
- AI CLI profile execution and AgentRun launch.
- Transcript capture and review workflow.
- Generated diff/artifact review.
- Running-process safe close.
- Paste protection for real terminal input.
- File watcher.
- Overwrite-confirmation UI.
- Durable audit storage.
- Plugin marketplace, remote/container projects, debugger, cloud sync, and collaboration.

### Release Gate Status

Completed before release:

- clean working tree;
- `git diff --check`;
- `cargo fmt --check`;
- `cargo test --all-targets`;
- `cargo clippy --all-targets --all-features -- -D warnings`;
- `cargo build --release --locked`;
- `cargo package -p tekstide-core --locked`;
- `cargo package -p tekstide --locked`;
- package smoke test from generated package artifacts;
- release-candidate review package accepted;
- `tekstide-core` and `tekstide` published to crates.io.

### Future Work Themes

- Terminal/PTY runtime and process lifecycle.
- AgentRun launch and AI CLI profile execution.
- Transcript retention, review, and generated-change workflow.
- Durable audit storage and security evidence.
- Desktop GUI runtime and final Content Mode widgets.
- Release automation/checklist hardening.

See [`rfcs/future-work.md`](rfcs/future-work.md) for the durable deferred-theme index.
