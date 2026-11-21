# RFC-078: Session-Member Recall When The Bulk Enumeration Stops Naming A Known Member

Status: **Accepted 2026-11-18 by the owner; D1 adopted as written.** No implementation slice has
begun. Proposed 2026-11-16, written at the owner's own direction (`rfcs/handoffs/
release-checklist.md`'s dated 2026-11-16 entries) after a confirmed, measured logical gap in
macOS session-enumeration containment: "this changes production containment behaviour and adds
per-session state, so it wants a decision record rather than a slice." **D1 below formalizes
review 683 round 5's own recommendation** — the reviewer's analysis, relayed through the owner's
own written entry but authored by the reviewer, not the owner, and not a proposal I originated —
written up for acceptance before any implementation slice begins, per RFC-043's own precedent
(D1-D4 "decided by the architect on acceptance and recorded in this file before implementation
begins"). The recommendation and the decision are attributed separately on purpose: review 683
round 6 named this project's own standing confusion between the two (fifth occurrence) and asked
that it stop here.

**Correction, review 683 round 7: this RFC does not fix, and must not be read as fixing, the
real red that prompted it.** The full 66-enumeration attempt-1 log (`test-process-leak.md`'s own
dated entry) refutes bulk omission for that specific run: `list_all_pids` never returned a short
list, and a bulk-visible member's own `live_member_of_session` answer flipped to `false` instead
— a different, still-open question (`release-checklist.md`'s own entry). The logic gap D1 closes
is confirmed reachable in the abstract, by the pure-function test cited below, independent of
whether it ever explains a real observation; this RFC stands on that confirmation alone, as a
precautionary closure, not as the explanation for the incident that led here.

Related RFCs:

- [RFC-043](../done/043-terminal-process-containment.md) — owns the session-scoped containment
  sweep (`processes_in_session`, `request_terminate`) this RFC closes a gap in. Closed and
  shipped; this is an addendum, not a reopening of D1-D4.
- [RFC-074](../done/074-capability-boundaries.md) / [RFC-077](../done/077-the-capability-split.md)
  — unrelated subsystem (the module-graph gate). No interaction; mentioned only because the gap
  below was found while working on RFC-077 Stage 7 and its review (683).

## Summary

macOS's `processes_in_session` re-checks a pid's liveness directly, bypassing the bulk
`proc_listallpids` scan, in exactly one case: the session leader, when absent from that scan.
Every other member is found, or not, purely through the bulk scan. A member the bulk scan never
names is read as departed, with nothing to re-check it against. **Confirmed, not merely
suspected**: a pure-function test (`decide_processes_in_session`, `runtime/terminal/launch.rs`
and `tests.rs`, landed `62febe9`) shows this produces `Some(vec![])` — a session the containment
sweep reports empty — when a genuinely alive, non-leader member is missing from the bulk list.

This RFC proposes closing that gap the same way the leader's own case is already closed:
remember which pids a session's enumeration has previously reported, and when a later
enumeration's bulk scan no longer names one of them, re-check it directly rather than read its
absence as departure.

## What is actually true today, measured

- **No process-group backstop exists for the class of process this gap is about.**
  `runtime/terminal/launch.rs:684-688`'s own comment: a backgrounded job started inside the shell
  "gets its *own* process group ... but stays in this same session, which is exactly why session
  id, not process group id, is what the kill above ... cannot reach." Session enumeration is not
  one containment mechanism among several for this class of process — it is the only one. A
  member this enumeration misses is signalled by nothing.
- **The asymmetry is deliberate for the leader and structural for everyone else**, not an
  oversight: the leader's own pid is known in advance (it *is* `session_id`, by construction), so
  it can always be re-checked when absent. No other pid in the scan can stand in for a member
  whose absence it did not itself discover.
- **The gap is confirmed by the pure-function test alone, not by a real observation, and that
  distinction is now settled rather than open.** `test-process-leak.md`'s own
  `a_real_close_kills_a_live_hup_immune_backgrounded_job` red (run `43779370314`, commit
  `8a9da38`) was first read from a twelve-line excerpt as *consistent with* this gap. **The full
  66-enumeration log refutes that reading (review 683 round 7): `list_all_pids` never returned a
  short list across any of the 66 reads (~137 pids throughout); a member was found within that
  bulk list for 62 consecutive reads and then stopped, which is a bulk-visible pid failing
  `live_member_of_session`, not the bulk list omitting one.** That real incident is therefore
  evidence for a *different*, still-open question (see `release-checklist.md`'s own entry), not
  for this gap. This gap stands on `decide_processes_in_session`'s own test alone — confirmed
  reachable by construction, not (yet, or perhaps ever) observed on a real machine.
- **Whether `proc_listallpids` can actually produce this omission for a live, non-`P_WEXIT`
  process on a real machine is not established.** It is an empirical question about a kernel API
  this project has not measured directly (unlike the leader's own case, measured 13 of 13 firings,
  reviews 625-630). This RFC does not wait for that measurement, for the same reason RFC-043
  itself did not wait to measure every leak before deciding D1/D2: the failure mode, if reachable
  at all, is silent and security-relevant, and the fix costs little in the common case (below).
- **Linux has no analogous gap.** `processes_in_session` there lists a real `/proc` directory at
  one instant — no bounded buffer, no silent truncation-without-signal, no second API path a
  pid could be invisible to while still being enumerable by `kill`/`getsid`. This is a macOS-only
  concern, same scope as `SessionLeaderState` itself.

## Options considered

1. **Accept the residual as explicit, no code change.** Rejected, per the point above: with no
   process-group backstop, this is not "a confirmation that overclaims" — it is "a process can
   survive a close, silently," which is the exact defect RFC-043 was written to close, reappearing
   in the one enumeration path that RFC-043 itself could not have tested against (the window did
   not reproduce until long after that RFC shipped).
2. **Refuse emptiness more often when completeness is uncertain.** No concrete trigger condition
   exists beyond the truncation check `list_all_pids` already has (`written_bytes >=
   capacity_bytes`) — there is no signal from `proc_listallpids` that says "this reply, while not
   truncated, might still be missing someone." Widening refusal without such a signal means
   refusing unconditionally more often, which lengthens the grace period for every close, not only
   the ones that needed it.
3. **Recommended — session-member recall.** Remember the pids a session's enumeration has
   previously reported as members. When a later enumeration's bulk scan no longer names a
   previously-seen pid, re-check it directly — via `live_member_of_session`, the same predicate
   every non-leader member in the bulk scan is already checked with (review 683 round 6; see D1's
   own required correction below for why this, and not `session_leader_state`'s membership-blind
   check, is the one that must be used) — instead of reading its absence as departure. Costs one
   direct check per *vanished, previously-seen* pid, only on that path, so the common case
   (nothing vanished since the last look) pays nothing extra.

## D1 — adopt option 3 (session-member recall)

**Accepted 2026-11-18**, per RFC-043's own D1-D4 precedent — see *Decided on acceptance* at the
end of this document for what the acceptance does and does not settle. As accepted:

### Design sketch, not yet implemented

- **Per-session state**, alongside `RunningTerminal::process_group_id` (`launch.rs`): the set of
  pids the most recent successful `processes_in_session` call reported as members. Updated after
  every call that returns `Some(members)` — not merely at construction, since a session's
  membership changes over its own lifetime (a job started after launch, finished before close).
- **`decide_processes_in_session`'s own shape extends, not replaces**: alongside the bulk `pids`
  and the leader-state oracle it already takes, a third input — the previously-seen pids absent
  from `pids` this time.
- **Required, as decision text, not left to implementation (review 683 round 6): the recall
  re-check is `live_member_of_session(pid, session_id)` — not `session_leader_state`, and not any
  oracle shaped like it.** `session_leader_state` is membership-blind by construction and
  correctly so for the leader: the leader's own pid *is* `session_id`, so its membership needs no
  check, only its liveness. A recalled pid has no such guarantee, and under pid reuse — the
  recalled pid exits, the kernel hands the number to an unrelated process — a membership-blind
  liveness check reads `Alive` and the recall path pushes that unrelated pid into `members`,
  which `signal_candidates(..., SIGKILL)` then kills. **This machine is shared; other agents run
  real sessions on it concurrently.** `live_member_of_session` already exists, is already what
  every non-leader member in the bulk scan is checked with, and already rejects a reused pid by
  construction (`getsid(pid) == session_id` **and** `!is_gone_or_going(pid)` — `Some(false)` for
  a pid that has wandered into another session). The RFC's own stated principle — "the leader is
  not held to a looser standard than this, and this must not be held to a looser one than the
  leader" — is correct and requires the *stricter* check here: the leader is handed membership
  for free; a recalled pid must earn it. Included in `members` only when `live_member_of_session`
  returns `Some(true)`.
- **Removed, review 683 round 8: this bullet claimed "in a run shaped like the real red, that
  pid *was* seen by an earlier call and would be in the recalled set" — the exact claim round 7's
  own correction refutes, surviving here after being fixed everywhere else in this document. It
  also did not support D1 even on its own terms: the cited test makes an explicit, out-of-band
  `processes_in_session` call the product's actual close path does not make. D1 needs no claim
  about run `43779370314` to stand — it stands on `decide_processes_in_session`'s own
  pure-function test alone, stated once, at the top of this document and in the Summary, not
  here.**

### Known limit, corrected (review 683 round 6) rather than claimed closed

**There is no enumeration outside the termination path at all** — every production caller of
`processes_in_session`/`session_confirmed_empty` runs inside a close or a drop:

```
launch.rs:509        wait_briefly_for_session_empty   (grace polling, from Drop)
launch.rs:656        Drop's own SIGKILL enumeration
termination.rs:287   request_terminate's confirmation
termination.rs:445   grace polling
termination.rs:567   signal_session_survivors
```

(`launch.rs:747`, `assert_session_is_empty`, is test-support only — no production call site.)
The first version of this section said the limit was "a member never seen in *any* enumeration
*over the session's life*," which reads like a corner case. It understated itself: with no
steady-state observation of session membership, the recalled set can only ever be populated by
**an earlier enumeration within the same close sequence** — grace poll, then the signalling
enumeration, then the confirmation. The real limit is **"never named by an earlier enumeration
of this same close,"** a much larger class than "never named over the session's whole life."

**It is still worth doing.** A member visible to an earlier enumeration in a close and missing
from the signalling one — the one that decides what dies — is exactly what recall catches. (It is
**not** what `test-process-leak.md`'s own red is shaped like — review 683 round 7's full-log read
refutes that, see the correction at the top of this document — so this paragraph states what
recall catches in the abstract, not a claim about that incident.) But if the bulk read omits the
member from the *first* enumeration of that close, recall never had it, and this RFC's own
benefit is bounded to one close's own internal window, not a session's whole life. Stated here
so the Risks section's own "false sense of closure" hazard does not apply to this section itself.

**Not a condition of accepting D1, but worth recording where the residual actually comes from**:
the non-goal "not a periodic background enumeration independent of close" (below) is what would
populate recall *before* a close ever starts, which is the difference between recall covering one
close's internal window and covering a session's whole life. Kept out of scope here; recorded so
a future reader does not have to re-derive why the limit is this wide.

## Scope

1. The per-session recall state and its update point, in `RunningTerminal` or wherever the
   implementer finds it causes the least churn to thread through.
2. `decide_processes_in_session`'s own extension to accept and act on it, with the existing three
   tests (`processes_in_session_cannot_recover_a_live_member_the_bulk_list_never_named` and its
   two companions) updated or superseded to reflect the new behaviour — the first of those three
   is the regression guard this RFC exists to eventually make pass differently, not a fixture to
   leave stale.
3. A fourth test exercising recall itself: a previously-seen, now-vanished-from-the-bulk-list pid
   that re-checks `Alive` is still reported a member.

## Non-goals

- **Not fixing the kernel-API uncertainty itself.** No mechanism to ask `proc_listallpids` "are
  you sure this is complete" is proposed or assumed to exist.
- **Not a periodic background enumeration independent of close.** Recall only ever runs inside an
  enumeration that already had a reason to happen (a close, today's only caller). Extending this
  to catch a member never seen at all (the Known Limit above) would need one, and is explicitly
  out of scope here.
- **Not Linux.** `processes_in_session` there has no analogous gap (see above); this RFC does not
  touch it.
- **Not re-opening RFC-043's own D1-D4.** Kill-not-leave, session-scoped-not-cgroup, the audit
  record's claim and the leak-assertion shape are all unchanged; this closes a gap in how D2's
  mechanism enumerates, not what the mechanism is.

## Risks

- **Per-session state that must be kept correct across a session's lifetime**, not merely at one
  call site — a cache that is updated inconsistently (e.g. only on the final, close-time
  enumeration, not on whichever ones preceded it) could silently stop protecting against the gap
  it exists for while still compiling and still passing a test written against the easy case. The
  implementation slice's own review should check this specifically.
- **A false sense of closure.** The Known Limit above is real and should appear in whatever
  commit message and release note this eventually ships in, not only in this document.

## Acceptance-time decisions

**D1 is the owner's to decide**, the same way RFC-043's D1-D4 were. Recorded here, before any
implementation slice begins, per that precedent.

---

## Decided on acceptance

**D1 is accepted as written, by the owner, 2026-11-18** — option 3, session-member recall, with the
design sketch above as the shape to build and its two *Required* decision bullets binding on the
implementation rather than open to it:

1. **The recall re-check is `live_member_of_session(pid, session_id)`**, never `session_leader_state`
   or anything shaped like it. This is the safety half of the decision, not a style preference: a
   membership-blind liveness check reads `Alive` for a reused pid that now belongs to an unrelated
   process, and `signal_candidates(..., SIGKILL)` would then kill it. This machine is shared and
   other sessions run on it concurrently. A recalled pid must earn membership; the leader is handed
   it for free because its own pid *is* the session id.
2. **The per-session state is updated after every call that returns `Some(members)`**, not only at
   construction and not only on the close-time enumeration — the Risks section's first entry names
   the inconsistently-updated cache as the failure mode that still compiles and still passes a test
   written against the easy case, and the implementation slice's own review is required to check it
   specifically.

**What this acceptance does not do, stated here so it is not read off the status line alone:**

- **It does not discharge `release-checklist.md`'s containment Standing Watch.** Recall triggers on
  absence from the bulk list, and the full 66-enumeration log of run `43779370314` shows the list
  was never short — see the correction at the top of this document. That watch stays open on its own
  question (what makes a bulk-visible member stop matching its own session while alive), and this
  RFC is accepted as a *precautionary* closure of a separately confirmed logic gap.
- **It does not narrow the Known Limit**, which is wider than it first reads and must be quoted as
  this document states it: *"never named by an earlier enumeration of **this same close**"* — not
  "never seen in any enumeration over the session's life." With no steady-state observation of
  session membership, the recalled set can only ever be populated inside one close's own sequence.
  The Risks section requires that limit to appear in the implementing commit message and the release
  note, not only here.
- **It schedules nothing.** The slot is the owner's. Part 1 of the containment decision
  (`release-checklist.md`, 2026-11-18) was the only piece gating `0.41.0`, and it is discharged.

Reviewer's note on the acceptance itself: the document accepted here is the corrected one. D1's
`live_member_of_session` requirement was a review finding before implementation, not after (the
first draft's predicate would have let a reused pid be signalled), and round 8 removed the one
bullet that still rested on the refuted motivating evidence. D1 stands on
`decide_processes_in_session`'s own pure-function test alone.
