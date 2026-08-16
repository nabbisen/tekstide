---
title: "RFC-022: Adapter Spawn and the Command Approval Surface — Task Breakdown / PR Plan"
rfc: "RFC-022"
rfc_file: "../../proposed/022-adapter-spawn-and-command-approval-surface.md"
status: "Ready for implementation"
target_milestone: "M11"
created: "2026-08-16"
---

# Task Breakdown

Six slices. **[`what-the-dialog-must-not-lie-about.md`](./what-the-dialog-must-not-lie-about.md)
is required reading before any of them.**

## PR-022-A — Design and handoff acceptance

Granted 2026-08-16 with the pack. RFC-022's open question 1 is answered in the RFC itself
(no shipping AI CLI speaks this protocol; the first adapter is ours). Questions 2 and 3
remain the owner's and are not blocking until PR-022-E.

## PR-022-B — The reference adapter [Implemented — pending review]

First, because nothing else is demonstrable without it, and because it can be built and
proven **headlessly** against RFC-021's existing socket code with no GUI involved. That
makes it the cheapest place to discover that the protocol does not work as assumed.

See [`qa-evidence.md`](./qa-evidence.md#pr-022-b---the-reference-adapter) for the full
evidence record.

Review gate:

- Speaks RFC-021's protocol against the **real** socket channel and coordinator — not a
  mock, not a reimplementation of the encoding.
- A full round trip proven: proposal sent, classified, decision returned, adapter acts on it.
- **Both decisions exercised**, approve and reject. A test covering only approve proves the
  easier half.
- **The token is read from the environment** by the adapter, as a real child process would.
- Behaviour on a **missing or wrong token** is defined and tested — not left to whatever
  the socket happens to do.
- **Named and documented as a test-and-proof artifact**, not a product feature. Its own
  doc comment says so.

## PR-022-C — The adapter spawn path, and token delivery [Accepted with required change — response 216]

Core. Launches PR-022-B's adapter for real.

See [`qa-evidence.md`](./qa-evidence.md#pr-022-c---spawn-path-and-token-delivery) for the
full evidence record, including the socket-path delivery decision this slice had to make
(RFC-022's own text decides token delivery but not socket-path delivery), the resulting
change to the reference adapter's own contract from PR-022-B, and response 216's required
fix decoupling the approval channel's state root from transcript capture's.

Review gate:

- **A spawn path distinct from `spawn_shell`**, inside RFC-009's boundary.
- `.env_clear()` preserved; the token is one additional `.env(...)` with a value Tekstide
  generated. **Nothing inherited** — `ExplicitAllowlist` stays rejected, and a test pins
  that it is still rejected.
- `inject_token_into_environment` gains its **first production caller**; the enumeration
  naming that call site fails by name if a second appears.
- **A real spawned adapter completes a real approval round trip**, end to end, headless.
- Transcript capture works on this path (RFC-011 Amendment 2) — this is the first time
  anything configures a transcript writer in production, so it is the first real exercise
  of that amendment. **Say what that proves and what it does not.**

## PR-022-D — AgentRun creation, and a route to start one

Review gate:

- `launch_agent_run_with_runtime` gains its **first production caller**.
- A user can start an agent run from the GUI: `NavigationAction`, `AppCommand`, keybinding,
  dispatch arm. Check the binding mechanically against `KeybindingStatus` rather than by
  reading, and do not collide with a `Reserved` binding.
- **A selected-run concept exists**, since `ProjectOpenSurface::AgentRunDetail` carries no
  id — the gap RFC-020's surface work found. Decide its shape and state it; RFC-020's
  report surface will build on it.
- Resource limits enforced in **core**, not at the call site, matching how
  `terminal_session_limit` was done. A limit enforced by the caller is one the next caller
  forgets.
- Refusal is a typed error the shell can render — the user pressed a key and is owed an
  answer.

## PR-022-E — The approval dialog

The security surface. Do not fold it into D.

Review gate:

- **The proposed command is escaped at the widget**; raw bytes survive the model.
- **The falsifiable claim, tested**: a proposal containing a bidi override renders it
  visibly as an escape marker.
- **No double-escaping**, shown against literal `<U+202E>` text.
- **The cooperative limit appears on the surface**, in words a user reads. Quote the wording
  chosen and justify it — the highest-consequence sentence in this RFC.
- **No claim that rejection prevents execution**, anywhere in the surface or its copy.
- Modal exclusivity holds, under a live positive control (a `Tab` visibly moving a focus
  marker in the same capture) proving keystrokes reached the app while none reached the PTY.
- **A decision that can no longer be delivered is not recorded as if it were.** Decided by
  the architect 2026-08-16, replacing what was open question 2.

  The waiting belongs to the adapter, not to Tekstide: after sending a proposal the adapter
  does its own blocking read and sets its own timeout — PR-022-B's reference adapter already
  uses 30 seconds. Tekstide cannot make it wait longer or stop sooner. **What Tekstide owns
  is what happens when the user answers after the adapter has already given up.**

  Left alone, the user clicks Approve, an approval is written to the `command_approval`
  audit family, and it is sent to a connection nobody is reading. The audit trail then
  states that a command was approved which never ran — a false record in the one subsystem
  whose entire purpose is being an accurate one — and the user sees a button do nothing.

  Required: the dialog detects the connection is gone and **says so**, and **no decision is
  recorded for an undeliverable proposal**. Prove it against a real adapter that has actually
  exited, not a synthesised closed socket. Ablate it: remove the check and show the audit
  record that appears for a command nothing ran.

- Open question 3 (does this dialog interrupt a user mid-edit) is the owner's and must be
  answered before this lands; if the implementation forces it earlier, **stop and raise it**.
- The `command_approval` audit family gains its first real producer — it has been wired with
  no caller since RFC-021.

## PR-022-F — Closeout

Review gate:

- Claim statement checked **against RFC-022's own text**, not only the evidence file.
- **No claim of enforcement.** The single largest risk in this RFC.
- **No claim that real AI CLIs are supported.** The reference adapter proves the pathway,
  not the ecosystem.
- **No claim that the token is a security boundary.**
- What this unblocks for RFC-020, stated precisely: its surfaces become reachable; that is
  not the same as done.
- `rfcs/future-work.md`'s adapter-spawn standing theme updated in the same commit.

## Sequencing

```
A ─→ B ─→ C ─→ D ─→ E ─→ F
```

**B first is the important one.** It is the only slice that can fail for protocol reasons
rather than integration reasons, and it costs least to discover that in. **E last** because
it is the security surface and should be built against a pathway already known to work.

## What this hands forward

- The selected-run concept from D — RFC-020's report surface needs it.
- The escaping pattern for adapter-proposed content.
- Whether the transcript amendment holds up under its first production exercise.
