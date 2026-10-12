---
title: "RFC-060 acceptance and QA checklist"
source_rfc: "../../done/060-command-approval-a-user-can-reach.md"
source_rfc_status: "Implemented and closed 2026-10-12; `0.36.0` candidate, not yet published."
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

- [x] **Accepted.**
- [ ] Accepted with required follow-up.
- [ ] Requires re-review after changes.

Accepted 2026-10-12 as the `0.36.0` candidate, across review rounds **535 through 541**. Every
required item raised in those rounds is closed; nothing is carried as a condition of acceptance.

Reviewer notes:

```text
What acceptance rests on -- the reviewer's own measurements, not the implementer's reports:

  * Full release-checklist gate re-run by the reviewer at review 541: fmt clean, clippy
    --workspace --all-targets --all-features -D warnings 0 diagnostics, cargo test --doc
    --workspace clean, 782 + 22 + 1140 with 0 failed, 0 fixture entries in a fresh short
    /dev/shm TMPDIR, the owner's real state directory at 0.
  * Both packages built and smoke-checked; the published-shape manifest pins
    tekstide-core = "0.36.0". cargo publish --workspace --dry-run --locked clean for both.
    Unpacked tekstide-core-0.36.0 with all [[bin]]s built first: 1139 passed, 1 failed,
    that one the documented real_repository_filesystem_scan_cost_headless_benchmark.
  * cargo audit: 0 vulnerabilities; three allowed warnings matching the register exactly.
  * D8/D9 checked by diff, not assertion: approval/protocol.rs and agent/profile.rs are
    untouched across the entire RFC, so the protocol did not change and no adapter was
    shipped for any real AI CLI.
  * The half-decision guard ablated by the reviewer in a worktree: supplying only the
    compatibility level for a planted third ConfiguredAiCliAdapter variant is now E0308,
    where at review 535 the same half-decision compiled clean with all 131 config tests
    green and produced Managed + structured_action_approval: false.
  * The socket-path refusal ablated by the reviewer: removing the new match arm fails the
    new test on exactly its own assertion, quoting the old generic message back.

Defects found in review and fixed before acceptance:

  * 535 -- a fixture leaked on every *passing* run (no guard at all), and the gate line
    claimed 0; measured 1, twice. Also a gate count that did not reproduce (1139 vs 1140).
  * 535 -- the two outputs were derived in two places from one field, so a third variant
    could pass through one and fall through the other. Demonstrated, then fixed to one
    match producing both.
  * 538 -- the approval socket's AF_UNIX path-length limit, known here only as test-fixture
    guidance, also binds a real launch's $XDG_STATE_HOME. The user saw "Couldn't start an
    agent run." while core knew the exact reason. PR-060-A had just made that reachable.
  * 539 -- the request claimed a documentation cross-link that did not exist.

Reviewer errors recorded with this acceptance:

  * D3 asked for a cooperative-not-enforced sentence on the dialog. That sentence had
    shipped in August under RFC-022 (2726e67, 7e9b553), two months before this RFC was
    proposed. The implementer checked, pushed back, and declined to add a redundant second
    catalog key -- correctly. I scoped the mechanism closely and inferred from "no user has
    seen this dialog" that its words had not been written; one grep of en.ftl would have
    shown otherwise, and I had done exactly that grep for RFC-059.
  * D4 asked for a sweep for *enforcement* claims. Every such claim was already correct;
    the two real findings were *reachability* claims that PR-060-A had falsified. The sweep
    found what the instruction did not ask for.

Deliberately out of scope, recorded rather than fixed:

  * An adapter for a real AI CLI -- RFC-069, reserved on this RFC's acceptance (D9).
  * A terminal-resize test whose poll condition (<= 20) is weaker than its assertion
    (== 20), disclosed at review 538 with an open verdict. test-process-leak.md.
```
