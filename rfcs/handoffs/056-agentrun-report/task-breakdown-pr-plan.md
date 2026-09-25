---
title: "RFC-056 — task breakdown and PR plan"
rfc: "RFC-056"
rfc_file: "../../accepted/056-agentrun-report-and-classification.md"
source_rfc_status: "Accepted 2026-09-25 — agent remainder"
target_milestone: "agent remainder"
created: "2026-09-25"
---

# Task breakdown and PR plan

Four slices. **A is not about this RFC at all** and goes first because it repairs a defect that is
live in published artifacts right now.

## PR-056-A — the pin, and two gate steps that would have caught it

`Cargo.toml`, and the release procedure.

Found in `0.26.0`'s post-publish check: `cargo install tekstide --version 0.25.0` **fails to
compile**. It resolves `tekstide-core 0.26.0` and dies on three `E0004`s, because every published
`tekstide` carries `[dependencies.tekstide-core] version = "0"` — *any* `0.x` — which opts out of
Cargo's own rule that `0.26` and `0.25` are incompatible. **Corrected at review 436:** the claim that `--locked` cannot rescue it was **wrong**. It was
read from the *local* `target/package` archives, which are packaged before their core is on the
registry and so name the previous one. The **published** archives name the matching core
(`0.25.0` → core `0.25.0`), and `cargo install tekstide --version 0.25.0 --locked` builds —
measured against the registry, twice, independently. The defect is `version = "0"` alone.

- `tekstide-core = { version = "0.27.0", path = "crates/tekstide-core" }` in
  `[workspace.dependencies]`, bumped with each release from now on.
- Two post-publish gate steps, written into the release procedure: **install this release into a
  temporary root** and assert it builds, and assert the app archive's `Cargo.lock` names the
  **matching** core.
- `0.27.0`'s changelog says plainly that older pinned installs do not build, that their metadata is
  frozen and cannot be repaired, and that the remedy is the latest release or a git tag.

Land it alone. It is small, it is not this RFC, and it should not wait behind it.

## PR-056-B — the record

`tekstide-core`: the transcript/state layer, and `AgentRun`. **No surface change.**

- `run.json` beside the transcript in `agent-run-<id>/`. References and the user's own words only:
  profile, prompt summary, timestamps, transcript ref, change-set/approval/audit ids, status,
  classification, notes. **A version field from this first release** (R4/D7).
- Written **atomically**, and **on every change**, not only at run end (D9): a classification set
  during a long run survives the app being killed an hour later.
- Read at project open, beside the existing transcript scan, so a restored run carries its prompt and
  profile and its transcript is attached to it rather than orphaned.
- A restored run is complete by definition, or **says it does not know its ending** (D6, D12). Never
  the start time, never zero.
- A record that cannot be parsed, or carries an unknown version, is **moved aside and named** —
  `recent-projects.json`'s `next_corrupt_path` is the precedent, not a new mechanism.
- **Caps** on the notes and on the id lists, and the record says when it was bounded (risk §4).
- The disk-usage figure counts the record as the product's own, not as unclaimed bytes.

## PR-056-C — purge takes it too

`project/session.rs` and the purge dialog.

**Read the risk document's §1 before writing a line of this.** Purge removes the transcript file
today and leaves the directory; a record left behind makes the purge a lie.

- Purge removes the record and the transcript, then the directory **only if it is then empty**.
- The dialog's counts (`purgeable_transcript_count`, `purgeable_transcript_bytes`) include the
  record, so the dialog never promises less than it removes.
- **The ablation plants a third file in the run directory** and asserts the directory and that file
  survive a purge.
- After a purge, a search of the state directory finds nothing naming the purged run's prompt, paths
  or notes — asserted against the disk.

## PR-056-D — the classification, the notes, and the report

`tekstide-core` and the GUI, the catalog, the book.

- The **seven** classifications `REQ-AGENT-015` names and no more; `custom` carries a user string,
  quoted on render *and in the exported file*, including a bidi control and a newline.
- Notes: the user's text, and **nothing else ever writes them** (D4).
- The report assembled on demand from the record plus whatever transcript and change sets still
  exist, and written where the user asks. **Tekstide keeps no second copy**, and says at the moment
  of export what the file will contain and that it is then outside the product's reach.
- The exported file keeps the user's notes distinguishable from agent-derived content (risk §3).
- **Reachable, and captured live** (D10): no environment variable, a person doing it. `REQ-AGENT-011`
  and `REQ-AGENT-015` move to implemented on that capture, not on the fields existing — RFC-021 is
  the precedent for why.

## Order, and why

A first and alone, because it is repairing something already published. B before C because purge
cannot take a record that does not exist. C before D because a record that outlives a purge is a leak,
and adding the surface that fills it with a user's notes before closing that is the wrong order to
discover it in. D last, where the words, the book and the capture move together.
