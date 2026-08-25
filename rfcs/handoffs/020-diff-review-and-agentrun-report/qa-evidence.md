---
title: "RFC-020: Diff Review and AgentRun Report Surfaces - QA Evidence"
rfc: "RFC-020"
rfc_file: "../../done/020-diff-review-and-agentrun-report.md"
status: "PR-020-B implemented 2026-08-18 (core: responses 198/199, commits b74d8d5/c92d97e; surface: pr-020-b-report-surface.md, 2026-08-18), accepted. PR-020-C (change review surface) implemented 2026-08-25, accepted with one required item (review response 322); closeout item delivered 2026-08-25. Both RFC-020 surfaces now implemented and reachable."
target_milestone: "M10"
created: "2026-08-15"
---

# QA Evidence

Record results here as each slice lands, with the reasoning that produced them.

**This file holds results. The obligations live in
[`task-breakdown-pr-plan.md`](./task-breakdown-pr-plan.md).** Four obligations in this
project have been lost to that gap — an item recorded only in an evidence file is an item
the next implementer does not read. If a slice discovers a new obligation, put it in the
task breakdown, not only here.

## Conventions

- **Ablation**: break the property, watch the *specific* named test fail, restore. One
  ablation per property. **A green ablation is a defect in the ablation, not a pass.**
- **Positive control**: prove the check reaches real data before asserting what it does
  not find.
- **GUI evidence**: `niri msg action screenshot-window`; synthetic input with
  `env -u WAYLAND_DISPLAY`, `xdotool windowfocus` (not `windowactivate`), always
  `--clearmodifiers`. Compare captures at one window geometry — comparing across
  geometries, or across different *screens*, has produced wrong claims here twice.
- State what each piece of evidence **does not** prove, alongside what it does.

## PR-020-A — Design and handoff acceptance

Granted 2026-08-12 with the pack. RFC-020's four open questions answered in the pack's
README: Option B (owner's decision), no second bound (RFC-024's measured 4 MiB stands),
read-only, and `DiffContent` left owned with its limitation carried forward accurately.

## PR-020-B — The transcript reader, and the AgentRun report surface

**Core (the reader): implemented 2026-08-15, reviewed and accepted (responses 198/199,
commits `b74d8d5`/`c92d97e`).** Surface (the AgentRun report widget): not started — the
reader alone is not this slice's own completion (see `task-breakdown-pr-plan.md`'s own
framing, "a reader with no consumer cannot be shown to be correct"), recorded here as a
checkpoint, not a claim of PR-020-B being done.

