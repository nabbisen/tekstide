# RFC-011: Transcript Retention and Local Data Policy

Status: Implemented with documented limitations
Target milestone: M6
Date: 2026-07-21

Related baseline documents:

- `tekstide-requirements-v0.md`
- `tekstide-external-design-v0.md`
- `tekstide-security-threat-model-v0.md`
- `tekstide-roadmap-milestones-v0.md`
- [`ROADMAP.md`](../../ROADMAP.md)

Depends on:

- [RFC-002](../done/002-core-domain-model-projectsession-terminalsession-agentrun-auditevent.md)
- [RFC-006](../done/006-projectsession-state-and-file-explorer-editor-basics.md)
- [RFC-008](../done/008-terminalsession-process-lifecycle.md)
- [RFC-009](../done/009-terminal-security-boundary.md)
- [RFC-010](../done/010-agentrun-launch-model-and-ai-cli-profiles.md)

Blocks:

- bounded transcript capture for Tekstide-created AgentRuns;
- per-run transcript opt-out;
- transcript purge controls;
- generated-work review models that depend on retained AgentRun output;
- later durable audit references to transcript metadata;
- later GUI transcript/review surfaces.

## Summary

RFC-011 defines the first policy and model for retaining AgentRun transcript bytes locally. RFC-010 intentionally kept transcript behavior metadata-only. This RFC changes that boundary only when Tekstide can prove that transcript capture is local-only, bounded, purgeable, opt-out capable, and stored outside project roots.

The design builds on the existing `Transcript` metadata model and `TranscriptPrivacyPolicy` vocabulary. It does not introduce durable audit storage, final GUI transcript panes, generated-change review UI, command approval, provider cloud integration, or secret redaction guarantees.

## Motivation

AgentRun launch is useful only if users can inspect what happened. Without retained transcript bytes, Tekstide cannot support credible generated-work review, troubleshooting, or later audit references. At the same time, terminal output and AI CLI transcripts are private by default: they may include prompts, file paths, tool output, credentials printed by external programs, copied shell history, or project contents.

The transcript policy must therefore answer:

- when transcript bytes are captured;
- where they are stored;
- how large and how old they may become;
- how a user opts out before launch;
- how transcript bytes are purged;
- what summaries may expose without leaking transcript contents;
- which later features may reference transcripts without claiming more than this RFC implements.

## Goals

- Allow bounded local transcript byte capture for Tekstide-created AgentRuns.
- Keep transcript capture disabled for plain terminal sessions unless a later reviewed feature explicitly opts in.
- Preserve per-run opt-out before process start.
- Store transcript bytes under Tekstide-managed local state, never inside the project root.
- Reject transcript byte capture when the storage path cannot be proven inside the Tekstide state root.
- Enforce retention by per-transcript, per-project, and app-wide aggregate byte limits plus age limits.
- Expose local-data accounting for total retained transcript bytes.
- Support explicit purge for one transcript, one AgentRun, and one ProjectSession.
- Keep transcript summaries metadata-only: ids, sizes, timestamps, retention state, and bounded errors, not transcript content.
- Treat stored transcript bytes as untrusted terminal output.
- Keep search indexing disabled unless a later reviewed policy enables it.
- Provide implementation slices and QA evidence requirements for M6.

## Non-Goals

- Durable audit persistence, audit migrations, or audit database schema.
- Final GUI transcript panes, review panes, purge dialogs, or settings screens.
- Generated-change diff detection beyond optional metadata references.
- General command approval or managed adapter command interception.
- Provider-specific AI API integration.
- Cloud sync, remote upload, telemetry, or shared transcript storage.
- Secret detection, content redaction, or a claim that retained transcripts are sanitized.
- Retaining arbitrary plain shell scrollback by default.
- Retaining full prompt text outside transcript bytes when the prompt is not emitted to the terminal stream.
- Cross-device retention policy.

## Design Principles

1. **Local first, local only.** RFC-011 transcript bytes are local application data. They are not project files, telemetry, cloud sync data, or provider data.
2. **Bounded or absent.** Transcript byte persistence is allowed only with an effective nonzero size or age bound and purge support.
3. **Opt-out before launch.** The user or caller must be able to disable transcript capture for an AgentRun before process start.
4. **Paths are policy.** A transcript path outside Tekstide state or inside the project root is a launch-time policy failure.
5. **Content stays untrusted.** Transcript bytes may contain terminal escape sequences and private text. Renderers and summaries must treat them as untrusted content.
6. **No redaction overclaim.** Metadata may be structured and bounded, but transcript bytes are not promised to be redacted.

