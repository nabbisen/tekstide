---
title: "RFC-040: QA evidence"
rfc: "RFC-040"
rfc_file: "../../accepted/040-affordance-completion.md"
source_rfc_status: "Accepted 2026-08-25 — M12, first of three"
target_milestone: "M12"
created: "2026-08-25"
---

# QA evidence

One section per PR. Cite the command that produced each result.

Screenshots in `evidence/pr-040-<letter>/` with the launch command beside them;
`../first-run-correction/evidence/cold-start-empty-board.md` is the reference for form.

## PR-040-A — the audit as a test

**Build.** `keyboard_help::control_coverage` (`#[cfg(test)]` -- this is audit infrastructure, not
production logic; nothing outside the test suite has a reason to ask "does this action have a
visible control"), exhaustive over `NavigationAction` the same way `action_catalog_key` already
is, mapping every live action to either `ControlCoverage::VisibleControl { description,
on_press_snippet }` (a real button's own literal `.on_press(Message::Variant` text) or
`ControlCoverage::KeyboardOnly(reason)`. Three actions have a real control today
(`OpenProjectBoard`, `SwitchActiveProject`, `OpenFolderBrowser` -- all three built by RFC-038/039);
two are permanent, reasoned allow-list entries (`PasteIntoTerminal`, D3's own convention;
`OpenProjectEntryField`, whose *workflow* the Browse button already serves even though the
*action* has none); the remaining eight carry `"no visible control yet -- tracked for RFC-040
PR-040-C"`, honest about what does not exist rather than a placeholder. This is the write-the-
allow-list-before-anything-depends-on-it step the README/task-breakdown both required.

**Two tests, matching the two required properties.**

- `no_click_mechanism_other_than_button_on_press_exists_anywhere_in_the_crate` -- the premise:
  `mouse_area`/`MouseArea`/`.on_click(` absent from every scannable source file, the same shape
  `no_raw_color_construction_anywhere_in_the_crate` already established for a different premise.
- `every_live_action_has_a_visible_control_or_a_reasoned_allow_list_entry` -- the coverage: every
  `Candidate` rule with a binding is looked up in `control_coverage`; `VisibleControl` entries are
  checked against the real, current source (their `on_press_snippet` must actually appear
  somewhere in the crate); `KeyboardOnly` entries are checked for a non-empty reason; anything
  missing from the match at all is a `None` the exhaustive match cannot produce for a live action
  without a compile error first.

**Security/correctness note, found by the ablation itself.** The first version of the coverage
test scanned every file `scannable_source_files()` returns, including `keyboard_help.rs` --
which is where every `on_press_snippet` string literal is *defined*. That made the check vacuous:
searching the whole crate for a string that is guaranteed to appear in its own definition site
always finds it, regardless of whether the real button it names still exists anywhere else. Caught
by running the required ablation (below), not by inspection -- the test passed even after the
snippet was replaced with one that does not exist. Fixed by excluding `keyboard_help.rs` from the
scan for this one check; the premise test above still scans it (a stray `mouse_area` there would
be just as real a violation).

**Ablations.**

- Replaced `SwitchActiveProject`'s own `on_press_snippet` with a string that exists nowhere in the
  crate, ran the coverage test: failed, naming `SwitchActiveProject` by variant, with the false
  snippet quoted in the panic message. This is the same run that first exposed the
  `keyboard_help.rs`-self-match bug above -- the *first* attempt at this ablation passed when it
  should have failed, which is what caught the bug; the version recorded here is the one that
  correctly fails, after the fix. Reverted.
- Appended a line containing the literal substring `mouse_area` to `surface/editor.rs`, ran the
  premise test: failed, naming that file. Reverted (`git checkout --`).

**Gates.** `cargo build`, `cargo fmt --all --check`,
`cargo clippy --workspace --all-targets --all-features -- -D warnings` -- clean.
`cargo test --workspace --all-targets --all-features` (385 tekstide + 734 tekstide-core, up from
383/734; two new tests, no others changed): three consecutive runs under default parallelism, per
the checklist's own explicit requirement -- run 1 failed
`command_approval_family_produces_real_durable_audit_records_through_the_pipeline`, runs 2 and 3
clean. **Disclosed, not investigated further**: this is the flake `test-process-leak.md`'s own
table already names (one of its original four) and RFC-039's own `qa-evidence.md` already
recorded a second, distinct cause for (a shared-`AuditStore` query-race under parallel load,
response 312) -- this occurrence is neither new nor caused by anything in this PR, since neither
new test in this slice touches the audit store at all. `git diff --check` clean.

## PR-040-B — modals get buttons

_Pending._

## PR-040-C — visible controls

_Pending._

## PR-040-D — closeout

_Pending._

## Known limitations (RFC-040-wide)

_Pending._
