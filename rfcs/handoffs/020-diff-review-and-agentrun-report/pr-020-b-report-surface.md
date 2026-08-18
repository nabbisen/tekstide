---
title: "PR-020-B, surface half — the AgentRun report surface"
status: "Scheduled 2026-08-18, awaiting implementation"
rfc_file: "../../proposed/020-diff-review-and-agentrun-report.md"
target_milestone: "M11"
created: "2026-08-18"
---

# PR-020-B, surface half — the AgentRun report

## Where this picks up

PR-020-B's **reader half landed 2026-08-15** (RFC-011 Amendment 1, commits `b74d8d5`/`c92d97e`).
Its **surface half was stopped the same day** by response 200, for a reason that was correct
then and is false now:

> an `AgentRun` cannot exist at all in production today

`0.10.0` made agent-run launch reachable (RFC-032 granted trust, RFC-022 built the spawn
pathway). `0.11.1`'s investigation then established the rest of the chain, which nobody had
traced end to end:

| link | where |
| --- | --- |
| a transcript is written for every AI CLI run | `runtime/terminal/launch.rs:47` |
| registered on the project | `project/session.rs:540` |
| discoverable from the run | `AgentRun.transcript_ref` → `Transcript.storage_path` |
| path reconstructable | `TranscriptPathResolver::resolve_agent_run` |
| bounded reader | `transcript::reader::read_window` |

**Every link is production code reached by a real key press.** This is the first RFC-020
surface whose producer is real.

**Read `the-window-boundary.md` in this pack before writing any code.** It is required
reading and nothing here replaces it.

## The question the pack raised and never got answered

> Before building the AgentRun report widget, request 200 asked *which* `AgentRun` it should
> show, since no "currently selected run" concept exists anywhere.

`NavigationAction::OpenCurrentAgentRunDetail` says **current**, and nothing defines it.

**Answer: the most recently launched run in the active project.** Reasons, so this is not
re-litigated: it matches the action's own name; `agent_run_limit` bounds how many can exist;
and a selector is a second surface with its own navigation decisions, which is not what this
slice is for.

**If there is no run, the surface must say so** — "no agent run in this project yet" — not
render empty chrome. An empty surface that looks broken is the failure this project's
zero-reachable-surface rule exists to prevent, arriving one layer in.

## Reachability — name it before building, per `ARCHITECTURE.md`

`OpenCurrentAgentRunDetail` has a **real `AppCommand`** already
(`ProjectOpenSurface::AgentRunDetail`) and **two gaps**:

1. **No render arm.** `content_mode_view` falls through to the plain editor for
   `AgentRunDetail`. A binding without a render arm makes the key silently open an editor,
   which is worse than a dead key.
2. **No binding.** `Configurable`/`None`.

Both are this slice's. Order matters: **render arm first, binding second**, so the key never
exists in a state where it does the wrong thing.

Follow `approval-history-binding.md`'s shape for the binding — a real `Candidate`, collision
checked mechanically, not by reading the table. `Ctrl+Alt+R` (Report) looks free; verify.

## What the surface renders

Transcript content for the selected run, through `read_window`, **escaped at the widget**.

RFC-020's §Security core is binding and is not negotiable in this slice: the transcript is
**untrusted third-party output**, it escapes, and it inherits neither the terminal grid's
raw-bytes exception nor the editor's. The justifications for those two do not transfer, and
the RFC says why.

Also render, because RFC-011's bounds are real and a user reading a window needs to know it
is a window:

- **The delivered start offset**, not the requested one.
- **Whether the transcript is complete or still being written** — the reader expresses this
  in its type; the surface must not flatten it.
- **Reader window versus writer truncation, rendered differently.** Conflating them is the
  named failure mode: "you are seeing part of this file" and "part of this file was never
  kept" are different facts about the user's data.

## The gate

From the pack's own PR-020-B gate, the items that need the widget to exist:

- **Escaping at the widget, and no double-escaping**: content containing the literal text
  `<U+202E>` is distinguishable from a real override. This is a test, not an assertion.
- **Raw bytes survive the reader**, proven against `text_safety`'s own bidi probe.
- **Reader window vs writer truncation render differently**, pinned by a test.
- **The window size is measured** against the real 32 MiB ceiling, not estimated. Two
  estimated figures in this project were wrong once measured.

And from this slice's own shape:

- **Proven from a real key press** — the binding, through `update`, to rendered content — not
  from a dispatched `AppCommand`. Response 248's lesson.
- **A real transcript from a real run**, using the seam `transcript-capture-evidence.md`
  added rather than a hand-written file. The producer is real now; test against it.
- **The no-run case renders its own message**, asserted.

## What this does not do, and the closeout must say so

- **It does not make the change review surface reachable.** That is PR-020-C and it is
  blocked on the `DetectedChanges` projection described in RFC-020's scoping section.
- **It does not render what an agent *changed*** — only what it *said*. A user reading this
  surface is reading terminal output, not a change list.
- **It does not establish that the real Claude Code CLI behaves well under it.** Every test
  uses a controlled executable, as everywhere else in this project.

## Not in scope

- Selecting among runs. Answered above: most recent.
- Purging or opting out of transcripts — RFC-033.
- The change review surface — PR-020-C.
