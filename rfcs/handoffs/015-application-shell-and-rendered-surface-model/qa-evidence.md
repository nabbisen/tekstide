# RFC-015: Application Shell and Rendered Surface Model - QA Evidence

Status: Proposed — implementation pending
Date opened: 2026-07-29
Date accepted: Pending

## Scope

RFC-015 builds the application shell and defines the rendered-surface contract, layer model, and input routing that RFC-017, RFC-019, RFC-020, and RFC-022 all build on.

Evidence in this file must not be used to claim terminal rendering, editor/explorer/diff/report surfaces, real security dialogs, locale catalogs, configuration loading, multi-window support, or screen-reader accessibility — unless later reviewed implementation explicitly supports that claim.

## Inherited obligations

Carried in from RFC-014's approved decision record:

- **R1 — latency unverified.** C2/C3/C4 were not measured; `iced::window::frames()` forces continuous redraw once subscribed. **PR-015-F discharges this or re-records the residual honestly.** Another all-zero figure is not an acceptable outcome.
- **R6 — focus-trap property does not transfer.** The spike's property held only because its terminal emitted no messages. **PR-015-C must re-establish it under an input-accepting design**, with a real test rather than a structural argument.
- **R2 — no screen-reader support**, owner-accepted 2026-07-29. Public claims must state the limitation; no simulated affordance.
- **R9 — survivorship bias** in confirmed-only percentiles applies to any reused synthetic-input harness.

## Design Review

Pending PR-015-A acceptance.

## Implementation Evidence

### PR-015-B — Window, layers, chrome, seams

Pending implementation.

### PR-015-C — Input routing and focus model

Pending implementation.

**Reminder:** this is the security-critical slice. The test of correct structure is that *deleting a guard condition produces a compile error, not a security regression*.

### PR-015-D — Project Board surface

Pending implementation.

### PR-015-E — Mode switching and Content-mode scaffolding

Pending implementation.

### PR-015-F — Measurement: R1 discharge

Pending implementation.

### PR-015-G — Closeout evidence

Pending implementation.

## Known Limitations

- Screen-reader support absent for the life of the `iced` substrate decision (RFC-014 R2, owner-accepted).
- Terminal rendering, editor, explorer, diff, and report surfaces are out of scope; the shell provides only the contract they plug into.
- The modal layer ships with a placeholder dialog for testing; real dialogs are RFC-022.
