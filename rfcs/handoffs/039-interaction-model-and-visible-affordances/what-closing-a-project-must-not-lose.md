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

## 4a. `SafeCloseAbandon` is not offered, and this is why

Added 2026-08-24 after request 310 asked what it is for and found nothing in these documents
answering.

RFC-013 named **terminate/abandon** as the two selectable safe-close actions, abandon meaning
close the project and leave its running work running. RFC-031 recorded both action kinds as
unused and disclosed, blocked on a surface that did not exist.

**It stays unused, now as a deliberate exclusion rather than an omission.** Offering "close this
project but leave its terminals and agent run running" would produce processes that **nothing
owns**: `close_project` removes the `ProjectSession` from `AppState` while `TerminalRuntime`
keeps its `RunningTerminal` values, with no project referencing them and no route to reach or
stop them. That is exactly the orphaning §6 exists to prevent, and the shape that stranded 4023
shells during PR-038-C.

RFC-013 envisioned abandon before it was known that nothing in this product terminates a process
group and that `close_project` never touches the runtime. **Abandon becomes offerable when there
is a real detach model** — something that still owns those processes and can reattach to them —
and not before. Until then the dialog offers terminate or cancel.

## 5. The strip is trusted chrome

RFC-016's grid exception applies to terminal output. It does **not** apply to chrome, and the
tab strip lives in the top bar. Every project name in it is untrusted text in trusted chrome —
the exact case RFC-018 §2 names.

**Required:** escaped, bounded so a long name cannot push the rest of the strip off-screen, and
tested with the bidi-override fixture this project already has in its recent-projects state.

## 6. `close_project` does not stop anything — verified, not assumed

This section originally told you to read `close_project`'s contract and escalate if it did not
stop child processes. It was read during PR-038-C's review (request 299), so here is the answer
instead of the instruction.

**`close_project` never touches the terminal runtime.** It calls `assess_project_close`, and if
the assessment says safe, `remove_active_project_session` — which removes the `ProjectSession`
from a `Vec` and updates `active_project_id`. That is all. The runtime's `RunningTerminal`
values are held elsewhere and are not consulted.

`RunningTerminal` has **no `Drop` impl**, and nothing in `runtime/` or `project/` calls
`.kill()`.

There *is* a correct termination path — `TerminalRuntime::request_terminate` in
`runtime/terminal/termination.rs`, SIGTERM escalating to SIGKILL against the process group, with
a guard refusing to signal a group id ≤ 1. **It has zero production callers.** Another reviewed,
tested, unreachable capability, and this one is load-bearing for the slice you are about to
write.

What saves the product today is that `assess_project_close` refuses: a project with live
terminals or an active agent run is **not safe to close**, so `close_project` returns the
assessment and removes nothing. The leak is currently unreachable because the close is.

**So PR-039-C must not force the close past the assessment.** The confirmed path is:

1. Confirmation accepted →
2. `request_terminate` on each of the project's terminals, and wait for the runtime to observe
   the group gone — its first production use, so treat it as new code, not as plumbing →
3. *then* `close_project`, which will now assess as safe on its own terms.

Removing the project first and terminating afterwards, or bypassing the assessment, orphans
every shell in that project with nothing owning them. At scale that exhausts the PTY pool: the
dev team hit exactly this in testing, **4023 orphaned shells, 4096/4096 PTYs consumed**, from
the same `Child::drop`-does-not-kill defect in the production spawn path.

If `request_terminate` turns out not to do what it says — it has never run in production —
**that** is the escalation, and it is a core finding, not something to compensate for in the
surface.