## Transcript Policy Model

RFC-011 extends the current `TranscriptPrivacyPolicy` direction into a concrete launch and storage model.

Core policy concepts:

```text
TranscriptCapturePolicy
├─ storage_policy
├─ retention_limit
├─ default_capture
├─ per_run_opt_out
├─ purge_support
├─ search_indexing
└─ redaction_claim
```

Required initial values:

- `storage_policy`: local-only application state;
- `retention_limit`: bounded by size and age;
- `default_capture`: enabled for Tekstide-created AgentRuns when storage preflight passes;
- `per_run_opt_out`: available before launch;
- `purge_support`: required before byte persistence is permitted;
- `search_indexing`: disabled;
- `redaction_claim`: structured metadata only.

The existing metadata-only policy remains valid for:

- AgentRuns where the user opts out;
- launch contexts where transcript storage preflight fails;
- plain terminal sessions;
- tests or harnesses that intentionally validate no byte persistence.

## Default Retention

The initial M6 default is:

- maximum 32 MiB per transcript;
- maximum 256 MiB retained transcript bytes per project;
- maximum 1 GiB retained transcript bytes app-wide;
- maximum 30 days retained;
- truncation allowed after the byte limit;
- age-based expiration processed by explicit cleanup/harness paths first, not background automation hidden from the user.

The implementation may make these constants configurable later, but RFC-011 acceptance does not require a settings UI.

Retention state should distinguish at least:

- active and complete;
- active and truncated by byte limit;
- expired by age policy;
- purged by user or policy;
- capture disabled by opt-out;
- capture failed before process start.

Aggregate retention accounting must include at least:

- current retained transcript bytes for one ProjectSession;
- current retained transcript bytes across the Tekstide app state root;
- transcript count per ProjectSession;
- whether a per-transcript, per-project, or app-wide budget caused truncation, expiration, or purge.

When a project or app-wide budget is exceeded, cleanup should process inactive transcripts first, oldest first by retention metadata. Running transcript writers must not be silently deleted underneath active AgentRuns. If no inactive transcript bytes can be removed and the budget is exhausted, `LocalBounded` capture should truncate or disable further writes with metadata, while `RequiredLocalBounded` should reject launch or fail preflight before process start.

## Storage Path Policy

Transcript bytes must be stored under a Tekstide-managed local state root, not under the project root. A conventional shape is:

```text
<tekstide-state-root>/
  transcripts/
    <project-id>/
      <agent-run-id>/
        transcript.log
```

The exact path may differ, but the storage resolver must prove:

- the state root is absolute;
- the transcript path canonicalizes under the state root when parent directories exist;
- path components derived from project or AgentRun identifiers are sanitized;
- the transcript path is not inside the canonical project root;
- workspace symlinks cannot redirect transcript storage into the project root or another unreviewed location;
- transcript metadata records only the local path or a stable local reference, not transcript contents.

If the path cannot be proven safe before launch, transcript byte capture must be disabled or the launch must be rejected according to the selected capture mode. Tekstide must not silently write transcript bytes into the project tree.

## Capture Modes

RFC-011 introduces three launch-level capture modes:

| Mode | Behavior |
| --- | --- |
| Disabled | No transcript bytes are written. Metadata may record that capture was disabled. |
| LocalBounded | Capture is enabled only if local path and retention preflight succeed before process start. |
| RequiredLocalBounded | Launch is rejected if local bounded transcript capture cannot be prepared. |

The initial AgentRun default should be `LocalBounded` when the global policy permits byte persistence. Per-run opt-out sets the mode to `Disabled`.

If a future UI or profile requires a transcript for a review workflow, it may request `RequiredLocalBounded`, but that claim needs implementation evidence.

## Capture Source and Content Boundary

Transcript capture records bytes from Tekstide-owned AgentRun terminal output after process start. It does not capture:

- plain terminal sessions by default;
- terminal scrollback from sessions not created as AgentRuns;
- environment values or launch diagnostics outside the terminal stream;
- full prompt text unless it appears in the terminal stream;
- files read from the project unless an external process prints them.

Captured transcript bytes remain untrusted. Stored bytes may include ANSI/VT sequences and private output. Any renderer or review model must sanitize or parse them through the terminal security boundary from RFC-009 before display. Metadata summaries must not include raw transcript snippets.

