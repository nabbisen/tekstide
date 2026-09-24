---
title: "RFC-054 — acceptance and QA checklist"
rfc: "RFC-054"
rfc_file: "../../done/054-user-configuration-completion.md"
source_rfc_status: "Implemented and closed 2026-09-24 — M12 (closed it)"
target_milestone: "M12"
created: "2026-09-24"
---

# Acceptance and QA checklist

Every box is a property. A box whose plan assigns it elsewhere, or that cannot be satisfied as
written, stays unticked with the contradiction named — the reviewer's error to fix, not the
implementer's to paper over. *(Ablation boxes say "fails its own test, and names anything else that
shares its fixture" — "alone" has been falsified three times by suites sharing fixtures properly.)*

## PR-054-A — keybindings and the repair

- [x] **No project-local configuration is read** (§1). Pinned by a test with a `config.toml` beside
      the project root, and an **ablation** that makes the loader look there — the test fails. *(A behavioural test and a structural scan; the ablation trips the scan, and `qa-evidence.md` says why the behavioural test alone could not.)*
- [x] Every advertised chord **round-trips**: the spelling the Help modal and `--help` print is the
      spelling the file accepts.
- [x] A rebind colliding with another rule **refuses**; a `Reserved` chord **refuses**; the default
      stands and the board says why. **Ablation:** accept last-wins; the collision test fails.
- [x] **D3′'s repair:** every action ends with a real default binding **or** an explicitly dead
      status, distinguishable **by the type**. `CycleVisibleTerminalSession` and
      `OpenSafeCloseDialog` are decided, each with its reachability stated.
- [x] An action cannot be both bound and dead — held by the type, not by review.

- [x] **Required at review 427: pin the caller, not only the module.** `ConfigStore::load` takes any
      `PathBuf`, so a future caller passing a project-derived path is caught by neither the
      behavioural test nor the structural scan. Pin that it has **exactly one production call site**
      and that the path it receives is the one `ConfigPaths` derived — "the configuration comes from
      this path and no other", which is what D2 actually wants.

## PR-054-B — theme and fonts

- [x] A pair below **4.5:1** falls back, and the diagnostic carries the **measured ratio**.
      **Ablation:** drop the check; that test fails.
- [x] An unavailable font family falls back and says so; a size outside **8–32 px** falls back at
      each edge.
- [x] **A font is a name, never a path** — no configured string reaches a file open.
- [x] No diagnostic echoes a configured string unescaped (`quote_untrusted`).
- [x] The colour-alone and i18n completeness scans still pass.

### Required at review 428

- [x] **R1: the focus border meets 3:1** against the surfaces it is drawn on — WCAG's non-text
      minimum, the one `derived_contrast_pairs` already holds the defaults to. Accent and the
      non-focus border stay unmeasured **and the book says so**, as a decision rather than an
      oversight.
- [x] **R2: the scrim's alpha is capped**, with a diagnostic. RFC-018 wants it translucent so a
      backdrop cannot be mistaken for a rectangle the application did not draw; no configuration may
      produce a spoofable surface.
- [x] **`theme.rs`'s claim that every colour comes from a `Theme` value is corrected** — it is not
      true of the buttons.
- [x] **The buttons follow the theme**, if it is routing the existing roles into their styles. If it
      is more than that, ship with the limitation **stated** and it gets scheduled.
- [x] **`ARCHITECTURE.md` records the UI-font global** as the one piece of process-global UI state,
      with the reason (iced fixes the default font at build, and a live family needs one) — so the
      next author does not read it as permission for a second.

## PR-054-C — scrollback and the live proof

- [x] The scrollback cap is **measured**, not chosen: bytes per line at a realistic width, cap set so
      **one pane at the cap stays under 64 MB**, number and measurement in the book. Above it clamps
      with a diagnostic.
- [x] **`Ctrl+Alt+C` applies all five live**, and `Reserved` stays reserved across a reload.
- [x] **Live capture**: default dress → edit the file → `Ctrl+Alt+C` → the same window in the user's
      colours, font and size, with a rebound chord working. A second capture: a bad value falling
      back, named on the board. Throwaway `mktemp -d` config; **no floating or resizing** — this
      desktop is shared.

## Whole-RFC

- [x] `REQ-CONFIG-006`, `REQ-CONFIG-007`, `NFR-UX-004`, `REQ-TERM-004` move to implemented **with
      evidence they are reachable**, not merely parsed — the distinction the 2026-09-23 audit caught
      the plan getting wrong.
- [x] `cargo fmt`, `clippy --workspace --all-targets -D warnings`, `git diff --cached --check` after
      staging, `rfc_docs_invariants`, **three consecutive full-workspace runs with `--no-fail-fast`**
      to files, **0 fixture entries left** in a fresh `TMPDIR`.
- [x] Every new intermittent failure has a dated row in `test-process-leak.md`. *(None this slice: three
      consecutive runs green; the one failure seen, `bind_recovers_from_a_stale_socket_file` at load 11 during
      an ablation, is the register's original row.)*
- [x] Commits are pushed once the gate is green.

## Final Acceptance Decision

- [x] Accepted.
- [ ] Accepted with required follow-up.
- [ ] Requires re-review after changes.

Reviewer notes:

```text
Accepted 2026-09-24 (reviews 427-429). RFC-054 closed, and M12 with it.

D3' repaired a category error carried since RFC-022: Configurable/None read as "bindable" and meant
"dead", and two surfaces shipped unreachable because of it. Bound-and-dead is now unrepresentable,
and a death certificate carries a reachability claim -- which immediately surfaced that only the
Primary terminal receives keystrokes (recorded in the delivery plan; it needs a feature, not a chord).

The scrollback cap was measured and the measurement caught its author three times: a feeding buffer
counted in, rows allocated in blocks of 1,024 rather than per line, and a block count that is not
ceil(lines/1024). a_pane_at_the_cap_stays_under_the_memory_budget failed at 67,924,707 against
67,108,864 -- review did not find that, the allocator did.

Ruled at 429: the 12,000 cap stands; the book states the combining-character worst case in bytes
rather than as a multiplier; bounding combining marks per cell is RFC-061, on the terminal
boundary's ground, chosen against real scripts rather than against Zalgo.

Reviewer gate: 657 + 12 + 945, green three times, 0 fixture entries left.
```
