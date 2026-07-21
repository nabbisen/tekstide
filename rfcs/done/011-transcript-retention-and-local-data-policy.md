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
