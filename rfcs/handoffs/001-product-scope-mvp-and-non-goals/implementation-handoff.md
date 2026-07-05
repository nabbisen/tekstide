# RFC-001 Implementation Handoff

Source RFC: [RFC-001](../../proposed/001-product-scope-mvp-and-non-goals.md)
Target: foundation release planning for `0.1.0`

## Purpose

Preserve the `0.1.0` foundation scope boundary while later implementation RFCs and code work proceed.

## Decisions To Preserve

- Tekstide is a local-first, multi-project AI CLI workbench.
- `0.1.0` is scoped as a core/shell foundation through RFC-006.
- Project Board and ProjectSession state are `0.1.0` critical.
- Root-bound file access, bounded explorer state, text document state, Restricted Mode policy, and shell-visible Content Mode evidence are `0.1.0` critical.
- AgentRun runtime, terminal/PTY runtime, transcript/review flow, desktop GUI, and durable audit storage are post-`0.1.0` future work.
- Linux is the first implementation and verification target.
- Editor richness, full Git GUI, plugins, remote/container development, debugger, multi-window, marketplace, and cloud/provider account management are deferred.
- Command approval must not be promised for arbitrary terminals or unsupported AI CLIs.

## Implementation Guidance

- Use this RFC as a scope gate for later RFCs and implementation handoffs.
- Reject scope additions that pull deferred features into `0.1.0` without a later accepted RFC.
- Keep feature flags or abstraction points small and dormant; do not ship deferred behavior through them.
- Make every user-visible feature traceable to foundation release scope or an explicit later RFC.
- Preserve the future-work themes in release notes, review requests, and later handoffs.

## Stop Conditions

Pause for RFC amendment if release preparation starts to claim terminal/PTY runtime, AgentRun launch, transcript/review flow, desktop GUI, durable audit storage, public plugins, remote/container sessions, debugger behavior, multi-window orchestration, built-in cloud AI provider accounts, or universal command interception.
