---
title: "RFC-018: Rendered Paste Protection and Trusted-UI Evidence - QA Evidence"
rfc: "RFC-018"
rfc_file: "../../proposed/018-paste-protection-and-trusted-ui-evidence.md"
status: "PR-018-B: response 169's required fix applied 2026-08-10, awaiting confirmation"
target_milestone: "M9"
created: "2026-08-08"
---

# QA Evidence

Record results here as each slice lands: gate output, ablations with the exact failure they produced, findings, and limitations.

**This file is where results go. It is not where obligations go.** If a slice discovers something a later slice must handle, put it in that slice's entry in `task-breakdown-pr-plan.md` as well — that is what an implementer reads before starting. This project has lost obligations to that gap four times.

## Recording conventions

- **Ablations name the exact failure**, not "the test failed." A specific wrong value is checkable; a green/red result is not.
- **One ablation per property.** An ablation that breaks two things proves neither.
- **A green ablation is a defect in the ablation**, not a pass. PR-017-C's first P1 ablation passed because `Term::set_title` has no grid effect, so blocking it and bypassing it were indistinguishable — the ablation was redesigned around an observable effect.
- **Screenshots state what they prove and do not.**
- **Disclose rather than manufacture.** Declining to produce an artifact, with the reason, is worth more than a staged one.

## PR-018-A — Design and handoff acceptance

Granted by the human owner 2026-08-08 with RFC-018. Handoff pack authored the same day.

## PR-018-B — Paste ingress

Implemented 2026-08-10, not yet reviewed. Against `pr-018-b-paste-ingress.md`'s review gate:

**Starting state, confirmed by enumeration.** Before this slice, `TerminalInputPolicy`/`evaluate` had no production caller anywhere in `crates/tekstide` (confirmed by `grep` before writing any code, per the handoff's own instruction). `terminal_input_policy_evaluate_has_exactly_one_production_call_site` now pins the *current* state mechanically — exactly one `.evaluate(` call site, inside `update`'s `TerminalPasteResolved` handler — so a second classifier growing anywhere else in this crate fails the test by name rather than needing to be caught by review.

**One PTY ingress, enumerated and ablated.** `write_terminal_input` is the one function that calls `TerminalPane::write_input` for real, modal-gated user input; both the pre-existing keystroke arm (`RoutedInput::Terminal`) and the new paste path (`TerminalPasteResolved`, once `evaluate` returns `Allow`) call it rather than writing directly — the shared guard `pr-018-b-paste-ingress.md` asked for ("two arms, two guards, and the second one drifts"). `write_terminal_input_has_exactly_the_three_named_production_call_sites` enumerates every `.write_input(` call site in `shell.rs` and asserts the exact set by name: `write_terminal_input` (the real ingress), plus two pre-existing, already-reviewed synthetic-measurement bypasses this slice did not touch — `update`'s `MeasuredTerminalInput` arm and `launch_measurement_terminal_pane`'s `FLOOD_SCRIPT` write (both RFC-017 PR-017-G, both documented as deliberately bypassing `TextStream`/routing). A fourth call site — a parallel `write_paste`, or a second inline write in a new arm — fails this test.

**Modal exclusivity re-proven with a real paste.** `modal_open_blocks_paste_write_and_closing_it_resumes_delivery` mirrors the existing keystroke test exactly: a real `TerminalPane`, a real modal, a resolved paste that produces zero bytes while the modal is open, then the same target resolved again after the modal closes reaching the PTY — the "resumes afterward" half ruling out "the pane was simply broken" as the explanation for the earlier silence.

**No classification in `crates/tekstide`; every `TerminalPasteClass` exercised against real bytes.** Four tests drive real content through the real `update` handler against a real pane: `single_line_paste_is_allowed_and_reaches_the_pty` and `empty_paste_is_allowed_and_is_a_harmless_no_op` (`Allow`), `multiline_paste_requires_confirmation_and_blocks_visibly` (`RequiresConfirmation`, blocks, notice recorded and non-empty), `control_containing_paste_is_blocked_outright` (`Block`, blocks outright). `a_paste_targeting_a_different_terminal_than_the_one_focused_now_is_blocked` proves the fifth path — `WrongTerminal` — by naming a target that does not match `active_terminal_focus`'s current answer.

**The real `TerminalTrustedUiState` is passed, derived in one place.** `trusted_ui_state` is the sole conversion from `state.modal` to the core type (`trusted_ui_state_is_inactive_without_a_modal`/`_is_active_with_a_modal_open`), and the wrong-terminal/modal-exclusivity tests above prove it is actually wired into `evaluate`'s call, not hardcoded `Inactive`. **Provisional and disclosed**: today there is exactly one modal kind (the `TEKSTIDE_LAYER_DEMO` placeholder), so any open modal maps to `TerminalTrustedUiState::SecurityDialogActive` — the most generic of the five variants, chosen so it does not collide with `PasteConfirmationActive`, which PR-018-C's real dialog will need for itself. The specific variant is cosmetic today (`is_active_or_modal()` treats all four active variants identically); revisit once PR-018-C and RFC-022 give this function real, distinguishable dialog kinds to map.

**The keybinding collides with nothing.** `Ctrl+Shift+V`, checked mechanically against the whole `KeybindingPolicy::linux_mvp()` table (`paste_into_terminal_shortcut_is_a_candidate_that_collides_with_no_other_rule`, `tekstide-core`), the same shape `LaunchTerminal`'s own collision test uses.

**`RequiresConfirmation` blocks, visibly, and the temporary state is recorded here.** PR-018-C does not exist yet. A multiline paste is refused with a real, catalog-driven notice (`terminal-paste-refused`, `$reason = multiline`) rather than silently discarded — proven by `multiline_paste_requires_confirmation_and_blocks_visibly`. This is a deliberate, temporary conservative state, not a permanent design: once PR-018-C's dialog exists, `RequiresConfirmation` should render into it instead of blocking outright.

**Clipboard read is bounded.** `paste_bytes_within_bound` caps clipboard content at 256 KiB, reasoned on paste's own terms (a command, heredoc, or short script/config snippet — not a document, which belongs in an editor, not a PTY), not borrowed from `read_available_bounded_for`'s output-direction cap.

**Response 169 Required, applied**: over-cap content is refused whole, before `evaluate` is ever called, not truncated and then classified. The initial version truncated first, which had two consequences the reviewer named: (1) truncation could change the classification itself — a paste whose only newline sat past the cap truncated to `SingleLine` and was `Allow`ed, bypassing the `RequiresConfirmation` RFC-009 exists to enforce; (2) the concrete harm, a silent partial write — a user believing they pasted a full command while only a truncated prefix reached the shell. Fixed by failing closed: `paste_bytes_within_bound` returns `None` for anything over the cap, and the caller refuses with a new `TerminalPasteRefusal::TooLarge` (`terminal-paste-refused`, `$reason = too-large`) before constructing a `TerminalRuntimeHandle` or calling `evaluate` at all — `evaluate` now always sees a paste's real, complete bytes, never a prefix. `an_oversized_paste_is_refused_whole_and_reaches_neither_evaluate_nor_the_pty` proves both halves in one test: nothing is written, and the refusal is the `TooLarge` notice, not a `Block`/`RequiresConfirmation` from a classifier that never ran.

**Gates**: `fmt --check`, `clippy --workspace --all-targets --all-features -D warnings`, full test suite (503 `tekstide-core` — 1 net new — + 143 `tekstide` — 16 net new, after response 169's fix), `git diff --check`. All passed.

**Not done, correctly**: no dialog (PR-018-C's job — `RequiresConfirmation` blocks conservatively as required). No `paste_blocked` audit producer (PR-018-D's job — no audit rows written for blocks in this slice). No trusted-UI evidence (PR-018-E's job). README's privacy section was checked and needs no change this slice — it already states there is no rendered paste dialog and describes only the existing `plain_terminal_observation` producer; PR-018-D is where a real check against a new producer becomes necessary.

## PR-018-C — The confirmation dialog

Pending implementation.

## PR-018-D — The `paste_blocked` audit producer

Pending implementation.

## PR-018-E — Trusted-UI evidence

Pending implementation.

## PR-018-F — Closeout

Pending implementation.

## Known Limitations

Consolidated at closeout. Carried in from RFC-018's own text, to be restated with evidence:

- **The frozen schema records paste refusals only.** `valid_paste_blocked` requires `outcome == Blocked`, so a paste the user approves has no valid encoding in the family. Not a defect in this RFC; a constraint of RFC-013's frozen v1 schema, and amending it needs the owner.
- **No semantic detection of dangerous pasted commands.** RFC-009 excludes it by design. A classifier that catches some dangerous pastes invites the belief that it catches all of them.
- **Nothing here improves terminal performance.** `NFR-PERF-004`, the three-terminal limit, and the ~374 KB/s output ceiling are downstream of the poll defect and owned by readiness-driven terminal I/O.
