---
title: "RFC-054 — acceptance and QA checklist"
rfc: "RFC-054"
rfc_file: "../../accepted/054-user-configuration-completion.md"
source_rfc_status: "Accepted 2026-09-24 — M12 (closes it)"
target_milestone: "M12"
created: "2026-09-24"
---

# Acceptance and QA checklist

Every box is a property. A box whose plan assigns it elsewhere, or that cannot be satisfied as
written, stays unticked with the contradiction named — the reviewer's error to fix, not the
implementer's to paper over. *(Ablation boxes say "fails its own test, and names anything else that
shares its fixture" — "alone" has been falsified three times by suites sharing fixtures properly.)*

## PR-054-A — keybindings and the repair

- [ ] **No project-local configuration is read** (§1). Pinned by a test with a `config.toml` beside
      the project root, and an **ablation** that makes the loader look there — the test fails.
- [ ] Every advertised chord **round-trips**: the spelling the Help modal and `--help` print is the
      spelling the file accepts.
- [ ] A rebind colliding with another rule **refuses**; a `Reserved` chord **refuses**; the default
      stands and the board says why. **Ablation:** accept last-wins; the collision test fails.
- [ ] **D3′'s repair:** every action ends with a real default binding **or** an explicitly dead
      status, distinguishable **by the type**. `CycleVisibleTerminalSession` and
      `OpenSafeCloseDialog` are decided, each with its reachability stated.
- [ ] An action cannot be both bound and dead — held by the type, not by review.

## PR-054-B — theme and fonts

- [ ] A pair below **4.5:1** falls back, and the diagnostic carries the **measured ratio**.
      **Ablation:** drop the check; that test fails.
- [ ] An unavailable font family falls back and says so; a size outside **8–32 px** falls back at
      each edge.
- [ ] **A font is a name, never a path** — no configured string reaches a file open.
- [ ] No diagnostic echoes a configured string unescaped (`quote_untrusted`).
- [ ] The colour-alone and i18n completeness scans still pass.

## PR-054-C — scrollback and the live proof

- [ ] The scrollback cap is **measured**, not chosen: bytes per line at a realistic width, cap set so
      **one pane at the cap stays under 64 MB**, number and measurement in the book. Above it clamps
      with a diagnostic.
- [ ] **`Ctrl+Alt+C` applies all five live**, and `Reserved` stays reserved across a reload.
- [ ] **Live capture**: default dress → edit the file → `Ctrl+Alt+C` → the same window in the user's
      colours, font and size, with a rebound chord working. A second capture: a bad value falling
      back, named on the board. Throwaway `mktemp -d` config; **no floating or resizing** — this
      desktop is shared.

## Whole-RFC

- [ ] `REQ-CONFIG-006`, `REQ-CONFIG-007`, `NFR-UX-004`, `REQ-TERM-004` move to implemented **with
      evidence they are reachable**, not merely parsed — the distinction the 2026-09-23 audit caught
      the plan getting wrong.
- [ ] `cargo fmt`, `clippy --workspace --all-targets -D warnings`, `git diff --cached --check` after
      staging, `rfc_docs_invariants`, **three consecutive full-workspace runs with `--no-fail-fast`**
      to files, **0 fixture entries left** in a fresh `TMPDIR`.
- [ ] Every new intermittent failure has a dated row in `test-process-leak.md`.
- [ ] Commits are pushed once the gate is green.

## Final Acceptance Decision

- [ ] Accepted.
- [ ] Accepted with required follow-up.
- [ ] Requires re-review after changes.

Reviewer notes:

```text
Pending review.
```
