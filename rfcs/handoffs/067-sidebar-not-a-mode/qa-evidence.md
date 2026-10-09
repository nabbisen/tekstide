# RFC-067 QA evidence

## PR-067-A — the sidebar persists

### D1 — the explorer renders and is reachable in both modes

`sidebar_view` no longer takes a `mode` parameter at all: it renders the explorer whenever a
project is active, regardless of mode, and falls back to an empty element only for the
(structurally unreachable while routed to `ActiveProjectWorkspace`) no-active-project case.

**A second, non-obvious gate had to be found and removed too.** `handle_explorer_key` itself
returned early unless `project.mode() == ProjectMode::Content` — so even once the tree was
visually present in Terminal mode, `Up`/`Down`/`Enter` would have done nothing there. Found by
reading the function this RFC's own task breakdown pointed at, not by the handoff pack naming it.
`ensure_explorer_scanned` had the identical gate, which would have left a project opened straight
into Terminal mode showing the always-visible tree stuck on "Loading…" forever — found the same
way. Both removed.

**Ablated**: reverting `handle_explorer_key`'s own guard to the old `if project.mode() !=
ProjectMode::Content { return; }` fails `activating_a_file_in_the_explorer_while_in_terminal_mode_
switches_to_content_mode` with exactly the message naming the regression:
`"D1: the explorer must be reachable, not just visible, in terminal mode"`.

### D1 — the placeholder is deleted, not reworded

`sidebar-placeholder-title` removed from `en.ftl`; `sidebar_label` (its only caller) removed
entirely, since its whole job was composing that one string with a focus marker. The i18n
enforcement suite (`report_catalog_keys_unused_by_any_render_call`,
`every_source_locale_key_resolves_in_every_shipped_locale`) confirms nothing still references it.
Two tests that depended on `sidebar_label` updated: `sidebar_label_reflects_focus` deleted (its
own premise no longer exists), `the_sidebar_and_content_placeholders_name_no_rfc_and_say_
something_true` narrowed to `the_content_mode_placeholder_names_no_rfc_and_says_something_true`
(only the content-mode placeholder, which this RFC does not touch, remains to test).

### D7 — activating a file in terminal mode switches to Content mode and shows it

**A real, pre-existing bug found while implementing this, not invented by it.**
`ProjectSession::open_text_document` — the one function every document-open path in this crate
calls through — unconditionally called `self.set_mode(ProjectMode::Content)`. Before this RFC that
was harmless: the explorer was the only reachable caller, so "every open forces Content" and "the
one caller that should switch mode does" were the same fact. RFC-067 made the sidebar (and this
function) reachable from a second caller — RFC-027's recovery offer, which also opens documents
through this same path — so the two stopped being the same decision, and the unconditional switch
became D8's exact violation (confirmed live: my first version of the D8 test below failed with
`left: Some(Content), right: Some(TerminalImmersion)` before this was found).

**Fixed at the root, not patched at the symptom.** `set_mode` removed from `open_text_document`
entirely (see its own new doc comment). A new, symmetric `AppState::open_active_project_content_
workspace()` (mirrors the already-existing `open_active_project_terminal_workspace` exactly) is
called explicitly, once, from the one place D7 names: `handle_explorer_key`'s own `Action::Open`
arm, immediately after a successful `open_active_project_text_document`.

A pre-existing core-level test, `opening_text_document_from_terminal_mode_forces_content_mode`,
had encoded the old universal behaviour as its own name and assertion. **Inverted, not deleted**:
renamed to `opening_a_text_document_through_the_shared_path_does_not_force_content_mode`, same
scenario, assertion flipped to prove the shared path now holds the mode.

**Ablated**: removing the explicit `open_active_project_content_workspace()` call from the
explorer's own arm fails `activating_a_file_in_the_explorer_while_in_terminal_mode_switches_to_
content_mode` with `left: TerminalImmersion, right: Content` — the exact defect this call exists
to prevent.

Live-captured: `evidence/pr-067-a/activating-a-file-switches-to-content-mode.png` — a real
`main.rs` activated from the explorer while a real terminal was running in Terminal mode; the main
area shows the file's real content, and `Switch to Terminal` (not `Switch to Content`) confirms the
mode actually moved.

### D8 — a document opened by any path that is not the user's own activation does not change the mode

Driven through RFC-027's recovery offer specifically, per the task breakdown's own instruction
("it is the only one of the three that both opens a document and is reachable while a terminal is
on screen"): `accepting_a_recovery_offer_in_terminal_mode_does_not_switch_the_mode`
(`shell/tests.rs`) sets a project to `TerminalImmersion` *before* the offer modal is ever
constructed, writes a real recovery record, accepts it through the real `Message::ModalActivate`
path, and asserts both that the buffer became visible (the offer still works) and that the mode
never moved.

The third path D8 names, the background watch notice (`record_project_watch_notice`), was checked
by direct inspection rather than given its own test: it only ever marks documents already in the
open set as touched, and never calls `open_text_document`/`open_active_project_text_document` at
all, so it cannot reach the removed code path by construction — consistent with the task
breakdown's own account of it ("opens nothing new").

**Ablated, both layers** (the core-level test and the GUI-level test independently prove the same
property at two different seams): reverting `open_text_document`'s own removal of `set_mode` fails
both `accepting_a_recovery_offer_in_terminal_mode_does_not_switch_the_mode` (`tekstide`) and
`opening_a_text_document_through_the_shared_path_does_not_force_content_mode` (`tekstide-core`),
each with the exact `Content`-instead-of-`TerminalImmersion` symptom.

### D3 — no new focus zone, `Tab` cycle unchanged

`FocusZone` (`input.rs`) is untouched by this slice — no variant added, `next()`/`previous()`
unchanged. Every existing focus-cycling test (`tab_cycles_shell_focus_with_a_real_terminal_focused_
and_writes_nothing` and others) passes unchanged, which is the proof: a zone added or reordered
would have broken them.

### D9 — the six-terminal bound, session bar and two visible slots are untouched

No terminal-workspace code was touched by this slice. The live capture
(`evidence/pr-067-a/terminal-mode-running-terminal-and-tree.png`) shows a real running terminal,
its session bar entry (`Terminal 1 (Primary) — Running`), and the status bar's own `1 running`
alongside the now-persistent file tree — nothing about the terminal side changed shape.

### Live capture

`cargo build --release -p tekstide`, isolated `XDG_STATE_HOME` under `/dev/shm`, a throwaway
project with two real files (`main.rs`, `src/lib.rs`).

- `evidence/pr-067-a/terminal-mode-with-the-file-tree.png` -- **the required D2 proof, the direct
  replacement for the deleted placeholder**: Terminal mode, no terminal running yet, and the real
  file tree (`src`, `main.rs`) beside the "Nothing is running... Start a terminal" placeholder --
  not "Files are listed here in Content mode."
- `evidence/pr-067-a/terminal-mode-running-terminal-and-tree.png` -- a real terminal launched and
  running (`tekstide$` prompt, session bar, `1 running` in the status bar), the file tree still
  beside it.
- `evidence/pr-067-a/activating-a-file-switches-to-content-mode.png` -- D7's own proof, described
  above.

Processes terminated cleanly with `SIGTERM` after capture; throwaway state and project directory
removed from `/dev/shm` afterward, keeping only the three images copied into this RFC's own
`evidence/pr-067-a/` directory.

### Book, swept

The checklist's own Whole-RFC item names `what-works-today.md` as describing the sidebar as
mode-dependent. **Checked directly: it does not** -- its own "Projects, files, and editing"
section never claims the explorer is Content-mode-only. The real stale claim was in
`keyboard-reference.md`: *"With `Tab` focused on the sidebar in Content mode, `Up`/`Down` move the
explorer highlight..."* Fixed to say the tree is in both modes now, and that opening a file from
Terminal mode switches to Content mode. `configuration.md`'s two "file tree" mentions are about
font family, unrelated to mode. `getting-started.md`'s `Ctrl+Alt+M` keybinding row is still
accurate (D9: the toggle itself is untouched).

### Gate, PR-067-A

- `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`: clean.
- `i18n::enforcement` (23/23, one catalog key removed, confirmed unreferenced), `rfc_docs_
  invariants` (17/17, this slice touches no RFC document): clean.
- `cargo test --doc --workspace`: clean.
- `cargo test -p tekstide-core --lib`: 1118/1118. `cargo test -p tekstide --bin tekstide`:
  758/758.
- **Three consecutive full-workspace runs, `--no-fail-fast`, fresh short `TMPDIR` each run**
  (`/dev/shm/g067a{1,2,3}`): `758 + 17 + 1118`, 0 failed, 0 fixture entries left each time, clean
  on the first attempt.

## PR-067-B — measure the switch

### D4 — the render cost of a mode switch, measured

**What is measured, and why this shape.** Switching the mode itself is a field write
(`ProjectSession::set_mode`); what a user actually waits on is the subsequent `Element` tree
rebuild for whichever mode they land in. `mode_switch_render_cost_measurement`
(`shell/tests.rs`) times `active_project_workspace_view(state)` directly -- this crate's own
established "view-build cost" sense (`ARCHITECTURE.md`: wall-clock time for `view` to construct
its `Element` tree; compositor/GPU present excluded), the same convention
`change_review_content_view_build_cost_by_line_count_measurement` already uses for a different
surface. **Not measured: layout, shaping or paint** -- the same disclosed lower bound every prior
measurement in this crate carries.

**Fixture**: one project, used for both conditions, so nothing but the mode differs between
measurements -- a representative explorer tree (`src/lib.rs`, `README.md`), a real spawned
terminal with real output already flowing (`printf` into a real `/bin/sh`, polled for), and a
real open document (300 representative lines, `editor_baseline::fixture_prefix`). The terminal
keeps running and the document keeps its text regardless of which mode is active, matching D4's
own premise: this fixture exists to measure what *drawing* the result costs, not to construct two
different projects.

**Paired, control inside the same run (D4's own words).** Content and Terminal mode's own
view-build cost measured back to back, alternating which goes first across five rounds -- the
same shape `editor_typing_latency_under_a_recovery_persist_tick` already uses, so an ambient load
spike lands on both conditions almost equally. There is no separate "idle" control the way that
measurement has: the two conditions here are each other's control, and "the cost of switching
into a mode" is simply that mode's own absolute view-build cost, since the switch itself (the
field write) has none worth measuring separately.

**Required at review 509: the first version's figures sat on the timer's own resolution floor.**
One `Instant::elapsed()` around a single view-build call, for a quantity of 4-16 us -- every
figure was an exact multiple of 4 us, and each median equalled its own minimum, the RFC-027
reviews 485/486 defect from the other side: there the floor showed up in the spread's sign
(straddling zero), here it showed up in the medians themselves without announcing itself.
**Fixed**: the build now repeats 200 times inside the timed region (`BUILDS_PER_SAMPLE`), divided
back down as `f64` nanoseconds (never through `Duration::as_micros()`'s own integer truncation,
which would have reintroduced the identical quantization one step later). The total timed region
is now comfortably above the timer's own resolution, and the per-build figure below is real, not
a tick count.

**Measured** (`CARGO_PROFILE_RELEASE_DEBUG_ASSERTIONS=true cargo test --release -p tekstide
mode_switch_render_cost_measurement -- --ignored --nocapture`, the same invocation shape
`editor_typing_latency_baseline_100_000_lines` established and for the identical reason: the
suite's headless view-build tests use iced's `()` renderer, which exists only with debug
assertions on). Two independent runs, both reproduced after the fix, numbers varying run to run
(ordinary machine conditions) while staying the same order of magnitude and no longer quantized:

| Condition | Median (5 rounds), run 1 | Spread, run 1 | Median, run 2 | Spread, run 2 |
| --- | --- | --- | --- | --- |
| Content mode | 18.0 us | 18.0 .. 19.5 us | 23.2 us | 22.8 .. 24.7 us |
| Terminal mode | 3.9 us | 3.9 .. 4.0 us | 5.0 us | 5.0 .. 5.1 us |

**Worst of the two across both runs, 0.023 ms, against this project's own existing latency
criterion used as the nearest order-of-magnitude yardstick, not the threshold this operation is
actually held to** (`NFR-PERF-003`: typing latency in a 100k-line file, p95 <= 16 ms, p99 <= 33
ms; review 509's own smaller note, taken) -- **the margin is still roughly three orders of
magnitude**, unchanged by the fix above: the conclusion never depended on the exact figure, only
on whether it was real, which it now is.

No rate was derived by dividing one condition's figure by another condition's count (the RFC-027
correction this checklist's own item names) -- both figures reported are each condition's own
absolute cost, nothing is divided by a count from the other condition.

**Not ablated: this test asserts nothing.** The same shape `change_review_content_view_build_
cost_by_line_count_measurement`'s own "curve" section already has -- a diagnostic report, printed
for a reader to read, not a pass/fail gate. There is no guard to remove and watch fail.

### A new cost PR-067-A introduced, surfaced here per review 508

**Not a render cost, and not this measurement's own subject, but the right place to say so
rather than let it pass unremarked.** PR-067-A removed `ensure_explorer_scanned`'s own
Content-mode guard (required, D1 -- a tree visible in Terminal mode and stuck on "Loading…"
forever would have been worse than no tree). One consequence: a project opened straight into
Terminal mode now also requests an explorer scan at open, where before it scanned only on the
first switch to Content, and for a terminal-only session, never at all.

**Checked, not assumed, to be bounded and asynchronous.** `request_explorer_root_scan_if_needed`
(`session.rs:1573`) is a single enum match plus a flag write -- it marks the scan pending and
returns; **it does not scan anything itself.** The real directory read runs on a worker thread
(`explorer_scan_subscription`), never the render thread (pinned by this crate's own
`the_shell_never_scans_a_directory_on_the_render_thread`), and that scan's own cost is already
measured and bounded elsewhere in this project, not newly introduced by this slice:
`a_hundred_thousand_entry_directory_is_a_bounded_scan_not_a_walk_of_all_of_it` and
`asking_git_adds_a_few_milliseconds_to_a_hundred_thousand_entry_scan` (`tekstide-core`) already
prove the scan itself is capped and fast even at an adversarial size. What changed is *when* that
already-bounded, already-asynchronous work gets requested, not its own cost or where it runs.

### Gate, PR-067-B

- `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`: clean.
- `i18n::enforcement` (23/23), `rfc_docs_invariants` (17/17, this slice touches no RFC document):
  clean.
- `cargo test --doc --workspace`: clean.
- **Three consecutive full-workspace runs, `--no-fail-fast`, fresh short `TMPDIR` each run**
  (`/dev/shm/g067b{1,2,3}`): `758 + 17 + 1118`, 0 failed, 0 fixture entries left each time, clean
  on the first attempt. 8 ignored in `tekstide` (one more than PR-067-A's own count: the new
  measurement test); confirmed it still runs correctly with `-- --ignored`.

## PR-067-C — the decision

### D5 — read against the owner's own framing, and the decision stated

**The number.** PR-067-B's own measurement, re-run clean after review 509's timer-resolution fix:
Content mode's view-build cost on the order of 20 us, Terminal mode's under 5 us, both roughly
three orders of magnitude under this project's own existing latency criterion for "a user would
notice" (16 ms).

**Read against *"at a time or a near real-time"*** (the Summary's own question, quoted in full in
the RFC): a switch costing tens of microseconds to redraw is near-real-time in every sense that
framing asks about. The remaining gap the RFC's own Summary named explicitly — *watching* a
terminal while editing, as distinct from operating one — is real and this slice does not close
it, but it is narrower than "three surfaces visible at once" was, and nothing measured says it
needs closing by this RFC.

**Decision: build nothing.** Recorded in the RFC's own document (`rfcs/done/067-the-sidebar-
is-not-a-mode.md`, new `## D5 answered, PR-067-C` section) and in `CHANGELOG.md`'s own `0.33.0`
entry, not only here — per the checklist's own explicit requirement that the number which made
further work unnecessary be recorded in the changelog, not left in the evidence alone.

