---
title: "RFC-053 — acceptance and QA checklist"
rfc: "RFC-053"
rfc_file: "../../accepted/053-what-the-window-says-is-true.md"
source_rfc_status: "Accepted 2026-09-24 — M12 remainder"
target_milestone: "M12 remainder"
created: "2026-09-24"
---

# Acceptance and QA checklist

Every box is a property. A box whose plan assigns it elsewhere, or that cannot be satisfied as
written, stays unticked with the contradiction named — the reviewer's error to fix, not the
implementer's to paper over.

## PR-053-A — what the surfaces say

- [ ] **No catalog string contains an internal identifier**, held by a scan (D1/D9). The scan **fails
      on a planted string** and **passes on a doc comment**, and its message says which it covers.
- [ ] Terminal mode's empty state says what the mode is and how to start a terminal — no `RFC-`
      anywhere near it.
- [ ] Change Review's empty state names **agent runs**, and points at where repository changes show
      (D4).
- [ ] A freshly opened project reports **zero** terminals and zero agent runs (D5), and
      `ProjectProviderState` is untouched.
- [ ] The board says **unsaved** for editor buffers; *changed* is left to mean Git (D6).
- [ ] Blocked automations carry their labels, not a bare count (D7).
- [ ] Approval History's caveats follow its content (D8).
- [ ] **Ablation:** restore one old wording; its own test fails alone.
- [ ] **Capture**: one frame where the board, the status bar and Change Review agree about a project
      with two changed files and no agent run. Throwaway state only.

## PR-053-B — the layout

- [ ] At **760×560**, every modal's `Close` control and dismiss hint are reachable; content scrolls.
- [ ] At **520×400**, `content_area_height` subtracts the **rendered** bar height: the content area
      shrinks by exactly the bar's extra height. **Ablation:** restore the constant; that test fails
      alone.
- [ ] A terminal open at the narrow size is sized to the space that exists — captured, not reasoned.
- [ ] No other change rides along in this commit.

## Whole-RFC

- [ ] The colour-alone scan and the i18n completeness scan still pass (§7).
- [ ] `cargo fmt`, `clippy --workspace --all-targets -D warnings`, `git diff --cached --check` after
      staging, `rfc_docs_invariants`, and **three consecutive full-workspace runs with
      `--no-fail-fast`**, output redirected to files.
- [ ] Every new intermittent failure has a dated row in `test-process-leak.md`.
- [ ] Commits are pushed once the gate is green.

## Final Acceptance Decision

- [ ] Accepted.
- [ ] Accepted with required follow-up.
- [ ] Requires re-review after changes.

Reviewer notes:

```text
Pending review.
```
