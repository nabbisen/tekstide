---
title: "RFC-042: QA evidence"
rfc: "RFC-042"
rfc_file: "../../done/042-change-content-legibility.md"
source_rfc_status: "Implemented and closed 2026-08-26 — RFC-042 is in rfcs/done/"
target_milestone: "M12"
created: "2026-08-26"
---

# QA evidence

One section per PR. Cite the command that produced each result.

## PR-042-A — chrome and untrusted content stop being the same type (D2, structural half)

**No behaviour change**, per the task breakdown's own requirement. `ChangeReviewContentLine`
(new) is the only way to carry a piece of an untrusted file's own bytes -- constructed solely via
`ChangeReviewContentLine::from_escaped`, which calls `quote_untrusted` internally, the same
single-constructor idiom `DisplayText` already uses. `ChangeReviewContentPreview` (new) replaces
the old `Vec<String>` return from `change_review_content_lines` with three fields -- `heading`,
`chrome: Vec<String>`, `content: Vec<ChangeReviewContentLine>` -- so a content value and a chrome
value are different Rust types, not the same type told apart by position (`if index == 0`).

**Ablation, exactly as the checklist specifies ("a compile failure is the ablation here -- record
the error, not a test name")**: in `change_review_content_lines`'s `Ok` arm, swapped
`ChangeReviewContentPreview { chrome, content }` to `{ chrome: content, content: chrome }`.
Result:

```
error[E0308]: mismatched types
    --> crates/tekstide/src/shell.rs:8415:25
     |
8415 |                 chrome: content,
     |                         ^^^^^^^ expected `Vec<String>`, found `Vec<ChangeReviewContentLine>`
```

(and the mirrored error for the other field). Confirmed, then reverted -- not committed. Recorded
in the type's own doc comment in `shell.rs` too, per the checklist.

**Gate**: full suite green with the same test count as before this slice (426 tekstide / 742
tekstide-core) -- no test added, none removed, none changed except the six call sites of
`change_review_content_lines` updated to call the new `.all_lines()` test-only convenience method
instead of using the old `Vec<String>` return directly. `cargo fmt --all -- --check` and `cargo
clippy --workspace --all-targets --all-features -- -D warnings` both clean.

## PR-042-B — the frame stops scrolling (D1)

**Still no line-splitting** -- content is one escaped `ChangeReviewContentLine` per selection,
exactly as PR-042-A left it. `change_review_view` no longer wraps the whole surface (frame +
content) in one `scrollable`. The frame -- heading, disclosure, detection status, file count, the
file rows, both omission lines, review state, and the preview's own `heading`/`chrome` (which
includes the "not a diff" label) -- renders in `column(lines)`, a plain, non-scrolling column.
Only `content_elements` (the file's own escaped bytes) is wrapped in its own, independent
`scrollable`, placed below the frame in an outer `column![frame, scrollable(content)]`.

**Tests**:

- `change_review_content_label_survives_content_long_enough_to_scroll` -- a real ~100,000-byte
  modified file (40 lines of ~2,500 bytes each -- deliberately few lines, so this test's own claim
  never couples to PR-042-C's separate line-count bound), real detection, real selection. Asserts
  `preview.chrome` still contains the "not a diff" label and `preview.content`'s combined length
  exceeds 60,000 characters -- proving the **data-level** guarantee that chrome is structurally
  independent of content's size. States plainly in its own doc comment what it cannot see: this
  project's own `frames()`-avoidance convention means no unit test here can observe real
  interactive scrolling or real pixels (`ARCHITECTURE.md`, "latency criteria stop the clock at
  state change, not at pixels").
- `change_review_frame_lines_never_feed_the_scrollable` -- the companion **wiring** proof, in the
  source-scan idiom `modal_layer_always_applies_the_scrim_style` already established for this
  exact class of property (where an `Element` gets placed, not what text it produces). Asserts
  `change_review_view`'s own source pushes `preview.heading`/`preview.chrome` into `lines`, that
  only `content_elements` feeds `scrollable(...)`, and that `lines` itself is never wrapped in a
  `scrollable`.

**Ablated**: moved the `for chrome_line in &preview.chrome` loop to push into `content_elements`
instead of `lines` (i.e. put the label back inside the scroll region). Result:
`change_review_frame_lines_never_feed_the_scrollable` failed on its first assertion, naming
`change_review_view`. Reverted, not committed; full `change_review` test group re-run green
(23 → 25 tests after these two additions).

**Live GUI evidence.** Release binary, `mktemp -d` fixture project (`/tmp/tmp.pgzhKLaKI4`),
`TEKSTIDE_CHANGESET_DEMO=1`, launched as `tekstide "$FIXTURE_DIR"` (the CLI takes a project path
directly, so no path was ever typed into the app). `niri msg action focus-window --id <id>` then
`wtype -M ctrl -M alt d -m alt -m ctrl` (per `ARCHITECTURE.md`'s corrected convention: `wtype`,
not `xdotool`, for this niri/Wayland setup) opened the Change Review surface; `wtype -k Return` on
the one file row selected it. Screenshots:
`.git-exclude/tmp/rfc042-evidence/pr042b-00-board.png`,
`.git-exclude/tmp/rfc042-evidence/pr042b-01-diffreview.png`,
`.git-exclude/tmp/rfc042-evidence/pr042b-02-selected.png` -- kept out of the repo, not because
they show anything under `$HOME` (they do not), but because the project list persisted from
earlier, unrelated sessions renders alongside the fixture row and includes at least one path
whose own directory name encodes an operator identity (this session's own scratchpad naming
convention) -- the same "any evidence that quotes what was on screen can quote a path" caution
`ARCHITECTURE.md` names for text evidence, applied here to a screenshot that happens to capture
more of the board than the one row this evidence is about.

- `pr042b-00-board.png`: Project Board, this session's fixture row (`/tmp/tmp.pgzhKLaKI4`).
- `pr042b-01-diffreview.png`: Change Review surface open, one file row.
- `pr042b-02-selected.png`: the row selected -- "Preview: tekstide-changeset-demo.txt", the "not
  a diff" label combined with the absence-of-visible-change disclosure, and the real escaped
  content, all rendering correctly as chrome above the (mostly empty, since the fixture is 80
  bytes) content region.

**What this live pass does not show, disclosed rather than silently narrowed**: genuine
interactive scrolling with the label staying pinned. `TEKSTIDE_CHANGESET_DEMO`'s own seed writes a
fixed, short (80-byte) file -- changing that write is out of this slice's scope (it is frozen,
reviewed production code from the RFC-020 closeout), and producing a large file through it live
would need either editing that function or orchestrating a full real managed agent run through the
GUI's approval flow purely for evidence, which was judged disproportionate for what the two tests
above already prove directly and more rigorously (a real 93 KB file through the real detection
and read path, plus a structural proof of the wiring that makes the frame's placement
non-accidental). One session interruption is also disclosed for completeness: an earlier attempt
at this same evidence lost the compositor focus mid-sequence because the operator was actively
using another window (`wtype` has no window-targeting of its own -- it delivers to whatever holds
compositor focus at the instant it runs, unlike `niri msg action screenshot-window --id`, which
targets by id regardless of focus) -- that attempt was abandoned rather than risking a stray
keystroke reaching the operator's own session, and redone once the desktop was confirmed free.

## PR-042-C — lines become lines, bounded (D2 visible half, D3)

**1. Line splitting.** `change_review_content_body_lines` splits on the raw byte `b'\n'` --
never a decoded `char` boundary, so a `\n` can never be bisected out of a multi-byte UTF-8
sequence -- and escapes each segment independently via `quote_untrusted`. The line break is the
only character this slice stops escaping; every other control character (`\r`, tab, ANSI escapes,
bidi overrides) is still escaped, per test below.

**2. The boxed container (D2 visible half).** `content_body` -- built only when
`content_elements` is non-empty -- wraps the file's own lines in a `container` styled with
`theme.surface_elevated()` background and a `theme.border_default()` border, the same boxed-card
style `approval_history_entry_view` already uses for a different untrusted-adjacent payload.
`Deleted`/`NonFile`/every refusal has no content and gets no box.

**3. The line bound (D3).** `DiffPreviewPolicy.max_lines` (new field, beside `max_input_bytes`,
in `tekstide-core`), enforced in `read_diff_content` after the bounded byte read (row count is
only knowable from real content) and before any content is ever returned. Refuses whole via a new
`DiffContentError::TooManyLines { relative_path, lines, max }` -- never truncates, matching
RFC-024's own byte-bound shape exactly. `change-review-content-error-too-many-lines` is its own
Fluent key, worded as a refusal, never as a third omission count next to
`omitted_changed_file_count`/`changed_files_omitted_by_detection`.

**The bound's value, measured** (`shell::tests::change_review_content_view_build_cost_by_line_count_measurement`,
`cargo test --release -- change_review_content_view_build_cost_by_line_count_measurement --nocapture`):
view-build cost -- escaping each line plus building one `text` `Element` per line, the exact
operation `change_review_view` performs for `content_elements` -- at candidate line counts, both
profiles:

| lines | release | debug |
| --- | --- | --- |
| 100 | 57µs | 155µs |
| 1,000 | 490µs | 1.41ms |
| 4,000 | 1.62ms | 5.65ms |
| 10,000 | 4.00ms | 14.34ms |
| 50,000 | 16.95ms | 71.75ms |
| 100,000 | 31.69ms | 140.85ms |

`NFR-PERF-003`'s existing budget (typing latency, p95 ≤ 16ms) is the closest already-established
latency criterion for a per-keystroke-shaped view rebuild, reused here as the RFC's own
instruction directs ("measure against this project's existing latency criteria"). At 4,000 lines
both profiles clear it with large margin (~10x release, ~3x debug); at 10,000 lines debug cost
alone (14.34ms) already leaves almost none; at 50,000 lines release cost alone (16.95ms) exceeds
it. **`DEFAULT_MAX_DIFF_LINES = 4000`**, chosen for the margin it leaves in the *slower* profile,
not only the shipped release one -- confirming, not overriding, RFC-042's own "expect the low
thousands." Not a tight regression bound in the committed test (machine-dependent) -- a
diagnostic report, the same shape `real_repository_filesystem_scan_cost_headless_benchmark`
(`tekstide-core`) already uses; what it cannot measure is real layout/shaping/paint cost inside
`iced`'s own renderer, which happens after `view` returns and is exactly what this project's
`frames()`-avoidance convention excludes.

**Tests** (all five fixtures the pack README requires, spoof written to prove the attack per
`what-a-legible-preview-must-not-become.md` §4's own trap -- "a test that asserts the right thing
about the wrong property"):

- `change_review_content_renders_real_line_structure_not_one_escaped_blob` -- fixture 1, an
  ordinary 4-line source file. Asserts 5 real elements (4 lines + one trailing empty line from the
  file's own final newline), and that no element contains `<U+000A>` any more.
- `change_review_content_spoof_lines_are_never_rendered_as_chrome` -- fixture 3 (D2), a file whose
  first three lines read `Detection: Complete`, `Review state: Accepted`, `1 file changed`.
  Asserts none of them appear in `preview.chrome`, and that the spoof text still renders --
  **only** inside `preview.content`.
- `change_review_content_line_split_does_not_weaken_escaping_of_other_control_characters` --
  fixture 5, "the one most likely to be skipped": tab, carriage return, an ANSI escape sequence,
  and a bidi override, each alongside real line breaks. Asserts all four still render as
  `<U+XXXX>` markers, no raw control character reaches the surface, and `<U+000A>` itself never
  appears any more.
- `change_review_content_refuses_over_the_line_bound_and_names_it` -- fixture 4, `DEFAULT_MAX_DIFF_LINES
  + 1` lines. Asserts `preview.content` is empty (refused whole, not truncated) and the refusal
  names "too many lines" -- distinct wording from RFC-024's own byte-bound refusal.
- `change_review_content_view_build_cost_by_line_count_measurement` -- the measurement itself,
  kept as a permanent diagnostic (loose bound, `< 500ms`, so it guards against a real regression
  without being flaky against machine noise).

Fixture 2 (long enough to test D1) is PR-042-B's own
`change_review_content_label_survives_content_long_enough_to_scroll`, already covering this slice
too since it now exercises the real line-splitting path.

**Ablations, all three the checklist requires, confirmed then reverted:**

1. *Escape the line break again* -- reverted `change_review_content_body_lines` to its
   PR-042-A/B one-blob shape. `change_review_content_renders_real_line_structure_not_one_escaped_blob`
   failed: `left: 1, right: 5`, and the rendered output showed the pre-slice single escaped blob
   with `<U+000A>` between every line.
2. *Relax `quote_untrusted` for a character other than the line break* -- temporarily excluded
   tab from `text_safety::is_untrusted_display_control`.
   `change_review_content_line_split_does_not_weaken_escaping_of_other_control_characters` failed
   on its own first assertion ("tab must still be escaped"), with the raw tab visible in the
   panic's own rendered output.
3. *Truncate instead of refusing over the bound* -- replaced the `if line_count > policy.max_lines
   { return Err(...) }` check with two `let _ =` no-ops. `change_review_content_refuses_over_the_line_bound_and_names_it`
   failed: `content lines=4002` -- the over-bound content rendered in full instead of being
   refused.

All three reverted immediately after confirming failure; none committed.

**Live GUI evidence.** Same release binary and `mktemp -d` fixture project as PR-042-B, relaunched
after a session gap (the desktop was in active use in between -- confirmed free before retrying,
the same care as PR-042-B's own disclosed interruption). `pr042c-01-boxed.png`
(`.git-exclude/tmp/rfc042-evidence/`, kept out of the repo for the same reason as PR-042-B's own
screenshots) shows the new boxed container clearly: a bordered box with a distinct background,
visibly separate from the surrounding chrome, around the file's content. **What this screenshot
does not show**: genuine multi-line wrapping -- `TEKSTIDE_CHANGESET_DEMO`'s fixed, single-line
seed content cannot demonstrate that live without either editing that frozen function or
orchestrating a full real managed agent run through the GUI purely for evidence, the same
disproportionate-effort judgement PR-042-B's own live pass already made and disclosed for
genuine scrolling. The five real-file tests above are the actual proof of line-splitting,
exercised through the full production read path; this screenshot's own job is narrower --
confirming the container is visually real, not merely present in the `Element` tree.

## Whether a real mouse click was sent (stated either way, per the checklist)

**No.** Every live interaction across both passes (PR-042-B and PR-042-C) was keyboard-only --
`niri msg action focus-window --id`, `wtype` for `Ctrl+Alt+D` and `Enter`. No pointer-injection
tool was available in this session (the same gap `0.14.0`'s own release gate disclosed). This
slice's own new control (the file row) is unchanged from RFC-041 -- still a real
`iced::widget::button` -- and RFC-041's own click path is already covered by
`clicking_a_change_review_row_selects_it_for_preview` and
`change_review_surface_shows_real_content_from_a_real_agent_run`, which dispatch the real
`Message::ChangeReviewFileRowPressed` through `update`. Nothing in RFC-042 changes that control or
its own click handling -- only what renders once a selection already exists -- so no new click-path
gap is introduced by this slice specifically.

## Response 331 -- three required fixes

### Required 1: D1 amended -- pin the claims, scroll the lists

The reviewer's own reproduction (release binary, `mktemp -d` fixture, window height 380px) showed
the file-row button clipped mid-height and the "not a diff" label **unreachable** -- not scrolled
past, unreachable, because the pinned frame had no scroll region of its own and grew with the
file-row list's length. `change_review_view` is restructured into four independent regions,
assembled by a new `assemble_change_review_layout`:

1. `pinned_top` -- heading, disclosure, detection status, file count. Fixed count, four items.
2. The file-row list, now in **its own** `scrollable`, independent of everything else.
3. `pinned_middle` -- both omission lines, review state, the preview's own heading and chrome
   (including the "not a diff" label). Fixed count, unaffected by list length.
4. The content region, unchanged from PR-042-C -- its own `scrollable`, its own boxed container.

**Verified**: the pinned regions' own combined height is now independent of both list lengths
(proven by the new test below, real numbers). Re-verified live at several window heights
(`.git-exclude/tmp/rfc042-evidence/331-reverify-*.png`): at 800px and 450px, the file row, review
state, preview heading, and boxed content all render correctly, simultaneously, with no scrolling
needed. **At exactly 380px -- the reviewer's own reproduction height -- the preview heading and
content still fall below the window's bottom edge**, though the file-row list no longer grows
unboundedly with row count (this fixture has only one row, so that specific defect was not what
clipped it here).

**Disclosed, not silently narrowed**: this is a *different* cause than what was fixed. `pinned_top`
alone (heading plus a disclosure that wraps to three lines at this width, plus two more lines)
already consumes most of a 380px window once title bar and tab chrome (~140px) are subtracted --
independent of any list. Making `pinned_middle` reachable at 380px specifically would require
either shrinking the disclosure text (not in this slice's scope), enforcing a minimum window
height, or making the pinned regions themselves scrollable (independently of content, which the
"file-row list" fix would still leave sound) -- but the amendment's own words are "**pin** the
claims," and a pinned region that must also always fit inside an arbitrarily short window is a
different, harder requirement than "does not grow with list length," which is what was reproduced
and is what this fix addresses. Left as an open question for the architect rather than guessed at:
is a bounded-but-still-clippable-at-380px pinned region acceptable, or does D1 need a fourth
decision about a minimum window height or a scrollable claims region?

### Required 2: a guard on the property, not the prose

`change_review_frame_lines_never_feed_the_scrollable` (the previous, source-text-scan guard) is
removed. `change_review_layout_pins_fixed_regions_regardless_of_list_length` replaces it, calling
the real, production `assemble_change_review_layout` -- not a copy -- with `()`, iced's own
headless test renderer (`iced_core::renderer::null`, `impl Renderer for ()`), computing a real
`layout::Node` tree: no GPU, no font backend, no window. It measures, at 1-of-each and 200-of-each
file rows/content lines: (a) the pinned regions' combined height, and (b) `pinned_middle`'s own Y
offset from the page top. Both must stay constant.

**Both of the reviewer's own attacks, reproduced against this new test and confirmed failing,
then reverted -- neither committed:**

1. Wrapped the whole assembled layout in an extra outer `scrollable(...)` (the reviewer's exact
   defeat). The top-level widget becomes that `Scrollable`, whose own `layout::Node` has exactly
   **one** child, not four -- failed the child-count assertion directly (`left: 1, right: 4`), a
   stronger and earlier catch than the height/position checks ever needed to run.
2. Removed the file-row list's own `scrollable(...)` wrapper (the *original* D1 defect,
   reproduced). Growing the fixture from 1 file row to 200 pushed `pinned_middle`'s own Y position
   from 16px to 592px -- measured, not asserted from reasoning about what should happen.

### Required 3: D2's move-out gap, closed

The reviewer's own exploit -- `for content_line in &preview.content { lines.push(text(content_line.as_str().to_string())...) }`
inside `change_review_view` itself -- rendered content in the chrome frame, unboxed, indistinguishable
from a real chrome line. **Closed, not merely documented** (the reviewer's own preferred option):
`render_change_review_content_body` is now the *only* function in this module that calls
`ChangeReviewContentLine::as_str()`, and it always returns the bordered container in the same
step -- `change_review_view` itself never touches `.as_str()` and has no intermediate, un-boxed
`Vec<Element>` of content lines lying in its own scope for a future edit to misplace.

**Guard**: `change_review_view_never_calls_as_str_on_content_directly` asserts the *absence* of
the call syntax `.as_str()` anywhere in `change_review_view`'s own source body -- robust to
reformatting in the way the D1 prose-match was not, since it checks for one specific method call,
not one specific multi-line wiring shape. **Ablated**: pasted the reviewer's own exploit back in;
failed, naming the reason; reverted, not committed.

`change_review_content_spoof_lines_are_never_rendered_as_chrome`'s own doc comment is corrected --
it proves the *data-level* classification (chrome from catalog lookups, content from file bytes,
never crossing by construction, true since PR-042-A) but does not, and never did, reach the
*render-level* question `change_review_view_never_calls_as_str_on_content_directly` now covers.

### The gate: PTY exhaustion was transient, not this slice's

The reviewer's own three runs each failed ~50 tests in under a second (`PtyUnavailable ... No
space left on device`), diagnosed as `test-process-leak.md`'s disclosed-but-unscheduled third
cause (2,060 orphaned `/bin/sh`, `/dev/pts` at its ceiling). By the time this response's own gate
ran, `/dev/pts` had recovered on its own (5 of 4096 in use, one ordinary shell) -- all three
previously-PTY-blocked tests (the two real-agent-run `change_review_*` tests and the general
suite) passed cleanly. Six full-workspace runs performed in total for this response (see
`test-process-leak.md`'s own new "Recurrence, 2026-08-26" section): runs 1 and 2 both hit the
already-known, unrelated `command_approval_family_produces_real_durable_audit_records_through_the_pipeline`
flake (row 3), assertion message captured for the first time for that row; runs 3 through 6 all
clean, 434 tekstide + 742 tekstide-core.

Not addressed here, and not this response's to fix: the disclosed third-cause PTY leak itself
(unscheduled, per the reviewer's own note that they raised it to the owner separately) and
row 6's own still-unconfirmed status.

## Response 332 -- two required fixes

### Required 1: the ordering invariant, stated and tested

The reviewer accepted the four-region split (D1's own growth-independence property holds) but
found, at 380px, that `pinned_middle` clips too, and so does the content -- the invariant that
actually matters ("content is never visible without the claim that qualifies it") holds, but "by
accident of ordering," untested. Added to `change_review_layout_pins_fixed_regions_regardless_of_list_length`:
for viewport heights `[100, 380, 600, 1200]`, `pinned_middle`'s own bottom edge must stay at or
above the content region's own top edge.

**A real gap found ablating this addition, not assumed correct.** Read literally (fixed indices
2 and 3), the check is vacuous: a `Column` always renders its children in **declaration order**,
so `children[2]` is bottom-bounded above `children[3]` for *whatever* widgets occupy those two
slots -- confirmed directly by swapping `pinned_middle` and `content`'s declaration order inside
`assemble_change_review_layout`'s own body: **both the index-based Y check and the growth
checks stayed green.** The fixed-index comparison alone cannot tell "pinned_middle above
content" (correct) apart from "content above pinned_middle" (transposed, wrong).

**Strengthened with a type-level check, not another prose match.** `iced::advanced::widget::Tree`
records each child's `Tag` (`iced_core::widget::tree::Tag`, keyed on `TypeId`) -- stateless
(`Tag::stateless()`) for a plain `Column`/`Text`, non-stateless for a stateful `Scrollable`
(which tracks scroll position). Asserting `tree.children[0]`/`[2]` are stateless and
`tree.children[1]`/`[3]` are not confirms the *structural shape* `[Column, Scrollable, Column,
Scrollable]` holds -- which the transposition ablation above violates directly (`children[2]`
becomes the content `Scrollable`, non-stateless, where a stateless `Column` was asserted).
Reproduced the swap again with this check in place: failed immediately, naming the exact
transposition. Reverted; nothing committed.

**What this combination proves, precisely**: given the real production `assemble_change_review_layout`
places a plain `Column` (matching `pinned_middle`'s own shape) at position 2 and a `Scrollable`
(matching `content`'s own shape) at position 3 -- confirmed by the type check -- `Column`'s own
declaration-order guarantee then makes the Y-ordering hold, at every tested viewport height.
Neither check alone was sufficient; both together close the gap the first version left open.

### Required 2: D2's move-out gap, actually closed this time

Response 331's fix (collapsing the un-boxed intermediate value into one render function) left
`.as_str()` reachable from anywhere in `shell.rs` -- the reviewer defeated it by extracting a
second helper next to the render function and calling `.as_str()` from there. A source-scan of
`change_review_view`'s own body cannot see a call made from a *different* function, however the
scan is written.

**`ChangeReviewContentLine` and the render function now live in their own module**
(`mod change_review_content`, inline in `shell.rs`). The struct's field and its `as_str` accessor
are both private to that module (the accessor additionally `#[cfg(test)]`-gated, an explicit,
named escape hatch for tests that need the raw string rather than an informal one). Nothing
outside the module -- `change_review_view`, a sibling helper, anything, at any distance -- can
read a content line's own text. The stale source-scan test
(`change_review_view_never_calls_as_str_on_content_directly`) is removed: there is nothing left
to scan for at the render level, since the compiler now enforces the property directly.

**Both of the reviewer's own exact reproductions confirmed as compile errors, then reverted:**

1. Inlined `content_line.0.as_str()` directly in `change_review_view` (reaching the private
   field from outside its module): `error[E0616]: field \`0\` of struct
   \`ChangeReviewContentLine\` is private`.
2. Pasted the reviewer's own `leak_content_into_frame` helper verbatim, extracted next to
   `assemble_change_review_layout`, calling `line.as_str()` from there: `error[E0599]: no method
   named \`as_str\` found for reference \`&ChangeReviewContentLine\` in the current scope` -- the
   method does not exist at all outside `cfg(test)`, regardless of which function tries to call
   it, so extraction to any distance cannot reach it.

Both confirmed, then reverted -- neither committed.

### Gate

Three consecutive full-workspace runs, each logged to a file: all three clean, 433 tekstide + 742
tekstide-core + 2 doc-invariant, zero failures. `git diff --check` and `rfc_docs_invariants`
clean. (Test count is 433, one fewer than response 331's 434, since the stale source-scan test
was removed and replaced by strengthening an existing test rather than adding a new one.)
