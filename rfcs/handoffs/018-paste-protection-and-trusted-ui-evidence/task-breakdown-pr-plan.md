---
title: "RFC-018: Rendered Paste Protection and Trusted-UI Evidence - Task Breakdown / PR Plan"
rfc: "RFC-018"
rfc_file: "../../proposed/018-paste-protection-and-trusted-ui-evidence.md"
status: "Accepted 2026-08-08 — ready for implementation"
target_milestone: "M9"
created: "2026-08-08"
---

# RFC-018 Task Breakdown

Six slices. **PR-018-B is the security-critical one and has its own document** ([`pr-018-b-paste-ingress.md`](./pr-018-b-paste-ingress.md)) — read it before writing code.

## PR-018-A — Design and handoff acceptance

Granted 2026-08-08 with the RFC. Nothing to implement.

## PR-018-B — Paste ingress

Scope: a real clipboard read wired to `TerminalInputPolicy::evaluate`; `Allow` writes through the existing single call site; `Block` writes nothing; `RequiresConfirmation` blocks conservatively until C.

Review gate: see [`pr-018-b-paste-ingress.md`](./pr-018-b-paste-ingress.md). The short form —

- **One PTY ingress**, enumerated mechanically and ablated.
- Modal exclusivity re-proven **with a real paste against a real pane**.
- No classification in `crates/tekstide`.
- Every `TerminalPasteClass` exercised against real bytes.
- The real `TerminalTrustedUiState` passed, not `Inactive`.
- `RequiresConfirmation` blocks **visibly**, and that temporary state is recorded.

## PR-018-C — The confirmation dialog

Scope: `RequiresConfirmation` renders on RFC-015's modal layer; the user's answer is the only thing that releases bytes.

**Build on the existing modal layer. Do not generalise into a dialog framework** — RFC-018 decides this, and the reasoning is PR-015-D's: one implementor gives nothing to generalise from. RFC-022 is the second implementor.

Review gate:

- **Dismissal defaults to not pasting.** Escape, click-away, focus loss, and any other exit that is not an explicit accept must leave the PTY untouched. Test each exit path, not one representative.
- **Pasted content in the dialog goes through `text_safety::quote_untrusted`.** RFC-016's grid exception does not reach chrome. **Test a bidi/control case specifically** — a paste containing `\u{202E}` must render escaped, not reorder the dialog's own text.
- **The accept path is the only thing that writes**, and it still goes through PR-018-B's single ingress rather than a new one.
- Focus cycle demonstrated across the dialog's controls, with the byte-identical third screenshot proving the cycle returns — PR-015-C's convention.
- `NFR-UX-002`: the accept/reject distinction is not colour-only.
- **RFC-018 §Open questions 2 answered**: preview the pasted content, or only describe it. Decide with the escaping already in place, so the decision is about usefulness rather than risk, and record which and why.

**Both items carried forward into this slice are discharged, implemented 2026-08-10 -- neither is open work for PR-018-C's reviewer to re-raise:**

1. **`trusted_ui_state` (`shell.rs`) now maps the real dialog to `PasteConfirmationActive` specifically.** `trusted_ui_state`'s match is on the `ModalContent` variant directly: `PasteConfirmation(_) => PasteConfirmationActive`, `LayerDemo { .. } => SecurityDialogActive` (the placeholder keeps the generic mapping, since it represents no real dialog kind of its own).
2. **The "possibly-truncated preview" concern turned out to be moot**, as the note itself predicted: `RequiresConfirmation` is unreachable for anything over 256 KiB (refused as `TooLarge` before `evaluate` ever runs), so the dialog's content is always the user's real, complete paste. No disclosure about partial previews was needed.

## PR-018-D — The `paste_blocked` audit producer

Scope: wire the family that exists in the frozen v1 schema with no producer.

Review gate:

- Conforms to the frozen family: `action_kind == TerminalPaste`, `actor_kind == AppPolicy`, `action_source == PolicyEngine`, `reason_code == Some(PastePolicy)`, `outcome == Blocked`. **No schema amendment.** If one seems needed, that is RFC-013's and needs the owner.
- Written via `AuditCoordinator`, never `AuditStore` directly.
- **Sentinel test**: no pasted content, no clipboard text, no command text reaches the durable store. **Drop the store before scanning, scan every file in the audit directory, and include a positive control** asserting a genuinely persisted field is present. PR-017-F's first version read `database_file()` on an open WAL-mode store and scanned a page the write had never reached — the assertion passed for a reason unrelated to privacy. Do not repeat it.
- **The confirmed-paste gap stated**: `outcome == Blocked` means a paste the user *approves* has no valid encoding in this family, so the store records refusals only. Record it as a known limitation; do not amend.
- **Check whether the README's privacy section is still true.** It has been wrong three times across RFC-017. This slice adds a second producer; verify rather than assume, and fix in the same commit if it is stale.

