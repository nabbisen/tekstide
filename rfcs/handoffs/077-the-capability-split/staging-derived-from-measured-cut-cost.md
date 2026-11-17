---
title: "RFC-077 — staging, derived from measured cut cost"
rfc: "RFC-077"
rfc_file: "../../accepted/077-the-capability-split.md"
source_rfc_status: "Accepted 2026-11-03 by inheritance, split out of RFC-074 on 2026-11-12 — RFC-077 is in rfcs/accepted/"
target_milestone: "M14"
created: "2026-11-15"
---

# Staging, derived from measured cut cost

RFC-077's D8 says staging order is derived from measured cut cost, and its *Acceptance* defers the
enumeration until RFC-074's gate has behaved on two real stages. It has: Stages 1 and 2 landed in
`0.40.0`, each lowering the bound in the same commit as the move, and the gate was ablated at every
bound change. **This is that enumeration.**

Measured 2026-11-15 against `87de992`, with RFC-074's own method — production `.rs` under
`crates/tekstide-core/src`, `tests.rs`/`*_tests.rs`/`tests/` and `bin/` excluded, full-line
comments stripped, one reference per `crate::<module>` occurrence naming another top-level module.
Starting point: **largest SCC 8** — `agent`, `approval`, `audit`, `content`, `project`, `runtime`,
`security`, `transcript` — with 27 edges and 61 references inside it.

## The derived path: the original derivation (stages 7-9 superseded 2026-11-16)

Each row is the cheapest cut (or cheapest coherent group, where no single cut shrinks the
component) that lowers the bound by one, computed by removing the edge and re-deriving the largest
component:

| Stage | Bound | Cut | Refs | Where |
| --- | --- | --- | --- | --- |
| 3 | 8 → 7 | `security -> project` | **1** | `security.rs` |
| 4 | 7 → 6 | `content -> project` | 4 | `content/{document,open,save,snapshot}.rs` |
| 5 | 6 → 5 | `approval -> audit` | 5 | `approval/coordinator.rs` |
| 6 | 5 → 4 | `approval -> project` **and** `approval -> agent` | 2 | **blocked — see "Stage 6 does not happen as scoped" below** |
| ~~7~~ | — | ~~`project -> agent`~~ | ~~3~~ | **superseded — dropped entirely, see the re-derived path below** |
| ~~8~~ | — | ~~`transcript -> runtime` **and** `transcript -> project`~~ | ~~4~~ | **superseded — became the new stage 7** |
| ~~9~~ | — | ~~`runtime -> project`~~ | ~~3~~ | **superseded — became half of the new stage 9** |

**Stage 3 is one reference in one file.** Stage 6 is the first place D8's *"one cut or one coherent
group"* clause is load-bearing: at a component of 5 (`agent`, `approval`, `project`, `runtime`,
`transcript`) **no single cut shrinks it at all** — measured, all fifteen inner edges tried. The
cheapest pair is both of `approval`'s outgoing edges, two references, which is also the most
coherent reading of the move: `approval` stops reaching into `project` and `agent` together, or not
at all.

Alternatives at each step were computed and the cheapest taken; at stage 3 the other two candidates
were `content -> project` (4) and `approval -> audit` (5), which is why they become stages 4 and 5
rather than disappearing.

## Stage 6 does not happen as scoped (2026-11-16, review 682)

The dev team held stage 6 rather than forcing it, which is RFC-077's own rule, and the finding is
correct. **One of stage 6's two references cannot be cut in any shape without weakening a
documented security guarantee.**

`approval -> agent` is the single import `use crate::agent::VerifiedCwd;`. That type exists so that
`approval::coordinator::receive_proposal` cannot typecheck against an *unverified* path — concretely
`CommandProposal::cwd()`, an adapter's own untrusted claim — and the guarantee rests on its
constructor being private to `agent/launch.rs`, which holds its one call site, immediately after the
containment check. All three of D5's moves were tried and none fits:

- **Move 1** (type → T0) does not compile. `pub(in crate::agent)` on an item defined under
  `crate::domain::agent` is **E0742**: visibilities restrict only to *ancestor* modules, and the two
  are siblings. Reproduced in isolation on rustc 1.100.0 in a scratch crate containing none of this
  project. The only visibility that compiles is `pub(crate)`, which takes minting from one file to
  the whole crate.
