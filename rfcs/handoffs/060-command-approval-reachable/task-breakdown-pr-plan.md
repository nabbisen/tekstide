---
title: "RFC-060 task breakdown and PR plan"
source_rfc: "../../accepted/060-command-approval-a-user-can-reach.md"
---

# RFC-060 — task breakdown and PR plan

Three slices. **PR-060-B is not optional polish**: the plumbing without the claim is the half that
does harm.

## PR-060-A — the way in

**No UI in this slice.**

1. **Decide the configuration shape first, and write it down before writing it.** The constraint
   is §1: a `Managed` profile must be *unrepresentable* without an adapter, not merely rejected.
   One declared adapter implies the level and the capability; it is not three fields that must
   agree.
2. `to_ai_cli_profile` stops being unconditional. A profile with no adapter declaration stays
   exactly as it is today — `Supervised`, `structured_action_approval: false` — and that path
   must keep its existing tests passing unchanged.
3. **Prove the impossibility, do not assert it.** A test that tries to express the dangerous
   combination and cannot is worth more than one that expresses it and checks the error. If the
   chosen shape makes it a compile error, ablate it in a worktree and show the error.
4. The existing guard
   (`managed_compatibility_level_without_structured_action_approval_is_still_rejected`) stays and
   must still mean something afterwards — if the new shape makes it unreachable, say so plainly
   rather than deleting it quietly.
5. **An end-to-end launch**: a real configuration file producing a `Managed` profile that reaches
   `prepare_adapter_approval` and gets a real channel back. Until that exists, nothing in this RFC
   has been demonstrated.

## PR-060-B — the sentence and the sweep

1. **D3.** One sentence on the dialog, in `en.ftl`, that a user reads before approving. It has to
   carry: approval answers a request; Tekstide does not prevent a rejected command from running.
   The wording is argued in the review request, against §2 of
   `what-the-dialog-must-not-lie-about.md` — *"the highest-consequence sentence in this RFC."*
2. **The wording guard asserts what the view composes**, through the factored-out function, not a
   string assembled in the test. (RFC-058 review 518's shape; RFC-059 followed it and caught a
   real Fluent-argument bug with it.)
3. **D4, the sweep.** Every place the product describes approval: `locales/en.ftl`, both book
   pages, `README.md`, the changelog entry this release will carry. Check each against what is
   true. §6 lists the dangerous words that do not contain "approval".
4. **Record what the sweep found, including "nothing".** A sweep that reports no findings is a
   result; a sweep that is not mentioned is indistinguishable from one that did not happen.

## PR-060-C — the whole path, once, for real

1. A configured profile, a real adapter process (the reference adapter), a real proposal, the real
   dialog, a real decision — one sequence, between real processes.
2. **Read the audit records back through `0.35.0`'s own audit view**, not by direct query. This is
   the first release where that is possible, and it is the best available check that the two
   features agree about what happened.
3. **Name the reference adapter as what it is**, in the review request, in `qa-evidence.md`, and in
   any capture's sidecar: a test-and-proof artifact that speaks a protocol this project invented,
   which no shipping AI CLI implements. §4 of RFC-022's risk doc, and the reason it exists.
4. Live capture of the dialog with D3's sentence visible, throwaway state only.

## The changelog, when the candidate comes

This release's entry is where `REQ-AGENT-013` is easiest to breach, because the natural way to
describe this work is *"command approval now works"*. What is true is narrower:

- the approval **UI and API** are now reachable, for a profile that declares an adapter;
- no shipping AI CLI implements the protocol, so for most users nothing changes;
- rejecting does not prevent execution, and nothing in this release changes that.

Write the entry so that a reader who stops after the first sentence has not been misled.
