# RFC-067 acceptance and QA checklist

## PR-067-A — the sidebar persists

- [ ] The explorer renders in **both** modes; `sidebar_view` no longer matches on `ProjectMode`.
- [ ] `sidebar-placeholder-title` is **deleted**, not reworded (§row 3), and nothing still composes it.
- [ ] Activating a file in terminal mode switches to Content mode and shows it (D7).
- [ ] **A document opened by any path that is not the user's own activation does not change the
      mode** (D8, §row 1) — proven by a test that opens one while in terminal mode and asserts the
      mode held. This is the row that protects RFC-027's recovery offer, which ships first.
- [ ] `Tab` cycles the same zones in the same order; **no zone added** (D3, §row 2).
- [ ] **Live capture**: terminal mode with the file tree beside it, same throwaway fixture the
      placeholder appears in today.
- [ ] The six-terminal bound, the session bar and the two visible slots are untouched (D9).

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
