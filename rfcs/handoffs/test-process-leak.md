---
title: "The leaked-child test flake — cause known since 2026-08-16, still unfixed"
status: "Scheduled 2026-08-19, awaiting implementation"
rfc_file: "none — a test-harness defect, not product behaviour"
target_milestone: "M12"
created: "2026-08-19"
---

# The leaked-child test flake

## Why this is being scheduled now

**The cause was found on 2026-08-16 and has never been fixed.** From `future-work.md`:

> `Child::drop` does not kill the process, so **any test that panics before reaching its own
> cleanup leaks a shell process**

Since then, **three distinct tests** have been reported failing intermittently under the
resulting pressure, each disclosed separately and each moved past:

| test | first reported |
| --- | --- |
| `approval::tests::channel::bind_recovers_from_a_stale_socket_file` | the original, response 213 |
| `approval::tests::coordinator::agent_run_queue_limit_is_enforced_and_only_counts_live_entries` | request 260 |
| `command_approval_family_produces_real_durable_audit_records_through_the_pipeline` | request 276 |

Every one of those disclosures was the right individual call — reported rather than re-run
past, confirmed non-deterministic in isolation, not attributed to the slice that saw it.
**Collectively they are the problem**: a diagnosed defect is being re-observed instead of
repaired, and each new symptom costs a reviewer and an implementer the time to establish it is
the same old thing.

Measured rate, from the sampling already recorded: **3 failures in 150 full-suite runs.** Not
enough to block work. Enough that every contributor will meet it.

## Why it matters beyond tidiness

The affected area is **the command-approval and socket path** — the security-critical machinery
RFC-021 and RFC-022 built. A suite that fails intermittently there trains everyone to re-run
rather than investigate, which is precisely the habit that hides a real regression in that code
the first time one appears.

## What to build

A guard that kills a spawned child on drop, applied to the test helpers that spawn real
processes, so a panicking test cannot leak one. `std::process::Child::kill` plus `wait`, in a
`Drop` impl on a wrapper the helpers return instead of a bare `Child`.

Find every test helper that spawns a real process and returns it. The reachability-audit
technique applies: enumerate them mechanically rather than by reading.

## The gate

- **Measure before and after, the same way.** The existing figure is 3/150 full-suite runs. A
  fix claimed without a comparable post-measurement is a fix claimed on hope — and this
  project's own convention is to measure bounds rather than estimate them.
- **Show a leak happening, then not happening.** A test that panics deliberately, with the
  process count observed before and after. Without that, "no leaks" is unfalsifiable.
- **Do not chase the three symptoms individually.** If the cause is fixed and one still flakes,
  that is a *second* finding and worth having — but fixing the tests rather than the harness
  would hide it.

## What this does not establish

That the product leaks processes. This is a test-harness defect: `Child::drop`'s documented
behaviour, met by helpers that assume otherwise. Nothing here says a shipped Tekstide leaks
anything, and the closeout must not imply it does.
