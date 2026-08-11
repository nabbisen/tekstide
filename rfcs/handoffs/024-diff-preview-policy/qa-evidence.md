---
title: "RFC-024: Diff Preview Policy - QA Evidence"
rfc: "RFC-024"
rfc_file: "../../done/024-diff-preview-policy.md"
status: "All four slices accepted — PR-024-B (response 192, after a relay gap recorded below), RFC-012 Amendment 1 (response 190), PR-024-C (response 191), PR-024-D (response 193). RFC-024 closed to rfcs/done/ 2026-08-11."
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

Implemented 2026-08-11. Against `task-breakdown-pr-plan.md`'s review gate.

> **Status correction (2026-08-11).** Review request 188 was filed for this slice the same
> day, before RFC-012 Amendment 1 or PR-024-C existed. It never received a response.
> Responses 190 and 191 (for the amendment and PR-024-C, both built on top of this slice)
> each independently flagged that PR-024-B has never actually been reviewed on its own —
> the earlier status lines in this file that said "PR-024-B accepted (response 190)" were
> wrong: response 190 reviewed only the `ChangeLifecycle` amendment commit, never this
> slice's own commit (`46a40fa`). Resubmitted as review request 192, since 188 appears not
> to have reached the reviewer through whatever relays these files. Left unresolved as of
> this note.
>
> **Root cause confirmed (response 193, 2026-08-11).** Both `188` and `192` exist on disk
> and were never relayed to the reviewer — the architect's own words: "I was wrong four
> times in a row... concluding from 'I did not see it' that 'it does not exist'." Not a
> filing gap on this side; a relay gap, now named rather than guessed at. `188`/`192` still
> need relaying and PR-024-B still needs a real review before RFC-024 can close — response
> 187's passing praise of two fixtures inside a different slice's review does not count as
> one.
>
> **Accepted (response 192, 2026-08-11).** Once relayed, verified not only against the
> original commit (`46a40fa`) but against current `main` — the ordering survived two
> subsequent slices building on it, including PR-024-C's `evaluate_gate` refactor, which
> reused it rather than re-deriving it. Both fixtures singled out as "the best evidence in
> this RFC": the `chmod 000` fixture "turns an ordering claim into an observable one," and
> the FIFO fixture "proves boundedness, which no assertion on a return value can." RFC-024
> closed to `rfcs/done/` the same day.

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
`rfcs/done/024-diff-preview-policy.md` §Correction, 2026-08-11;
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

**Still not started** *(as of the amendment landing; superseded below — the content-access
work this paragraph described as not-started is now implemented and reviewed via response
190, which accepted the `ChangeLifecycle` amendment above and cleared PR-024-C to begin)*:
PR-024-C's own content-access implementation (the bounded read itself, the per-change-kind
content shape, and the review gate items in `task-breakdown-pr-plan.md` beyond the
`ChangeLifecycle` gate item this entry covers). This entry documents adopting the
amendment — a prerequisite the amendment note itself names PR-024-C's "second
prerequisite" — not PR-024-C's content-access work.

### PR-024-C's own content access — implemented 2026-08-11, not yet reviewed

**Reuses the gate rather than re-deriving its checks.** `read_diff_content`
(`crates/tekstide-core/src/project/diff.rs`) calls the same evaluation
`gate_diff_content_read` itself calls (factored into a private `evaluate_gate`, so the two
public functions share one implementation rather than PR-024-C re-deriving PR-024-B's own
Added/Modified/Deleted-from-`ChangeLifecycle` property). `evaluate_gate` was also changed
to carry the already-resolved `FileAccessTarget` alongside its `Readable`/`NonTextContent`
outcomes internally, so `read_diff_content` performs the bounded read against the exact
path the gate already resolved — no second, independent `resolve_existing` call, and so no
TOCTOU-race error case to reason about or (impossibly, without real concurrent file
mutation) test. `gate_diff_content_read`'s own public signature and observable behaviour
are unchanged; this is a pure internal refactor for reuse, re-verified by rerunning every
PR-024-B test afterward (all still pass, including the ordering ablation below).

