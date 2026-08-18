---
title: "RFC-023: Configuration System - Acceptance / QA Checklist"
rfc: "RFC-023"
rfc_file: "../../proposed/023-configuration-system.md"
status: "PR-023-B implemented 2026-08-19, awaiting review; PR-023-C onward pending"
target_milestone: "M12"
source_rfc_status: "Proposed"
created: "2026-07-28"
updated: "2026-07-28"
---

# RFC-023 Acceptance / QA Checklist

**A checked box means evidence exists.** A requirement that could not be met stays unchecked with a stated reason in `qa-evidence.md`.

## Path and Format Checklist

- [x] Platform paths resolved per `REQ-CONFIG-002` (Linux/macOS/Windows).
      (`ConfigPathProvider::linux_from_env`/`macos_from_env`/`windows_from_env`, `config/path.rs`,
      2026-08-19)
- [x] `XDG_CONFIG_HOME` honored on Linux with `~/.config` fallback.
      (`linux_from_env_prefers_xdg_config_home_over_home`,
      `linux_from_env_falls_back_to_home_dot_config`)
- [x] Path canonicalized; symlink redirection outside the configuration root rejected.
      (`a_symlinked_config_directory_escaping_the_configuration_root_is_rejected`, ablated for
      real — see `qa-evidence.md`'s PR-023-B section)
- [ ] Format is TOML; parsing cannot execute code. **PR-023-C** — no parser exists yet; B builds
      the path and typed-model layers only.
- [ ] Missing file yields working defaults, not an error. **Partially covered by B, completed by
      PR-023-C.** The path layer resolves cleanly with nothing on disk
      (`resolving_with_nothing_on_disk_yet_succeeds`) and `ConfigurationDocument::default()` is
      total, but there is no loader yet to connect "file absent" to "defaults applied" as one
      observed behavior — that connection is C's atomic load pipeline.

## Model Checklist

- [x] Compiled defaults are total; no `Option` handling downstream.
      (`every_section_default_is_the_documented_value`,
      `transcript_retention_default_reuses_the_real_compiled_constant`, `config/tests/model.rs`)
- [x] All `REQ-CONFIG-007` sections covered: theme, fonts, keybindings, scrollback, profiles, resource limits.
      (`ConfigurationDocument`'s eight fields, `config/model.rs`)
- [ ] Keybindings configurable (`REQ-CONFIG-006`). **Typed storage exists**
      (`KeybindingSettings.overrides: BTreeMap<String, String>`, default empty) **but nothing
      reads it** — no consumer wires it into `input.rs`'s real binding policy. This pack's own
      Scoping section already names keybindings as an out-of-scope, owner-pending consumer;
      unchecked here for the same reason, not a gap introduced by B.

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
