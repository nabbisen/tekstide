---
title: "RFC-017: Terminal Renderer and Immersion Mode - Acceptance / QA Checklist"
rfc: "RFC-017"
rfc_file: "../../proposed/017-terminal-renderer-and-immersion-mode.md"
status: "Accepted 2026-08-01 — PR-017-B/C/D/E/F reviewed and approved (responses 144-153); PR-017-G (NFR-PERF-004) recorded not met 2026-08-03 (arithmetic verdict, owner ship/hold decision pending); PR-017-H closeout evidence complete, pending owner sign-off on the NFR-PERF-004 ship/hold trade"
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
- [x] Session state distinguishable without colour (`NFR-UX-002`), including hidden sessions. `session_bar::view` renders slot and status as distinct, catalog-resolved text labels for every registered session, hidden included — proven distinct over real resolved values, not hardcoded strings (response 150 Required; `every_slot_and_status_combination_resolves_to_distinct_text`) and shown live in the screenshot evidence.
- [x] Hidden-session grid-state decision made and recorded against the scrollback bound. **Decided: retained in memory, always polled, not torn down** — the bound does not change with visibility, and session count is itself bounded (`terminal_session_limit`). Demonstrated (not only argued): a hidden pane keeps accumulating real PTY output across the real `TerminalDemoTick` → `update` path and retains it across a later slot reassignment; ablated by simulating "poll only visible panes" and confirming the hidden pane's content is then missed.

## Input Checklist (PR-017-D)

- [x] **Modal open ⇒ no PTY write, demonstrated with a live terminal**, not argued. `modal_open_blocks_pty_write_and_closing_it_resumes_delivery` sends the same `TextStream` at a real, launched `TerminalPane`, once with `state.modal` set and once cleared, polling the real PTY both times — not a synthetic id, not an assertion about `update`'s return value. Ablated: removing the `state.modal.is_none()` guard makes the blocked half of this test fail immediately. **No GUI screenshot for this one**: the demo modal only ever opens once, at boot (`TEKSTIDE_LAYER_DEMO`, read once), with no runtime trigger to reopen it — there is no real user-accessible sequence that gets Terminal Mode active *and* the modal open at the same time to screenshot. The live-`TerminalPane` test above is the demonstration; see `qa-evidence.md`.
- [x] `TextStream` cannot address shell or modal state. Unchanged from RFC-015 — `to_pty_bytes` converts the key it already carries, never reaches into `shell::State`.
- [x] Global keybindings win over terminal focus. Unchanged routing precedence (`input::tests::a_global_keybinding_wins_over_a_focused_terminal`), now also proven with a real, live terminal id in `shell::tests` rather than only a synthetic one.
- [x] Tab decision made, reasoning recorded, escape hatch tested — and the hatch does not depend on the terminal cooperating. **Decided: Tab never reaches the terminal**, recorded in `input`'s own module doc. Escape hatch is structural (Tab is intercepted in routing before `terminal_focus` is even consulted), tested against a real, live terminal (`tab_cycles_shell_focus_with_a_real_terminal_focused_and_writes_nothing`) and ablated: swapping the two precedence checks makes both this test and RFC-015's original headless one fail.

## Audit Checklist (PR-017-F)

- [x] `plain_terminal_observation` conforms to the frozen v1 family; **schema unamended**. `record.validate()` asserted directly against a real producer's output; ablated by attaching a field `valid_plain_terminal` forbids and confirming the store's own validation rejects the write (`Degraded`, not `Persisted`).
- [x] Written via `AuditCoordinator`, not directly to the store. `AuditCoordinator::record_plain_terminal_started` is the only production call site in `crates/tekstide`; no direct `AuditStore::append` call exists there.
- [x] **Sentinel test on raw on-disk bytes**: no command text, output, or path reaches the durable store. `sentinel_terminal_derived_text_never_reaches_the_durable_audit_store` launches a real pane with sentinel-laden title/root path, checks both the typed query and raw bytes read off the real audit directory **after the store is dropped** (response 152 Required 2: the store's own write lives in the `-wal` sidecar while open, so scanning `database_file()` alone while the store was still open was a vacuous check — fixed by dropping the store first, which triggers SQLite's WAL checkpoint, and scanning every file under the audit directory). Re-ablated with a positive control (the real, persisted `terminal_id` must appear in the scan) against the original open-store scan, confirming that scan was blind to genuine content, not just insensitive to planted sentinels.
- [x] **The store is not created by ordinary use.** Response 152 Required 1: `open_real_audit_store` was called unconditionally in `State::new`, creating the (empty) database with full schema on every launch regardless of `TEKSTIDE_TERMINAL_DEMO` — probe-confirmed. Fixed by moving the open inside `launch_terminal_demo_panes`, behind both the env-var gate and the active-project check, so the store is opened only when a demo terminal is actually about to launch; README updated to match.

## Performance Checklist (PR-017-G)

