---
title: "RFC-052 — QA evidence"
rfc: "RFC-052"
created: "2026-09-24"
---

# QA evidence

## PR-052-A — the fixture, and the mechanism decision

Measurement only. **No user-visible change.** The decision is recorded as **D3′** at the end of
[RFC-052](../../accepted/052-a-file-explorer-a-user-can-read.md).

### What was built

| | What | Where |
| --- | --- | --- |
| The fixture | **One** standard-library-only builder: every row RFC-052 names, in a fresh directory under the system temp directory, its own state, nothing read from the developer's machine. | `crates/tekstide-core/src/project/root/explorer/hostile_fixture.rs` |
| The guards, judged against it | Eight tests over the same fixture, the falsifying ablation first. | `…/explorer/hostile_tests.rs` |
| The mechanism measurement | A standalone crate (its own empty `[workspace]`, **not a member**, outside `crates/` so neither published crate packages it) that `#[path]`-includes the *same* fixture file, points `iced-swdir-tree 0.9.3` at it, and reads back what the widget draws through iced's own `Operation::text`. | `rfcs/handoffs/052-file-explorer/measurement/` |

The fixture's rows: a bidi-override name (`invoice\u{202E}fdp.exe`), a newline name, a **non-UTF-8**
name (`bad-\xFF\xFE-name.txt`), a symlinked directory and a symlinked file that both escape the root,
a broken symlink, an in-root symlink, **100 000** entries in one directory, **1 500** nested
directories, a mode-000 directory, an ordinary control tree, and an `outside/` sibling holding what the
escape links point at.

### The falsifying ablation, run first

`with_the_guards_removed_the_hostile_fixture_actually_escapes_and_renders_raw` runs the *same* fixture
through a guardless listing and asserts the hazards are real: `read_dir` of `links/escape-dir` **lists
`secret.txt` from outside the root**; the bidi name reaches a renderer containing U+202E; the newline
name contains `\n`; the non-UTF-8 name has `to_str() == None` and its lossy form invents a U+FFFD the
file does not have; the broken link is a real dangling link; the unreadable directory really refuses.
If this test ever stops passing the fixture has stopped being hostile and every test after it proves
nothing.

**Then the production guards were removed one at a time**, each from a committed tree, restored, and
`sha256sum` compared before and after:

| Guard removed | Failed |
| --- | --- |
| `node_for_entry`: a blocked entry reported `Available` | `the_escape_rows_are_blocked_rows_…` (new) **and** the two pre-existing `scanner_labels_in_root_symlink…` / `scanner_reports_broken_symlink…` |
| `escape_untrusted_chars`: pass everything through | `hostile_names_are_escaped_at_the_one_choke_point_…` (new) **and** two pre-existing config tests |
| The 256-child cap | `a_directory_of_many_entries_is_capped_…` **and** `a_hundred_thousand_entry_directory_is_a_bounded_scan_…` |

Hashes identical after restoring (`explorer.rs` `c5193be2…`, `text_safety.rs` `6d139c84…`).

The unreadable row needs a filesystem that refuses. As root, mode 000 refuses nothing, so the test
**asserts its own precondition and skips with a message** rather than "proving" a guard nobody asked.

### The measurement harness has its own ablations

The `CHECK` lines only mean something if they can fail. Each was made to, on the composition:

| Change to the harness | Failed |
| --- | --- |
| Raw `n.name` instead of `quote_untrusted` | `D3.2` |
| The `(blocked)` word removed | `D3.1b` and `D3.3` |
| The scanner's cap removed (`usize::MAX`) | `D3.4a`: 100 000 rows, **785 ms** |

`main.rs` restored, `sha256sum` identical.

### The eight properties

Two mechanisms, one fixture. **`DirectoryTree`** is the widget as shipped: it walks the filesystem
itself. **The composition** is the widget's `ItemTree` rendering nodes that *our* scanner
(`FileExplorerScanner`, the real one, as a path dependency) produced, with names that *our*
`quote_untrusted` escaped — the widget's rows with none of its filesystem access. What each draws is
read back from the widget tree, not from what the code meant to draw. Full output, two runs:
`measurement/results-run1.txt`, `results-run2.txt`.

