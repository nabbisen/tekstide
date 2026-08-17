# RFC-032: Workspace Trust Granting

Status: Proposed
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

## Open questions for the owner

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