- [x] **`iced::window::frames()` not reintroduced.** `Criterion::TerminalFlood` reuses `Typing`/`ModeSwitch`'s `record_input`/`measured_key_subscription` mechanism unchanged; only `Startup` uses `frames()`, untouched by this slice.
- [x] **`NFR-PERF-004` p95 ≤ 16 ms recorded as NOT MET, with the arithmetic reason.** `terminal_demo_subscription`'s 50ms poll tick is the only place PTY bytes reach the grid; poll-wait alone contributes an expected p95 of ~47.5ms (0.95 × 50ms), independent of any live run, before any pty/VTE/layout/paint cost — roughly 3× the budget. RFC-014 never verified this criterion at all; this is its first real verdict, not a regression. The fix (readiness-driven I/O instead of polling) is out of scope for this slice — it touches the P1/P2-proven ingress path and needs its own PR/RFC-scale review (`qa-evidence.md`).
- [x] **The saturation hypothesis answered, headlessly, without needing a GUI.** Three live-GUI attempts were blocked in turn by swap pressure then a machine-specific GPU/EGL driver failure (`qa-evidence.md`) — response 158 recognized the discriminator question (does the update loop keep up with a real flood) doesn't need a window at all, since `poll()` is pure CPU. `terminal_poll_handler_cost_under_a_real_flood_headless_benchmark` (new) launches a real pane under the real flood and times `poll()` on the real 50ms cadence: consistently ~10.3ms across three runs (~21% of the tick period), nowhere near saturation — real evidence, not an estimate, that the confounded ~1.1–1.2s live-run plateau was environment noise, not a structural defect in this crate's poll loop. Ablation-verified (injected a 600ms sleep, confirmed the regression guard catches it, reverted).
- [x] **Dropped bytes under a genuine flood: `0`, with the cause identified as a `tekstide-core` defect (response 159 correction), not a design characteristic.** The same headless benchmark measured observed in-app throughput at ~374KB/s against the flood script's own ~17.2MiB/s standalone rate (~46× lower). Arithmetic from the real numbers shows the reader sustains ~69MB/s for ~0.5% of each tick, sleeping through the rest: `read_available_bounded_for`'s 10ms `WouldBlock` sleep overshoots its own 5ms budget, so the loop reads once then exits. **This is why the 64KiB cap is never hit today, and fixing the sleep alone would unmask a real stream-truncation risk P4 does not cover** — the sleep fix and the cap's silent-truncation policy must be fixed together, recorded in `qa-evidence.md`'s Known Limitations for `tekstide-core`'s owner.
- [ ] **End-to-end p50/p95/p99/max under real synthetic input, delivery loss, and the non-contamination control (instrumentation on vs. off)** — still genuinely need the GUI event loop, which this machine's graphics stack currently can't provide (a machine-specific GPU/EGL driver issue, diagnosed and confirmed unrelated to this crate — `qa-evidence.md`). Per the reviewer, there is no urgency on these two; does not block the `NFR-PERF-004` verdict above, which is arithmetic.
- [x] **Result is non-degenerate** in the sense that matters: the confounded live numbers were recognized as non-credible and discarded rather than reported as a clean pass or a clean fail: the ~47.5ms arithmetic floor is real, reproducible from the code alone, and is what the not-met verdict above rests on.

## Honesty Checklist (PR-017-H)

- [x] Closeout states what may be claimed about the terminal surface. `qa-evidence.md`'s "The claim statement" section, PR-017-H.
- [x] **No claim of trusted-UI separation or spoofing resistance** — RFC-018 owns it. Stated explicitly; the grid-spoofing risk item in the RFC's own Risks section is recorded as RFC-018's to close, not this RFC's.
- [x] **RFC-014 PR-014-D's screenshot is not cited** as evidence for the product's boundary. Confirmed by construction — no PR-017 evidence section references it, and the claim statement names the trap explicitly.
- [x] Screenshots each state what they prove **and do not**. Confirmed per-slice in the Surface/Input Checklists above (PR-017-C/D/E each state what their screenshots do not prove: trusted-UI separation/spoofing resistance, real per-project terminal creation UX).
- [x] Every unchecked line above carries a stated reason. The two unchecked Performance Checklist items above (end-to-end GUI figure, non-contamination control) each state theirs: blocked by machine-specific issues, no urgency, don't affect the recorded verdict.

## Evidence Required

- [x] Commit/PR list. `qa-evidence.md`, PR-017-H.
- [x] Gate command output. `qa-evidence.md`, PR-017-H — same four gates, every slice, final state 497 + 120 + 18 + 0, 0 failures.
- [x] P1-P4 ablation results. `qa-evidence.md`, PR-017-H, pointing to the Filter Checklist's per-property detail.
- [x] Split-corpus generation method and case count. 8 families, 80 generated split points, 88 total cases.
- [x] Sentinel privacy test result. Fully approved, response 153.
- [x] `NFR-PERF-004` measurement under flood. Recorded not met, arithmetic + headless-benchmark-corroborated; two GUI-bound items open, no urgency.
- [x] Known limitations. Nine items consolidated in `qa-evidence.md`, PR-017-H.
- [x] Answers to the RFC's open questions. All three answered, PR-017-H.

## Final Acceptance Decision

- [ ] Accepted.
- [x] **Accepted with required follow-up** — the architect's recommendation (response 155/159): accept `NFR-PERF-004` recorded not met for `0.5.x`/M9, schedule Option B (readiness-driven terminal I/O) as scoped follow-up work. **Formal owner sign-off on this specific trade is the one item this closeout surfaces rather than resolves** — see Reviewer notes.
- [ ] Requires re-review after changes.
- [ ] Blocked — the filter cannot be shown single-ingress in product code (Option B fallback).

Reviewer notes:

```text
Pending the owner's confirmation of the ship/hold trade recorded above. Every other checklist item in this
document is checked with evidence, or unchecked with a stated reason that does not block closeout. The
architect's own recommendation stands as recorded in qa-evidence.md's PR-017-G section (responses 155, 159);
this file does not put words in the owner's mouth by checking "Accepted" outright before that confirmation
arrives.
```