## AgentRun Integration

RFC-011 updates the RFC-010 launch path:

1. Resolve profile and project.
2. Apply active-file safety checks as RFC-010 requires.
3. Resolve transcript capture mode from profile/default/user opt-out.
4. Preflight transcript policy, state root, path, retention, and purge support.
5. Reject or disable capture according to the selected mode.
6. Create transcript metadata before process start when capture is enabled.
7. Attach transcript metadata to the AgentRun and TerminalSession after successful runtime launch.
8. Stream AgentRun terminal output into the bounded transcript writer.
9. Update byte count, truncation state, and last-write timestamp.
10. On terminal exit/failure/cancellation/detach, finalize transcript metadata without treating transcript state as process truth.

AgentRun lifecycle still follows TerminalSession/runtime observations. Transcript capture must not become a second process supervisor.

## Purge Semantics

Purge must be explicit, idempotent, and local.

Required purge scopes:

- one transcript id;
- one AgentRun id;
- one ProjectSession id.

Purge behavior:

- delete transcript byte files when present;
- update transcript metadata to a purged state;
- preserve a content-free tombstone transcript reference by default;
- clear AgentRun/TerminalSession transcript references only when the storage model cannot preserve a content-free tombstone without retaining sensitive path, content, or environment metadata;
- return bounded errors without transcript content;
- succeed when bytes are already absent but metadata is present;
- never delete project files.

RFC-011 may emit in-memory audit events or audit metadata if existing models require it, but durable audit persistence remains RFC-013.

## Privacy and Security Rules

- Transcript summaries may include ids, project id, AgentRun id, terminal id, byte count, truncation/purge state, timestamps, retention policy name, and bounded error categories.
- Transcript summaries must not include raw prompt text, terminal output, file contents, environment values, or shell history.
- Search indexing remains disabled.
- Redaction claims are limited to structured metadata. Transcript bytes are retained as-is.
- Restricted Mode does not prohibit local Tekstide transcript storage by itself, but it still prohibits workspace-local profile/prompt/env/executable loading as defined by RFC-010.
- Transcript files are local sensitive data. The design does not claim OS encryption or secure deletion.
- Any later GUI must visually separate transcript content from trusted approval/security UI.

## Data Model Impact

Expected model additions or refinements:

- transcript capture mode;
- bounded retention policy;
- aggregate retention accounting;
- local transcript path resolver;
- transcript retention state;
- transcript purge result;
- transcript writer summary;
- AgentRun launch transcript preference;
- ProjectSession transcript purge operations.

Existing `Transcript` metadata can remain the root entity if it grows enough state to represent active, truncated, disabled, failed, and purged outcomes.

## Implementation Plan

Recommended slices:

1. **PR-011-A: Transcript policy and path model.**
   Add capture modes, retention policy, aggregate accounting model, local path resolver, retention states, and tests proving no project-root paths are accepted.
2. **PR-011-B: Bounded transcript writer.**
   Add an append-only local writer/harness with byte limits, truncation state, metadata updates, and no search indexing.
3. **PR-011-C: AgentRun launch integration.**
   Wire transcript preflight and opt-out into RFC-010 launch validation, attach transcript metadata, and stream AgentRun terminal output into the writer.
4. **PR-011-D: Purge and local data summaries.**
   Add transcript/AgentRun/project purge operations, tombstone behavior, and metadata-only local data summaries with aggregate byte counts.
5. **PR-011-E: Closeout evidence.**
   Update checklist, QA evidence, known limitations, and RFC lifecycle state after review acceptance.

## Test and Evidence Requirements

- Policy tests proving byte persistence is rejected without bounded retention, opt-out, and purge support.
- Path tests proving transcript storage stays under Tekstide state and outside project roots, including symlink and traversal cases where practical.
- Launch tests proving opt-out writes no bytes and records only metadata.
- Launch tests proving enabled capture creates transcript metadata before process start and attaches it to the AgentRun/TerminalSession after successful launch.
- Failure tests proving unsafe transcript paths reject or disable capture according to mode.
- Writer tests proving byte limits truncate and update metadata.
- Aggregate retention tests proving project/app byte budgets are accounted and cleanup ordering is deterministic.
- Purge tests proving transcript, AgentRun, and project purge scopes remove bytes and leave bounded metadata.
- Tombstone tests proving purged transcripts remain referentially visible by default without content/path leakage.
- Privacy tests proving summaries do not include transcript snippets, prompt text, environment values, or terminal output.
- Regression tests proving plain terminal sessions do not retain transcript bytes by default.

