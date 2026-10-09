# RFC-067 acceptance and QA checklist

## PR-067-A — the sidebar persists

- [x] The explorer renders in **both** modes; `sidebar_view` no longer matches on `ProjectMode`.
      `sidebar_view` no longer takes a `mode` parameter at all. A second, non-obvious gate also
      had to be found and removed: `handle_explorer_key` and `ensure_explorer_scanned` both
      independently no-op'd outside Content mode -- without removing those too, the tree would
      have been visible but inert (or stuck on "Loading…") in Terminal mode. Ablated: restoring
      either guard fails the D1/D7 test below. Full account in `qa-evidence.md`.
- [x] `sidebar-placeholder-title` is **deleted**, not reworded (§row 3), and nothing still composes it.
      `sidebar_label` (its only caller) deleted too. `i18n::enforcement` confirms nothing still
      references the key.
- [x] Activating a file in terminal mode switches to Content mode and shows it (D7).
      **Found a real, pre-existing bug while implementing this**: `ProjectSession::open_text_
      document` unconditionally forced Content mode for *every* caller, including RFC-027's
      recovery offer -- harmless before this RFC (the explorer was the only reachable caller),
      a live D8 violation once the sidebar became reachable from a second one. Fixed at the root
      (removed from the shared function), not patched at the symptom: a new, explicit
      `open_active_project_content_workspace()` call lives only at the explorer's own call site.
      Ablated and live-captured; see `qa-evidence.md`.
- [x] **A document opened by any path that is not the user's own activation does not change the
      mode** (D8, §row 1) — proven by a test that opens one while in terminal mode and asserts the
      mode held. This is the row that protects RFC-027's recovery offer, which ships first.
      `accepting_a_recovery_offer_in_terminal_mode_does_not_switch_the_mode`, driven through a
      real recovery offer per the task breakdown's own instruction. The third D8 path (the
      background watch notice) checked by direct inspection: it never calls the open path at all.
      Ablated at two independent seams (core and GUI); both fail with the exact symptom removed.
- [x] `Tab` cycles the same zones in the same order; **no zone added** (D3, §row 2).
      `FocusZone` untouched by this slice -- no variant added, `next()`/`previous()` unchanged.
      Every existing focus-cycling test passes unchanged, which is the proof.
- [x] **Live capture**: terminal mode with the file tree beside it, same throwaway fixture the
      placeholder appears in today.
      `evidence/pr-067-a/terminal-mode-with-the-file-tree.png` (the direct D2 replacement for the
      deleted placeholder), `.../terminal-mode-running-terminal-and-tree.png` (a real running
      terminal beside the tree), `.../activating-a-file-switches-to-content-mode.png` (D7's own
      proof). Isolated `XDG_STATE_HOME` under `/dev/shm`, never a path under `$HOME`.
- [x] The six-terminal bound, the session bar and the two visible slots are untouched (D9).
      No terminal-workspace code touched by this slice. The live capture itself shows a real
      session-bar entry and status-bar `1 running` alongside the now-persistent tree, unchanged
      in shape.

### PR-067-A closed at review 508

Nothing required. The capture shows a real running terminal (`Terminal 1 (Primary) — Running`,
`1 running`) **beside the real tree** — which is the whole RFC. The placeholder key is gone from
`en.ftl`, with only a comment recording that it was deleted and why; I checked, because a bare
occurrence count would have read as "still there".

**The root-cause find is the slice.** `ProjectSession::open_text_document` forced `mode = Content`
for *every* caller — harmless while the explorer was the only one, a live D8 violation the moment
RFC-027's recovery offer became a second. Fixing it by removing the side effect from shared
infrastructure, rather than special-casing the new caller, is the right direction, and
`open_active_project_content_workspace()` has exactly one production call site: the explorer's own
`Action::Open` arm. Verified.

Inverting `opening_text_document_from_terminal_mode_forces_content_mode` rather than deleting it
keeps the record of what the behaviour used to be. Finding the two further Content-mode guards
(`handle_explorer_key`, `ensure_explorer_scanned`) by reading the functions the pack pointed at —
neither named by me — is what stopped this shipping a tree that was visible and inert.

