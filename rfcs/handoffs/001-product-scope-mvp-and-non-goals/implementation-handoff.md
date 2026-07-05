# RFC-001 Implementation Handoff

Source RFC: [RFC-001](../../proposed/001-product-scope-mvp-and-non-goals.md)
Target: foundation planning for `v0.1.0`

## Purpose

Preserve the MVP scope boundary while later implementation RFCs and code work proceed.

## Decisions To Preserve

- Tekstide is a local-first, multi-project AI CLI workbench.
- Project Board is MVP-critical.
- AgentRun is MVP-critical.
- Linux is the first implementation and verification target.
- Editor richness, full Git GUI, plugins, remote/container development, debugger, multi-window, marketplace, and cloud/provider account management are deferred.
- Command approval must not be promised for arbitrary terminals or unsupported AI CLIs.

## Implementation Guidance

- Use this RFC as a scope gate for later RFCs and implementation handoffs.
- Reject scope additions that pull deferred features into `v0.1.0` without a later accepted RFC.
- Keep feature flags or abstraction points small and dormant; do not ship deferred behavior through them.
- Make every user-visible feature traceable to MVP scope or an explicit later RFC.

## Stop Conditions

Pause for RFC amendment if implementation requires public plugins, remote/container sessions, debugger behavior, multi-window orchestration, built-in cloud AI provider accounts, or universal command interception.
