---
title: "What deleting a transcript must not do"
rfc: "RFC-049"
rfc_file: "../../accepted/049-transcript-retention-enforcement.md"
source_rfc_status: "Accepted 2026-09-12 — M12"
target_milestone: "M12"
created: "2026-09-12"
---

# What deleting a transcript must not do

**Required reading before writing code.** Transcripts are private local data: prompts, file paths,
tool output, and whatever an external program printed. This slice removes them without being asked.

## §1 Prefer keeping too much over deleting too early

Every rule below resolves the same way when it is ambiguous. **A transcript kept past its limit is
a bounded disappointment. A transcript deleted before its limit is gone**, with no undo, no
tombstone content, and nothing the user can do.

So: off-by-one in favour of keeping. An age comparison that is unsure is *not expired*. A budget
that is exactly at its limit is **not** over it. A transcript whose liveness cannot be determined is
**live**.

## §2 A live writer is never selected, at any pressure

RFC-011: *"Running transcript writers must not be silently deleted underneath active AgentRuns."*

Not "deprioritised". Not "selected last". **Never selected**, even when the app-wide budget is
exhausted and nothing else can be freed — that case has its own answer, which is
`RequiredLocalBounded` failing preflight (D4), not deleting from under a running process.

Liveness comes from the transcript's **`AgentRun.status`** — not from `lifecycle_state`, which
production never moves off `Active`, and not from `last_write_at`, which production never
writes (**D8′**, replacing D8 on 2026-09-13). Only `Completed | Failed | Cancelled` are not
live; **`Detached` is live**; a transcript naming no run is live. *(Corrected at response 385:
this line still read "from `lifecycle_state`… (D8)" in the required reading, after D8′ had
reversed it.)*

## §3 One function deletes transcript bytes

RFC-033 built it. This slice calls it. **Do not write a second deletion**, however small the
temptation — an `fs::remove_file` inside a cleanup loop is a second path that will not get
RFC-033's tombstone handling, its bounded errors, or its "succeed when bytes are already absent"
behaviour.

If RFC-033's purge does not fit, say so and stop. Changing it is a decision; routing around it is a
defect.

## §4 The trail must say the product did this, not the user

D6: policy cleanup records `(AppPolicy, ExplicitCleanup)`. The user's own purge records
`(User, TrustedUi | AppCommand)`.

**A trail that cannot tell those apart is worse than no trail**, because someone reading it later
concludes the user deleted their own data. The frozen schema already permits both pairings and
already validates them; using the wrong one is not a schema question, it is a false statement.

**And a cleanup that deletes nothing writes nothing.** The triggers are frequent; a record per
trigger would drown the family that matters.

## §5 The number the user reads must be computed against the limits in force

D5. `transcript_local_data_summary_for` passes `agent_run_default()` — the compiled constants —
so since `0.18.0` a user who configured a retention value sees pressure computed against a
different one.

**Rule: the summary takes the session's configured limits.** A budget figure computed against a
budget that is not the governing one is §4.1's shape done in arithmetic instead of prose, and it is
harder to spot because numbers look like facts.

## §6 Nothing here may claim more than it enforces

The mirror of §1, for the text. Until C lands, **no user-facing sentence says transcripts are
removed** — A marks and B is unreachable from production.

When C does land, the changelog must say the thing that is easy to leave out: **the first run after
upgrading will delete every transcript already older than the configured age.** That is correct
behaviour and a surprise, and RFC-045's own changelog earned a required fix for exactly this class
of omission. Say it before someone finds it.
