# RFC-023: Configuration System - QA Evidence

Status: Proposed — implementation pending
Date opened: 2026-07-28
Date accepted: Pending

## Scope

RFC-023 defines the configuration system: format, paths, precedence, atomic validation, security-sensitive settings, explicit reload, and the `sensitive_config_changed` audit producer. Headless.

Evidence in this file must not be used to claim a graphical settings editor, automatic reload on file change, configuration sync or migration, plugin configuration, or any relaxation of RFC-010 executable provenance — unless later reviewed implementation explicitly supports that claim.

## Design Review

Pending PR-023-A acceptance.

## Vocabulary Note — record this before implementing

RFC-013 froze two action kinds whose names mislead:

- **`config_policy_increase`** — increases the **permitted capability surface**, weakening the security posture. Requires an operation id and explicit authorization.
- **`config_policy_reduce`** — reduces the permitted capability surface, tightening the posture. No operation id; applied directly.

The names read the opposite way to most people. The authorization asymmetry in the frozen schema is what settles the meaning. Restated here because a future reader will meet the names before the RFC.

## Implementation Evidence

### PR-023-B — Paths, format, typed model, defaults

Pending implementation.

### PR-023-C — Atomic load, validation, diagnostics

Pending implementation.

### PR-023-D — Security-sensitive classification, reload, audit

Pending implementation.

### PR-023-E — AI CLI profiles from configuration

Pending implementation.

**Reminder for whoever takes this slice:** write the four bypass tests before the feature. Configuration that names an executable is the highest-risk surface in this RFC.

### PR-023-F — Closeout evidence

Pending implementation.

## Known Limitations

- **Automatic reload on file change is not implemented.** Explicit reload only; the file watcher arrives in M13 and will call the same entry point without a policy change.
- Security-sensitive settings do not hot-reload by design; they require confirmation or apply to future sessions only.
- Workspace configuration may be reserved rather than implemented in the first slice, provided the blocked-feature vocabulary and precedence rule land with it.
- Durable audit records the setting name and change direction only. It cannot answer "what value was it changed to" — consistent with RFC-013's exclusion of values from audit throughout.
