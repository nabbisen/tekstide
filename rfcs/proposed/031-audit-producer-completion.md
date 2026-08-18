# RFC-031: Audit Producer Completion

Status: **Accepted by the human owner 2026-08-18.** Scoped to `restricted_mode_blocked` and `project_added`; `safe_close_decision` stays blocked on a surface that does not exist.
Target milestone: M11 — the last M11 item
Date: 2026-08-18

Related baseline documents:

- `tekstide-security-threat-model-v0.md`
- `tekstide-requirements-v0.md` — `REQ-SEC-014`

Depends on:

- [RFC-013](../done/013-durable-audit-store-and-local-data-policy.md) — the store, the frozen
  schema, and the twelve families. **Frozen means this RFC adds producers, not families.**
- [RFC-004](../done/004-security-baseline-and-restricted-mode.md) — what Restricted Mode
  blocks, which is what `restricted_mode_blocked` would record.

## Summary

Give the audit families that record nothing a producer, or say why they should not have one
yet.

## Why this is scheduled

`REQ-SEC-014` is the requirement this serves, and every release since `0.1.0` has shipped a
statement of the form *"these producers are defined in the audit schema but not yet wired."*
That statement is currently in both crate READMEs and on two crates.io pages.

An audit family that records nothing is not neutral. It is a **schema commitment the product
has not kept**: the store is queryable, the family is enumerable, and a reader — a user, or a
future feature — cannot distinguish "this never happened" from "nothing was ever recorded."
That is the same conflation this project has now refused twice, in the conflict-vs-external-change
dialog and in truncated-vs-clean change detection.

## The three are not equal, and the reservation's title implies they are

The number was reserved for `safe_close_decision`, `restricted_mode_blocked` and
`project_added`. Checked before writing:

| family | trigger in the shipped product | producer method |
| --- | --- | --- |
| `RestrictedModeBlocked` | **live** — `Ctrl+Alt+A` on an untrusted project refuses with `WorkspaceDiscoveryBlocked` (`shell.rs:1761`) | none |
| `ProjectAdded` | **live** — opening a project from the CLI or the board | none |
| `SafeCloseDecision` | **none** — `OpenSafeCloseDialog` maps to `None`; the dialog does not exist | none |

**No producer method exists for any of the three.** This is not a wiring slice: `record_paste_blocked`,
`record_plain_terminal_started` and the rest exist as coordinator methods; these have no
equivalent. Both the producer and its call site are this RFC's work.

**Recommend scoping this RFC to the two with live triggers**, and treating `safe_close_decision`
as blocked on a surface that does not exist. Building an audit producer for an event nothing can
cause would reproduce, in the audit layer, exactly the zero-reachable-surface failure this
project has a standing rule against.

## The security core — `summary` is free text, and that is where this goes wrong

`AuditEvent` carries `summary: String`. Every family already wired observes the same discipline,
and the READMEs make it a public promise: an event records **that** something happened, never
its content. `plain_terminal_observation` "names only whether the process exited or was
signalled — never a command, its output, or a path." `paste_blocked` "names only that a paste
was blocked and which project/terminal it was aimed at — never the pasted content."

Both new producers are more dangerous than they look:

- **`project_added` naturally wants the project path.** A path is untrusted,
  attacker-influenceable text — this project escapes it at every widget for exactly that reason
  — and writing it into a durable, append-only store is a different act from rendering it. The
  store is not escaped on read.
- **`restricted_mode_blocked` naturally wants to say what was blocked.** RFC-004 blocks nine
  features; naming which one is genuinely useful to a user asking "why did my run refuse?" But
  the refusal often carries a path, an executable name, or a profile identifier, and the useful
  version of this event is one step away from the leaky one.

**Decide the summary content explicitly, per family, and state the rule the way the READMEs
already state it for the wired families.** A producer whose summary is built by formatting
whatever error was in hand is how a store that promises to hold no paths ends up holding paths.

## Scope

1. A coordinator producer for `RestrictedModeBlocked`, called from the real refusal path.
2. A coordinator producer for `ProjectAdded`, called where a project is really added.
3. The public statement in both READMEs updated to name what is now wired — and what is not.

## Non-goals

- **`safe_close_decision`.** Blocked on a surface; see above. Its `SafeCloseTerminate` /
  `SafeCloseAbandon` action kinds stay unused and that stays disclosed.
- **`SensitiveConfigChanged`** — RFC-023's.
- **`TranscriptPurge`** — RFC-033's, which should wire it as part of building purge.
- New families. RFC-013 froze the schema; if one of these does not fit, that is a finding to
  report, not a schema change to make here.

## Decisions required

**D1 — what `restricted_mode_blocked`'s summary says.** Recommend the blocked *feature class*
from RFC-004's own vocabulary and nothing derived from the attempt — no path, no executable, no
profile id. A user learns "workspace discovery was blocked"; the store learns nothing it would
be embarrassing to leak.

**D2 — does `project_added` record a path at all?** Recommend **no path**, only the project id
and that an add occurred. The recent-projects file already holds paths in plaintext and is the
honest place for them; the audit store's value is the immutable sequence of security-relevant
acts, not a second copy of the user's directory layout.

**D3 — is a *blocked* event recorded every time, or once?** A user hammering `Ctrl+Alt+A` on an
untrusted project would generate an event per press. Recommend recording every occurrence —
suppression is a policy that hides evidence — but check the retention interaction, because
RFC-013's store has bounds and a repeated refusal is the cheapest way to reach them.

## Risks

- **A summary that leaks.** The whole §Security core. Mitigated by D1/D2 and by a test that
  asserts the *absence* of a path in a produced record, not merely the presence of the event.
- **Producing events nobody can read.** `OpenApprovalHistory` was unreachable for a release;
  the audit store has no user-facing view at all. This RFC does not create one, and should say
  so rather than implying that recording an event makes it visible.
