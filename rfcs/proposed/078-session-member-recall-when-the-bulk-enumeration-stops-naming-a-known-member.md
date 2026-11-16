# RFC-078: Session-Member Recall When The Bulk Enumeration Stops Naming A Known Member

Status: **Proposed 2026-11-16.** Written at the owner's own direction (`rfcs/handoffs/
release-checklist.md`'s dated 2026-11-16 entries, review 683 rounds 4-5) after a confirmed,
measured logical gap in macOS session-enumeration containment: "this changes production
containment behaviour and adds per-session state, so it wants a decision record rather than a
slice." D1 below is a formalization of the owner's own recommendation in that document, not a
proposal I originated — written up for acceptance before any implementation slice begins, per
RFC-043's own precedent (D1-D4 "decided by the architect on acceptance and recorded in this file
before implementation begins").

Related RFCs:

- [RFC-043](../done/043-terminal-process-containment.md) — owns the session-scoped containment
  sweep (`processes_in_session`, `request_terminate`) this RFC closes a gap in. Closed and
  shipped; this is an addendum, not a reopening of D1-D4.
- [RFC-074](../done/074-capability-boundaries.md) / [RFC-077](../accepted/077-the-capability-split.md)
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
- **The scenario that exercises the gap is realistic, not contrived.** `test-process-leak.md`'s
  own `a_real_close_kills_a_live_hup_immune_backgrounded_job` red (run `43779370314`, commit
  `8a9da38`, a push touching none of `runtime::terminal`) showed the session leader absent from
  the bulk scan and separately confirmed departed, with the backgrounded job — alive by the
  test's own precondition — unaccounted for either way, ending in `0 member(s) found`. That log
  is *consistent with* this gap; it does not *prove* it, because the same log is also consistent
  with the already-fixed leader-specific misread (review 630). The pure-function test above
  removes that ambiguity for the logic itself, independent of which one explains that one run.
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
   previously-seen pid, re-check it directly — the identical `kill`/`sysctl` primitives
   `session_leader_state` already uses for the leader — instead of reading its absence as
   departure. Costs one direct check per *vanished, previously-seen* pid, only on that path, so
   the common case (nothing vanished since the last look) pays nothing extra.

## D1 — adopt option 3 (session-member recall)

Proposed; **left for the owner to decide on acceptance**, per RFC-043's own D1-D4 precedent. If
accepted:

### Design sketch, not yet implemented

- **Per-session state**, alongside `RunningTerminal::process_group_id` (`launch.rs`): the set of
  pids the most recent successful `processes_in_session` call reported as members. Updated after
  every call that returns `Some(members)` — not merely at construction, since a session's
  membership changes over its own lifetime (a job started after launch, finished before close).
- **`decide_processes_in_session`'s own shape extends, not replaces**: alongside the bulk `pids`
  and the leader-state oracle it already takes, a third input — the previously-seen pids absent
  from `pids` this time — each re-checked through the same style of oracle the leader already
  uses (`FnMut(pid) -> Option<SessionLeaderState>`-shaped, or equivalent; the exact signature is
  implementation, not decision). Included in `members` only if the re-check reads `Alive`,
  excluded otherwise — the leader is not held to a looser standard than this, and this must not
  be held to a looser one than the leader.
- **The information is available in exactly the scenario at issue.**
  `a_live_hup_immune_backgrounded_job_is_reported_as_a_session_member` (`runtime/terminal/
  tests.rs`) already asserts `processes_in_session` reports the backgrounded job as a member on
  the enumeration that sees it — so in a run shaped like the real red, that pid *was* seen by an
  earlier call and would be in the recalled set, not absent from it by construction.

### Known limit, to be documented rather than claimed closed

**A member never seen in *any* enumeration over the session's life stays invisible** — nothing
can re-check a pid that was never named once. This is a strictly smaller residual than today's
(“any member absent from *this* enumeration”), not zero. A job started and never observed by an
intervening enumeration before the bulk scan that misses it both exist is still a gap. Whether
that residual is acceptable, or whether it motivates a periodic background enumeration
independent of close (out of this RFC's own scope — see Non-goals), is a question for whoever
reviews this design's own implementation, not settled here.

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

*(Not yet decided.)*
