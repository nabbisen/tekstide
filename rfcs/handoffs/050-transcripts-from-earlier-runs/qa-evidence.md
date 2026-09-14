---
title: "RFC-050 — QA evidence"
rfc: "RFC-050"
rfc_file: "../../accepted/050-transcripts-from-earlier-runs.md"
source_rfc_status: "Accepted 2026-09-13 — M12"
target_milestone: "M12"
created: "2026-09-13"
---

# Evidence

## PR-050-A — the lock, the id, the origin

`tekstide-core` only. **Nothing is loaded from disk, and nothing new is deletable**: `FoundOnDisk`
is constructed only in tests (grepped), and nothing walks `transcripts/`.

### The writer's lock, and the order it forces

`BoundedTranscriptWriter::create` now opens **without** truncating, takes an exclusive `try_lock`,
and only then truncates. **The order is the substance**: truncating on open would wipe a file
another writer still holds before discovering the lock — RFC-049 §2's deletion under a live writer,
by a different route. **Any** lock failure refuses the writer, not only `WouldBlock`: a filesystem
that cannot lock would otherwise produce writers every other process reads as dead.

**Truncation applies to regular files only** — exactly what `truncate(true)` on open used to affect.
`O_TRUNC` is ignored for a FIFO or a character device; `ftruncate` refuses them. My first version
truncated unconditionally and **turned the full gate red**: three reader tests capture into
`/dev/full` or a FIFO, and all three failed to launch with `OpenFileFailed`. The targeted run could
not have shown it. A test now pins that an unlocked leftover still starts empty
(`a_writer_on_an_unlocked_existing_file_starts_it_empty`), which the moved truncation otherwise had
no test for.

### A locked file degrades the launch; it does not refuse it

Before this slice any writer-creation failure refused the launch. A **lock** failure now starts the
process without capture and emits `TerminalRuntimeEvent::TranscriptWriterLockUnavailable`; every
other creation failure still refuses, unchanged. The session reads the event, **attaches no
`Transcript` record** — a record would aim this session's purge and retention at a file another
handle holds — and sets `AgentRun.transcript_absence = Some(TranscriptAbsence::WriterLockUnavailable)`.

**The reason is on the run, not a lifecycle state** (D4′ item 4): no record exists to hold one, and
neither `DisabledByOptOut` nor `CaptureFailed` is true of this case. `TranscriptAbsence` has one
variant; RFC-049 PR-049-C adds its budget case. **Rendering it in the run's detail is not here** — A
is core only.

### The origin type — proposed here, as the plan asks

```rust
pub enum TranscriptOrigin {
    LaunchedHere { terminal_id: TerminalId, agent_run_id: Option<AgentRunId> },
    FoundOnDisk { writer_held_lock_at_load: bool },
}
```

`Transcript.terminal_id` and `.agent_run_id` **moved into it** rather than staying beside it:
leaving them as fields would force a found record to carry *some* `TerminalId`, the fabrication D2
forbids. `terminal_id()` / `agent_run_id()` accessors replace the reads and use `if let`, so **the
only exhaustive match on the origin is the liveness predicate** — a new origin fails to compile there
and nowhere else.

`FoundOnDisk` carries only what the loader will measure. A lock held at the probe is live for the
session; a free one is not, because a file an earlier process left has no writer here and a new run
never reopens an old file.

### Findings about the RFC's own premises

- **`AgentRunId` already validated.** `impl_id!` gives every id type `from_persisted`, which strips
  the prefix and parses the rest as a UUID. The RFC measured it as missing — likely a search for
  `impl AgentRunId`, which a macro does not contain. The box needed a test, not code.
- **Run directories are `agent-run-<uuid>`, not bare UUIDs**, because the resolver uses the prefixed
  id. Risk document §1's *"both directory names must parse as UUIDs"* is exact for the project
  directory only; `from_persisted` handles the run directory correctly.
- **`from_persisted` does not enforce "only what this product writes".** A throwaway probe (run,
  then deleted) found both `ProjectId` and `AgentRunId` accept **uppercase, hyphen-less, braced and
  `urn:uuid:`** spellings, inherited from `uuid::Uuid::parse_str`. The product writes only lowercase
  hyphenated ids, so PR-050-B's loader would accept — and make deletable — names this product never
  wrote. Tightening it changes what persisted state parses, so it is not taken here. The test is
  named for what it proves (`agent_run_id_rejects_non_uuid_and_unprefixed_names`) and says so.
- **Minimum toolchain.** Neither crate declares `rust-version`; `File::try_lock` is stable from 1.89,
  so building either published crate now needs 1.89 or later.

### The lock made two fixture tests interact — and I first blamed load

**My new lock test's release assertion was racy.** It failed 1 run in 40 alongside the two
PTY-spawning launch tests, and 0 in 40 alone. Inferred mechanism: a concurrent fork copies the
writer's descriptor until exec, and `flock` lives until the last copy closes. The test now asserts
release within 5 s; a writer that never releases still fails it (ablation L11).

**The two `/dev/full` reader tests failed because of this change, and I first recorded it as load.**
`local_bounded_…_genuinely_unwritable` timed out while my own measurement loop ran, passed 5 of 5 in
isolation, and I wrote in the register that the load was mine and the change was innocent. **Both
halves were wrong.** The next ablation run showed both `/dev/full` tests failing inside ablations
that do not touch them, and under L4 (lock failure refuses the launch) the error named the cause:
`LockUnavailable` on a path symlinked to `/dev/full`.

Both tests symlink their transcript to **the same device inode**, and `RealProcessLimiter` admits six
real-process tests at once. Under the lock, the second of two overlapping tests correctly launches
without capture (D3) and then waits for a `CaptureFailed` that cannot come. **Fixed in the fixture**:
both hold a shared mutex for their whole duration. The overlap exists only because the fixture reuses
one device; a real run writes its own freshly named regular file, so the rule is not weakened for it.
**The register entry is corrected in place**, stating that the first attribution was wrong.

The lesson: passing in isolation shows a test is not broken on its own. It does not show the change
is innocent of how concurrent tests interact.

**An alternative for you, not taken:** lock only regular files, by the same reasoning as truncating
only regular files. That would avoid the interaction without the mutex, but it bends D3's *"a writer
never writes unlocked"*, which is a decided rule, so it is yours.

### An interrupted run, and how the tree was recovered

This table took three attempts. The first was stopped mid-ablation when the session ended, and the
machine rebooted, wiping the scratchpad backups. The second was stopped by me on purpose once it
exposed the `/dev/full` interaction above, because its results carried failures from that and not from
the ablations. Ablations restore and hash-check one at a time, so
only the last applied could remain: **L11's appended `Drop` impl was still in `writer.rs`**. The
other markers were checked absent. Removed by exact match against the text L11 appends — the edit
refuses if the tail differs.

**Stopping the second run killed my own restore script**: `pkill -f "050a-run2/run.sh"` matched its
own command line, which contained that string. The files were checked against the recorded baseline
hashes afterwards and already matched, and no ablation markers remained. The table below is the third
run, from a fresh baseline, with the mutex in place.

### Sequencing questions

- **The request-387 disclosure does not exist.** The README places RFC-050 after "the purge-defect
  disclosure commit and PR-DOC-C", and D8 removes the disclosure. Neither commit is in the log, and
  grepping `README.md`, the book and `CHANGELOG.md` finds no such text.
- **D8 and the plan put the removal in different slices.** D8: removed *"in the same commit that
  makes purge true: not before, not after."* Purge becomes true in **B**; the plan removes it in
  **C**.

### Ablations — the third run, from a fresh baseline, each file restored and hash-checked

| Ablation | Tests that fail |
| --- | --- |
| **L1** remove the writer's lock | the lock test's *held* assertion, the refused-writer test, the untouched-bytes test, the launch-degrade test |
| **L2** try the lock but ignore failure | refused-writer, untouched-bytes, launch-degrade |
| **L3** truncate on open | untouched-bytes, launch-degrade — the held file is wiped before the lock is tried |
| **L4** refuse the launch on a lock failure | launch-degrade **alone** |
| **L5** attach a record despite the lock failure | launch-degrade **alone** |
| **L6** treat every found-on-disk file as not live | found-and-locked test **alone** |
| **L7** add a `TranscriptOrigin` variant | **one compile error, `project/session.rs:1029`** — the liveness predicate, nowhere else |
| **L8** `from_persisted` stops parsing the UUID | the ids test (plus register row 1's socket flake, unrelated — see below) |
| **L9** skip the regular-file truncate | truncation test **alone** |
| **L10** truncate every file type | the FIFO test and both `/dev/full` tests — `ftruncate` refuses them, exactly the gate failure this slice first shipped |
| **L11** a writer whose lock outlives it | the lock test's *release* assertion, and the required-mode `/dev/full` test |

**L1 cannot fail "alone", and the checklist box says it should.** Every test that proves something
*about* the lock needs the lock to exist, so removing it fails all four. The lock test's own failure
is its first assertion, which is what the box asks for; the other three are a disclosed composition
rather than an entanglement, because each asserts a different consequence of the same fact.

**L11 hung, and that was a flaw in the ablation, not the product.** It leaks the writer's whole
descriptor on drop, because a lock cannot be leaked without the descriptor that holds it. That also
holds the FIFO test's write end open, so its reader waited more than ten minutes for an end-of-file
that could not come; the test binary was killed by PID. This hang is also the likeliest reason two
earlier background runs lost their wall-clock time before their sessions ended.

**L8's second failure is not L8.** `approval::tests::channel::bind_recovers_from_a_stale_socket_file`
is register row 1, a stale-socket bind that uses no persisted id. Another project's release build,
wasm build and test run were loading the machine throughout. Recorded as a dated recurrence.

### Greps the checklist asks for

```
exhaustive matches on TranscriptOrigin:   session.rs (the predicate) — accessors use `if let`
FoundOnDisk constructed:                  project/tests/retention.rs only
ids minted beside a transcript record:    none outside TerminalSession::new / AgentRun::draft
directory walks under transcripts/:       none
reads of the removed fields:              none
```

### Gate

`cargo fmt --all --check`, `clippy --workspace --all-targets -D warnings`: clean. `git diff --check`:
clean after removing a trailing blank line my register edit left. **Three consecutive full-workspace
runs, output redirected to files: 507 + 6 + 791, green every time** (+8 core tests). Load rose from
6.3 to 9.2 across them, with another project's builds running.
