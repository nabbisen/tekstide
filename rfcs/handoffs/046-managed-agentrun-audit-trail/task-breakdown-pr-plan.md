---
title: "RFC-046 — task breakdown and PR plan"
rfc: "RFC-046"
rfc_file: "../../accepted/046-managed-agentrun-audit-trail.md"
source_rfc_status: "Accepted 2026-09-06 — M12"
target_milestone: "M12"
created: "2026-09-06"
---

# Task breakdown and PR plan

Three slices. **A and B are in `tekstide-core` and change nothing a user can see; C is the one that
makes the trail exist.** The order matters: C is the only slice that can be wrong in a way that
misleads a reader, and it should be written against an API that is already correct.

**Sequenced after RFC-047 PR-047-D.** See the pack README.

## PR-046-A — make the producer safe to call

**D1 and D4. No new behaviour; the producer still has no callers when this lands.**

- `Authorized` phase moves from `append_required` to `append_observation` (D1). The launch no longer
  depends on the audit store.
- Rename to `launch_audited_agent_run` (D4). Keep the `Plain` rejection and comment *why* — Plain is
  the unsupervised passthrough and is deliberately out of scope. The old name must not survive as an
  alias; a second name for one producer is how RFC-036's problem starts.
- Doc comment gains §5's two sentences: what the trail answers, and what it does not.

**Required tests, each ablated separately:**

- A launch **succeeds when the authorization write does not persist**, and returns a real result.
  Drive it through `with_writer` (§2, corrected) — PR-047-B's fixtures cannot reach this code, since
  a coordinator can only be built around an already-open store. **This must fail if `append_required`
  returns** — check that by reverting the one word, not by reasoning about it.
- **`Started` is not attempted when the authorization did not persist — its own test, its own name.**
  Bundling this with the box above leaves §3's property failing under a name that describes the other
  one (response 367 R1).
- The store's own `MissingAuthorization` check must never be the thing that catches this in
  production — the producer skips the attempt itself.
- `Plain` is still rejected.

## PR-046-B — the return value carries everything the current path returns

**D2.** `AuditedAgentLaunch` gains the approval endpoint, and the audited path returns what
`launch_agent_run_with_runtime` returns today. Still no production caller.

**Required test:** the endpoint is **carried through**, asserted today with `None` (§4). Write it so
it fails if the field is dropped — not so it passes because the launch succeeded. If the assertion
would still pass with the field deleted, it is not the test.

## PR-046-C — production calls it

**The slice that makes the trail exist.** `attempt_agent_run_launch_with_profile_state_root_and_capture`
calls `launch_audited_agent_run` instead of `launch_agent_run_with_runtime`, and
`register_approval_channel` receives the endpoint from the audited result.

Everything the current path does after launch — the pre-launch generated-change baseline, the
baseline insert keyed by `agent_run_id`, the terminal-id read, the approval channel registration —
must still happen, in the same order, with the same values. **This is a substitution, not a
rewrite.**

**Required tests:**

- A real launch through the production path writes `Authorized` then `Started` to a real store, read
  back. Not "the producer was called" — the records, from the store.
- The same launch **with a degraded store** still launches, still registers the approval channel,
  and writes no records. This is the test that proves D1 reached production and not just the API.
- The generated-change baseline is still captured and still keyed to the launched run.
- **Refusals are unchanged**: Restricted Mode still records, and run-limit / validation /
  plan-transition still record nothing (D5).

**Evidence:** unit-level plus one live launch against a `mktemp -d` fixture showing the records in
the store afterwards. Per the delivery plan's bounded-evidence rule, if a live capture does not
reproduce within three rounds, document the gap and proceed — the store read-back is the load-bearing
evidence here, not a screenshot.

## Not in this plan

- Termination records (D3; RFC-048 reserved).
- New refusal records (D5).
- Schema or record-type changes — none are needed.
- Anything about `Managed` profiles. None exists yet; D2's test is written for that fact, not
  against it.
