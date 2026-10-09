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

## PR-067-B — measure the switch

- [ ] Render cost **per switch**, paired, with the control inside the same run and the **spread
      published beside the median**.
- [ ] No rate derived by dividing one condition's figure by another condition's count.

## PR-067-C — the decision

- [ ] The number is read against *"at a time or a near real-time"*, and the decision is stated.
- [ ] **If nothing is built, the number that made it unnecessary is recorded** — in the changelog,
      not only in the evidence.
- [ ] If more is wanted, this slice hands it to a new RFC and designs nothing.

## Whole-RFC

- [ ] The colour-alone, i18n completeness and internal-identifier scans still pass.
- [ ] `cargo fmt`, `clippy --workspace --all-targets -D warnings`, `git diff --cached --check`
      **after staging**, `rfc_docs_invariants`, `cargo test --doc --workspace`, **three consecutive
      full-workspace runs with `--no-fail-fast`**, **0 fixture entries left** in a fresh short fixed
      `TMPDIR`.
- [ ] Every new intermittent has a dated row in `test-process-leak.md`.
- [ ] The changelog is written **incrementally as each slice closes**, re-read against the finished
      set at the candidate.
- [ ] The book is read against the changelog **in both directions**. `what-works-today.md` describes
      the sidebar as mode-dependent today — **that becomes false in PR-067-A** and must change in
      the same slice.
- [ ] The core pin bumps with the version. *(Release-cut item.)*
- [ ] Commits are pushed once the gate is green.

## Final Acceptance Decision

- [ ] Accepted.
- [ ] Accepted with required follow-up.
- [ ] Requires re-review after changes.

Reviewer notes:

```text
```
