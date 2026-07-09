# RFC-007: Runtime Substrate and PTY Feasibility Gate — Developer Handoff Pack

Source RFC: [RFC-007](../../proposed/007-runtime-substrate-pty-feasibility.md)  
Target milestone: **M4 feasibility gate**  
Source RFC status: **Proposed**

## Files

- `implementation-handoff.md` — developer-facing spike constraints and evidence rules.
- `task-breakdown-pr-plan.md` — recommended spike sequence and review gates.
- `acceptance-qa-checklist.md` — acceptance traceability, QA checklist, and stop conditions.
- `qa-evidence.md` — template for observed spike evidence, measurements, security notes, and Go/No-Go recommendation.

Review disposition: RFC-007 was re-reviewed and accepted with notes on 2026-07-09.

This handoff inherits the source RFC lifecycle state. RFC-007 remains in `proposed/` until the RFC lifecycle moves it.

## Source Summary

RFC-007 defines a narrow TUI-first feasibility harness for proving Tekstide's minimum PTY loop before production terminal/process work begins. The spike must prove shell start, output render, input, resize, termination, foreground-child behavior, bounded output-flood handling, basic latency measurement, and initial terminal security observations on Linux.

