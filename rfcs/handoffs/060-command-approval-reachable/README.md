---
title: "RFC-060 handoff — Command Approval A User Can Reach"
status: "**Accepted 2026-10-12; not started.** `0.36.0`, **a 1.0 blocker**. D1–D9 decided on acceptance; no real-CLI adapter this release (that is RFC-069)."
source_rfc: "../../accepted/060-command-approval-a-user-can-reach.md"
source_rfc_status: "Accepted by the human owner 2026-10-12"
target_release: "0.36.0"
---

# RFC-060 — Command Approval A User Can Reach

Read [the RFC](../../accepted/060-command-approval-a-user-can-reach.md) first, and then read
**RFC-022's own risk document**,
[`what-the-dialog-must-not-lie-about.md`](../022-adapter-spawn-and-command-approval-surface/what-the-dialog-must-not-lie-about.md).
That document is not background; it is the bar this release is judged against, and this pack's own
risk file deliberately does not repeat it.

## The short version

**The approval UI and API are finished and no user can reach them.** Roughly 3,100 lines of
protocol, channel, coordinator and risk classifier in `tekstide-core/src/approval/`, a dialog in
`shell.rs`, Approval History beside it — all proven end to end against the reference adapter, and
all unreachable in production because:

- `prepare_adapter_approval` returns `Ok(None)` unless the profile is `Managed`
  (`agent/launch.rs:476`);
- both production profile constructions are `Supervised` (`agent/profile.rs:102`,
  `config/profile.rs:71`);
- `ConfiguredAiCliProfile` has two fields, `display_name` and `command`;
- `structured_action_approval` has no production setter, and `validate_compatibility` rejects
  `Managed` without it.

This release adds the way in. **It does not add an adapter for any real AI CLI** — that is RFC-069,
reserved on this RFC's acceptance.

## The thing most likely to go wrong

Not the plumbing. **The claim.**

This is the first release in which a real user can see the approval dialog, so it is the first
release in which `what-the-dialog-must-not-lie-about.md` §2 has to actually ship:

> Rejecting does not prevent execution. Nothing in Tekstide intercepts execution — a rejected
> adapter can run the command anyway, and Tekstide would not know. The model is cooperative, not
> enforced.
>
> A dialog that looks like a security control while being an honour system is worse than no
> dialog: it manufactures confidence that is not warranted.

D3 puts that on the surface. D4 makes `REQ-AGENT-013` a sweep of everything the product says about
approval — the book, the README, the changelog, `en.ftl` — looking for anything that reads as
enforcement. **A slice that lands the plumbing and leaves the claim unswept has not done the
dangerous half.**

## Slices

| Slice | What it is | Done when |
| --- | --- | --- |
| **PR-060-A** | The way in: `ConfiguredAiCliProfile`'s new fields, `to_ai_cli_profile`'s conditional level, `structured_action_approval` following from the adapter declaration rather than being its own switch. No UI. | A config file can produce a `Managed` profile that launches, and **cannot** express `Managed` without an adapter — structurally, not by validation refusal |
| **PR-060-B** | The sentence (D3) and the sweep (D4). | The dialog says what approval does and does not do, and nothing anywhere in the product claims enforcement |
| **PR-060-C** | The whole path once, for real: configured profile → real adapter process → real proposal → real dialog → real decision → the audit records it leaves, read back through `0.35.0`'s audit view. | Demonstrated end to end, with the reference adapter **named as a test-and-proof artifact** wherever it appears |

## Conventions

- Evidence under `evidence/pr-060-<slice>/`, paths relative to the repository root.
- Screenshots show **throwaway state only** — `mktemp -d` project, `mktemp -d` `XDG_STATE_HOME`.
  Capture as `wl-copy --clear` → `niri msg action screenshot-window --id <id>` → confirm
  `wl-paste --list-types` prints `image/png` → paste. **`wl-paste` returns the previous clipboard
  contents when a capture does not land**; two captures meant to differ must have different
  checksums.
- Ablations that need isolation go in a disposable `git worktree`, never a hand-dirtied tree —
  `ablate.sh`'s own header carries the recipe and the cases the script cannot see.
- The gate is `rfcs/handoffs/release-checklist.md`.

## What this pack does not decide

An adapter for a real AI CLI is **RFC-069**, not a stretch goal here. A slice that finds itself
wanting to write one has found a scope error worth reporting, not a feature to add.
