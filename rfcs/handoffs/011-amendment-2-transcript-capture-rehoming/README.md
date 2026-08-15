---
title: "RFC-011 Amendment 2: Re-homing transcript capture — handoff pack"
rfc: "RFC-011 Amendment 2"
rfc_file: "../../done/011-transcript-retention-and-local-data-policy.md"
status: "Ready for implementation — authorised by the owner 2026-08-15"
target_milestone: "M11 prerequisite"
created: "2026-08-15"
---

# Start here

This unblocks adapter-spawn, which is M11's priority and which four reviewed capabilities
are waiting behind: RFC-021's command approval, RFC-024's diff content, RFC-011 Amendment
1's transcript reader, and RFC-020's two surfaces.

**It is not adapter-spawn.** It is the one prerequisite that must land first.

## Reading order

1. **RFC-011 §Amendment 2** — the decisions and their reasoning. Read it before this pack;
   everything here assumes it.
2. **[`the-ordering-and-the-failure.md`](./the-ordering-and-the-failure.md)** — required
   before any code. D2's ordering invariant and D3's failure policy are the two things that
   are easy to implement plausibly and wrongly.
3. [`task-breakdown-pr-plan.md`](./task-breakdown-pr-plan.md) — slices and gates.
4. [`acceptance-qa-checklist.md`](./acceptance-qa-checklist.md) — tick at closeout.
5. [`qa-evidence.md`](./qa-evidence.md) — record as you go.

## The situation in one paragraph

`LinuxTerminalRuntime::read_available_bounded_for` did two unrelated things in one loop: it
returned a bounded buffer to its caller, **and** it appended every byte to
`session.transcript_writer` and flushed it (`launch.rs:131-136`, `:162-169`). Those are the
only non-test writes to a `BoundedTranscriptWriter` in the workspace. RFC-017 Amendment 1
replaced the terminal's ingress with `TerminalReader`, whose module contains the string
"transcript" **zero times**. Nothing failed, because no production code creates an
`AgentRun`, so no writer is ever configured. Adapter-spawn is what changes that.

## What you are building

Transcript capture, moved into the reader thread, writing before the bytes enter the
channel, with a stated policy for what happens when a write fails mid-stream.

That is three decisions and one consequence, all in the amendment: **D1** where the write
happens, **D2** the ordering invariant, **D3** the failure policy per capture mode, **D4**
that backpressure now includes the disk.

## What must not change

- **Retention limits, capture modes, budget scope, purge semantics.** All decided in
  RFC-011's body. This amendment moves *where* capture happens — not what is captured, nor
  how long it is kept.
- **RFC-017 Amendment 1's reader contract**: bounded window, resynchronization, read-only,
  `dropped_bytes` structurally unreachable.
- **P1 and P2 as re-proven by PR-A1-B.** The writer is a new consumer of the byte stream
  *inside* the reader thread. It must not become a second path *out* of it. **Re-run those
  enumerations rather than assuming they transfer** — that assumption is what this whole
  amendment exists to correct, one layer down.

## The trap this work was created by

The old read path did transcript capture as a *side effect* of a function named for
something else. When that function stopped being the ingress, the capability disappeared
and no test noticed, because the subsystem that needed it was itself unreachable.

**Do not reproduce that shape.** Whatever writes the transcript should be named for
writing the transcript, and its absence should be detectable by something other than a
human reading a diff two months later.

## Out of scope

- **Adapter-spawn itself.** This unblocks it.
- The transcript **reader** (Amendment 1) — unchanged.
- Any change to retention, capture modes, or purge.
- Making `AgentRun` reachable. Still nothing creates one; that is adapter-spawn's job, and
  this work must be correct *before* it, not demonstrated through it.
