# RFC-069: An Adapter For A Real AI CLI

Status: **Accepted by the human owner 2026-11-15** — D1 through D7 as written, in `0.41.0`,
proposed the same day. Reserved 2026-10-12 on RFC-060's acceptance (D9).

**The two owner-level questions in *Acceptance* are answered by that acceptance, and are recorded
here so nobody re-opens them from the question's own wording:** (1) **a bridge is shipped** — this
project carries the maintenance dependency rather than leaving the approval pathway inert for
anyone who will not write code; (2) **an unproven upstream version is refused by default**, with
the documented override D5 requires. If either reading is wrong, say so and it costs a sentence —
but D2's measurement comes first either way, and both may be revisited on what it finds, which is
the point of doing it first.

## Summary

Every piece of command approval works and nothing uses it. The socket, the capability token, the
proposal and decision types, the modal, the audit producers and the spawn path were all built,
gated and closed across RFC-021, RFC-022 and RFC-060 — and the only program that has ever spoken
the protocol is `reference_adapter`, whose own module doc says *"no shipping AI CLI implements it,
and this program must never be presented as evidence that one does."*

A user can set `adapter = "managed"` on a profile today. `ConfiguredAiCliAdapter::Managed`'s own
documentation says what that does: *"declaring this does not make it true of the program — the
protocol is cooperative, and Tekstide cannot verify a claim like this one."* So the configuration
is reachable and inert, which is the same defect RFC-060 was written to fix one layer down.

This RFC makes the pathway real for **one named CLI**. The code is the smaller half. The subject is
**what this project may promise about an interface it does not own.**

## Why now, measured against the code as it stands

- The built-in profile `claude-code` (`agent/profile.rs`) launches `claude` from reviewed lookup
  paths at `AgentCompatibilityLevel::Supervised`. There is no built-in path to `Managed`.
- `ConfiguredAiCliAdapter` has exactly two states, `None` and `Managed`, and declaring `Managed` is
  the same act as declaring an adapter — deliberately one field, so a document cannot set them
  inconsistently.
- `reference_adapter` reads `TEKSTIDE_APPROVAL_TOKEN` and `TEKSTIDE_APPROVAL_SOCKET_PATH`, sends one
  `CommandProposal`, waits for a `CommandDecision`, and exits with a code naming the outcome. That
  is the whole contract an adapter must satisfy, and it is already demonstrated end to end against
  the real production channel.
- `ROADMAP.md`'s M14 names *"real AI CLI dogfooding for supported profile flows"*; 1.0's own
  minimum expectations name *"command approval for adapter-supported workflows."* Neither is met by
  a test artifact.

## The constraint that governs everything below

`what-the-dialog-must-not-lie-about.md` §4 forbids presenting the reference adapter as evidence
about real CLIs. **The failure this RFC must not commit is the same error pointed the other way**: a
bridge that works on the version it was built against and silently stops intercepting when upstream
changes would be *worse* than today's inert configuration, because a user would believe approvals
are being requested while commands run unapproved. Today's state is honest and useless. The failure
mode to avoid is dishonest and useful-looking.

## Decisions

### D1. The bridge is a separate process this project ships. Not a patch to the CLI, not a promise from upstream

The profile launches a small program of ours, which spawns the real CLI as its child and translates
that CLI's own permission mechanism into RFC-021 proposals. Nothing modifies the user's
installation of the CLI, and nothing depends on upstream adopting our protocol.

### D2. The upstream interface is measured and written down before any bridge code exists

**The first slice produces no product code.** It produces a dated handoff naming: the exact
upstream mechanism the bridge will bind to, the upstream version it was observed on, how that
version is detected at runtime, and what the mechanism does *not* cover. This RFC deliberately does
not name the mechanism, because the reviewer has not measured it, and an RFC that guesses at an
interface it has not seen is the thing this project's own history says gets rewritten.

### D3. Any mismatch fails closed, and the run does not start

If the bridge cannot establish the interception it expects — unknown upstream version, missing
mechanism, handshake failure — **the AgentRun does not launch**. It must never degrade to running
the CLI unsupervised, and the refusal must name which side failed, in the user's terms.

### D4. `Managed` never means "Tekstide verified this"

The word in the UI and in the docs describes a *declaration*, not a guarantee. Whatever is written
where this feature is described must survive the question *"what happens if the CLI ignores the
adapter?"* being asked by a user who then reads the answer. The exact wording is the owner's; the
prohibition is this RFC's.

### D5. The bridge pins the upstream version it was proven against, and refuses an unproven one by default

A security surface built on someone else's interface should not assume forward compatibility.
Default: refuse, name the proven version and the observed one, and offer the user an explicit,
documented override rather than a silent best-effort. **This is the decision most likely to be
wrong in the annoying direction** — it will refuse on a harmless upstream patch bump — which is why
the override exists and why D2's measurement must say how narrow the detection can safely be.

### D6. One CLI, named, shipped as a profile. Not a framework

Exactly one adapter, for the CLI this project already has a built-in profile for. No general
adapter SDK, no plugin surface, no second CLI "for symmetry". The contract `reference_adapter`
already documents is the extension point for anyone else.

### D7. The absence of a promise is itself documented

Upstream may change without warning; this project does not promise continuity and says so where a
user reads about the feature, rather than leaving them to discover it when a run refuses to start.

## Out of scope, deliberately

- **Not a general adapter SDK or plugin runtime.** Appendix A's MVP position is unchanged.
- **Not modifying the user's CLI installation**, ever — no config rewriting, no wrapper scripts
  installed into their `PATH`.
- **Not making approval mandatory for the built-in profile.** `Supervised` stays available and
  stays the default; this adds a reachable `Managed` path, it does not remove the other one.
- **Not a second CLI.** D6.

## What this buys, named against the requirement that demands it

`REQ-AGENT-012`/`013` and `REQ-SEC-012`/`013` — command approval, which RFC-060 made reachable and
nothing yet reaches · M14's *"real AI CLI dogfooding for supported profile flows"* · 1.0's own
*"command approval for adapter-supported workflows"*, which today is true only of an artifact this
project wrote to test itself.

## Acceptance

**What the owner decides:**

1. **Whether to ship a bridge at all** (D1), or to publish the adapter contract and leave the
   writing to whoever wants it. Shipping one means this project carries a maintenance dependency on
   an interface it does not control, every release, forever. Not shipping one means the approval
   pathway stays inert for anyone who will not write code.
2. **Refuse or warn on an unproven upstream version** (D5). Refusing is the safe default and will
   annoy; warning is friendlier and can leave a user believing they are supervised when the
   interception silently no longer binds.

**What I am asking to be accepted**: D1 through D7 as written, with **D2's measurement as the first
slice and no product code in it** — and with the explicit understanding that D1's and D5's shape
may change once that measurement exists, which is the point of doing it first.
