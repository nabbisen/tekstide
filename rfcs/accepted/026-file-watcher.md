# RFC-026: File Watcher

Status: **Accepted by the human owner 2026-09-30.** D1–D8 as written, plus D9–D12 — see *Decided on acceptance*. Proposed 2026-09-30. `0.29.0` in the authorised schedule, and **M13 proper**. Closes
`REQ-FILE-003` (the explorer updates when files change externally), `REQ-FILE-004` (watching is
debounced and does not block editor input), `NFR-PERF-007` (batched under churn), and the
single-document limit that "multi-document editing" in the 1.0 list depends on.

## Summary

The explorer reads a directory when you open it and never again: a file created afterwards appears
when you close and reopen the folder. One document is open at a time. Both were scheduled long ago —
and two things have changed since, which decide how this is built.

**RFC-055 made each directory scan ask git**, so a re-scan is now a subprocess. An event storm would
be a subprocess storm, and batching stops being an optimisation. **RFC-055 also gave us the ignore
answer**, which is what makes the watch budget affordable: this repository is 18,974 directories, and
1,485 once the ignored ones are gone.

## What is true today, measured

| | Measured |
| --- | --- |
| 1 | `active_document: Option<TextDocument>` (`project/content.rs:22`) — singular, and **74 production call sites** read it, across `project/content.rs`, `app.rs`, both `shell.rs`, `navigation.rs`, `project/session.rs` and `audit/integration.rs`. |
| 2 | A document is no longer just text. Since RFC-057 it carries `text`, `last_known_snapshot`, `cursor`, `viewport`, **`undo_stack`, `redo_stack` (bounded at 500) and `undo_bound_reached`**. N open documents multiply all of it, not only the bytes. |
| 3 | `DEFAULT_MAX_EDITABLE_BYTES` is **4 MiB per document**. |
| 4 | **No watcher crate is in the dependency graph at all** — zero matches for `notify`/`inotify` in `Cargo.lock`. This would be the **first new dependency in many releases**, and every recent gate record says "No dependency added". |
| 5 | This machine allows **524,288 inotify watches and 1,024 instances**, and I found **no `sysctl` override setting them** — so that is this kernel's own value and another machine's may be far lower. **The budget has to be treated as unknown.** |
| 6 | This repository is **18,974 directories** in total and **1,485** excluding `.git`, `target` and the built book. Watching everything costs about **13×** what watching the interesting subset costs. |
| 7 | `ExplorerTree` already tracks **`expanded: BTreeSet<PathBuf>`** — the directories a user has actually opened. A watch scope already exists; nothing reads it for this purpose yet. |
| 8 | **A re-scan is now a subprocess.** RFC-055 made every directory scan ask git: the gate's configuration half (~1.3 ms) plus one `check-ignore` (~1 ms, measured at review 431). |
| 9 | External change already has a state machine — `TextDocumentState::{ExternalChanged, Conflict}`, `refresh_external_state`, and a Reload path that constructs a **fresh** `TextDocument` (proved at RFC-057 PR-057-D, which is why undo cannot cross a reload). |

## Decisions required

**D1 — The watcher watches what the user opened, not the project.** The project root, the directories
in `ExplorerTree::expanded`, and the directories holding open documents. Measurements 6 and 7: a
recursive watch costs about thirteen times as much on this repository alone, and the set that matters
is already tracked. The scope grows and shrinks with what the user expands.

**D2 — The watch budget can run out, and the product says so when it does.** Measurement 5: the limit
is a per-user kernel resource whose value we cannot assume. Exhaustion is neither a crash nor silence
— the explorer falls back to exactly what it does today, stale until reopened, and **the sidebar says
watching stopped and why**, in the same vocabulary RFC-055 uses when git cannot answer.

**D3 — Events are batched before they reach a scan, because a scan is a subprocess.** Measurement 8
is why `NFR-PERF-007`'s "batched under high churn" is a requirement here rather than a nicety: one
scan per directory per debounce window, regardless of how many events arrive in it. **The window is
stated in the book**, not tuned silently.

