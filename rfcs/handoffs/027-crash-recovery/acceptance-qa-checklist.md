# RFC-027 acceptance and QA checklist

Tick a box only when the thing it names has been **run**, not reasoned about. Each slice's boxes are
its own; the Whole-RFC section is checked once at the end.

## PR-027-A — the marker

- [x] A real `SIGKILL` of a real process leaves a marker whose pid is not alive; a clean exit leaves
      none. Not a `simulate_crash()` helper (D6, §4 row 15). `qa-evidence.md`:
      `a_real_sigkill_leaves_a_marker_a_later_startup_detects_as_a_crash`,
      `a_clean_exit_leaves_no_marker_behind`.
- [x] **Two concurrent instances do not make each other look crashed** (D12). Start a second while
      the first runs; neither reports a crash. `a_concurrent_sibling_instance_is_not_reported_as_a_crash`.
- [x] The liveness check is production code. `test_support` is not imported by the product.
      `recovery::instance::pid_is_alive`, not `#[cfg(test)]`.
- [x] Pid reuse is disclosed in the write-up, with its direction of failure named. `qa-evidence.md`'s
      own "Pid reuse is disclosed, not fixed" section; direction: not offered, never falsely offered.
- [x] Nothing user-facing, and no buffer content written anywhere.
      `a_first_run_detects_nothing_and_writes_only_its_own_marker` asserts the only file on disk
      is the marker itself.

### Required at review 481

The slice is correct and I verified the whole of it against the real binary, including the thing
you disclosed as unverified. One required item, and it is required because of what PR-027-B does
next, not because of anything it costs today.

- [ ] **Validate the pid parsed from a marker filename before casting it.** `parse::<u32>()` feeds
  `pid as libc::pid_t` unbounded. I planted three markers and ran the real binary: `999999999` was
  detected and removed correctly, but **`0` and `4294967295` survived the scan and always will** —
  `kill(0, 0)` probes the caller's own process group, and `4294967295` casts to `-1`, which probes
  every process the caller may signal. Both answer "alive", so neither marker can ever be cleaned.
  Today that is two stray files and it fails in the safe direction. **It stops being harmless in
  PR-027-B**: a record whose lifecycle is tied to a marker that can never be removed is user content
  that outlives its reason, which is §2 row 7 of the risk document. Reject `0` and anything above
  `i32::MAX` at parse time, so a marker is always either live or removable.

**Verified live against `target/debug/tekstide`, not from the report** — these are not asks, they
are what I ran:

- The marker is created named by the app's own pid; a real `SIGKILL` leaves it behind.
- A restart detects it, names the pid on stderr (*"the previous session (pid 2635125) did not exit
  cleanly"*) and removes it.
- **A real window close through the window manager drops `State` and removes the marker.** Your
  reading of `iced_winit` 0.14.1 was right, and it is no longer a structural claim — see below.
- Two concurrent real instances hold both markers; the second reports nothing and does not touch
  the first's.
- The only files under the state directory are `recent-projects.json`, its `.bak`, and the markers.
  **No buffer content is written anywhere**, checked on the filesystem rather than from the code.
- `test_support` is `#[cfg(test)]` at `lib.rs:31`, so production genuinely cannot reach it; the
  three remaining mentions in product source are doc comments.
- Gate reproduces: `739 + 16 + 1095`, 0 failures, 0 fixture entries left.

## PR-027-B — the record, with its purge

- [ ] A **dirty** document gets a record; a **clean** one does not (D2, §3 row 12). Proved by what
      is on disk, not by a count the code reports about itself.
- [ ] Records are `0600` in a `0700` directory (§2 row 6), checked by reading the mode.
- [ ] No buffer content reaches the audit store (D13, §2 row 5).
- [ ] **Measurement 3 — the cadence**, chosen against `editor_baseline.rs`'s paired harness with the
      control carried inside the same run (D7). The window is justified by the number, not the
      number by the window.
- [ ] **Measurement 4 — per-document cost** at one and at ten dirty documents, with twenty
      extrapolated and **labelled as an extrapolation** (D8, §4 row 14).
- [ ] **Measurement 5 — the bound** refuses a too-large buffer and names the buffer and the limit
      (D9, §1 row 4).
- [ ] **Measurement 6 — the record is gone** after a save, and after a close (D11, §2 row 7).
- [ ] The per-project purge removes recovery records; Trust Settings' *Retained locally* figure
      counts them (D10, D14, §2 row 8).
- [ ] `local-data-and-privacy.md` has its section, **and the sentence saying the retained figure
      counts transcripts only is corrected** — it is made false by this slice.
- [ ] The setting exists and defaults on (D15).
- [ ] Nothing in this slice writes to a path inside the project (§1 row 1).

## PR-027-C — the offer

- [ ] **Measurement 1 — the offer** lists each recoverable buffer with its project and path, and
      **declining leaves every file on disk untouched**, proved on real files (D4, §1 row 3).
- [ ] **Measurement 2 — the real round trip**: edit without saving, `SIGKILL`, restart, recover,
      verified against what is on disk (D6).
- [ ] The **unchanged** disk file restores the buffer as dirty.
- [ ] The **changed** disk file goes through the existing `ExternalChanged`/conflict path, and
      **no new conflict vocabulary was minted** (D5, §1 row 2). If one seemed necessary, that is
      reported as a finding instead.
- [ ] The **deleted** disk file offers the text with the deleted state.
- [ ] The offer says recovered buffers come back **without undo history** (D3, §3 row 10).
- [ ] Recovery data with no marker is reported as a cleanup bug, not consumed as a crash
      (D1, §3 row 11).
- [ ] **Live capture**, including the changed-on-disk case, not only the easy one.

## Whole-RFC

- [ ] `REQ-RECOVER-002` and `REQ-RECOVER-005`'s coverage rows updated — and **the "where safe" and
      "where technically feasible" hedges are reported as *decided*, naming what they were decided
      to mean**, not repeated back.
- [ ] The colour-alone, i18n completeness and internal-identifier scans still pass.
- [ ] `cargo fmt`, `clippy --workspace --all-targets -D warnings`, `git diff --cached --check`
      **after staging**, `rfc_docs_invariants`, `cargo test --doc --workspace`, **three consecutive
      full-workspace runs with `--no-fail-fast`**, **0 fixture entries left** in a fresh short fixed
      `TMPDIR` — a literal, not `mktemp`.
- [ ] Every new intermittent has a dated row in `test-process-leak.md`.
- [ ] The changelog is written incrementally as slices close, **and re-read against the finished set
      at the candidate** — not against the last slice's diff (the lesson of RFC-065 review 479).
- [ ] The book is read against the changelog **in both directions**, including for any user-visible
      word this RFC's commits touch (`release-checklist.md`, "A word quietly widening").
- [ ] The core pin bumps with the version. *(Release-cut item; not a Whole-RFC item.)*
- [ ] Commits are pushed once the gate is green.

## Final Acceptance Decision

- [ ] Accepted.
- [ ] Accepted with required follow-up.
- [ ] Requires re-review after changes.

Reviewer notes:

```text
```
