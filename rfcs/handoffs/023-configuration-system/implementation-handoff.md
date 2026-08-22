---
title: "RFC-023: Configuration System - Implementation Handoff"
rfc: "RFC-023"
rfc_file: "../../done/023-configuration-system.md"
target_milestone: "M12"
source_rfc_status: "Implemented and closed 2026-08-22 — RFC-023 is in rfcs/done/"
created: "2026-07-28"
updated: "2026-07-28"
---

# RFC-023 Implementation Handoff

Covers PR-023-B through PR-023-F. All headless.

## 1. Where this code lives

New module `crates/tekstide-core/src/config/`:

```
config.rs             module root and re-exports
config/path.rs        platform path resolution and validation
config/model.rs       typed configuration with total defaults
config/load.rs        parse, validate, construct, swap
config/sensitive.rs   security-sensitive classification and audit mapping
config/tests/
```

`config/path.rs` should follow the same shape as `audit/path.rs` and `transcript/path.rs` — those are the project's reference implementations for canonicalized, symlink-rejecting path policy. Reuse the reasoning; do not invent a third approach.

## 2. Typed model with total defaults

Every setting has a compiled default. The configuration type is always fully populated, so no code downstream handles "unset."

This matters more than it looks: it means a missing file, an empty file, and a file setting one key all produce the same shape, and no caller needs `Option` handling for configuration.

Sections to cover, from `REQ-CONFIG-007` and the external design example: `[core]`, `[ui]`, `[keybindings]`, `[terminal]`, `[projects]`, `[agent]` including profiles, `[security]`, `[resources]`.

## 3. Validation pipeline — atomic or nothing

```
read bytes → parse TOML → validate whole document → construct typed config → swap in
```

Failure at any stage leaves the **previously active configuration completely unchanged**. There is no stage at which a half-applied configuration exists.

Rules:

- **Unknown keys warn; they do not fail.** Users hand-edit this file, and forward compatibility matters more than strictness.
- **Unknown values for known keys are errors.** A typo in an enum must not silently fall back.
- Diagnostics carry file path, error location where available, and the offending key — **never file contents, never a value that could be secret-shaped.** Follow the bounded-diagnostic discipline from `audit/` throughout.
- On first start with an invalid file: defaults apply, diagnostic surfaces, Tekstide starts. Refusing to start turns a typo into a denial of service.

## 4. The profile bypass — the trap in this RFC

Configuration lets a file name the executable Tekstide launches. This is where a real vulnerability is most likely to enter.

**RFC-010's validation is not relaxed, reimplemented, or shortcut.** A config-defined profile is an `AiCliProfileSource::UserGlobal` profile and goes through the *existing* launch validation unchanged: executable provenance classification, workspace-local executable rejection, wrapper and shim and symlink-target rejection, project-local `PATH` rejection, and the implicit workspace-discovery gate.

Additional rules specific to configuration:

- If the configuration file itself lives inside an open project root, profiles from it are **workspace-local** regardless of declared source.
- A profile whose executable resolves inside a project root is rejected at launch — configuration is not a bypass.
- Adding or editing a profile is security-sensitive: confirmation plus audit.
- Declaring `Managed` in configuration does not confer it. RFC-010 and RFC-021 still gate that.

**Write the bypass tests before the feature.** Four cases minimum: config profile pointing at a project-root executable; at a wrapper script inside the root; at a symlink whose target resolves into the root; and with a project-local `PATH` entry. All four must be rejected at launch by the existing RFC-010 code, and your tests must prove the path reaches it.

## 5. Security-sensitive settings

These never hot-reload, never come from workspace configuration at any trust level, and always produce an audit event:

- Restricted Mode defaults and blocked-feature policy
- Command approval policy (once RFC-021 lands)
- AgentRun environment policy and allowlists
- AI CLI profile definitions
- Transcript retention and purge policy
- Audit store location and retention
- Plugin restrictions (when plugins exist)

Everything else — theme, fonts, keybindings that validate, scrollback for new sessions, resource limits for new tasks — may hot-reload.

## 6. Workspace configuration

Add `RestrictedModeFeature::WorkspaceConfigLoading` to the existing nine variants in `security.rs`, and update `ALL` plus any exhaustive matches.

Workspace configuration is blocked in Restricted Mode and, even when trusted, may never set anything from §5. Trusting a workspace permits project-local conveniences; it does not let a cloned repository alter approval or environment policy.

**Shipping only defaults + user-global in the first slice is acceptable**, provided the blocked-feature vocabulary and precedence rule land with it. Reserve the behavior; implement when needed.

## 7. Hot reload — explicit only

Automatic reload on file change needs a file watcher, which is **M13**. Building a bespoke watcher here would duplicate that work and risk diverging debounce semantics.

Implement an **explicit reload entry point** — a command or API call that re-reads and re-validates. Design it so the M13 watcher can call the same path with no policy change.

On reload: safe settings apply immediately; security-sensitive settings surface as pending changes requiring confirmation, or apply to future sessions only with a visible notification; validation failure changes nothing.

## 8. Audit mapping

Frozen `sensitive_config_changed` family. Read the CHECK constraint in `audit/schema.rs` first.

| Direction | action_kind | operation_id | outcome |
| --- | --- | --- | --- |
| Weakens posture (increases permitted capability) | `config_policy_increase` | required | `authorized` → `applied`/`failed` |
| Tightens posture (reduces permitted capability) | `config_policy_reduce` | none | `applied` |

`reason_code` is always `policy_changed`. `terminal_id`, `agent_run_id`, `approval_id`, `subject_kind`, `risk_level`, `adapter_profile_ref` all absent. `project_id` may be null — configuration is global.

**No configuration values in audit records.** Setting names and direction only. Write a sentinel test: put a distinctive string in a config value, change it, and assert the string never appears in the audit database. Model it on RFC-012's sentinel privacy test.

Write via `AuditCoordinator` (`audit/integration.rs`), not directly to the store.

## 9. What you must not build

- A graphical settings editor — M12 UI work, not this RFC.
- A file watcher — M13.
- Configuration sync, profiles, or machine migration.
- Any format permitting code execution during parsing.
- Plugin configuration.

## 10. What I will probe at review

- **Profile bypass:** config profiles pointing at project-root executables, wrappers, symlinks resolving into the root, and project-local `PATH` — all four must be rejected.
- **Atomicity:** an invalid file after a valid one, confirming zero settings changed.
- **Diagnostic leakage:** grepping diagnostics for file contents and secret-shaped values.
- **Audit leakage:** sentinel value in a config setting, then grepping the database.
- **Workspace escalation:** a workspace config attempting to set a §5 setting while trusted.

## 11. Gates

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets --all-features
git diff --check
```

Product code — thorough tests expected, in `src/config/tests/` per the project convention.
