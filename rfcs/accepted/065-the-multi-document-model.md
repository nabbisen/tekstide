# RFC-065: The Multi-Document Model

Status: **Accepted by the human owner 2026-10-07.** D1–D9 as written, plus D10–D13 — see *Decided on acceptance*. Proposed 2026-10-07. `0.30.0`, M13. Split from RFC-026 on 2026-10-07 when the watcher
shipped alone. **Its first slice repairs a data-loss defect that is live in `0.29.0`.**

## Summary

"Multi-document editing" is in the roadmap's 1.0 minimum list. The editor holds **one** document, and
opening a second **silently discards the first's unsaved edits** — no prompt, no refusal, no record.
That is not a missing capability. It is work a user loses in the ordinary flow of using the editor,
and it is shipping now.

The open set is the fix, because with a set there is nothing to replace. This RFC builds it, repairs
the loss first, and makes `REQ-EDIT-004`'s own plural — *dirty **buffers*** — true for the first time.

## What is true today, measured

| | Measured |
| --- | --- |
| 1 | **Opening a second file discards the first's unsaved edits, silently.** `Action::Open(path)` → `open_active_project_text_document` → `app.rs:466` → `session.rs:1472` → `content.rs:190`, which is `self.active_document = Some(document)`. **No `dirty`, `unsaved` or `confirm` appears anywhere on that chain** — checked function by function, zero hits in all four. |
| 2 | **`NFR-REL-005` enumerates three causes** — external file changes, close events, process termination — and this is a **fourth**. The requirement lists rather than generalises, which is why nothing caught it. |
| 3 | **`REQ-EDIT-004` says "dirty *buffers*"**, plural. `dirty_file_count` is `u32::from(<one Option>)`: it can only ever return 0 or 1. The coverage row lists `004` as covered and qualifies the row *"single active document"*. |
| 4 | `open_buffer_count` and `dirty_file_count` have **one reader each**, at `session.rs:1769–1770`. |
| 5 | **83 production call sites** read `active_document`/`active_text_document` — up from **74** when RFC-026 measured it, so the surface grew by nine while the watcher was built. |
| 6 | **No save-all exists**: zero call sites, no catalog string. RFC-026's checklist assumed one; it is new work, not an adaptation. |
| 7 | A document already carries its own `cursor` and `viewport`, so per-document view state travels on a switch with no new storage. |
| 8 | `watch_inputs()` already returns `open_document_directories` — **plural, fed by one**. The watcher needs no change in shape, only in count. |
| 9 | Each document is up to **4 MiB** plus two 500-deep undo stacks, and its refresh reads the **whole file** (RFC-026 review 464). |
| 10 | **Nothing switches between open documents.** The tab strip is for projects. |

## Decisions required

**D1 — The repair ships first, and alone.** Measurement 1 is a live defect, not a design gap. Slice A
makes opening a second file stop discarding the first — by the set if the set is ready, by a refusal
or a prompt if it is not. It does not wait for the rest of the model, and it does not wait for a
switcher.

**D2 — The open set, and `active` keeps its meaning.** One *active* document; a plural *open* set.
Carried from RFC-026 D6 and D9: of the 83 sites, the ones that must change are the ones that
**count**, and measurement 4 says there are exactly two with one reader each. A rename across 83 sites
to prove a point is how this goes wrong.

**D3 — `REQ-EDIT-004`'s plural becomes true, and the coverage row says it was half met.** Measurement
3: the requirement has said *buffers* since it was written and the product has only ever had one. That
is a coverage claim to correct by name, in the same way RFC-057 corrected `REQ-EDIT-002`'s.

**D4 — The set is bounded, and the bound is stated when reached.** Measurement 9: each document is up
to 4 MiB plus two undo stacks, and RFC-026 measured its refresh at about 4 ms per document. N open
documents multiply both.

**D5 — Switching is keyboard-first and reachable, or it is not implemented.** Measurement 10: nothing
switches today. RFC-021's lesson is the standing one — a model nobody can reach is not implemented —
and `REQ-EDIT-004`'s counts are meaningless to a user who cannot see the second buffer they describe.

**D6 — Save-all is new work with its own risks.** Measurement 6. Saving several files at once is a
sequence of writes that can fail partway, and a partial save-all must say which succeeded. It is not a
loop around the existing save.

