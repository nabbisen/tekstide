# RFC-023: Configuration System - QA Evidence

Status: PR-023-B/C/D accepted; PR-023-E implemented 2026-08-20 (profile translation, four bypass
tests, Managed-denial, profile-direction audit classification), awaiting review; PR-023-F not
started
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

**Response 269 follow-up, same day: the character rule was a second escaping primitive.** The
reviewer found that `bound_key_segment`'s `?`-replacement rule (any non-ASCII-graphic character
→ `?`) violated RFC-020's own required reading, "do not add a second escaping primitive" —
`text_safety::escape_untrusted_chars` already exists, is reviewed, and handles both hostile
cases (control characters, bidi overrides) by turning them into a visible `<U+XXXX>` marker.
Worse, the `?` rule was lossy in a way that mattered: it replaced *every* non-ASCII character,
not only hostile ones, so a Polish `ł`/`ą` or a profile named in Japanese or Cyrillic became an
unreadable row of `?` — defeating the diagnostic for exactly the users the i18n work exists to
serve, while solving a problem the reviewed primitive already solves without that cost.

Fixed: `bound_key_segment` now truncates the *raw* input to the 128-character cap first, then
calls `text_safety::escape_untrusted_chars` on the truncated result — truncating before escaping
because escaping expands (a marker is 8 characters), so truncating the raw input first keeps the
escaped result bounded without ever risking cutting a `<U+XXXX>` marker in half; escaping first
and truncating the expanded string afterward would risk exactly that.

Two new tests plus one existing test strengthened: `legitimate_non_latin_text_in_an_unknown_key_survives_unescaped`
(real Polish and Japanese text in a key survives completely unchanged, not replaced with `?`);
`a_hostile_character_at_the_truncation_boundary_is_never_split` (a bidi override placed at the
127th character produces the whole `<U+202E>` marker or none of it, never a fragment);
`a_bidi_override_or_control_character_in_an_unknown_key_is_neutralized` now asserts the actual
`<U+202E>`/`<U+0007>` markers are present (positive assertion) alongside the original
absence-of-raw-character assertion, and that the surrounding legitimate text (`safe`, `evil`,
`bell`) survives.

**Three separate ablations, each isolating a different property**:

1. Bypassed `bound_key_segment` entirely (returned input unchanged) — all five key-bounding
   tests failed with the hostile/overlong text present verbatim.
2. Bypassed only the escaping step, keeping truncation (`bounded = truncated_raw.clone()`) — the
   two marker-asserting tests failed, while `legitimate_non_latin_text_in_an_unknown_key_survives_unescaped`
   still passed, confirming that test does not vacuously pass under a broken implementation (it
   exercises no hostile characters, so it can't distinguish correct escaping from none — the
   marker tests are what actually prove escaping runs).
3. Swapped the ordering to escape-then-truncate — `a_hostile_character_at_the_truncation_boundary_is_never_split`
   failed with the literal fragment `"core.aaa...aaa<…"` (a lone `<` where the marker should have
   been whole), the exact failure mode the reviewer described.

All three restored, confirmed green after each.

Gates re-run after this fix: `cargo fmt --all --check` clean, `cargo clippy --workspace
--all-targets --all-features -- -D warnings` clean, `cargo test --workspace --all-targets
--all-features` run twice: `tekstide-core` 663 passed (was 661 — 2 new tests), `tekstide` 303
passed, `reference_adapter` 0 tests — both runs clean, `git diff --check` clean.

### PR-023-D — Security-sensitive classification, reload, audit

**In progress, 2026-08-19. First landed piece: classification correction for the two settings
response 270 asked to check, applied rigorously to the whole `[security]`/`[terminal]` surface
rather than only the two named.** Reload gating, the `sensitive_config_changed` producer, and
`RestrictedModeFeature::WorkspaceConfigLoading` are not yet built — see Pending below.