## Acceptance Criteria

- AgentRun transcript byte capture is local-only, bounded, purgeable, and opt-out capable.
- Transcript storage paths are outside project roots and under Tekstide-managed state.
- Retention is bounded per transcript, per project, and app-wide, with metadata-only accounting of total retained transcript bytes.
- Transcript metadata records byte count, retention state, timestamps, and local reference without storing content in summaries.
- Capture integrates with AgentRun launch without weakening RFC-010 launch validation or lifecycle truth.
- Purge works for transcript, AgentRun, and ProjectSession scopes.
- Purge preserves content-free tombstone references by default.
- Search indexing remains disabled.
- Redaction claims are limited to structured metadata.
- Tests and QA evidence cover policy, paths, writer bounds, launch integration, purge, privacy, and plain-terminal non-capture.

## Risks

- **Transcript data can be sensitive.** Keep default retention bounded, provide opt-out, keep summaries content-free, and avoid redaction claims.
- **Path mistakes can write into projects.** Treat path resolution as a security boundary and test traversal/symlink cases.
- **Terminal output is untrusted.** Stored transcript bytes must not be rendered as trusted UI or approval text.
- **Capture can distort lifecycle logic.** Keep TerminalSession/runtime as process truth; transcript writer state is storage truth only.
- **Scope can expand into audit/UI/review.** Keep durable audit, rendered review surfaces, and generated-change workflows in later RFCs unless separately reviewed.

## Resolved Review Decisions

- `LocalBounded` remains the initial default for Tekstide-created AgentRuns, provided PR-011-C proves caller-visible opt-out before process start.
- `RequiredLocalBounded` remains model vocabulary but must not become the default or be used by an unreviewed workflow.
- Purge preserves content-free tombstone transcript metadata and AgentRun/TerminalSession references by default.
- Initial retention defaults are 32 MiB per transcript, 256 MiB per project, 1 GiB app-wide, and 30 days retained.

## Amendment 1: Bounded Transcript Reader

**Status:** Decided by the architect as design authority 2026-08-12, under the owner's
standing delegation (recorded in `delivery-plan.md` §Owner decisions, 2026-08-11).
Flagged to the owner rather than escalated, because it meets the delegation's test:
**additive and invariant-preserving, with no migration, no retention change, and no
breaking removal.** Escalate if any implementation pressure would change that.

**Amendment type:** Additive — a read path over data this RFC already governs.

**Why an amendment and not a new RFC.** RFC-011 has already decided capture mode,
retention limits, budget scope, purge semantics, and the untrusted-content boundary. A
bounded reader is "read back what policy already governs," and the constraints to review
it against exist here. RFC-020 §Sequencing reached the same conclusion, and contrasted it
with the diff *content* model — correctly **not** an amendment, because that was new state
and new I/O. This is neither.

### What is authorised

A read-only, bounded reader in `tekstide-core`'s `transcript/` module, which today has
`path.rs`, `policy.rs` and `writer.rs` and **no reader at all**.

### D1 — A bounded window, not a refusal, and not the writer's truncation

RFC-024 chose **refuse, never truncate** for diff content. **This amendment deliberately
chooses differently, and the difference must not be read as inconsistency.**

A truncated diff misleads about what changed — a reviewer acts on it and approves
something they did not see. A transcript is an append-only log, consumed newest-first, and
refusing to show a 32 MiB transcript would withhold the feature exactly when a
long-running agent makes it most valuable. Truncating a diff removes information the user
is deciding on; windowing a log removes history they can still ask for.

So: **the reader returns a bounded window over the tail**, because the end of a log is
what a reader wants first.

**The window is not the writer's truncation and must never be conflated with it.** RFC-011
already lets the *writer* truncate at the byte limit and record that in retention metadata
(`active and truncated by byte limit`). That is a permanent fact about the file. A reader
window is a transient fact about one request. A surface that shows "truncated" without
saying which one is lying to the user about whether bytes still exist.

**The window size is measured, not estimated.** Two estimated figures in this project were
wrong once measured. Measure against the real retention ceiling (32 MiB per transcript),
not a comfortable sample.

### D2 — The window boundary is outside the property the filter was proven against