**A design refinement made while implementing, disclosed rather than silently done**:
`DiffGateDecision::Readable`/`NonTextContent`'s `lifecycle` field was narrowed from
`ChangeLifecycle` (three variants) to a new, module-private-in-spirit-but-necessarily-public
`ContentLifecycle { Added, Modified }` (two variants). Reasoning: a `Deleted` lifecycle is
always returned as `DiffGateDecision::Deleted` before either field-bearing variant is
reachable, so carrying the full `ChangeLifecycle` there would let `Readable { lifecycle:
Deleted }` exist as a representable-but-meaningless value — the identical class of bug
RFC-012 Amendment 1 just fixed for `ChangePathKind` carrying its own `Deleted`, reproduced
one level up in this module's own types if not narrowed here too. The conversion from
`ChangeLifecycle` to `ContentLifecycle` happens once, in `evaluate_gate`'s own exhaustive
match with the `Deleted` arm returning early — a compile-time-enforced impossibility, not
an `unreachable!()` covering a runtime assumption. This is local to `diff.rs` (RFC-024's
own module, not RFC-012's), so it did not need to be raised as a review request the way the
`ChangeLifecycle` amendment itself did — full latitude for this module's own internal shape
was already established by PR-024-B's acceptance.

**Corrected scope table, delivered as distinct constructors, not a shared shape**:
`DiffContent::Added { bytes }` and `DiffContent::Modified { bytes }` are separate enum
variants carrying the same `Vec<u8>` shape — RFC-024 §Correction's requirement that
modified content reach the surface "explicitly not a diff" is carried by the variant name
itself, satisfying the review gate's "not left to a doc comment a renderer could ignore."
`DiffContent::Deleted { kind }` reports the fact of deletion from metadata alone (`kind`
from the baseline, RFC-012 Amendment 1); `NonTextContent`/`NonFile` pass through from the
gate exactly as decided, since neither ever had bytes to read.

**Decision 1's third clause made structural, not conventional — the review gate's own
required framing.** `DiffContent` derives neither `Clone` nor `Serialize`. `ProjectSession`
derives `Clone` across all of its fields uniformly (`project/session.rs:35`), so a
`DiffContent` field there would fail to compile; every `AuditCoordinator::record_*` call
requires a `Serialize` event (`audit/recovery.rs`'s own persisted-event shapes), so passing
this type to one would fail to compile too. Both are compile errors available to any future
change, not promises kept only by not calling an API that remains callable — matching this
codebase's own established idiom (`DisplayText`, `VerifiedCwd`, `CommandProposal::decode`:
construction-gated types whose shape itself, not a comment, is what a misuse would run into)
adapted for a non-retention property rather than those three's validity property. By
enumeration (grep), nothing in this commit references `DiffContent` outside `diff.rs`/
`diff/tests.rs` — no wiring into `ProjectSession` or `AuditCoordinator` exists yet, since
RFC-020 owns that surface, not this slice.
>
> **Correction (2026-08-11, response 191).** The paragraph above overstates what is
> enforced. Enum variant fields are public in Rust: a consumer can pattern-match
> `DiffContent::Added { bytes }`, move the `Vec<u8>` out, and retain *that* in any struct
> that is neither `Clone` nor `Serialize` — the two derives block storing the *wrapper*,
> not the bytes once unwrapped. What is actually structural: storing `DiffContent` itself
> in a `Clone` type is a compile error, and passing it to an audit producer is a compile
> error. General retention of the unwrapped bytes is not prevented — a defensible design
> (accepted as such), but the claim above should have said this from the start rather than
> the stronger form. The strictly stronger option, a lifetime-bound
> `DiffContent<'a> { bytes: &'a [u8] }` tied to a request-scoped buffer, would make
> retention genuinely unrepresentable; not required here since it would constrain RFC-020's
> own rendering architecture (rendering would have to happen inside the borrow) and no
> consumer exists yet to weigh that cost against — recorded as an open choice for RFC-020
> to decide once a real caller exists, in RFC-020's own text as well as here.

**Not pre-escaped, proven rather than asserted architecturally.**
`content_is_not_pre_escaped_raw_bytes_survive_unaltered` writes a file containing the exact
right-to-left-override/pop-directional-formatting probe `text_safety`'s own tests use,
reads it back through `read_diff_content`, and asserts the returned bytes equal the
original exactly — no `quote_untrusted` wrapping, no visible-marker substitution. Bytes are
raw (`Vec<u8>`), not decoded to `String`: RFC-024 Decision 4 deliberately chose a NUL-sniff
over "a UTF-8-decode-and-handle-failure," so this function does not perform the stricter
check the sniff exists to avoid.

**Enumeration test, not a one-time grep recorded in prose — PR-024-C's own required review
gate item.** `enumeration_confirms_only_the_closed_list_reads_full_file_content` recursively
scans every non-test `.rs` file under `crates/tekstide-core/src` for a raw full-file byte
read (`read_to_end(`/`fs::read(`), against a closed, disclosed four-entry allowlist:
`project/diff.rs` (this slice), `content/open.rs` (`TextDocument`'s pre-existing, unrelated
editor read), `project/recent/store.rs` and `audit/recovery.rs` (both read a whole file's
bytes but neither reads project/generated-change content — a recent-projects state file and
an audit recovery manifest, respectively; found by the scan itself while writing this test,
not assumed). Uses the same recursive-scan-plus-closed-list technique
`i18n::enforcement`'s scans already use in `crates/tekstide` for a different property.
**Ablated**: removed `project/diff.rs` from the allowlist, reran — failed correctly, naming
`project/diff.rs` exactly. Reverted before committing.

**Refuse-never-truncate ablated for the content read itself, independent of the gate's own
size check.** `read_bounded` mirrors `content::open::read_file_bounded`'s own
`.take(max + 1)`-then-check idiom (a different module, not shared code, but the same shape)
as defense in depth: even though the gate already confirmed size from metadata moments
earlier, this read independently refuses if it observes more than the bound, rather than
trusting the earlier check alone. **Ablated**: changed the read limit from `max_bytes + 1`
to `max_bytes` — a 101-byte file against a 100-byte bound then returned `Ok([...100
bytes...])`, a silently truncated prefix with no signal anything was cut, instead of
`Err(())`. Reverted before committing; both ablations in this section were diffed against a
pre-ablation backup of `diff.rs` after reverting, confirming a clean revert (`diff` exit 0,
no output).

**Gates**: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets
--all-features -- -D warnings`, full workspace suite (`tekstide-core` 527 passed, up from
518 — 9 new tests; `tekstide` 203 passed, unchanged — no `crates/tekstide` changes),
`git diff --check`. All clean.

**Known limitation, disclosed rather than silently accepted**: `DiffContent` still derives
`Debug` (needed for `assert_eq!` throughout this crate's existing test style) and `Debug`
output is not redacted — unlike `TerminalSecurityDiagnostic`, which never stores a raw
payload field at all specifically so its `Debug`/summary output cannot leak one. This
module's `Debug` impl, if ever passed to a logging or diagnostic sink, would print raw file
bytes. No such call site exists today (the enumeration test above would not catch a future
`format!("{:?}", diff_content)` passed to a log macro, since that is not a raw-file-read
call site) — a real, narrower gap than the ones this slice's review gate named, worth
flagging for RFC-020 to keep in mind rather than assuming closed by this slice's other
protections.

## PR-024-D — Baseline authority, and closeout

**Implemented 2026-08-11. Accepted (response 193).** The `ExternalChangeDecision` narrowing
below was independently confirmed as "the third instance of the same discipline" (RFC-012
Amendment 1's `ChangePathKind`, PR-024-C's `ContentLifecycle`, this slice's staleness
type) — "declining part of a reuse instruction, with the reason stated, is better
compliance than reusing wholesale." The ablation was confirmed to find a real security
consequence: folding every `Access` error into `Ok(true)` would report a symlink-escape
attempt as ordinary staleness rather than a policy refusal. Closeout (move to
`rfcs/done/`) was briefly blocked on PR-024-B's own review never having reached the
reviewer — a relay gap, not a defect in this slice — resolved once response 192 accepted
PR-024-B; see the correction in PR-024-B's section above. **RFC-024 closed to
`rfcs/done/` 2026-08-11.**

**Reuses `content::FileSnapshot` — no second staleness mechanism, per Decision 3's own
requirement.** `DiffContent::Added`/`Modified`/`NonTextContent` each gained a `baseline:
FileSnapshot` field, captured by a new `capture_baseline_snapshot` helper
(`crates/tekstide-core/src/project/diff.rs`) at the same moment `read_diff_content` reads
the content itself — reusing the exact type `TextDocument::refresh_external_state` already
compares (`content::FileSnapshot`, already crate-public), not a parallel struct. `Deleted`/
`NonFile` carry no baseline: neither ever resolved a file to snapshot.

**`content_hash` deliberately left `None`.** Hashing would mean either a second full read
of every `Added`/`Modified` file already read once, or — for `NonTextContent` — a full read
of binary content Decision 4's own `NonTextContent` outcome exists specifically to avoid
("no diff is attempted" for a non-text change; hashing it purely for a staleness baseline
would be exactly that, under a different name). `len` and `modified_at` alone match the
granularity RFC-012's own `changed_paths_between` already uses to decide whether a path
changed (`ReviewBaselineEntry`'s `len`/`modified_unix_nanos`, compared by equality) — reused
here rather than invented at a stricter, inconsistent precision for only some outcomes.

**`diff_content_is_stale(baseline, root, path) -> Result<bool, DiffContentError>`** is the
new public function. Only two states are meaningful, not `content::ExternalChangeDecision`'s
three: diff preview is read-only (RFC-024 §Scope: "Out: any *action* on a change"), so there
is no local-edit state a live disk change could *conflict* with the way an open, dirty
`TextDocument` can. Reusing the 3-variant type wholesale would have let a `Conflict` outcome
exist that this read-only flow can never actually produce — the same representable-but-
meaningless-state class PR-024-C already caught and fixed once for `ContentLifecycle`
(caught again here, independently, while designing this slice). A `bool` is the correct,
minimal type for a genuinely binary question, not an enum with a dead arm.

**The review gate's own required proof, against a real file changed on disk, not a
synthesised value.** `a_stale_baseline_is_reported_as_stale_not_silently_diffed`: reads
content (capturing its `baseline`), sleeps 10ms (coarse filesystem mtime resolution), writes
new bytes to the *same real file*, then asks whether that baseline is still current —
`Ok(true)`. `an_unchanged_baseline_is_reported_as_unchanged` is the negative: no mutation,
`Ok(false)` — proving this is a real comparison, not a function that always answers "stale".
`a_file_deleted_since_capture_is_reported_as_stale` covers the file vanishing entirely —
also `Ok(true)`, mirroring `TextDocument::refresh_external_state`'s own treatment of a
missing current target as "changed", not an error.

**A real policy violation is not silently folded into "stale", proven and ablated.**
`a_real_access_violation_surfaces_as_an_error_not_silent_staleness`: a symlink inside the
sandbox escaping the project root (`FileAccessBlockedReason::SymlinkEscape`, the same
fixture shape `root::tests::file_access_blocks_symlink_escape` already uses) must surface
as `Err(DiffContentError::Gate(DiffGateRefusal::Access(_)))`, not `Ok(true)` — mirroring
`TextDocument::refresh_external_state`'s own narrower distinction, where only
`FileAccessBlockedReason::MissingPath` folds into a changed-state outcome and every other
access refusal propagates. **Ablated**: removed the `MissingPath` guard so every `Access`
error folded into `Ok(true)` — the test then failed exactly as expected, returning `Ok(true)`
instead of the real `Err`, proving a security-relevant refusal would otherwise be silently
swallowed as ordinary staleness. Reverted before committing; diffed the restored file
against a pre-ablation backup (clean, no output).

**Open question 2 answered in RFC-024's own text**, not only here (per this slice's own
review gate item): per-path, not whole-review. Full reasoning:
`rfcs/done/024-diff-preview-policy.md` §Open questions, item 2. Summary: this RFC's own
content-access model is already per-path (Decision 1 clause 2, on-demand); whole-review
staleness would need either a fresh RFC-012 re-scan (out of scope) or a review-level flag
answering a question no user asked. Real cost weighed as the question requires: one extra
bounded `fs::metadata` call per check, the same cost class already accepted for the original
read.

**No claim that this RFC renders anything, and no claim about diff quality** — unchanged
from PR-024-A through C; PR-024-D adds no rendering code and no diff algorithm.

**Gates**: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets
--all-features -- -D warnings`, full workspace suite (`tekstide-core` 531 passed, up from
527 — 4 new tests; `tekstide` 203 passed, unchanged — no `crates/tekstide` changes),
`git diff --check`. All clean.

**Not done in this slice**: moving `rfcs/proposed/024-diff-preview-policy.md` to
`rfcs/done/`, and the index/reference updates that go with it — deferred to after this
slice's own review, matching this project's established closeout sequencing (RFC-019 was
moved to `rfcs/done/` only after its own closeout review was accepted, not alongside it).

> **Update (2026-08-11).** The move happened after response 193 accepted this slice and
> response 192 separately accepted PR-024-B (closing the relay gap recorded in PR-024-B's
> own section above). `rfcs/proposed/024-diff-preview-policy.md` is now
> `rfcs/done/024-diff-preview-policy.md`; every cross-reference to the old path in this
> handoff pack and `rfcs/README.md` was updated in the same commit as the move.

## Known Limitations

Consolidated at closeout. Carried in from RFC-024's own text:

- **This RFC renders nothing.** RFC-020 owns the surfaces; content produced here is
  unescaped by design.
- **No action on a change** — no accept, revert, or stage. Detection and preview only.
- **Git-backed detection is unchanged**, and still gated behind RFC-012's own safety
  evidence.
- **The diff algorithm is not this RFC's contribution.** Its value is the policy around a
  solved problem.
- **No two-sided diff for a modified file.** Only current content, explicitly labelled not
  a diff (RFC-024 §Correction) — the before-bytes were never captured under
  `FilesystemSnapshot` detection and are gone by request time.

Found during PR-024-C/D, carried forward per response 191's own instruction:

- **`DiffContent`'s retention protection is narrower than a first reading suggests.**
  Deriving neither `Clone` nor `Serialize` blocks storing the wrapper in a `Clone` state
  struct (`ProjectSession`) or passing it to an audit producer — both compile errors. It
  does **not** prevent a consumer from pattern-matching a variant, moving the `Vec<u8>` out,
  and retaining the unwrapped bytes indefinitely; general retention is not structurally
  prevented, only those two specific paths are. See the correction in this file's PR-024-C
  section and RFC-020's own §Open questions item 4.
- **`DiffContent` derives `Debug`, unredacted.** Unlike `TerminalSecurityDiagnostic`, which
  never stores a raw payload field at all so its `Debug` output structurally cannot leak
  one, `DiffContent`'s `Debug` would print raw file bytes if ever passed to a logging or
  diagnostic sink. No such call site exists today, and the enumeration test would not catch
  one if it appeared (`format!("{:?}", ...)` is not a raw-file-read call site).
- **The owned-vs-lifetime-bound question is now RFC-020's to decide**, against its own real
  rendering shape (Option A/B, iced's update/view cycle) — recorded as RFC-020's own §Open
  questions item 4, not decided here, since PR-024-C had no real consumer to weigh the cost
  against.

## What this RFC hands to RFC-020

- **The produced diff's shape, and that it is unescaped.** `DiffContent::Added { bytes,
  baseline }` / `Modified { bytes, baseline }` — raw `Vec<u8>`, not `String`, not escaped.
  `Modified`'s own variant name is the "not a diff" label; a renderer must not display it
  under a heading implying a two-sided comparison.
- **The refusal's shape**, so a surface can render one rather than showing nothing:
  `DiffGateRefusal` (gating-time) and `DiffContentError` (read-time), both `pub` from
  `tekstide_core::project`.
- **The non-text and non-file shapes**: `DiffContent::NonTextContent { len, lifecycle,
  baseline }` and `DiffContent::NonFile { kind }` — report a change without attempting
  content.
- **The deletion shape**: `DiffContent::Deleted { kind }` — the fact of deletion, from
  metadata, `kind` reporting what the path *was*.
- **The stale-baseline signal's shape.** `diff_content_is_stale(baseline, root, path) ->
  Result<bool, DiffContentError>`, per-path (Open question 2). `baseline: FileSnapshot` is
  carried on every content-bearing `DiffContent` variant; RFC-020 must call this before
  trusting previously-fetched content is still current, the same way `TextDocument` already
  re-checks before trusting its own last-known state.
- **The bound**: `DEFAULT_MAX_DIFF_INPUT_BYTES = 4 MiB` per side, `DiffPreviewPolicy`,
  `tekstide_core::project` — so RFC-020 does not introduce a second one.
- **The retention caveat and the `Debug` exposure**, both above under Known Limitations —
  RFC-020's rendering architecture should account for both, not assume either is closed.
- **RFC-020's own §Open questions item 4** (owned vs. lifetime-bound `DiffContent`) should
  be decided before or during whichever slice first calls `read_diff_content`.
