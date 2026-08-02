---
title: "RFC-017: Terminal Renderer and Immersion Mode - Acceptance / QA Checklist"
rfc: "RFC-017"
rfc_file: "../../proposed/017-terminal-renderer-and-immersion-mode.md"
status: "Accepted 2026-08-01 — PR-017-B/C/D reviewed and approved (responses 144-149); PR-017-E (immersion mode, split policy, session bar) implemented 2026-08-03, pending review"
target_milestone: "M9"
created: "2026-08-01"
---

# RFC-017 Acceptance / QA Checklist

**A checked box means evidence exists.** A requirement that could not be met stays unchecked with a stated reason in `qa-evidence.md`. Do not edit a requirement so the implementation satisfies it.

## Filter Checklist (PR-017-B)

- [x] **P1 — single ingress.** Enumerated, not summarised: `grep -rl "alacritty_terminal\|vte::" crates/tekstide/src/` finds exactly two files (`filter.rs`, `filter/tests.rs`); the only `Term`/`Processor` construction and the only `.advance()` call are in the test harness, always through one `SecurityFilter::new(&mut term)` per chunk. **Partial claim, disclosed**: no production caller exists yet (PR-017-C builds the pane), so this is the enumeration available today, not a system-wide guarantee re-established over real production code — that re-check is PR-017-C's.
- [x] **P2 — no side channels.** `Term::grid_mut()` confirmed unreachable from the PTY byte path by reading `vte` 0.15's dispatch tables directly; it appears in this crate only inside doc-comment prose, never as a call. Same partial-claim disclosure as P1 — no real pane exists yet to have a second input (e.g. resize) to identify and bound; that is PR-017-C's job, not asserted here.
- [x] **P3 — classification parity.** One `vte::ansi::Processor` shared per chunk sequence, unchanged from the spike's own construction; no separate pre-scan pass exists.
- [x] **P4 — stream-position independence.** `every_named_family_blocks_with_no_grid_effect_at_every_split_boundary` (8 families, 80 generated split points, 88 total cases) plus the carried V2/V4/V5/V7 findings from the spike.
- [x] Each of P1-P4 **independently ablated**. P1: simulated a second ingress (unconditional `set_title` forwarding) — the corpus test failed naming `osc_title`. P3/P4 together: fresh `Processor` per chunk instead of one shared instance — four independent tests failed for four different reasons (title leak, DCS leak, OSC-52 split misclassification, UTF-8 reassembly failure). Both reverted and re-confirmed passing. See `qa-evidence.md` for the exact diffs and failure output.
- [x] `cargo tree -p tekstide-core --edges normal | grep -ciE 'vte|alacritty'` → `0`.
- [x] The shell-side filter contains **no policy decision**. Every accepted method's forward call is gated on `tekstide_core::runtime::terminal::security::TerminalSequencePolicy::ACCEPTED.contains(...)`, asked live at the call site — ablation-verified by removing `CsiClearScreen` from that list in `tekstide-core` and confirming the shell-crate filter's forwarding behaviour changed with it (three tests failed, none of them in `tekstide-core`'s own suite).
- [x] Unsupported families produce **no observable grid effect**, compared on full grid state. `grid_snapshot` (full grid text across all lines, plus cursor position) asserted equal to a pristine baseline for every corpus case and every split point — stronger than the spike's own marker-on-line-0 check.
- [x] Accepted families produce the effect core says they should — the boundary proven in both directions. `accepted_printable_text_reaches_the_grid`, `accepted_sgr_cursor_and_clear_do_not_block`, `accepted_c0_controls_do_not_block`, `accepted_clear_screen_actually_clears_previously_written_text` (the last checking the real grid effect, not just the blocked-list, after a gap in that direction was found by ablation and fixed — see `qa-evidence.md`).
- [x] Truncated/malformed sequences fail closed without unbounded buffering. `vte::ansi::Processor`'s own fixed-capacity parameter/intermediate buffers handle this; this filter never buffers anything of its own. `v5_parameter_overflow_does_not_desync_the_parser`/`v5_parameter_overflow_followed_by_osc_52_still_blocks_clipboard` (carried from the spike) prove parameter-list length cannot be used to desync classification of what follows.

## Surface Checklist (PR-017-C, PR-017-E)

- [x] Pane holds no state duplicating `tekstide-core`. `TerminalPane`'s fields are its own rendering state (`term`, `processor`) or a handle back to `tekstide-core`'s runtime (`runtime`, `handle`) — nothing shadows `ProjectSession`/`ApplicationShell` state.
- [x] Pane cannot render trusted chrome and cannot reach modal state. `TerminalPane::view` takes `&TerminalPane`/`font_size` only — no `&shell::State`, no path to `state.modal` or chrome fields, the same shape `surface::board::view` uses.
- [x] **The RFC-016 exception is the grid only** — session titles, pane headers, and tooltips derived from output go through `text_safety`. No chrome element derived from terminal output exists in this slice (session titles/pane headers are PR-017-E's `session_bar.rs`) for the exception boundary to be tested against yet; the grid itself renders unescaped, deliberately, per the exception.
- [x] Grid renders as data; nothing from PTY bytes occupies, overlaps, or imitates Tekstide's own chrome. `main_area_view` substitutes the pane only for the inner content of the existing zone container — the chrome-level focus border is unaffected (screenshot evidence, `qa-evidence.md`).
- [x] Bounded scrollback, bound stated, tested under sustained output. `SCROLLBACK_LINES = 2_000`; ablation-verified (`qa-evidence.md`).
- [x] Uses `TerminalPanePolicy`/`TerminalLayoutClass`/`visible_terminal_limit` — no parallel layout model. `launch_terminal_demo_panes` registers real sessions via `AppState::attach_terminal_session`/`assign_terminal_visible_slot` (new, delegating straight to `ProjectSession`'s existing methods); `active_project_terminal_sessions`/`terminal_workspace_view` read slot state fresh from `tekstide-core` every call, no shell-local bookkeeping.
- [x] Split driven by real font metrics and DPI; sub-minimum-column splits refused. `layout_class_for` measures the real monospace glyph advance at the pane's actual render size and refuses a two-pane split below a full pane's worth of real columns (`COLS` = 80), rendering one pane instead — proven with 7 unit tests and a real window-resize screenshot (`qa-evidence.md`).
- [x] Session state distinguishable without colour (`NFR-UX-002`), including hidden sessions. `session_bar::view` renders slot and status as distinct text labels for every registered session, hidden included — proven distinct (`every_slot_and_status_has_a_distinct_textual_label`) and shown live in the screenshot evidence.
- [x] Hidden-session grid-state decision made and recorded against the scrollback bound. **Decided: retained in memory, always polled, not torn down** — the bound does not change with visibility, and session count is itself bounded (`terminal_session_limit`). Demonstrated (not only argued): a hidden pane keeps accumulating real PTY output across the real `TerminalDemoTick` → `update` path and retains it across a later slot reassignment; ablated by simulating "poll only visible panes" and confirming the hidden pane's content is then missed.

## Input Checklist (PR-017-D)

- [x] **Modal open ⇒ no PTY write, demonstrated with a live terminal**, not argued. `modal_open_blocks_pty_write_and_closing_it_resumes_delivery` sends the same `TextStream` at a real, launched `TerminalPane`, once with `state.modal` set and once cleared, polling the real PTY both times — not a synthetic id, not an assertion about `update`'s return value. Ablated: removing the `state.modal.is_none()` guard makes the blocked half of this test fail immediately. **No GUI screenshot for this one**: the demo modal only ever opens once, at boot (`TEKSTIDE_LAYER_DEMO`, read once), with no runtime trigger to reopen it — there is no real user-accessible sequence that gets Terminal Mode active *and* the modal open at the same time to screenshot. The live-`TerminalPane` test above is the demonstration; see `qa-evidence.md`.
- [x] `TextStream` cannot address shell or modal state. Unchanged from RFC-015 — `to_pty_bytes` converts the key it already carries, never reaches into `shell::State`.
- [x] Global keybindings win over terminal focus. Unchanged routing precedence (`input::tests::a_global_keybinding_wins_over_a_focused_terminal`), now also proven with a real, live terminal id in `shell::tests` rather than only a synthetic one.
- [x] Tab decision made, reasoning recorded, escape hatch tested — and the hatch does not depend on the terminal cooperating. **Decided: Tab never reaches the terminal**, recorded in `input`'s own module doc. Escape hatch is structural (Tab is intercepted in routing before `terminal_focus` is even consulted), tested against a real, live terminal (`tab_cycles_shell_focus_with_a_real_terminal_focused_and_writes_nothing`) and ablated: swapping the two precedence checks makes both this test and RFC-015's original headless one fail.

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
