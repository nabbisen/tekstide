# RFC-031: Audit Producer Completion

Status: **Implemented and closed 2026-08-19.** `restricted_mode_blocked` and `project_added`
have real producers with real production callers, each proven from the path a user actually
takes — `Ctrl+Alt+A` on an untrusted project, and opening a project from the CLI. **Does not
claim** that a user can see any of it: nothing renders the audit store, and recording an event
does not make it visible. **Does not claim** to say *which* of RFC-004's nine restricted
features blocked a launch — one `RestrictedMode` reason code carries no such field, and finer
granularity would be a frozen-schema change. `safe_close_decision` remains unwired and
unreachable, as scoped. Accepted by the human owner 2026-08-18; see
[the handoff pack](../handoffs/031-audit-producer-completion/README.md) for the full evidence.
Target milestone: M11 — the last M11 *audit* item. **Corrected 2026-08-19**: this said "the
last M11 item" outright, which was false when written. RFC-033 is also M11, and RFC-020 is M10
slipping to M11; both were open the day this closed.
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

## The security core — corrected 2026-08-19, before any handoff derived from it

**The first draft of this section (2026-08-18) was wrong and is replaced.** It said the danger was the
free-text `summary` field, and recommended careful wording. Checked against the code
afterwards: **`DurableAuditRecordV1` — the record actually written to the store — has no
free-text field at all.**

```rust
pub struct DurableAuditRecordV1 {
    project_id: Option<ProjectId>,      family: AuditEventFamily,
    outcome: AuditOutcome,              action_kind: AuditActionKind,
    subject_kind: Option<AuditSubjectKind>,  subject_ref: Option<AuditReference>,
    risk_level: Option<AuditRiskLevel>, reason_code: Option<AuditReasonCode>,
    actor_kind: AuditActorKind,         action_source: AuditActionSource,
    /* ids and a timestamp */
}
```

Every field is a typed enum or an id. `AuditEvent.summary` exists on the older RFC-002 domain
type, not on what RFC-013 persists.

**RFC-013 made this leak-resistant by construction, and that is the stronger property.** The
one string-shaped field is a validated newtype:

```rust
pub fn new(value: impl Into<String>) -> Option<Self>   // AuditReference
// non-empty, bounded length, and only [A-Za-z0-9-_.:]
```

**A filesystem path cannot be stored**: `/` is not in the permitted set, so
`AuditReference::new("/home/u/project")` returns `None`. This is the same design as
`DisplayText` in `text_safety` — the mistake is not expressible in a value the API accepts,
which beats any amount of careful wording.

### What remains, and it is narrower and real

Two things the type system does *not* decide:

1. **`subject_ref` accepts a single path segment.** `my-project` passes the charset check.
   So does `..`, and so does a directory name chosen to be confusing. The newtype restricts
   the *character set*, not the *meaning* — it prevents a path, not untrusted text.
   **Recommend: `project_added` carries `project_id` (a generated id) and leaves
   `subject_ref` as `None`.** The display name is attacker-influenceable and belongs nowhere
   in an append-only store that is not escaped on read.

2. **`reason_code` is a closed enum and already contains `RestrictedMode`.** So
   `restricted_mode_blocked` has its answer without inventing anything — which also means
   the useful-but-leaky version I worried about (naming the executable, the profile, the
   path) is not reachable through this schema even if someone wanted it. The remaining
   question is only whether the *feature class* deserves finer granularity than one
   `RestrictedMode` code, and **that would be an RFC-013 schema change**, which is frozen.
   Recommend using the existing code and recording the coarseness as a limitation.

**Why this correction is recorded rather than silently replacing the draft**: the original
reasoning would have produced a handoff whose security-critical document was about wording
discipline for a field that does not exist. That is a scoping error of the same class this
project keeps catching — asserting about code without reading it — and the correction is
cheap only because it happened before the pack was written.

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

**D1 — `restricted_mode_blocked`'s typed fields.** `reason_code: RestrictedMode` already
exists. Decide `action_kind` (`RestrictedFeature` is the obvious one), `outcome`, and whether
`subject_kind`/`subject_ref` carry anything at all. **Recommend they do not** — see the
security core. Record that one `RestrictedMode` code cannot distinguish which of RFC-004's
nine blocked features fired, and that finer granularity would be a frozen-schema change.

**D2 — `project_added` carries `project_id` and no `subject_ref`.** The generated id
identifies the project without reproducing the user's directory layout in an append-only
store. The recent-projects file already holds paths in plaintext and is the honest place for
them.

**D3 — is a *blocked* event recorded every time, or once?** A user hammering `Ctrl+Alt+A` on an
untrusted project would generate an event per press. Recommend recording every occurrence —
suppression is a policy that hides evidence — but check the retention interaction, because
RFC-013's store has bounds and a repeated refusal is the cheapest way to reach them.

## Risks

- **Untrusted text reaching `subject_ref`.** Not a path — the newtype rejects `/` — but a
  single attacker-influenceable path segment would pass. Mitigated by D1/D2 leaving it `None`,
  and by a test asserting the *absence* of a `subject_ref` on both records rather than only
  the presence of the event.
- **Producing events nobody can read.** `OpenApprovalHistory` was unreachable for a release;
  the audit store has no user-facing view at all. This RFC does not create one, and should say
  so rather than implying that recording an event makes it visible.
