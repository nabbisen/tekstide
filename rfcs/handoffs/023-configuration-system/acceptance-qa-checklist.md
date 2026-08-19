---
title: "RFC-023: Configuration System - Acceptance / QA Checklist"
rfc: "RFC-023"
rfc_file: "../../proposed/023-configuration-system.md"
status: "PR-023-B and PR-023-C accepted 2026-08-19; PR-023-D in progress (classification accepted, reload gating landed, awaiting review)"
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
- [x] Format is TOML; parsing cannot execute code. (`toml` crate, table/value AST only — no
      execution hook exists in the format or the crate; `parse_and_validate`, `config/load.rs`,
      2026-08-19)
- [x] Missing file yields working defaults, not an error.
      (`store_load_with_no_file_present_yields_defaults_and_no_diagnostic`, `config/tests/load.rs`)

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
- [x] **`default_trust` cannot express "trusted."** Response 266 / RFC-023's own
      §Security-Sensitive Settings correction (2026-08-19): a two-valued `default_trust` would be
      a trust-granting mechanism bypassing RFC-032's per-project, two-deliberate-act design, and
      security-sensitive classification (confirm-on-change) does not close that gap. Fixed:
      `ProjectSettings.default_trust` is now `RestrictedDefaultTrust`, a zero-field unit struct
      with exactly one possible value — inert by construction, not by runtime validation.
      (`default_trust_has_exactly_one_possible_value`, `config/tests/model.rs`)

## Atomic Validation Checklist

- [x] Pipeline is parse → validate whole → construct → swap.
      (`parse_and_validate`, `config/load.rs` — parse to `toml::Table`, validate+construct each
      of the 8 sections, `ConfigStore::reload` swaps `self.current` in exactly one place, only
      after the whole document validates)
