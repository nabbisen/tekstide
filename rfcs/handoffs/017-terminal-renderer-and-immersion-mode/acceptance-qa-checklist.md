---
title: "RFC-017: Terminal Renderer and Immersion Mode - Acceptance / QA Checklist"
rfc: "RFC-017"
rfc_file: "../../proposed/017-terminal-renderer-and-immersion-mode.md"
status: "Accepted 2026-08-01 — pending implementation"
target_milestone: "M9"
created: "2026-08-01"
---

# RFC-017 Acceptance / QA Checklist

**A checked box means evidence exists.** A requirement that could not be met stays unchecked with a stated reason in `qa-evidence.md`. Do not edit a requirement so the implementation satisfies it.

## Filter Checklist (PR-017-B)

- [ ] **P1 — single ingress.** Every PTY byte reaches the emulator through one filter entry point; enumeration written out, not summarised.
- [ ] **P2 — no side channels.** Every mutating emulator API enumerated and shown unreachable or wrapped. Resize identified as a second, non-byte input and bounded.
- [ ] **P3 — classification parity.** One parse, shared between filter and emulator. No separate pre-scan pass.
- [ ] **P4 — stream-position independence.** Every family split at every internal byte boundary; classification and grid effect identical to unsplit.
- [ ] Each of P1-P4 **independently ablated**.
- [ ] `cargo tree -p tekstide-core --edges normal | grep -ciE 'vte|alacritty'` → `0`.
- [ ] The shell-side filter contains **no policy decision** — no conditional deciding acceptability.
- [ ] Unsupported families produce **no observable grid effect**, compared on full grid state.
- [ ] Accepted families produce the effect core says they should — the boundary proven in both directions.
- [ ] Truncated/malformed sequences fail closed without unbounded buffering.

## Surface Checklist (PR-017-C, PR-017-E)

- [ ] Pane holds no state duplicating `tekstide-core`.
- [ ] Pane cannot render trusted chrome and cannot reach modal state.
- [ ] **The RFC-016 exception is the grid only** — session titles, pane headers, and tooltips derived from output go through `text_safety`.
- [ ] Grid renders as data; nothing from PTY bytes occupies, overlaps, or imitates Tekstide's own chrome.
- [ ] Bounded scrollback, bound stated, tested under sustained output.
- [ ] Uses `TerminalPanePolicy`/`TerminalLayoutClass`/`visible_terminal_limit` — no parallel layout model.
- [ ] Split driven by real font metrics and DPI; sub-minimum-column splits refused.
- [ ] Session state distinguishable without colour (`NFR-UX-002`), including hidden sessions.
- [ ] Hidden-session grid-state decision made and recorded against the scrollback bound.

## Input Checklist (PR-017-D)

- [ ] **Modal open ⇒ no PTY write, demonstrated with a live terminal**, not argued.
- [ ] `TextStream` cannot address shell or modal state.
- [ ] Global keybindings win over terminal focus.
- [ ] Tab decision made, reasoning recorded, escape hatch tested — and the hatch does not depend on the terminal cooperating.

## Audit Checklist (PR-017-F)

- [ ] `plain_terminal_observation` conforms to the frozen v1 family; **schema unamended**.
- [ ] Written via `AuditCoordinator`, not directly to the store.
- [ ] **Sentinel test on raw on-disk bytes**: no command text, output, or path reaches the durable store.

## Performance Checklist (PR-017-G)

- [ ] `NFR-PERF-004` p95 ≤ 16 ms **under bounded background output**.
- [ ] `iced::window::frames()` not reintroduced.
- [ ] Non-contamination proven for this criterion by idle-CPU comparison.
- [ ] p50/p95/p99 and max reported; delivery loss reported; stopped on confirmed on-disk counts.
- [ ] Result is non-degenerate.

## Honesty Checklist (PR-017-H)

- [ ] Closeout states what may be claimed about the terminal surface.
- [ ] **No claim of trusted-UI separation or spoofing resistance** — RFC-018 owns it.
- [ ] **RFC-014 PR-014-D's screenshot is not cited** as evidence for the product's boundary.
- [ ] Screenshots each state what they prove **and do not**.
- [ ] Every unchecked line above carries a stated reason.

## Evidence Required

- [ ] Commit/PR list.
- [ ] Gate command output.
- [ ] P1-P4 ablation results.
- [ ] Split-corpus generation method and case count.
- [ ] Sentinel privacy test result.
- [ ] `NFR-PERF-004` measurement under flood.
- [ ] Known limitations.
- [ ] Answers to the RFC's open questions.

## Final Acceptance Decision

- [ ] Accepted.
- [ ] Accepted with required follow-up.
- [ ] Requires re-review after changes.
- [ ] Blocked — the filter cannot be shown single-ingress in product code (Option B fallback).

Reviewer notes:

```text
Pending implementation.
```
