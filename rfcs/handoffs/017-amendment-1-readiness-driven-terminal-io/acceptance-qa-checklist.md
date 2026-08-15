---
title: "RFC-017 Amendment 1: Readiness-driven terminal I/O - Acceptance / QA Checklist"
rfc: "RFC-017 Amendment 1"
rfc_file: "../../done/017-terminal-renderer-and-immersion-mode.md"
status: "Open"
target_milestone: "M9 (carried), shipping in 0.8.0"
created: "2026-08-15"
---

# Acceptance / QA Checklist

Every unchecked line at closeout carries a stated reason.

## The new path (PR-A1-A)

- [ ] Reader thread blocks on readiness; no sleep, no busy-wait. Mechanism named.
- [ ] Channel is bounded; full channel stops the reader rather than dropping.
- [ ] `dropped_bytes` unreachable, proven by enumeration, not asserted.
- [ ] UI thread never blocks — shown, not asserted.
- [ ] Backpressure end to end: fast producer stalls on `write()`, resumes, no byte loss.

## The ingress re-proof (PR-A1-B)

- [ ] P1 re-enumerated against the new shape; new write site fails by name.
- [ ] P1 ablated with a deliberate filter-bypassing path.
- [ ] P2: exactly one channel consumer, preferably unrepresentable otherwise.
- [ ] P2 ablated with a second consumer.
- [ ] Modal exclusivity: mechanism stated, and proven under a live positive control.
- [ ] Output-vs-input asymmetry addressed explicitly.

## Removal (PR-A1-C)

- [ ] 50 ms tick gone; no polling path remains.
- [ ] 10 ms `WouldBlock` sleep gone.
- [ ] 64 KiB truncation behaviour gone, not merely unreached.
- [ ] No test amended to keep passing; any that needed it reported as a finding.

## Measurement and honesty (PR-A1-D)

- [ ] `NFR-PERF-004` measured, non-contamination proven per criterion.
- [ ] `iced::window::frames()` not reintroduced.
- [ ] Throughput re-measured against the ~374 KB/s baseline.
- [ ] `terminal_session_limit` raised from a new measurement taken after the tick is gone.
- [ ] Claim statement checked against the amendment's own text.
- [ ] `future-work.md` updated in the same commit.
- [ ] If `NFR-PERF-004` is still unmet, recorded as the honest outcome.

## Final Acceptance Decision

- [ ] Accepted
- [ ] Accepted with required follow-up
- [ ] Rejected

Reviewer notes:
