---
title: "RFC-052 — QA evidence"
rfc: "RFC-052"
created: "2026-09-24"
---

# QA evidence

## PR-052-A — the fixture, and the mechanism decision

Measurement only. **No user-visible change.** The decision is recorded as **D3′** at the end of
[RFC-052](../../done/052-a-file-explorer-a-user-can-read.md).

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

**Correction, 2026-10-20, review 612.** That 25 ms worker figure was `OMITTED_COUNT_LIMIT`
(`project/root/explorer.rs`, introduced by PR-052-B, the same slice this section documents) bounding
the *stats*, not the *walk*: counting every one of a directory's omitted entries past the per-level
cap scaled with the real directory's size, not with the cap, so a capped scan cost as much as an
uncapped one once the hidden tail passed about ten thousand entries. Found when the measurement
crate's `Cargo.lock` was refreshed after sitting at `tekstide-core 0.23.0` for a month and the
composition arm's own D8 check (`measurement/results-run2.txt`, `scan+model+set_tree 1.02 ms`)
regressed to `24.44 ms` (`results-run3-2026-10-20.txt`) against `0.37.0` — the regression had been
live and uncaught since PR-052-B itself; this section's own "25 ms" was the first time anyone wrote
the cost down, not a baseline later lost. `OMITTED_COUNT_LIMIT` lowered `1_000_000` → `10_000`
(worst case ~2.5 ms, measured at ~0.24 µs/entry), after which a directory with more hidden entries
than the limit reports "at least `N`" rather than an exact count — the lower-bound machinery
`omitted_is_lower_bound` already existed for exactly this and had simply never been exercised at
this scale. Re-measured, same test: worker **4.46 ms** (was 25 ms), render thread **15.7 µs**
(unchanged — never the part that was slow). `measurement/results-run4-2026-10-20.txt`: composition
arm's D8 check **passes**, `4.82 ms` against the `16.67 ms` frame. The loose `< 2 s` test this
regression hid behind (`a_hundred_thousand_entry_directory_is_a_bounded_scan_not_a_walk_of_all_of_it`)
is now a machine-independent paired ratio instead, timing a 300-entry and a 100 000-entry scan in
the same run and asserting the ratio between them stays under 15x — it would have caught this.

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

### Gate

