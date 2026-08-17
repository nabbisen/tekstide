# Tekstide RFCs

This directory follows [RFC 000](./done/000-rfc-lifecycle-policy.md).

## Baseline Documents

| Role | Current file | Status |
| --- | --- | --- |
| Requirements | `tekstide-requirements-v0.md` | Baseline v0 |
| External design | `tekstide-external-design-v0.md` | Baseline v0 |
| UI/UX wireframes | `tekstide-uiux-wireframes-v0.md` | Baseline v0 |
| Security threat model | `tekstide-security-threat-model-v0.md` | Baseline v0 |
| Extensibility appendix | `tekstide-appendix-a-extensibility-plugin-v0.md` | Baseline v0 |
| Naming appendix | `tekstide-appendix-b-naming.md` | Baseline v0 |
| Roadmap | `tekstide-roadmap-milestones-v0.md` | Baseline v0 |

## Proposed

| RFC | Title | Status |
| --- | --- | --- |
| 020 | [Diff Review and AgentRun Report Surfaces](./proposed/020-diff-review-and-agentrun-report.md) | **Accepted — the spine of `0.8.0`, ready for implementation 2026-08-15.** Both prerequisites met: RFC-024 (content access) implemented and closed, RFC-011 Amendment 1 authorises the bounded transcript reader (which does not exist yet — PR-020-B builds it). [Handoff pack](./handoffs/020-diff-review-and-agentrun-report/README.md) complete; all four open questions answered there |
| 023 | [Configuration System](./proposed/023-configuration-system.md) | Proposed |

### Reserved numbers — check this before authoring

These are claimed by [`delivery-plan.md`](./delivery-plan.md)'s RFC queue but have no
document yet. **An RFC number is taken the moment the plan reserves it, not when a file
appears**, so this table exists to make a reservation visible to whoever authors next.

| RFC | Title | Milestone |
| --- | --- | --- |
| 031 | Audit Producer Completion (`safe_close_decision`, `restricted_mode_blocked`, `project_added`) — split from the original 022 reservation 2026-08-16 | M11 |
| 025 | Notifications | M12 |
| 026 | File Watcher and Multi-Document Model | M13 |
| 027 | Crash Recovery and Unsaved Buffer Persistence | M13 |
| 028 | Cross-Platform Support | M14 |
| 029 | Documentation, CI, and Release Automation | M14 |
| 030 | Git Integration | M12 |

Added 2026-08-12 after a real collision: RFC-024 was authored just-in-time as
*Diff Preview Policy* and took a number the delivery plan had already reserved for Git
Integration. Nothing connected the two files, so the clash went unnoticed until the plan
was read for release planning, by which point RFC-024 was implemented, closed and shipped
in `0.7.0` — and Git Integration had silently lost its number. Git Integration is now 030;
the intervening numbers could not absorb the shift because RFC-029 is referenced from
closed RFCs (013, 016), and closed documents are not edited to match a later state.

## Handoffs

