---
title: "RFC-023: Configuration System - Task Breakdown and PR Plan"
rfc: "RFC-023"
rfc_file: "../../proposed/023-configuration-system.md"
target_milestone: "M12"
created: "2026-07-28"
updated: "2026-07-28"
---

# RFC-023 Task Breakdown and PR Plan

Six slices, all headless. May run in parallel with RFC-014 and RFC-021.

## PR-023-A — Design and handoff acceptance

Maintainer sign-off on format, precedence, the security-sensitive set, the explicit-reload decision, and the `config_policy_increase`/`reduce` semantics.

## PR-023-B — Paths, format, typed model, defaults

Scope:

- Platform path resolution (Linux/macOS/Windows) with canonicalization and symlink rejection, following `audit/path.rs` and `transcript/path.rs`.
- Typed configuration model covering all `REQ-CONFIG-007` sections.
- Compiled defaults, total — no `Option` handling downstream.

Review gate:

- Missing file yields working defaults, not an error.
- Path validation rejects a symlinked configuration directory escaping the configuration root.
- Defaults are genuinely total.

## PR-023-C — Atomic load, validation, diagnostics

Scope:

- Parse → validate whole document → construct → swap.
- Unknown keys warn; unknown values for known keys error.
- Bounded, content-free diagnostics.
- Invalid file at first start: defaults apply, Tekstide starts.

Review gate:

- **Atomicity proven** — an invalid file after a valid one leaves every setting unchanged.
- No partial application at any pipeline stage.
- Diagnostics contain no file contents and no secret-shaped values.

Reviewer focus: I will feed a file that is valid in its first half and invalid in its second, then assert nothing from the first half took effect.

## PR-023-D — Security-sensitive classification, reload, audit

Scope:

- Classification of the §5 security-sensitive set.
- Explicit reload entry point, designed for the M13 watcher to call later.
- `sensitive_config_changed` producer via `AuditCoordinator`.
- `RestrictedModeFeature::WorkspaceConfigLoading` added to the vocabulary.

Review gate:

- Security-sensitive settings do not apply on reload without confirmation.
- Audit conforms to the frozen schema; increase/reduce direction correct per the pinned semantics.
- **Sentinel test: no configuration values in the durable audit store.**
- Blocked-feature vocabulary complete and surfaced.

## PR-023-E — AI CLI profiles from configuration

**The highest-risk slice.** Write the bypass tests first.

Scope:

- Configuration-defined profiles as `AiCliProfileSource::UserGlobal`.
- Routing through *existing, unmodified* RFC-010 launch validation.
- Config file inside a project root ⇒ profiles treated as workspace-local.
- Profile add/edit as a security-sensitive, audited change.

Review gate:

- Four bypass attempts rejected: project-root executable, wrapper inside the root, symlink resolving into the root, project-local `PATH`.
- RFC-010 validation code **unmodified** — configuration reuses it rather than reimplementing.
- `Managed` in configuration does not confer `Managed`.

If any bypass succeeds, **stop and escalate.** Configuration that can name an arbitrary executable while evading provenance checks is a critical defect, not a follow-up item.

## PR-023-F — Closeout evidence

Scope: checklist, QA evidence, known limitations, answers to the RFC's open questions, and an explicit statement that automatic reload awaits M13.

## Sequencing

B → C → D is strictly sequential. E needs B and C but not D. F needs all.

E may be deferred to a later release if schedule pressure demands — the rest of the configuration system is useful without user-defined profiles, and deferring is safer than rushing the slice that names executables.
