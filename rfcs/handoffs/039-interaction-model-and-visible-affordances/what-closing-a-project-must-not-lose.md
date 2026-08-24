---
title: "What closing a project must not lose"
rfc: "RFC-039"
rfc_file: "../../accepted/039-interaction-model-and-visible-affordances.md"
source_rfc_status: "Accepted 2026-08-24 — M12, after RFC-038"
status: "Required reading before any RFC-039 close code"
created: "2026-08-24"
---

# What closing a project must not lose

Everything else in RFC-039 is additive: a strip that shows things, controls that navigate.
**Closing is the one destructive action**, and it is the only part of this RFC that can lose a
user's work. It gets its own document for that reason.

## 1. A close can kill a running process

A project may own live terminal sessions and an active agent run. Closing it ends them. That is
not a rendering concern — an AI CLI mid-task, or a shell with unsaved state in it, disappears.

**Required:** a project with **no** running terminal and **no** active agent run closes
directly. One with either raises a confirmation naming what will be lost — counts, not vague
warning text. "2 terminals, 1 agent run" is actionable; "this project has unsaved work" is not.

**Why not always confirm:** a confirmation that appears every time trains people to dismiss
confirmations without reading. RFC-018 built an entire paste model around not doing that, and
weakening it here would undo the lesson elsewhere.

## 2. The confirmation must name the project by path, not by name

Project display names are filesystem-derived and attacker-influenced. Escaping them is required
and is **not sufficient here.**

For switching, a misleading label is a wrong belief with no wrong action — activating a tab
switches to the project that tab genuinely is. For closing, a wrong belief becomes a destructive
action against the wrong target.

**Required:** the close confirmation identifies the project by its **canonical path**, escaped
and bounded, so a user can tell which project they are about to close even if two display names
were chosen to look alike. Show the name too if it helps; the path is what must be there.

## 3. Closing is not purging

`close_project` removes a project from the session. It must not delete transcripts, audit
records, or anything on disk. RFC-033 owns purge, it has its own confirmation, and the two must
not be confusable in the UI: a user reaching for "close" must never wonder whether their data
just went.

**Required:** a test that closing a project leaves its transcripts and audit records intact.
State it in the confirmation too, briefly — a user closing a project with capture history should
not have to guess.

## 4. `safe_close_decision` is unblocked and must be wired

That audit family has never had a producer. RFC-031 scoped it out with a stated reason: blocked
on a close dialog that did not exist. This RFC builds the dialog, so the reason is gone.

**Required:** wire it, recording the decision — closed, or cancelled — for a project that had
live work. Both outcomes, not only the destructive one; "the user was asked and said no" is the
more interesting record.

That takes unwired audit families from two to one. Do not let it slip to a closeout: producers
wired at closeout are how `safe_close_decision` got skipped the first time.

## 5. The strip is trusted chrome

RFC-016's grid exception applies to terminal output. It does **not** apply to chrome, and the
tab strip lives in the top bar. Every project name in it is untrusted text in trusted chrome —
the exact case RFC-018 §2 names.

**Required:** escaped, bounded so a long name cannot push the rest of the strip off-screen, and
tested with the bidi-override fixture this project already has in its recent-projects state.

## 6. What this document does not cover

Whether a closed project's terminals are killed or reaped, and by what mechanism. That is
`close_project`'s own contract in core — read it before assuming, and if it turns out **not** to
stop child processes, **stop and escalate**: that is a leak with a user-visible trigger, and it
is a finding, not something to work around in the surface.
