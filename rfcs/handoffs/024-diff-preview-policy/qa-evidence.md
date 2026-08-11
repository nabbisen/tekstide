---
title: "RFC-024: Diff Preview Policy - QA Evidence"
rfc: "RFC-024"
rfc_file: "../../proposed/024-diff-preview-policy.md"
status: "PR-024-B implemented 2026-08-11, not yet reviewed; RFC-012 Amendment 1 (ChangeLifecycle) implemented 2026-08-11, not yet reviewed; PR-024-C content-access work not yet started"
target_milestone: "M10"
created: "2026-08-11"
---

# QA Evidence

Record results here as each slice lands: gate output, ablations with the exact failure
they produced, findings, and limitations.

**This file is where results go. It is not where obligations go.** If a slice discovers
something a later slice must handle, put it in that slice's entry in
`task-breakdown-pr-plan.md` as well. Four obligations have been lost to that gap.

## Recording conventions

- **Ablations name the exact failure**, not "the test failed."
- **One ablation per property.** An ablation breaking two things proves neither.
- **A green ablation is a defect in the ablation**, not a pass.
- **Measured beats estimated.** Two bounds in this project were wrong until measured.
- **Retire obligations explicitly.** When a carried item stops applying, say so and why,
  where it was recorded.
- **Correct by annotation, not rewrite.** If a claim here is later found wrong, annotate
  it in place — the record of *when* something was discovered is part of the evidence.

## PR-024-A — Design and handoff acceptance

Granted by the human owner 2026-08-11 with RFC-024. Handoff pack authored the same day.

## PR-024-B — Gating and bounds

Implemented 2026-08-11, not yet reviewed. Against `task-breakdown-pr-plan.md`'s review gate.

**A real architectural question, raised before writing any code, not guessed at.** Before
starting, I traced where a diff's "before" text would actually come from and found it
does not exist anywhere in this codebase today for the only production-reachable
detection source (`FilesystemSnapshot` — Git detection remains unavailable/unsupported
per RFC-012 §Resolved Decisions). `GeneratedChangeDetector::capture_filesystem_baseline_for_agent_run`
captures metadata only (`ReviewBaselineEntry { relative_path, kind, len, modified_unix_nanos }`,
no content, no hash); RFC-012 §Design Principles states summaries "must not include file
contents"; `ChangeSet::artifact_refs` is documented as "currently opaque strings" with
"durable reference semantics remain future work." By the time a diff is requested, an
AgentRun has typically already overwritten a modified file, so the "before" bytes are
gone, not merely unretained. Filed as review request 187 rather than guessed at, since
two different answers produce two structurally different architectures for PR-024-C and
PR-024-D. **This slice does not depend on the answer** — gating and bounds apply
symmetrically to whichever side of a diff is being read, whenever "before" content turns
out to come from — so it proceeds while that question is open; PR-024-C is where I stop
until it is answered.

**Starting state confirmed by enumeration, not assumed.** `grep -rn "changed_paths\|changed_files\|DetectedChanges\|DetectedChangedPath"` across both crates, excluding `change_detection.rs` itself, matched only `ChangeSet`'s own metadata fields (`domain/changeset.rs`) and `ProjectSession`'s attach-to-ChangeSet code (`project/session.rs:656-701`) — both path/count metadata, never content. Nothing in `tekstide-core` read generated-change content before this slice.

**The gate, in the order Decision 2 → Decision 4 requires.** `gate_diff_content_read` (new, `crates/tekstide-core/src/project/diff.rs`): confirm the path is in `detected.changed_paths` (Decision 1 clause 1) → resolve it through the existing `ProjectFileAccessPolicy::resolve_existing` (the same root/symlink safety `TextDocument::open` uses, not re-implemented) → check size against `fs::metadata` alone, before any read → sniff a bounded prefix for a `NUL` byte to classify text vs. binary. A full read is never attempted by this function; `Readable` is the furthest it goes — content access itself is PR-024-C's job.

**Non-`File` kinds (`Deleted`, `Directory`, `Symlink`, `Other`) never reach the filesystem at all.** Reported as `DiffGateDecision::NonFile { kind }` immediately from the already-detected kind — RFC-024 itself names this as a real outcome ("a non-text change is reported as a change with its size and kind"), extended here to the non-`File` case symmetrically. A `Symlink` specifically is never resolved or touched, matching this project's consistently cautious treatment of symlinks elsewhere (`FileAccessSymlinkStatus`, the explorer's status-not-target decision).

**Two properties proven, not merely ordered correctly in the source — both ablated for real.**

