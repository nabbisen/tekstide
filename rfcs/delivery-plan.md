# Tekstide Delivery Plan

Date: 2026-07-28
Covers: M8 through M14 (`0.4.x` → `1.0.0`)
Companion to: [`../ROADMAP.md`](../ROADMAP.md)

This is the ordered RFC queue for the remaining work, with the requirements gap analysis that justifies it. It answers one question for a developer: **what do I pick up next, and what do I read first?**

## How to pick up work

1. Find the next RFC in [§ RFC Queue](#rfc-queue) whose status is *Accepted* and whose dependencies are met — or equivalently, look in `rfcs/accepted/`, which holds exactly those (RFC-037, 2026-08-19).
2. Read the RFC in **`rfcs/accepted/NNN-*.md`** — that is where startable work lives (RFC-037). `rfcs/proposed/` is for RFCs still under review, `rfcs/done/` for shipped ones.
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
| **Audit producers** — ~~7~~ ~~3~~ **2** of 12 families unwired as of 2026-08-19 (`safe_close_decision`, `sensitive_config_changed`). **`safe_close_decision` is unblocked as of RFC-039's acceptance** — it was scoped out of RFC-031 for want of a close dialog, and RFC-039 D2 builds one and wires it | `REQ-SEC-014` | M12 |
| **LSP** | `REQ-LSP-001`..`005` | Deferred beyond 1.0 by design |
| **Cross-platform** — Linux only | `NFR-PORT-001`..`003` | M14 |
| **Documentation** — `docs/` absent | Project rules | **Split 2026-08-01 at the owner's direction ("as soon as possible").** Minimal user documentation → **M9, alongside RFC-017**. Full `docs/src` mdBook by persona stays with RFC-029 (M14). |
| **CI** | — | M14 |
| **Pre-rendered English in `tekstide-core`** — 5 sites the catalog cannot reach; `ProjectBoardRow::trust_label` renders today, from two independent sources (`WorkspaceTrust::label()` and `recent_project_row`'s hardcoded literal) | RFC-016 §Enforcement | **M9, alongside RFC-017** — scheduled 2026-08-01. Small (`ProjectBoardRow` exposes `WorkspaceTrust` and an equivalent for the recent-project path; the shell selects catalog keys). Not deferred to RFC-019/020: the string renders **today**, and M10 is two milestones away. No RFC needed — the same treatment as the RFC-004 redaction gap. |
| **Environment secret redaction** — RFC-004 states the policy; no pattern set exists in code | RFC-004 | M12 (with configuration) |

**Note on the redaction gap.** Found during PR-021-C review (response 110): RFC-004 says Tekstide "may redact known secret-like environment variable values," and the RFC-021 handoff told the developer to reuse "the secret-like patterns already used for environment redaction." No such pattern set is implemented. RFC-021's classifier carries its own `SECRET_LIKE_PATH_PATTERNS`, deliberately **not** shared: that list matches filesystem path components (`.ssh`, `.aws`), while redaction needs environment *variable names* (`AWS_SECRET_ACCESS_KEY`, `GITHUB_TOKEN`). These are different pattern kinds and must not be consolidated into one list where half the entries are inert in each use. Whichever RFC implements environment redaction authors its own.

### Audit producer coverage

**Re-checked against the code 2026-08-19, at RFC-031's closeout — the table below had been
stale in four rows.** `paste_blocked` and `plain_terminal_observation` were recorded as having
no producer long after RFC-017/RFC-018 gave them one, and `command_approval` was recorded as
having no caller after RFC-022 gave it a real audited producer. **Ten of twelve families now
have producers** (nine at RFC-031's closeout, plus `transcript_purge` at RFC-033's the same day);
two remain, each owned by an accepted RFC.

| Family | Producer | Owner |
| --- | --- | --- |
| `trust_change` | wired, real user callers (RFC-032) | — |
| `managed_process_lifecycle` | wired | — |
| `root_access_blocked` | wired | — |
| `audit_store_recovery` | wired (in `audit/recovery.rs`, not the coordinator) | — |
| `paste_blocked` | wired (RFC-018) | — |
| `plain_terminal_observation` | wired (RFC-017) | — |
| `command_approval` | wired, with a real producer as of RFC-022 — but exercisable only by the reference adapter, since no shipping AI CLI speaks RFC-021's protocol | — |
| `restricted_mode_blocked` | **wired 2026-08-19 (RFC-031)** | — |
| `project_added` | **wired 2026-08-19 (RFC-031)** | — |
| `safe_close_decision` | none — blocked on a dialog that does not exist | RFC-031 scoped it out; needs a surface first |
| `sensitive_config_changed` | **API built, zero callers** (`AuditCoordinator::record_sensitive_config_policy_increase`/`_reduce`, RFC-023) | RFC-036 — re-homed 2026-08-22 when RFC-023 closed headless. Not a missing producer: a sensitive setting cannot change at runtime in an application that never loads configuration |
| `transcript_purge` | **wired 2026-08-19 (RFC-033)** — records the purge and its scope, never a path or a byte count | — |

**Nothing renders the audit store.** Every row above describes what is *recorded*, not what any
user can see; there is no view of it at all.

## RFC Queue

Status values: **In progress** · **Next** · **Queued** · **Blocked**

| RFC | Title | Milestone | Depends on | Headless | Status |
| --- | --- | --- | --- | --- | --- |
| 014 | Desktop GUI Substrate and Terminal Rendering Strategy | M8 | — | no | **Closed 2026-08-01.** Decision `iced` + Option A; R1/R6 discharged by RFC-015, R4-R7 carried to RFC-017. Moved to `done/` |
| 015 | Application Shell and Rendered Surface Model | M8 | 014 | no | **Implemented and closed 2026-08-01.** `0.4.0` (B/C/D/F/G) + `0.4.1` (E, focus indicator, C4). Moved to `done/`. RFC-014 R1 and R6 discharged |
| 016 | Internationalization and Localization | M8 | 014 | partly | **Implemented and closed 2026-08-01** (PR-016-B/C/D/E/F). Moved to `done/` |
| 017 | Terminal Renderer and Immersion Mode | M9 | 014, 015 | no | **Accepted 2026-08-01 — next up.** Unblocked: 014 decided, 015 shipped `0.4.0` |
| 018 | Rendered Paste Protection and Trusted UI | M9 | 017 | no | **Implemented and closed 2026-08-10** (`0.5.1`). PR-018-G, the carried-forward background scrim, landed 2026-08-12 (`0.7.0`) |
| 019 | Editor and Explorer Surfaces | M10 | 015 | no | **Implemented and closed 2026-08-11** (`0.6.0`) |
| 042 | Change Content Legibility | M12 | 041, 024, 018 | no | **Implemented and closed 2026-08-26.** Moved to `done/`. Found by the `0.14.0` release gate, not the suite. Lines are lines; the frame and the file-row list sit outside the content's scroll region; the content type lives behind a module boundary so rendering it outside its container is a compile error; over a measured 4,000-line bound the preview refuses whole rather than truncating. Three review rounds — the first two found guards that passed while I defeated the property they named |
| 034 | Change Review Actions and Review State | M12 | 020, 012, 042 | no | **Implemented and closed 2026-08-26.** Moved to `done/`. RFC-012's `transition_change_set_review_state` finally has a caller. Two controls offering an opinion, never a fact; one sentence carrying finality, session-scope and "no file is touched", each clause independently guarded; reachable by mouse and by `a`/`r`. No audit record, and the successor question — should the audit store record a user's decision about generated code? — is written down rather than left to be rediscovered |
| — | Audit-store test isolation | M12 | — | no | **Scoped 2026-08-26**, third of three for `0.15.0`, ahead of the doc invariants because it corrupts the evidence every other slice is accepted on. Measured, not hypothesised: parallel 6–23 failures per run, serial 444/444 clean, raising the query window made it worse. 23 test call sites share one SQLite store resolved from the real environment — and with `XDG_STATE_HOME` unset the suite writes the developer's own audit store. Handoff: `handoffs/audit-store-test-isolation.md` |
| — | The suite assumes it owns the machine | M12 | — | no | **Scoped 2026-08-26.** Two test defects, one cause: the suite treats a shared machine as its own. `open_real_agent_run_state_root` is the last production consumer of `AppStatePathProvider::linux_default()` a test can reach, so `transcripts/` and `approval/` still land in a real `$HOME` — worse than the audit-store version was, because RFC-033 shows users a retained-bytes count and offers a purge, both now including test debris. Plus a wall-clock assertion that failed 6 of 7 runs at load 59.7 while its own message says a failure cannot be load. Handoff: `handoffs/suite-assumes-it-owns-the-machine.md` |
| 044 | Surface-Local Keyboard Affordances | M12 | 039, 040, 016 | no | **Implemented and closed 2026-08-27.** Moved to `done/`. The gap was inexpressible, not unnoticed: `control_coverage`'s domain excluded closing a project and `ControlCoverage` had no `MouseOnly` arm. A production registry of fourteen surface actions, a test-only exhaustive mirror so an undecided action fails to compile, `Delete` closing the highlighted tab through the button's own path, and Help/`--help` generated from that one registry. One gap counted, one closed, zero remaining |
| — | PTY master fd inheritance | M12 | — | **yes** | **Scoped 2026-08-26. Security fix, ahead of RFC-043 and not gated on it — no product decision in it.** `OpenPty::new` uses `libc::openpty` with no `O_CLOEXEC` and `spawn_pty_child` closes no master, so every child inherits every PTY master open in the parent; one live `/bin/sh` measured holding 27. A PTY master is read/write access to that terminal, so an AI CLI agent in one project's terminal can read and inject into another's. Crosses RFC-009's boundary in shipped code. Handoff: `handoffs/pty-master-fd-inheritance.md` |
| 043 | Terminal Process Containment | M12 | 008, 009, 013 | **yes** | **Implemented and closed 2026-08-27.** Moved to `done/`. Session-scoped termination, master-close before `SIGHUP` on both paths, zombies excluded from the survivor scan, and the safe-close audit field read from a real session re-scan instead of inferred from an outcome variant that could not see a sibling process group. Eight review rounds; the leak guard from PR-043-A caught two of the regressions in PR-043-B within days of being built. Orphans 28 → 0, measured |
| 035 | Change Detection Coverage and Disclosure | M12 | 012, 020 | no | **Implemented and closed 2026-08-25.** The `.git/hooks`/`.git/config` supervision hole closed and `max_changed_paths` stops discarding a list it computed. Items 3 and 4 (mid-run triggers, baseline surviving the application) stay deferred. **Added to this queue retroactively at Final Acceptance** — authored, accepted, implemented and closed without ever appearing here, the third instance of the bookkeeping gap RFC-032's own row records |
| 036 | Dormant Capability Closure | M12 | — | no | **Implemented and closed 2026-08-28.** Moved to `done/`. A decision per orphan with a measured count and D4's search shape. Nine functions off the published surface into `0.16.0`, named individually; four kept against RFC-045; zero wired, stated as a finding. Found two shipped defects its own opening argument predicted — an agent-run launch writes no durable audit record (RFC-046), and a corrupted audit store fails silently while the interface reports "Calm" (RFC-047) |
| 045 | Configuration Reachability | M12 | 023, 036 | no | **Reserved 2026-08-27 by RFC-036's D2**, unauthored. RFC-023 shipped a configuration system nothing constructs: `ConfigStore`, `to_ai_cli_profile`, `set_resource_limits` and the `sensitive_config_changed` producer are all correct, all tested, none reachable. They are conditioned rather than dead, and D2's named-consumer rule needs that consumer to exist as a number rather than an intention. Also carries RFC-023's own OQ3 first-use confirmation gate, deferred to exactly this slice |
| 046 | Managed AgentRun Audit Trail | M12 | 013, 036 | **yes** | **Reserved 2026-08-28 by RFC-036's triage.** `launch_managed_agent_run` and siblings write real `ManagedProcessLifecycle` records, are proven against the real store, and have zero production callers — so launching an AI CLI agent, the action trust and command approval exist to control, leaves no durable audit record. The crate README claimed otherwise until 2026-08-27. What to record and when is design, hence an RFC |
| 047 | Audit Store Corruption Recovery | M12 | 013, 036 | **yes** | **Reserved 2026-08-28 by RFC-036 PR-036-C.** Two corruption shapes reproduced against the release binary in a scratch state root: a corrupted `audit.sqlite3` and a blocked migration. Both **completely silent** — the app runs, the interface says "Calm", `recover()`/`resume()` exist and nothing calls them. Recommended split: auto-`resume()` the safe case, decide the corrupted case explicitly, and log something findable in both, since today there is not even that |
| 041 | Change Content Preview | M12 | 024, 020 | no | **Implemented and closed 2026-08-26.** Moved to `done/`. RFC-024 built and gated content access in `0.7.0`; it sat with zero production callers for six releases because `add_detected_generated_change_set` discarded the `DetectedChanges` it needed. A change review row is now a real button and renders the file's content per change kind, with a stale baseline refusing and naming why. Content preview, not a diff — the two-sided case stays blocked on RFC-030, disclosed on the surface itself, not cancelled |
| 040 | Affordance Completion | M12 | 039 | no | **Implemented and closed 2026-08-25.** Moved to `done/`. Visible controls 3 → 11 of 13; every modal completable by mouse. **Originally accepted 2026-08-25, first of three scheduled together.** RFC-039's audit: three of thirteen live actions have a visible control, and every one of the nine modals is keyboard-only for its own decision. The audit becomes a test in the first slice so the rest is measured rather than asserted |
| 020 | Diff Review and AgentRun Report Surfaces | M10 | 015, 024, **adapter-spawn** | no | **Implemented and closed 2026-08-25.** Moved to `done/`. Both surfaces ship; the change review surface is metadata-only and diff content stays unrendered. Unblocks RFC-034. Previously: **AgentRun report shipped in `0.12.0`; the change review surface scheduled 2026-08-25 and UNBLOCKED** — scoped that day: a real `ChangeSet` exists in production, `change_sets()` is public, and `bounded_summary` already carries `omitted_changed_file_count` and `detection_status`. Route, render arm and visible control are the remaining work. **This RFC's own slice letters are swapped** and the handoff is named by surface instead. Superseded below: **Model complete, both surfaces BLOCKED** (2026-08-15, response 200). The transcript reader landed and is reviewed. Neither surface can be built: nothing in production creates an `AgentRun` (`launch_agent_run_with_runtime` and `add_agent_run` have no production caller) or a `ChangeSet` (`crates/tekstide` has zero references to change sets, baselines, or detection). Both would render nothing, forever. Real prerequisite is the adapter-spawn pathway, below |
| 021 | Command Approval Model and Adapter Capability | M11 | — | **yes** | **Implemented headless and fully closed 2026-07-30. Moved to `done/`. Not reachable by any user until the adapter-spawn slice lands** |
| 022 | Adapter Spawn and the Command Approval Surface | M11 | 015, 021 | no | **Implemented and closed 2026-08-17.** Moved to `done/`. Retitled from the reserved "Security Dialogs and Audit Producer Completion" — the dialog and the spawn pathway proved inseparable (an adapter whose requests nobody can answer is useless or dangerous), and audit-producer completion split out to RFC-031. **Not reachable by any real user**: no shipping AI CLI speaks RFC-021's protocol, so `Managed` is exercisable only by the reference adapter, a test artifact |
| 032 | Workspace Trust Granting | M11 | 004, 013, 022 | no | **Implemented and closed 2026-08-17.** Moved to `done/`. Authored just-in-time 2026-08-17 after RFC-022's closeout found that `grant_project_trust` had no production caller, so every project was permanently `Restricted` and RFC-022's whole agent-run chain was unreachable behind it. **Added to this queue retroactively at Final Acceptance** — it was authored, implemented and closed without ever appearing here, the same bookkeeping gap that produced the RFC-024/030 collision. **It has now happened twice more**: RFC-034, RFC-035 and RFC-036 were accepted 2026-08-18 and none appeared in this table until 2026-08-25, by which time RFC-035 had been implemented and closed. The gap is not that people forget — it is that nothing checks. Every other index in this repository has a mechanical guard (`rfc_docs_invariants`); this table has none |
| 031 | Audit Producer Completion | M11 | 013, 004 | partly | **Implemented and closed 2026-08-19.** Moved to `done/`. `restricted_mode_blocked` and `project_added` wired, each proven from a real user path; `safe_close_decision` scoped out, blocked on a dialog that does not exist. **The last M11 item** |
| 033 | Transcript Lifecycle Controls | M11 | 011, 013 | no | **Implemented and closed 2026-08-19.** Moved to `done/`. Per-project capture opt-out, purge and retained-bytes visibility. Closes the limitation `0.11.1` published on a privacy claim |
| 039 | Interaction Model and Visible Affordances | M12 | 003, 005, 038 | no | **Implemented and closed 2026-08-25.** Moved to `done/`. Tab strip, switching, a route home, and closing a project with a real confirmation. Its own audit found three of thirteen live actions have a visible control — carried to RFC-040. **Originally accepted 2026-08-24**, after the owner reviewed RFC-038's first slices and named the real gap: no workflows, no button or link to open a project, close it, or return to the entrance. Five buttons exist application-wide and none on the entry surface; `close_project` has no production caller. Sequenced after RFC-038 and ahead of RFC-020/034 — a change-review surface is worth less than the ability to move between projects at all |
| 038 | First-Run and Project Entry | M12 | 005, 015 | no | **Implemented and closed 2026-08-24.** Moved to `done/`. The in-app route to open a project exists: path field, folder browser, one-key reopen, and a Help modal. Breaking change to `tekstide-core` (`ProjectBoardEmptyState`'s dead fields); `0.13.0` prepared, unshipped. **Originally accepted 2026-08-24 as the first M12 item.** The product has no in-app way to open a project — `add_project_from_path`'s only production caller is a CLI argument — so every capability below is gated behind a terminal invocation. Found by the owner running the `0.12.0` binary, three weeks and twelve releases after the empty state shipped naming two actions that do not exist. Scheduled ahead of RFC-020 PR-020-C: a second surface on an unenterable product is worth less than the door |
| 023 | Configuration System | M12 | — | **yes** | **Implemented and closed 2026-08-22.** Headless, exactly as this row warned: it shipped alone and *is* the zero-reachable-surface shape `0.7.0` nearly hit — accepted deliberately in its §Scoping addendum, with the consumer slice named and left unscheduled, rather than discovered at closeout. Three items it owned were re-homed to RFC-036 in the closing commit. **Carries two now-named gaps**: `OpenTrustSettings` aside, every navigation action with `KeybindingStatus::Configurable` and a `None` binding is *dead*, not pending — `OpenApprovalHistory` and `SwitchActiveProject` among them (see `future-work.md`) |
| 024 | Diff Preview Policy | M10 | 012 | **yes** | **Implemented and closed 2026-08-11** (`0.7.0`). Authored out of order as RFC-020's content-access prerequisite; carried RFC-012 Amendment 1, a breaking change |
| 030 | Git Integration | M12 | — | **yes** | Queued (parallel-ready). **Renumbered from 024** 2026-08-12: this row still claimed a number RFC-024 (Diff Preview Policy) had taken on 2026-08-11, so an M12 item was left unaddressable. 025-029 could not absorb the shift — RFC-029 is referenced from closed RFCs (013, 016) and from `handoffs/minimal-user-documentation.md`, and closed documents are not edited to match a later state |
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
- **030 Git Integration** — repository detection, branch, dirty state, per-file status. Must honor the subprocess safety rules already specified in RFC-012 (reviewed non-project-local executable, no shell, deterministic argv, sanitized environment, no workspace hooks, bounded time/output).
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

**RFC-030 (Git) and RFC-027 (crash recovery)** are also parallel-ready and can absorb capacity whenever GUI work blocks.

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

## Re-plan, 2026-08-15 — the bottleneck is adapter-spawn, not rendering

Authorised by the owner the same day, on the architect's recommendation, after response
200 found that **neither of RFC-020's surfaces can be reached**.

### What was found

Nothing in production creates the two things RFC-020 renders:

- **`AgentRun`** — `launch_agent_run_with_runtime` (`project/session.rs:376`) and
  `add_agent_run` (`:318`) have zero production callers; every call site is a test.
  `crates/tekstide`'s only references are an i18n dormancy annotation and
  `NavigationAction::OpenCurrentAgentRunDetail`, which returns `None`.
- **`ChangeSet`** — `crates/tekstide` contains zero references to change sets, review
  baselines, or generated-change detection. `add_detected_generated_change_set` has no
  production caller. `NavigationAction::OpenDiffReview` also returns `None`.

Built as scheduled, both surfaces would render nothing, forever — the
zero-reachable-surface failure the standing rule exists to catch, three days after it
gated `0.7.0`.

**This was the architect's error**, and a repeat: the same assumption went unchecked on
RFC-024 (diffability assumed without confirming the inputs existed), was recorded as a
lesson, and recurred one RFC later at larger scale. The convention that would have caught
it now exists in [`../ARCHITECTURE.md`](../ARCHITECTURE.md) §Evidence conventions —
*reachability comes before correctness*.

### The accumulation this exposed

**Four reviewed capabilities are dark behind one missing pathway:**

| Capability | State | Blocked by |
| --- | --- | --- |
| RFC-021 command approval | implemented, headless, closed | adapter-spawn |
| RFC-024 diff content | implemented, shipped `0.7.0`, no surface | adapter-spawn (no `ChangeSet` producer) |
| RFC-011 Amd. 1 transcript reader | implemented, reviewed | adapter-spawn (no `AgentRun`, so no transcript) |
| RFC-020 both surfaces | not built | adapter-spawn |

This is now the project's dominant risk. It is not that any one thing is wrong — every one
of those is correct and reviewed. It is that **the project keeps building models nobody can
see**, and each one added to the pile was individually justified.

The adapter-spawn pathway has been named in `future-work.md` as a standing theme since
2026-08-01. It was never scheduled.

### Decisions

1. **`0.8.0`'s spine becomes readiness-driven terminal I/O.** It is the only available work
   that improves something a user can reach today — terminals launch via `Ctrl+Alt+T`, and
   the latency floor, the ~374 KB/s throughput ceiling and the terminal-count limit are all
   felt. It was already scoped for `0.9.0`, so this moves it forward rather than inventing
   new work. Owns `NFR-PERF-004`. Architect scopes it as an RFC-009/RFC-017 amendment;
   the amendment comes to the owner, since it changes an invariant two slices were built to
   prove.

2. **Adapter-spawn becomes the M11 priority**, ahead of the rest of M11. It is what makes
   the four capabilities above reachable, and every milestone spent elsewhere adds to the
   pile rather than reducing it.

3. **RFC-020 stays open, model-complete, surfaces blocked.** It is not withdrawn and not
   re-scoped: the surfaces are correct work scheduled against inputs that do not exist yet.
   It resumes when adapter-spawn lands.

4. **M10 does not close** on RFC-020's closeout as previously planned, because the
   milestone's second half cannot be delivered. What M10 delivered — RFC-019's editor and
   explorer, RFC-024's diff policy, the transcript reader — is real; what it did not is
   recorded here rather than absorbed silently.

### Re-plan status, 2026-08-15 (end of day)

**Decision 1 is delivered.** RFC-017 Amendment 1 (readiness-driven terminal I/O) was
authored, authorised by the owner, implemented across PR-A1-A through PR-A1-D, and closed
the same day. `0.8.0` now has a spine that a user can feel: the 50 ms tick, the 10 ms sleep
and the terminal-pane truncation path are gone; throughput moved from ~374 KB/s to
~17.4-18 MB/s; `terminal_session_limit` was re-derived from a fresh headless N-pane
measurement rather than carried, 3 → 6.

**`NFR-PERF-004` remains not met**, for the second time and on better evidence: the
structural cause is removed, but "met" needs an *upper* bound on the end-to-end path, and
the previous "not met" only ever needed a *lower* one. See `future-work.md`
§Readiness-driven terminal I/O — including an **open owner question** on whether a
criterion we cannot verify under our own measurement discipline should be restated in terms
we can bound (RFC-015's `input-to-state-change`) or accepted as permanently unverified.

**Corrected 2026-08-17: there was no open owner question, and the paragraph above states
the boundary wrongly.** `NFR-PERF-004` already excludes compositor and GPU present time —
its own text says so, made explicit 2026-08-15 by RFC-017 Amendment 1, and
`ARCHITECTURE.md` §Evidence conventions applies the same rule to every `NFR-PERF-*`
figure. The "upper bound on the end-to-end path" framing describes a superseded reading of
the criterion. What remains is **a measurement**, on a machine that is not swapping, under
the bounded-output load the requirement names — not a decision, and nothing from the
owner. The stale note in `future-work.md` is corrected there too; both are kept rather
than deleted because the correction is the record.

**Decision 2 (adapter-spawn as M11 priority) has gained a hard prerequisite**, found while
this work was underway: `TerminalReader` has no transcript hook, and the path that had one
is no longer on the ingress. Recorded in `future-work.md` as blocking. Do not start
adapter-spawn without re-homing transcript capture.

**Still true**: RFC-020's two surfaces remain blocked, model-complete, waiting on
adapter-spawn. M10 does not close.

### Re-plan status, 2026-08-16

**Decision 2's named prerequisite is discharged.** RFC-011 Amendment 2 closed 2026-08-16:
transcript capture is re-homed onto the readiness-driven reader, with the ordering proven by
direct observation and a per-mode failure policy tested against a genuinely unwritable
transcript.

**Adapter-spawn is still not ready to start, and this is the part worth reading.** The
amendment discharged *one named blocker*. Checking what else it needs — done by the
implementing slice rather than assumed — found at least two more, neither touched by this
work:

- nothing launches an AI CLI as an adapter;
- ~~`TerminalEnvironmentPolicy::ExplicitAllowlist` is rejected by the Linux runtime.~~
  **Corrected 2026-08-16: adapter-spawn does not need it.** Delivering the capability token
  is `command.env(APPROVAL_TOKEN_ENV_VAR, token)` — *setting* a value Tekstide generated.
  `ExplicitAllowlist(Vec<String>)` is a list of **names with no values**, which can only mean
  *inheriting* variables from Tekstide's own environment. The type is decisive: a generated
  token's value cannot be expressed as a name in a `Vec<String>`, so that variant is
  structurally incapable of delivering it. The runtime already does `.env_clear()` plus five
  fixed `.env(...)` calls (`launch.rs:482-487`); the token is a sixth, inheriting nothing.

  **The two questions separate, and only one belongs to adapter-spawn:**

  - *May the runtime set one additional variable to a value Tekstide itself generated?* —
    what adapter-spawn needs. A real question (a token in a child's environment is readable
    by that child and anything it spawns, which constrains scoping and lifetime), but not a
    tradeoff against usefulness, and it touches no inheritance.
  - *May a child inherit named variables from Tekstide's environment?* — the genuine
    security-versus-usefulness question, sharpened by RFC-004's redaction policy having no
    implemented pattern set. **Adapter-spawn does not force it**, and it can stay rejected
    until something actually wants inherited environment.

  Recorded because this entry previously asserted the boundary change was required, which
  would have put a decision to the owner that the design does not need — and would have
  invited weakening an allowlist boundary for a reason that turns out not to apply.

So adapter-spawn needs **scoping** before implementation — architect work, and the next
thing on the critical path. Until it lands, RFC-021's command approval, RFC-024's diff
content, RFC-011 Amendment 1's transcript reader, and RFC-020's two surfaces all remain
correct, reviewed and unreachable.

**The distinction this records**: "my prerequisite is discharged" and "the thing it blocked
is ready" are different claims. Conflating them is exactly how RFC-020's surfaces came to be
scheduled as `0.8.0`'s spine.

**Next release is `0.9.0`, not `0.8.1`** — `TranscriptWriterConfig` gained a public `mode`
field, a breaking change to `tekstide-core`. Field additions are how this gets missed, since
the reflex is to look for removals.

### RFC-022 closed, 2026-08-17 — what M11 now has, and what it does not

RFC-022 is implemented and closed. The adapter-spawn pathway, token delivery, the approval
dialog, the arrival model and the `ApprovalHistory` surface are all built and proven end to
end against production code.

**It does not make command approval reachable by a real user**, and the RFC says so in its own
Status. No shipping AI CLI speaks RFC-021's protocol, so `Managed` can only ever be exercised
by the reference adapter — a test artifact. The pathway is proven; the ecosystem does not
exist. Anyone reading "command approval shipped" into this is reading more than the record
says.

**What it unblocks — corrected 2026-08-17, hours after being written wrong.** The sentence
here originally read that RFC-020's two surfaces become reachable because "a real `AgentRun`
can now be created." **Both halves are false, and checking took one grep:**

- **`add_detected_generated_change_set` still has zero production callers.** Nothing runs
  change detection, so no `ChangeSet` can exist and RFC-020's change-review surface is
  **still blocked**, exactly as response 200 found it. **Superseded 2026-08-18 by the
  `change-detection-wiring` handoff** (Slices A–D, `9d55cb8`/`8f0abff`): it has a real
  production caller now. A real agent run captures a filesystem baseline before its process
  spawns, and on that run's real terminal exit a real `ChangeSet` is created, strongly
  associated with the run, listing the files it actually changed — proven end to end from a
  real key press. **RFC-020's surfaces still render nothing**: this produces their input, so
  the change-review surface is **buildable, not reachable**, and that distinction is the
  whole point rather than a hedge. Disclosed limitations travel with it — exit is the only
  completion trigger, so a long interactive session reports nothing until it ends; the
  baseline is in-memory, so it does not survive the application; and `.git/`, `target/` and
  `node_modules/` are excluded from detection by design.
- **No `AgentRun` can be launched by a real user either.** `Ctrl+Alt+A` refuses with
  `WorkspaceDiscoveryBlocked` for every project, because the Claude Code profile honestly
  declares `MayDiscoverWorkspaceFiles` and every project is permanently `Restricted` —
  `grant_project_trust` still has **zero production callers** (PR-022-D's finding,
  re-verified). **Superseded 2026-08-17 by RFC-032**, authored in direct response to this
  bullet: trust is now grantable through `Ctrl+Alt+U` → `TrustSettings` → the confirmation
  dialog, so this gate is passable and an agent run using such a profile launches for real
  in a trusted project. The bullet is kept rather than deleted because the correction
  history above is the point. *(Its original reason — that the other half, no `ChangeSet`,
  still stood — was itself superseded on 2026-08-18; see the bullet above. Both halves of
  the paragraph this note corrects are now false, each fixed by the RFC or handoff its own
  correction prompted, which is the delivery plan working as intended rather than a third
  error.)*
  **Noted at RFC-032's own Final Acceptance**: this paragraph sat false for the length of
  RFC-032's implementation, inside the very passage complaining that reachability claims
  get written without checking. The closeout that fixed the underlying gap updated the RFC,
  `rfcs/README.md`, the handoff pack and `future-work.md` — and not this file. **Updating
  `delivery-plan.md` belongs in the closeout gate**, alongside those four.

**So RFC-022 discharged a prerequisite without making anything user-reachable.** What it
genuinely unblocked is narrower and worth stating exactly: the *machinery* for an `AgentRun`
exists and is proven, so RFC-020's report surface has a code path that can produce inputs —
once something can grant trust.

This is the fourth time in this project that a claim about what is reachable was written
without checking it against the code, and the third by this document's own author. The check
is always cheap; the habit of writing the optimistic reading first is the defect.

**Still open, and recorded rather than carried silently:**

- **The active-project-change promotion trigger**, blocked on project-switching existing
  anywhere in the GUI — `switch_active_project` still has no production caller.
- **The reachability audit** (`future-work.md`): seven instances of a correct, reviewed
  capability in `tekstide-core` with no route from `crates/tekstide`. RFC-022 alone found
  four, and building the first reader of one of them surfaced **two real shipped defects**.
  That is the strongest evidence yet that the audit is worth scheduling rather than
  discovering the eighth the same way. **Run 2026-08-17** (`handoffs/reachability-audit.md`):
  **104 of 132 candidates dormant**, of which 30 are compiler-plausible true orphans — a
  floor, not a count, since the remaining 74 call chains were not traced to their roots. Its
  two priority items are discharged (terminal `resize`; trust granting, as RFC-032). The rest
  stands open in `future-work.md`.
- **RFC-021's protocol has no client surface**, and a rejected adapter cannot tell why. Both
  become real the moment anyone writes a genuine adapter, which is what RFC-022 was building
  toward.
