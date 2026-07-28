# RFC-021: Command Approval Model and Adapter Capability - QA Evidence

Status: Proposed — implementation pending
Date opened: 2026-07-28
Date accepted: Pending

## Scope

RFC-021 defines the command approval model and adapter capability contract. Headless; the rendered dialog is RFC-022.

Evidence in this file must not be used to claim command-level enforcement, OS-level interception, approval for `Plain`/`Supervised` runs, pattern-based auto-approval, a rendered approval dialog, or that any specific AI CLI supports the adapter contract — unless later reviewed implementation explicitly supports that claim.

**Standing constraint on all wording here:** approval is a *cooperative protocol*. Tekstide cannot intercept what a spawned process executes. No entry in this file may imply otherwise.

## Design Review

Pending PR-021-A acceptance.

## Implementation Evidence

### PR-021-B — Protocol types and validation

Pending implementation.

### PR-021-C — Risk classifier

Pending implementation.

### PR-021-D — Sideband channel

Pending implementation.

### PR-021-E — Approval coordinator and audit correlation

Pending implementation.

### PR-021-F — Closeout evidence

Pending implementation.

## What Tekstide May Claim

To be completed at closeout. This section is the honesty artifact — it states in plain language what a user gets, and must be usable verbatim in README and release notes.

Draft constraint for whoever completes it: the claim must distinguish *"Tekstide shows you commands an adapter submits, and does not run them until you decide"* from *"Tekstide controls what the AI CLI runs."* Only the first is true.

## Known Limitations

- Approval applies only to adapter-submitted proposals. An AI CLI that does not implement the contract is unaffected, and `Plain`/`Supervised` runs have no approval path by design.
- No pattern-based or always-allow rules. Every proposal requires a decision, which may prove noisy in practice; the remedy is a reviewed pattern language in a later RFC, not a silent threshold.
- No approval timeout. A pending proposal blocks the adapter indefinitely.
- The exact command is held in memory for display but never persisted, so durable audit cannot answer "what command was approved" — only that an approval occurred, at what risk level, for which run.
