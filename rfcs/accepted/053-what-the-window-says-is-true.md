# RFC-053: What The Window Says Is True

Status: **Accepted 2026-09-24, through the owner's authorisation of the release schedule** — RFC-053 is its first row, and authorising a schedule whose first release is this RFC is what accepting it means. Say otherwise and it goes back to `proposed/` at no cost. **D1–D8 decided by the architect on acceptance.** Proposed 2026-09-24. First release in the 2026-09-24 schedule (`0.23.0`), from the GUI
audit of 2026-09-23.

## Summary

Six defects found by running the `0.22.0` release binary. Every one of them is **the product telling
a user something that is not true**, and one of them breaks an invariant the code states in a doc
comment and does not enforce. None is hard. Together they are the difference between a workbench and
a prototype that ships.

## What is true today, measured

Captured against the release binary, a throwaway `XDG_STATE_HOME` and a real Git fixture.

| | Measured | Where |
| --- | --- | --- |
| G1 | Terminal mode's main area renders **"> Terminal / Agent Immersion Mode. RFC-017 adds the terminal here."** — an internal RFC number, in the shipping product, as the first thing a user sees before launching a terminal | `surface/terminal.rs` |
| G2 | At 760×560 the keyboard reference **clips**: every per-surface section is gone, and **`Close` and "Escape closes this." are both below the cut**, with blank space beneath them. No scrollbar, no indicator. `Esc` still works; a pointer user is stuck | modal rendering |
| G3 | At 520px wide the status bar **wraps to three lines and still clips**. `status_bar`'s own doc comment states the one-line invariant: `content_area_height` subtracts the bar's height to size real PTYs, "so a second line would silently shrink every PTY". The invariant is asserted in prose and enforced nowhere | `shell.rs` |
| G4 | Change Review's empty state reads **"No changes have been detected in this project yet"** while the status bar in the same frame reads **`2 changed`** and the explorer shows an untracked badge. Change Review means *agent-generated* change sets; the sentence claims the project | change review surface |
| G5 | A freshly opened project's board row reads **"terminals: unknown / agent runs: unknown"**. After the first terminal it reads "1 terminal / 0 agent runs". The answer was zero all along | `project_board.rs` |
| G6 | The board reads **"0 dirty files"** beside the bar's **"2 changed"**. `runtime_summary.dirty_files` counts **open editor buffers with unsaved edits**; `changed_file_count` counts Git's changed files. Two different facts, adjacent, in near-identical words | `close.rs`, `project_board.rs` |

Two more from the same session, same class:

- The board renders **"9 blocked automations"** as a bare count, while `blocked_automation_labels`
  already exists in the model and is not shown — the "actionable labels, not only a number" rule
  `REQ-NOTIFY-003` applies to the status bar and not, yet, to the board.
- **Approval History leads with two paragraphs of caveats** before its empty state, on a surface that
  today can never hold an entry.
- The sidebar renders the literal word **"Sidebar"** over an empty panel in Terminal mode.

## The problem, stated plainly

This project has twice caught a surface contradicting another surface in review — the board's branch
label against the status bar (review 411), and before that a number describing something adjacent to
what was measured. **The same defect class keeps arriving because nothing re-checks a surface after a
neighbouring fact becomes real.** Change Review's sentence was true until Git state shipped. The
board's "unknown" was true until counts were computed at open. G3 was true until a window got narrow.

## Decisions required

**D1 — No internal identifier reaches a user.** G1's line is replaced by an empty state that says
what the mode is and how to start a terminal, and a mechanical check refuses the pattern `RFC-0`
anywhere in the catalog, the way the colour-alone and i18n scans already work.

**D2 — Modal content scrolls; its actions never leave the viewport.** The Close control and the
dismiss hint stay reachable at any window size the application can be given. Pinned by a test at a
small size, not by looking at it once.

**D3 — The bar may wrap, and the layout must then know it.** The choice is between eliding fields to
protect the invariant and measuring the real height; **measuring is the honest one** — `REQ-NOTIFY-002`
names five fields and eliding silently drops one. `content_area_height` derives from the **rendered**
bar height rather than a constant, and a test pins that a taller bar shrinks the content area by
exactly that much. The doc comment's claim becomes a property instead of a promise.

**D4 — An empty state says what it is empty of.** Change Review's says no *agent run* has produced
changes, and points at where repository changes are shown instead.

**D5 — `Unknown` means unknown.** A project whose collections have never been mutated reports **zero**,
because zero is what it has. `Unknown` stays for facts that genuinely are not known.

**D6 — Two facts, two words.** The board's *dirty files* counts unsaved editor buffers and says so —
`N unsaved` — leaving *changed* to mean what Git means. Whichever wording wins, the two must not be
distinguishable only by which surface you read.

