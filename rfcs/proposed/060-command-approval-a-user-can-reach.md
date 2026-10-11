# RFC-060: Command Approval A User Can Reach

Status: **Proposed 2026-10-12.** `0.36.0`, pre-1.0, **a 1.0 blocker**. `REQ-AGENT-012`,
`REQ-AGENT-013`, `REQ-SEC-012`, `REQ-SEC-013`. Builds on RFC-021 (the protocol) and RFC-022 (the
spawn path and the dialog). D1–D7 and one open question are for the owner, on acceptance.

## Summary

**The command-approval UI and API are finished, proven end to end, and no user can reach them.**
Not a missing feature: a missing path to a built one. The same shape as RFC-059, and for the same
reason — a capability landed ahead of the surface that would have used it.

This RFC makes the pathway reachable without making a claim the product cannot keep.

## What is actually built, and what actually blocks it

Verified by reading the code, not the plan:

- **The protocol, channel, coordinator and risk classifier exist** — `crates/tekstide-core/src/approval/`,
  roughly 3,100 lines including tests — and are proven end to end against Tekstide's own real
  production channel code by `reference_adapter`.
- **The dialog exists** (`ApprovalDialogButton`, `shell.rs:560`), with Approval History beside it.
- **`prepare_adapter_approval` returns `Ok(None)` unless the profile is `Managed`**
  (`agent/launch.rs:476`). No `Managed` profile, no approval channel, ever.
- **Nothing in production ever produces a `Managed` profile.** There are exactly two production
  constructions — the built-in `claude-code` profile (`agent/profile.rs:102`) and every
  configuration-defined profile (`config/profile.rs:71`) — and **both are `Supervised`**.
- **`ConfiguredAiCliProfile` has two fields**: `display_name` and `command`. There is nothing a
  user could write to ask for anything else.
- **`structured_action_approval` has no production setter at all**, and
  `validate_compatibility` (`agent/launch.rs:847`) rejects `Managed` without it — so even a
  `Managed` profile obtained some other way would fail to launch.
- **No environment escape hatch exists.** The only approval-related variables
  (`TEKSTIDE_APPROVAL_TOKEN`, `TEKSTIDE_APPROVAL_SOCKET_PATH`) are what Tekstide *sets for the
  adapter*, not switches a user can flip.

So `REQ-AGENT-012`, `REQ-SEC-012` and `REQ-SEC-013` are unmet in the only sense that matters: the
behaviour they require cannot occur.

## What the 1.0 list promises, read carefully

> Adapter-mediated command approval UI/API.

— the requirements' own 1.0 scope list, and the external design says *"adapter-mediated command
approval **where supported**."*

**The promise is a reachable UI and API for adapter-mediated workflows. It is not a promise that
any particular AI CLI is supported.** That distinction is what makes this RFC deliverable at all,
and it is why the open question below is a scope decision rather than a correctness one.

## The honesty constraints, already decided and not reopened here

`rfcs/handoffs/022-adapter-spawn-and-command-approval-surface/what-the-dialog-must-not-lie-about.md`
settled these, and this RFC inherits them rather than revisiting them:

- **Rejecting does not prevent execution.** Nothing intercepts execution; a rejected adapter can
  run the command anyway and Tekstide would not know. The model is **cooperative, not enforced**.
- **The capability token is not a security boundary.** It says which run is asking, not that the
  asker is trustworthy.
- **The reference adapter proves nothing about real AI CLIs.** It speaks a protocol this project
  invented; none of them implement it. It is a test-and-proof artifact and must be called one
  wherever it appears.

That document also says the cooperative limit **belongs on the surface, in words a user reads** —
*"a dialog that looks like a security control while being an honour system is worse than no
dialog."* RFC-022 wrote that sentence; this RFC is where it has to actually ship, because this is
the first release in which a user can see the dialog at all.

## Decisions

