# RFC-023: Configuration System

Status: **Accepted by the human owner 2026-08-18.** Authored earlier and held while its
prerequisites landed; accepted alongside RFC-031/033/034/035/036 in one decision.
Target milestone: M12 (headless model may be implemented earlier — see Scheduling)
Date: 2026-07-28

Related baseline documents:

- `tekstide-requirements-v0.md`
- `tekstide-external-design-v0.md`
- `tekstide-security-threat-model-v0.md`
- [`ROADMAP.md`](../../ROADMAP.md), [`delivery-plan.md`](../delivery-plan.md)

Depends on:

- [RFC-004](../done/004-security-baseline-and-restricted-mode.md)
- [RFC-010](../done/010-agentrun-launch-model-and-ai-cli-profiles.md)
- [RFC-011](../done/011-transcript-retention-and-local-data-policy.md)
- [RFC-013](../done/013-durable-audit-store-and-local-data-policy.md)

Blocks:

- user-defined AI CLI profiles (currently code-defined only — an RFC-010 limitation);
- configurable keybindings, theme, fonts, scrollback, and resource limits;
- the `sensitive_config_changed` durable audit producer;
- the rendered configuration surface.

## Summary

RFC-023 defines Tekstide's configuration system: file format and location, load and precedence order, atomic validation, which settings may hot-reload and which may not, and how configuration changes reach the durable audit log.

Configuration is a security surface, not a convenience feature. It can weaken Restricted Mode, redirect AI CLI executables, and change environment policy. The design treats it accordingly: workspace-supplied configuration is untrusted, validation is all-or-nothing, and security-relevant changes are audited and never applied silently.

## Motivation

`REQ-CONFIG-001` through `007` require a human-readable configuration file with atomic validation, diagnostics, configurable keybindings, and configurable theme, fonts, scrollback, AI CLI profiles, and resource limits. None of it exists — there is no configuration module.

Three consequences are visible today:

- AI CLI profiles are code-defined only. RFC-010 records this as a limitation; users cannot define a profile at all.
- The `sensitive_config_changed` audit family has no producer, because there is no configuration to change.
- Nothing in `NFR-UX-004` (configurable fonts) or `REQ-CONFIG-006` (configurable keybindings) can be satisfied by the GUI milestones without this.

## Goals

- Define a single human-readable configuration format and its platform paths.
- Define load order and precedence, with workspace configuration untrusted by default.
- Guarantee atomic validation: configuration applies completely or not at all.
- Separate hot-reloadable settings from security-sensitive settings that require explicit confirmation.
- Move AI CLI profiles from code-defined to user-defined without weakening RFC-010's executable provenance rules.
- Produce the `sensitive_config_changed` audit events within the frozen RFC-013 schema.
- Fail closed on every parse, validation, and IO error.

## Non-Goals

- A graphical settings editor. `REQ-CONFIG` and external design §11.1 both accept text configuration as the primary interface; a read-mostly surface is M12 UI work.
- Automatic reload on file change. That requires file watching, which is M13 — see Hot Reload.
- Per-project configuration stored inside project roots as a trusted source. See Workspace Configuration.
- Plugin configuration. Plugins remain deferred.
- Configuration sync, profiles, or migration between machines.

## Format and Location

TOML, per `REQ-CONFIG-002` and the external design's worked example.

| Platform | Path |
| --- | --- |
| Linux / Unix | `$XDG_CONFIG_HOME/tekstide/config.toml`, falling back to `~/.config/tekstide/config.toml` |
| macOS | `~/Library/Application Support/tekstide/config.toml` |
| Windows | `%APPDATA%\tekstide\config.toml` |

Rules:

- A missing configuration file is **not an error**. Defaults apply and Tekstide starts normally.
- The configuration path is resolved and canonicalized with the same discipline RFC-011 and RFC-013 apply to state paths: absolute, no symlink redirection outside the configuration directory.
- Parsing must not execute code. TOML satisfies this; no format permitting evaluation may be introduced later without a threat-model amendment (threat model §8.15).

## Load Order and Precedence

```
built-in defaults  →  user global config  →  (workspace config, only if explicitly trusted)
```

Later sources override earlier ones per-key. Tekstide always has a complete, valid configuration because defaults are compiled in and total.

### Workspace configuration

A repository may contain `.tekstide/config.toml`. It is **untrusted**:

