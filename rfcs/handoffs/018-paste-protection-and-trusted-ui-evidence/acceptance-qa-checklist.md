---
title: "RFC-018: Rendered Paste Protection and Trusted-UI Evidence - Acceptance / QA Checklist"
rfc: "RFC-018"
rfc_file: "../../done/018-paste-protection-and-trusted-ui-evidence.md"
status: "Accepted 2026-08-08 — not started"
target_milestone: "M9"
created: "2026-08-08"
---

# Acceptance / QA Checklist

Every unchecked line at closeout carries a stated reason. An absence with a reason is evidence; an absence without one is a gap.

## Ingress Checklist (PR-018-B)

- [ ] `TerminalInputPolicy` had **no production caller** before this slice, shown by enumeration rather than asserted.
- [ ] **Exactly one PTY ingress**, enumerated mechanically by call site and enclosing function name; a synthetic second call site fails the test.
- [ ] The enumeration is **ablated** — the test fails when a second ingress is introduced, and the failure names it.
- [ ] Modal exclusivity re-proven **with a real paste against a real `TerminalPane`**, not headless.
- [ ] No paste classification anywhere in `crates/tekstide`; every decision originates from `evaluate`.
- [ ] Each `TerminalPasteClass` exercised against real bytes: `Empty`, `SingleLine`, `Multiline`, `ControlContaining`.
- [ ] `ControlContaining` **blocks outright**, not confirms.
- [ ] The real `TerminalTrustedUiState` is passed, derived in one place, never hardcoded `Inactive`.
- [ ] The paste keybinding collides with no other rule, checked against the whole `linux_mvp()` table mechanically.
- [ ] `RequiresConfirmation` blocks and the user is told; the temporary state is recorded in `qa-evidence.md`.
- [ ] Clipboard read is bounded; a very large clipboard cannot become an unbounded write or render.

## Dialog Checklist (PR-018-C)

- [ ] Built on RFC-015's existing modal layer; **no dialog framework introduced**.
- [ ] **Every dismissal path defaults to not pasting** — Escape, click-away, focus loss, and any other non-accept exit, each tested individually.
- [ ] The accept path is the **only** thing that writes, and it goes through PR-018-B's single ingress.
- [ ] Pasted content rendered through `text_safety::quote_untrusted`.
- [ ] **A bidi/control case tested specifically**: a paste containing `\u{202E}` renders escaped and does not reorder the dialog's own text.
- [ ] Focus cycle demonstrated, with a byte-identical third screenshot proving the cycle returns.
- [ ] `NFR-UX-002`: accept/reject distinction is not colour-only.
- [ ] Every user-facing word goes through `Catalog`; no hardcoded English at the render layer.
- [ ] RFC-018 §Open questions 2 answered — preview or describe — with the reasoning recorded.

## Audit Checklist (PR-018-D)

- [ ] Conforms to the frozen `paste_blocked` family; **no schema amendment**.
- [ ] Written via `AuditCoordinator`, never `AuditStore` directly.
- [ ] Audit-write failure cannot fail the paste decision it observes.
- [ ] **Sentinel test**: no pasted content, clipboard text, or command text reaches the durable store.
- [ ] The sentinel scan **drops the store first**, scans **every file** in the audit directory, and carries a **positive control** proving the scan reaches real written content.
- [ ] Schema conformance ablated — a forbidden field is rejected by the store's own validation.
- [ ] The **confirmed-paste recording gap** stated: `outcome == Blocked` means approvals have no valid encoding.
- [ ] README privacy section re-checked against reality and fixed in the same commit if stale.

## Trusted-UI Evidence Checklist (PR-018-E)

- [ ] Genuine dialog screenshotted **over live, updating terminal output**.
- [ ] Terminal output drawing its best imitation of the dialog, screenshotted.
- [ ] The two side by side, with distinguishing features named **in prose**.
- [ ] The distinguishing property stated as a claim that **could be false**, not as an appeal to appearance.
- [ ] `NFR-UX-002`: the distinction is not colour alone.
- [ ] Evidence taken against a terminal opened by `Ctrl+Alt+T`, not an env-gated demo.
- [ ] **RFC-014 PR-014-D's spike screenshot is not cited.**
- [ ] Each screenshot states what it proves **and does not**.

## Honesty Checklist (PR-018-F)

- [ ] Closeout states what may be claimed about paste protection and trusted UI.
- [ ] The claim statement is checked **against the RFC's own text**, not only the evidence file.
- [ ] **No claim that terminal performance improved.** `NFR-PERF-004`, the three-terminal limit and the throughput ceiling are untouched by this RFC.
- [ ] No claim of semantic detection of dangerous pasted commands.
- [ ] The note to RFC-022: what was paste-specific, what looked general.
- [ ] Every unchecked line above carries a stated reason.

## Evidence Required

- [ ] Commit/PR list.
- [ ] Gate command output.
- [ ] Ingress enumeration and its ablation.
- [ ] Paste-class coverage results.
- [ ] Dismissal-path results, one per path.
- [ ] Sentinel privacy test result.
- [ ] The three trusted-UI artifacts.
- [ ] Known limitations, consolidated.
- [ ] Answers to the RFC's three open questions.

## Final Acceptance Decision

- [ ] Accepted
- [ ] Accepted with required follow-up
- [ ] Rejected

Reviewer notes:
