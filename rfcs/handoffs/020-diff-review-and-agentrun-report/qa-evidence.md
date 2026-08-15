---
title: "RFC-020: Diff Review and AgentRun Report Surfaces - QA Evidence"
rfc: "RFC-020"
rfc_file: "../../proposed/020-diff-review-and-agentrun-report.md"
status: "PR-020-B's core (transcript reader) implemented 2026-08-15, not yet reviewed — surface not started"
target_milestone: "M10"
created: "2026-08-15"
---

# QA Evidence

Record results here as each slice lands, with the reasoning that produced them.

**This file holds results. The obligations live in
[`task-breakdown-pr-plan.md`](./task-breakdown-pr-plan.md).** Four obligations in this
project have been lost to that gap — an item recorded only in an evidence file is an item
the next implementer does not read. If a slice discovers a new obligation, put it in the
task breakdown, not only here.

## Conventions

- **Ablation**: break the property, watch the *specific* named test fail, restore. One
  ablation per property. **A green ablation is a defect in the ablation, not a pass.**
- **Positive control**: prove the check reaches real data before asserting what it does
  not find.
- **GUI evidence**: `niri msg action screenshot-window`; synthetic input with
  `env -u WAYLAND_DISPLAY`, `xdotool windowfocus` (not `windowactivate`), always
  `--clearmodifiers`. Compare captures at one window geometry — comparing across
  geometries, or across different *screens*, has produced wrong claims here twice.
- State what each piece of evidence **does not** prove, alongside what it does.

## PR-020-A — Design and handoff acceptance

Granted 2026-08-12 with the pack. RFC-020's four open questions answered in the pack's
README: Option B (owner's decision), no second bound (RFC-024's measured 4 MiB stands),
read-only, and `DiffContent` left owned with its limitation carried forward accurately.

## PR-020-B — The transcript reader, and the AgentRun report surface

**Core (the reader): implemented 2026-08-15, not yet reviewed.** Surface (the AgentRun
report widget): not started — the reader alone is not this slice's own completion (see
`task-breakdown-pr-plan.md`'s own framing, "a reader with no consumer cannot be shown to be
correct"), recorded here as a checkpoint, not a claim of PR-020-B being done.