### No code changed in this slice

Nothing to gate beyond what PR-067-A/B already gated clean. This slice is the decision itself,
read from PR-067-B's own measurement — the RFC's own words, *"a third slice that builds nothing,
and records the number that made it unnecessary, is a success,"* taken as written.

### Gate, Whole-RFC

- `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`: clean.
- `rfc_docs_invariants` (17/17, including `every_delivery_plan_row_agrees_with_its_rfc_folder` once
  the register row was updated to match the RFC's move to `rfcs/done/`): clean.
- `cargo test --doc --workspace`: clean.
- **Three consecutive full-workspace runs, `--no-fail-fast`, fresh short `TMPDIR` each run**: all
  three clean on the first attempt: `758 + 17 + 1118`, 0 failed, 0 fixture entries left each time.

### Candidate cut

`[workspace.package] version` and the `tekstide-core` pin both bumped `0.32.0 -> 0.33.0`;
`cargo check --workspace --all-targets` confirms the regenerated `Cargo.lock` still builds. RFC-067
moved `rfcs/accepted/` -> `rfcs/done/` with a new `## Closed (2026-10-09)` section; the handoff
pack's own frontmatter (`status`, `rfc_file`) updated to match. `rfcs/README.md`'s Accepted-table
row removed, Handoffs-table row and a new Implemented-table row added. `rfcs/delivery-plan.md`'s
own register row updated from "Accepted" to "Implemented and closed... candidate, not yet
published," the same shape RFC-066's own row set one entry earlier —
`every_delivery_plan_row_agrees_with_its_rfc_folder` passes against the new row. `CHANGELOG.md`'s
status line promoted from "in progress" to "candidate, not yet published." Gate reproduces clean
on the first attempt: `758 + 17 + 1118`, 0 failed, 0 fixture entries left. Pushed.