**D7 — The board's counts get their labels** (`REQ-NOTIFY-003`, applied where it was not): blocked
automations name what is blocked, from the labels the model already carries.

**D8 — Caveats follow content.** A surface with nothing in it says that first; the caveats about
retention and risk classification appear when there is something to caveat.

## Non-goals

The explorer (RFC-052), user configuration (RFC-054), the editor's gutter, caret and undo (RFC-057),
and any change to what the counts themselves mean.

## Risks

- **D3 is a layout change under a terminal surface.** Getting the measured height wrong resizes every
  PTY. The test that pins it is the deliverable, not the change.
- **D5 touches a shared enum's meaning**, and `ProjectFileState` reads the same `Unknown`. The change
  is at the producer, not the type.
- **A mechanical check for "no internal identifier" can be over-broad.** `RFC-0` in a *doc comment* is
  correct and must stay legal; only catalog strings are user-facing.

## Acceptance criteria

- No user-facing string contains an internal identifier, held by a scan rather than by review.
- At 760×560 and at 520×400, every modal's Close control is reachable and the status bar's height is
  what the content area subtracts — both asserted, both captured live.
- Change Review, the board and the status bar agree, in one frame, about a project with two changed
  files and no agent run — captured.
- A freshly opened project reports zero terminals and zero agent runs.
- The colour-alone and i18n scans still pass.


## Decided on acceptance (2026-09-24)

**D1–D8 as written.** Two of them carry a consequence worth stating before anyone writes code.

**D3 is a layout change underneath a live terminal.** `content_area_height` sizing every PTY from a
*constant* bar height is the defect; deriving it from the **rendered** height is the fix, and the test
that pins "a taller bar shrinks the content area by exactly that much" is the deliverable. Getting it
wrong resizes every terminal a user has open, so this slice goes last and alone.

**D5 changes what a producer reports, never what the shared type means.** `ProjectFileState` reads the
same `Unknown`; the change belongs at the board row that has the counts, not in the enum.

**One addition — D9.** The scan D1 asks for is the third mechanical check of this kind in the crate
(colour-alone, i18n completeness, and now internal identifiers). **Doc comments stay legal**: `RFC-0`
in a doc comment is correct and useful; only catalog strings are user-facing, and the scan must say so
in its own failure message, the way `FILES_ALLOWED_TO_READ_FULL_FILE_CONTENT` does.

**Ships as `0.23.0`**, ahead of RFC-052, per the authorised schedule.

## D3′ — decided 2026-09-24 at review 419, from measurement

**The pane size is measured, not computed.** D3 said `content_area_height` should derive from the
*rendered status-bar height*. That was one input short of the defect. Before writing code the
implementer ran the `0.22.0` binary and counted: **`seq 1 200` ended at line 177 in a full-size
window** — twenty-three rows and the prompt below the visible area, with no narrow bar involved.

**Reproduced by the reviewer**, from a worktree build of the `0.22.0` tag: 177 on the released
binary, **200 with the prompt visible** on this branch, same fixture, same window.

Four independent causes, only one of them G3:

1. `top_bar_height` counted one heading row; the top bar has been three rows since RFC-039/040.
2. The mode-toggle row and the `+ New Terminal` row were never in the formula at all.
3. The status bar wraps at a narrow width (G3, the one the RFC knew about).
4. Rows were **counted** at `LineHeight::Relative(1.0)` and **drawn** at `rich_text`'s default
   `1.3` — every pane was given about 30% more rows than its height held.

So: a `MeasureSize` wrapper publishes the size the layout engine gives the pane region, and the
terminal reads it. Seven chrome constants, `window_size`, `WindowResized`, `WindowOpened` and two
window subscriptions are **deleted** — they existed only to feed the formula.

### This reverses response 242, and here is the condition

Response 242 held that **"a computed size needs no measurement"**. That was right while the
computation was checkable where it was used. It stops being right the moment the formula depends on
constants that *other slices* change: the top bar grew a row in RFC-039/040 and no test in that slice
had any reason to look at a terminal's row count. **A formula whose inputs are maintained elsewhere is
an unenforced invariant**, and this one was wrong in four independent ways, three of them invisible to
every test in the suite.

**The rule, going forward:** compute a size only when everything it depends on is visible and
asserted at the point of use. Otherwise measure — and pin the measurement with a test that fails when
the value stops being the real one.

**The smaller alternative was offered and declined.** Keeping the formula and adding measured inputs
for the top bar and the two workspace rows preserves exactly the failure mode that produced this
defect, and adds partial measurement on top of it.