| RFC | Handoff Pack |
| --- | --- |
| 001 | [Product Scope, Foundation Release, and Non-Goals](./handoffs/001-product-scope-mvp-and-non-goals/README.md) |
| 002 | [Core Domain Model: ProjectSession, TerminalSession, AgentRun, AuditEvent](./handoffs/002-core-domain-model-projectsession-terminalsession-agentrun-auditevent/README.md) |
| 003 | [Information Architecture and UI Mode Model](./handoffs/003-information-architecture-and-ui-mode-model/README.md) |
| 004 | [Security Baseline and Restricted Mode](./handoffs/004-security-baseline-and-restricted-mode/README.md) |
| 005 | [Application Shell and Project Board](./handoffs/005-application-shell-and-project-board/README.md) |
| 006 | [ProjectSession State and File Explorer / Editor Basics](./handoffs/006-projectsession-state-and-file-explorer-editor-basics/README.md) |
| 007 | [Runtime Substrate and PTY Feasibility Gate](./handoffs/007-runtime-substrate-pty-feasibility/README.md) |
| 008 | [TerminalSession and Process Lifecycle](./handoffs/008-terminalsession-process-lifecycle/README.md) |
| 009 | [Terminal Security Boundary](./handoffs/009-terminal-security-boundary/README.md) |
| 010 | [AgentRun Launch Model and AI CLI Profiles](./handoffs/010-agentrun-launch-model-and-ai-cli-profiles/README.md) |
| 011 | [Transcript Retention and Local Data Policy](./handoffs/011-transcript-retention-and-local-data-policy/README.md) |
| 012 | [Generated Change Review Foundations](./handoffs/012-generated-change-review-foundations/README.md) |
| 013 | [Durable Audit Store and Local Data Policy](./handoffs/013-durable-audit-store-and-local-data-policy/README.md) |
| 014 | [Desktop GUI Substrate and Terminal Rendering Strategy](./handoffs/014-desktop-gui-substrate-and-terminal-rendering/README.md) |
| 015 | [Application Shell and Rendered Surface Model](./handoffs/015-application-shell-and-rendered-surface-model/README.md) |
| 016 | [Internationalization and Localization](./handoffs/016-internationalization-and-localization/README.md) |
| 017 | [Terminal Renderer and Immersion Mode](./handoffs/017-terminal-renderer-and-immersion-mode/README.md) |
| 011 A2 | [Re-homing transcript capture](./handoffs/011-amendment-2-transcript-capture-rehoming/README.md) — RFC-011 Amendment 2 |
| 022 | [Adapter Spawn and the Command Approval Surface](./handoffs/022-adapter-spawn-and-command-approval-surface/README.md) |
| 017 A1 | [Readiness-driven terminal I/O](./handoffs/017-amendment-1-readiness-driven-terminal-io/README.md) — RFC-017 Amendment 1 |
| 018 | [Rendered Paste Protection and Trusted-UI Evidence](./handoffs/018-paste-protection-and-trusted-ui-evidence/README.md) |
| 019 | [Editor and Explorer Surfaces](./handoffs/019-editor-and-explorer-surfaces/README.md) |
| 020 | [Diff Review and AgentRun Report Surfaces](./handoffs/020-diff-review-and-agentrun-report/README.md) |
| 024 | [Diff Preview Policy](./handoffs/024-diff-preview-policy/README.md) |
| — | [Minimal user documentation](./handoffs/minimal-user-documentation.md) — no RFC; pulled forward from RFC-029 to M9 |
| 021 | [Command Approval Model and Adapter Capability](./handoffs/021-command-approval-model-and-adapter-capability/README.md) |
| 023 | [Configuration System](./handoffs/023-configuration-system/README.md) |

## Implemented

