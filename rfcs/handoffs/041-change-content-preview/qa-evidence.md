---
title: "RFC-041: QA evidence"
rfc: "RFC-041"
rfc_file: "../../accepted/041-change-content-preview.md"
source_rfc_status: "Implemented 2026-08-25, both slices"
target_milestone: "M12"
created: "2026-08-25"
---

# QA evidence

One section per PR. Cite the command that produced each result.

## PR-041-A — retention and reaching the gate

**D1, as built**: `state.detected_changes_by_change_set: HashMap<ChangeSetId, DetectedChanges>`
on `tekstide`'s GUI-level `State` — not a field on the persisted `ChangeSet`, and not inside
`tekstide-core::ProjectSession` either. Session-scoped in the same sense
`agent_run_change_baselines` already is (an existing precedent this slice follows rather than
invents): in-memory only, does not survive the application closing, the same disclosed
limitation. `attempt_generated_change_detection` (`shell.rs`) is the one call site with a real
`DetectedChanges` to retain — it now captures the `ChangeSetId`
`add_detected_generated_change_set` returns and moves `detected` into the map keyed by it, only
when a real `ChangeSet` was actually created (a run whose detection produced no `ChangeSet` has
no id to key on, and nothing worth retaining).

**Reused, not re-derived**: `read_diff_content`/`gate_diff_content_read`/`diff_content_is_stale`
are called exactly as RFC-024 built them, from `select_change_review_file` and
`change_review_content_lines` (`shell.rs`) — no second gating path. `DiffPreviewPolicy::default()`
used directly; RFC-024's bound (4 MiB), refusal-not-truncation, non-text classification ordering,
and staleness machinery are all untouched.

**Tests**:
- `change_review_content_is_unavailable_when_retention_was_dropped` — retention removed after
  selection; content preview renders its own honest refusal (`change-review-content-unavailable`)
  while `summary.changed_file_count` (metadata, from `ChangeSet` alone) stays exactly correct.
  This **is** D1's own required ablation, encoded as a real, permanent test rather than a manual
  before/after cycle — dropping a `HashMap` entry is genuinely testable code, unlike RFC-035's
  `GIT_WATCHED_ENTRY_NAMES` const, which needed a hand-edit ablation because nothing runtime-driven
  could flip it.
- `change_review_surface_shows_real_content_from_a_real_agent_run` — the real production call
  site, end to end: a real managed agent run writes a real file, a real exit runs
  `attempt_generated_change_detection` for real, and the retained `DetectedChanges` is what a
  later real click actually reads from.

## PR-041-B — rendering