## PR-018-E — Trusted-UI evidence

Scope: screenshot-backed evidence that the genuine dialog is distinguishable from terminal output imitating it.

**The adversarial condition no longer needs env vars.** `Ctrl+Alt+T` opens a real terminal, so the evidence can be taken against a terminal a user genuinely opened, with real output updating behind a real dialog. A modal over a frozen terminal is much weaker evidence than a modal over a live one.

Review gate:

- **Three artifacts**: genuine dialog over live output; terminal output drawing its best imitation; the two side by side with distinguishing features named **in prose**.
- **The distinguishing property stated as a claim that could be false.** "The dialog composites above the grid and the grid cannot draw outside its pane bounds" is checkable. "The dialog looks different" is not.
- `NFR-UX-002`: whatever distinguishes them is not colour alone.
- **RFC-014 PR-014-D's spike screenshot is not cited.** Six slices and counting.
- Each screenshot states what it proves **and does not**.

**Discharged, implemented and captured 2026-08-10 — nine screenshots against `evidence/pr-018-e/`, detailed in `qa-evidence.md`.** Approved 2026-08-10 (response 175) with one required correction, applied without recapturing: the first draft's spatial claim compared a pane boundary from one launch (`04`) against a dialog from a different launch (`01`), the same cross-image-comparison error this RFC had already made twice in the other direction (responses 173, 174). Corrected within each artifact's own geometry, the finding reverses — occupying chrome depends on pasted-content width (`01`'s short paste stays inside the pane; `05`'s longer hostile paste crosses it) and is not something a user can rely on. **The distinguishing claim now rests on one property, not two**: keystroke suppression while the dialog is open, behavioural, user-checkable, backed by a positive control (`Tab` moving the focus marker) proving keystrokes were reaching the app at the moment they went missing from the grid. Confinement itself is still sound and unconditional — `04` alone shows the grid can never draw outside its own pane — but whether the *genuine dialog* visibly uses that headroom is content-dependent and is reported as an observation, not a claim.

**Carried forward into PR-018-F: the dimming recommendation is stronger than originally flagged, not weaker.** A background scrim would be content-independent — unlike the spatial tell, which an attacker can suppress by inducing a short paste. Response 173's suggestion was deliberately not implemented in this slice (evidence work should not also change what it evidences). PR-018-F should decide whether to recommend it, with `01` vs. `05`'s reversal as the argument for why the current spatial-only backing is not enough on its own.

## PR-018-F — Closeout

Scope: checklist, QA evidence, known limitations, answers to the RFC's open questions, and an explicit statement of what may be claimed.

Review gate:

- The claim statement survives the honesty test, and is **checked against the RFC's own text**, not only against the evidence file. RFC-017's closeout passed its own gate while the RFC still asserted two things the verdict had made false — that check is now part of the pattern.
- **The note to RFC-022**: which parts of the paste dialog were paste-specific and which looked general. RFC-022 should start from evidence rather than a guess, and this is the only slice positioned to say.
- Known limitations consolidated, including the confirmed-paste audit gap and anything PR-018-B recorded as temporary.

**Carried forward from PR-018-D (response 171): there are two audit-observability gaps, not one, and they share a root cause.** RFC-018 named the first — `outcome == Blocked` means a paste the user later *approves* has no valid encoding. PR-018-D creates a second: an over-cap paste (`TooLarge`) leaves **no durable record at all**, because it never reaches `evaluate` and so has no real `TerminalInputDecisionReason` this family's fixed `reason_code` could honestly attach to it. Both gaps trace to the same cause — `paste_blocked` fixes a single `reason_code`, so it can only describe policy refusals, not the surrounding decisions (a confirmed paste, or a paste that never reached the policy at all). Record them together, with the shared cause named, so the owner sees one coherent question — *should paste observability be widened?* — rather than two unrelated footnotes. Still no schema amendment without the owner.

## Sequencing

**B → C is strict** — a dialog with no ingress renders nothing, and an ingress that treats `RequiresConfirmation` as `Allow` is the unsafe state this ordering exists to prevent. D needs B. E needs C. F needs all.

```
A ─→ B ─┬─→ C ─┬─→ E ─→ F
        └─→ D ─┘
```
