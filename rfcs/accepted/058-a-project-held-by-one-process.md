# RFC-058: A Project Held By One Process

Status: **Accepted by the human owner 2026-10-10.** D1–D7 as written; **the open question answered with a third option the owner proposed** — see *Decided on acceptance*. Proposed 2026-10-10. `0.34.0`, M13. `REQ-PROJ-009`, moved out of the Project-lifecycle
coverage row on 2026-09-23 because no such mechanism existed. **It is a `must`, and the hazard it
names got worse in `0.31.0`.**

## Summary

> `REQ-PROJ-009`: Tekstide **must** use a process-visible project lock or equivalent
> conflict-prevention mechanism for **write-sensitive project state**. In-memory-only locks are
> insufficient.

Two Tekstide processes can hold the same project root today and neither knows. That was a latent
problem in September, when it was moved out of the coverage table. **RFC-027 made it a live one**:
recovery records are keyed by project id and a hash of the document's relative path, with nothing
about the process in either — and `AppState::open_project` **reuses the project id for the same
canonical root**, looked up from `recent-projects.json`, because that reuse is what lets recovery
survive a restart at all.

So: open one project root in two Tekstides, edit the same file differently in each, and both write
**the same recovery record file**. Last tick wins. On a crash the user is offered one of the two
edits with nothing saying the other ever existed.

**That is write-sensitive project state with no conflict-prevention mechanism — the requirement's
own words — and the product's own safety feature is what introduced it.**

## What is actually at risk, and what already is not

The requirement is scoped to *write-sensitive project state*, not to "the project". The work is to
find what two processes corrupt, not to invent a general lock.

| State | Shared between two instances? | Protected today |
| --- | --- | --- |
| **Recovery records** (`recovery/records/<project-id>/<path-hash>`) | **Yes — identical paths** | **No. The live hazard.** |
| `recent-projects.json` (trust decisions, ordering) | Yes, app-global | No — last writer wins |
| The audit store | Yes | **Yes, already.** SQLite's own file locking with a `busy_timeout` (`journal_mode = DELETE`) serialises writers between processes — *"or equivalent conflict-prevention mechanism"* is already satisfied here |
| Transcripts | Per agent-run id | Not shared; two instances produce different runs |
| Instance markers | Per pid, by design | Not shared (RFC-027 D12) |

**D1 — The audit store needs no work, and the RFC says so rather than re-implementing it.** A
project that already satisfies a requirement in one place should not grow a second mechanism beside
it. Confirm it; do not duplicate it.

## Decisions

**D2 — Reproduce the clobber before designing against it.** Two real processes, one real project
root, the same file edited differently in each, and the record file read afterwards. This is the
RFC-066 D5 lesson applied before the fact rather than after: **the chain above is read from the
code and has not been watched happen.** If it does not reproduce, that finding outranks this RFC.

**D3 — The mechanism is visible to another process, because the requirement says in-memory is
insufficient.** A file-based lock under the state directory, held by the owning instance.

**D4 — A lock must not outlive the process that took it.** A crash leaves the lock file behind, and
a lock that blocks forever after a crash converts a rare conflict into a permanent outage. This
project has met this exact hazard twice: RFC-027 D12's pid-based marker, and
`transcript::tests::a_live_writer_holds_an_exclusive_lock_until_it_is_dropped`, where a
fork-duplicated descriptor kept an `flock` alive past its drop. **Both are prior art and both must
be read before choosing the primitive.**

**D5 — Failing safe means refusing to protect, never refusing to run.** If the mechanism cannot
decide who owns a project, the answer is that *nobody* claims ownership of the write-sensitive
state, not that the editor stops working. A text editor that will not open because a lock file is
confusing has chosen the wrong failure.

**D6 — Whatever the second instance does not get, it is told.** RFC-027's risk document §1 row 4
already states this: *never claim protection it is not providing*. If the second instance does not
write recovery records, that is a real downgrade of a safety feature and the user must be able to
find out at the time, not discover it after a crash.

