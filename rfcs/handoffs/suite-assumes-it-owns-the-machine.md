---
title: "The suite assumes it owns the machine — implementation handoff"
rfc: "none"
source_rfc_status: "No RFC. Two test defects with one cause; the decisions are settled below."
target_milestone: "M12"
created: "2026-08-26"
---

# Two places the suite treats a shared machine as its own

**No RFC.** Nothing a user sees changes. The decisions an implementer would otherwise have to
make alone are settled in each item.

Both items are the same mistake in different resources: **the test suite assumes it is the only
thing on the machine.** One assumes it owns the state directory, the other assumes it owns the
CPU. Bundled because that is a real common cause, not because they are both small.

Both were found and disclosed by other people's work. Both are the reviewer's to have scoped and
were not, until now.

## Item 1 — `transcripts/` and `approval/` still write to a real `$HOME`

### The finding

`open_real_agent_run_state_root` (`crates/tekstide/src/shell.rs`) calls
`AppStatePathProvider::linux_default()` unconditionally — `$XDG_STATE_HOME`, falling back to
`$HOME/.local/state/tekstide`. There is no `#[cfg(test)]` split. It is now **the last production
consumer of that resolution a test can reach**, and its own doc comment says so, because review
336 required that comment to be corrected when the audit store's split made it false.

Found by the dev team while verifying the audit-store isolation: with `XDG_STATE_HOME` unset — the
ordinary case for anyone running `cargo test` — `audit/` came out byte-for-byte identical, and
`transcripts/` gained several dozen files and directories under a real developer's real state
directory.

They correctly declined to fix a path they had not traced, in the same sitting, outside their
handoff's scope. Correct call; this is that follow-up.

### Why this is worse than the audit-store version was

The audit store has no user-facing surface. **`transcripts/` does.**

RFC-011 owns transcript retention. RFC-033 shipped a per-project purge **and a retained-bytes
display**. So a user reading how many bytes of transcript they are storing is reading a number
inflated by test debris, and a user purging their transcripts is purging test debris mixed into
their own. A transcript is the terminal output of an agent run — the most sensitive local data
this application keeps.

### Decided: the same shape the audit store got, for the same reason

> **Isolation is automatic, not opt-in, and reaching the real state directory from a test fails
> loudly.**

`resolve_audit_state_dir`'s `#[cfg(test)]` split plus its thread-local lazy default plus
`assert_not_the_real_audit_state_dir` is the pattern. Reuse it; do not invent a second one.

**And note what that slice learned the hard way**: its first attempt was an opt-in guard wired
into the 23 call sites its handoff named, and the suite immediately failed **58 other tests**
reaching the same path through `update()`. The count of sites in a handoff is a lower bound. Do
not wire per-site.

### Scope

1. `open_real_agent_run_state_root` gets the same `#[cfg]` split and automatic per-thread
   isolation.
2. **Enumerate what else resolves a real path**, as the audit slice was asked to and did. The
   `approval/` subtree moved too; establish whether that is this same function (a `Managed` launch
   binds its socket under the state root) or a third site.
3. The loud guard, covering both.

### Acceptance

- [ ] **Reproduced first**: snapshot `~/.local/state/tekstide`, run the suite with
      `env -u XDG_STATE_HOME`, diff, and show `transcripts/` moving. A fix for a defect nobody
      watched happen is a fix for a defect nobody established.
- [ ] **After: the diff is empty.** Not "audit is clean" — empty. That is the box review 336 could
      not honestly check, and this item exists to make checkable.
- [ ] The guard ablated: point a test at the real directory, watch it fail loudly, restore.
- [ ] Every real-path resolution enumerated in evidence, including ones you decided need no
      change, and why.
- [ ] Gates. **`/dev/pts` occupancy recorded before and after**, since this slice touches the
      launch path.

## Item 2 — a wall-clock assertion on a loaded machine

`change_review_content_view_build_cost_by_line_count_measurement` asserts
`elapsed.as_millis() < 500`. It failed **6 of 7 red runs** in review 338's gate, at load average
**59.7** on a 32-core box — a load produced by this project's own repeated full-suite runs.

**Its message is false**, and that is the actual defect:

> got {}ms, which would indicate a real regression, not measurement noise

Every observed failure *was* measurement noise. The message tells the next person to hunt a
regression that does not exist, and someone has already half-spent a day on it.

### Decided: keep the measurement, stop asserting the cause

- **The assertion may not claim which of the two causes it hit.** It cannot distinguish them, so
  it must not say. Reword to state what is true: this exceeded the budget, which is *either* a
  regression *or* load, and here is how to tell them apart.
- **Print the load average alongside the figure** when it fails. One line; turns a coin-flip into
  a diagnosis.
- **Do not simply raise the number.** A budget nobody can violate tests nothing, and the
  measurement is genuinely useful — RFC-042's D3 bound was set from it.
- Whether it becomes `#[ignore]`-by-default and deliberately invoked is **yours to decide**, with
  the reason written down. There is precedent either way in this codebase.

This originated in RFC-042, which the reviewer accepted without considering a loaded machine.

### Acceptance

- [ ] The failure message no longer asserts which cause it hit.
- [ ] Load is reported on failure.
- [ ] The budget is unchanged, or changed with a recorded reason that is not "it kept failing."
- [ ] The `#[ignore]` question answered either way, in writing.

## Not in scope

- The audit store (done, review 336).
- RFC-043's containment work. This does not change what termination means.
- Any production behaviour. Both items are test-side; item 1 touches production *code* only to add
  a `#[cfg(test)]` seam, exactly as the audit store's did.