`cargo fmt --check`, `clippy --workspace --all-targets -D warnings`, `mdbook build docs`,
`git diff --cached --check`, and **three consecutive full-workspace runs, `--all-targets --no-fail-fast`,
to files, into a fresh `TMPDIR` each: 615 + 9 + 901, green all three, and `0` entries left in the temp
directory after all three.** Against `0.23.0`'s 593 + 9 + 877: +22 in `tekstide`, +24 in
`tekstide-core`. No intermittent this time. The gate ran **before** the push (review 422's lesson).

## PR-052-C — how it reads

Captures: `evidence/01-…` (the tree, before C's changes to the rows), `evidence/03-…` (C: folders first, icons, `[open]`, monospaced), release binary, `mktemp -d` project under `/dev/shm`.

### What changed

| | Change |
| --- | --- |
| Order | **Folders first**, then files, each by name compared without case, exact name as the tie-break (scanner, `explorer.rs`). |
| Icons | `▣` folder, `▢` open folder, `▫` file replace `[DIR]`/`[FILE]`. `[OTHER]` keeps its word: a link or special entry has no other channel that says what it is. |
| Selection | `[open]` marks the file open in the editor. The keyboard highlight is `>`. Independent: a row can have either, both, or neither. |
| Font | The tree and its detail area are **monospaced** (owner's request), and the sidebar is 300 px wide, not 220, to hold about thirty columns of it. |

### Two measurements that decided design

**Icons are text symbols, not emoji.** With the same 28-row window: colour-emoji glyphs cost **18 ms** to lay out (debug), symbols **3 ms** — six times — because each emoji goes through font fallback, and they need a font many machines lack. `the_kind_icons_are_text_symbols_not_emoji` fails if one comes back.

**Monospace costs ~9x in layout, and is still inside a frame.** Release build, headless renderer, 28 rows (`measurement/examples/font_probe.rs`): default font **0.32 ms**, `Font::MONOSPACE` **2.8 ms** — the generic family is looked up per text. 2.8 ms is 17 % of a 16.7 ms frame, so the owner's choice stands.

**Amended 2026-11-08: the in-suite test no longer carries an absolute bound at all** (`drawing_the_largest_tree_builds_only_the_window_and_fits_inside_a_frame` — five occurrences of a load-sensitive red against the old 80ms-debug/8ms-release bound traced to a stale citation, not a product issue; see `test-process-leak.md`'s own dated section and the test's own doc comment). What it still asserts, unconditionally, is the one thing that has never failed across any of those five occurrences: the window is a tiny fraction of building the tree whole (~9 s scaled from a 500-row sample in a debug build; the test no longer builds all 10 000, which took 9 s). **The half-a-frame budget itself moved here**, worst-of-5, in `measure`'s own `CHECK D-window` (`src/main.rs`, `results-run5-2026-11-08.txt`): a 22-row window (the same row count `rows_that_fit` measures in the real product for a 700px-tall area) costs **well under 1 ms** against the 8.33ms half-frame budget, built fresh each of 5 iterations, worst kept. Not a call into the product's own private row-formatting code (this crate has no library target to link against — see `src/main.rs`'s own doc comment on the check), so the row text is a plausible approximation, the same precedent `font_probe.rs` already set; the row count and widget kinds are real.

### D4, checked

An icon never carries meaning alone: `every_status_a_row_can_carry_is_a_word_and_none_is_an_icon` asserts every state, symlink and Git word is present for every value, and the icon slot holds only kind — which the row's position (folders first, under its parent, `[+]`/`[-]`) and its name also carry. What is **not** done: per-extension file-type icons (`.rs`, `.md`, …). They need an icon font, which is an asset and a dependency (D3 already rejected `lucide-icons` at 561 KB), so this is two icons, not a set. Named so it is not mistaken for done.

### Also reported by the owner during this slice, fixed as its own commits (not RFC-052)

The Project Board, seen in the same captures: the last card's "Open" button was cut through because the cards did not scroll; the open project's card had no button and said nothing; and every line of every card was the same size. Now: the cards scroll and the list follows the keyboard highlight; the open project's card says "Open now"; a card is a bold heading, a path, then **badges** — a word in a pill, never a replacement for one — for state and for each count, wrapping in a narrow window, with a heavier border *and* words for a project that needs attention. Regrouping is pinned to say exactly what the old nine lines said (`a_cards_sections_say_exactly_what_its_lines_say`). **Not verified visually:** the scroll following the highlight with a real key press; my captures of it were interrupted, and only the request is unit-tested (`moving_the_board_highlight_asks_the_card_list_to_follow_it`).

### Live-capture hazard, disclosed

`wtype` types into whichever window has focus, and this desktop runs other projects' apps. During C one run of keystrokes reached another project's window (`orbok`), which then showed a folder-chooser dialog. I did not touch that window. After that every key was sent through a helper that re-focuses my window and refuses if it is not focused.

### Gate

fmt, clippy `-D warnings`, `mdbook build docs`, `git diff --cached --check`, three consecutive full-workspace runs to files, fresh `TMPDIR` each: **626 + 9 + 902, green all three, 0 entries left.** An earlier attempt hit two registered intermittents under a load average of ~30 (other work on the machine); they are dated in `test-process-leak.md`, and the gate was redone when the load was under 6 rather than counted.

## Review 424 follow-up — the board scroll, B1, B2

**The scroll bug the reviewer measured was real, and my test could not have caught it.** `handle_project_board_row_key` set `project_board_scroll_request`; the only thing that turned it into a `snap_to` was the tail of `update()`, and the key-press arm `return`s before the tail — so every board key press set the request and nothing ran it. My test (`moving_the_board_highlight_asks_the_card_list_to_follow_it`) asserted the request was *set*, which is a producer with no consumer: I disclosed "unit-tested, not visually verified" when the honest phrase was "not tested". **Fix:** `update` is now a wrapper around `update_message` that consumes the request after *any* return path. **New test:** `a_board_down_key_through_update_returns_the_scroll_task` goes through `update` and asserts on the `Task` it returns (`units() > 0` over several cards, `0` for one card, and no request left behind). **Ablated with `ablate.sh`** (drop the request in the wrapper — the shipped bug): that test fails, alone.

**Live, with content not geometry** (`evidence/04-…`, `05-…`): ten fabricated recent projects under `/dev/shm` via `XDG_STATE_HOME`, so no path under `$HOME` is on screen; the window at its natural size; seven `Down` presses through the focus-verified helper. Before: the top of the list. After: the list has scrolled and `kappa` is highlighted, in view.

**B1.** A project that is not open shows the trust badge, one *"Not open: counts appear once it is opened"* badge and any count it does know — not five `…: unknown` pills, an unavailable branch or a "Calm" it has no evidence for. `a_project_that_is_not_open_says_so_once_…`, `a_not_open_card_keeps_the_counts_it_knows`; ablated (never show the badge) → both fail.

**B2.** The blocked-automation names show on the highlighted card only; the count stays on every card, so what is blocked is one key press away and never absent. `the_blocked_automation_names_show_only_on_the_highlighted_card`; ablated (`|| true`) → fails, alone.

**A rename this needed:** four existing "exactly N production call sites" tests named `update` as the enclosing function; the body is now `update_message`, so they say that. Nothing about what they pin changed.

## 2026-11-17 (review 690) -- a known, named, un-guarded property: per-pull cost inside the bounded drain

`a_hundred_thousand_entry_directory_is_a_bounded_scan_not_a_walk_of_all_of_it` now compares two
directories that **both** exceed `OMITTED_COUNT_LIMIT` (20 000 against 100 000) rather than a
sub-limit directory against a supra-limit one. Reviews 688-690 have the full derivation; the short
version is that `.take(policy.omitted_count_limit + 1)` makes a scan's cost constant above the
limit, so the old 300-vs-100 000 comparison measured a fixed ~33x work difference whose ratio
depends on each machine's own balance of per-scan overhead against per-pull cost -- 1.9 on a 4-core
CI runner against 4.94 on a 32-core dev machine -- and it reddened on the thin-headroom machine
rather than on a regression.

**Confirmed in absolute terms, which the old form could never show**: 1.939249 ms at 20 000 entries
against 1.974247 ms at 100 000 (ratio 1.018, independent measurement, review 690) -- a 5x
difference in directory size costing 1.8% more time. That is this document's own
"worst case ~2.5 ms, independent of how large the real directory is" claim for
`OMITTED_COUNT_LIMIT = 10_000`, measured directly rather than inferred.

**What the new comparison cannot see, recorded so it is not later mistaken for a defect nobody
noticed.** A **per-pull cost** regression inside the bounded drain -- stat-ing drained entries, say,
instead of pulling them bare, the property `ExplorerDirectoryScan::omitted_entries`' own doc states
("counted by draining the rest of the directory *without* stat-ing anything") -- inflates both
supra-limit sides together, so the ratio stays ≈ 1.0. The old comparison could see it, because its
denominator did 300 pulls against the other side's 10 001; that sensitivity was incidental to a
comparison that was structurally machine-dependent, not a property it was designed for.

Where the class stands today, checked rather than assumed:

- `hostile_tests.rs` has exactly two `elapsed()` sites -- this test's own best-of-5 and the
  depth-timing test, a different property.
- This document's "~2.5 ms, measured at ~0.24 µs/entry" is recorded evidence, not an assertion.
- What actually caught review 612 was the **measurement crate's D8 composition check**
  (`scan+model+set_tree`, 1.02 ms → 24.44 ms against the 16.67 ms frame) -- not in the workspace,
  not run by CI, and it let that regression live from PR-052-B until a `Cargo.lock` refresh
  surfaced it a month later. The class therefore has a guard, and that guard is manual and
  demonstrably lagging.

**Not to be closed with a timing bound on the supra-limit side**: an absolute number reintroduces
exactly the machine dependence the new comparison removed (the 4-core runner's supra-limit side
will be several times the 32-core machine's 1.94 ms, so any bound is either loose there or tight
here -- the 1 ms floor's own failure mode). The only honest automated instrument is a counting seam
on the scanner, rejected as too much new surface on security-relevant code, and nothing about that
trade has changed. The change that would close it properly is bringing the D8 composition check
into the automated gate -- a process decision, not a test change, and the owner's to make.

## 2026-11-17 (review 691) -- the breadth ratio's three measured baselines, macOS's own size-dependent term, and the ablation the bound move still owes

The dedicated `--nocapture` CI steps landed (`gate` and `macos-measurement`), so this measurement
is now a standing time series rather than a local capture. All three baselines, verified from the
job logs rather than quoted:

| machine | 20 000 entries | 100 000 entries | ratio |
|---|---|---|---|
| dev Linux, 32 cores | 1.939249 ms | 1.974247 ms | **1.018** |
| CI Linux, 4 cores (`gate`) | 7.966868 ms | 7.972688 ms | **1.001** |
| macOS runner (`macos-measurement`) | 12.193542 ms | 15.646125 ms | **1.283** |

**The disposition is vindicated on Linux**: the baseline spread between a 4-core runner and a
32-core dev machine collapsed from 4.9x under the old sub-limit-vs-supra-limit comparison (1.9
against 4.94) to **1.7%**.

**macOS carries a size-dependent term even in the bounded case, and it is measured, not assumed.**
Fitting the two macOS points to `cost = F + (OMITTED_COUNT_LIMIT + 1)·pull + size·s` gives
`s ≈ 0.043 µs` per directory entry *present, whether or not it is drained*, with `F ≈ 11.3 ms`. So
on APFS a capped scan's cost is not constant above `OMITTED_COUNT_LIMIT` the way it is on ext4: 5x
the entries costs 28% more time though both sides pull exactly `OMITTED_COUNT_LIMIT + 1` of them.
The "both sides do byte-identical work, so the ratio is 1.0" argument holds at the **syscall-count**
level everywhere and at the **cost** level only on Linux. 1.283 against a 2.0 bound is fine; macOS
is simply the margin to watch.

**What is still owed: the signal side has never been measured.** RFC-074's standing requirement --
*"Each bound move re-ablates... Lowering a bound makes the assertion stricter; it does not re-prove
the measurement underneath still discriminates"* -- applies to this bound's 8.0 → 2.0 move. The
noise side now has three machines and an under-load run (0.77-0.99 on the dev machine while a
second full workspace test ran, confirming contention cancels between equal work). The signal side
is an extrapolation from review 612's `~0.24 µs/entry`, taken in 2026-10 on unidentified hardware.

Separating fixed from per-pull cost using each machine's own sub-limit point (the old form's
300-entry side) puts the real signal below the figure the comment claims:

| machine | 300 entries | 10 001 pulls | derived µs/pull | a 612-class regression would read |
|---|---|---|---|---|
| dev Linux | 1.13 ms | 1.94 ms | 0.084 | ~3.4x |
| CI Linux | 5.07 ms | 7.97 ms | 0.299 | ~3.2x |
| macOS | 6.15 ms | 20.11 ms (loaded) | ~1.44 (inflated) | ~4.3x, least reliable |

So the test comment's *"would read ~5x against this bound, a 2.5x margin"* holds only on the
fastest machine's arithmetic; the real signal margin is ~1.6-2.2x. Both margins then sit in the
same 1.56-2.2x band, which is why **2.0 is well chosen and should not be raised** -- raising it
eats the thinner side.

**The ablation is free**: `FileExplorerScanPolicy::omitted_count_limit` is a `pub` field, so a
test-only `FileExplorerScanPolicy { omitted_count_limit: 1_000_000, ..linux_mvp() }` reproduces
review 612's exact regression with no production change and nothing to restore. Run the ratio under
it on both runners, confirm it crosses 2.0, and record the per-platform number -- which turns every
figure in the table above into a measurement. A permanent test asserting the crossing is better
still, since it keeps discriminating after the next change to the scanner.

## 2026-11-18 (review 692) -- the signal side measured, review 691's extrapolations corrected, and the bound re-derived to 2.5

`an_unbounded_policy_makes_the_breadth_ratio_cross_its_bound` landed (a test-only
`FileExplorerScanPolicy { omitted_count_limit: 1_000_000, ..linux_mvp() }`, no production change),
so the 8.0 → 2.0 move now satisfies RFC-074's own re-ablation rule. **The signal side is measured**,
and it supersedes review 691's table in the section above:

| machine | review 691 estimate | measured | estimate error |
|---|---|---|---|
| CI Linux (`gate`) | ~3.2x | **3.774** (8.512283 ms → 32.125399 ms) | low by 1.18x |
| dev Linux, 32 cores | ~3.4x | **6.664** (4.4463 ms → 29.62811 ms), range 3.96-9.47 | low by ~2x |
| macOS runner | ~4.3x, flagged least reliable | **13.664** (14.327541 ms → 195.7785 ms) | **low by 3.2x** |

The estimates were derived by separating fixed from per-pull cost using each machine's own
sub-limit point; the macOS row used a loaded run's numbers and was the furthest off, as flagged.
Use the measured column. macOS's per-entry drain cost is ~2.27 µs (from the ablation's own two
points), roughly 7.6x `gate`'s ~0.299 µs.

**The bounded comparison's macOS baseline moved in the same run, and that is the number that
matters.** From run 44108575432, the bounded step:

| | run `39b5678` | run `055b022` | change |
|---|---|---|---|
| `gate` | 1.001 | 1.010 | stable |
| `macos-measurement` | 1.283 | **1.739** | ×1.36 |

macOS's absolute sides: 12.193542 → 18.089125 ms on the 20 000 side (×1.48) against 15.646125 →
31.451709 ms on the 100 000 side (×2.01). **Asymmetric inflation inside the bounded comparison** --
which the disposition's own argument says cannot happen, since both sides pull
`OMITTED_COUNT_LIMIT + 1` entries, and which happens on macOS for the reason review 691 measured:
APFS charges ~0.043 µs per directory entry *present*, so the two sides are not cost-identical there.

**Re-derived bound: 2.5.** The window between worst measured noise and worst measured signal is
[1.739, 3.774] -- macOS loaded at one end, `gate`'s ablation at the other. Geometric mean 2.56;
2.5 gives 1.44x noise margin and 1.51x signal margin. The current 2.0 gives **1.15x noise against
1.89x signal**: lopsided, with the thin side the one that produces a false red, on the platform
this register has had to disposition against load more than any other.

**This reverses review 691's own retraction, on data rather than preference.** That review recorded
"2.5 would cost more margin than it buys" as an instinct not to act on, and that was right against
what existed then: an extrapolated signal (~3.2x) and a single clean macOS noise point (1.283).
Both are now measured and both move the optimum up. Re-deriving a bound when the measurements
arrive is what the pre-registration was for -- and doing it before a red, with the derivation
written down, is the difference between calibration and conceding to a flake. It is not review
688's forbidden move: raising 8.0 was wrong because *that* baseline was machine-dependent and its
red was information about the instrument.

**Next decision point, named rather than discovered.** If macOS's bounded ratio climbs past roughly
1.9 -- inside 1.3x of even the raised bound -- the honest conclusion is that a ratio between two
supra-limit directories does not work on APFS, and macOS needs a different assertion rather than a
third bound. Two points do not establish a trend; the standing `--nocapture` step will.

**The per-pull class is still un-guarded, by either test.** An unbounded policy makes the two sides
do *unequal* work; a per-pull-cost regression inside the bounded drain leaves both sides pulling
`OMITTED_COUNT_LIMIT + 1` entries and makes each pull dearer -- *equal* work, more costly, ratio
unchanged. Different regressions. The ablation sees only the size-scaling one (under a per-pull
regression its own ratio rises toward 5x and it passes more strongly, having no upper bound), so
the section above stands unchanged: the class has one guard, the measurement crate's D8 composition
check, and that guard is manual.

## 2026-11-18 (review 693, same day, second one) -- macOS's third point: 1.739 was a degraded run, and the size-dependent term is itself load-sensitive

The 2.5 bound landed (both the bounded assertion and the ablation's own threshold, so the ablation
keeps testing against the real bound). All four of that run's measurements verified from the job
logs: `gate` bounded 1.006 / ablation 3.869; `macos-measurement` bounded 1.303 / ablation 9.291.

**Three macOS points now exist, and the third settles the trend question review 692 left open:**

| run | 20 000 side | 100 000 side | ratio | fitted `s` (µs per entry present) |
|---|---|---|---|---|
| `39b5678` | 12.193542 ms | 15.646125 ms | 1.283 | 0.0432 |
| `055b022` | 18.089125 ms | 31.451709 ms | **1.739** | **0.167** |
| `66af604` | 10.160083 ms | 13.238375 ms | 1.303 | 0.0385 |

The ~1.9 watch threshold is **not** being approached: two of three sit at 1.28-1.30, and the
outlier was slow on *both* sides. So the ratio form has not been shown to fail on APFS, and no
macOS-specific assertion is needed on this evidence.

**The mechanism is sharper than "a degraded run," and it is the part worth keeping.** APFS's
size-dependent term is itself load-sensitive -- ~0.039-0.043 µs per directory entry present when
clean against 0.167 under degradation, a 4x move -- which is exactly why macOS's ratio wanders
while `gate`'s sits at 1.001-1.010 across the same runs. The term is not a platform constant; it is
a platform constant *times* whatever pressure the volume is under.

**And that shows the re-derivation bought protection, not margin for its own sake.** With
`ratio = 1 + s·80000/small`, pairing the fastest observed small side (10.16 ms, run 3) with the
worst observed `s` (0.167, run 2) -- two halves each already measured, never yet together -- gives
**2.31**: a red against the old 2.0 bound, comfortably inside 2.5. Reaching 2.5 from run 2's own
small side would need `s ≈ 0.34 µs`, another 2x beyond anything observed.

Note for whoever next touches the bounded test's comment: it still describes the term as "fitted
from its two points." It is three, and the spread between them (the load-sensitivity above) carries
more information than the single figure quoted there.
