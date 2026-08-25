# Tekstide RFCs

This directory follows [RFC-000](./done/000-rfc-lifecycle-policy.md), in the **5-folder
variant** adopted by [RFC-037](./done/037-five-folder-rfc-lifecycle.md) on 2026-08-19.

RFC-000 is written for the 4-folder variant and describes both; read RFC-037 for which one this
project uses and why. The folder is the source of truth for an RFC's state — if a file's Status
field and its folder ever disagree, the folder wins.

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

RFCs open for review.

| RFC | Title | Status |
| --- | --- | --- |

*(empty — and that is the correct state. An empty `proposed/` means nothing is awaiting review,
not that a folder is missing. See [RFC-037](./done/037-five-folder-rfc-lifecycle.md).)*

## Accepted

Review complete; an implementer may start. **This is the first place to look for work.** An RFC
stays here while it is being implemented — `done/` means shipped, so a partially-implemented RFC
belongs here, not there.

| RFC | Title | Status |
| --- | --- | --- |
| 034 | [Change Review Actions and Review State](./accepted/034-change-review-actions-and-review-state.md) | **Accepted 2026-08-18; UNBLOCKED 2026-08-25** by RFC-020's change review surface. Note it renders **metadata only** — an action defined here is taken on what the surface shows, not on inspected diff content, which is still unrendered. **Accepted 2026-08-18.** Gives `transition_change_set_review_state` a route, and decides the question RFC-020's Q3 deferred: whether a review decision is a record or an operation. Blocked on RFC-020 |
| 035 | [Change Detection Coverage and Disclosure](./accepted/035-change-detection-coverage-and-disclosure.md) | **Accepted 2026-08-18.** The `.git/hooks/` supervision hole and `max_changed_paths` discarding a computed list; the exit-only trigger and non-persistent baseline recorded and deferred |
| 036 | [Dormant Capability Closure](./accepted/036-dormant-capability-closure.md) | **Accepted 2026-08-18.** Wire / delete / document, per orphan from the reachability audit. Separates `recover` and `purge_all_records` as worse than dormant — recovery and deletion paths never exercised from the application |


### Reserved numbers — check this before authoring

These are claimed by [`delivery-plan.md`](./delivery-plan.md)'s RFC queue but have no
document yet. **An RFC number is taken the moment the plan reserves it, not when a file
appears**, so this table exists to make a reservation visible to whoever authors next.

