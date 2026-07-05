# RFC-001 Acceptance / QA Checklist

Source RFC: [RFC-001](../../proposed/001-product-scope-mvp-and-non-goals.md)

This checklist is the release-scope acceptance checklist. Items remain unchecked until the release-candidate package records final evidence.

## Scope Checks

- [ ] Project Board remains in `0.1.0` foundation scope.
- [ ] ProjectSession, navigation, security policy, root policy, explorer, and text document foundations remain in `0.1.0` scope.
- [ ] AgentRun runtime remains tracked as post-`0.1.0` future work.
- [ ] First release target is `0.1.0`.
- [ ] Linux-first implementation/verification target is documented.
- [ ] Deferred features are listed without being represented as `0.1.0` blockers.
- [ ] UI and docs do not imply arbitrary terminal command approval.
- [ ] UI and docs do not imply terminal/PTY, AgentRun launch, transcript/review, desktop GUI, or durable audit behavior is implemented.

## Evidence Required

- [ ] List of implementation RFCs or tasks checked against RFC-001 scope.
- [ ] Known limitations for any intentionally deferred foundation-adjacent feature.
- [ ] Future-work list for terminal/PTY, AgentRun, transcript/review, desktop GUI, durable audit, and release process.
- [ ] Security note confirming no new cloud/network/provider-account behavior.
