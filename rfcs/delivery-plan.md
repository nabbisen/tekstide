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
| i18n and text safety | Project rules, UI/UX §18 | **Complete** — catalog, locale fallback, pluralization, shared text-safety primitive, and mechanical enforcement. Translation *content* and runtime locale switching remain out of scope (RFC-016 closed 2026-08-01) |
| Command approval | `REQ-AGENT-012`, `013`; `REQ-SEC-012`, `013` | **Model complete and audited; headless and unreachable** — no adapter-spawn pathway, no dialog. Cooperative, not enforced. RFC-021 closed 2026-07-30 |

### Not implemented

| Area | Requirements | Milestone |
| --- | --- | --- |
| **Desktop GUI** — every rendered surface | External design §3, UI/UX baseline | M8-M11 |
| **Configuration system** — no module at all | `REQ-CONFIG-001`..`007` | M12 |
| **Git integration** — no module; RFC-012 detector reports Git unavailable | `REQ-GIT-001`..`007` | M12 |
| **Notifications** — no domain type | `REQ-NOTIFY-001`..`005` | M12 |
| **File watcher** | `REQ-FILE-003`, `004` | M13 |
| **Multi-document** — one active document only | External design §3.4 | M13 |
| **Syntax highlighting** | `REQ-EDIT-003` | M10 (optional) |
| **Crash / unsaved buffer recovery** | `REQ-RECOVER-005` | M13 |
| **Audit producers** — 7 of 12 families unwired | `REQ-SEC-014` | M9, M11, M12 |
| **LSP** | `REQ-LSP-001`..`005` | Deferred beyond 1.0 by design |
| **Cross-platform** — Linux only | `NFR-PORT-001`..`003` | M14 |
| **Documentation** — `docs/` absent | Project rules | **Split 2026-08-01 at the owner's direction ("as soon as possible").** Minimal user documentation → **M9, alongside RFC-017**. Full `docs/src` mdBook by persona stays with RFC-029 (M14). |
| **CI** | — | M14 |
| **Pre-rendered English in `tekstide-core`** — 5 sites the catalog cannot reach; `ProjectBoardRow::trust_label` renders today, from two independent sources (`WorkspaceTrust::label()` and `recent_project_row`'s hardcoded literal) | RFC-016 §Enforcement | **M9, alongside RFC-017** — scheduled 2026-08-01. Small (`ProjectBoardRow` exposes `WorkspaceTrust` and an equivalent for the recent-project path; the shell selects catalog keys). Not deferred to RFC-019/020: the string renders **today**, and M10 is two milestones away. No RFC needed — the same treatment as the RFC-004 redaction gap. |
| **Environment secret redaction** — RFC-004 states the policy; no pattern set exists in code | RFC-004 | M12 (with configuration) |

**Note on the redaction gap.** Found during PR-021-C review (response 110): RFC-004 says Tekstide "may redact known secret-like environment variable values," and the RFC-021 handoff told the developer to reuse "the secret-like patterns already used for environment redaction." No such pattern set is implemented. RFC-021's classifier carries its own `SECRET_LIKE_PATH_PATTERNS`, deliberately **not** shared: that list matches filesystem path components (`.ssh`, `.aws`), while redaction needs environment *variable names* (`AWS_SECRET_ACCESS_KEY`, `GITHUB_TOKEN`). These are different pattern kinds and must not be consolidated into one list where half the entries are inert in each use. Whichever RFC implements environment redaction authors its own.

### Audit producer coverage

Five of twelve v1 families have runtime producers. The remaining seven are the clearest measure of how much product surface is still missing. `command_approval` is wired but **produces nothing**, because nothing calls the coordinator — counted as wired, listed with that caveat:

| Family | Producer | Milestone |
| --- | --- | --- |
| `trust_change` | wired | — |
| `managed_process_lifecycle` | wired | — |
| `root_access_blocked` | wired | — |
| `audit_store_recovery` | wired | — |
| `paste_blocked` | none | M9 |
| `plain_terminal_observation` | none | M9 |
| `command_approval` | wired, no caller | M11 (reachable only once the adapter-spawn slice lands) |
| `safe_close_decision` | none | M11 |
| `restricted_mode_blocked` | none | M11 |
| `project_added` | none | M11 |
| `sensitive_config_changed` | none | M12 (needs config system) |
| `transcript_purge` | none | M12 |

## RFC Queue

Status values: **In progress** · **Next** · **Queued** · **Blocked**

| RFC | Title | Milestone | Depends on | Headless | Status |
| --- | --- | --- | --- | --- | --- |
| 014 | Desktop GUI Substrate and Terminal Rendering Strategy | M8 | — | no | **Closed 2026-08-01.** Decision `iced` + Option A; R1/R6 discharged by RFC-015, R4-R7 carried to RFC-017. Moved to `done/` |
| 015 | Application Shell and Rendered Surface Model | M8 | 014 | no | **Implemented and closed 2026-08-01.** `0.4.0` (B/C/D/F/G) + `0.4.1` (E, focus indicator, C4). Moved to `done/`. RFC-014 R1 and R6 discharged |
| 016 | Internationalization and Localization | M8 | 014 | partly | **Implemented and closed 2026-08-01** (PR-016-B/C/D/E/F). Moved to `done/` |
| 017 | Terminal Renderer and Immersion Mode | M9 | 014, 015 | no | **Accepted 2026-08-01 — next up.** Unblocked: 014 decided, 015 shipped `0.4.0` |
| 018 | Rendered Paste Protection and Trusted UI | M9 | 017 | no | Blocked |
| 019 | Editor and Explorer Surfaces | M10 | 015 | no | Blocked |
| 020 | Diff Review and AgentRun Report Surfaces | M10 | 015 | no | Blocked |
| 021 | Command Approval Model and Adapter Capability | M11 | — | **yes** | **Implemented headless and fully closed 2026-07-30. Moved to `done/`. Not reachable by any user until the adapter-spawn slice lands** |
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
- **029 Documentation, CI, Release Automation** — `docs/src` mdBook by persona, CI gates and build matrix, release automation. **Minimal user documentation was pulled forward to M9** (owner, 2026-08-01); RFC-029 keeps the full mdBook.

**Minimal user documentation — M9 scope, and why it is small.** Handoff: [`handoffs/minimal-user-documentation.md`](./handoffs/minimal-user-documentation.md). `0.4.1` is installable from crates.io and starts a GUI, so there are now users who are not contributors. README's Quick Start still says `cargo run -p tekstide`, which is a *contributor* instruction — someone who ran `cargo install tekstide` has no correct command anywhere. Five items, none of them design work:

1. **Fix Quick Start for installed users** (`cargo install tekstide`, then `tekstide`), keeping the from-checkout instructions clearly marked as such.
2. **Keyboard reference.** `Ctrl+Alt+P` (Project Board), `Ctrl+Alt+M` (mode switch), `Tab`/`Shift+Tab` (focus cycle), `Esc`/`Enter` in modals. These exist and are documented nowhere a user would look.
3. **What Tekstide does and does not do today** — largely assembling what README's Current Status and the changelog already say honestly.
4. **Where local state lives** (recent projects, audit store) and how to purge it. A privacy-relevant question a user can currently only answer by reading source.
5. **A pointer to known limitations**, so the honest disclosures are reachable from the README rather than only from RFC closeouts.

## Scheduling Recommendation

### Current, as of 2026-07-30

**Do not cut a release now.** There is no user-visible change to describe, and RFC-021's honesty constraint makes even naming it awkward — the model, protocol, channel, classifier, and coordinator are complete and tested, and none of it is reachable. A `0.3.1` would spend its release notes saying so.

**Next, in this order:**

1. **RFC-016 PR-016-C** (text safety) — owner-approved to lead RFC-016, and it now also retires the duplicate-escaping debt described under Release Cycle Tracking. Adopt `approval::coordinator::display_argv`'s escaping as the canonical shared primitive; make `approval` call it.
2. **RFC-015** (application shell) — the M8 spine. Everything in M8-M10 waits on it.
3. **RFC-016 PR-016-B, D, E** — catalog plumbing, after the shell exists to hold it.

**`0.4.0` remains M8**, unchanged. The version/milestone mapping in ROADMAP stays; the correction is to work the milestone that bears the version, not to renumber.

### `0.4.0` / `0.4.1` split — decided 2026-07-30

Owner delegated the call, allowing either and accepting `0.4.1` if it is better and safer. **Decision: the Project Board stays in `0.4.0`; the mode-switching scaffolding moves to `0.4.1`.**

The framing in the original question was wrong, and checking the current binary is what showed it. `crates/tekstide/src/main.rs` is 40 lines that print `shell.render_text()` and exit — `0.3.0` is a non-interactive CLI, and Project Board *state* (`ApplicationShell`, `recent_project_state`) already exists in `tekstide-core`. So PR-015-D is a **rendering** slice over state that is already built and reviewed, not a new-feature slice.

That inverts the safety argument:

- **Deferring the board makes `0.4.0` a GUI window with chrome and nothing in it.** A user could launch it and do less than `0.3.0`'s CLI already does. A release that regresses observable function is the composition failure recorded above, one layer up — and it would be the second consecutive cycle with nothing a user can reach.
- **Deferring the board buys no safety**, because the security-critical slice is PR-015-C (input routing and the three message classes), and it is *upstream* of PR-015-D. The board renders through the routing model, so it cannot ship ahead of that gate. Cutting D would ship a window, not a smaller risk.

**`0.4.0` contents:** PR-016-C (text safety, canonical shared primitive) → PR-015-B (window, layers, chrome, theme and i18n seams) → PR-015-C (input routing, focus, modal exclusivity — probed empirically) → PR-015-D (Project Board over existing state) → PR-015-F (R1 latency, warm startup) → PR-015-G (closeout).

**`0.4.1` contents:** PR-015-E (mode switching, Content-mode sidebar and main-area scaffolding) and the `NFR-PERF-002` mode-switch evidence that depends on it.

### Two M8 scope errors found while deciding this

Both are mine, from the M8-M14 restructure on 2026-07-28.

1. **"Theme and typography primitives; configurable font family and size plumbed through"** — user-configurable typography needs RFC-023 (configuration system, M12). M8 can build the primitives and the plumbing seam; it cannot deliver user configuration. ROADMAP wording corrected.
2. **`NFR-PERF-002` (mode switch) as an M8 review gate** — mode switching in M8 switches between the Project Board and an empty content area, because Terminal Mode has no terminal until M9. Measuring it in M8 measures scaffolding. The gate moves to `0.4.1` with PR-015-E, and the substantive mode-switch measurement belongs in M9 when there are two real modes.

### Original (2026-07-28), retained for the reasoning

**Start RFC-021 and RFC-023 now, in parallel with the RFC-014 spike.** Both are headless, neither depends on the substrate, and both remove risk from later milestones:

- **RFC-021 (command approval)** is the product's central safety promise and the largest honesty gap in the current release. Building and proving the model now means M11 only has to render a dialog over an already-reviewed policy, instead of designing the policy under UI pressure.
- **RFC-023 (configuration)** unblocks AI CLI profiles as user data — currently a documented RFC-010 limitation where profiles are code-defined only — and is a prerequisite for the `sensitive_config_changed` audit producer.

Sequencing these behind the GUI would leave the two most product-defining gaps until last, when schedule pressure is highest. That is the wrong order for work whose correctness matters most.

**RFC-024 (Git) and RFC-027 (crash recovery)** are also parallel-ready and can absorb capacity whenever GUI work blocks.

## Release Cycle Tracking

Added 2026-07-30 after the owner asked whether release cycles were being watched. They were not — the review queue was being run slice by slice with no cadence measurement. This section is the correction, and it is reviewed every cycle.

### Observed cadence

| Release | Date | Interval |
| --- | --- | --- |
| `0.1.0` | 2026-07-06 | — |
| `0.2.0` | 2026-07-17 | 11 days |
| `0.3.0` | 2026-07-28 | 11 days |
| *(unreleased)* | — | 2 days elapsed |

**Cadence is not the problem.** Two releases at an 11-day interval, and the current cycle is 2 days old. Nothing is late.

### Composition is the problem

33 commits since `0.3.0`. Releasable user-visible surface in them: **zero.**

- ~20 commits are RFC-021 (M11), which closes headless — no adapter-spawn pathway, so neither `ApprovalCoordinator` nor `inject_token_into_environment` has a production caller.
- ~8 are RFC-014's GUI spike, in a crate marked `publish = false`.
- ~5 are RFC and roadmap documents.

A release cut today would be indistinguishable from `0.3.0` for a user. ROADMAP §"Why M8 Was Split" names this exact failure mode — *"accumulating unreleased work, which is the failure mode that produced three unreleased milestones before `0.3.0`"* — and M8, the milestone that bears `0.4.x`, has had **zero implementation commits** since the substrate decision was accepted. At two days this is a trajectory, not yet a failure. It is the trajectory that section was written to prevent.

### Sequencing cost incurred: duplicate text-safety escaping

RFC-016 Open Question 1 reserves for PR-016-C: *"Should the escaping function live in `tekstide-core` (shared with RFC-021's approval model) or remain shell-local?"* RFC-016 §Risks states: *"escaping belongs to the shared untrusted-text render path, not to each surface."*

Response 115 Required A directed a full `Cc`+`Cf` category escaping implementation into `approval::coordinator::display_argv`, pre-empting that reserved decision and creating the second implementation RFC-016 warns against. **This is the reviewer's error, not the implementers'** — requiring escaping in E1 was correct because `display_command` was being constructed there unsafely, but it should have been required as a call into a shared primitive, with PR-016-C sequenced first. PR-016-C had already been owner-approved to lead RFC-016 on 2026-07-29 at 17:06; RFC-021 implementation began at 17:09 instead.

**Consequence:** PR-016-C must adopt `display_argv`'s escaping as the canonical implementation and `approval::coordinator` must call it, rather than PR-016-C building a parallel one. Recorded as required scope for PR-016-C, not as a later cleanup.

### Metrics to carry each cycle

1. **Releasable-surface ratio** — commits producing user-visible change ÷ total. Currently 0/33.
2. **Milestone/version alignment** — is the milestone bearing the next minor version the one being worked? Currently no: M11 was worked, M8 bears `0.4.x`.
3. **Review rounds per slice** — RFC-021: 5 implementation slices, 8 review requests, **1.6 rounds/slice**. Each extra round found a real defect (including the duplicate-proposal-id approval-laundering bug), so this is not waste — but estimates for remaining RFCs should assume 1.6×, and none did.
4. **Next-release contents** — named before the cycle starts, not discovered at the end.

### Standing rule

**A cycle may not consist entirely of headless work.** Headless parallel tracks exist to de-risk later milestones (ROADMAP §Parallel Tracks), not to become the only track. When a cycle's work is entirely unreachable by a user, either the release-bearing milestone gets capacity in the same cycle or the cadence commitment is explicitly suspended with the owner's agreement.

### Cycle review: `0.4.1` → present, measured 2026-08-08

**Cadence is healthy. Composition has the same failure as the `0.3.0` cycle, for a different reason.**

45 commits in 6 days since `0.4.1`. **Releasable user-visible surface in them: zero.**

RFC-017 delivered a real, filtered, PTY-backed terminal renderer — grid, split policy, session bar, input, audit producer — and **every part of it is behind `TEKSTIDE_TERMINAL_DEMO`**. No keybinding, command, or UI affordance launches a terminal. A user running HEAD sees exactly what `0.4.1` gave them. PR-017-E disclosed this honestly at the time ("no real terminal-creation UX"), and it was correct not to absorb that into a rendering slice; the gap is that nothing scheduled it afterwards.

Against the four metrics:

1. **Releasable-surface ratio: 0/45.** Same figure as the `0.3.0` cycle's 0/33, and this time it is not headless work — it is *reachable-by-nobody* work, which the standing rule covers in spirit but names less precisely. **Amend the rule's wording accordingly**: the test is whether a user can reach it, not whether it has a GUI.
2. **Milestone/version alignment: good, and improved.** M9 bears `0.5.x` and M9 is what was worked. The `0.3.0` cycle failed this (M11 worked, M8 bore `0.4.x`); this cycle does not.
3. **Review rounds per slice: 2.3** (16 requests ÷ 7 slices), against RFC-021's 1.6. **PR-017-G alone consumed 6** (requests 154–159). Excluding it: 10 ÷ 6 = **1.67**, on the established baseline. The process is not degrading — one measurement slice on a shared, heavily loaded machine (swap exhaustion, then a dual-GPU EGL failure) absorbed the entire overrun, and produced real findings while doing it.
4. **Next-release contents: not named before the cycle.** Nobody stated what `0.5.0` contains. That is the metric that would have caught items 1 and 3 early, and it is the one still not being run.

**Recommendation: terminal-launch UX before RFC-018.**

`rfcs/future-work.md` §Terminal / PTY Runtime already lists *"Add app/UI commands for launching, selecting, and closing terminals"* — known, unscheduled. Three reasons to schedule it now rather than after RFC-018:

- **`0.5.0` cannot honestly claim terminal support while no user can open a terminal.** Standing Constraint 3 (honesty over completeness) makes that a release-notes problem, not just a product one.
- **RFC-018's evidence inherits the gate.** Spoofing resistance proven against a demo-gated terminal is weaker evidence than against one a user opened, and RFC-018's whole purpose is adversarial proof.
- **Two consecutive M9 RFCs of terminal work with nothing reachable** is the trajectory the standing rule exists to interrupt, and it would be the third such cycle in the project's history.

Small slice, plausibly one PR against RFC-008's existing lifecycle API — `AppState::attach_terminal_session` and `assign_terminal_visible_slot` already exist and are exercised, so this is wiring a command to reviewed code rather than new runtime work.

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

## Owner decisions, 2026-08-11

Recorded here because a decision that lives only in conversation is one nobody can act on later.

1. **The RFC-018 background scrim is accepted.** It needed only acceptance — the design, the rationale, and the evidence (`01` vs `05`'s content-dependent reversal) are already in `rfcs/future-work.md` and RFC-018's closeout. A small slice against the existing modal layer; no new RFC.

2. **Readiness-driven terminal I/O is to be scheduled.** `future-work.md` sizes it as its own slice with P1/P2 re-enumeration and re-ablation, plausibly an RFC-009/RFC-017 amendment rather than a patch — so scheduling it means scoping it first, which is the architect's. It is the only open item a user can currently feel: three terminals, ~374 KB/s, `NFR-PERF-004` unmet.

3. **Amendments to closed RFCs stay with the architect as design authority.** Confirmed after RFC-006 Amendment 1 (decided directly) and RFC-012 Amendment 1 (raised and authorised). The distinction that matters is not "closed RFC" but **cost**: additive and invariant-preserving is the architect's; anything with a migration, a retention change, or a breaking API removal comes to the owner. RFC-012 Amendment 1 was correctly escalated on the last of those.

4. **The crates.io page corrections ride the next release.** Both are already correct in the published artifacts; only the rendered pages were unverified, and a release re-renders them.

5. **`0.7.0`: as soon as reasonable, not as soon as possible.** The version is already forced to a minor bump by RFC-012 Amendment 1's breaking removal. What is missing is reachable surface — 20 commits since `0.6.0` touched `crates/tekstide/` zero times, so a release today would be indistinguishable from `0.6.0` for a user. It cuts when items 1 and/or 2 land, or when RFC-020 makes RFC-024 reachable.