- **Blocked in Restricted Mode.** This requires a new `RestrictedModeFeature::WorkspaceConfigLoading` variant alongside the existing nine. Blocking is surfaced like any other Restricted Mode block.
- **Never loadable for security-sensitive settings, even when trusted.** Trusting a workspace permits project-local conveniences; it does not permit a cloned repository to alter approval policy, environment policy, or Restricted Mode defaults. That set is fixed by the Security-Sensitive Settings section and is not overridable from a workspace file at any trust level.
- Workspace configuration is a v1 *design allowance*, not required functionality. Implementing only defaults + user-global in the first slice is acceptable, provided the blocked-feature vocabulary and the precedence rule land with it.

## Atomic Validation

`REQ-CONFIG-003` requires invalid configuration to be rejected atomically; `REQ-CONFIG-004` requires diagnostics rather than silent unsafe fallback.

The pipeline is: **parse → validate whole document → construct typed configuration → swap in.** No partially applied state at any point.

On failure:

- The previously active configuration remains in force, complete and unchanged.
- A bounded diagnostic records file path, error location where available, and the offending key — never the file's full contents, and never a secret-shaped value.
- On first start with an invalid file, compiled defaults apply and the diagnostic surfaces prominently. Tekstide does not refuse to start; refusing would make a typo a denial of service.

Unknown keys are a **warning, not an error** — forward compatibility matters more than strictness for a config file users hand-edit. Unknown values for known keys are errors.

## Security-Sensitive Settings

These may never be applied silently, may never be hot-reloaded, and may never come from workspace configuration (`REQ-CONFIG-005`, threat model §8.15):

- Restricted Mode defaults and the blocked-feature policy.
- Command approval policy, once RFC-021 lands.
- AgentRun environment policy and any environment allowlist.
- AI CLI profile definitions — executable path, argv template, compatibility level.
- Transcript retention and purge policy.
- Audit store location and retention.
- Plugin restrictions, when plugins exist.
- **Workspace trust defaults (added 2026-08-19).** This list was written before RFC-032
  existed. See below — for this one, security-sensitive treatment is **not sufficient**.

Changing any of these requires explicit user confirmation and produces an audit event. A change that cannot be confirmed is not applied.

### `default_trust` is the one setting confirmation does not make safe

PR-023-B's typed model added `[projects].default_trust`, from the external design's worked
example. **Confirm-once-and-audit-the-change is weaker than what RFC-032 requires**, and the
gap is not a detail:

| RFC-032's granting design | what a `default_trust = "trusted"` setting gives instead |
| --- | --- |
| a confirmation dialog **per project** | one confirmation, when the setting changes |
| **two deliberate acts**, focus defaulting to Cancel | none, thereafter |
| bound to the project's **canonical path** | bound to nothing |
| a `TrustGrant` audit record **per grant** | one `sensitive_config_changed` record, ever |

So a setting that can express "trusted" is a **trust-granting mechanism that bypasses the
trust-granting design**, and RFC-023's own security-sensitive machinery does not close it,
because the machinery governs *changing the setting*, not the grants the setting then performs
silently forever.

**There is a concrete escalation path**, which is what makes this more than tidiness. An agent
run in a trusted project executes with the user's own permissions and can write
`~/.config/tekstide/config.toml` — user-global configuration is trusted by this RFC's own load
order and is not protected from the user's own processes. An agent that writes
`default_trust = "trusted"` has arranged for **every future project to be trusted at creation**,
which is precisely the state RFC-032 exists to make a deliberate act.

**Decision: `default_trust` may only ever express the more restricted state.** Either drop the
field, or validate it to a single accepted value (`"restricted"`) so it is inert by
construction and the vocabulary is reserved for a future design that actually reasons about
it. **Do not make it a two-valued setting governed by confirmation.** The same test applies to
any future setting: if flipping it would grant a capability that some other RFC requires a
deliberate per-use act for, confirmation-on-change is the wrong control.

## AI CLI Profiles From Configuration

This is the highest-risk part of the RFC: it lets a file define what executable Tekstide launches.

**RFC-010's validation is not relaxed.** A configuration-defined profile is an `AiCliProfileSource::UserGlobal` profile and passes through the identical launch gates — executable provenance classification, rejection of workspace-local executables and wrappers and symlink targets resolving into the project root, rejection of project-local `PATH` entries, and the implicit workspace-discovery gate.

Additional rules specific to configuration:

- The configuration file itself must live outside every open project root. A profile sourced from a file inside a project root is workspace-local regardless of its declared source.
- A profile whose executable resolves inside a project root is rejected at launch, exactly as today. Configuration is not a bypass.
- Profile definitions are security-sensitive: adding or editing one requires confirmation and audit.
- `Managed` in configuration remains subject to RFC-010 and RFC-021 — declaring it does not confer it.

## Hot Reload

External design §11.4 permits hot reload for safe settings: theme, font family and size, keybindings that validate, terminal scrollback for new sessions, resource limits for new tasks.

