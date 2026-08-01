---
title: "RFC-015: Application Shell and Rendered Surface Model - Implementation Handoff"
rfc: "RFC-015"
rfc_file: "../../done/015-application-shell-and-rendered-surface-model.md"
target_milestone: "M8"
source_rfc_status: "Proposed"
created: "2026-07-29"
updated: "2026-07-29"
---

# RFC-015 Implementation Handoff

Covers PR-015-B through PR-015-G. This is **product code**, not a spike — thorough tests are expected, and the `crates/tekstide` binary stops being a text harness.

> **PR-015-C has its own document.** Read [`pr-015-c-input-routing.md`](./pr-015-c-input-routing.md) before designing the message enum, not after.

## 1. Where this code lives

The shell becomes the real `crates/tekstide` binary. Suggested layout:

```
crates/tekstide/src/main.rs        entry point, iced application boot
crates/tekstide/src/shell.rs       layer composition, update/view root
crates/tekstide/src/input.rs       the three input classes and the router
crates/tekstide/src/theme.rs       Theme value + compiled default (RFC-023 seam)
crates/tekstide/src/i18n.rs        string lookup seam + English default (RFC-016 seam)
crates/tekstide/src/surface.rs     Surface contract
crates/tekstide/src/surface/board.rs   Project Board surface
```

`iced` moves from the spike into `crates/tekstide`'s own `[dependencies]`, referenced via `.workspace = true` — the workspace table already declares it. **`crates/tekstide-core` must not gain a GUI dependency.** The core/runtime boundary has held from RFC-008 through RFC-013 and is what makes a future substrate change survivable (RFC-014 R2 obligation 3).

## 2. The shell is a view — the rule that matters most

```
Message → update() → AppCommand → ApplicationShell::dispatch()
view()  ← shell.state()
```

**Do not add state to the shell that mirrors core state.** This is the most likely architectural drift in this RFC, and it is insidious: a shell-local `Vec<Project>` "for rendering convenience" works fine until it diverges from core after a dispatch, and then produces a UI showing something that is not true.

Legitimate shell-local state is limited to genuinely presentational concerns: which zone has focus, whether a modal is open, scroll offsets, measurement counters.

If rendering appears to need state the core does not expose, **stop and raise it**. Either the model already expresses it and you have not found it, or the model has a real gap that belongs in a core change with its own review — not a shell shadow copy.

Reuse `KeybindingPolicy::linux_mvp()` for bindings. Do not invent keybindings; RFC-003 already settled them and `KeybindingPolicy::binding_is_reserved_for` exists to check conflicts.

## 3. Layer composition

```
Chrome  (top bar, status bar)          trusted
Content (sidebar + active surface)     may contain untrusted content
Modal   (dialogs)                      trusted, exclusive
```

Use `stack`/`opaque`, the composition proven by RFC-014 C8 — the spike's screenshot evidence is the reference for what "structurally separable" looks like when rendered.

**Surface code cannot reach the modal layer.** A surface may emit a message requesting a dialog; only shell code may open, populate, or dismiss one. Enforce with module privacy, not convention.

## 4. Seams — build them first, not last

### i18n (RFC-016 fills this)

Every user-facing string goes through a lookup from the first line of shell code:

```rust
t("project_board.title")   // not "Project Board"
```

English default returned from a compiled map for now. RFC-016 adds catalogs, locale selection, fallback, pluralization, and RTL policy behind the same call.

**Enforce mechanically where practical** — a test that greps the shell crate for string literals in widget constructors will catch regressions far more reliably than review attention.

### Theme and typography (RFC-023 fills this)

A `Theme` value carries colours, font families, and sizes, with a compiled default. **No widget hardcodes a colour or a font size.** `NFR-UX-004` requires these configurable; RFC-023 will supply them from configuration.

`NFR-UX-002` is binding: **status must never rely on colour alone.** The spike's `[focused]` text prefix beside a border is the reference pattern — the border alone would not satisfy it.

## 5. Project Board surface — `CountDisplay` fidelity

The Project Board renders `ApplicationShell` state: project name, branch, trust state, terminal count, AgentRun state, pending approvals, last activity.

**A count that is `Unavailable` or `NotImplemented` must never render as `0`.** RFC-005 built `CountDisplay` deliberately to avoid fake zeroes, and a rendered surface is exactly where that distinction gets flattened by a careless `unwrap_or(0)`.

Render them as distinguishable text — `—`, `n/a`, or similar — and write the test that proves it.

## 6. Discharging R1 (PR-015-F)

RFC-014's C2/C3/C4 are **unverified**, not met: `iced::window::frames()` forces continuous redraw once subscribed, so input-to-frame measured frame availability rather than rendering cost.

Requirements for this slice:

- Instrumentation **built into the shell** behind a measurement flag, not bolted on externally.
- **Prove it does not force redraw when inactive** — idle-CPU comparison, the same method the spike used to find the problem.
- If a non-contaminating input-to-frame path still cannot be found, **measure input-to-state-change and frame cost separately** and report the decomposition. A decomposed honest answer beats another degenerate combined figure.
- Release builds only, ≥1,000 samples, p50/p95/p99, machine identification. Latency described as **app-internal**, not end-to-end.
- The **survivorship-bias** caveat from RFC-014 R9 applies to any synthetic-input harness reused here: if delivery loss correlates with the app being busy, confirmed-only percentiles are optimistic. Record loss rates.

Budgets: warm start ≤ 800 ms (`NFR-PERF-001`; spike measured 227.9 ms median), mode switch p95 ≤ 32 ms (`NFR-PERF-002`), typing p95 ≤ 16 ms (`NFR-PERF-003`).

Escalation policy from RFC-014's handoff §5 still applies: a >2× miss stops work and is reported when confirmed, not at closeout.

## 7. Accessibility

- Visible, non-colour-dependent focus indicators on every focusable element.
- Focus trapping in the modal layer with a **real test** — RFC-014 R6 explicitly requires this upgrade over the spike's structural argument.
- Every shell workflow keyboard-reachable (`NFR-UX-001`).
- **Screen-reader support is absent** and stays absent while `iced` has no accessibility bridge. This was accepted by the owner (RFC-014 R2). **Do not add a partial or simulated accessibility affordance** that implies support exists — that would be worse than the honest absence.

## 8. What you must not build

- Terminal rendering — RFC-017.
- Editor, explorer, diff, or AgentRun report surfaces — RFC-019, RFC-020.
- Real security dialogs — RFC-022. You build the modal *layer*; a placeholder dialog for testing the layer is fine and should be clearly marked as such.
- Locale catalogs — RFC-016.
- Configuration loading — RFC-023.
- Multi-window.
- Any change to `tekstide-core` state models without raising it first.

## 9. Gates

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets --all-features
git diff --check
```

Product code — tests in `src/some_mod/tests.rs` per project convention, not inline `#[test]` modules.