1. *Refusal happens before any content read.* `refusal_happens_from_metadata_alone_before_any_open_is_attempted` writes an oversized file, `chmod 000`s it (removing all permissions — `fs::metadata` needs only directory search permission and still succeeds; `File::open` for reading would fail with `EACCES`), and asserts the result is `TooLarge` with the real, accurately measured length. That result is only reachable if the size check ran from metadata alone and returned before ever attempting to open the file. **Ablated**: moved a `sniff_is_binary` call ahead of the size check — the test failed correctly, `Err(MetadataUnavailable {...})` instead of `Err(TooLarge {...})`, proving the open attempt against the 0o000 file failed exactly where an unordered implementation would surface it. Reverted before committing.
2. *The binary sniff never reads past its own bound.* `the_binary_sniff_never_reads_past_its_own_bound` uses a real FIFO (`libc::mkfifo`): a writer thread supplies exactly `BINARY_SNIFF_BYTES` (with a `NUL` first byte, so classification succeeds immediately) and then blocks without closing — an unbounded reader on the other end would itself block waiting for more data that never comes, since nothing sends EOF. The sniff call runs on its own thread with a 500 ms timeout via `mpsc::recv_timeout`. **Ablated**: removed the `.take(BINARY_SNIFF_BYTES as u64)` bound from `sniff_is_binary` — the test failed correctly with `Timeout`, proving a hang is exactly what an unbounded read produces against this fixture. Reverted before committing.

**Boundary test, against the real default, not a substituted small policy.** `the_boundary_is_exact_not_greater_than_or_equal` writes real files at exactly `DEFAULT_MAX_DIFF_INPUT_BYTES` and one byte over; `== bound` returns `Readable`, `bound + 1` returns `TooLarge` with the real length — matching `content_within_bound_accepts_content_exactly_at_the_cap`'s own shape for RFC-018's bound.

**No truncation behaviour exists anywhere.** `TooLarge` is a terminal refusal (`Err`); nothing in this module ever shortens content — there is no code path that could, since the size check runs before any content-length-bearing read.

### Open question 1 — the bound's number, measured

**`DEFAULT_MAX_DIFF_INPUT_BYTES = 4 MiB`, applied to each version independently (not their sum).** Measured, not estimated: a throwaway Rust harness (`rustc -O`, `/proc/self/status` `VmRSS` deltas before/after allocating two `String`s built from a repeated realistic line pattern, not a pathological all-zero buffer) showed holding two ~4 MiB text buffers costs approximately 10.2 MiB of real transient RSS:

```
mib=4 before_text_len=4194342 after_text_len=4194379 rss_delta_kb=10248
```

