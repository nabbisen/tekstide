---
title: "RFC-069 — task breakdown and PR plan"
rfc: "RFC-069"
rfc_file: "../../accepted/069-an-adapter-for-a-real-ai-cli.md"
source_rfc_status: "Accepted by the human owner 2026-11-15 — RFC-069 is in rfcs/accepted/"
target_milestone: "pre-1.0"
created: "2026-11-15"
---

# Task breakdown and PR plan

Two slices, and the first one writes no product code. That is D2, and it is the decision the rest
of this RFC rests on: **an interface this project does not own gets measured before anything is
built against it.**

## PR-069-A — measure the upstream interface. No product code

**Deliverable**: one document,
`rfcs/handoffs/069-an-adapter-for-a-real-ai-cli/upstream-interface-measured.md`. No `.rs` file
changes. No profile changes. Nothing in `crates/`.

What it must name, each from something you ran or read rather than recalled:

1. **The mechanism** the bridge will bind to — what the CLI offers for intercepting or approving
   an action before it happens, by its real name, with the invocation or configuration that
   activates it.
2. **The version it was observed on**, exactly as the CLI reports it, and the command that reports
   it.
3. **How that version is detected at runtime**, by a program that must decide whether to proceed.
   D5 refuses an unproven version by default, so this is the detection that refusal depends on —
   and say how narrow it can safely be, since refusing on a harmless patch bump is the annoying
   failure D5 already admits to.
4. **What the mechanism does not cover.** The honest half, and the one most likely to be skipped:
   which actions the CLI can take that this mechanism never sees. An approval surface that covers
   most actions and is described as covering actions is the `what-the-dialog-must-not-lie-about.md`
   failure in a new place.
5. **What happens when it is absent or fails** — what the CLI does if the mechanism is configured
   and the far side never answers, which is what D3's fail-closed behaviour has to be built on.

**If the mechanism does not exist, or does not cover enough to be worth bridging, say so and
stop.** That is a finding, not a failed slice, and it goes straight back to the owner — D1's whole
premise is that a bridge is worth shipping, and this slice is where that premise is first testable.

### Boundaries for this slice, and one of them is about someone else's data

- **Public documentation and the installed CLI's own self-description only** — `--help`, a
  `--version`, a schema it publishes, its own documented configuration format.
- **Do not read the owner's personal configuration, session history, transcripts or credentials**
  for this CLI, and do not reproduce anything from them in the handoff. This machine is shared and
  the measurement needs the interface's shape, not anyone's use of it. If the only way to learn
  something is from the owner's own data, stop and say so.
- **Do not modify the user's CLI installation** — no config rewriting, no files placed in their
  `PATH`. RFC-069's *Out of scope* forbids it permanently, not just in this slice.
- No bridge code, no profile change, no `ConfiguredAiCliAdapter` work.

## PR-069-B — the bridge, scoped after A lands

Deliberately not broken down here. D1's shape and D5's refusal threshold may both change on what
A measures, and scoping a bridge against an interface nobody has looked at yet is what D2 exists
to prevent. The breakdown is written into this pack when A is accepted.

## Order, and what makes a slice finished

A → B. **A is finished when a reader who has never seen the CLI could implement the detection in
item 3 from the document alone**, and when item 4 is specific enough to be argued with.

## What is not in this pass

A general adapter SDK, a plugin runtime, a second CLI, making approval mandatory for the built-in
profile, or any change to `Supervised` remaining the default. RFC-069's *Out of scope* holds.