**Design**: content is **never stored in `State`**, at any point past selection.
`ChangeReviewSelection` (new, on `State`) holds only `change_set_id`, `relative_path`, and an
`Option<FileSnapshot>` baseline — no bytes, no `DiffContent` (which cannot be stored regardless:
derives neither `Clone` nor `Serialize`). `select_change_review_file` performs exactly one real
read, purely to capture whatever baseline it produces; the bytes from that read are used for
nothing and dropped immediately. `change_review_content_lines` re-runs `read_diff_content` **fresh
on every render** while a selection is active — `what-a-content-preview-must-not-claim.md` §3's
own instruction ("if rendering seems to need content in state, that is the design telling you
something: render from a value that lives for the request") taken literally: the "request" is one
render call, not the whole time a preview stays open.

**Per change kind** (RFC-024's own classification, reused): Added → whole content, no label;
Modified → current content **with** the "not a diff" label; Deleted → the fact of deletion;
NonTextContent → size only, no read attempted; NonFile → kind only. Every gate refusal
(`PathNotDetected`/`Access`/`MetadataUnavailable`/`TooLarge`) and `ReadFailed` get their own named
line.

**Reachability**: each shown file is a real `iced::widget::button`
(`Message::ChangeReviewFileRowPressed`), converging with `handle_change_review_key`'s own `Enter`
case on `select_change_review_file` — RFC-040's "one setup, two routes" shape. `ArrowUp`/`ArrowDown`
move `change_review_highlight`, the same cursor-then-activate shape
`explorer_highlight`/`approval_history_highlight` already establish; the `"> "`/`"  "` prefix
convention matches `board::highlighted_row_lines`. **Not a `NavigationAction`** — no global
keybinding names a single row, the same reason explorer/approval-history row activation have no
`control_coverage` entry either; `click_message_kind` classifies the new message as
`BackgroundControl` instead, which is the mechanism that actually governs it.

**Tests**:
- `change_review_content_modified_content_is_labelled_not_a_diff` /
  `change_review_content_added_content_has_no_not_a_diff_label` — the label appears exactly where
  RFC-024's own classification says it must and nowhere else.
- `change_review_content_refuses_when_the_file_changes_after_selection` (D2) — a real, later
  write after selection produces the real staleness refusal, naming the reason, and the newer
  content never renders under the older selection.
- `change_review_content_escapes_a_bidi_override_in_file_content` (§5) — a real bidi override in
  real file content renders as the escaped marker, never raw.
- `clicking_a_change_review_row_selects_it_for_preview`,
  `change_review_key_navigation_selects_the_highlighted_row_on_enter`,
  `clicking_a_change_review_row_while_a_modal_is_open_has_no_effect` — reachability and modal
  exclusivity, the same shapes every other RFC-040-pattern control in this codebase already
  proves.
- `change_review_surface_shows_real_content_from_a_real_agent_run` — **the acceptance criterion,
  end to end**: real managed launch, real approval, real exit, a real click on the real row
  button (`Message::ChangeReviewFileRowPressed` dispatched through `update`, not
  `select_change_review_file` called directly the way the unit tests above do for speed), the
  real rendered lines asserted against the real bytes the agent actually wrote.

**Ablated** (ties to `what-a-content-preview-must-not-claim.md`'s own italicised warning): removed
the "not a diff" label push from the `Modified` arm — `change_review_content_modified_content_is_labelled_not_a_diff`
failed exactly as expected (`got ["Preview: ...", "\u{2068}after<U+000A>\u{2069}"]`, no label
line). Reverted; full `change_review` test group re-run green.

**D3**: `DiffContent`'s `Debug` is hand-written — kind and length only, never bytes, for
`Added`/`Modified`/`NonTextContent` alike; `Deleted`/`NonFile` print their `kind` (never
content-bearing regardless). Ablated by hand (temporarily printing `bytes` directly for `Added`):
`diff_content_debug_never_prints_file_bytes` failed with a 21,047-character debug string for a
5,200-byte payload versus the redacted 255; reverted, green. The move-out gap is documented at
the type (`DiffContent`'s own doc comment in `diff.rs`), not closed — RFC-041 D3's explicit
instruction, since closing it needs a lifetime-bound `DiffContent`, a larger change than this
slice takes on.

**Escaping**: `change_review_content_body_text` converts bytes via `String::from_utf8_lossy` (the
gate's own NUL-sniff, not a strict UTF-8 decode, means bytes are not guaranteed perfectly valid
UTF-8 even when classified as text) then `quote_untrusted`, the same discipline every other
untrusted string on this surface already uses. `what-a-content-preview-must-not-claim.md` §5's own
carve-out honoured: only the achievable half is asserted (a real override renders as a visible
marker) — no test claims literal `<U+202E>` text is distinguishable from a real override, which
RFC-020 already established is impossible under this project's escaping design.

## i18n enforcement, an incidental finding

Adding `$len`/`$max` as new Fluent variables broke
`i18n::enforcement::every_source_locale_key_resolves_in_every_shipped_locale` — its own
`generic_args()` fixture provides a fixed set of variable names for every key to resolve against
across every shipped locale, and `len`/`max` were not among them. Fixed by adding both to
`generic_args()` (`i18n/enforcement.rs`), following the exact precedent already documented there
for `shown_len`/`total_len`/`bytes`. Not a defect in this slice's own keys — the enforcement test
did precisely its job.

## Live GUI evidence

Release binary, real project directory, `niri`/`xdotool`. To demonstrate the "Modified" case (the
demo seed only ever writes to one fixed path, `tekstide-changeset-demo.txt`, so a file already
present at that path *before* launch — with different content — is enough to make the seed's own
overwrite land as a real, detected modification, not a new addition): pre-created that file with
different content, then launched with `TEKSTIDE_CHANGESET_DEMO=1`.

**A real defect found this way, not merely a verification exercise**: the first attempt showed
"Content preview is no longer available for this change set" instead of real content. Reading why:
`seed_change_review_demo_change_set` (predates RFC-041, part of the RFC-020 closeout) creates a
real `ChangeSet` via `add_detected_generated_change_set` but never inserted the matching
`DetectedChanges` into `state.detected_changes_by_change_set` — it only ever had an
`&mut ApplicationShell`, not the `State` that field lives on, since `State::new` is still
assembling `Self` at that call site. **Fixed**: both seeding functions now return
`Option<(ChangeSetId, DetectedChanges)>`, and `State::new` seeds
`detected_changes_by_change_set` from it directly in the struct literal
(`seeded_change_review_demo.into_iter().collect()`) rather than always starting from an empty map.
Regression test: `seed_change_review_demo_change_set_creates_a_real_unlinked_change_set`, extended
to assert the returned pair matches the real `ChangeSet` it was built from.

After the fix, re-verified live: the Change Review surface opens (`Ctrl+Alt+D`), the one shown row
renders as a real button with the keyboard highlight marker, `Enter` selects it, and the content
preview renders — heading (`Preview: tekstide-changeset-demo.txt`), the required "not a diff"
label together with the absence-of-visible-change disclosure in the same line, and the real,
escaped file content (the seed's own real written text, with its trailing newline rendered as
`<U+000A>`). One caveat disclosed rather than silently substituted: mouse clicks were not
reliably reaching the window in this session (an automation-environment issue unrelated to the
product — keyboard input reached it reliably throughout), so the live walkthrough proves keyboard
reachability (`Ctrl+Alt+D`, `Enter` on the row) live; the automated test suite's own
`clicking_a_change_review_row_selects_it_for_preview` and
`change_review_surface_shows_real_content_from_a_real_agent_run` (which dispatches the real
`Message::ChangeReviewFileRowPressed` through `update`, not the click event itself) cover the
click path's own code, just not with a literal mouse click observed on screen this round.

## Known limitations (RFC-041-wide)

- **No two-sided diff for a modified file.** The before-bytes were never captured under
  `FilesystemSnapshot` detection and are gone by request time, not merely unretained (RFC-024
  §Correction, unchanged by this slice). Blocked on a real before-source — only RFC-030
  (Git-backed detection, unauthored, gated behind RFC-012's unmet safety evidence) could provide
  one.
- **Absence of visible change is not absence of change.** A user viewing a modified file's
  current content cannot tell whether the agent's own edit is still there, was reverted, or was
  overwritten by something else since. Stated on the surface itself, in the same
  `change-review-content-not-a-diff` line as the "not a diff" label — the direct consequence of
  the limitation above, and the one a user is most likely to get wrong in a way that matters.
- **The `DiffContent` move-out gap stays open, documented at the type.** Non-retention protects
  the wrapper (cannot be stored, cloned, or serialized) but not bytes a future consumer
  destructures out of it after a pattern match. Closing it needs a lifetime-bound `DiffContent`,
  a larger change than this slice takes on (RFC-041 D3, explicit).
- **`detected_changes_by_change_set` does not survive the application closing.** In-memory only,
  the same disclosed limitation `agent_run_change_baselines` already carries. A `ChangeSet`
  restored (if that ever happens) with no matching retained `DetectedChanges` simply cannot offer
  content preview — its metadata is unaffected, proven directly by
  `change_review_content_is_unavailable_when_retention_was_dropped`.
- **`detected_changes_by_change_set` has no eviction.** It grows in lockstep with
  `project.change_sets()`, which itself has no bound or eviction either (a pre-existing
  characteristic, not introduced or worsened by this slice) — disclosed rather than silently
  inherited.
