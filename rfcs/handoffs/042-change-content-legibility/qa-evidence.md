---
title: "RFC-042: QA evidence"
rfc: "RFC-042"
rfc_file: "../../accepted/042-change-content-legibility.md"
source_rfc_status: "Accepted 2026-08-26 — M12, first of three for 0.15.0"
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

- `change_review_content_label_survives_content_long_enough_to_scroll` -- a real 93,000-byte
  modified file (`"line of real modified content\n".repeat(3000)`), real detection, real
  selection. Asserts `preview.chrome` still contains the "not a diff" label and `preview.content`
  is a single element over 60,000 characters -- proving the **data-level** guarantee that chrome
  is structurally independent of content's size. States plainly in its own doc comment what it
  cannot see: this project's own `frames()`-avoidance convention means no unit test here can
  observe real interactive scrolling or real pixels (`ARCHITECTURE.md`, "latency criteria stop the
  clock at state change, not at pixels").
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
