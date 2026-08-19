# RFC-023: Configuration System - QA Evidence

Status: PR-023-B and PR-023-C implemented 2026-08-19, awaiting review; PR-023-D onward pending
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

**Implemented 2026-08-19.** New `crates/tekstide-core/src/config/load.rs`, plus
`config/tests/load.rs`. Adds the `toml` crate (workspace dependency, version `"1"` — a
newly-added dependency, so pinned to the current major rather than an older one).

**Pipeline.** `parse_and_validate(source: &str) -> Result<ConfigLoadOutcome, ConfigDiagnostic>`:
parses to an untyped `toml::Table`, then walks each of the 8 sections in turn
(`extract_core`/`extract_ui`/.../`extract_resources`), each pulling its own known fields out of
its own sub-table via small typed helpers (`take_bool`/`take_u32`/`take_string`/
`require_string`/`take_string_array`) and pushing anything left unconsumed as a warning. The
function is pure — no shared state, so "no partial application" is not a property that needs
separate proving; there is no code path that mutates anything outside the function's own locals,
so either a complete `ConfigurationDocument` comes back or nothing does.

**`ConfigStore`** wraps the pure function with the actual file I/O and the "swap" step the RFC's
own pipeline names: `load` (initial, defaults on missing/invalid) and `reload` (explicit,
designed for the M13 watcher to call unchanged later). `self.current` is assigned in exactly one
place in the whole type — `reload`'s last line — gated on `parse_and_validate` having already
returned a fully valid document. **Ablated for real**: temporarily inserted a premature
`self.current.core.recent_projects_limit = 5;` before the validation call (simulating a
"partial early-apply" bug), confirmed
`reload_with_a_file_valid_in_its_first_half_and_invalid_in_its_second_changes_nothing` — the
review's own planned test, verbatim: *"a file that is valid in its first half and invalid in its
second, then assert nothing from the first half took effect"* — fails with exactly that leaked
value, restored, confirmed green again.

**Unknown keys warn, unknown values error**, proven both ways with dedicated tests at every
level: unrecognized top-level section, unrecognized key inside a known section, unrecognized key
inside a `[agent.profile.*]` table (all three warn, not fail); wrong-typed value for a known key,
and a missing required key inside a profile (`command`), both error, naming the offending key.