**A real, pre-existing panic found and fixed first, in a different RFC's own module.**
Building D2's resynchronization proof required calling `TerminalSecurityParser::parse`
(RFC-017) on a buffer deliberately truncated mid-CSI-sequence — a shape no existing caller
had ever produced, since `parse` currently has zero production call sites in this crate
(confirmed by grep before touching anything). This reproducibly panicked:
`parse_csi`'s `body = &sequence[2..sequence.len().saturating_sub(1)]` underflows when
`take_until_csi_final`'s fallback path (no real CSI final byte found within the scan
window) returns a slice shorter than 3 bytes — reachable with as little as a bare `ESC [`
at a buffer's own end. The existing guard, `let Some(final_byte) = sequence.last().copied()`,
could never catch this: a non-empty slice always has a last byte, so that branch had never
actually triggered for any input, well-formed or not. A first fix attempt (check the last
byte's *value* is in the CSI final-byte range) was also insufficient and caught by this
slice's own tests before committing: `[` (0x5b) is itself inside `0x40..=0x7e`, so a 2-byte
`ESC [` fallback still passed the check and still panicked. The real fix:
`take_until_csi_final` now returns an explicit `found_final_byte` signal instead of leaving
the caller to infer it from the returned bytes. Ablated for real (reverting the fix
reproduces the identical panic message). Committed separately (`c229781`) from the reader
itself (`1c7b980`), since it is a standalone defect in a different, already-shipped RFC's
module, not part of this RFC's own deliverable — flagged prominently here rather than
folded quietly into the reader's own commit.

**D1 — the window size, measured, not estimated.** **Correction (response 198, Finding
2): the sweep below is wrong and superseded — left in place, annotated, per this project's
own evidence-correction convention, rather than silently rewritten.** It varied only the
window in isolation and never allocated the mandatory `MAX_SCAN_BYTES` (32 MiB) scan buffer
`read_window` always fills on every call, understating real peak RSS by roughly an order of
magnitude:

```text
mib=1 window_len=1048576  escaped_len=1572864  rss_delta_kb=2572
mib=2 window_len=2097152  escaped_len=3145728  rss_delta_kb=7172
mib=4 window_len=4194304  escaped_len=6291456  rss_delta_kb=12296
mib=8 window_len=8388608  escaped_len=12582912 rss_delta_kb=24584
```

**Corrected measurement**, against a real on-disk file at the writer's own 32 MiB retention
ceiling, opened and `read_to_end`'d into a `Vec::with_capacity(total_len)` scan buffer
exactly as `read_window` does, plus the window's own content copy, plus a simulated escaped
copy alongside it:

```text
mib=1 scan_len=33554432 content_len=1048576 escaped_len=1081344 rss_delta_kb=34988
mib=2 scan_len=33554432 content_len=2097152 escaped_len=2162688 rss_delta_kb=38860
mib=4 scan_len=33554432 content_len=4194304 escaped_len=4325376 rss_delta_kb=43020
mib=8 scan_len=33554432 content_len=8388608 escaped_len=8650752 rss_delta_kb=51692
```

Real peak for a full-size transcript is ~33-50 MiB, dominated by the fixed 32 MiB scan
buffer — every call pays that cost regardless of the requested window. **1 MiB remains
chosen**, but not for the "trivial memory cost" reason the wrong figure gave: since the
scan buffer's fixed 32 MiB dominates every candidate size, the window choice cannot
meaningfully change *peak* memory, only the smaller marginal cost on top, where 1 MiB is
still cheapest. The window is chosen for what it always was: 1/32nd of the retention
ceiling is meaningfully a window, not "basically the whole transcript," and at ordinary PTY
text density is tens of thousands of lines, far more than a report view could usefully show
on one screen. Unlike RFC-024's bound, not reused from an existing standard by analogy (a
transcript tail is not shaped like a whole edited file or a single paste) — a fresh,
measured number. Full doc comment and methodology in
`crates/tekstide-core/src/transcript/reader.rs`, on `DEFAULT_TRANSCRIPT_WINDOW_BYTES`.

**D2 — resynchronization, proven against real captured PTY output, not a synthesised
fixture.** `a_window_starting_inside_a_real_control_sequence_classifies_identically_to_the_whole`
spawns a real shell via the same `LinuxTerminalRuntime` harness `runtime::terminal::tests`
already uses, runs a real `printf` that emits a genuine SGR escape sequence, and captures
the raw PTY bytes. Phrased as a splitting invariant (`TerminalSecurityParser::parse` does
not expose per-effect byte offsets, so this avoids needing to reconstruct them): splitting
the real captured bytes at the *resynchronized* boundary and parsing each half separately
equals parsing the whole buffer in one call; splitting at the *raw, non-resynchronized*
offset does not — both checked against the identical fixture, so the property that broke
and the property that holds are demonstrated against the same real bytes, not two
different ones. **Ablated** (`ablation_without_resynchronization_the_split_misclassifies`):
skipping the resynchronize call and splitting at the raw offset reproduces the divergence
directly. The delivered start offset (`TranscriptWindow::delivered_start()`) is reported
distinctly from the requested one (`requested_start()`) in the type itself.

**No UTF-8 scalar split**, proven with a real 2-byte scalar (`é`) and a target offset
landing on its second byte — `resynchronization_never_splits_a_utf8_scalar`.

**D3 — raw bytes survive**, proven against the same bidi/format-character probe
`text_safety`'s own tests use (`raw_bytes_survive_the_reader_including_bidi_and_format_characters`)
— the reader never calls `quote_untrusted`.

