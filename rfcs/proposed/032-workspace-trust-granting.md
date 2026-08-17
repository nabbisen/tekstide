# RFC-032: Workspace Trust Granting

Status: **Accepted by the human owner 2026-08-17.** Both open questions below remain unanswered and **bind before implementation**, not during — see §Open questions.
Target milestone: M11
Date: 2026-08-17

Related baseline documents:

- `tekstide-security-threat-model-v0.md`
- `tekstide-requirements-v0.md`
- `tekstide-uiux-wireframes-v0.md`

Depends on:

- [RFC-004](../done/004-security-baseline-and-restricted-mode.md) — Restricted Mode, the nine
  blocked features, and the trust model this makes reachable.
- [RFC-013](../done/013-durable-audit-store-and-local-data-policy.md) — the audit store a
  trust grant is recorded in.
- [RFC-022](../done/022-adapter-spawn-and-command-approval-surface.md) — the agent-run chain
  this unblocks.

## Summary

Let a user grant workspace trust, and revoke it. Today they cannot, so **no project can ever
leave `Restricted`.**

## Why this is scheduled

`AuditCoordinator::grant_project_trust` is correct, tested, and records both `TrustGrant`
authorization and application. `ProjectSession::revoke_trust` exists beside it. **Neither has
a production caller**, and `crates/tekstide` contains no trust-granting anything.

Every project defaults to `Restricted` at `ProjectSession::new` and stays there for the life
of the installation. RFC-004's Restricted Mode is not a mode; it is the only state.

**The consequence is not theoretical.** RFC-022 PR-022-D found that `Ctrl+Alt+A` refuses for
every real user with `WorkspaceDiscoveryBlocked`, because a Claude Code profile honestly
declares `MayDiscoverWorkspaceFiles` and no project is ever trusted. The entire adapter-spawn
chain — built, reviewed and closed — is unreachable behind this one gap.

It was found seventh in a sequence of dormant capabilities and confirmed by the reachability
audit (2026-08-17). It is the highest-consequence one.

## What granting trust actually authorises

`RestrictedModeFeature` (`security.rs:5-15`) — nine things, and they share one shape:

| | |
| --- | --- |
| `WorkspaceAiProfileLoading` | AI CLI profiles from the workspace |
| `WorkspaceAiPromptLoading` | prompts from the workspace |
| `WorkspaceEnvironmentLoading` | environment from the workspace |
| `WorkspacePluginLoading` | plugins from the workspace |
| `WorkspaceCommandPaletteEntry` | palette entries from the workspace |
| `AutomaticTaskExecution` | tasks that run without being asked |
| `TekstideInitiatedGitHook` | git hooks Tekstide triggers |
| `AutomaticLspStartup` | language servers started automatically |
| `BackgroundProjectAutomation` | anything else running unprompted |

**In one sentence a user can evaluate: files inside this folder may configure Tekstide and
cause programs to run.** That is the grant. The nine-item list is not something a person can
reason about at a dialog; the sentence is.

## The security core

### 1. Trust granted before an agent runs is trust extended to whatever the agent writes

This is the decision that makes this RFC security-critical rather than a wiring task, and it
is not in RFC-004 — RFC-004 predates agent runs being reachable.

**An AgentRun writes files into the project.** So a user who trusts a folder and then runs an
agent in it has authorised the agent's own output to configure Tekstide and run programs. The
agent can write a workspace AI profile, an environment file, or a task definition, and trust
already covers it.

That is not an argument against trust. It is an argument that **the dialog must say what is
being trusted — the folder's contents, present and future — and not merely "this project."**
A user who reads "trust this project" thinks about the files they wrote. The grant covers the
files anything writes.

### 2. Trust is about a path, and paths are not stable

`RecentProjectAvailability::PathChanged` already exists, so this project already knows a
recorded path can stop pointing where it did. A trusted path that is later a symlink to
somewhere else is a trust grant redirected without the user's involvement.

State what trust is bound to — the canonical path, the project identity, or both — and what
happens when they diverge. **`ProjectSession` already carries `root_path` and
`canonical_root_path` separately**, which is the seam this decision lands on.

### 3. Revocation must be reachable, or the grant is one-way

`revoke_trust` exists. If granting is wired and revoking is not, this RFC creates a state
users cannot leave — which is the same defect it was written to fix, in the opposite
direction.

### 4. The dialog is not the paste dialog