| # | Property | `DirectoryTree` as shipped | Composition (`ItemTree` + our scanner) |
| --- | --- | --- | --- |
| 1 | **Root is a boundary** | **Half.** A symlinked directory is *not expanded* (`swdir` classifies by the entry's own type, so `escape-dir` is `is_dir = false`) — but it is **not reported** either: it is drawn as an ordinary file row with no marker, and the legitimate in-root link is listed as a leaf too. There is no notion of a root beyond the path it was given. **FAIL** on "reported". | **PASS.** `Blocked(SymlinkEscape)` is drawn as `(blocked)`; `escape-dir` has no caret; the in-root link still opens (`resolve_existing` keeps it inside). |
| 2 | **We supply the display text** | **FAIL.** `file_name().to_string_lossy()` is the label, with no hook. Read back from the drawn tree: **raw U+202E drawn, raw newline drawn, U+FFFD drawn.** | **PASS.** Drawn as `<U+202E>` and `<U+000A>` inside isolate marks; the non-UTF-8 row keeps its exact bytes in its *path*. |
| 3 | **Per-node status is ours to draw** | **FAIL.** A row is icon + `file_name`. There is no slot for a Git badge or a word. An unreadable directory is a `⚠` icon and grey text — **colour and an icon carrying meaning alone**, which is what D4 forbids. | **PASS.** Whatever is in `Row`'s `Display` is drawn: `(blocked)`, `(unreadable)`. |
| 4 | **Bounded** | **FAIL.** 100 000 entries: `swdir::scan_dir` alone **67–79 ms**, scan + normalise + update **83–95 ms**, all **100 000 rows are built** (52 ms) and laid out (**2.2–2.3 s**) against a **16.7 ms** frame. An unreadable directory is a row (pass) and nothing panicked. | **PASS.** 256 rows and a truncation row: scan + model + `set_tree` **1.0 ms**, build + layout **1.2 ms**, **2.1–2.3 ms** total. Unreadable is a typed error → a row. |
| 5 | **Keyboard does not outrank `KeybindingPolicy`; a modal suppresses** | **PASS, by absence.** It does not listen for keys: `ArrowDown` + `Enter` through iced's simulator emitted **no messages**. `handle_key` is `&self` and only runs when the *app* calls it. | **PASS**, same. |
| 6 | **No writes; no drag-and-drop** | **Writes: none** (its source touches the filesystem only to scan). **Drag: FAIL.** There is no switch; one click emitted `Drag(Entered)`, `Drag(Pressed)`, `Drag(Released)` — a row press *is* a drag-machine event, and a drop target gets highlighted. We could ignore `DragCompleted`, but the user would see drop feedback for a drop we do not accept. | **PASS.** `ItemTree::with_drag_and_drop` defaults to `false`; a click emitted nothing of its own. |
| 7 | **Its own strings are ours or unused** | **PASS.** No word it wrote is drawn (its `Error` display, `I/O error at …`, is never drawn). | **PASS.** Every word drawn is a `Row`; the only string of its own is a one-space alignment spacer. |
| 8 | **Cost** | See below — the same crate either way. | See below. |

**Property 8, measured** (`ItemTree` adopted in a scratch worktree, a probe referencing its `view`
and `update` so the linker kept them, `--locked`, nothing committed):

| | Measured |
| --- | --- |
| New crates | **7**, all in `Cargo.lock`: `iced-swdir-tree`, `swdir`, `rayon`, `rayon-core`, `crossbeam-deque`, `crossbeam-epoch`, `either`. (`cargo tree -e normal` goes 335 → 343 lines; one of the eight is a repeated `iced` line, not a crate.) |
| `rayon`'s thread pool in our process | **None built.** Threads before/after the widget scanned 100 000 entries: **1 / 1**. `swdir::scan_dir` documents that "rayon overhead is not justified" for one directory; the pool exists only for its recursive `walk`, which the widget does not call. |
| `lucide-icons` (561 KB) | **0.** It is behind the widget's `icons` feature, which is off. `UnicodeTheme` (▸ ▾ 📁 📄 ⚠) is the default. |
| `iced`'s `svg` feature | **Not required.** `cargo tree -e features` shows no `iced feature "svg"`. |
| Release binary | **28 597 368 → 28 756 688 bytes (+159 320, +0.56 %)** with `ItemTree` referenced. `DirectoryTree` was not measured: it is rejected on 2, 3, 4 and 6. |
| MSRV | **1.90 holds.** `cargo +1.90 check --workspace --locked` passes with the dependency. `tekstide-core` does not take it, so its 1.89 is untouched. `rayon` and `rayon-core` declare 1.80. |
| `cargo audit` on the modified lock | The **same three carried advisories** (`paste`, `ttf-parser`, `lru`), zero vulnerabilities, **nothing new**. |
| The composition with no dependency | **0 crates.** |

### Two measurements D3 did not ask for, and 052-B needs

**The per-level cap is not a total cap.** Section E of the harness opens *N* directories of 300 files,
each capped to 256 rows, at once, and lays out the composition:

| Directories open | Rows | Widget build + layout |
| --- | --- | --- |
| 1 | 256 | 1.3 ms |
| 5 | 1 280 | 4.9 ms |
| 10 | 2 560 | 9.2–9.6 ms |
| 20 | 5 120 | **18.8–19.8 ms** |
| 40 | 10 240 | **41–42 ms** |

About **4 µs a row**, so **~4 000 visible rows fit a 16.7 ms frame** — and `ItemTree` builds every
visible row, as would any column of our own rows. D8's "does not block a frame" is met by *one*
expansion (2.2 ms); it is **not** met by a user who opens twenty full directories. That is a constraint
on 052-B, not a decision made here: the same rule (measure, then decide) applies to whether 052-B
bounds the *total* visible rows or windows the list.

**Scanning gets slower with depth, because paths get longer.** One scan at depth *N* (the widget's
`scan_and_feed`, and our own `scan_directory` in `hostile_tests`, agree):

| Depth | Widget | Ours |
| --- | --- | --- |
| 100 | 0.30 ms | 0.33 ms |
| 500 | 7.0 ms | 10.0 ms |
| 1 000 | 27.6 ms | 37.6 ms |
| 1 500 | 64 ms | 67 ms |

Linear in path length per scan, so a full walk of 1 500 levels is quadratic (about **30 s**, measured;
which is why `hostile_tests` *samples* levels instead of walking them). A real path is under a hundred
deep (0.3 ms) and a user clicks one level at a time, so this is not a frame problem in use — but it
crosses a frame at about 700 levels, so **the scan must not run on the render thread in either
mechanism.** Nothing overflowed the stack at 1 500 levels: expand, view, layout and drop all
completed.

### Judgment calls, disclosed

- **`ItemTree` is a mechanism RFC-052 did not name.** D3 is written about "the widget", meaning the
  directory widget that walks the filesystem. `ItemTree` passes properties 1–7 *only because it does
  none of what made the widget attractive* — it does not scan, does not know the root, and needs a
  placeholder child for a closed directory (a node with no children has no caret). I measured it
  anyway because it is the honest third option and D3's rule leaves it unruled. D3′ records the
  rule's own outcome for `DirectoryTree` and my recommendation on `ItemTree`, and **says plainly that
  the second is a judgment the reviewer can overturn**, with the price of doing so.
- **The 100 000-entry widget timing is an upper bound on the render thread.** In production the scan
  and normalise run on a worker; only `update(Loaded)` runs on the render thread, and the widget's
  `LoadPayload` fields are crate-private, so that step cannot be timed alone. Build + layout (2.3 s)
  is the number that decides it and is unambiguous.
- **`scan_and_feed` is the crate's own `#[doc(hidden)]` test shim.** It drives `Toggled` and `Loaded`
  synchronously, "exactly like the async scan task, minus the thread hop". Used because the
  alternative is running an iced executor; the shim is not covered by semver and this harness pins
  `=0.9.3`.
- **One row of `hostile_tests` is a timing.** It asserts a capped scan of 100 000 entries is under
  **2 s** (measured **~1 ms**). Deliberately loose: it pins "bounded by the cap, not by the directory",
  not a machine's speed.
- **`/tmp` was full during this slice** (a 30 GB tmpfs at 100 %, almost all of it other projects'
  scratch), which broke a `cargo check` mid-run. The MSRV run was redone with `TMPDIR` and
  `CARGO_TARGET_DIR` on `/home`. Nothing here depends on it; it is named because the gate's runs write
  to the temp directory.

### Gate

`cargo fmt --check`, `clippy --workspace --all-targets -D warnings`, `git diff --cached --check` after
staging, and **three consecutive full-workspace runs with `--all-targets --no-fail-fast`, to files:
593 + 9 + 885, green all three** (the 9 include `rfc_docs_invariants`). Against `0.23.0`'s 593 + 9 + 877, core is
**+8: exactly this slice's eight tests**; nothing else moved.
An earlier three-run attempt had one already-registered flake (row 8) in run 3; it is dated in
`test-process-leak.md` with the reason and the run was redone rather than counted. The same note
records that a *long* `TMPDIR` fails 47 tests with `SocketPathTooLong`, which cost one whole attempt.

### Scope

Nothing under `crates/*/src` changed except registering two `#[cfg(test)]` modules in `explorer.rs`.
No catalog string, no surface, no dependency, no `Cargo.lock` change in the product.

## Required at review 421 — the suite stops leaving its fixtures behind

Reviewer's count on this machine: **42 932 entries under `/tmp`**, of which 14 780 `tekstide-run-*`,
3 504 `tekstide-audit-test-default-*`, 1 517 `approval-audit-*`.

| Builder | Fix | Pinned by |
| --- | --- | --- |
| `resolve_agent_run_state_dir` (`tekstide-run-*`) and `resolve_audit_state_dir` (`tekstide-audit-test-default-*`) | The per-thread default is a `TestStateDir` that **owns** its directory and removes it when the thread ends — each `#[test]` has its own thread, so that is the end of the test. A directory a test *named* (`test_audit_state_dir`) is marked not-owned and never removed. | `a_tests_default_state_directories_are_removed_when_its_thread_ends`; `a_directory_a_test_named_is_not_removed_by_the_seam` |
| `TestAudit` (`approval-audit-*`) | `Drop` removes the state root. | `a_test_audit_removes_its_state_root_when_it_is_dropped` |

**Ablation, three.** Never remove → the thread-end test fails, alone. `TestAudit::drop` a no-op → its
test fails, alone. `named` marked owned → the *named-directory* test fails, alone (the direction the
first ablation cannot reach). Each restored from a committed tree.

**A regression I caused and the gate caught.** `sentinel_command_text_never_reaches_the_durable_audit_store`
drops its `TestAudit` *so the store checkpoints*, then reads every file in the directory. The new `Drop`
removed the directory first, so the scan read nothing and the positive control failed — correctly. Fixed
with `close_keeping_files`, which closes the store but leaves the directory for the test to remove itself;
it now reads, removes, and *then* asserts, so a failing assertion does not leave the directory behind.
Not a flake: it failed deterministically and I did not register it as one.

**Measured.** Each full-workspace run into a fresh `TMPDIR` now leaves **≈497 entries** (498, +497, +497
across three runs) where the same suite previously left several thousand a run; **none** of the three targeted prefixes remain. What is left, by count:
`tekstide-shell-test-session-limit-benchmark`, short-named `t`/`tsr`/`tsms` builders, and the
`tekstide-shell-test-*` family — **not fixed here**, named so they are not mistaken for done.

`ARCHITECTURE.md` now carries the `TMPDIR` lesson (short, because the approval socket lives under it;
logs somewhere with space) and the rule that a fixture builder removes itself.

**Gate**: fmt, clippy `-D warnings`, `git diff --cached --check`, and three consecutive full-workspace
runs, `--no-fail-fast`, to files: **595 + 9 + 886, green all three** (+2 shell, +1 core: exactly the three
new tests).

## PR-052-B — the tree

Commits `97576f3` (the residue, first, as ruled at review 422), `cda2d6e` (the tree), `5573582`, and the
follow-ups named below. Captures: `evidence/01-…` and `evidence/02-…`, the **release binary** against a
`mktemp -d` project under `/dev/shm`, read image by image for a path (none; the window shows only `proj`).

### The residue, first

Every bare-`PathBuf` fixture builder now hands its path to a per-thread scratch list
(`scratch_for_this_test` in `tekstide`, `test_support::remove_when_this_test_ends` in `tekstide-core`) that
removes it when the test's thread ends: no caller changes. **A full-workspace run into a fresh `TMPDIR` now
leaves 0 entries** (it was ~498 after review 421, ~43 000 before). Pinned in both crates by a test that a
directory *and* a bare file are gone after the thread ends; ablated (both helpers no-ops) → both fail.
**A mistake, disclosed:** I ran `git checkout -- crates/tekstide/src/shell.rs` to restore that ablation
while the file still held uncommitted work, and lost the helper; I had the diff in this conversation and
re-applied it, and re-ran both pins green. The standing rule (commit before ablating) was the one I broke.

### What changed

| | Change | Where |
| --- | --- | --- |
| Model | `ExplorerTree`: the scans it holds, which folders are open, which scans are in flight; flattens to rows. Pure, no `iced`, never scans by itself. | `tekstide-core` `project/explorer_tree.rs` |
| Count | A capped scan **counts what it left out** by draining the directory without stat-ing, on the scanning thread; says "at least" if it stopped at its own limit (1 000 000). | `project/root/explorer.rs` |
| Off the render thread | `ensure_explorer_scanned` and a folder toggle only **mark a scan pending**; `explorer_scan_subscription` runs it on a dedicated thread (the `git_summary_subscription` shape) and `Message::ExplorerScanFinished` applies it. Stale results are dropped by generation. | `shell.rs` |
| Rows | `row_text`: indentation (two spaces a level), `[+]`/`[-]`, then the escaped, catalog-routed row. The `Parent` row and `explorer-parent-entry` are gone. | `surface/explorer.rs` |
| Bound | **Virtualisation**: only the rows that fit are built. A line says which (*Rows 18–60 of 272*); the model itself stops at 10 000 rows with a row naming how many were not kept. | same |
| Order | Kind, state, symlink and Git words come **before** the name. | `en.ftl` |
| Detail | The highlighted row, whole, under the tree. | `surface/explorer.rs` |
| Keys | `Enter` toggles a folder, opens a file; `Up`/`Down` move; the window follows. | `shell.rs` |

### The total-row bound, decided by measurement (the rule from review 421)

PR-052-A measured ~4 µs a row to build and lay out. Measured again against the real view with a real
headless renderer (`drawing_the_largest_tree_builds_only_the_window_and_fits_inside_a_frame`, debug build):

| | Build + layout |
| --- | --- |
| The window of the largest tree the model allows (10 000 rows, 28 drawn) | **3.0 ms** |
| Building all 10 000 rows | **951 ms** |

**Virtualisation, not a cap**, because a cap hides rows and D8's one open property is that nothing is
hidden silently; here nothing is dropped and a line says which rows are on screen. The model's own bound
(10 000) stays as a safety, flattens in ~47 µs for 3 589 rows, and ends in a row that names what it did not
keep. **D8 for the 100 000-entry folder:** the worker (scan + count) takes **25 ms**; the render thread
(apply + flatten) takes **14.5 µs**, against a 16.7 ms frame.

### Two defects the live capture found in my own first version, both fixed

1. **A clipped row lost its Git word.** The sidebar draws each row on one line and clips at its edge, and
   `[FILE] new.md [untracke` cut the status the RFC says must stay a word. Fixed by putting kind/state/
   symlink/Git before the name (the name is the only part that can be arbitrarily long), pinned by
   `every_status_word_comes_before_the_name_so_only_the_name_can_be_clipped`.
2. **The escape row's own report was clipped** (`[OTHER] (blocked) [symli`). Status-before-name is not
   enough for a nested row, so the **detail area** shows the highlighted row whole, escaped, wrapped over a
   fixed number of lines so the window arithmetic stays exact. The two "N more not shown" rows are sentences
   that clip too, so they have a detail and a shorter wording.

### Ablations — from a committed tree, each restored with `git checkout --` of that one file

| Removed | Failed |
| --- | --- |
| Any entry expandable (kind and state ignored) | 3 core tests (incl. the escape test) + 4 in `tekstide` |
| **Follow the link**: any non-file entry expandable | the tree's escape test, and the detail test whose fixture is that same row |
| Escaping off (`escape_untrusted_chars`) | 35 tests across the workspace lean on that one choke point (22 in `tekstide`, 13 in `tekstide-core`); the explorer's own are `a_bidi_override_node_name_…`, `hostile_names_at_any_depth_…`, `the_detail_shows_…`, the core hostile-names test |
| Row bound removed | `the_tree_is_bounded_…` (core) and `passing_the_row_bound_…` (surface), nothing else |
| Window ignores the highlight | 3: the rule, the position line, the shell test that walks down a long list |
| Root scanned synchronously in `ensure_explorer_scanned` | `the_shell_never_scans_a_directory_on_the_render_thread`, `starting_the_explorer_…`, and one more |
| Status words after the name | the ordering test and the detail test |
| Stale-generation check removed | `a_stale_result_is_dropped_…`, alone |
| Omitted count reported as 0 | 5 (scanner, tree, hostile fixture, 100 000-entry, shell) |

### What replaced the tests that changed shape (RFC-052 §7)

| Old | Now |
| --- | --- |
| `visible_rows_never_exceeds_…_plus_the_parent_entry`, `the_parent_row_resolves_through_the_catalog` | `a_trees_rows_are_exactly_its_scans_nodes_and_there_is_no_parent_row`; `the_rows_that_only_say_something_resolve_through_the_catalog` |
| `a_truncated_scan_renders_the_truncation_notice` | `a_truncated_scan_names_how_many_entries_it_left_out` (exact, singular, "at least") |
| Two RFC-019/RFC-038 tests naming the one production call site of each synchronous scan | **`the_shell_never_scans_a_directory_on_the_render_thread`**: none has a call site; the request runs only inside the spawned thread of `explorer_scan_stream`; `FileExplorerScanner` is not named in `shell.rs` |
| `failed_explorer_scan_clears_previous_scan_result` (a one-view idea) | the same test now fails the **root**, plus `a_failed_scan_of_one_folder_does_not_erase_the_root_listing` |
| every escaping / status / git-badge test | unchanged in what they assert; `node_line` gained the `expanded` argument |

`node_line`'s escaping tests hold as they were; `hostile_names_at_any_depth_never_reach_a_row_raw` adds the
depth and the newline (a newline in a name must not become a second line).

### Judgment calls, disclosed

- **The collapse list still opens.** D7 says the list "stays exactly as it is"; the old explorer let you
  step into `target/` and it was labelled *(collapsed)*. I kept that: those folders are labelled while
  closed, the word is dropped once open (it would contradict the rows under it), and the same 256 cap
  applies. If D7 meant "cannot be opened", that is `is_expandable` returning `false` for `Collapsed`.
- **Virtualisation needed a measured sidebar.** `MeasureSize` now wraps the sidebar
  (`Message::ExplorerViewportMeasured`), the RFC-053 D3′ rule: measure, do not compute. Until the first
  layout the window holds 20 rows.
- **The scan is a subscription with a thread, not `Task::perform`.** The shell's update helpers return
  `()`, and the same shape already runs the Git summary; the property D3′ asked for (off the render thread,
  pinned by a test) holds either way.
- **Rows are not clickable.** The old explorer had no mouse either; expanding is by `Enter` only. Not in
  the plan; named so it is not mistaken for done.
- **Ordering is unchanged** (`.git`, `README.md`, `docs`… by name, files and folders interleaved). A
  conventional tree lists folders first; that is a one-line sort change, and it reads as PR-052-C's ("how
  it reads"), not this slice's.
- **The lower-bound branch of the omitted count** (1 000 000) is tested through
  `FileExplorerScanPolicy::omitted_count_limit`, set to 10, not with a million files.
- **The measured window is 3.0 ms in a debug build**, released ~an order lower; the assertion is half a
  frame and its failure message prints the load average, as the change-review benchmark's does.
