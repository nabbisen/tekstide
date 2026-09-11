---
title: "RFC-045 — QA evidence"
rfc: "RFC-045"
rfc_file: "../../accepted/045-configuration-reachability.md"
source_rfc_status: "Accepted 2026-09-12 — M12"
target_milestone: "M12"
created: "2026-09-12"
---

# Evidence

## PR-045-A — the parser tells the truth

`tekstide-core` only. **No production caller yet** — `boot()` still does not read a configuration
file, and the launch path still hardcodes `claude_code_linux_default()`. What changed is what the
parser is willing to accept.

### D3': the accepted key set is the set with a consumer

`parse_and_validate` now accepts exactly four keys plus a profile's `display_name`/`command`, and
**the withdrawn fields are gone from the model structs** rather than parsed into fields nothing
reads. `ConfigurationDocument` went from eight sections to two:

| Removed from the model | Why it could not stay |
| --- | --- |
| `CoreSettings`, `UiSettings`, `KeybindingSettings`, `TerminalSettings`, `ProjectSettings`, `SecuritySettings` | every field unreached; no consumer anywhere outside `config/` |
| `ConfiguredAiCliProfile::{args, adapter, environment_policy}` | §1's own opening example — `to_ai_cli_profile` read none of the three |
| `ResourceSettings::{max_terminal_output_mb_per_session, max_agent_transcript_mb_per_run, max_file_watch_events_per_batch}` | D6: no field maps to `ProjectResourceLimits` |
| `SecuritySensitiveField`'s six unreached variants | a confirmation gate over a value nothing consumes is a control over nothing |
| `RestrictedDefaultTrust`, `RequiredMultilinePasteConfirmation`, `RequiredDestructiveCommandApproval` | one-valued types guarding fields that no longer exist — see the note below, the protection did **not** go with them |

Added: `[agent] default_profile` (D8), `[resources] agent_run_limit` (D6).

**A refusal and a warning are now different things, and the distinction is load-bearing.** A key
RFC-023's schema really defined, which this build has no consumer for, is a `ConfigDiagnostic`
naming it and saying it has **no effect yet**. A key this parser has never heard of — a typo, or a
key from a newer Tekstide — still warns, unchanged, so forward compatibility survives and PR-045-B's
board line still has a *loaded with warnings* state to render.

### The one place this slice departs from a literal reading of D3', named rather than buried

D3' says every other key is refused "with a diagnostic naming it and saying it has no effect yet."
**Three keys are refused with a different message instead**, and this is a decision the reviewer
should overrule cheaply if it is wrong:

`projects.default_trust`, `terminal.multiline_paste_protection`, and
`security.require_approval_for_adapter_destructive_commands` are the three that responses 266/270
made *unrepresentable in memory* — a field type with exactly one possible value — because each
would bypass a deliberate per-use act another RFC requires. D3' withdraws those fields, which
removes the type-level guarantee along with the field it guarded, leaving the parser's refusal as
the whole of the protection.

Telling a user that `default_trust = "trusted"` **"has no effect yet"** would promise a future
version in which configuration grants workspace trust. That is the opposite of a settled decision,
and it is §1's own failure shape — a lie the user reads — pointed at the reader of a security
setting. So those three keep a message that says *why*, permanently
(`PERMANENTLY_REFUSED_KEYS`, `load.rs`), and three tests in `tests/model.rs` assert the message
does **not** contain "no effect yet" — one test per key, since each is its own RFC's decision.

Nothing is accepted either way; only the sentence differs.

### Required tests, each ablated separately

**One test per withdrawn section, each failing alone** — not a shared table test where any
regression fails the same assertion under another section's name (RFC-046 response 367's lesson,
applied in advance):

- `a_core_key_is_refused_because_nothing_reads_it`, and the same for `ui`, `keybindings`,
  `terminal`, `projects`, `security`, plus `a_withdrawn_agent_key_…` and
  `a_withdrawn_resources_key_…`, and one per withdrawn profile key (`args`, `adapter`,
  `environment_policy`).
- **Ablation A** — removed `("agent", "capture_changed_files")` from `WITHDRAWN_KEYS`: failed
  `a_withdrawn_agent_key_is_refused_because_nothing_reads_it` **alone** (87 passed, 1 failed).
- **Ablation B** — dropped `refuse_withdrawn_free_form_section(&mut root, "ui")`: failed
  `a_ui_key_is_refused_because_nothing_reads_it` **alone**.

**A first attempt at this ablation failed to fail, and the design changed because of it.** The
original `WITHDRAWN_KEYS` listed all ~24 keys including the six fully-withdrawn sections' own.
Removing `("ui", "font_size")` from it changed nothing: the section-level refusal already caught
every key in `[ui]`, so those eighteen rows were data no behaviour depended on — unfalsifiable by
construction. They are gone; the table now holds only `agent` and `resources`, the two sections
where a row is what separates *refuse* from *warn*.

