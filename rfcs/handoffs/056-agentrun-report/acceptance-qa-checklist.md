---
title: "RFC-056 — acceptance and QA checklist"
rfc: "RFC-056"
rfc_file: "../../accepted/056-agentrun-report-and-classification.md"
source_rfc_status: "Accepted 2026-09-25 — agent remainder"
target_milestone: "agent remainder"
created: "2026-09-25"
---

# Acceptance and QA checklist

Tick a box when the evidence for it is in `qa-evidence.md`, not when the code looks right. A box that
says *measured* with no number in the evidence is not ticked.

## PR-056-A — the pin, and two gate steps

- [x] `tekstide-core = { version = "0.27.0", path = "crates/tekstide-core" }` in
      `[workspace.dependencies]`.
      *(Pinned to `"0.26.0"`, the crate's version today: `"0.27.0"` cannot resolve until the crate is `0.27.0`.
      A test holds the pin equal to the version, so the `0.27.0` candidate's bump is red until the pin moves.
      `qa-evidence.md` says so.)*
- [x] The release procedure gains both post-publish steps: **install this release into a temporary
      root** and assert it builds; assert the app archive's `Cargo.lock` names the matching core.
- [x] The `0.27.0` changelog says older pinned installs do not build, that published metadata is
      frozen, and what the remedy is.
- [x] Evidence shows the `0.25.0` failure reproduced at least once, with the three `E0004` lines — so
      the fix is measured against the defect, not against a description of it.

## PR-056-B — the record

- [ ] `run.json` sits in `agent-run-<id>/` beside the transcript, carries a **version field**, and
      holds references and the user's words only — no run content.
- [ ] It is written **atomically** and **on every change**, not only at run end. Ablated: a
      classification set mid-run survives a kill.
- [ ] A restored run carries its prompt, profile, ids and classification, and **its transcript is
      attached to it** rather than orphaned. Captured live: close the app, reopen, the run is there.
- [ ] A run whose process was killed with the app **says its ending is unknown** — not the start
      time, not zero, not the moment the record was read. Ablated.
- [ ] A corrupt record, and a record with an unknown version, are each **moved aside and named**; the
      run then appears as a transcript with no run, never as a run with invented fields.
- [ ] The caps hold, and the record **says it was bounded**: a run with more ids than the cap and an
      over-long note.
- [ ] The disk-usage figure counts the record as the product's own, not as unclaimed bytes.

## PR-056-C — purge takes it too

- [ ] Purge removes the record and the transcript, then the directory **only if it is then empty**.
- [ ] **The ablation plants a third file in the run directory**: the directory and that file survive
      the purge. This is the box that matters.
- [ ] The purge dialog's counts include the record, so it never promises less than it removes.
- [ ] After a purge, **a search of the state directory** finds nothing naming the purged run's
      prompt, changed paths or notes. Asserted against the disk, not against the code.

## PR-056-D — the classification, the notes, and the report

- [ ] The seven classifications and no more; `custom` carries a user string, quoted **on render and
      in the exported file**, including a bidi control and a newline.
- [ ] Nothing but the user ever writes the notes field — held structurally, not by review.
- [ ] The report is assembled on demand and **Tekstide keeps no second copy**; the export says what
      the file will contain and that it is then outside the product's reach.
- [ ] The exported file keeps the user's notes distinguishable from agent-derived content.
- [ ] **Reachable, captured live, no environment variable** (D10): a person classifying a run, writing
      a note, and exporting the report.
- [ ] The book says what the report contains, where it does not go, and that a purge does not chase
      an exported file.

## Whole-RFC

- [ ] `REQ-AGENT-011` and `REQ-AGENT-015` move to implemented **with evidence a user can reach both**
      — the capture, not the fields. RFC-021 is why this box is worded that way.
- [ ] The colour-alone, i18n completeness and internal-identifier scans still pass.
- [ ] `cargo fmt`, `clippy --workspace --all-targets -D warnings`, `git diff --cached --check` after
      staging, `rfc_docs_invariants`, **three consecutive full-workspace runs with `--no-fail-fast`**
      to files, **0 fixture entries left** in a fresh short `TMPDIR`.
- [ ] Every new intermittent failure has a dated row in `test-process-leak.md`.
- [ ] Commits are pushed once the gate is green.

## Final Acceptance Decision

- [ ] Accepted.
- [ ] Accepted with required follow-up.
- [ ] Requires re-review after changes.

Reviewer notes:

```text
```