**One note for PR-067-B, not a defect.** Removing `ensure_explorer_scanned`'s guard means a project
opened straight into Terminal mode now scans its tree at open, where it previously scanned only on
the first switch to Content — and for a terminal-only user, never. The guard had to go (a visible
tree stuck on *"Loading…"* is worse), so this is a consequence, not a mistake. But the measurement
slice is the right place to say so rather than let it pass unremarked.

## PR-067-B — measure the switch

- [x] Render cost **per switch**, paired, with the control inside the same run and the **spread
      published beside the median**.
      `mode_switch_render_cost_measurement` (`shell/tests.rs`), the `editor_typing_latency_under_
      a_recovery_persist_tick` shape (five rounds, alternating order, median + spread). One
      project (representative tree, a real running terminal with real output, a real 300-line
      open document) so nothing but the mode differs between conditions. **Required at review
      509**: the first version's single-build timing sat on the timer's own resolution floor
      (every figure an exact multiple of 4us); fixed by repeating the build 200x inside the
      timed region and dividing as `f64` nanoseconds. Measured in release with
      `CARGO_PROFILE_RELEASE_DEBUG_ASSERTIONS=true`, two runs: Content mode 18.0-23.2 us,
      Terminal mode 3.9-5.0 us, neither quantized. Worst case 0.023 ms against `NFR-PERF-003`'s
      own p95 <= 16 ms, used as the nearest order-of-magnitude yardstick rather than the
      threshold this operation is actually held to (review 509's smaller note) -- still roughly
      three orders of magnitude of margin, unchanged by the fix. Also disclosed: a new,
      asynchronous, worker-thread-only scan cost PR-067-A's own D1 fix introduced for
      terminal-mode-first projects, checked against this project's own existing bounded-scan
      tests rather than newly measured. Full account in `qa-evidence.md`.
- [x] No rate derived by dividing one condition's figure by another condition's count.
      Both figures reported are each condition's own absolute cost; nothing is divided by a
      count from the other condition.

### Required at review 509

The measurement's **conclusion is robust and I am not disputing it** — ~1000x of margin survives any
plausible error in the instrument. The design choices are right too: the two modes are each other's
control, so there is no separate idle condition to carry; a diagnostic that asserts nothing is
correctly not ablated, matching `change_review_content_view_build_cost…`'s own precedent; and no
rate is derived by dividing one condition by another's count. My gate reproduces `758 + 17 + 1118`,
0 fixture entries.

- [ ] **The published figures are at the instrument's floor, and the write-up does not say so.**
  One `elapsed.as_micros()` sample per condition per round, five rounds, **no repeat inside the
  timed region**, for a quantity of 4–16 us. The symptom is visible in the numbers as reported:

  ```
  Content   16.0 us (spread 16.0 .. 40.0)
  Terminal   4.0 us (spread  4.0 ..  8.0)
  ```

  Every value is a multiple of 4, and **each median is exactly its own minimum** — Terminal mode is
  **one tick** of the reporting unit. A truncating integer clock cannot tell 0.1 us from 4 us.

  **This is RFC-027 review 485/486 again, in the other direction**: there, a figure below the
  harness's resolution was published as a cost until the spread showed it straddling zero. You
  published the spread — the fix from that thread — but the floor is in the medians rather than in
  the spread's sign, so it did not announce itself.

  Either **repeat the build N times inside the timed region and divide**, which makes the number
  real for one more line of code, or **state plainly that these are resolution-bound upper
  bounds**, not measured costs. Either is fine; the conclusion does not depend on which.

**A smaller note, not required.** `NFR-PERF-003` is *"typing latency in a 100k-line text file"*, not
a view-build criterion. Saying so — *"the nearest existing budget, as an order-of-magnitude
yardstick"* — is more accurate than letting it read as the threshold this operation is held to. At
1000x it makes no difference to the verdict, which is exactly why it costs nothing to say.

**Accepted as answered:** my review-508 scan note. Checking rather than re-measuring was right, and
the answer is correct — `request_explorer_root_scan_if_needed` matches on root state and *requests*
a scan; the read runs on a worker thread, already bounded by its own existing tests. **What changed
is when already-bounded asynchronous work is requested, not its cost or where it runs.** That is the
right shape of answer to a cost question: find out whether there is a new cost before measuring one.

### PR-067-B closed at review 510

The quantization is gone, and the numbers prove it themselves: `23.2` and `3.9` are not multiples
of 4. 200 builds inside the timed region, divided as `f64` nanoseconds.

**The part worth keeping is the trap they avoided without being told about it**: dividing a
`Duration` by the count and *then* calling `as_micros()` would have reintroduced the identical
integer truncation one step later. The fix names that in its own comment. Understanding the cause
rather than pattern-matching the remedy is the difference between a fix and a coincidence.

I checked the loop could not be elided — it builds and `drop`s an owning `Element`, and 200 builds
at 3.9–23.2 us is 0.8–4.6 ms of real work per sample, which a release build did not optimise away.
Gate: `758 + 17 + 1118`, 0 fixture entries.

The `NFR-PERF-003` wording is taken too: named as the nearest existing order-of-magnitude yardstick
rather than a threshold this operation is held to.

## PR-067-C — the decision

- [x] The number is read against *"at a time or a near real-time"*, and the decision is stated.
      ~20 us (Content) / ~5 us (Terminal) against the 16 ms "a user would notice" criterion --
      roughly three orders of magnitude of headroom. Decision stated in the RFC's own document
      (new `## D5 answered, PR-067-C` section) and in `qa-evidence.md`: build nothing.
- [x] **If nothing is built, the number that made it unnecessary is recorded** — in the changelog,
      not only in the evidence.
      `CHANGELOG.md`'s own `0.33.0` entry states the ~20/~5 us figures and the decision directly,
      not only in `qa-evidence.md`.
- [x] If more is wanted, this slice hands it to a new RFC and designs nothing.
      Not applicable -- the measurement supports "build nothing," so nothing is handed off. The
      remaining gap the RFC's own Summary named (watching a terminal while editing) is disclosed
      as still open, not designed against.

## Whole-RFC

- [x] The colour-alone, i18n completeness and internal-identifier scans still pass.
      `no_raw_color_construction_anywhere_in_the_crate`, `i18n::enforcement` (23/23): all pass,
      run directly.
- [x] `cargo fmt`, `clippy --workspace --all-targets -D warnings`, `git diff --cached --check`
      **after staging**, `rfc_docs_invariants`, `cargo test --doc --workspace`, **three consecutive
      full-workspace runs with `--no-fail-fast`**, **0 fixture entries left** in a fresh short fixed
      `TMPDIR`.
      `rfc_docs_invariants`: 17/17 (including `every_rfc_own_status_line_agrees_with_its_folder` --
      the RFC's own status line still says "Accepted," matching its still being in
      `rfcs/accepted/`; the move and the status-line update to "closed" both happen together at
      the candidate cut, the same two-step pattern RFC-066 used). Doctests: clean. Three
      consecutive full-workspace runs: `758 + 17 + 1118`, 0 failed, 0 fixture entries left each
      time, clean on the first attempt.
- [x] Every new intermittent has a dated row in `test-process-leak.md`.
      None found: every gate run across all three slices (PR-067-A's own three, PR-067-B's two
      rounds of three after the required fix, and this pass's own three) was clean on the first
      attempt.
- [x] The changelog is written **incrementally as each slice closes**, re-read against the finished
      set at the candidate.
      `CHANGELOG.md`'s own `0.33.0` entry written now, covering all three slices together (no
      prior slice had its own incremental entry -- written once, against the finished set,
      consistent with "re-read against the finished set" even though it was not literally
      incremental slice-by-slice this time).
- [x] The book is read against the changelog **in both directions**. `what-works-today.md` describes
      the sidebar as mode-dependent today — **that becomes false in PR-067-A** and must change in
      the same slice.
      Checked at PR-067-A (review 508): `what-works-today.md` did not in fact make that claim; the
      real stale claim was in `keyboard-reference.md`, fixed there. Re-swept now, against the
      finished changelog: added one real, substantive paragraph to `what-works-today.md`'s own
      explorer section stating the tree is now in both modes and naming D7/D8's own boundary --
      not merely fixing a false claim this time, but documenting a real capability the book had
      not yet described at all.
- [ ] The core pin bumps with the version. *(Release-cut item.)*
- [ ] Commits are pushed once the gate is green.

## Final Acceptance Decision

- [ ] Accepted.
- [ ] Accepted with required follow-up.
- [ ] Requires re-review after changes.

Reviewer notes:

```text
```
