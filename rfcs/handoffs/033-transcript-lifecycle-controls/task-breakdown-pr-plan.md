---
title: "RFC-033 task breakdown and PR plan"
status: "Complete 2026-08-19"
rfc_file: "../../done/033-transcript-lifecycle-controls.md"
target_milestone: "M11"
created: "2026-08-19"
---

# RFC-033 — task breakdown

Four slices. **A is a prerequisite, not a convenience** — B cannot be built correctly before it.

## PR-033-A — decouple the approval channel from transcript capture

**Do this first, and it is small.**

`AgentRunLaunchRequest::without_transcript_capture()` sets `transcript_state_root = None`.
`prepare_adapter_approval` falls back to `transcript_state_root` when `approval_state_root` is
`None` — a documented convenience, added by RFC-022 PR-022-C response 216 **specifically so the
two would not be coupled**, with the escape hatch left for callers to use.

**The GUI never sets it.** So the moment PR-033-B lets a user opt out of capture, a `Managed`
run's approval channel loses its state root and fails to bind. Response 216 fixed this at the
model layer; the call site never took the fix up.

Set `approval_state_root` explicitly in `attempt_agent_run_launch_with_profile`. Latent today —
`claude_code_linux_default` is `Supervised`, so no `Managed` profile is reachable — which is
exactly why it must land before the thing that makes it live.

**Gate**: a test that opts out of capture and still binds an approval channel. Ablate by
removing the explicit `approval_state_root` and confirming that test fails, not merely that some
test fails.

## PR-033-B — the per-run opt-out

**Decision, and it is the one I would push back on if you disagree**: put it on the **Trust
Settings surface** (`Ctrl+Alt+U`), as a per-project setting, rather than a dialog in front of
`Ctrl+Alt+A`.

Reasons: that surface already exists and is already reachable; it already carries project-scoped
security state; and adding a confirmation step in front of the product's most-used action taxes
every run to configure something most users will set once. RFC-032 put granting behind two
deliberate acts because granting is dangerous — declining to record is the *safe* direction, and
loading it with the same friction gets the asymmetry backwards.

**Open question 1 from the RFC — does the setting persist per project?** Recommend **yes**, and
by the same reasoning trust persists: a security-relevant preference the user set deliberately
should survive a restart, and a per-session setting silently resets to the more data-retaining
state, which is the wrong default direction for a privacy control.

**Gate**: proven from a real key press through to a run that produces **no transcript file** —
asserted against the real path shape `transcript-capture-evidence.md` already pins, not against
"the request said disabled."

## PR-033-C — purge, and retained-data visibility

Read `what-purge-must-remove.md` first.

**Decide the scope, and make the confirmation name it.** Recommend: per-project purge, from the
same Trust Settings surface. Application-wide purge is a bigger claim with a bigger blast radius
and no natural home yet.

**Confirmation asymmetry, recommended**: confirm project-wide purge; a single run's transcript
needs none. Deleting is the safe direction, but *"delete everything for this project"* is a
different act from *"delete this one,"* and the confirmation should say what disappears and that
it cannot be undone.

**Visibility**: `transcript_local_data_summary(app_retained_bytes, limits)` exists and has no
caller. A user deciding whether to purge needs to see what is retained. Wire it to the same
surface.

**Gate**: bytes gone from the real filesystem, asserted directly — not the return value, not the
metadata. The tombstone remains and the surface says so.

## PR-033-D — the `transcript_purge` audit producer, and closeout

**Check the family's own `valid_transcript_purge()` before designing the record.** PR-023-D found
that `valid_config_change` had already settled a question that looked like a judgement call; the
same may be true here.

Record that a purge happened and its scope. Never a path, never a byte count.

**Closeout must remove the published sentence.** `README.md`'s *Local Data and Privacy* section
says there is no in-app way to turn capture off or purge it. That sentence is the reason this RFC
exists, and it goes in the same commit as the last slice — narrowed to whatever remains true, not
deleted wholesale.

## Not in scope

- **Changing the capture default.** The owner decided capture is intended.
- **A configurable default.** RFC-023's.
- **Rendering transcript content.** RFC-020's report surface, already shipped.