| RFC | Title | Milestone |
| --- | --- | --- |
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
| 020 | [Diff Review and AgentRun Report Surfaces](./handoffs/020-diff-review-and-agentrun-report/README.md) — **both surfaces implemented.** AgentRun report surface ([slice handoff](./handoffs/020-diff-review-and-agentrun-report/pr-020-b-report-surface.md)): reader landed 2026-08-15, surface (`Ctrl+Alt+R`, escaped transcript content, reader-window-vs-writer-truncation rendered distinctly) landed 2026-08-18. Change review surface ([slice handoff](./handoffs/020-diff-review-and-agentrun-report/change-review-surface.md)): landed 2026-08-25, `Ctrl+Alt+D`, metadata only — diff content rendering remains blocked on its own `DetectedChanges` projection |
| 040 | [Affordance Completion](./handoffs/040-affordance-completion/README.md) — **M12, first of three**; finishes the sentence RFC-039 started |
| 039 | [Interaction Model and Visible Affordances](./handoffs/039-interaction-model-and-visible-affordances/README.md) — **M12, after RFC-038**; the workflows the product never had |
| 038 | [First-Run and Project Entry](./handoffs/038-first-run-and-project-entry/README.md) — **M12, first**; the product's missing door |
| 024 | [Diff Preview Policy](./handoffs/024-diff-preview-policy/README.md) |
| — | [Minimal user documentation](./handoffs/minimal-user-documentation.md) — no RFC; pulled forward from RFC-029 to M9 |
| — | [The reachability audit](./handoffs/reachability-audit.md) — no RFC; run 2026-08-17, findings carried into RFC-023, RFC-031 and RFC-036 |
| — | [The 0.12.1 first-run correction](./handoffs/first-run-correction-0.12.1.md) — no RFC; the release's only evidence record, written after the fact because it shipped without a pack. Carries two corrections: `ca456c7` claims five valid ablations and four were, and `action_catalog_key`'s contract for non-live actions is untested |
| — | [The leaked-child process leak](./handoffs/test-process-leak.md) — no RFC. Two distinct causes: the approval call sites, fixed 2026-08-20; and **`runtime/terminal/launch.rs`, the production spawn path, SCHEDULED 2026-08-25** after a second incident left 3,899 orphaned shells and the PTY pool exhausted. Four symptom tests recorded; the socket flake is separate and also unfixed |
| — | [Terminal resize](./handoffs/terminal-resize.md) — no RFC; priority 1 from the audit, scheduled 2026-08-17 |
| — | [Change detection wiring](./handoffs/change-detection-wiring.md) — no RFC; the last structural blocker on RFC-020. **Implemented 2026-08-18 (Slices A-D)**: one shared ignore-directory model; a real repository's measured baseline/detect cost (~5-6ms at 1,506 entries after exclusion); `add_detected_generated_change_set` has its first real production caller, wired at agent-run launch (baseline, captured before the process spawns) and exit (detection, via the previously-dormant `apply_agent_terminal_outcome`); a truncated scan's status is recorded distinctly from a genuinely clean one, never collapsing into the same "no `ChangeSet`" shape. At the time this landed, made diff review **buildable, not reachable** — RFC-020's own surfaces were still unbuilt; both have since shipped (see RFC-020's own row above). Known, disclosed limitations: no mid-run change visibility for a long-lived interactive session (exit is the only trigger), and a captured baseline does not survive the application exiting mid-run |
| — | [Theme contrast verification](./handoffs/theme-contrast-verification.md) — no RFC; a measured WCAG 1.4.11 failure in `border_default` plus the gate that should have caught it. Implemented 2026-08-18: `theme/contrast.rs`'s anchor-validated WCAG math, a threshold test observed failing at the measured ratios before the fix, `border_default` raised 0.35 → 0.45 (3.85:1 / 3.48:1) |
| 011 | [Transcript capture evidence](./handoffs/transcript-capture-evidence.md) — no RFC; the test whose absence let a false privacy claim ship in two releases. **Implemented 2026-08-18**: a real launch on the real `Supervised` path (the same compatibility level `claude_code_linux_default` uses) writes a real transcript at the documented path shape with real, known content; a plain terminal writes none. New injectable state-root seam (`attempt_agent_run_launch_with_profile_and_state_root`), since none existed before this slice |
| — | [Derived contrast pairs](./handoffs/derived-contrast-pairs.md) — no RFC; makes a new `Theme` role impossible to leave unmeasured, after snora shipped the same fix we recommended to them. **Implemented 2026-08-18**: pair list derived from an exhaustive `Theme` destructure (`E0027` ablated); fixed-pair count unchanged at 8 (the old list was complete, not narrow); the modal-over-scrim backdrop — a real WCAG failure at 2.40:1, found by sweeping rather than sampling — is fixed by raising the scrim alpha 0.55 → 0.75, verified translucent against the real rendered window |
| 022 | [Approval-history binding](./handoffs/approval-history-binding.md) — the remedy for RFC-022's corrected record; the last `Configurable`/`None` action whose surface actually exists. **Implemented 2026-08-18**: `OpenApprovalHistory` has a real `Candidate` binding (`Ctrl+Alt+H`), mechanically checked to collide with nothing; opens the surface, not command approval itself |
| 031 | [Audit Producer Completion](./handoffs/031-audit-producer-completion/README.md) — the last M11 *audit* item; two families with live triggers and no producer |
| 033 | [Transcript Lifecycle Controls](./handoffs/033-transcript-lifecycle-controls/README.md) — closes the limitation `0.11.1` published on a privacy claim |
| 032 | [Workspace Trust Granting](./handoffs/032-workspace-trust-granting/README.md) |
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
| 020 | [Diff Review and AgentRun Report Surfaces](./done/020-diff-review-and-agentrun-report.md) | **Implemented and closed 2026-08-25.** Both surfaces ship. The AgentRun report landed in `0.12.0`; the change review surface landed now — **metadata only**, with the two disclosures the RFC made non-optional rendered on the surface itself: *not all changes*, and *not a review, approval, or safety claim*. Scan-level truncation (`Partial{limit}`) and display-level truncation stay two facts, never collapsed. **Diff content is still not rendered** and is recorded as blocked. A `ChangeSet` from a real agent run has been proven end to end in test and seen live only via an env-gated seed — never observed live from a real run, stated as a limitation. Unblocks RFC-034 |
| 022 | [Adapter Spawn and the Command Approval Surface](./done/022-adapter-spawn-and-command-approval-surface.md) | Implemented with documented limitations — **not reachable by any real user**; `Managed`, and therefore command approval, can only ever be exercised by the reference adapter, a test artifact, since no shipping AI CLI speaks RFC-021's protocol (open question 1, answered by the architect 2026-08-16). The pathway itself — spawn, token delivery, the arrival model's dialog and `ApprovalHistory` surface — is built and proven end to end against production code; cooperative, not enforced, the same limit RFC-021 shipped with. Unblocks (does not complete) RFC-020's two surfaces. Fully closed 2026-08-17. **Corrected 2026-08-17**: `OpenApprovalHistory` is `Configurable`/`None`, so the `ApprovalHistory` surface is unopenable by any user independently of the protocol limit — missed at closeout, remedy tracked in `future-work.md`. **Remedy landed 2026-08-18**: `OpenApprovalHistory` now has a real `Candidate` binding (`Ctrl+Alt+H`); the surface opens, the protocol limit above is unchanged |
| 024 | [Diff Preview Policy](./done/024-diff-preview-policy.md) | Implemented with documented limitations — gated, bounded content access per change kind (Added: whole content; Modified: current content, explicitly not a diff; Deleted: fact of deletion); **RFC-012 Amendment 1** (`ChangeLifecycle`) landed as a prerequisite, a **breaking change — ships in `0.7.0`, not a patch**; no two-sided diff for a modified file (before-bytes were never captured, RFC-012 §Design Principles); `DiffContent`'s non-retention is narrower than a first reading suggests (blocks two specific storage paths, not general retention — carried to RFC-020); no rendering, no diff algorithm, no action on a change. Fully closed 2026-08-11 |
| 031 | [Audit Producer Completion](./done/031-audit-producer-completion.md) | **Implemented and closed 2026-08-19.** `restricted_mode_blocked` and `project_added` have real producers, each proven from a real user path. The last M11 *audit* item — RFC-033 (M11) and RFC-020 (M10→M11) remained open. Does **not** make the audit store viewable — nothing renders it — and cannot say which of RFC-004's nine restricted features blocked a launch. `safe_close_decision` stays unwired, blocked on a dialog that does not exist |
| 037 | [Adopt the 5-Folder RFC Lifecycle](./done/037-five-folder-rfc-lifecycle.md) | **Implemented and closed 2026-08-19.** Added `rfcs/accepted/`; the five RFCs that were accepted but unfinished moved into it, and `proposed/` is now correctly empty. Accepted and migrated in one commit, per RFC-000 §Self-application. Also repaired three cross-references that had pointed at `proposed/` for RFCs long since in `done/` |
| 033 | [Transcript Lifecycle Controls](./done/033-transcript-lifecycle-controls.md) | **Implemented and closed 2026-08-19.** Per-project capture opt-out, purge, and retained-bytes visibility, all from the Trust Settings surface and all proven from real key presses. Removes the sentence `0.11.1` had to publish on a privacy claim. Does **not** remove every trace — a tombstone and an audit record remain, stated rather than discovered |
| 040 | [Affordance Completion](./done/040-affordance-completion.md) | **Implemented and closed 2026-08-25.** Visible controls moved from **3 to 11 of 13** live actions, the two remaining being permanent allow-list entries with stated reasons — a decision, not a debt. Every modal in the crate can now be completed and abandoned with a mouse; no flow that begins with a click needs a keyboard to finish it. The audit became a test **first**, so the rest was measured rather than asserted — and that test's own required ablation caught it passing vacuously, scanning the file where its search strings are defined. Three defects in this arc were found by clicking the product rather than by the suite |
| 039 | [Interaction Model and Visible Affordances](./done/039-interaction-model-and-visible-affordances.md) | **Implemented and closed 2026-08-25.** A project tab strip in the top bar: see what is open, switch by click or keyboard, a permanent route home, and `×` to close — with a confirmation naming the project by canonical path when work is live, `close_project`'s and `request_terminate`'s first production callers, and `safe_close_decision` wired at last, taking unwired audit families from two to one. **Its own affordance audit is the finding that matters**: three of thirteen live actions have a visible control and every modal is keyboard-only for its own decision — carried to RFC-040 rather than closed over. Fixed two latent defects belonging to nobody: `close_project` could never return `SafeToClose` for any real project, and `set_file_state`'s one-way downgrade would have poisoned any project whose file provider hiccuped |
| 038 | [First-Run and Project Entry](./done/038-first-run-and-project-entry.md) | **Implemented and closed 2026-08-24.** The product has a door. A person who has read nothing can open a project from the window — typing a path, browsing to a folder, or reopening a remembered one with a single key — and can find out what else exists from a Help modal reachable anywhere, including Terminal Immersion. Removes `ProjectBoardEmptyState`'s two dead fields, a **breaking change** to `tekstide-core`. `0.13.0` is prepared and deliberately unshipped. Found and fixed mid-RFC: a **trust-restoration gap** — every runtime path that opened a remembered project restored cached `Trusted` from the user-writable recent-projects file without confirming it against the audit store, because `verify_restored_trust` had exactly one caller and it ran only at boot. Four of this pack's own instructions rested on unchecked claims by the architect and were caught by the implementer checking before executing |
| 032 | [Workspace Trust Granting](./done/032-workspace-trust-granting.md) | Implemented with documented limitations — trust is grantable and revocable through a real, reachable route (`Ctrl+Alt+U` → `TrustSettings` → the confirmation dialog), proven end to end from a real key event: a profile requiring workspace discovery, refused with `WorkspaceDiscoveryBlocked` in a fresh `Restricted` project, launches for real once trust is granted. Persists across sessions, bound to the canonical path (proven against a real, redirected symlink); the audit store, not the user-writable recent-projects cache, is authoritative for restored trust. Does not claim a trusted project is safe, and does not make any other gated surface reachable — RFC-022's `ApprovalHistory` has its own, independently found and separately corrected reachability gap. Fully closed 2026-08-17 |
| 023 | [Configuration System](./done/023-configuration-system.md) | **Implemented and closed 2026-08-22.** File format, precedence, atomic validation, diagnostics, the security-sensitive reload rule over eight fields, and configuration-defined AI CLI profiles routed through *unmodified* RFC-010 validation. **Headless — not reachable by any user.** Nothing constructs a `ConfigStore`, so the shipped application never reads a configuration file: `to_ai_cli_profile`, `set_resource_limits` and both `record_sensitive_config_policy_*` methods have zero production call sites at closeout. That was the scope accepted in its own §Scoping addendum, not a shortfall found late. Three things it owned and did **not** discharge were re-homed in the closing commit rather than left pointing at a closed RFC: `set_resource_limits` and the `sensitive_config_changed` producer (both to RFC-036, conditioned), and OQ3's first-use confirmation gate for configuration-defined profiles (to the slice that first lets one reach `attempt_agent_run_launch`). The WCAG contrast-gate question is **still open** — RFC-023 v1 wires no configuration value into theme selection, so the gate was never put at risk; two transferable precedents from the snora review are recorded in the pack for whoever takes it |

## Archive

No archived RFCs yet.
