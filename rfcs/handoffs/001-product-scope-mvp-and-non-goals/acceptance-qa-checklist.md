# RFC-001 Acceptance / QA Checklist

Source RFC: [RFC-001](../../done/001-product-scope-mvp-and-non-goals.md)

This checklist records the accepted `0.1.0` foundation release-scope evidence. Final publish/tag evidence is still handled by the release checklist.

## Scope Checks

- [x] Project Board remains in `0.1.0` foundation scope.
- [x] ProjectSession, navigation, security policy, root policy, explorer, and text document foundations remain in `0.1.0` scope.
- [x] AgentRun runtime remains tracked as post-`0.1.0` future work.
- [x] First release target is `0.1.0`.
- [x] Linux-first implementation/verification target is documented.
- [x] Deferred features are listed without being represented as `0.1.0` blockers.
- [x] UI and docs do not imply arbitrary terminal command approval.
- [x] UI and docs do not imply terminal/PTY, AgentRun launch, transcript/review, desktop GUI, or durable audit behavior is implemented.

## Evidence Required

- [x] List of implementation RFCs or tasks checked against RFC-001 scope. Covered by RFC-002 through RFC-006 and RC review request 032.
- [x] Known limitations for any intentionally deferred foundation-adjacent feature. Covered by `CHANGELOG.md`, `rfcs/future-work.md`, and RC review request 032.
- [x] Future-work list for terminal/PTY, AgentRun, transcript/review, desktop GUI, durable audit, and release process. Covered by `rfcs/future-work.md`.
- [x] Security note confirming no new cloud/network/provider-account behavior. Covered by RFC-004, RFC-006 closeout evidence, and RC review request 032.

## Final Notes

RFC-001 is accepted as the `0.1.0` foundation release scope. Publishing remains subject to `rfcs/handoffs/release-checklist.md`.
