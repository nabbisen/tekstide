---
title: "RFC-017: Terminal Renderer and Immersion Mode - Task Breakdown / PR Plan"
rfc: "RFC-017"
rfc_file: "../../proposed/017-terminal-renderer-and-immersion-mode.md"
status: "Accepted 2026-08-01 — ready for implementation"
target_milestone: "M9"
created: "2026-08-01"
---

# RFC-017 Task Breakdown

Eight slices. **PR-017-B is the security-critical one and has its own document** ([`pr-017-b-filter-promotion.md`](./pr-017-b-filter-promotion.md)) — read it before writing code.

## PR-017-A — Design and handoff acceptance

Granted 2026-08-01 with the RFC. Nothing to implement.

## PR-017-B — Filter promotion

Scope: `SecurityFilter` from spike to product code, delegating to `tekstide-core`'s existing classifier; P1-P4 re-proven including adversarially chunked input.

Review gate:

- P1-P4 each **independently ablated**, one ablation per property.
- The single-ingress and side-channel enumerations written out, not summarised.
- `cargo tree -p tekstide-core --edges normal | grep -ciE 'vte|alacritty'` → `0`.
- Split corpus: every family, every internal byte boundary, generated not hand-written; classification and observable grid effect identical to the unsplit case.
- Unsupported families produce **no observable grid effect**, compared on full grid state.
- Truncated/malformed sequences fail closed without unbounded buffering.

**If P1-P4 cannot be re-established, stop and escalate rather than proceeding to C.** Option B (own the parser) is live.

## PR-017-C — Terminal pane rendering

Scope: the emulator grid rendered as a surface under RFC-015's contract. No input yet.

**P1 and P2 re-open here** (review 144). PR-017-B established them against a crate whose only `Term` construction is in a test harness — a true statement about a system with no production caller. This slice builds the first code that constructs an emulator for real, and "no second ingress exists" cannot be finally settled against code that did not yet exist. Re-enumerate and re-ablate both against production code; do not treat PR-017-B's enumeration as the final word.

Review gate:

- Surface contract holds: no state duplicating `tekstide-core`, cannot render trusted chrome, cannot reach modal state.
- Grid renders unescaped (the RFC-016 exception); **any chrome around it goes through `text_safety`** — session titles, pane headers, tooltips.
- Bounded scrollback, with the bound stated and tested under sustained output.
- Screenshot of a real PTY session, with what it proves and does not stated explicitly.
- **Rendering tests compare full grid-plus-cursor state against a pristine baseline**, not marker presence or absence on one line. PR-017-B set this standard for its own corpus, above what its gate asked for; it carries here because a rendering test that checks only what it expects to see is the failure class this project has hit six times — a test that passes for the wrong reason.

## PR-017-D — Input

Scope: `TextStream` with a real `TerminalId`; modal exclusivity and global-keybinding precedence verified under a live terminal; the Tab decision made.

Review gate:

- **Modal exclusivity demonstrated, not argued**: a dialog open means no PTY write. RFC-015 proved this headless with `terminal_focus` hard-coded `None`; it must be re-proven with a real terminal.
- `TextStream` still cannot address shell or modal state.
- Global keybindings still win over terminal focus — a terminal that swallows shell navigation is one the user cannot escape.
- **The Tab decision made, recorded with its reasoning, and its escape hatch tested** — the hatch must not depend on the terminal cooperating.

## PR-017-E — Immersion mode, split policy, session bar

Scope: at most two visible panes; split from real font metrics and DPI; session bar with non-colour-reliant state; hidden-session handling.

**This is the slice that gives the terminal pane chrome, and two obligations land with it** (review 148):

1. **The `terminal.rs` colour-scan exemption must narrow or move.** PR-017-C exempted the file because its single `Color::from_rgb` builds a grid cell's *PTY-determined* colour — per-cell data no theme defines. That justification is about the call, not the file, and `is_scan_exempt` matches on file name. A session bar or pane focus border is chrome, wants theme colours, and would be silently permitted to hardcode them. Narrow the exemption, move the grid rendering, or add a companion check — but do not leave a file-level exemption justified by a claim that has stopped being true.
2. **The RFC-016 grid-not-chrome boundary becomes live for the first time.** Nothing PTY-derived currently reaches chrome because the pane has no chrome — a true statement about an absence, not a property being enforced. A session title derived from OSC 0 is untrusted text in trusted chrome and goes through `tekstide_core::text_safety`. This is the first slice where that boundary can actually be violated.

Review gate:

- Uses `TerminalPanePolicy`/`TerminalLayoutClass`/`visible_terminal_limit` from `tekstide-core::navigation` — **no parallel layout model**.
- Split driven by real font metrics; a split producing panes below the minimum column count is refused, not rendered.
- `NFR-UX-002`: session state distinguishable without colour, including for hidden sessions.
- **The hidden-session grid-state decision made**, against the bounded-scrollback decision rather than separately from it.
- Screenshots of both split and single-pane layouts.

## PR-017-F — `plain_terminal_observation` audit producer

Scope: wire the family that already exists in the frozen v1 schema and has no producer.

**This slice makes a README claim false, and must fix it in the same change** (review 145). `README.md` §Local Data and Privacy currently states that the desktop application creates *only* the recent-projects list, and that running `tekstide` "does not create an audit database or retain any transcripts" — verified true at `0.4.1`. This is the first audit producer with a real GUI caller, so from here `tekstide` creates an audit database on the user's machine. Update the privacy section to say where it lives, what it holds, and how to purge it, citing RFC-013's policy. A privacy claim that silently expires is worse than one that was never made.

Review gate:

- Conforms to the frozen family; **no schema amendment.** If one seems needed, that is an RFC-013 amendment with owner authorisation — see RFC-013 Amendment 1 for the shape and the migration it required.
- **Sentinel privacy test**: no command text, no output, no path reaches the durable store. Probe raw bytes on disk, not just the typed query — the shape RFC-021 PR-021-E2 used.
- Written via `AuditCoordinator`, not directly to the store.

## PR-017-G — Measurement: `NFR-PERF-004`

Scope: terminal input latency p95 ≤ 16 ms **under bounded background output**.

Review gate:

- **Under flood.** Latency on an idle terminal measures nothing, and flood is where P4 failures surface.
- Reuses PR-015-F's harness; **`iced::window::frames()` is not reintroduced** — it forced continuous redraw and produced RFC-014's degenerate all-`0µs` results.
- Non-contamination proven for this criterion, not inherited.
- p50/p95/p99 and max, delivery-loss rate reported, stopping on confirmed on-disk sample counts rather than dispatched ones (R9).
- **Another all-zero figure is not an acceptable outcome.**

## PR-017-H — Closeout evidence

Scope: checklist, QA evidence, known limitations, answers to the RFC's open questions, and an explicit statement of what may be claimed about the terminal surface.

Review gate: the claim statement must survive the honesty test. In particular it **may not claim trusted-UI separation or spoofing resistance** — that is RFC-018, and RFC-014 PR-014-D's spike screenshot does not transfer.

## Sequencing

**B → C is strict.** D needs C. E needs C. F is independent of D/E and can run in parallel. G needs D and E. H needs all.

```
A ─→ B ─→ C ─┬─→ D ─┬─→ G ─→ H
             ├─→ E ─┘
             └─→ F ─────────┘
```