**A real, pre-existing panic found and fixed first, in a different RFC's own module.**
Building D2's resynchronization proof required calling `TerminalSecurityParser::parse`
(RFC-017) on a buffer deliberately truncated mid-CSI-sequence — a shape no existing caller
had ever produced, since `parse` currently has zero production call sites in this crate
(confirmed by grep before touching anything). This reproducibly panicked:
`parse_csi`'s `body = &sequence[2..sequence.len().saturating_sub(1)]` underflows when
`take_until_csi_final`'s fallback path (no real CSI final byte found within the scan
window) returns a slice shorter than 3 bytes — reachable with as little as a bare `ESC [`
at a buffer's own end. The existing guard, `let Some(final_byte) = sequence.last().copied()`,
could never catch this: a non-empty slice always has a last byte, so that branch had never
actually triggered for any input, well-formed or not. A first fix attempt (check the last
byte's *value* is in the CSI final-byte range) was also insufficient and caught by this
slice's own tests before committing: `[` (0x5b) is itself inside `0x40..=0x7e`, so a 2-byte
`ESC [` fallback still passed the check and still panicked. The real fix:
`take_until_csi_final` now returns an explicit `found_final_byte` signal instead of leaving
the caller to infer it from the returned bytes. Ablated for real (reverting the fix
reproduces the identical panic message). Committed separately (`c229781`) from the reader
itself (`1c7b980`), since it is a standalone defect in a different, already-shipped RFC's
module, not part of this RFC's own deliverable — flagged prominently here rather than
folded quietly into the reader's own commit.

**D1 — the window size, measured, not estimated.** A real Rust harness (`rustc -O`,
`/proc/self/status` `VmRSS` deltas around a realistic PTY-output-shaped buffer — repeated
SGR-styled lines, not a pathological all-zero buffer) swept 1/2/4/8 MiB candidates:

```text
mib=1 window_len=1048576  escaped_len=1572864  rss_delta_kb=2572
mib=2 window_len=2097152  escaped_len=3145728  rss_delta_kb=7172
mib=4 window_len=4194304  escaped_len=6291456  rss_delta_kb=12296
mib=8 window_len=8388608  escaped_len=12582912 rss_delta_kb=24584
```

**1 MiB chosen**: ~2.6 MiB real transient RSS (trivial), 1/32nd of the retention ceiling —
meaningfully a window, not "basically the whole transcript." Unlike RFC-024's bound, not
reused from an existing standard by analogy (a transcript tail is not shaped like a whole
edited file or a single paste) — a fresh, measured number.

**D2 — resynchronization, proven against real captured PTY output, not a synthesised
fixture.** `a_window_starting_inside_a_real_control_sequence_classifies_identically_to_the_whole`
spawns a real shell via the same `LinuxTerminalRuntime` harness `runtime::terminal::tests`
already uses, runs a real `printf` that emits a genuine SGR escape sequence, and captures
the raw PTY bytes. Phrased as a splitting invariant (`TerminalSecurityParser::parse` does
not expose per-effect byte offsets, so this avoids needing to reconstruct them): splitting
the real captured bytes at the *resynchronized* boundary and parsing each half separately
equals parsing the whole buffer in one call; splitting at the *raw, non-resynchronized*
offset does not — both checked against the identical fixture, so the property that broke
and the property that holds are demonstrated against the same real bytes, not two
different ones. **Ablated** (`ablation_without_resynchronization_the_split_misclassifies`):
skipping the resynchronize call and splitting at the raw offset reproduces the divergence
directly. The delivered start offset (`TranscriptWindow::delivered_start()`) is reported
distinctly from the requested one (`requested_start()`) in the type itself.

**No UTF-8 scalar split**, proven with a real 2-byte scalar (`é`) and a target offset
landing on its second byte — `resynchronization_never_splits_a_utf8_scalar`.

**D3 — raw bytes survive**, proven against the same bidi/format-character probe
`text_safety`'s own tests use (`raw_bytes_survive_the_reader_including_bidi_and_format_characters`)
— the reader never calls `quote_untrusted`.

**D4 — read-only, by enumeration.** `only_this_module_opens_a_transcript_file_for_reading`
scans `tekstide-core` for `transcript_file()` combined with a raw byte-open, against a
closed one-entry allowlist (`transcript/reader.rs` itself; `transcript/writer.rs` is
excluded by name, since it opens the file for *writing*). RFC-024's own, broader
enumeration test (`project::diff::tests`) updated to disclose this module's one call site
too, since its own scan is now broad enough to also catch transcript reads.

**D5 — complete vs. still-being-written, in the type.** `TranscriptWindow::Complete`/
`::StillBeingWritten` are separate constructors (matching `DiffContent`'s own precedent),
selected by a caller-supplied flag — nothing on disk distinguishes a live process paused
between writes from a finished transcript, so this cannot be inferred from the file alone
and is not guessed at. Proven by `still_being_written_threads_into_the_returned_variant`.

**Path safety reused, not duplicated.** `TranscriptStoragePath::is_safe_for_read` delegates
to the existing `is_safe_for_write` containment check (identical logic, a name that does
not misdescribe why read-only code calls it) rather than either calling a write-named
method from read code or duplicating the same four `path_contains` calls under a second
name that could drift from the first. `an_unsafe_storage_path_is_refused_before_any_read`
proves the refusal happens before any file I/O.

**Gates**: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets
--all-features -- -D warnings`, full workspace suite (`tekstide-core` 546 passed, up from
531 — 15 new tests across the panic fix and the reader; `tekstide` 206 passed, unchanged —
no `crates/tekstide` changes, matching "core first" sequencing), `git diff --check`. All
clean.

**Not done in this checkpoint**: the AgentRun report surface itself (the widget, the
escaping at the point of rendering, the reader-window-vs-writer-truncation rendered
distinction, the no-double-escaping proof) — all deferred to the next round of this slice.
Nothing here claims PR-020-B complete.

## PR-020-C — The change review surface

*Not started.*

## PR-020-D — Closeout

*Not started.*

## Known limitations, consolidated

To be filled at closeout. The ones already known going in, which must survive into the
closeout rather than being rediscovered:

- **No two-sided diff for a modified file.** The before-bytes were never captured
  (`ReviewBaselineEntry` is metadata-only by RFC-012 §Design Principles 2) and are gone,
  not merely unretained, by preview time.
- **Detection is metadata-only and conservative**; the change set may be incomplete.
- **`DiffContent` blocks two specific storage paths**, not general retention — a consumer
  can destructure it and keep the bytes.
- **The transcript window is a view, not the whole transcript**, and is distinct from the
  writer's retention truncation.
- **No Git-backed before-source exists**; it is gated behind RFC-012's unmet safety
  evidence.