**This is the security-critical decision in this amendment, and it is easy to miss.**

RFC-017 PR-017-B/C established **P4 (stream-position independence)**: the terminal filter
classifies identically regardless of how the byte stream is chunked. **P4 covers chunking
where every byte arrives.** A tail window does not chunk — it **drops the prefix**. The
first byte of the window can land in the middle of a CSI or OSC sequence, and the filter
was never proven correct for a stream that begins mid-sequence, because no code path could
previously produce one.

A window that starts mid-sequence can therefore present the leading fragment of a control
sequence as ordinary text, or swallow following text into an unterminated sequence. That
is a classification difference produced by *where the read started* — precisely what P4
exists to deny, arrived at through a door P4 does not cover.

**Required:** the reader must **resynchronize** — advance from the raw window start to the
first position where a fresh parse is sound, and report the delivered start offset rather
than the requested one. It must additionally not split a UTF-8 scalar at either edge.

**Evidence owed:** a test that a window starting inside a control sequence classifies
identically to the same content read whole — and an ablation removing the
resynchronization that shows the *specific* divergence, with the exact wrong value. A
green ablation here is a defect in the ablation.

### D3 — Raw bytes out; escaping belongs to the surface

This RFC already says captured bytes remain untrusted and that any renderer must take them
through RFC-009's boundary before display. The reader **must not pre-escape**.

The reasoning is RFC-024 PR-024-C's and it held there: a model that escapes hides content
from every non-rendering consumer, and makes "what is actually in the file" unanswerable.
Per the escaping asymmetry, a transcript is **reviewed, not edited** — so it renders
escaped, and RFC-020 owns that at the widget.

**Evidence owed:** raw bytes survive the reader unaltered, proven against the same bidi
probe `text_safety`'s own tests use.

### D4 — Read-only, enforced by enumeration

The reader must not delete, expire, purge, rewrite, or update retention metadata, and must
not become a second retention policy. RFC-020 §Risks names this risk explicitly.

**Evidence owed:** an enumeration test naming every production call site that opens a
transcript for reading, and proof that no reader path reaches a mutating call. A new call
site must fail the test by name.

### D5 — An actively-written transcript is readable, and says so

An AgentRun may still be running. The reader may observe a partial trailing write; it must
not present that as a completed transcript, and must not block the writer.

**Required:** the returned value distinguishes *complete* from *still being written*, in
the type rather than in a doc comment — the same instruction RFC-024 PR-024-C was given
for the not-a-diff case, and which it satisfied with separate constructors.

### What this does not decide

- **Rendering.** No widget, no layout, no words a user reads. RFC-020 owns all of it.
- **Search, filtering, or navigation within a transcript.** Not in this amendment.
- **Making retention configurable.** RFC-011 left that open; it stays open.
- **Any change to what is captured.** The capture boundary above is untouched.

## Amendment 2: Re-homing transcript capture onto the readiness-driven reader

**Status:** Authored by the architect 2026-08-15, **authorised by the human owner the same
day**, including D3's failure policy as proposed. It changes what happens to a running
process when capture fails, which is retention semantics rather than an additive accessor —
which is why it went to the owner rather than being decided under the standing delegation.

**Handoff:** [`../handoffs/011-amendment-2-transcript-capture-rehoming/README.md`](../handoffs/011-amendment-2-transcript-capture-rehoming/README.md)

**Amendment type:** Structural. Touches RFC-017 Amendment 1's `TerminalReader` and
RFC-008's session ownership.

### Why this exists

RFC-017 Amendment 1 replaced the terminal's read path. The old one,
`LinuxTerminalRuntime::read_available_bounded_for`, did two unrelated things in one loop:
it returned a bounded buffer to its caller, **and it appended every byte to
`session.transcript_writer` and flushed it**. Those are the only non-test writes to a
`BoundedTranscriptWriter` in the workspace.

The new `TerminalReader` does not write transcripts at all. Nothing failed, because no
production code creates an `AgentRun`, so no transcript writer is ever configured.

**Adapter-spawn — the work that makes `AgentRun`s real — is blocked on this.** Whoever
builds it will wire output through `TerminalReader`, because that is the reviewed path, and
this RFC's entire retention design plus Amendment 1's bounded reader would then operate on
files nothing ever writes.

### D1 — The writer moves into the reader thread