- **Move 2** (consumer trait, provider impl) is *mechanically* available under RFC-074 D5 as amended
  — the impl would sit on `agent -> approval`, an edge that already exists. It fails because a trait
  moves the privilege from *who may construct* to *who may implement*, which inside one crate is no
  narrower; and sealing it requires `approval` to implement the seal for `crate::agent::VerifiedCwd`,
  re-creating the edge being cut.
- **Move 3** (need is orchestration's) has nothing to relocate: verification already happens in the
  caller, before `approval` sees the value.

Two further shapes were searched and are also dead. Moving the type **together with its minting
check** into `domain::agent` compiles and keeps minting file-local, but turns the containment root
into a caller-supplied `&Path`, so `approval` itself could mint a token from `proposal.cwd()` — the
same laundering, now reading as safe code. Typing that parameter as `ProjectRootHandle` to make the
root's provenance unforgeable fails because the handle lives in `project/root.rs` and is minted only
from a `&ProjectSession`, which can never be T0.

**The generalization, which closes the question rather than leaving it open:** in a single crate,
*"only code under subtree X may mint this value"* is expressible only when the type is **defined**
inside X. `approval` must name the type and `agent` must own the minting, so the only construction
satisfying both is a dependency from `approval` to `agent`. **This edge is not a reference that
resists D5; it is the structural expression of the guarantee.** No fourth shape exists.

### Decided: the residual stays (owner, 2026-11-16)

`approval -> agent` is permanent. It is recorded as the seventh invariant in `ARCHITECTURE.md`
(*"a privilege-bearing type's dependency is the privilege"*), because the thing a future reader needs
is not a note that the edge was discussed but a statement that removing it deletes a guarantee.

**Correction to this document's first version of this section, which overstated what the residual
costs.** It claimed the residual floors the bound at 4 and forfeits acyclicity. Both are wrong, and
wrong for one methodological reason: the cost was computed by keeping the stage order that had been
derived on the assumption stage 6 would succeed, instead of re-deriving it — which is what D8
requires. Re-derived from the graph as it actually stands, **acyclicity is still reachable with the
residual in place**, because a single directed edge is not a cycle. What blocks acyclicity is the
*reverse* edge `agent -> approval`, which carries no privilege at all and is an ordinary D5 move-2
candidate. Measured:

| | now | + stage 7 | + stage 8 | + stage 9 |
| --- | --- | --- | --- | --- |
| bound | 5 | 4 | 2 | **1 (acyclic)** |
| edges / refs | 60 / 139 | 58 / 135 | 56 / 133 | 54 / 129 |

Option 1 (`pub(crate)`) would have terminated at 55 edges / 128 refs, bound 1. **So the residual
costs one edge and buys back a documented guarantee** — which is a better trade than the one it was
accepted on, and the only reason this correction does not reopen the decision.

Note on reading the table: `agent -> approval` counts as one reference only because the counter
counts `crate::X` occurrences and that one is a grouped `use` naming **six** symbols
(`ApprovalChannelEndpoint`, `ApprovalChannelErrorReason`, `ApprovalChannelPathError`,
`ApprovalChannelPathRequest`, `ApprovalChannelPathResolver`, `validate_token`). Reference counts
order the stages; the bound is what judges one complete, so the derivation is unaffected — but that
row is not the one-line change its count suggests.

### The re-derived path: three stages, 9 references, to acyclic

| Stage | Bound | Cut | Refs | Where |
| --- | --- | --- | --- | --- |
| 7 | 5 → 4 | `transcript -> project` **and** `transcript -> runtime` | 4 | a coherent group — this was the old stage 8 |
| 8 | 4 → 2 | `agent -> project` **and** `agent -> runtime` | 2 | a coherent group; drops the bound by two at once |
| 9 | 2 → 1 | `runtime -> project` **and** `agent -> approval` | 4 | **two unrelated moves, one stage** — see below |

The old stage 7 (`project -> agent`, 3 refs) is **gone**: with `approval` still in the component it
lowers nothing, and once stages 7–8 above have run it is not needed to reach acyclic at all. Three
references that no longer have to be touched.

**Stage 9 is one stage only because the bound says so, and that is worth noticing rather than
tidying away.** After stage 8 the graph holds two independent 2-cycles — `{agent, approval}` and
`{project, runtime}` — so the bound cannot drop below 2 until *both* are broken, by two moves with
nothing in common. This is the same crudeness the section below describes from the other end: near
the bottom, the largest-SCC bound stops distinguishing between related work and coincidence. Whoever
runs stage 9 should expect to justify it as two moves that happen to share a bound, not as one
coherent group in D5's sense.

### Required with stage 7: the guard the gate cannot provide

The gate measures cycles. It therefore **rewards** the one regression that matters here: changing
`receive_proposal` to take `&Path`, or widening `VerifiedCwd::from_validated` to `pub(crate)` so the
type can move, both delete the guarantee *and* improve the bound. The gate would go green on a
security regression.

So stage 7's commit must also carry, as product code:

1. A test asserting **exactly one** call site of `VerifiedCwd::from_validated` in production sources,
   failing loudly on a second — the same enumerate-by-name shape `ARCHITECTURE.md`'s single-ingress
   invariant already uses, and excluding the `#[cfg(test)] for_test` hatch, which is deliberate.
2. A test asserting `receive_proposal`'s verified-cwd parameter is still `&VerifiedCwd` and not
   `&Path`, so the signature cannot be widened silently.
3. A pointer from `approval/coordinator.rs`'s module doc and `agent/launch.rs`'s `VerifiedCwd` doc to
   the new `ARCHITECTURE.md` invariant, so the reasoning is one hop from both ends of the edge.

Those three are the condition on which the residual is safe to keep. Without them the decision is
recorded in prose that the next bound-lowering commit has every incentive to ignore.

### Stage 7 landed (`5e234dc`, review 683) — and the cut-cost column is wrong in a way that matters

Bound 5 → 4, `{agent, approval, project, runtime}`, `transcript` out of the component entirely, 58
edges exactly as predicted. The three required guards landed with it. One required fix was raised in
review 683: the signature guard pins the *spelling* `&VerifiedCwd`, not the type's identity, so
re-pointing the import to an alias cuts the edge, improves the bound, and keeps CI green — a
compile-time fn-pointer pin replaces it.

**The reference prediction was wrong, and it was this document's error.** Predicted 135, actual 142:

```
transcript -> project   3 -> 0      project -> transcript   3 -> 8   (+5)
transcript -> runtime   1 -> 0      project -> runtime      4 -> 5   (+1)
transcript -> domain    3 -> 4      project outgoing refs: 27 -> 33
```

**The cut-cost column counts references removed and has never counted the wiring a cut requires.**
Stage 7 removed 4 and added 7. Stages 8 and 9 will be wrong the same way, so read their Refs column
as a lower bound on churn, not an estimate of it.

Two consequences, smaller first because it inflates the number:

**The metric is sensitive to import style.** Five of the six added references are
`crate::transcript::X` written inline; a grouped `use` would have counted **one**. This is the same
artifact that makes `agent -> approval`'s six symbols count as one reference, seen from the other
side. Nobody should restructure code to move this number — it is the measurement that is coarse.

**The hub got denser while the bound dropped, and nothing was counting that.** `project` is the
module D7 exists to split, on the RFC's own reasoning that *"the cost of this split only ever
rises."* Stage 7 raised it. The placement was correct — D5 as amended puts the supply site in
orchestration, and `project` already had both edges — so this is not a defect in stage 7. It is a
cost that the bound cannot see and that stage 8 will incur again by the same technique, for the same
correct reason.

**`project`'s outgoing fan-out is therefore a candidate for the second measurement** this document
already says D7 will need (see the section below on declaring victory at stage 9) — and it has just
moved the wrong way. **Required with stage 8's review request:** report `project`'s outgoing
reference and edge counts before and after, next to the bound. Not as a target to optimise; as a
number that stops being invisible.

This is the second consecutive stage where the gate rewarded something working against the goal —
the first was the `VerifiedCwd` regression `ARCHITECTURE.md`'s seventh invariant now names. The
pattern is worth more than either instance: **a single scalar ratchet will keep finding the cheapest
way to be satisfied, and that way is not always progress.**

### Stage 8's brief, measured at `4bac19c` (review 683 round 9)

Both references are in **one file**, `agent/launch.rs` lines 10-11, and they are a **necessary**
group, not merely a coherent one — measured:

```
drop agent->project only : bound 4   (unchanged)
drop agent->runtime only : bound 4   (unchanged)
drop both                : bound 2   -- {agent, approval} and {project, runtime}, 56 edges / 140 refs
```

`agent -> project` is three names but **one real need**: `ProjectId` is already canonical in
`domain::identity` and `WorkspaceTrust` in `domain::trust` (moved by stage 3 itself), so both are
stale re-export references and pure housekeeping. Only `ProjectSession` requires a D5 move.
`agent -> runtime` is four terminal launch types — `AdapterApprovalConfig`, `TerminalDimensions`,
`TerminalEnvironmentPolicy`, `TerminalLaunchSpec` — none of which is in `domain`, so that half is a
real design question.

**Required with the request:** `project`'s outgoing reference and edge counts before and after,
alongside the bound (currently **33 refs across 7 edges**). Gate bound 4 → 2 in the same commit.

### A D5 move 1 that leaves its in-crate callers on the re-export converts its own cut into a later stage's work

Sixteen in-crate references outside `project/` still reach `domain` types through `project`'s
backward-compatible re-export:

| Type | Stale in-crate refs | Where |
| --- | --- | --- |
| `ProjectId` | 11 | `runtime/terminal/{types,launch}.rs`, `audit/{purge,record,store,integration}.rs`, `agent/launch.rs` |
| `WorkspaceTrust` | 2 | `app.rs`, `agent/launch.rs` |
| `FileAccessError`, `FileAccessBlockedReason` | 2 | `audit/integration.rs` |

Stage 3 moved `WorkspaceTrust` to `domain::trust` and left `agent/launch.rs` pointing at `project`;
stage 4 moved the five file-access types and left `audit/integration.rs` pointing at `project::root`.
Those references are now part of stages 8 and 9's measured cost. **The re-export exists for external
consumers, not internal ones** — so from here, a move 1 repoints every in-crate reference to the
canonical path in the same commit.

**Recommended before stage 8:** one mechanical commit repointing all sixteen, same shape as
`3390333`, labelled housekeeping. It does not move the bound — every affected edge survives on a
genuinely-needed name (`ProjectSession` for `agent`, `ProjectSession` for `runtime`) — but it reduces
`agent -> project` and `runtime -> project` to their real dependencies, which is what stages 8 and 9
then have to reason about.

**Housekeeping landed, `9b10535` (also fixed two references beyond the sixteen named above, found
by direct grep rather than trusting the count as exhaustive: `project_board.rs`'s own `ProjectId`
import and a fully-qualified `WorkspaceTrust` path at `project_board.rs:320` — `project_board` is
its own top-level module, distinct from `project`).**

### Stage 8 does not happen as scoped (2026-11-16, review 684)

`agent -> project` fits D5 move 2 cleanly: `AgentRunLaunchValidator::validate` needs exactly three
facts from a `ProjectSession` (`id`, `trust_state`, `canonical_root_path`), not the type itself.
Implemented, verified (zero external call-site changes, full gate clean), measured (bound stays at
4 alone, exactly as predicted) — then reverted, since alone it does not land the stage.

**`agent -> runtime` is not four types to move; it is one entangled question D5's three moves do
not answer.** `TerminalDimensions`/`TerminalEnvironmentPolicy` are clean move-1 vocabulary in
isolation, but moving only those two leaves the edge alive on `TerminalLaunchSpec`/
`AdapterApprovalConfig`. `TerminalLaunchSpec` itself cannot move (it depends on `transcript` and,
through `AdapterApprovalConfig`, `approval` — `domain` would gain two new outgoing edges, which
`domain_has_zero_outgoing_cross_module_references` holds absolutely). No single method call exists
to invert into a trait (move 2): `agent` constructs the value once
(`AgentRunLaunchPlan::from_validation`) and mutates it again later, from a *different* method
(`prepare_adapter_approval`), which binds a real `ApprovalChannelEndpoint` against the filesystem
and validates a token *before* the mutation that sets `AdapterApprovalConfig` — not a data
assignment, a security-relevant approval-binding step. Move 3 (orchestration constructs it,
`project::session.rs` already has edges to both `agent` and `runtime`) is the only shape that
fits, but it means relocating where agent-run launch's own approval-channel-binding step lives
relative to the value it mutates — a design decision in the shape of Stage 6's `VerifiedCwd`
finding, not an engineering pick like Stages 4/5's own D5 moves. Not attempted unilaterally.
Options recorded in review 684: accept the Move 3 restructuring (its own follow-up round, not
this one), or leave `agent -> runtime` as a residual, same disposition as `approval -> agent`.
Stage 9 is not re-derived until this is answered.

### Stage 8's third shape, directed (review 684 response, 2026-11-16)

Review 684's finding is correct: `agent -> project` fits move 2, and `agent -> runtime` does not fit
any of the three *as the question was framed*. The framing is what gives way. Its Move 3 was read as
"relocate agent-run launch's approval-channel-binding step"; only the **assembly of the runtime-typed
value** has to move, and the binding stays in `agent` untouched.

1. `TerminalDimensions` and `TerminalEnvironmentPolicy` → `domain::terminal` (move 1; both
   dependency-free, and `domain::terminal` already holds `TerminalKind`, `TerminalStatus`,
   `VisibleSlot`, `TerminalSession`, `TerminalTransitionError` — these two are what was left behind).
2. `AgentRunLaunchPlan` holds a runtime-free **intent** of `agent`'s own instead of a
   `TerminalLaunchSpec`: the four fields it already sets, plus `compatibility_level`, plus the
   `socket_path`/`token` pair from `prepare_adapter_approval`. The token is
   `approval::RunCapabilityToken` and `agent -> approval` is already the permanent residual, so no
   new edge.
3. **`prepare_adapter_approval` is unchanged** — bind, validate, then record, in that order, in
   `agent`. It records into `agent`'s own struct rather than mutating a `runtime` type.
4. `project/session.rs` assembles the real `TerminalLaunchSpec` where it already handles one
   (`:757` `into_parts()`, `:760` reads `environment_policy`, `:842` passes it to
   `launch_project_adapter`): `plain_shell`, the four fields,
   `authorize_validated_agent_run(compatibility_level)`, and `set_adapter_approval_config` when the
   intent carries one. Both are `pub(crate)`; no visibility widens.
   (`authorize_validated_agent_run` is an inherent method on the spec, `types.rs:62` — it is not an
   independent `runtime` reference to cut.)

**Required property, not an implementation detail:** `project` calls the method that grants
`ValidatedAgentRun` authority, so the intent type must not be mintable outside `agent`'s own
validator — it lives in `agent`, has no public constructor taking arbitrary fields, and `project`
consumes without being able to forge. This is the `VerifiedCwd` lesson applied where it *works*: the
type stays in the module owning the privilege and the consumer only reads.

**Directed, not escalated.** Stage 6 went to the owner because every answer traded away a documented
guarantee; here nothing does. Both halves land together (neither moves the bound alone), gate 4 → 2
in the same commit, and `project`'s fan-out is reported before and after — this is the third
consecutive stage to lower the bound by making the hub denser.

### Stage 8 landed (`880c01b`) and Stage 9 re-derived (review 684 round 2, 2026-11-17)

Bound 4 → 2. `agent -> project` and `agent -> runtime` are both gone; `agent` and `approval` left the
component entirely. 57 edges / 130 refs. `project`'s fan-out 33 → 35 refs across the same 7 edges.

**Stage 9 was scoped against a four-member component that no longer exists.** Re-derived: the graph
now holds **two independent 2-cycles**, and the bound cannot drop below 2 until both break.

| Cycle | Edges in it | Refs | Where |
| --- | --- | --- | --- |
| `{agent, approval}` | `agent -> approval` | **1** (6 symbols) | `agent/launch.rs` |
| | `approval -> agent` | 1 | `approval/coordinator.rs` — **permanent residual, 7th invariant** |
| `{project, runtime}` | `runtime -> project` | **2** | `runtime/git.rs`, `runtime/terminal/launch.rs` |
| | `project -> runtime` | 5 | `project.rs`, `project/{explorer_tree,session}.rs`, `project/root/explorer.rs` |

Since `approval -> agent` can never be cut, `{agent, approval}` breaks only via `agent -> approval`.
Measured, no single cut moves the bound; the cheapest pair that reaches **bound 1, acyclic** is:

```
agent -> approval  +  runtime -> project   = 3 refs  => bound 1, zero cycles
(the alternative, agent -> approval + project -> runtime, costs 6 refs for the same result)
```

**Two unrelated moves sharing one stage, again** — the same crudeness noted below for the old stage 9,
and for the same reason: at the bottom the bound stops distinguishing related work from coincidence.

**The thing to know before scoping it.** `agent -> approval`'s single reference is a grouped import of
six symbols — `ApprovalChannelEndpoint`, `ApprovalChannelErrorReason`, `ApprovalChannelPathError`,
`ApprovalChannelPathRequest`, `ApprovalChannelPathResolver`, `validate_token` — i.e. the whole
approval-channel binding cluster, flagged as "not the one-line change its count suggests" back in
review 682. And Stage 8's directed shape deliberately **kept** that binding in `agent`, to avoid
touching a boundary RFC-021/022 already reviewed. That was right, and it has a consequence: Stage 9
must now address the edge Stage 8 chose to preserve.

A D5 move 2 looks available and would not relocate the binding — `agent` declares the channel
operations it needs, `approval` implements them, and the impl can sit on `approval -> agent`, the
residual edge that must be kept anyway. The ordering (bind, validate, then record) would stay in
`agent`. **The likely sticking point is not the trait but the signature**:
`AgentRunLaunchPlan::prepare_adapter_approval` returns
`Result<Option<ApprovalChannelEndpoint>, AgentAdapterApprovalError>` (`agent/launch.rs:496`), so
`agent` names an `approval` type in its own public API. Cutting the edge means that return type
changes, and `ApprovalChannelEndpoint` is not vocabulary — it holds a bound socket. Settle that
before writing anything.

### Stage 9 does not happen as scoped (2026-11-17, review 685)

`runtime -> project` fits D5 cleanly (move 2 for `TerminalLaunchProject`, mirroring Stage 8's
`AgentRunLaunchProject`; move 1 for `FileGitStatus`/`ProjectGitSummary`/`ProjectProviderState` to
new `domain::git_status`). Measured, then landed as housekeeping rather than a stage (`c97d4ef`):
cutting it breaks the `{project, runtime}` cycle entirely but the bound stays at 2, since
`{agent, approval}` was already there.

**`agent -> approval` does not fit any of D5's three moves, and the reason is deeper than the
returned endpoint type this document already flagged.** Three further points, found by measuring
past that one line: `AgentAdapterApprovalError` is *defined* in `agent/launch.rs`, naming
`ApprovalChannelPathError`/`ApprovalChannelErrorReason` directly in its own variants, independent
of what `prepare_adapter_approval` returns; `ApprovalChannelErrorReason::SocketPathTooLong` is
pattern-matched by name in already-reviewed `tekstide` UI code (RFC-060 PR-060-D), which rules out
Stage 5's own opaque-wrapper move for the analogous audit case; and Stage 8's own
`AgentTerminalAdapterApproval` holds a `RunCapabilityToken` field, a fourth direct naming, itself
privilege-bearing the way `VerifiedCwd` is (`pub(crate)` accessor, by design). Move 2 (the trait
this document already suggested) fixes the *behavior* and leaves all three of these untouched,
because they are not calls `agent` makes, they are types `agent` *defines its own structs and
enums in terms of* — closing them means genericizing `AgentAdapterApprovalError`,
`AgentTerminalAdapterApproval`, `AgentTerminalLaunchIntent`, and `AgentRunLaunchPlan` itself, a
cascade through the type Stage 8 built one stage ago, not a trait-and-impl.

**Owner's decision, per review 685**: accept that cascade (a follow-up round, not this one), or
accept `agent -> approval` as a second residual alongside `approval -> agent` — RFC-077's own
bound would then floor at 2, which the section below already shows does not block D7 at a bound
of 1, let alone 2.

### Stage 9's third shape, directed (review 685 response, 2026-11-17)

`runtime -> project` is cut (`c97d4ef`, housekeeping — correctly not claimed as a stage, since alone
it leaves `{agent, approval}` at 2). Review 685's finding on `agent -> approval` is accurate on every
fact, and its conclusion — a generic-parameter cascade through `AgentRunLaunchPlan` — follows only
if the binding stays in `agent`. It does not have to.

**Correction the cascade rests on:** `RunCapabilityToken`'s privilege is **crate**-scoped — its
accessor is `pub(crate)` so no *downstream crate* can extract the secret. That blocks move 1
(relocating the type to `domain`); it does not constrain which in-crate module names it.

**Directed: move the binding cluster to `project`.** `agent`'s entire approval surface is that
cluster (the six-symbol import, `AgentTerminalAdapterApproval`'s token field and accessor,
`prepare_adapter_approval`'s body, and `AgentAdapterApprovalError`'s two variants — nothing else in
`agent` names an approval type). Its body needs only `compatibility_level`, the state root with its
documented fallback, `project_root` and `agent_run.id`, all reachable from `project` via
`into_parts()` and existing accessors. And `project` is already the sole caller (`session.rs:852`),
already imports `ApprovalChannelEndpoint` (`session.rs:5`), already branches adapter-versus-shell
(`:836-842`), and already assembles the spec carrying `AdapterApprovalConfig`.

So all four naming points resolve with no generics: the endpoint never crosses a boundary;
`AgentAdapterApprovalError` becomes `project`'s, where `shell.rs`'s arm already traverses
`ProjectAgentRuntimeLaunchError::AdapterApproval(...)` so `SocketPathTooLong` **stays concrete** and
the reviewed UX distinction is kept rather than traded; and `AgentTerminalAdapterApproval` is
**deleted** rather than genericized, since `project` writes the socket path and token straight into
the spec it already builds. Stage 8's `AgentRunLaunchPlan` stays concrete.

**Measured: bound 2 → 1, zero cycles, acyclic. 55 edges / 131 refs, no new edge.** `approval ->
agent` remains the single permanent residual, and one directed edge is not a cycle — so no second
residual is needed.

**Required**, because this relocates reviewed code: the ordering (resolve, bind, validate, record)
moves as one verbatim block, not re-derived; fail-closed on a missing state root is preserved
including the fallback `agent/tests.rs:1509` documents a past defect in (repoint that test, do not
rewrite it); the `expect` keeps its comment; `shell.rs`'s arm is proven by running its own test.
Gate 2 → 1 in the same commit, and report `project`'s fan-out — this is the fourth consecutive stage
to lower the bound by making the hub denser.

## The finding that matters more than the table

**Twenty-two references is small.** Stages 3 through 9 together are about the size of a single
ordinary slice — smaller, in references, than PR-074-B's own two stages were in files touched. The
multi-release problem this reviewer flagged when the split was made may simply not arise: **if
stages 3–9 land in one release, RFC-077 closes in that release and never has to be named by a
section it did not ship in.** That should be checked against real effort before being relied on,
but plan for one release and split only if the work argues for it.

## And the finding that should stop someone declaring victory at stage 9

**After stage 9 the graph is acyclic, the bound is 1, and D7 has not started.**

D7 is *"`project/` splits: orchestration plus its capabilities."* `project/` is **9,077 production
lines** across 17 entries today, `session.rs` alone 2,696. Cutting 22 references makes the module
graph acyclic; it does not split `project/` into anything. **Splitting an already-acyclic module
into siblings does not lower the largest-SCC bound** — it cannot, because the bound is already 1.

So RFC-074's ratchet, which has been the whole evidence mechanism since D1, **stops being evidence
exactly where D7's work begins.** Whoever scopes D7 must bring a second measurement with it — a
property the gate can hold that a `project/` split actually moves — or D7 lands with no witness,
which is the condition D1 was written to make impossible. This is not a reason to defer D7; it is
the thing to settle before starting it.

## Standing requirements carried from RFC-074

- **Each bound move re-ablates** (review 672's requirement, applied at every stage of PR-074-B):
  plant a reference that should fail the *new* bound, confirm red naming the module, restore.
  Lowering a bound makes the assertion stricter; it does not re-prove the measurement underneath
  still discriminates.
- **The bound moves in the same commit as the move that earns it.** A commit that cuts the edge
  without lowering the bound has not landed the stage; it has made the gate lie by one.
- **A stage that cannot lower the bound is not a stage** — fold it into one that can, or report
  that the measured cut cost was wrong.
- **Every disposition says which of D5's three legal moves it used, per reference** — and where the
  premise does not hold, says so rather than picking a label that misdescribes it, as PR-074-B's
  own B2 did for a test-only pair.

## What is not in this document

Which release these take, how many slices they become, and D7's own scoping. The first two are the
owner's and the reviewer's; the third needs the second measurement named above before it can be
written honestly.