**D2 — the profile id, ablated against the real function:**

- `a_profile_id_the_audit_trail_cannot_record_is_refused` (`[agent.profile."my tool"]`), plus the
  empty and over-128-byte edges, plus `every_character_the_audit_trail_accepts_is_accepted_in_a_
  profile_id` which asserts `AuditReference::new` accepts the id it then feeds to the parser.
- **Ablation** — replaced the `AuditReference::new` call with a hand-written character class
  admitting one extra character (a space): failed
  `a_profile_id_the_audit_trail_cannot_record_is_refused` **alone**. That is what ties the test to
  the real function rather than to a copy of its rules.

**D8:** `a_default_profile_naming_an_undefined_profile_is_refused` and
`a_default_profile_with_no_profiles_defined_at_all_is_refused`. **Ablation** — dropped the
`validate_default_profile` call: both failed, nothing else did.

**D6:** `an_agent_run_limit_of_zero_is_refused`, `an_absent_agent_run_limit_means_unlimited_not_zero`.
**Ablation** — made the zero check unreachable: failed `an_agent_run_limit_of_zero_is_refused`
alone. Absent and zero are kept distinguishable by `Option<u32>` and a dedicated
`take_agent_run_limit`, not by `take_u32` with a sentinel default.

### One thing clippy caught that review would have had to

`refuse_withdrawn_free_form_section`'s first draft used `for field in table.keys() { return Err(..) }`
— a loop that never loops. `clippy::never_loop` rejected it; rewritten as `match table.keys().next()`.
Recorded because the gate caught a real defect in new code, not a style nit.

### A comment corrected before it shipped

The first draft of `parse_and_validate`'s section-ordering comment claimed the diagnostic a user
sees is "the first thing wrong with their file in document order." It is not — TOML tables are not
ordered by document position, and the order is this function's own fixed list. That is §4.1's
recurring shape (a sentence describing something adjacent to what is true) caught in the writing
rather than in review. The comment now says what is actually guaranteed: a fixed order, so the same
document always produces the same diagnostic, and **not** "the first mistake in the file."

### One deliberate priority, pinned by a test rather than left to key ordering

Only one diagnostic is returned per file. When a section carries both a permanently-refused key and
an ordinary withdrawn one, the **permanent** refusal is reported — otherwise a user who asked for
`default_trust = "trusted"` could be told instead about a neighbouring key that is merely early.
The first implementation got this right only because `default_trust` happens to sort before
`open_duplicate_root`; it is now explicit, and
`a_permanent_refusal_is_reported_in_preference_to_a_merely_early_one` uses fixtures where the
ordinary key sorts first in both sections, so alphabetical luck would fail it.

### Three stale doc comments corrected, found by grepping for the deleted names

Deleting types and fields leaves comments that name them — RFC-036's own defect shape. Grepped the
tree for every removed identifier and fixed the three live hits:

- `audit/tests/integration.rs` cited `RestrictedDefaultTrust` as an example of "inert by
  construction"; now cites `ConfigDiagnostic.message` and points at where that property lives.
- `agent_profiles_direction` described modifying a profile's `command`/`args`/`adapter`/
  `environment_policy`; only `display_name`/`command` remain.
- `retention_direction` said it was "shared by both halves" of the retention policy; the other half
  (`max_agent_transcript_mb_per_run`) is withdrawn, so it has one caller now.

RFC-023's own handoff pack still names the withdrawn fields and was **not** edited: it is a closed
document, and this project does not edit closed documents to match a later state.

### Gate

`fmt`, `clippy --workspace --all-targets -D warnings`, `git diff --check`, `rfc_docs_invariants`
(4 tests): clean. Three consecutive full-workspace runs, **output redirected to files** per this
project's own rule that a filtered gate run cannot report a flake: **487 + 4 + 756, green every
time.**

**Flakes, measured and disclosed rather than re-run away.** Earlier in the same session this tree
saw **4 failures in 30 full-workspace runs** — `bind_recovers_from_a_stale_socket_file` (twice),
`is_still_answerable_reflects_the_real_connection_state`, and
`resize_makes_the_pty_the_emulator_and_the_render_path_agree` — all already-registered
load-sensitive tests, none touching configuration. They clustered with a load average this
implementer drove from 4.65 to **28.18** by running suites back to back, including against a second
checkout; 20 consecutive `tekstide-core`-only runs were green, as were 8 consecutive full-workspace
runs in the quietest stretch. A controlled before/after was attempted **twice** via a `git worktree`
and is impossible that way — a worktree's `.git` is a file, so the repository-scan benchmark refuses
to run in one. Dated row added to `test-process-leak.md`, including the failed methodology, so it is
not attempted a third time.