**D4 — read-only, by enumeration.** `only_this_module_opens_a_transcript_file_for_reading`
scans `tekstide-core` for `transcript_file()` combined with a raw byte-open, against a
closed one-entry allowlist (`transcript/reader.rs` itself; `transcript/writer.rs` is
excluded by name, since it opens the file for *writing*). RFC-024's own, broader
enumeration test (`project::diff::tests`) updated to disclose this module's one call site
too, since its own scan is now broad enough to also catch transcript reads.

**D5 — complete vs. still-being-written, in the type.** `TranscriptWindow::Complete`/
`::StillBeingWritten` are separate constructors (matching `DiffContent`'s own precedent),
selected by a caller-supplied flag — nothing on disk distinguishes a live process paused
between writes from a finished transcript, so this cannot be inferred from the file alone
and is not guessed at. Proven by `still_being_written_threads_into_the_returned_variant`.

**Correction (response 198, Finding 1): an oversized transcript now refuses rather than
returning the wrong window.** Before the fix, `total_len > MAX_SCAN_BYTES` had no guard: the
reader would read the first 32 MiB and return a window near the end of *that prefix* — the
middle of the real file, mislabelled as the tail. `total_len` reported the file's true size
while `requested_start`/`delivered_start` were offsets into the truncated buffer: internally
consistent, and inconsistent with the file they claimed to describe. Fixed with a new
`TranscriptReadErrorReason::TranscriptExceedsScanLimit`, checked immediately after reading
`total_len` and before any buffer allocation, proven by
`a_transcript_larger_than_the_scan_limit_is_refused_not_silently_windowed` (writes a
`MAX_SCAN_BYTES + 1`-byte file directly, bypassing `BoundedTranscriptWriter`'s own retention
limit, which would otherwise prevent creating a file this large).

**Correction (response 198, Finding 3): `read_window` now documents why it always scans
from byte 0.** Reading from offset 0 to serve a small tail window looks like an
optimizable inefficiency; it is load-bearing. Resynchronization (D2) walks tokens forward
from a position guaranteed to be a sound parse origin, and the file's true start is the only
position with that guarantee — seeking near the requested tail before scanning would put the
scan's own starting point at an arbitrary, possibly mid-sequence offset, reintroducing one
level down the exact defect D2 exists to prevent. Doc comment added directly on
`read_window`, next to `resynchronize`, so a future reader does not "fix" it with a seek.

**Path safety reused, not duplicated.** `TranscriptStoragePath::is_safe_for_read` delegates
to the existing `is_safe_for_write` containment check (identical logic, a name that does
not misdescribe why read-only code calls it) rather than either calling a write-named
method from read code or duplicating the same four `path_contains` calls under a second
name that could drift from the first. `an_unsafe_storage_path_is_refused_before_any_read`
proves the refusal happens before any file I/O.

**Gates**: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets
--all-features -- -D warnings`, full workspace suite (`tekstide-core` 547 passed, up from
531 — 16 new tests across the panic fix, the reader, and response 198's oversized-transcript
regression test; `tekstide` 206 passed, unchanged — no `crates/tekstide` changes, matching
"core first" sequencing), `git diff --check`. All clean, re-run after the three corrections
(commit `b74d8d5`).

**Not done in this checkpoint, and blocked rather than merely deferred (response 200,
2026-08-15)**: the AgentRun report surface itself (the widget, the escaping at the point of
rendering, the reader-window-vs-writer-truncation rendered distinction, the no-double-escaping
proof). Before writing any surface code, request 200 asked which `AgentRun` the report
should show; response 200 found no `AgentRun` can exist in production today —
`launch_agent_run_with_runtime` and `add_agent_run` have zero production callers (grep
independently re-verified against `agent/tests.rs`/`project/tests/*.rs` only), and
`NavigationAction::OpenCurrentAgentRunDetail` maps to `None` in `shell.rs`'s
`app_command_for` (verified at `shell.rs:1598-1620`). No adapter-spawn pathway exists to
create one. Building the report against this would render "nothing here" forever. No
surface code was written; nothing here claims PR-020-B complete. Full detail in
`task-breakdown-pr-plan.md`'s PR-020-B section.

### Surface: implemented 2026-08-18 (`pr-020-b-report-surface.md`)

The blocker response 200 found no longer holds: `0.10.0` made agent-run launch reachable
(RFC-032 grants trust, RFC-022 spawns), and `0.11.1` traced the rest of the chain end to end
(transcript written for every AI CLI run → registered on the project →
`AgentRun.transcript_ref` → `Transcript.storage_path` → `TranscriptPathResolver::resolve_agent_run`
→ `read_window`), all real production code reached by a real key press. This checkpoint
builds the surface against that real chain.

**"Which `AgentRun`" answered**: the most recently launched run in the active project
(`project.agent_runs().last()`) -- matches `OpenCurrentAgentRunDetail`'s own name,
`agent_run_limit` bounds how many can exist, and a selector is a second surface with its own
navigation decisions this slice was not for.

**Reachability, both gaps closed in the required order.** `content_mode_view` previously fell
through to the plain editor for `AgentRunDetail` (a binding without a render arm would have
made the key silently open an editor -- worse than a dead key); the render arm landed first,
then the binding (`Ctrl+Alt+R`, mechanically checked to collide with nothing, the same shape
`approval-history-binding.md` established). `surface_renders_editor` classifies
`AgentRunDetail` alongside `ApprovalHistory` (`false`) rather than `TrustSettings` (`true`) --
a deliberate choice, not a default: this is a pure read-only report with no interactive
elements of its own, so a document left open in the background must not keep absorbing
keystrokes underneath it, unlike `TrustSettings`'s own real Enter-driven actions.

**A real bug found and fixed while building the reachability test, not merely disclosed.**
The first version of the read-side lookup (`agent_run_transcript_window`) called
`open_real_agent_run_state_root()` internally, exactly mirroring the launch side's own
pre-`transcript-capture-evidence` shape. Every real production call (write at launch, read at
report time) resolves the same real `$XDG_STATE_HOME`, so this is invisible in production --
but it made the read side untestable with an injected state root, and the reachability test
below failed for real (`ReadFailed`, the real path never existing) against a transcript
captured under a test-injected root, before the fix. Split into
`agent_run_transcript_window`/`agent_run_transcript_window_with_state_root`, the identical
testability shape `transcript-capture-evidence.md` already established for the launch side --
production calls the plain wrapper; the reachability test supplies the same state root its own
launch used. This is the ablation: the bug's own presence made the real test fail with the
real wrong error, and the fix made it pass, observed directly rather than staged separately.

**Escaping, at the widget, using the one existing primitive.** `agent_run_detail_transcript_body`
lossy-decodes `TranscriptWindow::content()`'s raw bytes (D3: the reader never escapes) and
calls `text_safety::quote_untrusted` -- no second escaping primitive. Proven directly
(`transcript_body_escapes_a_real_override_and_does_not_double_escape_literal_marker_text`): a
real `U+202E` becomes a visible `<U+202E>` marker and the raw override character never
survives into the rendered text; literal ASCII text that already reads `<U+202E>` passes
through completely unchanged (`escape_untrusted_chars` only touches Unicode Control/
Default-Ignorable characters, none of which a plain `<`/`U`/`+`/digit/`>` sequence contains).
**What this test does not, and could not, prove**: that the two cases render as visually
*different* text -- they cannot, and that is `quote_untrusted`'s own already-proven contract,
not a property this widget could change. What it proves instead is the concrete, checkable
shape "double escaping" would take here: the isolate wrapping (`quote_untrusted`'s own
FSI/PDI marks) never itself appears as escaped text in either case, which is what a second
escaping pass over already-escaped content would produce (FSI/PDI are themselves Unicode
Format characters). No dedicated ablation of the escaping call itself: `DisplayText` has no
public constructor other than `quote_untrusted` (`text_safety`'s own design, "if the type
system can make untrusted text unrenderable without passing through this function, prefer
that"), so "forgot to escape" is not expressible in a value this function's own return type
accepts -- a stronger guarantee than a staged ablation would add, not a gap.

**Reader window vs. writer truncation, rendered as two independent, mechanically-checked
notices**, per this same file's own known-limitation entry below. `agent_run_detail_notices`
tested directly against constructed `Transcript`/`TranscriptWindow` values, no real file
needed: a full, `Complete`, untruncated window produces exactly two notices (status, window);
a partial reader window (`delivered_start() > 0`) produces a *different* window notice than
the full case; independently, marking the same `Transcript.truncation_state` as `Truncated`
adds a *third*, and that third notice is asserted textually distinct from the window notice --
the exact conflation `the-window-boundary.md` names as the failure mode.

**A real transcript from a real run, from a real key press.**
`a_real_key_press_opens_the_report_surface_and_reaches_a_real_transcript`: launches a real
`Supervised` profile (`claude_code_linux_default`'s own compatibility level, not the `Managed`
reference adapter) against the marker script `transcript-capture-evidence.md` added, waits for
the real marker to land in the real transcript through the exact production lookup the view
calls, then dispatches the real `Ctrl+Alt+R` key press through `update` (`shell_input_for_test`,
not a bypass) and confirms `open_surface()` is `AgentRunDetail` and the same production lookup
still succeeds and still contains the real marker after escaping.

**GUI evidence.** `niri msg action screenshot-window` against a real running instance, a fresh
project with zero agent runs (the no-run case, reachable without a real AI CLI installed).
This project's own documented convention (`env -u WAYLAND_DISPLAY`, `xdotool windowfocus`,
`--clearmodifiers`) did not work in this session's environment -- `xdotool search` found no
window (native-Wayland niri, no XWayland surface for the title to match) -- disclosed rather
than silently substituted: `wtype -M ctrl -M alt r -m alt -m ctrl` (native Wayland virtual
keyboard) was used instead, confirmed to actually deliver the keystroke (the earlier
`xdotool`-based attempt at the same capture visibly did not: the resulting screenshot showed
the unchanged Project Board, not the report surface, and is not used as evidence). The capture
shows the report surface open, focused (blue border), rendering `"No agent run in this
project yet."` -- the real catalog text, the real no-runs branch, not empty chrome. **What
this does not prove**: a real transcript rendering with real content in the live GUI (would
need a real AI CLI or the test-only marker script wired into the shipped binary, neither
available for a manual capture) -- that path is covered by the automated end-to-end test
above instead, not by this screenshot.

**Gates**: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --all-features
-- -D warnings`, full workspace suite (`tekstide-core` 615 passed; `tekstide` 297 passed -- 8
new tests: the collision check, the reachability test, the escaping test, the notices test,
the no-runs precondition test, plus the four already counted for
`transcript-capture-evidence.md`), run three times for stability, `git diff --check`. All
clean.

**Not done in this checkpoint**: the change review surface (PR-020-C) and closeout
(PR-020-D). This slice's own closing claim, stated precisely per its own gate: it makes the
AgentRun report **buildable and now actually reachable** -- a real user pressing `Ctrl+Alt+R`
in a trusted project after a real agent run sees real transcript content, escaped. It does
**not** render what an agent *changed*, only what it *said* -- that is PR-020-C, still
blocked on its own `DetectedChanges` projection. It does not establish that the real Claude
Code CLI behaves well under this surface; every test uses a controlled executable, as
everywhere else in this project.

## PR-020-C — The change review surface

Implemented 2026-08-25 per the 2026-08-25 handoff (`change-review-surface.md`). Reviewed and
accepted, one required item, response to review request 322 —
`.git-exclude/reviewed/tekstide-review-request-322-change-review-surface-response.md`.

**Scope**: metadata only — file paths, a count, detection status, review state — from
`ChangeSet::default_summary()`. No diff content (blocked on retaining `DetectedChanges` past
`add_detected_generated_change_set`, per RFC-020's own scoping addendum — recorded as future
work, not built). No approve/reject affordance; RFC-034's own job.

**What shipped**:

- `ProjectOpenSurface::DiffReview` gets a real render arm (`change_review_view`), moved from
  `surface_renders_editor`'s `true` arm to `false` alongside `ApprovalHistory`/`AgentRunDetail`.
- A real button, "Change Review", on `trust_settings_view`, alongside Launch AI CLI Run /
  AgentRun Report / Approval History.
- `Ctrl+Alt+D`, `KeybindingStatus::Candidate`, unclaimed proven mechanically
  (`open_diff_review_shortcut_is_a_candidate_that_collides_with_no_other_rule`). Both the button
  and the keystroke converge on one function, `open_diff_review` — the same "one setup, two
  routes" shape `open_folder_browser` established. RFC-036's dead-action count: three to two
  (`CycleVisibleTerminalSession`, `OpenSafeCloseDialog` remain).
- The two required disclosures render **on the surface**: not-all-changes (metadata-only,
  conservative, excludes `.git/`, `target/`, `node_modules/`) and not-a-review/approval/safety-claim.
  `ChangeDetectionStatus::Partial{limit}`'s scan-level truncation renders as a line distinct
  from `omitted_changed_file_count`'s display-level truncation — both, when both are true, not
  collapsed.
- File paths escaped via `text_safety::quote_untrusted` before display (the bidi fixture
  applies, tested).

**Tests** (`crates/tekstide/src/shell/tests.rs`): bidi/escaping for the file-entry line;
distinct-rendering for all five `ChangeDetectionStatus` variants and all five `ReviewState`
variants; click-reachability; keyboard-reachability; modal-exclusivity (extends RFC-040's own
pattern to this surface's new control); and a heavy end-to-end test
(`change_review_surface_renders_a_real_change_set_from_a_real_agent_run`) reusing the real
managed-agent-run/real-file-write/real-approval/real-exit pipeline already proven for
`ChangeSet` creation, extended through a real click on the real button and asserted against the
real rendered strings.

Plus the five test breakages the handoff predicted (`control_coverage`, the binding-count test
13→14, the dead-actions list, `click_message_kind`, the keyboard-help catalog key), and two a
full-suite run found that the handoff did not name:
`navigation::tests::advertised_bindings_are_exactly_the_live_ones` (a hardcoded ordered binding
list) and `shell::tests::opening_help_through_a_real_key_event_shows_every_live_binding` (a
hardcoded count). All fixed. The reviewer's own ablation (collapsing `Partial`'s symbol into
`complete`) confirmed the truncation-distinction test actually guards the property, not merely
intends it.

**Gates**: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --all-features --
-D warnings`, full workspace suite (`tekstide` 413 passed, `tekstide-core` 737 passed), run
three times for stability, `git diff --check`. All clean, both rounds (initial slice and the
closeout addition below).

**GUI evidence, round 1 (review request 322)**: real release binary, real project directory,
`niri`/`xdotool`. Cold start with an empty project: "Trust Settings" shows the real "Change
Review" button as the fourth item. Clicking it reaches the real surface's real empty state, "No
changes have been detected in this project yet." `Ctrl+Alt+D` from the Project Board reaches the
identical rendered surface, confirming both routes converge. `Ctrl+Alt+K` lists `Ctrl+Alt+D`
among all fourteen live bindings. **Did not show a populated surface** — the substitution
disclosed instead was the heavy end-to-end test above, string-level rather than pixel-level.

**Required by review response 322**: "someone must see the populated surface" — string-level
proof is not the same claim as a screenshot, and this arc has found three defects by clicking
that the suite did not catch.

**GUI evidence, round 2 (closeout) — the populated surface**: rather than drive a full live
agent run through raw `xdotool` automation against a real subprocess's real timing (judged
fragile in review request 322, and not asked for again by the reviewer, who called the
end-to-end test "stronger evidence than most live walkthroughs"), a small env-gated seeding
path, `TEKSTIDE_CHANGESET_DEMO` (`seed_change_review_demo_data` /
`seed_change_review_demo_change_set` in `shell.rs`), calls the same three real production
functions `attempt_generated_change_detection` calls at a real agent run's exit —
`capture_filesystem_baseline`, `detect_filesystem_changes`, `add_detected_generated_change_set`
— against one real file it seeds into the real project root, with no `agent_run_id` (honest:
no agent run produced this write). Same env-gated-demo convention as `TEKSTIDE_LAYER_DEMO`/
`TEKSTIDE_TERMINAL_DEMO`; the env check and the seeding are two functions, not one, so the
seeding logic is tested directly rather than by setting the env var
(`seed_change_review_demo_change_set_creates_a_real_unlinked_change_set`,
`seed_change_review_demo_change_set_is_a_no_op_without_an_active_project`) — process-global env
vars race against concurrently-running tests, the same reasoning
`measurement_and_the_demo_modal_are_mutually_exclusive` already documents.

Real release binary launched with `TEKSTIDE_CHANGESET_DEMO=1` against a fresh project directory:
the real file `tekstide-changeset-demo.txt` appears in the real explorer, and clicking "Change
Review" renders the real populated surface — heading, disclosure, "Detection: Complete", "1 file
changed", the real filename, "Review state: Unreviewed". This proves the render arm against real
content end to end, visually, closing the gap the reviewer named. **Does not** prove the surface
against a `ChangeSet` genuinely produced by a real agent run being visually observed live — that
remains string-level only (the end-to-end test above), a disclosed substitution, not a silent
one.

## PR-020-D — Closeout

Folded into PR-020-C's own evidence above rather than a separate slice — RFC-020's two surfaces
(AgentRun report, change review) are both now implemented, reviewed, and reachable. See "Known
limitations, consolidated" below for what remains disclosed rather than fixed, and the README /
RFC-020's own text for the corrections this work required (both previously said "you cannot see
what an agent changed" — no longer true, and both have been corrected rather than left stale).

## Known limitations, consolidated

- **No two-sided diff for a modified file, and no diff content at all.** The before-bytes
  were never captured (`ReviewBaselineEntry` is metadata-only by RFC-012 §Design Principles
  2) and are gone, not merely unretained, by preview time. Reading actual diff content
  (`DetectedChanges`/`read_diff_content`) is additionally blocked on retaining
  `DetectedChanges` past `add_detected_generated_change_set`, which currently discards it —
  recorded by RFC-020's own scoping addendum as future work, not built in this slice.
- **Detection is metadata-only and conservative**; the change set may be incomplete.
  `.git/`, `target/` and `node_modules/` are excluded by design, so a change an agent makes
  inside them is never reported. Stated on the Change Review surface itself, not only here.
- **`DiffContent` blocks two specific storage paths**, not general retention — a consumer
  can destructure it and keep the bytes.
- **The transcript window is a view, not the whole transcript**, and is distinct from the
  writer's retention truncation.
- **No Git-backed before-source exists**; it is gated behind RFC-012's unmet safety
  evidence.
- **The Change Review surface has never been visually observed populated by a `ChangeSet`
  a real agent run produced.** The real pipeline is proven end to end at string level
  (`change_review_surface_renders_a_real_change_set_from_a_real_agent_run`); the live
  screenshot of a populated surface uses `TEKSTIDE_CHANGESET_DEMO`'s seeded, non-agent-run
  `ChangeSet` instead. Disclosed, not silent — see PR-020-C's own evidence above for why a
  live real-agent-run walkthrough was judged impractical to orchestrate reliably.
- **No approve/reject/accept action of any kind on the Change Review surface.** RFC-034's
  own job, deliberately not built here.
- **`TEKSTIDE_CHANGESET_DEMO` writes a real file into the real, user-owned project root and
  never removes it — required disclosure, review response 323.** Unlike `TEKSTIDE_LAYER_DEMO`
  and `TEKSTIDE_TERMINAL_DEMO`, which touch no filesystem at all, this variable drops
  `tekstide-changeset-demo.txt` into whatever project is open on every launch it is set for.
  Inherent to what it proves (detection has to see a change *in the project*), not incidental
  — but do not set it in a shell profile or a CI environment and leave it set. Also stated in
  `seed_change_review_demo_data`'s own doc comment.
