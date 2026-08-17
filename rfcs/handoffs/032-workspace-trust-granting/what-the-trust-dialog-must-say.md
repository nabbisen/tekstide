---
title: "RFC-032 — What the trust dialog must say: implementation handoff"
rfc: "RFC-032"
rfc_file: "../../done/032-workspace-trust-granting.md"
status: "Discharged — every item below is implemented and evidenced; see qa-evidence.md's PR-032-D section"
created: "2026-08-17"
---

# The most consequential single click in this application

Approving a command authorises **one command**. Granting trust authorises **everything a
folder can do, for as long as it stays trusted** — and, since trust now persists, across
every future session too.

There is no larger grant in Tekstide, and there is no undo beyond revoking afterwards.

## 1. The dialog renders a path, and that is the live attack

**This is the same finding as RFC-022's, in a worse place.**

Response 221 established that `cwd` — not `argv` — was the sharper target in the approval
dialog, because a user reads the *command* carefully and reads the *directory* to confirm
context, and a skim-check is exactly what a rendering attack aims at.

**A trust dialog is almost entirely a path.** Its whole content is *"do you trust
`/home/you/work/thing`?"* And a directory name is attacker-influenceable: a repository can
contain a directory whose name carries a bidi override, so a folder that **displays** as
`/home/you/work/safe-project` can be something else entirely.

If a user grants trust to a path that renders as one thing and resolves to another, they have
authorised program execution in a folder they never looked at.

**So:**

- **Escape the path at the widget**, with `text_safety::quote_untrusted`/`DisplayText`, the
  same primitive every other untrusted-text site uses. Do not add a second.
- **Render the canonical path**, since that is what trust binds to
  (`docs/src/contributors/security-decisions.md`). Showing the path as typed while recording
  the resolved one would mean the user approves one string and the system stores another.
- **Show both when they differ.** `ProjectBoardRow` already carries a `secondary_path_hint`
  for exactly this case. A symlinked project should say so at the moment of trusting, not
  only on the board.

**Evidence owed:** a project whose directory name contains a bidi override renders it visibly
as an escape marker, stated as a falsifiable claim. No double-escaping. Ablate the escaping
and show the specific rendering difference.

## 2. Focus must not default to granting

RFC-018's paste dialog defaults focus to **Cancel**, so a stray keystroke does the safe thing.
That convention exists and this dialog inherits it — but the safe thing here is *not
granting*, and the asymmetry is larger.

A mistimed Enter on the paste dialog cancels a paste. A mistimed Enter here would authorise
program execution in a folder, permanently.

**Default focus to the non-granting action. Granting requires moving focus and then
activating** — two deliberate acts, never one.

## 3. Say what is trusted, in the words the decision page uses

**"Files inside this folder may configure Tekstide and cause programs to run."**

That sentence is canonical — it is in
[`docs/src/contributors/security-decisions.md`](../../../docs/src/contributors/security-decisions.md)
and it is what the page means by trust. Use it, or improve it and change the page too; do not
let a second, weaker wording exist alongside it.

**Do not enumerate the nine features in the dialog.** Nobody weighs a nine-item list at a
decision point, and a list invites the reader to believe they have understood something they
have not.

## 4. Say "present and future," because trust outlives the reason for it

A user reading *"trust this project"* thinks about the files they wrote. The grant covers
files **anything** writes — including an AI agent's own output, which lands in that folder
and is trusted from then on, in this session and every session after.

The dialog must name that. It is the one consequence a reasonable person would not infer, and
it is the reason this RFC is security-critical rather than a wiring task.

## 5. Revoking must be as reachable as granting

`revoke_trust` exists. If granting is wired and revoking is not, this RFC creates a state
users cannot leave — the same defect it was written to fix, pointing the other way.

**Not "reachable in principle."** If granting takes one action from the project board and
revoking takes three from a surface nobody visits, revocation is decorative. Prove they are
comparably reachable, and say by what path.

## 6. What the dialog may not claim

- **Not that trusting is safe.** It authorises execution; whether that is wise depends on the
  folder, which Tekstide cannot assess.
- **Not that Tekstide will police what runs.** Nothing here intercepts execution — the same
  cooperative limit RFC-021 and RFC-022 already state.
- **Not that trust can be withdrawn from something already run.** Revocation stops future
  loading; it does not undo what a trusted folder already did.

That last one is easy to imply by omission, and it is the one a user would most want to be
true.