**D7 — No new `REQ-`.** `REQ-PROJ-009` names this exactly, and this RFC is what lets it return to
the coverage row it was removed from.

## Non-goals

- **Not a general multi-instance model.** Nothing here coordinates two instances' views, selections
  or terminals. Only the write-sensitive state in the table above.
- **Not a fix for `recent-projects.json`'s last-writer-wins** unless the chosen mechanism gives it
  for free. It is app-global, not project-scoped, and `REQ-PROJ-009` is about project state. Name it
  as out of scope rather than let it look covered.
- **Not RFC-036's recent-project pruning gap**, which is separate and already recorded.

## Slices

- **PR-058-A — reproduce.** D2, against two real processes. The reproduction becomes the regression
  test.
- **PR-058-B — the mechanism.** D3, D4, D5, with the audit store confirmed untouched (D1).
- **PR-058-C — saying it.** D6, and the coverage row returned to `REQ-PROJ-009` with what the
  mechanism actually is.

## Open question for the owner, to decide on acceptance

**What does the second instance do?** Two shapes, and this is a product decision rather than a
technical one:

1. **It opens, but does not own the project's write-sensitive state** — no recovery records written
   for that project, and it says so. Two windows on one project keep working; the clobber becomes
   impossible; one of them is unprotected and knows it.
2. **It refuses to open that project** — stricter, simpler, and closer to the requirement's literal
   "project lock", but it forbids a workflow a multi-project workbench plausibly wants.

**My recommendation: 1.** The hazard is silent corruption of state, not two people editing — and
`REQ-PROJ-009` asks for conflict *prevention*, which option 1 delivers without taking away a
working editor. Option 2 trades a rare silent failure for a frequent visible obstruction.

## Decided on acceptance (2026-10-10)

**The owner proposed a third option, better than either I offered: *"activate the existing window
instead of opening another tab."*** It resolves the conflict by making it impossible rather than by
degrading protection, and it matches what a user actually means by opening a project they already
have open. It is adopted — **with one constraint, established by reading the substrate rather than
assuming it.**

**D8 — A second attempt sends the user to the holder; it does not open a duplicate.** The project
is held by one process. A second instance asked for the same root does not open it, names the
instance that holds it, and asks that window for attention.

**D9 — Raising the window is not possible on this platform, and nothing may promise it.** Checked
in the dependency tree actually in use:

- `iced 0.14` exposes **both** `window::gain_focus` and `window::request_user_attention`, so the
  API is reachable.
- `gain_focus` reaches winit's `focus_window`, whose own documentation reads: **"iOS / Android /
  **Wayland** / Orbital: Unsupported."** A Wayland client cannot raise itself — the compositor
  decides. That is a deliberate anti-focus-stealing property of Wayland, not a gap in winit, and
  this product runs on Wayland.
- `request_user_attention` **does** work there, via `xdg_activation_v1`: it marks the window as
  wanting attention, and the compositor surfaces that however it chooses. **The user still moves;
  the window does not come to them.**

**So D8 is implemented as: refuse the duplicate, name the holder, request attention best-effort —
and say nothing that claims a raise happened.** `0.23.0` was called *What The Window Says Is True*;
a message reading "switched you to the existing window" when the compositor did not switch anything
would be that defect returning. On platforms where `gain_focus` is supported (X11, macOS, Windows),
it may additionally be called — which makes this a **capability that degrades by platform**, and
therefore RFC-028's business at M14, not something to be designed around now.

**D10 — This needs the instances to talk, and that is new.** Instance B must reach instance A at
all. The prior art is the approval adapter's own `AF_UNIX` channel, including its socket-path-length
constraint — the `SocketPathTooLong` lesson this project has met more than once. **The lock and the
channel are the same problem seen twice**: the holder must be discoverable by another process, and
whatever makes it discoverable must not outlive it (D4).

**What this removes:** my option 1 is gone. No instance opens a project it does not own, so there is
no "opened but unprotected" state to explain, and D6's obligation to disclose a downgrade has
nothing left to disclose — a better outcome than the one I recommended.
