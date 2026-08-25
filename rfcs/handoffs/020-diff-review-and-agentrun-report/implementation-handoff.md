---
title: "RFC-020: Diff Review and AgentRun Report Surfaces — implementation handoff"
rfc: "RFC-020"
rfc_file: "../../done/020-diff-review-and-agentrun-report.md"
status: "Ready for implementation"
created: "2026-08-15"
---

# How to build it

Read [`the-window-boundary.md`](./the-window-boundary.md) first. Everything below assumes it.

## What already exists, and what does not

**Exists, reviewed, shipped:**

- `tekstide_core::project::DiffContent` and its gate (`project/diff.rs`) — RFC-024,
  in `0.7.0`. Ordering is fixed and proven: `PathNotDetected` → `resolve_existing` →
  metadata size check → `TooLarge` → `sniff_is_binary` → content read.
- `ChangeLifecycle { Added, Modified, Deleted }` on `DetectedChangedPath` — RFC-012
  Amendment 1.
- The transcript **writer**, path policy, and retention policy (`transcript/`).

**Does not exist:**

- **The transcript reader.** `transcript/` has `path.rs`, `policy.rs`, `writer.rs` and no
  reader. RFC-011 Amendment 1 authorises one; PR-020-B builds it.
- Any rendered surface for either.

## The change review surface

### Render per change kind, from `ChangeLifecycle` — never from `ChangePathKind`

RFC-012 Amendment 1 exists precisely so this distinction is readable. `ChangePathKind` is
`{ File, Directory, Symlink, Other }` — *what a path is*, not *what happened to it*.

| Lifecycle | What to render |
| --- | --- |
| `Added` | Full content — the whole change |
| `Deleted` | The fact of deletion, from metadata. Nothing to read |
| `Modified` | **Current content, labelled as not a diff** |

**The `Modified` case is the common one and the one that can mislead.** RFC-024's own
correction is definitive: no before-bytes were ever captured, so no two-sided diff is
possible under filesystem-snapshot detection. RFC-024 made the distinction representable in
the type; **this slice owns the words a user reads**.

Those words must not imply a comparison. A heading saying "Changes" or "Diff" over current
file content tells the user they have seen what changed when they have seen the file as it
is now. Say what it is: this is the file's current content, the previous version was not
captured, this is not a comparison. **Pick the wording and justify it** — it is the single
highest-consequence sentence in this slice.

### Render refusals, do not hide them

RFC-024 refuses rather than truncates: `TooLarge`, non-text content, a path not in
`DetectedChanges`, a symlink escape, an unreadable file. **Every refusal needs a rendering.**
A surface that shows nothing for a refused path is indistinguishable from one showing a
file with no changes, and the user cannot tell which they are looking at.

A stale baseline is its own case (`diff_content_is_stale`) and renders as stale, not as an
error and not as an empty diff.

### The detection limitation goes on the surface

RFC-012's detection is metadata-only and conservative. **The surface states what detection
does not cover** — not the handoff, not the closeout, the surface. Wording is yours; the
requirement is that a user reading the change list learns it may be incomplete.

## The AgentRun report surface

### Build the reader first, in `tekstide-core`

Per RFC-011 Amendment 1's five decisions. The two that shape the code most:

- **D1 — a bounded tail window**, sized from a measured figure against the real 32 MiB
  retention ceiling, not a comfortable sample. Deliberately different from RFC-024's
  refuse-never-truncate, and the amendment explains why: a truncated diff misleads a
  reviewer into approving what they did not see, while a windowed log withholds history
  they can still ask for.
- **D5 — complete vs. still-being-written in the type**, not a doc comment. RFC-024
  PR-024-C's separate constructors are the shape.

**The window is not the writer's truncation.** RFC-011 already lets the writer truncate at
the byte limit and records it in retention metadata — a permanent fact about the file. A
reader window is a transient fact about one request. **A surface that says "truncated"
without saying which one is lying about whether bytes still exist.** Render them
differently.

### Do not become a second retention policy

The reader does not delete, expire, purge, rewrite, or touch retention metadata. Enforce it
with an enumeration test naming every production call site that opens a transcript for
reading, so a new one fails by name. RFC-020 §Risks names this specifically.

## Boundary rules

- **Escaping happens in `crates/tekstide`**, at the widget. Models return raw bytes. See
  the window-boundary document.
- **No new bound.** RFC-024's 4 MiB is the content bound. A viewport or line cap is a
  *display* concern and must be named as one.
- **No shell state duplicating core.** PR-017-C's contract still holds: the session list
  and the change set live in `tekstide-core`; the shell renders them.
- **Read-only.** No accept, revert, or stage. If the surface looks like it needs a button,
  it does not — say so on the surface instead.

## Out of scope

- **Any two-sided diff, and any diff algorithm.** Neither is available and neither is this
  RFC's contribution.
- **Git-backed detection.** The only designed before-source, gated behind RFC-012's unmet
  safety evidence. Do not imply it exists.
- **Changing `DiffContent`'s ownership model.** README §4 explains why it stays as it is
  and what may not be claimed about it.
- **Search, filter, or navigation within a transcript.** Not in RFC-011 Amendment 1.
- **Widening `i18n::enforcement`'s scan.** Not this slice's territory, for the same reason
  RFC-019 was told the same.
