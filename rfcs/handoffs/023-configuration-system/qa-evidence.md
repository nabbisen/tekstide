# RFC-023: Configuration System - QA Evidence

Status: PR-023-B implemented 2026-08-19, awaiting review; PR-023-C onward pending
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

**Implemented 2026-08-19.** New module `crates/tekstide-core/src/config/` (`config.rs`,
`config/path.rs`, `config/model.rs`, `config/tests/{path,model}.rs`), following `audit/path.rs`'s
reference shape as instructed.

**Paths (`config/path.rs`).** `ConfigPathProvider::linux_from_env` /`macos_from_env` /
`windows_from_env`: pure, injectable functions (`AppStatePathProvider::linux_from_env`'s own
shape) computing `$XDG_CONFIG_HOME/tekstide` (falling back to `$HOME/.config/tekstide`),
`$HOME/Library/Application Support/tekstide`, and `%APPDATA%\tekstide` respectively — all three
fully unit-tested on this Linux machine, since they are pure path construction with no OS-specific
API calls. `linux_default`/`macos_default`/`windows_default` wrap them with real env reads;
none is wired into a real runtime entry point yet — no `#[cfg(target_os)]` dispatcher exists
because nothing calls one until a real cross-platform boot path needs it (out of scope here,
consistent with this pack's Scoping section).

`ConfigPathResolver::resolve` mirrors `audit/path.rs`'s `validate_existing_audit_paths` exactly,
retargeted: every check is `if let Ok(metadata) = fs::symlink_metadata(..)`, a no-op when nothing
exists on disk. **Deliberate divergence from the audit precedent, stated because it is the load-
bearing difference**: audit's `canonicalize_dir` *requires* `state_root` to exist (audit always
creates it first, since it is a write target); config's resolver does not require anything to
exist, so a first run with no `~/.config/tekstide/` at all resolves successfully — "a missing
configuration file is not an error" (RFC-023 §Format and Location) holds at the path layer, not
only the future loader
(`resolving_with_nothing_on_disk_yet_succeeds`).

`config_dir` (`tekstide/`) follows `audit_dir`'s rule: a symlink is allowed only if it resolves
within its parent (the platform configuration root, e.g. `$XDG_CONFIG_HOME`) —
`a_symlinked_config_directory_escaping_the_configuration_root_is_rejected` /
`..._staying_within_..._is_allowed` (positive control). `config_file` (`config.toml`) follows
`database_file`'s stricter rule: **any** symlink is rejected outright, even one resolving inside
the directory —
`a_symlinked_config_file_is_rejected_even_if_its_target_stays_inside_the_directory`. **Both
security checks ablated for real**: temporarily disabled each (`if false && ...`), confirmed the
specific test fails with the specific wrong result (the escape check's ablation returns `Ok` where
an error was expected; the file-symlink check's ablation falls through to a different error
variant, `ConfigFileTypeInvalid` instead of `ConfigFileIsSymlink`, proving the test pins the exact
reason, not merely "some error"), restored both, confirmed green again.

**Model (`config/model.rs`).** `ConfigurationDocument`, covering all eight sections
implementation-handoff.md names (`core`, `ui`, `keybindings`, `terminal`, `projects`, `agent`
including `profiles`, `security`, `resources`) — the external design's §11.5 worked example is
the shape source, since RFC-023's own body does not repeat an exact schema.
`#[derive(Default)]` end to end; every section has a compiled default, asserted field-by-field
(`every_section_default_is_the_documented_value`) so a future edit that silently changes one
(e.g. flips a security-relevant bool) fails by name. `agent.transcript_retention_days` reuses the
real `crate::transcript::DEFAULT_TRANSCRIPT_MAX_AGE_DAYS` constant rather than repeating `30`,
pinned against the constant itself
(`transcript_retention_default_reuses_the_real_compiled_constant`) so the two cannot drift apart
silently. Every other default not traceable to an existing compiled constant (font families,
theme name, resource-limit numbers, `[security]`'s five booleans) is the external design's own
suggested value, stated as such in the type's doc comments rather than presented as derived from
running code.

**What this does not establish, stated per this pack's own gate.** No parsing exists yet (TOML,
`serde`, and the atomic parse/validate/construct/swap pipeline are PR-023-C's). No setting in this
model is read by anything — `[keybindings]`, `[ui]`'s theme/font fields, `[agent]`'s profiles and
concurrency limits, `[security]`'s toggles, and `[resources]`'s limits are all typed storage with
no consumer, the same status this pack's own Scoping section already gives keybindings, theme,
locale, resource limits, and transcript-capture defaults — extended here to `[security]` and the
rest of `[agent]`, which the Scoping section did not name individually but which are in the exact
same position: no Goal of this RFC names "wire `[security]`'s specific toggles into live
Restricted-Mode enforcement" or "wire `[agent]`'s concurrency fields into the launch limiter."
`[projects].open_duplicate_root` is a plain string, not an enum, because only one value is
actually implemented today and an enum would assert a choice space that does not exist yet —
validating the string against whatever choice space PR-023-C settles on is that slice's job, not
this one's. **`default_trust` is not a string** — see the response-266 follow-up below; a plain
string was the original (wrong) design.

**Gates run**, 2026-08-19: `cargo fmt --all --check` clean. `cargo clippy --workspace
--all-targets --all-features -- -D warnings` clean (one `derivable_impls` lint on the first draft
of `ConfigurationDocument`'s manual `Default` impl, fixed by deriving instead). `cargo test
--workspace --all-targets --all-features` run twice: `tekstide-core` 638 passed (was 619 — the
19 new config tests), `tekstide` 303 passed (unchanged, no wiring here), `reference_adapter` 0
tests — both runs clean. `git diff --check` clean.

**Response 266 follow-up, same day: `default_trust` made inert by construction.** The reviewer
found that a two-valued `default_trust` (`"restricted"`/`"trusted"`) would be a trust-granting
mechanism bypassing RFC-032's per-project, two-deliberate-act design outright — RFC-023's own
security-sensitive classification (confirm-once-and-audit-the-change) governs *changing a
setting*, not the grants a setting then performs silently on every future project, so it does not
close the gap. The concrete escalation: an agent run in an already-trusted project can write
user-global `config.toml` (trusted by this RFC's own load order), so a config-writable trust
default would let a compromised or malicious agent trust every future project at creation, with no
per-project confirmation and no `TrustGrant` record. The RFC text itself was corrected by the
reviewer (§Security-Sensitive Settings) before this fix landed.

Fixed: `ProjectSettings.default_trust` is now `RestrictedDefaultTrust`, a zero-field unit struct.
This is not runtime validation of a string — there is no constructor, parse path, or field
assignment anywhere in the crate that can produce any value other than the one that exists,
enforced by the Rust compiler rather than a check that could later be weakened or forgotten.
`default_trust_has_exactly_one_possible_value` documents the property; the real enforcement is
the type definition itself in `model.rs`.

Gates re-run after the fix: `cargo fmt --all --check` clean, `cargo clippy --workspace
--all-targets --all-features -- -D warnings` clean (one `default_constructed_unit_structs` lint
on the test's first draft, fixed by comparing to the value directly instead of via
`::default()`), `cargo test --workspace --all-targets --all-features` run twice: `tekstide-core`
639 passed (was 638 — the one new inertness test), `tekstide` 303 passed, `reference_adapter` 0
tests — both runs clean, `git diff --check` clean.

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
