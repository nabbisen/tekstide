# Tekstide Delivery Plan

Date: 2026-07-28
Covers: M8 through M14 (`0.4.x` → `1.0.0`)
Companion to: [`../ROADMAP.md`](../ROADMAP.md)

This is the ordered RFC queue for the remaining work, with the requirements gap analysis that justifies it. It answers one question for a developer: **what do I pick up next, and what do I read first?**

## How to pick up work

1. Find the next RFC in [§ RFC Queue](#rfc-queue) whose status is *Accepted* and whose dependencies are met.
2. Read the RFC in `rfcs/proposed/NNN-*.md` (or `rfcs/done/` if already implemented).
3. Read its handoff pack at `rfcs/handoffs/NNN-*/README.md` — that file is the entry point and links everything else in reading order.
4. Implement one slice from the handoff's `task-breakdown-pr-plan.md`.
5. Record evidence in the handoff's `qa-evidence.md` and tick `acceptance-qa-checklist.md`.
6. Open a review request under `.git-exclude/review-request/`.
7. Do not start the next slice until the current one is accepted, unless the plan says slices are parallel.

**RFCs are authored just-in-time, not all up front.** An RFC written now for M13 would be stale by the time it is implemented, and RFC-014's substrate outcome constrains every GUI RFC after it. The queue below fixes scope, order, and dependencies; the documents get written as their blockers clear.

## Requirements Gap Analysis

Performed 2026-07-28 against `tekstide-requirements-v0.md` and the implemented surface in `crates/tekstide-core`. This is what the milestone plan is built from.

### Implemented

| Area | Requirements | Status |
| --- | --- | --- |
| Project lifecycle | `REQ-PROJ-001`..`009` | Model complete; no rendered surface |
| Text document | `REQ-EDIT-001`, `004`..`007` | Model complete, single active document |
| File explorer | `REQ-FILE-001`, `005` | Bounded read model; no tree UI |
| Terminal sessions | `REQ-TERM-001`..`010` | Linux runtime complete; no renderer |
| AgentRun | `REQ-AGENT-001`..`011`, `014`..`018` | Launch, lifecycle, transcript, review complete |
| AI CLI profiles | `REQ-CLI-001`..`005` | Model complete; code-defined only, not user config |
| Diff/review | `REQ-REVIEW-001`..`005` | Models complete; no rendered surface |
| Workspace trust | `REQ-SEC-001`..`006` | Complete at model level |
| Command safety | `REQ-SEC-010`, `011`, `014`, `015` | Paste classification and audit complete |
| Environment/secrets | `REQ-SEC-020`..`026` | Complete |
| Terminal security | `REQ-SEC-030`..`033` | Boundary complete; rendering deferred |
| Filesystem safety | `REQ-SEC-040`..`043` | Complete |
| Session recovery | `REQ-RECOVER-001`, `003`, `004` | Recent projects and run records restore |

### Not implemented

| Area | Requirements | Milestone |
| --- | --- | --- |
| **Desktop GUI** — every rendered surface | External design §3, UI/UX baseline | M8-M11 |
| **Command approval** — only domain vocabulary exists | `REQ-AGENT-012`, `013`; `REQ-SEC-012`, `013` | M11 |
| **Configuration system** — no module at all | `REQ-CONFIG-001`..`007` | M12 |
| **i18n** — no module; mandated by project rules | Project rules, UI/UX §18 | M8 |
| **Git integration** — no module; RFC-012 detector reports Git unavailable | `REQ-GIT-001`..`007` | M12 |
| **Notifications** — no domain type | `REQ-NOTIFY-001`..`005` | M12 |
| **File watcher** | `REQ-FILE-003`, `004` | M13 |
| **Multi-document** — one active document only | External design §3.4 | M13 |
| **Syntax highlighting** | `REQ-EDIT-003` | M10 (optional) |
| **Crash / unsaved buffer recovery** | `REQ-RECOVER-005` | M13 |
| **Audit producers** — 8 of 12 families unwired | `REQ-SEC-014` | M9, M11, M12 |
| **LSP** | `REQ-LSP-001`..`005` | Deferred beyond 1.0 by design |
| **Cross-platform** — Linux only | `NFR-PORT-001`..`003` | M14 |
| **Documentation** — `docs/` absent | Project rules | M14 |
| **CI** | — | M14 |

### Audit producer coverage

Four of twelve v1 families have runtime producers. The remaining eight are the clearest measure of how much product surface is still missing:

| Family | Producer | Milestone |
| --- | --- | --- |
| `trust_change` | wired | — |
| `managed_process_lifecycle` | wired | — |
| `root_access_blocked` | wired | — |
| `audit_store_recovery` | wired | — |
| `paste_blocked` | none | M9 |
| `plain_terminal_observation` | none | M9 |
| `command_approval` | none | M11 |
| `safe_close_decision` | none | M11 |
| `restricted_mode_blocked` | none | M11 |
| `project_added` | none | M11 |
| `sensitive_config_changed` | none | M12 (needs config system) |
| `transcript_purge` | none | M12 |

## RFC Queue

Status values: **In progress** · **Next** · **Queued** · **Blocked**

| RFC | Title | Milestone | Depends on | Headless | Status |
| --- | --- | --- | --- | --- | --- |
| 014 | Desktop GUI Substrate and Terminal Rendering Strategy | M8 | — | no | **Decision approved 2026-07-29 — `iced` + Option A** |
| 015 | Application Shell and Rendered Surface Model | M8 | 014 | no | **Unblocked — next to author** |
| 016 | Internationalization and Localization | M8 | 014 | partly | **Unblocked** |
| 017 | Terminal Renderer and Immersion Mode | M9 | 014, 015 | no | Blocked |
| 018 | Rendered Paste Protection and Trusted UI | M9 | 017 | no | Blocked |
| 019 | Editor and Explorer Surfaces | M10 | 015 | no | Blocked |
| 020 | Diff Review and AgentRun Report Surfaces | M10 | 015 | no | Blocked |
| 021 | Command Approval Model and Adapter Capability | M11 | — | **yes** | **Authored — ready for implementation** |
| 022 | Security Dialogs and Audit Producer Completion | M11 | 015, 021 | no | Blocked |
| 023 | Configuration System | M12 | — | **yes** | **Authored — ready for implementation** |
| 024 | Git Integration | M12 | — | **yes** | Queued (parallel-ready) |
| 025 | Notifications | M12 | 023 | partly | Queued |
| 026 | File Watcher and Multi-Document Model | M13 | 019 | partly | Blocked |
| 027 | Crash Recovery and Unsaved Buffer Persistence | M13 | — | **yes** | Queued (parallel-ready) |
| 028 | Cross-Platform Support | M14 | most | partly | Queued |
| 029 | Documentation, CI, and Release Automation | M14 | — | **yes** | Queued (parallel-ready) |

### Scope notes per RFC

- **015 Application Shell** — window, Content/Terminal mode layout, mode switching, focus model and keyboard routing, theme/typography primitives, Project Board surface. Defines the rendered-surface contract every later surface RFC builds on.
- **016 i18n** — string catalog format, locale loading and fallback, the rule that no user-facing string is hardcoded, and pluralization/RTL policy. Must land in M8; retrofitting extraction across a built UI is far more expensive than designing it in.
- **017 Terminal Renderer** — cell grid, the RFC-009 interposition strategy chosen by RFC-014, scrollback, selection, split policy from real font metrics, session bar.
- **018 Paste Protection and Trusted UI** — real paste-event wiring, rendered confirmation dialog, trusted-UI separation with screenshot evidence. Closes the RFC-009 deferral.
- **019 Editor and Explorer** — editing on RFC-006 models, line numbers, cursor, dirty state, explorer tree, optional syntax highlighting.
- **020 Diff Review and AgentRun Report** — rendered surfaces over RFC-012 change models and RFC-011 transcripts. Both render untrusted content and must say so.
- **021 Command Approval** — adapter capability contract, approval policy, risk classification, approve/deny/edit semantics, and the audit correlation. **Headless and unblocked.** The dialog is RFC-022's job.
- **022 Security Dialogs** — trust, safe-close, destructive confirmation, Restricted Mode blocked-action surfacing, plus wiring four audit producers.
- **023 Configuration** — file format, schema, atomic validation, diagnostics, hot-reload policy with the security-sensitive-settings rule, and moving AI CLI profiles from code-defined to user configuration.
- **024 Git Integration** — repository detection, branch, dirty state, per-file status. Must honor the subprocess safety rules already specified in RFC-012 (reviewed non-project-local executable, no shell, deterministic argv, sanitized environment, no workspace hooks, bounded time/output).
- **025 Notifications** — domain model, levels, actionable wording, and surfacing in Project Board and status bar.
- **026 Watcher and Multi-Document** — debounced watching, batching under churn, multi-document model replacing the RFC-006 single-document limitation, overwrite confirmation.
- **027 Crash Recovery** — unsaved buffer persistence and labelled recovery on restart.
- **028 Cross-Platform** — PTY, watcher, process groups, config paths, clipboard on Windows and macOS, with per-platform evidence.
- **029 Documentation, CI, Release Automation** — `docs/src` mdBook by persona, CI gates and build matrix, release automation.

## Scheduling Recommendation

**Start RFC-021 and RFC-023 now, in parallel with the RFC-014 spike.** Both are headless, neither depends on the substrate, and both remove risk from later milestones:

- **RFC-021 (command approval)** is the product's central safety promise and the largest honesty gap in the current release. Building and proving the model now means M11 only has to render a dialog over an already-reviewed policy, instead of designing the policy under UI pressure.
- **RFC-023 (configuration)** unblocks AI CLI profiles as user data — currently a documented RFC-010 limitation where profiles are code-defined only — and is a prerequisite for the `sensitive_config_changed` audit producer.

Sequencing these behind the GUI would leave the two most product-defining gaps until last, when schedule pressure is highest. That is the wrong order for work whose correctness matters most.

**RFC-024 (Git) and RFC-027 (crash recovery)** are also parallel-ready and can absorb capacity whenever GUI work blocks.

## Standing Constraints

Every RFC in this queue inherits these. They are not restated in each document.

1. **No product code may depend on a GUI substrate until the RFC-014 decision record is accepted.**
2. **The RFC-009 accepted-sequence boundary is not widened without an explicit reviewed decision and a threat-model amendment.**
3. **Honesty over completeness.** Every release states which audit producers are wired, that command approval does not exist until M11, and that support is Linux-only until M14.
4. **No colour-only status.** `NFR-UX-002` applies to every rendered surface.
5. **Untrusted content stays untrusted.** Terminal output, transcripts, diffs, Git metadata, and file contents are rendered as data, never as trusted chrome.
6. **Release at every milestone.** See the ROADMAP tracking rules.

## Maintenance

This plan is reviewed like product scope. When a milestone completes, mark its RFCs implemented, re-check the gap analysis against the requirements, and re-confirm the queue order before starting the next one.
