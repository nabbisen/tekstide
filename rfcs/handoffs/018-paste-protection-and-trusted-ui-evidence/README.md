# RFC-018: Rendered Paste Protection and Trusted-UI Evidence - Developer Handoff Pack

Source RFC: [RFC-018](../../proposed/018-paste-protection-and-trusted-ui-evidence.md)
Target milestone: **M9** (`0.5.x`), second half
Source RFC status: **Accepted by the human owner 2026-08-08**

**Start here.** This file is the entry point. Everything is linked below in reading order.

## Read in this order

| # | Document | Purpose |
| --- | --- | --- |
| 1 | [RFC-018](../../proposed/018-paste-protection-and-trusted-ui-evidence.md) | The policy being rendered, the dialog decision, and the evidence obligation. **Read "The security core" before anything else.** |
| 2 | This file | Orientation and what is binding. |
| 3 | [`pr-018-b-paste-ingress.md`](./pr-018-b-paste-ingress.md) | **Detailed instructions for PR-018-B, the security-critical slice.** Read before writing any code. |
| 4 | [`implementation-handoff.md`](./implementation-handoff.md) | What already exists, the seams, and what is genuinely missing. |
| 5 | [`task-breakdown-pr-plan.md`](./task-breakdown-pr-plan.md) | Slice boundaries and review gates. |
| 6 | [`acceptance-qa-checklist.md`](./acceptance-qa-checklist.md) | Required evidence. |
| 7 | [`qa-evidence.md`](./qa-evidence.md) | Where you record gates, findings, and limitations. |

Read before starting — RFC-018 conforms to these rather than amending them:

- [RFC-009](../../done/009-terminal-security-boundary.md) — the paste and trusted-UI policy. **This RFC renders it. It does not widen it.**
- [RFC-013](../../done/013-durable-audit-store-and-local-data-policy.md) — the frozen v1 schema. `paste_blocked` fits it as-is; **no amendment.**
- [RFC-015](../../done/015-application-shell-and-rendered-surface-model.md) — the modal layer the dialog is built on, and the input-routing model.
- [RFC-017](../../done/017-terminal-renderer-and-immersion-mode.md) — the terminal this protects. Its §Performance correction and its Status both state what this RFC may not claim.

## Where to start work

**Begin at PR-018-B.** PR-018-A is design acceptance, already granted with the RFC.

## The shape of this RFC, in one paragraph

**Nothing here needs designing.** RFC-009 already defines the policy and `tekstide-core` already implements it: `TerminalInputPolicy::evaluate` returns `Allow` / `RequiresConfirmation` / `Block`, and `TerminalTrustedUiBoundary::assess_terminal_output` produces a spoofing assessment. Both are tested and **neither has a production caller** — the same condition `plain_terminal_observation` was in before RFC-017 PR-017-F wired it. Your job is to call existing policy from real input, render the answer, and prove the result. If you find yourself writing a classification rule, stop: that rule already exists somewhere in core, or it belongs in an RFC-009 amendment.

## What is binding

1. **The shell renders decisions; it does not make them.** No paste classification in `crates/tekstide`.
2. **One PTY ingress.** Paste bytes reach the PTY through the same single, modal-gated call site keystrokes already use. Enumerate and ablate.
3. **`RequiresConfirmation` is not `Allow`.** Until PR-018-C exists, it blocks. A dialog being inconvenient to build is not a reason to let bytes through.
4. **The pasted-content preview is untrusted text in trusted chrome.** RFC-016's grid exception does not reach it.
5. **No schema amendment.** `paste_blocked` fits the frozen family as it stands.
6. **RFC-014 PR-014-D's spike screenshot is not evidence for the product.** This prohibition has held across six slices and is most tempting here.

## Three traps this project has already fallen into, waiting for you again

**The sentinel test that scans the wrong file.** PR-017-F's first version read `database_file()` while the store was open. In WAL mode the record it had just written was in the `-wal` sidecar, so the assertion scanned a 4 KB header page and passed for a reason unrelated to privacy. **Drop the store, scan every file in the audit directory, and add a positive control** asserting a genuinely persisted field appears. The fix is written; do not rediscover it.

**The test that passes with the thing it tests removed.** Six occurrences so far. Every ablation here must break the property and watch the *specific* test fail — one ablation per property, and if the test still passes, the ablation is the defect, not the result.

**Recording an obligation where nobody reads it.** If you find something the next slice must handle, put it in that slice's entry in `task-breakdown-pr-plan.md`, not only in `qa-evidence.md`. Evidence files are where results go; scope entries are what implementers read.