**`default_trust = "trusted"` carried forward from response 267.** Not a `take_string`/coercion
call — a dedicated match arm that accepts only the literal `"restricted"` and returns a named
`ConfigDiagnostic` for anything else, so the file is refused rather than the dangerous value
being silently dropped in favor of the safe default (response 267's own words: *"a false belief
[that blanket trust was configured] in that direction is exactly what this whole finding was
about"*). **Ablated for real** a second time at this layer: temporarily widened the accepted-value
guard to also accept `"trusted"`, confirmed `default_trust_set_to_trusted_in_the_file_is_an_explicit_named_error`
fails (the file is silently accepted, still producing the type-level-safe `RestrictedDefaultTrust`
value — proving this test guards *silent acceptance of the dangerous string*, independent of and
in addition to the type-level safety net response 266 already built), restored, confirmed green.

**Diagnostics: bounded, and now carrying the field the earlier draft of this section missed.**
`ConfigDiagnostic.message` is `&'static str` — inert by construction, the same shape response 266
used for `RestrictedDefaultTrust`, so there is no code path by which file content or a rejected
value could ever reach it, re-verified directly with a real secret-shaped sentinel string
(`a_secret_shaped_rejected_value_never_reaches_the_diagnostic`). `key` names the offending
`section.field`. `location` carries a byte-offset span for TOML syntax errors, when the
underlying parser provides one — `None` for validation errors constructed while walking the
already-parsed table, since that information isn't available at that layer; an honest "where
available," not an omission. **`path`**: the checklist requires "diagnostics carry file path,
error location where available, and the offending key" — the first draft of this pipeline had no
`path` field at all, caught while updating this checklist rather than by review. Fixed:
`parse_and_validate` itself stays path-agnostic (it validates source text, not a file), and
`ConfigStore` fills in `path` via `ConfigDiagnostic::with_path` on every diagnostic it returns —
`parse_and_validate_alone_leaves_the_diagnostics_path_unset` and
`store_load_with_an_invalid_file_at_first_start_yields_defaults_with_a_diagnostic` pin both
halves of that split.

**A fully-specified document round-trips every section** with zero warnings
(`a_fully_specified_document_round_trips_every_section`), including a real `[agent.profile.codex]`
table with `command`/`args` set and `adapter`/`environment_policy` left to default —
demonstrating the per-field default-vs-override behavior inside a nested table, not only at the
top level.

**What this does not establish.** Nothing here reads a *live* configuration anywhere in the
application — `ConfigStore` exists and is fully tested, but `main.rs`/`boot()` does not construct
one; wiring an actual load at startup is not named as this slice's own scope in the task
breakdown, and doing so here would be scope creep beyond "atomic load, validation, diagnostics."
Security-sensitive classification (which settings require confirmation on reload, and the
`sensitive_config_changed` producer) is PR-023-D's, not this slice's — `reload` today applies
every section identically, including ones §5 will later gate.

**Gates run**, 2026-08-19: `cargo fmt --all --check` clean. `cargo clippy --workspace
--all-targets --all-features -- -D warnings` clean. `cargo test --workspace --all-targets
--all-features` run twice: `tekstide-core` 658 passed (was 639 — 19 new load tests), `tekstide`
303 passed (unchanged), `reference_adapter` 0 tests — both runs clean. `git diff --check` clean.

**Response 268 follow-up, same day: `key` bounded against untrusted file text.** The reviewer
found that `ConfigWarning`/`ConfigDiagnostic.key` was bounded for a *known* field name (a fixed
literal this module wrote) but not for an *unknown* one — the raw TOML table key the file
supplied, unfiltered, in the warn path (top-level unrecognized sections, unrecognized keys inside
a known section, unrecognized keys inside a `[agent.profile.*]` table) and in the profile-name
error path (`agent.profile.<name>`, where `<name>` is itself an untrusted TOML key, not a value).
RFC-023 requires "a bounded diagnostic," and workspace configuration is untrusted by this RFC's
own design — a cloned repository's `.tekstide/config.toml` can carry a key of arbitrary length or
containing a bidi override, control characters, or other text shaped to mislead whatever
eventually renders it. Nothing renders diagnostics today, so this was not a live defect, but an
inherited requirement worth writing down rather than leaving silent.

Fixed: `bound_key_segment` (`config/load.rs`) caps a raw key segment to 128 characters (matching
`AuditReference`'s own cap — the same bounding rule, not a second one) and replaces any character
outside printable ASCII (plus space) with `?`, appending an ellipsis when truncated. Applied at
every point a raw file-derived key becomes part of a `ConfigWarning`/`ConfigDiagnostic.key`: the
top-level unknown-section loop, `warn_unconsumed` (covers every section's unrecognized-key
warnings and, transitively, unrecognized keys inside a profile table), the keybindings-section
type error, and the profile-name error/warning path. The profile case needed care: the *real*
profile name (used as the `BTreeMap` key and `display_name`'s default) is deliberately left
unbounded — bounding it would corrupt stored data PR-023-E still has to validate on its own terms;
only the text flowing into diagnostic/warning `key` fields is bounded.

Three new tests, each against the concrete shape named above: an overlong key (500 `a`s) is
truncated with a trailing ellipsis; a key containing a bidi override (`U+202E`) and a control
character (BEL, `U+0007`) — written as a TOML quoted key with `\uXXXX` escapes, the real syntax
either character requires — has both neutralized; a hostile profile name (bidi override plus 200
characters) is bounded in the resulting warning but the profile itself is still stored under its
real, unbounded name. **Ablated for real**: temporarily made `bound_key_segment` return its input
unchanged, confirmed all three tests fail with the hostile/overlong text present verbatim in the
warning, restored, confirmed green again.

Gates re-run after the fix: `cargo fmt --all --check` clean, `cargo clippy --workspace
--all-targets --all-features -- -D warnings` clean, `cargo test --workspace --all-targets
--all-features` run twice: `tekstide-core` 661 passed (was 658 — 3 new tests), `tekstide` 303
passed, `reference_adapter` 0 tests — both runs clean, `git diff --check` clean.

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