**The test, restated**: RFC-023 now carries a general rule from `default_trust`'s own correction —
*"if flipping a setting would grant a capability that another RFC requires a deliberate per-use
act for, confirmation-on-change is the wrong control."* Security-sensitive classification
(confirm-once-and-audit-the-change) governs *changing* a setting; it does not close a gap where
the setting, once changed, silently grants something a *different* RFC requires a deliberate act
for on every future use.

**Applied to every candidate in `[security]` and `[terminal]`, not only the two response 270 named
(both, plus three more re-examined on my own initiative since the same flaw class could plausibly
apply):**

- **`terminal.multiline_paste_protection` — fails the test.** RFC-018's multiline paste
  confirmation modal is unconditional in the real terminal input path today
  (`shell.rs`'s `TerminalInputDecisionReason::MultilinePasteRequiresConfirmation` — every
  multiline paste opens it; no existing code path skips it). A config value able to disable that
  modal would be a *new* bypass this codebase does not have today, for every terminal in every
  project, forever, with no per-paste confirmation ever again.
- **`security.require_approval_for_adapter_destructive_commands` — fails the test, the most
  severe of the group.** `approval::arrival::should_promote_to_modal` unconditionally promotes
  `High`/`Destructive` risk to the confirmation modal today; there is no existing "skip approval"
  path. A config value able to disable it would let every future destructive command an AI agent
  proposes execute with no human in the loop, in any project, ever.
- **`security.restricted_mode_blocks_workspace_prompts`/`_workspace_lsp`/`_workspace_plugins` —
  pass the test, stay real booleans.** These redefine Restricted Mode's own default *policy* —
  RFC-023 §Security-Sensitive Settings explicitly names "Restricted Mode defaults and the
  blocked-feature policy" as legitimately configurable-with-confirmation. Disabling one does not
  bypass RFC-032's trust grant, which is a separate axis: a project still needs its own,
  per-project trust decision for what that trust unlocks, independent of what Restricted Mode
  blocks for untrusted projects generally.
- **`security.redact_secret_like_environment_names` — passes the test, stays a real boolean.** No
  environment-variable disclosure flow exists yet for this to bypass; there is no other RFC's
  deliberate per-instance act in play.