**D4 — A new dependency, decided by measurement.** `notify` is the obvious candidate and nothing in
the graph competes with it. It is judged the way RFC-052 D3 judged a widget: against **our**
properties — bounded work under churn, no unbounded recursion, what it does with a symlink, and what
it does when the kernel refuses another watch — and adopted only if all of them hold, read from what
it **does**, not what its README says. Watching inotify directly stays the fallback; Linux is the
primary target through M13 either way.

**D5 — Multi-document is bounded, and the bound is stated.** Measurement 2 and 3: every open document
is up to 4 MiB of text plus two undo stacks of up to 500 operations. A cap on simultaneously open
documents, said plainly when reached, in the same shape as RFC-056's record caps and RFC-057's undo
bound.

**D6 — `active_document` keeps its meaning; what becomes plural is the set that is open.** There is
still exactly one *active* document. Measurement 1 is the argument: of 74 call sites, the ones that
must change are the ones that **count** — unsaved files, the close dialog, save-all — and the rest
keep working because "the document the user is looking at" still exists. A rename that touches 74
sites to prove a point is how this slice goes wrong.

**D7 — The watcher watches only paths the explorer's access policy already admits.** `REQ-SEC-043`
names unbounded recursive symlink loops. The policy that decides which paths are readable is the
policy that decides which are watchable; the watcher is not a second way into the filesystem.

**D8 — An external change arrives sooner, never differently.** Measurement 9: the states, the
conflict rules and the fresh-document reload already exist and are proved. A watcher must make
`ExternalChanged` arrive without the user re-opening the folder, and must not invent a second path to
it — in particular it must not silently reload a document with unsaved edits.

## Non-goals

Crash recovery and unsaved-buffer persistence (RFC-027, `0.30.0`). Watching outside the project root.
Reacting to changes in files nobody has open and no expanded directory contains. A file-system
abstraction layer. Windows and macOS watching, which is M14's, per the roadmap's own rule that
cross-platform evidence is produced per platform and never inferred.

## Risks

**R1 — the dependency.** Measurement 4: this would be the first added in many releases, in a project
whose gate record has said "No dependency added" every time. It arrives with a `cargo audit` row, an
MSRV interaction and a supply chain, and D4 is the gate.

**R2 — an event storm meeting a subprocess.** A build writing a thousand files into a watched
directory must cost one scan, not a thousand. The fixture is a real burst, not a loop with a sleep in
it.

**R3 — the watch budget on a machine unlike this one.** Measurement 5 says we cannot test the
interesting case by having a generous limit. Exhaustion has to be **forced** in a test, not waited for.

**R4 — 74 call sites.** D6 is the mitigation and it is not a guarantee; the ones that count are the
ones that will be missed, because a count that is wrong still compiles.

**R5 — a reload racing an edit.** An external change arriving while the user is typing must not
discard what they typed. RFC-057 gave every document an undo history; a reload that throws it away
silently is `NFR-REL-005`'s own failure.

**R6 — the watcher outliving what it watches.** A directory that is deleted, or a project that is
closed, must drop its watches; otherwise the scope grows monotonically and R3 arrives on this machine
too.

## Acceptance criteria

- **A file created in an expanded folder appears without the user reopening it**, captured live.
  `REQ-FILE-003` moves to met on that capture.
- **A burst writes 1,000 files into a watched directory and costs one scan per debounce window**, with
  the number of scans and the number of git subprocesses both **counted and written down** — not
  "batched" as a claim.
- **The watch budget is forced to exhaustion in a test**, and the result is the floor behaviour plus a
  sentence on screen, never a crash and never silence.
- **Typing does not stop.** `REQ-FILE-004`: an editor keystroke's latency under a watched burst,
  measured against RFC-057's own baseline harness, not asserted.
- The watch scope is what the user opened: expanding a folder adds watches, collapsing it removes
  them, closing a project removes all of its. Counted before and after.
- **A symlink that leaves the project root is not watched**, and a symlink loop does not make the
  watcher recurse. The hostile fixture, not reasoning.
- **An external change to a document with unsaved edits does not discard them**, and the undo history
  survives or the product says it did not.