**Automatic reload on file change is deferred to M13**, because detecting the change requires the file watcher that M13 introduces. Building a bespoke watcher here would duplicate M13 and risk diverging debounce semantics.

RFC-023 therefore specifies **explicit reload only**: a command or API call re-reads and re-validates. When the watcher lands, automatic reload wires into the same path with no policy change.

Reload rules:

- Safe settings apply immediately.
- Security-sensitive settings do **not** apply on reload. They surface as pending changes requiring confirmation, or apply only to future sessions with a visible notification.
- A reload that fails validation changes nothing and reports diagnostics.

## Audit — Disambiguating the Frozen Vocabulary

RFC-013 froze the `sensitive_config_changed` family with an asymmetry:

- `config_policy_increase` — requires an `operation_id`, outcomes `authorized` → `applied`/`failed`.
- `config_policy_reduce` — no `operation_id`, outcome `applied` only.

**The names are ambiguous** and RFC-023 must pin them without amending the schema. The authorization asymmetry settles it: only one reading explains why one direction needs authorization and the other does not.

> **`config_policy_increase`** — a change that **increases the permitted capability surface**, weakening the security posture. Examples: disabling a Restricted Mode block, widening an environment allowlist, adding an AI CLI profile, raising a retention limit. Requires explicit authorization before it is applied.
>
> **`config_policy_reduce`** — a change that **reduces the permitted capability surface**, tightening the posture. Examples: re-enabling a Restricted Mode block, narrowing an allowlist, removing a profile. Applied directly; tightening never needs permission.

Record this mapping in the handoff and evidence, because the names alone will mislead a future reader.

Other constraints from the frozen schema: `reason_code` is always `policy_changed`; `terminal_id`, `agent_run_id`, `approval_id`, `subject_kind`, `risk_level`, and `adapter_profile_ref` are all absent; actor/source is `user`/`trusted_ui` or `app_policy`/`policy_engine`. `project_id` may be null, which suits global configuration.

**No configuration values appear in audit records.** Setting names and the change direction only — never values, never file contents, consistent with RFC-013 throughout.

## Data Model Impact

- Typed configuration structure with total defaults.
- Configuration path resolver following the RFC-011/RFC-013 path discipline.
- Loader and validator producing either a complete configuration or a bounded diagnostic.
- Security-sensitive setting classifier driving the increase/reduce audit distinction.
- `RestrictedModeFeature::WorkspaceConfigLoading`.
- Explicit reload entry point, watcher-ready.

## Implementation Plan

1. **PR-023-A** — design and handoff acceptance.
2. **PR-023-B** — path resolution, format, typed model, total defaults.
3. **PR-023-C** — atomic parse/validate/swap with bounded diagnostics and fail-closed behavior.
4. **PR-023-D** — security-sensitive classification, explicit reload, and the `sensitive_config_changed` producer.
5. **PR-023-E** — AI CLI profiles from configuration, reusing RFC-010 validation unchanged.
6. **PR-023-F** — closeout evidence.

All slices are headless.

## Test and Evidence Requirements

- Missing file yields working defaults, not an error.
- Invalid file leaves the previous configuration completely unchanged; no partial application.
- Unknown keys warn; unknown values for known keys error.
- Diagnostics contain no file contents and no secret-shaped values.
- Workspace configuration blocked in Restricted Mode, with the blocked feature surfaced.
- Workspace configuration cannot alter any security-sensitive setting at any trust level.
- **Configuration-defined profiles cannot bypass RFC-010 provenance** — regression tests for a config profile pointing at a project-root executable, a wrapper, a symlink resolving into the root, and a project-local `PATH` entry.
- Security-sensitive changes are not applied by reload without confirmation.
- Audit tests: correct family and action per direction, and **no configuration values in any durable record**.
- Path tests: symlinked configuration directory escaping the configuration root is rejected.

## Acceptance Criteria

- Configuration is human-readable, atomically validated, and total by default.
- Invalid configuration never partially applies and never silently weakens security.
- Workspace configuration is untrusted, blocked in Restricted Mode, and can never set security-sensitive values.
- Configuration-defined AI CLI profiles are subject to unchanged RFC-010 validation.
- Explicit reload works; automatic reload is deferred to M13 without a policy change.
- `sensitive_config_changed` events conform to the frozen schema, with the increase/reduce semantics pinned above.
- No configuration values reach durable audit.

## Risks

- **Configuration as a bypass.** A config file that names an executable is a launch vector. Mitigation: unchanged RFC-010 validation, plus tests that specifically attempt the bypass.
- **Vocabulary misreading.** `config_policy_increase` could plausibly be read as "increase strictness." Mitigation: the definition above, restated in the handoff and evidence.
- **Silent weakening through reload.** Mitigation: security-sensitive settings never hot-reload.
- **Scope creep into a settings UI.** Mitigation: RFC-023 is headless; the surface is M12 UI work.

