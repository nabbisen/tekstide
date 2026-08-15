---
title: "Two status mappings that lie: implementation handoff"
owning_rfcs: "RFC-012 (change detection), RFC-019 (content workspace) — neither owns both"
status: "Ready for implementation — scheduled into 0.8.0 alongside RFC-020, 2026-08-12"
created: "2026-08-12"
---

# Two unconditional mappings, both of which tell the user something false

## Why these two are one slice

Both are the same defect shape: **a status derived unconditionally where the truth is
conditional**, in `tekstide-core`, visible to a user, with the correct answer already
available a few lines away. Neither belongs to an RFC currently in flight, which is why
both have been sitting in `rfcs/future-work.md` rather than being fixed.

They are grouped because fixing one and not the other leaves the codebase with a
half-applied lesson. They are **not** grouped because they share code — they do not.

This slice runs **parallel to RFC-020** and touches none of the same files.

## Fix 1 — the project board says terminals are not implemented

`00-baseline-no-modal.png` in RFC-018 PR-018-G's own evidence pack shows the board
rendering `terminals: not implemented`, in the same build where `05` in that same pack
shows a terminal running. It shipped that way in `0.7.0`, disclosed in the changelog.

The mechanism, traced:

- `RuntimeSummary::default()` sets `terminal_count: None` (`project/runtime.rs:25`).
- `refresh_runtime_summary_from_collections` raises it to `Some(..)` only when a
  collection actually mutates (`project/session.rs:1181`).
- `active_session_row`'s `.unwrap_or(CountDisplay::NotImplemented)`
  (`project_board.rs:192-195`) renders that `None` as **"not implemented."**

So `None` carries two incompatible meanings — *the feature does not exist* and *nothing
has happened yet* — and the label asserts the first when the truth is the second. Open a
project: it claims the feature is absent. Launch one terminal: the same line silently
becomes `terminals: 1`.

**Do not fix this by defaulting the count to `Some(0)` at construction.** That just moves
the guess: it would report a confident zero for a project whose terminals genuinely have
not been enumerated yet, which is a different false statement. **Separate the states.**
"Unknown" and "not implemented" are different answers and the type should be able to hold
both, so the board can say `terminals: 0` when it means zero.

`agent_run_count` has the identical defect on the same two lines. `recent_project_row`
(`project_board.rs:245-250`) hardcodes `NotImplemented` across five fields where the
honest answer is "no open session" — decide whether that is the same fix or a disclosed
limitation, and say which.

## Fix 2 — a blocked save reports a conflict that may not exist

`project/content.rs:174`:

```rust
SaveDecision::BlockedExternalChange => ProjectContentStatus::Conflict,
```

Unconditional. Meanwhile `refresh_active_document` in the same file (~224-227)
distinguishes correctly, mapping `ExternalChangeDecision::ExternalChanged` and
`::Conflict` to different statuses.

So the same underlying situation reports differently depending on which path observed it,
and the save path reports the more alarming of the two. A user whose file changed cleanly
on disk — with no local edits to lose — is told they have a conflict.

**This is the defect RFC-019 PR-019-E already found and fixed once, on the shell side.**
The shell now reads `document.state()` instead and no longer depends on this mapping.
`render_text()` also renders this status, so `tekstide-core`'s own pre-GUI harness still
reports "conflict" for a save that lost nothing.

**Fix it in `save_active_document`**, at the point of the error: read the document's own
dirty state the way the shell-side fix does, rather than collapsing both cases before any
caller sees them. A caller-side fix is what produced the split in the first place.

## Review gate

- **Both fixes ablated.** Restore each collapsed mapping, watch a *specific* named test
  fail, restore. A test that passes with the fix removed is the failure mode this project
  has hit at least six times.
- **Fix 1: a positive control before the negative.** Assert the board reports a real count
  for a project that has terminals, *then* assert what a project without them reports.
  Asserting only the absence would pass against a board that reports nothing at all.
- **Fix 1: state what `recent_project_row` does** — fixed the same way, or disclosed as a
  limitation with the reason.
- **Fix 2: proven against a real file changed on disk**, with no local edits, showing the
  status is not `Conflict`. Not a synthesised decision value. RFC-019 PR-019-D's conflict
  test is the shape: real file, real external write, real operation.
- **Fix 2: the genuine-conflict case still reports `Conflict`.** The risk in narrowing a
  status is over-narrowing it, and a test that only proves the new case passes would not
  catch that.
- **`render_text()`'s output checked**, since it renders this status and is the pre-GUI
  harness's own answer.
- `rfcs/future-work.md`'s two entries updated in the same commit to record the fix, not
  left describing defects that no longer exist.

## Out of scope

- **The `NavigationAction` / `OpenActiveProjectWorkspace` gap** — belongs to RFC-023's
  keybinding pass, per its own `future-work.md` entry.
- **Broadening the `no_count_display_or_attention_label` scan to free functions** —
  belongs to whoever next touches `i18n::enforcement`; that module is not this slice's
  territory, for the same reason RFC-019 was told not to widen it.
- **Any rendering change.** Both fixes are in `tekstide-core`. If a fix appears to require
  a shell change, stop and raise it rather than reaching across the boundary.