- Two documents open: the unsaved count, the close dialog and save-all each count **both**, proved by
  a test that fails if either is counted once.
- The open-document cap is stated when reached.
- `cargo audit` gains no vulnerability, and the new dependency's advisory position is recorded in
  `dependency-advisories.md` the way the existing three are.
- Gate green three times with `--no-fail-fast`, **0 fixture entries left** in a fresh short `TMPDIR`,
  and the core pin bumped with the version.

## If this is larger than it looks

The watcher and the document model are separable, and the split point is stated **now** rather than
discovered: the watcher (`REQ-FILE-003`, `004`, `NFR-PERF-007`) can ship as `0.29.0` alone, with the
multi-document model as `0.30.0` and RFC-027's crash recovery moving one release later. Taking that
option is a scheduling decision for the architect at review, on evidence — not a scramble at the
candidate.

## Re-scoped 2026-10-07: the multi-document model is RFC-065

**This RFC is now the watcher alone.** `REQ-FILE-003`, `REQ-FILE-004` and `NFR-PERF-007` — met and
measured. The single-document limit, D5's cap, D6's open set and D9's two counts move to **RFC-065**,
shipping as `0.30.0`, with two findings this RFC produced:

- **D8's refresh is O(file size), per document** (review 464). `refresh_external_state` reads the whole
  file, about 4 ms at the 3.3 MB fixture. With the open set plural, a burst touching N documents is N
  whole-file reads in one drain window, each up to the 4 MiB cap.
- **A save is a self-caused write into a watched directory** (review 464). It does not loop — the
  refresh finds the text matches disk — but every save costs a scan and a whole-file re-read of the
  file just written.

**Why the RFC split and not only the release.** A released changelog section may not name an RFC still
in `accepted/`, the invariant carried from RFC-053's late closure. An RFC spanning two releases would
trip it, or force `0.29.0`'s changelog to describe its own subject without naming it. Two halves with
different requirements, different risks and now different releases should have been two RFCs.
D1–D12 stand as written; D5, D6 and D9 travel to RFC-065 with the text that justified them.

## Decided on acceptance (2026-09-30)

**D1–D8 as written.** Four additions, one of which makes D6 checkable instead of hopeful.

**D9 — "the ones that count" is exactly two functions, and here they are.** `open_buffer_count()` and
`dirty_file_count()` (`project/content.rs:359` and `:363`) are each literally
`u32::from(<one Option>)`, and each has **one reader**, at `session.rs:1736–1737`. So the
multi-document change is two function bodies and what they feed — not seventy-four sites. D6 was a
hope when I wrote it; this makes it a claim a reviewer can check. **If a third counter turns up during
the work, that is a finding to report, not a detail to absorb.**

**D10 — The watcher is a subscription, in the shape the scan worker already has.** The explorer scan
runs off-thread through `explorer_scan_subscription` with newer-scan-wins, and its results arrive as
messages. The watcher is the same: events become messages, and nothing about watching touches the
render thread. Do not invent a second threading model beside the one that works.

**D11 — The batching is proved before any watcher exists.** R2's measurement — a thousand files
costing one scan per window — is a property of **our** debouncer, which we write, not of the
dependency. Build and falsify it against a simulated event stream first. Then D4's evaluation is
about the watcher alone, judged against a number that already exists. This is RFC-057 D11's lesson
applied to a dependency instead of a rewrite: the baseline comes first or the comparison is
unfalsifiable.

**D12 — What D4's evaluation must record, since it says measure.** The candidate as of today:
**`notify 8.2.0`**, MSRV **1.77** (comfortably under our 1.90 floor, so no pressure there), licence
**CC0-1.0** — unlike every other direct dependency we carry — and its last stable release
**2025-08-03**, more than a year before this RFC. None of those three disqualifies it and all three
belong in `dependency-advisories.md` at adoption, beside the answer to the question that actually
decides it: **what it does when the kernel refuses another watch.**

**Ships as `0.29.0`**, with the split point in *If this is larger than it looks* left open as an
architect's decision at review.