## Open Questions

1. Should workspace configuration ship in v1 at all, or should the first implementation support only defaults + user-global while reserving the vocabulary?
2. Should an invalid configuration file at first start be more prominent than a notification — a blocking dialog, once dialogs exist?
3. Should configuration-defined profiles require a one-time confirmation on first use, in addition to being audited when added?

## Scheduling

All slices are headless and have no GUI dependency. Recommended to start immediately alongside RFC-021 and the RFC-014 spike. Configuration unblocks user-defined AI CLI profiles — a limitation users encounter the moment they try to use the product — and is a prerequisite for one of the eight unwired audit producers.

---

## Scoping, 2026-08-19 — five consumers accumulated while this RFC waited

Scoped at the owner's request before handover. This RFC was authored early and held while its
prerequisites landed. In the meantime **five places in the shipped code named RFC-023 as the
thing that would supply their settings**, and this RFC's §Goals names none of them:

| what points here | where | what it expects |
| --- | --- | --- |
| **Keybindings** | `navigation.rs` (×3), `shell.rs` | Every `Configurable` action has a `None` binding and is **dead** "until RFC-023 exists" |
| **Theme values** | `theme.rs` (×2) | *"RFC-023 will supply these values from configuration"* — every colour and font size |
| **Locale preference** | `i18n.rs` (×2) | `LocalePreference::configured` is permanently `None`; the field exists so the signature need not change when RFC-023 arrives |
| **Resource limits** | reachability audit, priority 3 | `set_resource_limits` has no caller, so every tuned limit is fixed at its default forever |
| **Transcript capture default** | RFC-033 | Defaults are named as this RFC's, per-run opt-out as RFC-033's |

Plus one recorded in this RFC's own handoff pack: **the WCAG contrast gate does not survive
configurability** — it validates one compiled palette at build time, and a user-supplied one
would reach the renderer unchecked.

### The scoping decision, and it is the whole point of this note

**This RFC delivers the configuration *mechanism*, not every setting that wants to use it.**
Its Goals are paths and format, load order and precedence, atomic validation,
security-sensitive classification, hot-reload separation, AI CLI profiles, and the
`sensitive_config_changed` producer. That is already a large RFC.

If it also has to deliver keybindings, theme values, locale, resource limits and capture
defaults, it becomes "make everything configurable" and **it will not land**. Worse, each of
those five carries its own design question — a keybinding needs collision policy and a
rebinding UI story; a user-supplied palette needs the contrast gate promoted out of
`#[cfg(test)]`; a locale change needs runtime switching, which RFC-016 explicitly deferred.

**So: name each of the five as out of scope, in this RFC, with a stated owner.** The five code
comments will then be pointing at something true — a mechanism they can build on — rather than
at a promise this RFC never made. **Do not leave them pointing here silently**; that is how
`OpenApprovalHistory` sat unreachable for a release.

### The three Open Questions, answered

**OQ1 — does workspace configuration ship in v1?** **No.** Workspace configuration is a file
inside a project root, which is exactly the untrusted, attacker-influenceable surface RFC-032's
whole trust model exists to gate. Shipping the *vocabulary* while supporting only
defaults + user-global is the right first step, and it means a project cannot configure the
application that opens it until trust is a prerequisite that has been designed for it.

**OQ2 — should an invalid config file be more prominent than a notification?** The question
was written when no dialogs existed. **They exist now** (paste, external-change, trust,
approval), so the option is live. **Recommend a notification, not a blocking dialog**, for the
reason this RFC already states as a goal: *"an invalid file must not become a denial of
service."* A modal at startup that a user cannot dismiss without valid configuration is exactly
that.

**OQ3 — should configuration-defined profiles need a one-time confirmation on first use?**
**Yes.** A config-defined AI CLI profile is a reviewed launch contract supplied by a file; RFC-010's
provenance validation still applies, but provenance is not intent. This is the same asymmetry
RFC-032 chose for trust: the dangerous direction gets a deliberate act.

### Stale text in this document, corrected rather than left

- §Scheduling says *"start immediately alongside RFC-021 and the RFC-014 spike."* Both closed
  long ago; RFC-023 is now M12 and follows RFC-031.
- §Scheduling says configuration *"is a prerequisite for one of the eight unwired audit
  producers."* **Three** remain unwired as of 2026-08-19, and `sensitive_config_changed` is one
  of them — the statement's substance holds, its arithmetic does not.