| RFC | Title | Status |
| --- | --- | --- |
| 000 | [RFC lifecycle policy](./done/000-rfc-lifecycle-policy.md) | Implemented |
| 001 | [Product Scope, Foundation Release, and Non-Goals](./done/001-product-scope-mvp-and-non-goals.md) | Implemented for 0.1.0 foundation |
| 002 | [Core Domain Model: ProjectSession, TerminalSession, AgentRun, AuditEvent](./done/002-core-domain-model-projectsession-terminalsession-agentrun-auditevent.md) | Implemented |
| 003 | [Information Architecture and UI Mode Model](./done/003-information-architecture-and-ui-mode-model.md) | Implemented |
| 004 | [Security Baseline and Restricted Mode](./done/004-security-baseline-and-restricted-mode.md) | Implemented |
| 005 | [Application Shell and Project Board](./done/005-application-shell-and-project-board.md) | Implemented |
| 006 | [ProjectSession State and File Explorer / Editor Basics](./done/006-projectsession-state-and-file-explorer-editor-basics.md) | Implemented with documented limitations · **Amendment 1 implemented 2026-08-11** (additive `ProjectContentWorkspace::set_active_cursor` cursor-forwarding accessor, authorised by RFC-019 PR-019-D response 182; enables cursor-aware editing without reopening `active_document()`'s read-only invariant) |
| 007 | [Runtime Substrate and PTY Feasibility Gate](./done/007-runtime-substrate-pty-feasibility.md) | Implemented feasibility gate |
| 008 | [TerminalSession and Process Lifecycle](./done/008-terminalsession-process-lifecycle.md) | Implemented with documented limitations |
| 009 | [Terminal Security Boundary](./done/009-terminal-security-boundary.md) | Implemented with documented limitations |
| 010 | [AgentRun Launch Model and AI CLI Profiles](./done/010-agentrun-launch-model-and-ai-cli-profiles.md) | Implemented with documented limitations |
| 011 | [Transcript Retention and Local Data Policy](./done/011-transcript-retention-and-local-data-policy.md) | Implemented with documented limitations. **Amendment 1** (2026-08-12) authorises a bounded, read-only transcript reader — RFC-020's last prerequisite; `transcript/` had no reader at all. Additive: no retention change, no migration. Its D2 records that a tail window **drops** bytes and so falls outside RFC-017's **P4 (stream-position independence)**, which covers chunking where every byte arrives — the reader must resynchronize **Amendment 2** (closed 2026-08-16) re-homes transcript capture onto that reader: the previous write path was a side effect of `read_available_bounded_for`, which RFC-017 Amendment 1 removed from the ingress, so capture had silently stopped existing. Writer now lives in the reader thread, writes **before** the bytes enter the channel (the transcript is a superset of what was displayed), and has a per-mode mid-stream failure policy — `RequiredLocalBounded` stops reading so the child stalls rather than making unrecorded progress. **Breaking**: `TranscriptWriterConfig` gained a public `mode` field |
| 012 | [Generated Change Review Foundations](./done/012-generated-change-review-foundations.md) | Implemented with documented limitations on main at 34a1c55 |
| 013 | [Durable Audit Store and Local Data Policy](./done/013-durable-audit-store-and-local-data-policy.md) | Implemented with documented limitations · **Amendment 1 complete 2026-07-30** (additive v1 → v2 schema migration for RFC-021's `command_cwd_mismatch`; migration, convergence, and interrupted-migration properties all proven) |
| 014 | [Desktop GUI Substrate and Terminal Rendering Strategy](./done/014-desktop-gui-substrate-and-terminal-rendering.md) | Implemented with documented limitations — `iced` + Option A; R1/R6 discharged, R4-R7 carried to RFC-017 |
| 015 | [Application Shell and Rendered Surface Model](./done/015-application-shell-and-rendered-surface-model.md) | Implemented with documented limitations — `0.4.0` + `0.4.1`; RFC-014 R1 and R6 discharged |
| 016 | [Internationalization and Localization](./done/016-internationalization-and-localization.md) | Implemented with documented limitations — catalog, fallback, text safety, pluralization, enforcement; translation content and runtime locale switching out of scope |
| 017 | [Terminal Renderer and Immersion Mode](./done/017-terminal-renderer-and-immersion-mode.md) | Implemented with documented limitations — filtered terminal surface, input with modal exclusivity, first audit producer; trusted-UI separation deferred to RFC-018. **Amendment 1 (readiness-driven terminal I/O) closed 2026-08-15** ([handoff](./handoffs/017-amendment-1-readiness-driven-terminal-io/README.md)): the 50 ms tick, the 10 ms sleep and the terminal-pane truncation path are gone; throughput ~374 KB/s → ~17.4-18 MB/s; `terminal_session_limit` re-derived 3 → 6; P1/P2 re-enumerated and re-ablated against the new ingress. **`NFR-PERF-004` still not met** — structural cause removed, criterion unverified end-to-end, and *not* claimed met |
| 018 | [Rendered Paste Protection and Trusted-UI Evidence](./done/018-paste-protection-and-trusted-ui-evidence.md) | Implemented with documented limitations — paste ingress, real confirmation dialog, `paste_blocked` producer; dialog **distinguishable by a test the user can perform** (keystroke suppression, positive-control proven); occupying chrome is content-dependent and not relied on as a tell; pastes over 256 KiB refused whole; frozen-schema audit gaps recorded, not amended |
| 019 | [Editor and Explorer Surfaces](./done/019-editor-and-explorer-surfaces.md) | Implemented with documented limitations — trusted-chrome explorer tree and a cursor-aware editor over `TextDocument`, real save and conflict handling; **RFC-006 Amendment 1** added the cursor-forwarding accessor core was missing; conflict dialog distinguishes a genuine conflict from a clean externally-changed document (found live, fixed at closeout); no undo beyond RFC-006's own model, no syntax highlighting |
| 021 | [Command Approval Model and Adapter Capability](./done/021-command-approval-model-and-adapter-capability.md) | Implemented **headless** with documented limitations — not reachable by any user; cooperative, not enforced. Fully closed 2026-07-30 |
| 022 | [Adapter Spawn and the Command Approval Surface](./done/022-adapter-spawn-and-command-approval-surface.md) | Implemented with documented limitations — **not reachable by any real user**; `Managed`, and therefore command approval, can only ever be exercised by the reference adapter, a test artifact, since no shipping AI CLI speaks RFC-021's protocol (open question 1, answered by the architect 2026-08-16). The pathway itself — spawn, token delivery, the arrival model's dialog and `ApprovalHistory` surface — is built and proven end to end against production code; cooperative, not enforced, the same limit RFC-021 shipped with. Unblocks (does not complete) RFC-020's two surfaces. Fully closed 2026-08-17 |
| 024 | [Diff Preview Policy](./done/024-diff-preview-policy.md) | Implemented with documented limitations — gated, bounded content access per change kind (Added: whole content; Modified: current content, explicitly not a diff; Deleted: fact of deletion); **RFC-012 Amendment 1** (`ChangeLifecycle`) landed as a prerequisite, a **breaking change — ships in `0.7.0`, not a patch**; no two-sided diff for a modified file (before-bytes were never captured, RFC-012 §Design Principles); `DiffContent`'s non-retention is narrower than a first reading suggests (blocks two specific storage paths, not general retention — carried to RFC-020); no rendering, no diff algorithm, no action on a change. Fully closed 2026-08-11 |

## Archive

No archived RFCs yet.