- [x] **Invalid file leaves every previously active setting unchanged.**
      (`reload_with_a_file_valid_in_its_first_half_and_invalid_in_its_second_changes_nothing` —
      the review's own planned test, verbatim; ablated for real, see `qa-evidence.md`)
- [x] No partial application at any stage. Structural, not merely tested: `parse_and_validate`
      mutates no shared state, and `ConfigStore` has exactly one assignment to `self.current`,
      gated on `parse_and_validate` already having returned a complete document.
- [x] Unknown keys warn.
      (`an_unrecognized_top_level_section_warns_and_does_not_fail`,
      `an_unrecognized_key_inside_a_known_section_warns_and_does_not_fail`,
      `an_unrecognized_key_inside_a_profile_table_warns_and_does_not_fail`)
- [x] Unknown values for known keys error.
      (`an_unknown_value_for_a_known_key_is_an_error_naming_the_key`,
      `default_trust_set_to_trusted_in_the_file_is_an_explicit_named_error`)
- [x] Diagnostics carry path, location, offending key.
      (`store_load_with_an_invalid_file_at_first_start_yields_defaults_with_a_diagnostic` asserts
      `path`; `malformed_toml_syntax_is_a_parse_error_with_a_location_but_no_content` asserts
      `location`; every diagnostic carries `key`)
- [x] Diagnostics contain no file contents. `message`/`location`/`path` are all inert by
      construction; `key` is bounded (response 268/269: length capped to 128 characters via
      `AuditReference`'s own cap; character shape bounded by the **reviewed**
      `text_safety::escape_untrusted_chars` — not a second, ad-hoc escaping primitive — so control
      and bidi-override characters become a visible `<U+XXXX>` marker while every other character,
      including legitimate non-Latin scripts, passes through unchanged) whenever it carries text
      the file itself supplied (an unknown key, or a profile name). Truncation happens on the raw
      input *before* escaping, so a hostile character at the length boundary cannot be split into
      a mangled marker fragment. Ablated for real, three separate properties: (1) bypassed
      `bound_key_segment` entirely — all hostile/overlong-text tests failed; (2) bypassed only the
      escaping step (kept truncation) — the two marker-asserting tests failed while the
      legitimate-non-Latin-text test still passed, showing that test is not vacuously satisfied by
      either implementation; (3) swapped the ordering to escape-then-truncate — the boundary test
      failed with the literal split-marker fragment (`...a<…`) the reviewer described. Restored
      each time, confirmed green.
      (`an_overlong_unknown_key_is_truncated_in_the_warning`,
      `a_bidi_override_or_control_character_in_an_unknown_key_is_neutralized`,
      `a_hostile_profile_name_is_bounded_in_diagnostics_but_not_in_the_stored_profile`,
      `legitimate_non_latin_text_in_an_unknown_key_survives_unescaped`,
      `a_hostile_character_at_the_truncation_boundary_is_never_split`)
- [x] Diagnostics contain no secret-shaped values. `message` is `&'static str`, inert by
      construction — no code path can put runtime content into it; re-verified directly with a
      real secret-shaped sentinel,
      `a_secret_shaped_rejected_value_never_reaches_the_diagnostic`.
- [x] Invalid file at first start: defaults apply and Tekstide starts.
      (`store_load_with_an_invalid_file_at_first_start_yields_defaults_with_a_diagnostic`)

## Security-Sensitive Settings Checklist

- [x] Classification covers Restricted Mode defaults, approval policy, environment policy, profile
      definitions, transcript retention, audit location, plugin restrictions. Seven fields
      classified security-sensitive and reload-gated (`SecuritySensitiveField`, `config/sensitive.rs`):
      the three `restricted_mode_blocks_*` fields, `redact_secret_like_environment_names`,
      `agent.default_environment_policy`, `agent.transcript_retention_days`, `agent.profiles`.
      Approval policy has no remaining configurable surface to classify — its only candidate
      (`require_approval_for_adapter_destructive_commands`) is inert, not merely gated. Audit
      location and plugin restrictions have no corresponding field in this model at all (no
      `[audit]` section exists; plugins don't exist) — nothing to classify, not an omission.
- [x] Security-sensitive settings never hot-reload without confirmation.
      (`reload_applies_a_safe_change_but_holds_a_security_sensitive_one_pending`, ablated for
      real — see Reload Checklist below)
- [ ] Security-sensitive changes produce audit events. Not yet — no producer exists; the next
      piece of this slice.
- [x] Safe settings (theme, fonts, valid keybindings, new-session scrollback, new-task limits) hot-reload.
      (`applying_safe_fields_takes_every_safe_field_from_the_candidate`, and the same integration
      test above proves a safe field applies in the same reload a security-sensitive one is held
      back from)

**Correction landed 2026-08-19, ahead of the rest of this checklist**: response 270 named two
settings to re-examine against RFC-023's own general test (*"if flipping a setting would grant a
capability that another RFC requires a deliberate per-use act for, confirmation-on-change is the
wrong control"*) rather than reflexively classifying them security-sensitive. Both failed the
test — see `qa-evidence.md`'s PR-023-D section for the full reasoning — and are now inert by
construction (`RequiredMultilinePasteConfirmation`, `RequiredDestructiveCommandApproval`), the
same shape response 266 built for `default_trust`. The three `restricted_mode_blocks_*` fields
and `redact_secret_like_environment_names` were checked against the same test and found to pass
it (documented in `config/model.rs`'s own doc comments) — they stay real, security-sensitive
booleans pending the classifier and reload-gating work below.

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

- [x] Explicit reload entry point exists. (`ConfigStore::reload`, PR-023-C)
- [x] Designed so the M13 watcher can call the same path without policy change.
      (`reload`'s own doc comment states this explicitly)
- [x] Failed reload changes nothing and reports diagnostics.
      (`reload_with_a_file_valid_in_its_first_half_and_invalid_in_its_second_changes_nothing`,
      ablated for real, PR-023-C)
- [x] Automatic file-change reload **not** implemented; deferral to M13 recorded.
- [x] **Security-sensitive settings do not apply on reload without confirmation.** New this
      round: `config/sensitive.rs`'s `security_sensitive_diff`/`apply_safe_fields`, wired into
      `ConfigStore::reload`. A reload that changes both a safe and a security-sensitive field
      applies the safe one and leaves the security-sensitive one at its old value, naming it in
      `ConfigReloadOutcome.pending_security_sensitive_changes`. Ablated for real: bypassed the
      gating (`self.current = outcome.document.clone()`), confirmed
      `reload_applies_a_safe_change_but_holds_a_security_sensitive_one_pending` fails with the
      security-sensitive field applied, restored, confirmed green.
      (`reload_applies_a_safe_change_but_holds_a_security_sensitive_one_pending`, plus twelve
      `config/tests/sensitive.rs` unit tests for the diff/apply/direction functions in isolation)

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