(Full sweep: 1/2/4/8/16 MiB per side cost roughly 2.0/6.1/10.2/20.0/40.0 MiB RSS — consistently under 2.5× the raw per-side size, not a runaway multiplier.) Chosen to equal RFC-019's own reviewed `DEFAULT_MAX_EDITABLE_BYTES` rather than an unrelated new constant: a diff is fundamentally a comparison of two files, and bounding each side at the standard a human already edits one file under is coherent, not arbitrary. ~10.2 MiB transient (dropped immediately after the request, per Decision 1's third clause) is well inside safe headroom for a single on-demand action. **Disclosed limitation**: no diff algorithm exists yet (Decision 6: this RFC is not a diff engine, and none of its four slices' scopes mention invoking one), so "the computed difference" component of Decision 2's own bound reasoning is not itself measured — only the cost of holding both raw versions is. A future slice that adds a real diff computation should re-verify memory behaviour against it, not assume this figure already accounts for it.

### Open question 3 — lazy per-path, not eager whole-set

**Lazy, per-path, on explicit request.** Not a separate judgment call so much as what Decision 1 itself already requires: "content is read only on demand... never speculatively, never in the background." An eager whole-set diff would read content for every detected path before any user asked to review a specific one, which is exactly what Decision 1's second clause forbids. This interacts with the bound's shape as RFC-024 itself predicted: because gating is per-path, `DEFAULT_MAX_DIFF_INPUT_BYTES` bounds one file at a time rather than needing to reason about a whole change set's aggregate size.

## PR-024-C — Content access with a bounded lifetime

**Unblocked 2026-08-11 (response 187).** The architect confirmed the gap raised in
PR-024-B's own entry above and landed a scope correction in RFC-024's own text (`f48d245`)
before this slice starts, rather than leaving the fix to be inferred here: this RFC
cannot deliver a two-sided diff for a modified file under `FilesystemSnapshot`
detection — the before-bytes were never captured (deliberately, per RFC-012 §Design
Principles 2) and are gone by diff-request time, not merely unretained. Capturing
content at baseline time was rejected as a fix — it contradicts RFC-012's own stated
principle directly and needs RFC-012 **and** RFC-011 amendments plus owner
authorisation, not an implementation-level choice. Full correction text and reasoning:
`rfcs/proposed/024-diff-preview-policy.md` §Correction, 2026-08-11;
`task-breakdown-pr-plan.md`'s own PR-024-C entry carries the corrected per-change-kind
table forward as this slice's binding scope.

**A second gap, found immediately on starting implementation, filed as review request
189 rather than guessed at.** The corrected table needs to distinguish **Added** from
**Modified** for `File`-kind changes (Added: no "not a diff" label, current content is
the whole change by definition; Modified: current content, explicitly labelled not a
diff). `DetectedChangedPath { relative_path, kind }` cannot express this —
`changed_paths_between` (`change_detection.rs`) computes the distinction internally
(`(Some(_), Some(after))` = Modified vs. `(None, Some(after))` = Added) and discards it,
since both arms construct the same `{ relative_path, kind: after.kind }` shape.
`DetectedChanges` carries only an opaque `baseline_snapshot_ref: Option<String>`, not the
baseline's own entries, so the distinction cannot be recovered from outside
`change_detection.rs` either. This is RFC-012's own model, a different closed RFC — not a
change to make unilaterally inside this slice. Recommended a minimal, additive fix
(preserve the distinction `changed_paths_between` already computes, e.g. a
`ChangeLifecycle { Added, Modified, Deleted }` field) but did not implement it. **Not
started**: no PR-024-C implementation code exists yet.

**Response to 189 found a deeper defect than the one recommended above, and RFC-012
Amendment 1 landed 2026-08-11 to fix it.** The architect's own review of the
recommendation above found `Deleted` was in the wrong enum from the start:
`ChangePathKind { File, Directory, Symlink, Deleted, Other }` conflated "what kind of
thing" with "what happened to it" — two orthogonal axes. Adding `ChangeLifecycle`
*alongside* the recommendation above, without also removing `Deleted` from
`ChangePathKind`, would have made `{ kind: Deleted, lifecycle: Added }` representable
and meaningless — "precisely the mislabelling class response 187 just corrected this RFC
for" (architect's words). The owner authorised the breaking removal "on the grounds that
dead code harms future maintenance and extensibility." Full amendment text:
`rfcs/done/012-generated-change-review-foundations.md` §Amendment 1.

**Implemented 2026-08-11, not yet reviewed.** `ChangeLifecycle { Added, Modified,
Deleted }` added as a new field on `DetectedChangedPath`
(`crates/tekstide-core/src/project/change_detection.rs`); `Deleted` removed from
`ChangePathKind` (now `{ File, Directory, Symlink, Other }`). `changed_paths_between`'s
four match arms now set `lifecycle` explicitly per case, using the same distinction the
function already computed and previously discarded (no new detection capability, no
content read — matching the amendment's own justification for staying amendment-shaped
rather than a new RFC). `gate_diff_content_read`
(`crates/tekstide-core/src/project/diff.rs`) updated to check `changed.lifecycle ==
ChangeLifecycle::Deleted` *before* consulting `kind` at all — `DiffGateDecision` gained a
`Deleted { kind }` variant (kind reports what the path *was*, from the baseline) and
`Readable`/`NonTextContent` both gained a `lifecycle` field, satisfying the task-breakdown
gate item added alongside the amendment: "the Added/Modified/Deleted distinction is read
from `ChangeLifecycle`, never inferred from `ChangePathKind`."

**Ordering ablated for real.** Moved the `Deleted` check to run *after*
`ProjectFileAccessPolicy::resolve_existing` instead of before it, then re-ran
`a_deleted_path_is_reported_without_touching_the_filesystem` (whose fixture path,
`gone.txt`, is never written to disk). **Ablated result**: `Err(Access(FileAccessError {
reason: MissingPath, .. }))` instead of `Ok(DiffGateDecision::Deleted { kind: File })` —
proving the check-before-resolve ordering is load-bearing: without it, a deleted `File`-
kind path is reported as a filesystem-access failure instead of a deletion, the wrong
outcome for a caller trying to distinguish "refused" from "nothing to diff, reported."
Reverted before committing; the restored file was diffed against the pre-ablation backup
to confirm no other change survived the revert.

Full workspace gates after the amendment: `cargo fmt --all --check`,
`cargo clippy --workspace --all-targets --all-features -- -D warnings`,
`cargo test --workspace --all-targets --all-features` (518 passed, 0 failed), and
`git diff --check` all clean.

**Still not started**: PR-024-C's own content-access implementation (the bounded read
itself, the per-change-kind content shape, and the review gate items in
`task-breakdown-pr-plan.md` beyond the `ChangeLifecycle` gate item this entry covers).
This entry documents adopting the amendment — a prerequisite the amendment note itself
names PR-024-C's "second prerequisite" — not PR-024-C's content-access work.

## PR-024-D — Baseline authority, and closeout

Pending implementation.

## Known Limitations

Consolidated at closeout. Carried in from RFC-024's own text:

- **This RFC renders nothing.** RFC-020 owns the surfaces; content produced here is
  unescaped by design.
- **No action on a change** — no accept, revert, or stage. Detection and preview only.
- **Git-backed detection is unchanged**, and still gated behind RFC-012's own safety
  evidence.
- **The diff algorithm is not this RFC's contribution.** Its value is the policy around a
  solved problem.

## What this RFC hands to RFC-020

To be filled in at closeout — RFC-020's handoff will be written from this section:

- the produced diff's shape, and that it is unescaped;
- the refusal's shape, so a surface can render one rather than showing nothing;
- the stale-baseline signal's shape;
- the bound, so RFC-020 does not introduce a second one.