**Fix, mirroring `default_trust`'s exact shape (response 266/267).** Two new zero-field unit
structs in `config/model.rs`: `RequiredMultilinePasteConfirmation`,
`RequiredDestructiveCommandApproval` — each with exactly one possible value, inert by
construction rather than checked at runtime. `TerminalSettings.multiline_paste_protection` and
`SecuritySettings.require_approval_for_adapter_destructive_commands` now hold these types instead
of `bool`. The loader (`config/load.rs`) gained `take_required_true`, mirroring
`default_trust`'s dedicated match arm: a file that says `false` for either setting is an
explicit, named `ConfigDiagnostic` ("must be true — configuration cannot disable this
protection"), not silently coerced to the safe default — the same "silent acceptance is the
outcome to avoid" reasoning response 267 established.

**Tests**: `multiline_paste_confirmation_has_exactly_one_possible_value`,
`destructive_command_approval_has_exactly_one_possible_value` (model, mirroring
`default_trust_has_exactly_one_possible_value` — the type is the enforcement, the test documents
it); `multiline_paste_protection_set_to_false_in_the_file_is_an_explicit_named_error`,
`multiline_paste_protection_set_to_true_in_the_file_is_accepted`,
`destructive_command_approval_set_to_false_in_the_file_is_an_explicit_named_error`,
`destructive_command_approval_set_to_true_in_the_file_is_accepted` (load). **Ablated for real**:
widened `take_required_true`'s match to accept `false` too, confirmed both
`*_set_to_false_in_the_file_is_an_explicit_named_error` tests fail (the file is silently
accepted, the type stays safe due to inertness — proving these tests guard *silent acceptance of
the false value*, the same independent-of-the-type-level-safety-net property response 268
demonstrated for `default_trust`), restored, confirmed green.

**Gates run**, 2026-08-19: `cargo fmt --all --check` clean. `cargo clippy --workspace
--all-targets --all-features -- -D warnings` clean. `cargo test --workspace --all-targets
--all-features` run twice: `tekstide-core` 669 passed (was 663 — 6 new tests), `tekstide` 303
passed (unchanged), `reference_adapter` 0 tests — both runs clean. `git diff --check` clean.

**Response 271, same day: classification accepted, with the reasoning sharpened rather than the
code changed.** The reviewer corrected the *route* to the restricted-mode-blocks pass, not the
verdict: disabling a block does give untrusted projects a capability, so it is not "a separate
axis" from RFC-032 as I'd written — the real reason these three pass is that **no RFC defines a
deliberate per-use act for them to bypass** (RFC-004 treats them as a policy baseline, not a
per-project decision, and no GUI route exists to lift one for a single project) — which is
literally what the confirmation-on-change test asks. The reviewer also flagged
`redact_secret_like_environment_names`'s pass as *dated, not permanent*: it holds only because no
environment-variable disclosure flow exists yet, and that flow is on the delivery plan for M12
alongside this RFC. Written into RFC-023 itself as a trigger to re-test before environment
redaction ships — no code change required here, but recorded so the next reader inherits the
reasoning's own expiry date, not just its conclusion.

**Reload gating, 2026-08-19 — the second piece of PR-023-D.** New `crates/tekstide-core/src/config/sensitive.rs`
(`config/tests/sensitive.rs`), wired into `ConfigStore::reload`.

`SecuritySensitiveField` names the seven fields classified security-sensitive above (the two
inert settings are deliberately absent — they can never differ between two documents at all, so
gating a value that cannot change would be a no-op dressed up as a control).
`security_sensitive_diff` compares two documents and returns every one of the seven that differs.
`apply_safe_fields` builds the document that actually takes effect on reload: every safe field
from the freshly parsed candidate, every security-sensitive field held at the current value —
one complete new `ConfigurationDocument` constructed before ever being assigned, the same
"compute first, assign once" discipline `parse_and_validate`'s pipeline already uses, extended
one level. `ConfigStore::reload` now returns `ConfigReloadOutcome { warnings,
pending_security_sensitive_changes }` instead of a bare warning list — the changes are reported,
not silently dropped, even though nothing can act on the report yet (no confirmation surface
exists — M12 UI work, per this RFC's own Non-Goals).

**`SecuritySensitiveDirection` and `direction()`**, RFC-013's frozen `config_policy_increase`/
`config_policy_reduce` vocabulary applied per field, ahead of building the actual audit producer:
`true`→`false` on any of the four booleans is `Increase` (weakens); the reverse is `Reduce`. Longer
transcript retention is `Increase` (keeps more data around, weaker privacy posture); shorter is
`Reduce`. `AgentProfiles` and `AgentDefaultEnvironmentPolicy` return `None` — not because they
have no direction, but because this module does not yet define one: RFC-023's own text gives
per-profile examples ("adding a profile" = increase, "removing one" = reduce) that need per-profile
diffing PR-023-E's own profile-identity work is better positioned to build; `default_environment_policy`
has no defined value ordering yet (today's only real value is `"explicit"`). Both stay
reload-gated regardless — direction is an audit-producer question, gating already holds the field
back either way. `direction`/`security_sensitive_diff`/`apply_safe_fields` exported from `config.rs`
as genuine public API (this module is reusable library code, the same as `parse_and_validate`/
`ConfigStore`), which is also why `direction` doesn't trip `dead_code` despite the audit producer
that will call it in earnest not existing yet — it's real, tested, callable functionality, not
scaffolding.

**Tests**: twelve in `config/tests/sensitive.rs` covering the diff (every field, and that a safe
field never appears in it), the apply (every safe field taken from the candidate, every
security-sensitive field never taken from it), and direction (both transitions for the four
booleans and retention days; `None` for the two deferred fields) as pure functions in isolation.
One integration test, `reload_applies_a_safe_change_but_holds_a_security_sensitive_one_pending`,
proves the wiring through the real `ConfigStore::reload` a caller actually uses — one reload
changing both a safe and a security-sensitive field at once, asserting the safe one applies, the
security-sensitive one doesn't, and the outcome names it as pending. **Ablated for real**:
bypassed the gating in `reload` (`self.current = outcome.document.clone()`), confirmed the
integration test fails with the security-sensitive field applied, restored, confirmed green.

**Gates run**, 2026-08-19: `cargo fmt --all --check` clean. `cargo clippy --workspace
--all-targets --all-features -- -D warnings` clean (the `direction`/`security_sensitive_diff`/
`apply_safe_fields` export was needed to clear a `dead_code` warning on `direction`'s first draft,
before it had any caller). `cargo test --workspace --all-targets --all-features` run twice:
`tekstide-core` 682 passed (was 669 — 13 new tests), `tekstide` 303 passed (unchanged),
`reference_adapter` 0 tests — both runs clean. `git diff --check` clean.

**Response 272 follow-up, same day: retention policy was split across two sections, only one
half was gated.** RFC-023 §Security-Sensitive Settings names "transcript retention **and purge**
policy" as one policy; RFC-011 implements it as four bounds (per-transcript bytes, per-project
bytes, app-wide bytes, max age). This module's original classification covered only
`agent.transcript_retention_days` (max age) — `resources.max_agent_transcript_mb_per_run`
(per-transcript bytes) sat unclassified because the typed model happens to put it under
`[resources]` rather than `[agent]`. The reviewer's own framing: *"the classification followed
the section boundary rather than the policy."* Concrete consequence before the fix: raising
`max_agent_transcript_mb_per_run` from `128` to `4096` would have applied on reload with no
confirmation, quadrupling-and-more the transcript bytes retained per run.

Fixed: `SecuritySensitiveField::ResourcesMaxAgentTranscriptMbPerRun` added, gated in
`security_sensitive_diff`/`apply_safe_fields` alongside the other seven fields, with a
`retention_direction` helper shared with `transcript_retention_days` — larger bound (either days
or megabytes) is `Increase`, smaller is `Reduce`, same reasoning either way: more data retained
is the weaker privacy posture. `max_terminal_output_mb_per_session` (live output, not persisted
beyond the already-covered transcript path) and `max_file_watch_events_per_batch` (a throughput
bound for the M13 watcher, which doesn't exist yet) stay unclassified, with the reasoning
recorded in `sensitive.rs`'s own doc comment and asserted directly in
`a_change_to_a_safe_field_does_not_appear_in_the_diff` — neither is retention policy, so the
boundary excludes them on purpose.

Two new direction tests (`a_larger_per_run_transcript_byte_limit_is_an_increase`/
`a_smaller_..._is_a_reduce`); the existing "every field changes" and "never takes a
security-sensitive field from the candidate" tests extended to include the new field. **Ablated
for real**: made `apply_safe_fields` read `max_agent_transcript_mb_per_run` from `candidate`
instead of `current` (the exact bug the reviewer described), confirmed
`applying_safe_fields_never_takes_a_security_sensitive_field_from_the_candidate` fails with
`left: 4096, right: 128` — the literal escalation scenario from the review, reproduced and
caught — restored, confirmed green.

Gates re-run after the fix: `cargo fmt --all --check` clean, `cargo clippy --workspace
--all-targets --all-features -- -D warnings` clean, `cargo test --workspace --all-targets
--all-features` run twice: `tekstide-core` 684 passed (was 682 — 2 new tests), `tekstide` 303
passed, `reference_adapter` 0 tests — both runs clean, `git diff --check` clean.

**Final piece of PR-023-D, 2026-08-19: `WorkspaceConfigLoading`, the audit producer, and the
sentinel test.**

**`RestrictedModeFeature::WorkspaceConfigLoading`** added to `security.rs`'s vocabulary (`ALL`
bumped 9 → 10). Reserved, not wired to a real loader — RFC-023 ships only defaults + user-global
configuration in v1, so there is no workspace-config-reading code anywhere for this variant to
gate yet; landing the vocabulary now means a future workspace-config reader cites an
already-reviewed variant instead of adding one alongside itself. No other exhaustive `match` over
this enum exists anywhere in the workspace (confirmed by grep before adding the variant — every
other call site iterates `ALL` or reads `.len()` dynamically), so restricted-mode-blocking
coverage for the new variant came for free from the existing generic test
(`restricted_mode_blocks_workspace_local_automation_paths`). **One real regression found and
fixed**: `tekstide-core::shell::tests::populated_project_board_renders_placeholder_branch_status_without_process_probe`
asserted the literal rendered string `"blocked automation: 9"` — a genuine hardcoded count my
grep for bare `9` (combined with `restrict|feature`) missed because it's embedded in a longer
string. Fixed to derive the expected count from `RestrictedModeFeature::ALL.len()` instead of a
literal, so it can't silently go stale again the same way.

**The `sensitive_config_changed` producer**, `AuditCoordinator::record_sensitive_config_policy_increase`/
`_reduce` (`audit/integration.rs`), following `record_paste_blocked`'s *observe* shape, not
`grant_project_trust`'s *operate-and-audit* shape: no confirmation surface exists yet to call
this from (M12 UI work), so, like every other config producer, this does not perform the change
itself — it records one that has already been confirmed and applied. Both producers take **no
parameters describing what changed**, only that a change of a given direction occurred: the
frozen schema forces `subject_kind: None` for this family, and a separate crate-wide invariant
(`subject_kind.is_some() == subject_ref.is_some()`) then structurally forces `subject_ref: None`
too — so the reviewer's own carried-forward concern (should `AgentProfiles` name *which* profile
changed?) is settled by the schema itself, not a judgment call that could be gotten wrong. Also
settles why `project_id` is always `None`: workspace configuration isn't implemented, so every
config change today is global with no project to attribute it to.

**Increase**: writes `Authorized` then `Applied` under one fresh `AuditOperationId`, the same
two-linked-records shape `grant_project_trust` uses and for the same reason — by the time either
is called, the deliberate confirming act has already happened, so recording it as two stages in
one call is complete and honest. `User`/`TrustedUi`: RFC-023 requires explicit confirmation for
this direction. **Reduce**: single stage, `Applied` only, `operation_id: None` — `valid_config_change`
fixes both, matching RFC-023's own asymmetry that tightening never needs authorization.
`AppPolicy`/`PolicyEngine`, not a user actor: accurate, not merely the schema's other allowed
pairing — no confirming act is required for this direction. Both best-effort (`append_observation`,
not `append_required`), matching this pack's own stated rule for every config producer.

**Tests**: two full persistence-and-validation tests (increase's two linked records, reduce's
one), two schema-boundary ablations (`ConfigPolicyIncrease` without an `operation_id`;
`ConfigPolicyReduce` with one present), and the RFC's own required sentinel test —
`no_config_value_can_reach_a_sensitive_config_changed_record`, calling both producers and
asserting a distinctive secret-shaped string, plus literal field names
(`"agent.profile"`/`"restricted_mode_blocks"`), never appear in the real, persisted,
queried-back records' `Debug` output. The honest framing, stated in the test's own doc comment:
this is not proving a leak was closed, but confirming the structural guarantee (no value
parameter exists to leak) against the real store round-trip rather than only by reading the
source.

**Gates run**, 2026-08-19: `cargo fmt --all --check` clean. `cargo clippy --workspace
--all-targets --all-features -- -D warnings` clean. `cargo test --workspace --all-targets
--all-features` run twice (after the count-regression fix): `tekstide-core` 689 passed (was
684 — 5 new sensitive-config-changed tests), `tekstide` 303 passed (unchanged), `reference_adapter`
0 tests — both runs clean. `git diff --check` clean.

**Response 274 follow-up, same day: a reserved vocabulary variant was inflating a real,
user-visible count.** `RestrictedModeSummary::from_trust` built the Project Board's
`blocked_features` (and, downstream, its rendered "blocked automation: N" count and label list)
from `RestrictedModeFeature::ALL` — the full ten-variant vocabulary, including
`WorkspaceConfigLoading`, which blocks nothing since no code anywhere reads a workspace config
file yet. A real user opening an untrusted project was told ten automations were blocked when
nine actually were. The regression fix that added the tenth variant to the count in the first
place (deriving the expected string from `RestrictedModeFeature::ALL.len()` instead of a
hardcoded `9`) was mechanically correct — a literal does go stale — but it coupled a user-facing
claim to the *vocabulary* rather than to what is *enforced*, and the vocabulary now contains a
reserved entry with no enforcement point.

Fixed: `RestrictedModeFeature::ENFORCED` added alongside `ALL` — the nine variants with a real
production trigger today. `ALL` stays as the full vocabulary, used only where the property under
test is the *policy function's* exhaustive correctness
(`restricted_mode_blocks_workspace_local_automation_paths`/`trusted_workspace_allows_policy_checked_automation_paths`,
both in `security/tests.rs`, both correctly still iterate `ALL` — a reserved feature must still
be *correctly* blocked if it is ever checked, even though nothing checks it yet).
`RestrictedModeSummary::from_trust` now builds `blocked_features` from `ENFORCED`. Three test
call sites updated to expect `ENFORCED.len()` (9) instead of `ALL.len()` (10):
`restricted_mode_summary_exposes_ui_ready_blocked_feature_labels` (`security/tests.rs`, gained a
second assertion that `WorkspaceConfigLoading` is absent from the summary directly),
`project_rows_preserve_placeholder_field_shape_without_probing` (`project_board/tests.rs`), and
`populated_project_board_renders_placeholder_branch_status_without_process_probe`
(`shell/tests.rs` — the same test the earlier regression fix touched).

**Ablated for real**: reverted `from_trust` to `ALL`, confirmed all three tests fail with the
literal `left: 10, right: 9` mismatch (and the rendered-text test's own string-match failure),
restored, confirmed green again.

**`README.md`'s "the nine restricted features" claim was correctly left untouched**, per the
reviewer's own note: under this fix it is true again (nine are actually enforced), so "fixing" it
to say ten would have re-introduced the exact overstatement this response corrects. Recorded here
so a reader comparing the README against `RestrictedModeFeature::ALL.len()` (10) does not conclude
the README is stale — it isn't; it's counting the right thing.

Gates re-run after the fix: `cargo fmt --all --check` clean, `cargo clippy --workspace
--all-targets --all-features -- -D warnings` clean, `cargo test --workspace --all-targets
--all-features` run twice: `tekstide-core` 689 passed (unchanged — the fix strengthened existing
tests rather than adding new ones), `tekstide` 303 passed, `reference_adapter` 0 tests — both runs
clean, `git diff --check` clean.

**PR-023-D closes with this fix.** What remains for RFC-023 as a whole: PR-023-E (AI CLI profiles
from configuration, the highest-risk slice — bypass tests first) and PR-023-F (closeout evidence,
known limitations, answers to the RFC's open questions). Neither started.

### PR-023-E — AI CLI profiles from configuration

**Implemented 2026-08-20.** The highest-risk slice — write the bypass tests first, which is the
order this actually happened in.

**The translation** (`crates/tekstide-core/src/config/profile.rs`, new module):
`to_ai_cli_profile(id, &ConfiguredAiCliProfile) -> AiCliProfile`, exactly the scope
`ConfiguredAiCliProfile`'s own doc comment (PR-023-B) already named for this slice. `source` is
always `UserGlobal`, per RFC-023 §AI CLI Profiles From Configuration's own words. This function
does not, and structurally cannot, touch `AgentRunLaunchValidator::validate` — it only
constructs a value; `git diff` on `crates/tekstide-core/src/agent/launch.rs` is empty in the
final commit, confirmed directly rather than assumed from intent.

**Two fields with no home yet, stated rather than silently dropped.** `configured.args` has no
corresponding field on `AiCliProfile` at all — the launch pipeline has no argv-template concept
for a profile to configure — so this translation does not wire it anywhere; the same "typed
storage, no consumer yet" status this pack's own Scoping section already gives keybindings and
several other fields. `environment_policy`'s string value is not parsed into a specific
`AiCliEnvironmentPolicy` either; every config-defined profile gets `Minimal`, the same
least-exposure default `AiCliProfile::new` itself already uses, not a weaker one invented here.

**Executable resolution** (`resolve_executable`, private to `profile.rs`): a `command`
containing a path separator resolves as `AiCliExecutable::Absolute`; a bare name resolves via
`AiCliExecutable::PathLookup` against reviewed system paths only (`/usr/local/bin`, `/usr/bin`)
— **never** `ExecutableLookupPath::project_local`, because `ConfiguredAiCliProfile` has no field
that could request one. This is why the fourth bypass test (below) constructs its `AiCliProfile`
directly rather than through the translator: proving `AgentRunLaunchValidator::validate` itself
still refuses that shape is the point, since the translator cannot produce it today regardless.

**The four bypass tests**, all in `crates/tekstide-core/src/config/tests/profile.rs`, all
building a real `ConfiguredAiCliProfile`, translating it through the real `to_ai_cli_profile`,
and validating through the real, unmodified `AgentRunLaunchValidator`:
- `config_profile_pointing_at_a_project_root_executable_is_rejected`
- `config_profile_pointing_at_a_wrapper_script_inside_the_project_root_is_rejected` (a real,
  executable shell-script wrapper — same underlying guard as the case above, proven separately
  because the review gate names it as its own case)
- `config_profile_pointing_at_a_symlink_resolving_into_the_project_root_is_rejected` (a real
  symlink outside the root, target inside it)
- `config_profile_relying_on_a_project_local_path_entry_is_rejected` (constructed directly, per
  the executable-resolution note above)

Plus a positive control, `a_legitimate_config_profile_outside_the_project_root_validates_successfully`
— without it, a translator that rejected everything would trivially pass all four bypass tests.

**Managed does not confer Managed**, proven twice: `to_ai_cli_profile_never_sets_managed_compatibility_or_structured_action_approval`
(the translator structurally cannot produce it — no field exists to request it from, tried
across a range of `adapter` strings including `"managed"`, `"MANAGED"`, `"reference"`) and
`managed_compatibility_level_without_structured_action_approval_is_still_rejected` (a hand-forced
`compatibility_level: Managed`, simulating a hypothetical future translator bug, is still
rejected by RFC-010's own `validate_compatibility`, unmodified).

**Ablated for real, three separate guards, each restored and confirmed clean**:
1. `validate_executable_provenance`'s restricted-mode check — disabled, the three
   executable-location bypass tests all failed with the launch succeeding (the failure output
   showed the resolved `executable_path` genuinely inside the project root). Restored.
2. `validate_lookup_path`'s two checks (declared-project-local and canonicalize-and-compare) —
   disabled, `config_profile_relying_on_a_project_local_path_entry_is_rejected` still failed, but
   on a *different* error variant (`WorkspaceLocalExecutableBlocked`, from the independent
   downstream provenance check catching the same hostile resolved path) rather than passing
   outright — genuine defense-in-depth, and confirms this test is precisely pinned to the
   PATH-lookup guard specifically, not merely "some rejection happens." Restored.
3. `validate_compatibility`'s `Managed`-without-capability check — disabled,
   `managed_compatibility_level_without_structured_action_approval_is_still_rejected` failed with
   the launch succeeding as `Managed`. Restored.

`git diff` on `agent/launch.rs` confirmed empty after each restoration and in the final commit.

**Workspace-local via config-file-location: vacuously true, stated as such.** RFC-023 v1 loads
only defaults and user-global configuration — `ConfigStore::load` takes a single,
non-project-relative path, confirmed by reading the whole loading pipeline before writing this.
There is no code path anywhere that could load a configuration file from inside a project root,
so there is nothing for the "profiles from such a file are workspace-local" rule to apply to
yet. Same status the Workspace Configuration Checklist already gives the comparable
security-sensitive-setting question.

**Profile add/edit is audited: completing PR-023-D's own deferred piece.** Reload-gating for
`[agent.profile.*]` already existed (`current.agent.profiles != candidate.agent.profiles`,
unmodified this slice); what was missing was the audit-producer *direction* classification,
which `sensitive.rs`'s own doc comment named as PR-023-E's to do. Added
`agent_profiles_direction` (`crates/tekstide-core/src/config/sensitive.rs`): RFC-023's two own
worked examples (adding a profile is `Increase`, removing one is `Reduce`) plus a rule for the
case the RFC does not name — modifying an existing profile's fields — worst-case-wins:
`candidate` is `Reduce` only if it is a pure subset of `current` (every entry it still has exists
under the same key with an *identical* value); anything else, including a single-field
modification or a mixed add-and-remove, is `Increase`. Five new tests in
`config/tests/sensitive.rs` cover add/remove/modify/mixed cases. **Ablated for real**: inverted
the subset-check's two branches, confirmed all four direction-specific tests fail with the
literal `Reduce`/`Increase` values swapped, restored.

**OQ3, raised rather than built: "should configuration-defined profiles require a one-time
confirmation on first use?"** RFC-023's own Scoping addendum (2026-08-19) answers **yes**, and
names this as something PR-023-E "carries." Not implemented this slice, and the reason is a real
finding, not a schedule call: **`AiCliProfileSource::UserGlobal` cannot currently distinguish a
genuinely configuration-defined profile from a hardcoded one.** `AiCliProfile::claude_code_linux_default`
— the one real, shipped built-in profile — is *also* `AiCliProfileSource::UserGlobal` (its own
doc comment, `agent/profile.rs`). A confirmation-on-first-use gate keyed on `source == UserGlobal`
alone would therefore also demand confirmation for the built-in Claude Code profile, which needs
none — it is compiled in, already reviewed, not something a file supplied. Building the gate
correctly needs either a new, narrower marker distinguishing "this profile's values came from a
parsed file" from "this profile is compiled into the binary," or a confirmation-state registry
keyed by profile identity that a future M12 confirmation dialog would consult — both are genuine
design surfaces, not scaffolding this slice can add as a side effect, and the second crosses into
GUI-layer persistence this headless RFC does not otherwise touch. Raised in review request 281
rather than decided unilaterally.

**What this does not establish.** No confirmation-on-first-use exists yet (OQ3, above). No real
boot path constructs a `ConfigStore` or calls `to_ai_cli_profile` in production — this slice
proves the translation and its safety, not that a configured profile is reachable from the real
GUI yet, the same "typed storage / real logic, no production caller yet" status this pack's own
convention has given every other piece of RFC-023 so far. `configured.args` is not wired into
any launch pipeline field, for the reason stated above. `environment_policy`'s string value is
not parsed beyond always producing `Minimal`.

**Gates run**, 2026-08-20: `cargo fmt --all --check` clean. `cargo clippy --workspace
--all-targets --all-features -- -D warnings` clean. `cargo test --workspace --all-targets
--all-features` run three times, all fully clean, combined with the same-session, separately
committed leaked-child test-harness fix (`f0c5055`) landed just before this slice:
`tekstide` 311 passed (unchanged, no GUI-crate work this slice), `tekstide-core` 713 passed (was
695 before either same-session change; this slice's own 10 `config/tests/profile.rs` tests plus
a net +4 in `config/tests/sensitive.rs` account for 14 of the +18, the leaked-child fix's own 4
new `test_support` tests the rest). `git diff` on `crates/tekstide-core/src/agent/launch.rs`
confirmed empty. `git diff --check` clean. Committed as `855d063`, staged by explicit path,
separately from the unrelated leaked-child fix.

### PR-023-F — Closeout evidence

Pending implementation.

## Known Limitations

- **Automatic reload on file change is not implemented.** Explicit reload only; the file watcher arrives in M13 and will call the same entry point without a policy change.
- Security-sensitive settings do not hot-reload by design; they require confirmation or apply to future sessions only.
- Workspace configuration may be reserved rather than implemented in the first slice, provided the blocked-feature vocabulary and precedence rule land with it.
- Durable audit records the setting name and change direction only. It cannot answer "what value was it changed to" — consistent with RFC-013's exclusion of values from audit throughout.
