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

- [x] `run.json` sits in `agent-run-<id>/` beside the transcript, carries a **version field**, and
      holds references and the user's words only — no run content.
- [x] It is written **atomically** and **on every change**, not only at run end. Ablated: a
      classification set mid-run survives a kill.
      *(Ablations 1, 11, 17. A status change reaches the record within a second; the user's own change is written at once.)*
- [ ] A restored run carries its prompt, profile, ids and classification, and **its transcript is
      attached to it** rather than orphaned. Captured live: close the app, reopen, the run is there.
      *(Unticked, split: prompt, profile, ids and the attached transcript are tested and were captured live
      — `evidence/pr-056-b/01`. **The classification cannot be captured live until PR-056-D gives a person a
      way to set one**; its round trip is tested and ablated (1). D's live capture closes this box.)*
- [x] A run whose process was killed with the app **says its ending is unknown** — not the start
      time, not zero, not the moment the record was read. Ablated.
      *(Ablations 2, 3, 14, 15; live: `evidence/pr-056-b/01`.)*
- [x] A corrupt record, and a record with an unknown version, are each **moved aside and named**; the
      run then appears as a transcript with no run, never as a run with invented fields.
      *(Ablations 4, 5, 13, 16; live for the corrupt case: `evidence/pr-056-b/03`. The unknown-version case is tested, not captured.)*
- [x] The caps hold, and the record **says it was bounded**: a run with more ids than the cap and an
      over-long note.
      *(Ablations 6, 7, 8.)*
- [x] The disk-usage figure counts the record as the product's own, not as unclaimed bytes.
      *(Ablation 9, isolated.)*

## PR-056-C — purge takes it too

- [ ] Purge removes the record and the transcript, then the directory **only if it is then empty**.
- [ ] **The ablation plants a third file in the run directory**: the directory and that file survive
      the purge. This is the box that matters.
- [ ] The purge dialog's counts include the record, so it never promises less than it removes.
- [ ] After a purge, **a search of the state directory** finds nothing naming the purged run's
      prompt, changed paths or notes. Asserted against the disk, not against the code.

### Required at review 437

- [ ] **Q1:** `is_run_record_file_name` matches **exactly** — `run.json`, `run.json.tmp`,
      `run.json.corrupt`, or `run.json.corrupt-` followed by digits — and nothing else. It is a
      `starts_with` today, which in B miscounts bytes and in C would **delete files Tekstide never
      wrote**. Lands before the code that deletes.
- [ ] **Ruled at 437:** purge takes `run.json`, `run.json.tmp` and every aside name. D2 is a rule
      about content, and a set-aside record holds exactly what D2 says must not outlive a purge.
- [ ] **The third-file test plants `run.json.corruption-notes`** — a name a prefix match eats and an
      exact match does not, so it fails against today's matcher and passes only against Q1's.

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
