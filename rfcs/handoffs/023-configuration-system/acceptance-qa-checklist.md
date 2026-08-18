---
title: "RFC-023: Configuration System - Acceptance / QA Checklist"
rfc: "RFC-023"
rfc_file: "../../proposed/023-configuration-system.md"
status: "Ready for implementation — RFC-023 accepted 2026-08-18, scoped 2026-08-19"
target_milestone: "M12"
source_rfc_status: "Proposed"
created: "2026-07-28"
updated: "2026-07-28"
---

# RFC-023 Acceptance / QA Checklist

**A checked box means evidence exists.** A requirement that could not be met stays unchecked with a stated reason in `qa-evidence.md`.

## Path and Format Checklist

- [ ] Platform paths resolved per `REQ-CONFIG-002` (Linux/macOS/Windows).
- [ ] `XDG_CONFIG_HOME` honored on Linux with `~/.config` fallback.
- [ ] Path canonicalized; symlink redirection outside the configuration root rejected.
- [ ] Format is TOML; parsing cannot execute code.
- [ ] Missing file yields working defaults, not an error.

## Model Checklist

- [ ] Compiled defaults are total; no `Option` handling downstream.
- [ ] All `REQ-CONFIG-007` sections covered: theme, fonts, keybindings, scrollback, profiles, resource limits.
- [ ] Keybindings configurable (`REQ-CONFIG-006`).

## Atomic Validation Checklist

- [ ] Pipeline is parse → validate whole → construct → swap.
- [ ] **Invalid file leaves every previously active setting unchanged.**
- [ ] No partial application at any stage.
- [ ] Unknown keys warn.
- [ ] Unknown values for known keys error.
- [ ] Diagnostics carry path, location, offending key.
- [ ] Diagnostics contain no file contents.
- [ ] Diagnostics contain no secret-shaped values.
- [ ] Invalid file at first start: defaults apply and Tekstide starts.

## Security-Sensitive Settings Checklist

- [ ] Classification covers Restricted Mode defaults, approval policy, environment policy, profile definitions, transcript retention, audit location, plugin restrictions.
- [ ] Security-sensitive settings never hot-reload without confirmation.
- [ ] Security-sensitive changes produce audit events.
- [ ] Safe settings (theme, fonts, valid keybindings, new-session scrollback, new-task limits) hot-reload.

## Workspace Configuration Checklist

- [ ] `RestrictedModeFeature::WorkspaceConfigLoading` added; `ALL` and exhaustive matches updated.
- [ ] Workspace configuration blocked in Restricted Mode.
- [ ] Block is surfaced like other Restricted Mode blocks.
- [ ] **Workspace configuration cannot set any security-sensitive setting at any trust level.**

## Profile Bypass Checklist — write these tests first

- [ ] Config profile with a project-root executable → rejected at launch.
- [ ] Config profile with a wrapper script inside the project root → rejected.
- [ ] Config profile with a symlink resolving into the project root → rejected.
- [ ] Config profile relying on a project-local `PATH` entry → rejected.
- [ ] Config file located inside a project root → its profiles treated as workspace-local.
- [ ] **RFC-010 validation code reused unmodified, not reimplemented.**
- [ ] `Managed` declared in configuration does not confer `Managed`.
- [ ] Adding or editing a profile is audited.

## Reload Checklist

- [ ] Explicit reload entry point exists.
- [ ] Designed so the M13 watcher can call the same path without policy change.
- [ ] Failed reload changes nothing and reports diagnostics.
- [ ] Automatic file-change reload **not** implemented; deferral to M13 recorded.

## Audit Checklist

- [ ] Events conform to the frozen `sensitive_config_changed` family; schema unamended.
- [ ] `config_policy_increase` used for changes that **increase permitted capability**, with an operation id.
- [ ] `config_policy_reduce` used for changes that **reduce permitted capability**, without an operation id.
- [ ] `reason_code` always `policy_changed`.
- [ ] Written via `AuditCoordinator`, not directly to the store.
- [ ] **Sentinel test: no configuration values in the durable store.**

## Evidence Required

- [ ] Commit/PR list.
- [ ] Gate command output.
- [ ] Atomicity test results.
- [ ] Profile bypass test results — all four cases.
- [ ] Audit conformance and sentinel-privacy results.
- [ ] Known limitations.
- [ ] Answers to the RFC's open questions.

## Final Acceptance Decision

- [ ] Accepted.
- [ ] Accepted with required follow-up.
- [ ] Requires re-review after changes.
- [ ] Blocked — configuration can bypass RFC-010 provenance.

Reviewer notes:

```text
Pending implementation.
```
