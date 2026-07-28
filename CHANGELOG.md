# Changelog

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
