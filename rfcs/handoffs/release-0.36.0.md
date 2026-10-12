---
title: "Release 0.36.0: command approval a user can reach"
status: "**Published 2026-10-12.** Authorised by the owner; published, tagged and verified by the architect. The 1.0 blocker that turned out to be a finished feature with no door, and the first release whose published archives are byte-identical to the local ones."
rfc_file: "../done/060-command-approval-a-user-can-reach.md"
target_milestone: "pre-1.0"
created: "2026-10-12"
---

# Release 0.36.0

## What this release is

**A 1.0 blocker that was never a missing feature.** The command-approval protocol, channel,
coordinator, risk classifier and dialog were finished and proven end to end under RFC-021 and
RFC-022. No configuration could produce a `Managed` profile, `prepare_adapter_approval` returned
`None` for anything else, and `structured_action_approval` had no production setter — so the dialog
existed and no user could ever see it.

`adapter = true` on a configured profile is now the one key that asks for `Managed`, and the only
one. `ConfiguredAiCliProfile` is **structurally incapable** of representing `Managed` without it:
one two-variant enum, one match producing both the compatibility level and the capability together,
so a future third variant cannot be half-decided — that is a type error, demonstrated by ablation
rather than asserted.

## What it does not do, said here because it is the point

**For nearly every user this release changes nothing.** No shipping AI CLI speaks Tekstide's
protocol; only this project's own reference adapter does, and that is a test-and-proof artifact.
Declaring `adapter = true` on an ordinary AI CLI does not make Tekstide supervise, sandbox or
intercept it — it makes Tekstide wait for a proposal that never arrives.

**Rejecting a proposal does not prevent execution.** The model is cooperative, not enforced;
Tekstide has no execution path of its own to withhold. That sentence shipped on the dialog in
August 2026 under RFC-022 and has waited two months for a user who could reach it. This is the
release where it finally has one.

`REQ-AGENT-012`, `REQ-SEC-012` and `REQ-SEC-013` are met **for a profile that declares an
adapter**. `REQ-AGENT-013` is met by what the product does not say.

## A failure mode this release created and then named

A `Managed` launch binds its approval socket under `$XDG_STATE_HOME`, and a Unix domain socket's
path is capped at a small fixed size shared with the `approval/` segment and the run's own id.
`ApprovalChannel::bind` had distinguished `SocketPathTooLong` since response 113 — and nothing
carried that reason to the surface, so it fell into *"Couldn't start an agent run."*

Before this release that arm was unreachable, because no configuration could reach `Managed`. The
release that makes a failure mode reachable is the release that owes it a message, so `0.36.0`
names it and says what to change. It also documents the socket's path in the user book, where the
`AF_UNIX` limit had previously been recorded only as test-fixture guidance.

## The README has screenshots

Two, at the owner's request: the Project Board, and a project open with the file tree beside a
running terminal. Taken against a `mktemp -d` demo project under a `mktemp -d` `XDG_STATE_HOME`
with a throwaway `HOME`; the only path in either frame is `/tmp/tekstide-demo`, and the terminal
prompt renders as `tekstide$` with no user or host. The approval dialog was **deliberately left
out**: it photographs well and would imply to a first visitor that Tekstide supervises AI CLIs,
which it does not.

## Published

`cargo publish --workspace --locked`, core first. Tagged **`0.36.0` at `b6c6802`** and published
from that commit.

**Post-publish: the published and local archives are byte-identical, both crates.** Not the two
documented differences — none at all, because the workspace packaging that produced the local
archives ran at the same commit and paired both crates locally, so even the lockfile checksum
matched. The published `tekstide` lockfile names `tekstide-core 0.36.0` with checksum
`4c3b123c…`, and the published core `.crate` hashes to exactly that.
`post-publish-check.sh` **PASS** for `0.36.0` and `0.35.0`, both including the `cargo install`
build.

Run It, against the release binary: the first screen names only actions that exist, with the
app-wide `Audit` button present and the per-project `Trust Settings` correctly absent.

## What review cost, and bought

Reviews **535–541**. The pattern of this RFC was a reviewer who scoped the mechanism closely and
the surface not at all:

- **D3 asked for a sentence that had shipped two months earlier.** The implementer checked, pushed
  back, and declined to add a redundant second catalog key. They were right.
- **D4 asked for a sweep for enforcement claims.** Every enforcement claim was already correct; the
  real staleness was two *reachability* claims that the first slice had just falsified. The sweep
  found what the instruction did not ask for.

Three required items were about checks that could not fail or did not run: a fixture leaking on
every passing run under a gate line claiming zero, a derivation that could be half-decided, and a
summary describing a documentation cross-link that did not exist.

## Deferred, recorded rather than fixed

- **RFC-069** — an adapter for a real AI CLI, reserved on this RFC's acceptance. Its own question
  is what this project can promise about an interface it does not control.
- **RFC-068** — redacting what a run prints.
- A terminal-resize test whose poll condition is weaker than its assertion; open verdict in
  `test-process-leak.md`.
- The audit directory's permission modes, and the keyboard-unreachable controls on several
  surfaces, both in `rfcs/future-work.md`.