- **D1. Configuration gains the fields that declare an adapter-mediated profile.**
  `ConfiguredAiCliProfile` grows what is needed to say "this command speaks Tekstide's approval
  protocol" — at minimum the adapter command itself. A profile that does not declare one stays
  `Supervised`, exactly as today.
- **D2. Declaring `Managed` and declaring an adapter are the same act.** There is no field that
  sets `Managed` alone. `structured_action_approval` follows from the adapter declaration rather
  than being a second switch a user could set inconsistently — the shape `config/profile.rs`'s own
  doc already argues for, where the configuration is *structurally incapable* of expressing the
  dangerous combination rather than merely refused when it tries.
- **D3. The cooperative limit is on the dialog, not only in the book.** One sentence, in
  `en.ftl`, that a user reads before approving — stating that approval is an answer to a request
  and that Tekstide does not prevent a rejected command from running. Wording is decided in review,
  against §2's bar.
- **D4. `REQ-AGENT-013` is met by what the product *does not* say.** No surface, book page,
  changelog entry or README line may describe this as command-level enforcement. The requirement
  is a prohibition, and meeting it is a sweep, not a feature.
- **D5. The protocol does not change.** RFC-021's wire format, the token delivery and the socket
  path resolution all stay as they are. This RFC adds a way in, not a second dialect.
- **D6. A declared adapter that never proposes anything is a legitimate outcome.** The dialog not
  appearing is not an error, and nothing may report it as one. An adapter chooses what to propose.
- **D7. Approvals and denials already audit** (`REQ-SEC-014`, the `command_approval` family). This
  RFC makes those records reachable in practice for the first time — and `0.35.0` shipped the view
  that can read them, which is the right order by luck rather than design, but is the right order.

## Non-goals

- **Shipping an adapter for any real AI CLI.** See the open question.
- Claiming enforcement, interception, or sandboxing of proposed commands.
- Changing the risk classifier's heuristics.
- A second approval channel, or any transport other than the existing one.

## Slices

- **PR-060-A — the way in.** `ConfiguredAiCliProfile`'s new fields, `to_ai_cli_profile`'s
  conditional level, and the structural impossibility of declaring `Managed` without an adapter.
  No UI. Done when a configuration file can produce a `Managed` profile that launches, and cannot
  produce an inconsistent one.
- **PR-060-B — the sentence.** D3's wording on the dialog, and the `REQ-AGENT-013` sweep of every
  place the product describes approval. Done when nothing anywhere claims enforcement and the
  dialog says what approval does and does not do.
- **PR-060-C — the whole path, once, for real.** The configured profile, a real adapter process, a
  real proposal, the real dialog, a real decision, and the audit records it leaves — read back
  through `0.35.0`'s own audit view. Done when that sequence is demonstrated end to end with the
  reference adapter **named as what it is**.

## Open question for the owner, to decide on acceptance

**Does `0.36.0` also ship an adapter for a real AI CLI?**

Everything above makes the pathway reachable for *someone who writes an adapter*. Realistically,
almost nobody will. Two answers, and the difference is large:

1. **No — ship the reachable pathway and the contract.** `0.36.0` meets the 1.0 promise as the
   requirements actually word it ("UI/API", "where supported"), the product says plainly that no
   AI CLI ships support today, and a later RFC takes up adapters per CLI. Small, honest, and
   leaves a capability most users never see.
2. **Yes — also ship one adapter against a real AI CLI's own hook mechanism.** Approval becomes
   real for actual users of that one CLI. Much larger, and it binds this project to an external
   product's interface: that interface can change without warning, which means a committed check
   that re-runs against it, and a decision about what the product claims when the upstream hook
   changes shape.

My recommendation is **(1) for `0.36.0`**, with (2) scoped as its own RFC immediately after rather
than vaguely later. The reason is the honesty constraint, not the effort: option 2's value depends
entirely on an interface we do not control, and shipping the reachable pathway first means the
adapter RFC can be judged on whether it keeps its promise, instead of carrying the 1.0 blocker's
weight as well.
