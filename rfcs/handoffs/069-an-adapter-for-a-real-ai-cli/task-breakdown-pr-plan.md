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

## PR-069-B — the bridge. Scoped 2026-11-15, after A was accepted at review 677

A is accepted and the breakdown below is written against what it measured, not against what the
RFC guessed. **One thing A did not look at turns out to block B entirely, and it is not an
implementation detail** — see B0.

### B0 — the environment question, and it is the owner's

**Measured while scoping, not by A**: `spawn_adapter`
(`crates/tekstide-core/src/runtime/terminal/launch.rs`) builds the Managed-path child's
environment with `env_clear()` and exactly seven entries — `TERM`, `LANG`, `LC_ALL`,
`PATH=/usr/bin:/bin`, `PS1`, plus the approval token and socket path. Nothing is inherited. The
built-in `claude-code` profile inherits `AiCliEnvironmentPolicy::Minimal` from `AiCliProfile::new`
and never overrides it.

Two consequences, both fatal to a bridge as things stand:

1. **`claude` is not on that `PATH`.** The profile's own lookup paths put it at
   `$HOME/.local/bin/claude`; `/usr/bin:/bin` does not contain it.
2. **`HOME` is unset.** A real AI CLI needs its own configuration and credentials, which live
   under the user's home directory. A `claude` launched with no `HOME` is not a supervised
   session; it is a broken one.

So the Managed path's environment, which is exactly right for `reference_adapter` (a program this
project wrote, needing nothing), **cannot launch a real AI CLI at all**. Making it work means
deciding what of the user's environment a third-party CLI reaches — which is RFC-004 §7's subject
and a security decision, not a packaging one.

**B0 produces a proposal, not code**: the minimum environment a real CLI needs, each entry
justified, measured by trying rather than reasoned from plausibility, together with what that
exposes. It goes to the owner. **No bridge code until it is answered**, for the same reason A came
before B.

### B1 — the bridge. Refined 2026-11-15 after B0 and its addendum

B0 settled the environment: a **fresh scoped `HOME`**, with **`CLAUDE_CONFIG_DIR` inside it**, and
**`CLAUDE_CODE_OAUTH_TOKEN`** for authentication — all three documented, none needing the user's
real home. What B0 did not settle, and what scoping B1 found, is **how the credential reaches the
CLI**. That is still the owner's, and it is a sharper question than B0's was.

#### B1-0 — the credential route, and the product has never carried a secret

Two facts, both measured while scoping, neither in B0:

1. **This product persists no user credential anywhere.** No keyring, no keychain, no credential
   store — searched and found nothing. The approval token it already has is minted per run and
   passed by environment; nothing is written down.
2. **`AiCliEnvironmentPolicy::ExplicitAllowlist` — the one variant built for forwarding named
   variables — is not implemented.** `validate_launch_spec` calls
   `unsupported_environment_policy_summary` and **rejects** both `ExplicitAllowlist` and `Named`
   with `TerminalLaunchError::UnsupportedEnvironmentPolicy`. `Minimal` is the only policy the
   runtime implements. (It fails closed, so no user is told an allowlist is in force when it is
   not — that part is sound.)

So the obvious route is advertised by the type system and refused by the runtime. Three options,
in increasing order of what they ask the product to take on:

- **(a) Implement `ExplicitAllowlist` for the adapter path.** The user keeps `CLAUDE_CODE_OAUTH_TOKEN`
  in their own environment; Tekstide forwards only the variables a profile names and stores
  nothing. **This is the recommendation**: least new exposure, and it finishes a policy the product
  already carries in its own types and its own UI summary.
- **(b) A bridge-specific addition to the `Minimal` set.** Narrower in blast radius, but it
  special-cases one CLI inside shared, security-sensitive code that every terminal launch runs
  through.
- **(c) Tekstide obtains and stores the token itself.** This introduces credential persistence into
  a product that has none, and with it key storage, lifetime, revocation and purge — every one of
  which is a threat-model subject. **It should not be chosen by default, and this pack does not
  propose it.**

**No bridge code until the owner picks one.** (a) and (b) are implementation; (c) is a new RFC.

#### B1-1 — the spawn environment, once the route is chosen

`spawn_adapter` currently builds `env_clear()` plus seven fixed entries. It gains the scoped
`HOME`, `CLAUDE_CONFIG_DIR` beneath it, and whatever the chosen route forwards. **This is
security-sensitive shared code**: `validate_launch_spec` is the same function every terminal
launch runs through, and `spawn_adapter`'s own doc comment says it is "a sixth `.env(...)` call on
top of that existing set, not a redesigned one." Keep it that way.

Scoped directories are created per `AgentRun` and removed with it. B0 named the cost: a fresh
`HOME` re-onboards every run unless the bridge seeds a template **it writes itself**, never copied
from the user's real home. Nobody copies `.credentials.json`; B0's addendum says so and it is not
negotiable.

#### B1-2 — the hook, and both fail-closed axes

A proved they are independent and that missing either is silently open:

- `onFailure: "block"` on **every** registered entry — the hook's own *failure* path, not the
  mechanism's default.
- `exit 2`, or `hookSpecificOutput.permissionDecision: "deny"` — the hook's own *decision* path.
  **Never a bare `exit 1`.**

A test for each, and **each shown failing**. Registration through `--settings` as an inline JSON
string, so nothing is written to the user's installation — D1's *Out of scope*, permanently.
**Never `--bare`**, and never inherit it: A found it disables hook loading with no error at all.

**Confirm before building on it**: the hook is a command Claude Code spawns *per tool call*, not a
long-lived process like `reference_adapter`. The approval channel must tolerate repeated short
connections carrying the same token. If it does not, that is a finding about the channel.

#### B1-3 — version pin and refusal

The exact string A observed, refused by default on any mismatch, with D5's documented override.
A's item 3 shows why `major.minor` is unsafe: `onFailure` arrived at `2.1.295`, inside the `2.1`
line.

### B2 — what the product is allowed to say, given what A found

Not a UI polish slice; the honesty requirement D4 states. A established three things that bound
what *Managed* may claim:

- the interception is **opt-in to fail-closed on two independent axes**, not fail-closed by nature;
- on an ordinary personal account — no managed settings, no `managed-mcp.json`, no Team/Enterprise
  login — **a plugin mod handling `tool.check` can override this bridge's deny**;
- **several named categories never reach the hook**: `EndConversation`, `@`-mentioned file reads,
  validation-rejected calls.

The precedent for saying this well already exists:
`AiCliWorkspaceDiscoveryPolicy::MayDiscoverWorkspaceFiles { summary }` is how this product already
tells a user in plain language what a CLI may do. B2 decides where the equivalent sentence lives
for what this bridge does **not** cover. The wording is the owner's; that there must be one is
D4's.

## What is not in this pass

A general adapter SDK, a plugin runtime, a second CLI, making approval mandatory for the built-in
profile, or any change to `Supervised` remaining the default. RFC-069's *Out of scope* holds.

**And not the provider-disclosure gap.** The M14 security review's Part 2 (F6) found the product
has no way to disclose where an AI CLI *sends* what it reads, only what it may read. That is
adjacent to B2 and is not RFC-069's — it has no owner yet and should not be absorbed here by
proximity.

## Order, and what makes a slice finished

A → B. **A is finished when a reader who has never seen the CLI could implement the detection in
item 3 from the document alone**, and when item 4 is specific enough to be argued with.

## What is not in this pass

A general adapter SDK, a plugin runtime, a second CLI, making approval mandatory for the built-in
profile, or any change to `Supervised` remaining the default. RFC-069's *Out of scope* holds.
