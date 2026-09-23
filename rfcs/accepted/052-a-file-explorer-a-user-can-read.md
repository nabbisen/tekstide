# RFC-052: A File Explorer A User Can Read

Status: **Accepted by the human owner 2026-09-24.** **D1, D2, D4–D8 decided by the architect on acceptance; D3 stayed open by design; slice A decided it (D3′, below): compose our own.** Proposed 2026-09-23. Raised by the human owner, who said the sidebar "looks strange…
a series of lines such as `[DIR]` and `[FILE]` seems far from helpful and friendly to users", and
named three resources already in this ecosystem: `snora`, lucide icons, and `iced-swdir-tree`.

## Summary

The sidebar renders a file listing as bracket-tagged debug text, and it is not a tree. Both are
fixable, and the second is the larger defect. **The mechanism is open and slice A decides it**: adopt
`iced-swdir-tree` (the owner's own crate, already built for `iced 0.14`), or compose rows ourselves —
decided by measurement against a hostile fixture, not by preference.

## What is true today, measured

Against the `0.22.0` release binary, a real Git fixture, and the code:

| | Measured |
| --- | --- |
| 1 | A row reads `[FILE] scratch.txt [untracked]`, `> [DIR] .git (collapsed)`, `[DIR] docs`. Every row is **one string**, assembled by a single Fluent selector with five slots (`kind`, `name`, `state`, `symlink`, `git`). |
| 2 | **The explorer is not a tree.** `ExplorerRow` is `Parent | Node`, built from one `ExplorerDirectoryScan` of a single directory. `Enter` on a folder *replaces* the listing; a `Parent` row walks back out. A modified file inside `src/` is invisible until you step into `src/`. |
| 3 | `REQ-FILE-001` says "display the project root **tree**". What ships is a one-level browser. |
| 4 | The one-string row is not an accident: it centralises `quote_untrusted` escaping of attacker-controlled names, keeps every label in the catalog, and makes the whole row testable without `iced` — three disciplines this project paid for. |
| 5 | `iced-swdir-tree 0.9.3` (nabbisen, Apache-2.0) needs `iced ^0.14` — exactly ours — plus `swdir ^0.11` (own crate; deps `rayon`, `thiserror`) and optional `lucide-icons ^1` under its `iced` feature. Its README states: lazy async expansion that "never blocks the UI thread on disk I/O", multi-select, keyboard control, live search, and that both widgets "own UI state only — they never rename, delete, move, or write". |
| 6 | `lucide-icons 1.47.0` is **third-party** (WhySoBad, MIT AND ISC, 561 KB), optional `iced` dependency, and its `iced` feature enables only `dep:iced` — whether it needs `iced`'s own `svg` feature is unmeasured. |

## The problem, stated plainly

A file explorer is two things at once: **a renderer of names an attacker controls**, and **a walker
of a directory structure an attacker controls**. This project already treats the first as a hazard
(`RFC-016` text safety) and the second as a boundary (`REQ-SEC-040`..`043`: root-scoped, symlink
escapes detected, no unbounded recursion). Today's explorer satisfies both — and reads like a debug
dump while doing it. Any fix must keep both and still look like a product.

## Decisions required

**D1 — A row stops being one string.** It becomes a composed row: an icon, the escaped name, and its
status. **Escaping does not move**: whatever renders the name receives `quote_untrusted` output, never
a raw `OsStr`. Labels stay in the catalog. The row's *text content* stays assertable without `iced`,
or we have traded a discipline for a look.

**D2 — The explorer becomes a real tree** (`REQ-FILE-001`): folders expand in place, nested children
are visible, and the current bounded-scan policy (256 children per directory, collapsed
`[.git, node_modules, target]`) becomes per-level rather than per-view. The `Parent` row goes away.

**D3 — Mechanism: `iced-swdir-tree`, or our own composition. Open, and slice A decides it.**
Recommended direction: prefer the widget **if** it can be shown to keep every property below;
otherwise compose our own rows and add expansion ourselves, which is more work and no new dependency.
The properties, each measured against a fixture rather than read from a README:

1. **The project root is a boundary.** A symlinked directory pointing outside the root is not
   expanded, and a symlink whose target escapes is reported, not followed (`REQ-SEC-041`, `043`).
2. **We supply the display text.** A file named with a bidi override, a newline, or invalid UTF-8
   renders through our escaping — not through the widget's own `file_name()`.
3. **Per-node status is ours to draw**: the Git badge, `(blocked)`, `(unreadable)`, `[symlink]`.
4. **Bounded**: a directory with 100 000 entries does not stall the frame or the process; an
   unreadable directory is a row, not a panic.
5. **Keyboard**: the widget's own key handling does not outrank `KeybindingPolicy`, and a modal still
   suppresses it (`RFC-015`).
6. **No writes, and no drag-and-drop in this RFC.** The README says the widget only reports intent;
   this RFC does not accept that intent.
7. **Strings**: anything it renders itself (search field, "loading") is ours, or it is not used.
8. **Cost**: `rayon`'s thread pool in our process, `lucide-icons`' 561 KB, and whether `iced`'s `svg`
   feature becomes required. Each new crate gets a dated row in `dependency-advisories.md` (`RFC-030`
   D8's rule), and the MSRV is re-measured, not assumed.

**D4 — An icon never carries meaning alone.** `NFR-UX-002` and `REQ-NOTIFY-005` are mechanically
checked today. A file-type icon may replace `[DIR]`/`[FILE]` because *kind* is also carried by
position and by the name itself; **a Git badge stays a word**, with an icon or colour only reinforcing
it. "Untracked" does not become a dot.

**D5 — Deleted files still have no row**, and the count still includes them (`0.22.0`'s disclosure
stands). A tree changes nothing about that.

**D6 — Out of scope, named so nobody folds them in**: `.gitignore` handling and an `ignored` badge
(`REQ-FILE-005`, part of `002`); a hidden-file toggle (`REQ-FILE-006`); the file watcher and
multi-document model (`RFC-026`, M13); drag-and-drop; multi-select.

## Non-goals

- Re-platforming the UI onto `snora`. It is a GUI framework, not a widget; adopting it is a decision
  about the whole application, not about the sidebar, and it deserves its own RFC if the owner wants
  it.
- Syntax highlighting, the line-number gutter, or undo — the editor's own gaps, recorded in the
  2026-09-23 audit.

## Risks

- **A third-party widget renders attacker-controlled names.** This is the whole D3 measurement. If the
  widget will not take our display text, it cannot be used, regardless of how good it looks.
- **A parallel directory walker inside our process.** `swdir` brings `rayon`. Bounded work, thread
  count, and behaviour on an unreadable or enormous directory all need measuring, not assuming.
- **An icon set is a dependency with a supply chain**, and this one is the only third-party crate in
  the proposal.
- **Losing testability.** Today a row is one assertable string. If composition scatters that across
  widget calls, the tests that hold escaping and i18n stop holding them.

## Acceptance criteria

- A hostile fixture — a symlink escaping the root, a bidi-override name, a newline in a name, an
  unreadable directory, and a directory with 100 000 entries — is rendered by the chosen mechanism
  with **every property in D3 demonstrated**, and the ablation that proves the fixture hostile is run
  first (`RFC-030`'s discipline).
- Folders expand in place; a change inside `src/` is visible without stepping into `src/`.
- Every status a row can carry is still a word; the colour-alone scan still passes.
- A live capture against `mktemp -d`, in the release binary, showing the tree with icons, Git badges,
  and a symlink that escapes the root reported rather than followed.


## Decided on acceptance (2026-09-24)

**D1, D2, D4, D5, D6 as written.** Rows become composed rows; escaping does not move; labels stay in
the catalog; the row's text content stays assertable without `iced`. The explorer becomes a real tree
and the `Parent` row goes away. An icon never carries meaning alone — a Git badge stays a word.

**D3 stays open, and that is the decision.** The same shape as RFC-030 D1′: the mechanism is chosen
by measurement, by the rule fixed here, not by preference.

- **The widget wins if, and only if, every property in D3 holds against the hostile fixture** — our
  escaped text is what renders, the project root bounds expansion, a symlink that escapes is reported
  rather than followed, our badges draw, its keys do not outrank `KeybindingPolicy`, and its own
  strings are ours or unused.
- **Otherwise we compose our own rows and add expansion ourselves.** More work, no new dependency,
  and the same acceptance criteria apply to it.
- **Run the fixture's own falsifying ablation first.** A fixture that cannot be made to fail proves
  nothing — RFC-030's first box, and the reason its gate survived review.

**D7 — the collapse list stays until ignore rules replace it.** `[.git, node_modules, target]` are
collapsed today, and a tree that expands in place makes that list load-bearing in a way a one-level
browser never did: without it, the first `target/` a user opens is a hundred thousand rows. The list
stays exactly as it is in this RFC, and `REQ-FILE-005`'s real ignore handling is the next slice after
it (see the schedule), not a stretch goal folded in here.

**D8 — expansion has a budget, measured.** Expanding a directory must not block a frame
(`NFR-PERF-007`'s own rule for file churn, applied to the same surface). The 100 000-entry fixture is
where that is measured, not asserted — and if the chosen mechanism cannot expand it without stalling,
that is a D3 failure, not a performance note.

**Sequenced as `0.24.0`**, after RFC-053's truth slice. RFC-052 changes how the sidebar looks; RFC-053
stops three surfaces saying things that are not true. The second is smaller and more urgent.

## D3′ — decided 2026-09-24, from measurement (PR-052-A)

**Compose our own rows and add expansion ourselves. No new dependency.** By D3's rule, not by
preference: **`DirectoryTree`, the widget as shipped, fails five of the eight properties** (1, 2, 3, 4 and 6),
measured against the hostile fixture and read back from what the widget actually draws.

| D3 property | `DirectoryTree` |
| --- | --- |
| 1 root is a boundary | **Fails on "reported".** A symlinked directory is not expanded, but is drawn as an ordinary file row with no marker; the legitimate in-root link is a leaf. |
| 2 we supply the display text | **Fails.** Raw U+202E, a raw newline and U+FFFD are drawn; the label is `file_name().to_string_lossy()` and there is no hook. |
| 3 per-node status is ours | **Fails.** A row is an icon and a name; an unreadable directory is a `⚠` and grey text — colour and an icon alone, which D4 forbids. |
| 4 bounded | **Fails.** 100 000 entries build 100 000 rows and lay them out in **2.2–2.3 s** against a 16.7 ms frame. |
| 5 keyboard | Holds, by absence: it does not listen; `handle_key` runs only when the app calls it. |
| 6 no writes, no drag | Writes: none. **Drag fails**: no switch, a row press is a drag-machine event, a drop target is highlighted. |
| 7 its own strings | Holds: nothing it wrote is drawn. |
| 8 cost | Seven new crates; no thread pool built; no `lucide-icons`; no `svg`; +0.56 % binary; MSRV 1.90 holds; no new advisory. |

Per D8, a widget that cannot expand the 100 000-entry directory without stalling is **a D3 failure,
not a performance note**. It is one.

**`ItemTree`, the widget's other type, is a mechanism this RFC did not name, and it is measured too.**
Fed by our scanner and our escaping it holds 1–7 — because it then does none of what made the widget
attractive: it does not scan, does not know the root, and draws whatever `Display` we give it. What it
would still buy us is caret and selection rendering and key-to-event mapping. Against that, measured
or read from source: a closed directory needs a **placeholder child** or it draws no caret; the
selected-row colour (`Color::from_rgb(0.2, 0.5, 0.8)`) and the text size (14) are **fixed**, so it
cannot follow our theme; the row's structure is visible only through `iced`, which D1's "assertable
without `iced`" wants us not to depend on; and it costs seven crates and +159 KB for that. **My
recommendation is our own rows. That is a judgment the rule does not cover, and the reviewer can
overturn it** — the price of doing so is in `qa-evidence.md`, and the harness that measured it is
committed.

**What our own composition already has**, measured against the same fixture (properties 1–7 by
construction, and pinned by `explorer/hostile_tests.rs`): the scanner reports an escaping symlink as
`Blocked(SymlinkEscape)`, refuses to list the escaping directory, caps a level at 256 and says it was
truncated, returns an unreadable directory as a typed error, keeps a non-UTF-8 name's exact bytes in
its path, and takes one scan of a 100 000-entry directory in about a millisecond. Property 8: zero
crates.

### Two constraints the measurement adds for 052-B

1. **The per-level cap is not a total cap.** About **4 µs a row** to build and lay out, so roughly
   **4 000 visible rows fit a frame**. One capped expansion costs ~2 ms; twenty open at once cost
   ~19 ms. 052-B decides, by the same rule (measure, then decide), whether it bounds the *total*
   visible rows or windows the list. D2's "per level" was not written with this in view.
2. **The scan must not run on the render thread.** A scan is linear in path length: 0.3 ms at depth
   100, 28–38 ms at 1 000, ~65 ms at 1 500. Real paths are far shallower, so this is a hazard rather
   than a defect — but it crosses a frame, so the mechanism must be `Task`-shaped from the first
   commit, not retrofitted.

### Revisit condition

`iced-swdir-tree`'s manifest lists the same author as this workspace, so the failures above are not
permanent facts about the widget. If `DirectoryTree` gains (a) a label hook, (b) a per-row status
slot, (c) a bounded scan and a virtualised list, and (d) a switch for its drag machinery, the
committed harness (`rfcs/handoffs/052-file-explorer/measurement/`) is the check to re-run — with
the ablations first, as here. Until then this decision stands.

## D3′ accepted at review 421, with the argument the schedule adds

**Accepted as decided: compose our own rows and expansion.** The measurement stands on its own — five
of eight properties failed, read back from what the widget draws rather than from what its code
intends. Two additions from the reviewer.

**`ItemTree` is ruled out for a reason the implementer could not weigh: the schedule.** It holds the
safety properties when fed by our scanner and our escaping, and the seven extra crates and the caret
placeholder are real costs but arguable ones. What is not arguable: `item_tree.rs` draws every label
`.size(14)` and paints selection `Color::from_rgb(0.2, 0.5, 0.8)` — **verified in the crate's source
at review 421** — and **RFC-054 ships user-configurable theme and font size as the very next release**
(`0.25.0`). Adopting a widget that hardcodes both, one release before telling users they can change
them, means breaking that promise in the explorer or forking the widget immediately. That decides it
independently of size or dependency count.

**The revisit condition gains a fifth item.** If `DirectoryTree` later gains a label hook, a status
slot, a bounded scan and a drag switch, it must **also follow the host's theme** — text size and
selection colour from the caller, not constants — before the committed harness is worth re-running.

### Two findings PR-052-B inherits, with their rule

**A per-level cap is not a total cap.** Measured: ~4 µs a row, so ~4 000 rows fill a 16.7 ms frame;
twenty directories open at the 256 cap is ~19 ms. **D2's "per level" was written without this in
view.** B decides between a total visible-row bound and viewport virtualisation, by the same
measure-then-decide rule D3 used — with one property that is not open: **nothing is hidden silently.**
Whatever the tree does not render, it says, in a row that names how many are not shown.

**The scan is linear in path length** — 65 ms at depth 1 500, the same for the widget and for us — so
it crosses a frame on a deep tree. **The scan is `Task`-shaped from B's first commit**, not retrofitted
once someone notices a stall.