RFC-018's modal exclusivity rules apply, but the consequence is different in kind. A paste
affects one terminal. **Trust affects everything the folder can do, for as long as it is
trusted.** The dialog must not read as a routine confirmation, and it must not default focus
to the granting action.

## Scope

1. **A route to the trust decision.** `ProjectOpenSurface::TrustSettings` already exists,
   dormant. Use it rather than inventing a variant — and note it will be the second real
   `open_surface`-conditional dispatch, after RFC-022's `ApprovalHistory`.
2. **Grant, through `AuditCoordinator::grant_project_trust`** — its first production caller.
   The audit records already exist and are correct; this gives them a producer.
3. **Revoke, through the same path.**
4. **The dialog copy**, per §Security core.
5. **The board reflects it** — `ProjectBoardRow::trust_label` already renders trust state and
   is one of the pre-rendered-English sites RFC-016 flagged.

## Non-goals

- **Per-feature trust.** Nine independent toggles is a settings panel nobody can reason
  about. One grant, all nine, stated plainly.
- **Automatic trust for any path.** No "trust everything under `~/src`" heuristic — that is
  how trust prompts become meaningless.
- **Changing what Restricted blocks.** RFC-004's nine features are unchanged.
- **Making trust a prerequisite for anything it does not already gate.**

## What RFC-004 already decides, and what it deliberately left open

Checked before asking, because a question answerable from the repository is not the owner's:

- **Decided (RFC-004 §2, line 64):** *"Trust must be explicitly granted by the user. Opening a
  folder must not imply trust."* This RFC does not reopen it.
- **Deferred (RFC-004 line 198):** *"GUI trust dialogs"* is listed among RFC-004's own
  out-of-scope items. **This RFC is the discharge of that deferral**, not a new idea.
- **Left open by RFC-004 itself (line 208):** *"Should Trusted state expire when project root
  Git remote changes?"* — an **invalidation** question RFC-004 raised and never resolved.
  Open question 2 below is its direct descendant: RFC-004 asked when trust should stop being
  valid, and never said what it was valid *for* in the first place.

So neither open question is answerable from the repository. Both are genuinely the owner's,
and the second one has been open since RFC-004.

**One interaction the answers must resolve together.** If trust persists, then re-opening a
previously trusted folder grants its capabilities without an explicit act *this session* —
which sits close enough to *"opening a folder must not imply trust"* that a reader will
wonder. My reading is that persistence remembers an explicit decision rather than inferring
one from the act of opening, and so does not violate the rule. **That reading needs stating
in the RFC rather than left implicit**, whichever way the owner decides.

## Decisions (answering the questions below)

**Both answered by the owner 2026-08-17. The decisions and their reasoning live in
[`docs/src/contributors/security-decisions.md`](../../docs/src/contributors/security-decisions.md)
— the canonical home — and are summarised here in one line each rather than restated, so
there is one wording to keep true.**

1. **Trust persists across sessions.** Accepted with its costs recorded: a trusted folder's
   contents can change afterwards, an agent's own output inherits the trust, and trust
   accumulates. Three requirements make that acceptable — revocation always available, trust
   state visible on the board, and the dialog naming the folder's contents *present and
   future*. All three are scope items, not intentions.
2. **Trust binds to the canonical path**, not the path as opened. A literal-path binding would
   let a redirected symlink inherit an existing grant silently; the canonical binding costs
   only re-granting after a legitimate move.

## The questions as originally posed

1. **Does trust persist across sessions?** Recent projects persist, so a per-session grant
   means re-granting on every launch — which trains users to click through it, the
   habituation failure RFC-022's arrival model was designed around. Persisting means a folder
   trusted once stays trusted while its contents change. **My recommendation is persist, with
   revocation reachable and the grant visible on the board** — but it is a product decision
   about how much the application remembers on the user's behalf.
2. **Is the grant bound to the path, the canonical path, or the project identity?** See
   §Security core 2. This is a security decision with a real failure mode, and I would rather
   the owner see it than have it settled by whichever field was convenient.

## Risks

- **A trust prompt users click through.** The dominant failure of every trust dialog ever
  shipped. Mitigated by rarity (once per project, persisted) and by wording that describes
  consequence rather than asking permission for an abstraction.
- **Trust laundering through an agent.** §Security core 1. Mitigated by disclosure, not
  prevented — nothing stops a trusted folder's contents changing.
- **Wiring grant without revoke**, creating a one-way state.
