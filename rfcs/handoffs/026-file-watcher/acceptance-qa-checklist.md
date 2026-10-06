---
title: "RFC-026 — acceptance and QA checklist"
rfc: "RFC-026"
rfc_file: "../../accepted/026-file-watcher-and-multi-document-model.md"
source_rfc_status: "Accepted 2026-09-30 — M13"
target_milestone: "M13"
created: "2026-09-30"
---

# Acceptance and QA checklist

Tick a box when the evidence is in `qa-evidence.md`, not when the code looks right. A box naming a
measurement with no number in the evidence is not ticked.

## PR-026-A — the batching

- [x] A simulated burst of **1,000 events into one directory yields one scan request per window**.
      Both numbers written down: scan requests, **and git subprocesses**.
      *(1 scan request, 2 git subprocesses; five windows: 5 and 10. `qa-evidence.md` § PR-026-A.)*
- [x] Ablated: remove the batching and the count becomes 1,000.
      *(`ablate.sh`: 1,000 scan requests and 2,000 git subprocesses for both streams.)*
- [x] The window is a stated constant (`SCAN_WINDOW = 250 ms`), with its reasoning in the doc
      comment. **The book half moved to C at review 450** — nothing feeds the batcher yet, so no user
      can observe the window, and naming it in the book would describe behaviour the product does not
      have. The implementer declined to tick it and said why; that was right.
      *(The constant is stated in code: `SCAN_WINDOW = 250 ms`. **Not yet in the book**, deliberately:
      it is an internal number until a watcher exists to make it visible. The book names it in the slice
      that makes watching user-visible — B or C. Left unticked until then, rather than documenting
      behaviour the product does not yet have.)*
- [x] Events for different directories do not silently merge.
      *(`events_for_different_directories_do_not_silently_merge`.)*
- [x] No dependency added in this slice, and no watcher wired.

### Required at review 450

- [x] **`record` cannot be handed a non-directory.** It derives `path.parent().unwrap_or(Path::new(""))`,
      so `/` or a bare relative name silently becomes the empty path and then
      `ScanRequest { directory: "" }`. Unreachable from a real watcher, but B is about to feed it real
      events. **Make it unrepresentable** — take the directory B computes anyway, rather than deriving
      one and needing a fallback.
      *(`record(directory: PathBuf, at)` derives nothing. Residual, stated in `qa-evidence.md`: a
      `PathBuf` can still hold `""`; the batcher invents none, and B's caller must not hand it one.
      A stricter newtype needs B's root context.)*
- [x] **Verify `GIT_SUBPROCESSES_PER_SCAN = 2` against a real scan.** Its provenance is recorded
      (measurement 8, review 431) but it describes something nobody has run in A. If the real figure
      differs, **A's evidence numbers are corrected**, not left standing.
      *(Measured through the explorer's own scan seam: **2 on every warm scan, 3 on the first scan in a
      process** (`git --version`, cached per process). Steady state is 2, as the batching counts assume;
      the one-time extra is stated in the constant's doc comment and in `qa-evidence.md`. The
      measurement is `measured_git_subprocesses_per_real_scan`, `#[ignore]`d, run by hand.)*

## PR-026-B — the watcher and its dependency

- [ ] **D4's evaluation recorded before adoption**, including `notify`'s MSRV, its **CC0-1.0** licence
      (unlike every other direct dependency) and its last stable date — and the answer to **what it
      does when the kernel refuses another watch**, read from what it does.
- [ ] `dependency-advisories.md` carries the new crate the way it carries the existing three.
- [ ] Watch scope is `expanded` + the root + open documents' folders. **Counted before and after**
      expanding, collapsing and closing a project — a scope that only grows is a defect.
- [ ] **Budget exhaustion forced in a test**: no crash, today's fallback behaviour, and a sentence on
      screen saying watching stopped and why.
- [ ] Hostile fixture: a symlink leaving the root is not watched; a loop does not recurse.
- [ ] Nothing new on the render thread; the subscription is the shape `explorer_scan_subscription`
      already has.

## PR-026-C — the change arrives

- [ ] **`REQ-FILE-003` captured live**: a file created in an expanded folder appears without the user
      reopening it.
- [ ] **`REQ-FILE-004` measured**: editor keystroke latency under a watched burst, against RFC-057's
      baseline harness.
- [ ] **The book names the batching window** (moved here from A at review 450): by this slice a user
      can observe it, so it describes real behaviour rather than an internal constant.
- [ ] Unsaved edits survive an external change; **no silent reload**; a deleted open file is a state
      the product can say.
- [ ] A reload takes the undo history with the document, and the product does not pretend otherwise.
- [ ] **The split point considered and answered here**, not at the candidate.

## PR-026-D — the open set

- [ ] `open_buffer_count()` and `dirty_file_count()` count the whole open set. **A test fails if
      either counts one when two are open.**
- [ ] A third counter, if one exists by then, is reported rather than absorbed.
- [ ] The open-document cap is stated when reached.
- [ ] The close dialog and save-all count every open document.

## Whole-RFC

- [ ] `REQ-FILE-003`, `REQ-FILE-004` and `NFR-PERF-007` move to met **with evidence a user can see**,
      not with the fields existing.
- [ ] The colour-alone, i18n completeness and internal-identifier scans still pass.
- [ ] `cargo audit` gains no vulnerability.
- [ ] `cargo fmt`, `clippy --workspace --all-targets -D warnings`, `git diff --cached --check` after
      staging, `rfc_docs_invariants`, **three consecutive full-workspace runs with `--no-fail-fast`**
      to files, **0 fixture entries left** in a fresh short `TMPDIR` (a **short fixed literal** —
      `mktemp` plus nested run dirs crosses the `AF_UNIX` 108-byte limit).
- [ ] Every new intermittent failure has a dated row in `test-process-leak.md`.
- [ ] The core pin bumps with the version.
- [ ] Commits are pushed once the gate is green.

### Carried into `0.29.0` at review 451 — not RFC-026's subject

- [ ] **`change_review_content_view_build_cost_by_line_count_measurement` stops measuring the
      machine.** Assert a **ratio against a reference workload timed in the same process** — ten
      times the work must cost under some multiple of the time — rather than an absolute 500 ms.
      Scheduled after a **fourth** episode: `0.26.0`, the `0.28.0` candidate, the architect's `0.28.0`
      verification, and PR-026-A's own gate. The implementer flagged that the count was high enough
      to schedule rather than record again, and was right. Worked examples for both halves:
      `editor_typing_latency_baseline_100_000_lines` (publish the number, assert nothing) and snora
      0.52.0's CI ratio.

## Final Acceptance Decision

- [ ] Accepted.
- [ ] Accepted with required follow-up.
- [ ] Requires re-review after changes.

Reviewer notes:

```text
```