Not to the consumer. `TerminalPane::poll()` runs on the UI thread, and file I/O there is
the precise defect RFC-017 Amendment 1 was written to remove; re-introducing it as a
transcript write would undo that work under a different name.

The reader thread already blocks on `poll(2)`. It is the correct place for a blocking
write, and moving the write there is strictly better than the old design, which wrote
synchronously on the update thread.

**Consequence to design deliberately, not discover:** `session.transcript_writer` is
currently owned by the runtime and read by `transcript_write_summary`. Once the writer
lives in the thread, that accessor has no writer to consult. Return the summary through a
shared, lock-protected snapshot the thread updates, or through the reader's own API — but
**decide it, and say which**, rather than leaving `transcript_write_summary` returning
`None` and looking like "no transcript configured."

### D2 — Write before send

The transcript write happens **before** the bytes enter the channel, never after.

This gives one stated invariant: **the transcript is a superset of what was displayed.** A
crash between write and send loses nothing from the record; a crash between send and write
would leave the user having seen output the durable record does not contain, which defeats
the point of having a record.

### D3 — Mid-stream write failure, by capture mode

The old path returned `TerminalRuntimeError::TranscriptWrite` and failed the whole read. A
thread has no caller to return to, so this needs a real policy.

This RFC already decides the analogous budget-exhaustion case: `LocalBounded` truncates or
disables further writes with metadata; `RequiredLocalBounded` rejects launch or fails
preflight **before process start**. Mid-stream is different — the process is already
running, so "reject launch" is not available.

**Proposed, and the part most needing the owner's judgement:**

- **`LocalBounded`** — mark the transcript `CaptureFailed` (the state already exists,
  `domain/transcript.rs:93`), stop writing, **keep reading**. The terminal stays usable and
  the user is told the record stopped. Capture is best-effort in this mode by definition.
- **`RequiredLocalBounded`** — mark `CaptureFailed` and **stop reading**. Do not kill the
  child. Ceasing to drain applies Amendment 1's backpressure, so the process blocks on
  `write()` and makes no further unrecorded progress, while termination stays the caller's
  decision rather than a reader thread's.

The second is the interesting one: it uses backpressure as a *safety* mechanism rather than
a performance one, and it stops the run without the reader unilaterally killing anything. A
mode whose name says the record is required should not continue producing unrecorded work.

**Either way the failure must be observable.** A silent `CaptureFailed` is the same defect
class as the old code discarding the summary that carried `dropped_bytes`.

### D4 — Backpressure now includes the disk

Stated because it is a real behaviour change and will otherwise be discovered as a bug: a
slow or stalled disk now stalls the reader, which stalls the child. That follows directly
from D1 and D2 and is correct — but it means terminal liveness depends on transcript write
latency whenever capture is on.

The old design had the same coupling and worse, on the UI thread. **Do not "fix" this by
writing after the send or on a separate unbounded queue** — the first breaks D2's
invariant, the second reintroduces unbounded buffering that RFC-017 Amendment 1's D1
rejected.

### What must not change

- **P1/P2 as re-proven by RFC-017 Amendment 1 PR-A1-B.** The writer is a new consumer of
  the byte stream inside the reader thread; it must not become a second path *out* of it.
  Re-run those enumerations rather than assuming they transfer.
- **Retention limits, capture modes, budget scope, purge semantics.** All decided in this
  RFC's body and untouched here. This amendment moves *where* capture happens, not *what*
  is captured or *how long* it is kept.
- **Amendment 1's reader contract** — bounded window, resynchronization, read-only.

### Evidence owed

- A transcript written through the new path, byte-identical to the PTY output, proven
  against a real child process rather than a synthesised stream.
- **D2 proven by ordering**, not asserted: a test that observes the record contains bytes
  the consumer has not yet drained.
- Mid-stream failure exercised for real in both modes — a genuinely unwritable transcript
  (permissions, or a full filesystem), not an injected error value.
- `RequiredLocalBounded`'s stop-reading behaviour shown to stall the child rather than kill
  it, and shown to leave termination available to the caller.
- The `transcript_write_summary` decision from D1, with its mechanism named.
- Ablations: remove the write-before-send ordering and show the specific failure; remove
  the `CaptureFailed` marking and show what silently succeeds.

### Out of scope

- Any change to what is captured, retention limits, or purge.
- The transcript *reader* (Amendment 1) — unchanged.
- Adapter-spawn itself. This unblocks it; it is not it.
