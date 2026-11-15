---
title: "Audit: has this project stepped inside another product's security boundary anywhere else?"
status: "Audit output, 2026-11-15, ordered by the owner after RFC-069's D1 was withdrawn. Findings only."
rfc_file: "none — audit output"
target_milestone: "M14"
created: "2026-11-15"
---

# Boundary audit

RFC-069's D1 was withdrawn because it proposed taking over another product's security
responsibility while holding none of the control over it, on a reading of `REQ-AGENT-013` that
turned a prohibition into a mandate. The owner asked whether the same mistake exists elsewhere.
**This is what was checked and what was found.**

## The error, stated precisely enough to search for

Two distinct failures ran together in RFC-069, and they are searched for separately:

1. **A prohibition read as a mandate** — a requirement of the form *"must not X unless Y"* treated
   as *"achieve Y"*, when it is satisfied by not doing X.
2. **Taking responsibility for behaviour this product does not control** — making another
   product's internals load-bearing for a claim this product makes.

## 1. Prohibitions-with-conditions: four exist, one spawned work

Every requirement phrased as a prohibition with an escape clause:

| Requirement | Satisfied by | Verdict |
| --- | --- | --- |
| `REQ-EDIT-007` — must not follow symlinks outside the root without explicit permission | **Refusal.** Searched for a permission-grant path (`allow`/`permit`/`grant`/`consent` near symlink handling): none exists. The product refuses and fails closed. | Correct |
| `REQ-AGENT-010` — must not auto-commit/push/release without explicit user action | Not doing it | Correct |
| `REQ-GIT-007` — must not auto-commit/push/rebase without explicit user action | Not doing it | Correct |
| `REQ-AGENT-013` — must not claim command-level enforcement unless launched through an adapter **or wrapper** that reports | **This one.** The `unless` clause was built instead of the `must not` being honoured. | **The error. Withdrawn.** |

**Three of four were handled correctly, by inaction.** The fourth is the only one whose escape
clause named a thing that could be built, which is exactly why it was the one that went wrong.

## 2. Declared non-goals: none has been built

Checked across `rfcs/done/` and `rfcs/accepted/`. Most non-goals are *"not in this RFC"* scoping
later delivered deliberately by the RFC that owned them — not drift. The **permanent posture**
non-goals are intact: VM/container-grade isolation (RFC-004, RFC-009), fully sandboxing arbitrary
shells (RFC-004), malware detection (RFC-004), a complete IDE feature set (RFC-001). Nothing in
the product implements or claims any of them.

## 3. Other dependencies on a third party's behaviour: one, handled correctly

**Git configuration isolation.** `spawn_git_command` sets `GIT_CONFIG_GLOBAL`/`GIT_CONFIG_SYSTEM`
to `/dev/null` and `GIT_CONFIG_NOSYSTEM=1` so a read sees only the repository's own configuration.
That is another product's internals being controlled — but through **its own documented
environment interface**, used **defensively, to read less**, not to take over a responsibility.

And when the contract proved insufficient, the response was right: review 621 measured that
Apple's git reads a gitconfig bundled in the Xcode toolchain that `GIT_CONFIG_SYSTEM` does not
suppress, and the answer was **per-platform measurement plus disclosure**, plus a standing
release-checklist item that re-verifies it per platform rather than assuming. That is the opposite
of RFC-069's failure: a dependency on someone else's contract, discovered to be partial, and
answered by narrowing the claim rather than by building more machinery.

## 4. Claims this product makes about a third party: one, and it errs safe

The built-in `claude-code` profile declares
`AiCliWorkspaceDiscoveryPolicy::MayDiscoverWorkspaceFiles` with the summary *"Claude Code reads
files in the project workspace as part of normal operation."* Its own doc comment reasons that
claiming otherwise *"would be the same kind of dishonest disclosure this project's dialogs are
built to avoid"*, and names the consequence: a launch through this profile is **refused** in a
Restricted project until the user grants trust — *"not a bug to route around."*

That is over-disclosure producing a stricter gate. The opposite error from RFC-069's.

## 5. A mitigation a document promised: it exists, and in a better form than promised

RFC-004's own risk register says *"Users may expect full sandboxing. Mitigation: explicit non-goals
and clear labels."* **Clear labels live in the product, not in an RFC**, so the audit checked for
them. There is no disclaimer string in the catalogue — and the product does something stronger
instead: the Project Board **enumerates what Restricted mode blocks**, by name, nine capabilities
("automatic LSP startup, workspace AI profile loading, … background project automation"), visible
in `0.40.0`'s own Run It evidence. A named list is harder to over-read than a reassurance.

## The finding that matters most, and it is about the reviewer

**The discipline RFC-069 needed was already being applied elsewhere, by the same reviewer, in the
same week.** The M14 security review's Part 2, finding **F7**, refused to treat the threat model's
unadopted "audit export" sub-topic as a gap:

> *"§13 listed a sub-topic the project never adopted… not a gap to close by reflex, and listing it
> as one would be the review inventing work the document never justified."*

That is precisely the reasoning RFC-069 required. **The principle was held and not applied.** So
the corrective is not a new rule — it is to apply the existing one at the point where scope
language is first read as motivation, which is the scoping turn, not the fourth slice.

## What this audit did not do

Re-verify that any control works. Examine dependencies at the library level (`iced`,
`alacritty_terminal`, `rusqlite`) — those are code this project links, not boundaries it reaches
across. Look beyond requirements, declared non-goals, and the places the product names another
product by name.