**D7 — The refresh cost is per document and must be measured, not assumed.** Carried from RFC-026
review 464: a burst touching N open documents is N whole-file reads in one drain window. RFC-026's own
paired-control harness is the instrument, and the measurement belongs in this RFC rather than in a
later surprise.

**D8 — No requirement names multi-document, and this RFC does not mint one.** The roadmap's 1.0
minimum list names it; the spec does not. Same treatment as undo at RFC-057 D4: implement it, record
the gap for the owner, do not invent a `REQ-`.

## Non-goals

A tab bar for documents, if a simpler switcher reaches the same place. Split views. Per-document
terminals. Crash recovery of the open set (RFC-027, `0.31.0`, which this unblocks). Syntax
highlighting. Anything that makes the watcher's scope recursive.

## Risks

**R1 — the repair must not become a nuisance.** A prompt on every second file would be worse than the
defect for a user who never edits before browsing. The set removes the question entirely, which is why
D1 prefers it and allows the interim only if the set is not ready.

**R2 — 83 call sites, and the ones that count are two.** A count that is wrong still compiles. The
test that matters fails when either count returns one while two are open.

**R3 — N documents multiply three things**: bytes, undo history, and the per-document refresh. D4's
bound covers the first two; D7's measurement covers the third.

**R4 — the watcher's scope grows with the set.** Measurement 8: `open_document_directories` is already
plural, so this is arithmetic rather than design — but the watch budget is the kernel's, and RFC-026's
refusal path must still behave when the set is large.

**R5 — a switcher is a new surface in a sidebar that already spends four reserved lines**, and RFC-055
measured it at about 32 columns.

**R6 — closing a document is now a thing that can lose work**, in a way it was not when there was one.
The close dialog counts, and must count the set.

## Acceptance criteria

- **Opening a second file with unsaved edits in the first loses nothing**, captured live, and a test
  that fails against today's code. This is slice A and it is the release's headline.
- The two counts count the whole set: **a test fails if either returns one while two are open.**
- `REQ-EDIT-004` moves to met **for its own plural**, and the coverage row is corrected from implying
  it already was.
- The bound is stated when reached.
- **Switching is reachable by keyboard and captured live**, with no environment variable.
- Save-all reports which files were written when one of them fails.
- **The per-document refresh cost is measured** on RFC-026's paired-control harness, with the control
  in the same run — N documents against one.
- The watcher's scope follows the open set: opening and closing documents changes the watched count,
  counted before and after.
- The close dialog counts every open document, not the active one.
- The requirements gap is written up for the owner; no `REQ-` is minted.
- Gate green three times with `--no-fail-fast`, **0 fixture entries left**, the core pin bumped with
  the version, and **`cargo test --doc`** — the step `0.29.0` added after `--all-targets` was found
  never to have run the doctest guard.

## Decided on acceptance (2026-10-07)

**D1–D9 as written.** Four additions, two of which remove work rather than add it.

**D10 — The close dialog needs no separate change.** Measured: `session.rs:1706` is
`close_resources.dirty_files = file_state.dirty_file_count`, the **same count** D3 fixes. So the
acceptance criterion about the close dialog is discharged by a **test**, not by a second code path.
Said here so nobody builds one.

**D11 — The switcher has room, and RFC-054's model already handles it.** Eighteen chords are held,
all `Ctrl+Alt+<letter>` plus `Ctrl+S`, `Ctrl+Shift+{P,V,Z}` and `Ctrl+Z`. Most `Ctrl+Alt` letters are
free, and RFC-054's fixed-point collision resolution refuses every participant on a clash — so adding
one is a known operation, not a design question. D5's requirement is that it be **reachable**, not
that it be novel.

**D12 — Slice A's repair is demonstrated against the defect, not described.** The test is written
first and shown **failing against today's code**, then fixed — the shape RFC-026 PR-026-C used when it
planted the third file before any code that deletes existed. A repair whose test never saw the defect
is a claim.

**D13 — The per-document measurement reuses RFC-026's paired-control harness unchanged.** It exists,
it carries its own control in the same run, and it took three attempts to get right (reviews 462, 463,
464). Add a condition — N documents against one — and do not rebuild it.

**Ships as `0.30.0`**, with slice A first and alone.
