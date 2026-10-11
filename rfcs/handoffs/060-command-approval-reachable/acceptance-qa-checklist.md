---
title: "RFC-060 acceptance and QA checklist"
source_rfc: "../../accepted/060-command-approval-a-user-can-reach.md"
source_rfc_status: "Accepted by the human owner 2026-10-12"
---

# RFC-060 — acceptance and QA checklist

Every box here stays unchecked in this file: it is a per-slice template, not a record. A slice's
own run is recorded in its review request and in `qa-evidence.md`.

## PR-060-A — the way in

- [ ] A configuration file can produce a `Managed` profile that reaches `prepare_adapter_approval`
      and gets a real channel back — demonstrated, not inferred from types.
- [ ] A `Managed` profile **without** an adapter is unrepresentable, not merely rejected (§1). The
      review request says which it is and shows the attempt.
- [ ] A profile with no adapter declaration still produces exactly today's `Supervised` profile,
      with the existing tests unchanged.
- [ ] `managed_compatibility_level_without_structured_action_approval_is_still_rejected` still
      means something, or its new status is stated plainly.
- [ ] No UI changed in this slice.

## PR-060-B — the sentence and the sweep

- [ ] The dialog carries D3's sentence, and the wording guard asserts the text the view composes
      via a factored-out function.
- [ ] The sentence carries both halves: approval answers a request; a rejected command is not
      prevented from running.
- [ ] The `REQ-AGENT-013` sweep covered `locales/en.ftl`, `docs/src/users/what-works-today.md`,
      `docs/src/users/local-data-and-privacy.md`, `README.md` and the draft changelog entry.
- [ ] The sweep looked for the words that imply enforcement without saying "approval" —
      *protected*, *blocked*, *prevented*, *secured*, *requires approval before running* (§6).
- [ ] **The sweep's result is recorded even if it found nothing.**

## PR-060-C — the whole path, once, for real

- [ ] One real sequence between real processes: configured profile → adapter → proposal → dialog →
      decision.
- [ ] The resulting audit records are read back **through the audit view**, not by direct query.
- [ ] The reference adapter is named as a test-and-proof artifact everywhere it appears, including
      in any capture's sidecar (§4 of RFC-022's risk doc).
- [ ] Live capture of the dialog with D3's sentence visible; throwaway state only; checksum
      differs from any other capture in the same round.

## Whole-RFC

- [ ] Nothing in the product claims command-level enforcement (`REQ-AGENT-013`).
- [ ] Nothing describes the capability token as securing the channel (§4).
- [ ] A declared adapter that proposes nothing is not reported as an error anywhere (§3, D6).
- [ ] Nothing lets a rejection record be read as "the command did not run" (§5).
- [ ] `REQ-AGENT-012`, `REQ-SEC-012`, `REQ-SEC-013` are met for a profile that declares an adapter,
      and the changelog says for whom — not "command approval now works".
- [ ] No adapter for any real AI CLI was added (D8; that is RFC-069).
- [ ] RFC-021's protocol is unchanged (D5).
- [ ] `rfcs/handoffs/release-checklist.md` run in full for the candidate.

## Final Acceptance Decision

- [ ] Accepted.
- [ ] Accepted with required follow-up.
- [ ] Requires re-review after changes.

Reviewer notes:

```text
```
