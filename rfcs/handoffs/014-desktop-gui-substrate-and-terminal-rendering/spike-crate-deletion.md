---
title: "RFC-014 — Spike crate deletion: implementation handoff"
rfc: "RFC-014"
rfc_file: "../../done/014-desktop-gui-substrate-and-terminal-rendering.md"
status: "Ready for implementation — all four deletion conditions satisfied 2026-08-03 by RFC-017 PR-017-E"
created: "2026-08-04"
---

# Deleting `tekstide-gui-spike`, and deciding `tekstide-pty-spike`

## Why this document exists at all

RFC-014 §"When the spike crate is deleted" already records the four conditions, the forcing event, and one consequence to check. **It did not fire.** PR-017-E landed on 2026-08-03, satisfying the last condition, and the deletion did not happen — because the obligation lived in RFC-014 and in a `filter.rs` module comment, and neither is what PR-017-E's implementer was reading.

RFC-014 is now closed and in `rfcs/done/`, which makes it *less* likely to be read, not more. So this is the instruction, and RFC-014 is the reasoning behind it.

Four judgement calls below are **not** in RFC-014. They are the reason this is a document rather than a one-line request.

## `tekstide-gui-spike` — delete it

All four of RFC-014's conditions hold:

1. **No product code compiles against it** — verified: no shipped crate declares the dependency.
2. **Every property it proved has a product-code equivalent with its own tests** — `filter.rs` → PR-017-B, `terminal_pane.rs` → PR-017-C, `font_metrics.rs` → PR-017-E. All three landed and were approved.
3. **Evidence artifacts live outside the crate** — the nine screenshots are under `./evidence/`.
4. **Nothing still needs to read it as reference** — PR-017-B, C and E have all landed.

### The check RFC-014 explicitly asks for

> removing the crate changes the *workspace* dependency tree. `sys-locale` was once reported as costing `+0` because the spike pulled it transitively through `iced` → `cosmic-text` — an error corrected in review 122 by measuring `cargo tree -p tekstide` instead of diffing the workspace lock. Any figure still measured workspace-wide would shift on deletion. The correct measurements are already per-crate, so this should be a no-op; **confirm rather than assume.**

Confirm it with `cargo tree -p tekstide` and `cargo tree -p tekstide-core` before and after. If either shifts, that is a finding, not a formality.

### Decision 1 — four source references, and two of them are provenance

Deleting the crate strands four doc comments in shipped code:

| Site | What it is |
| --- | --- |
| `crates/tekstide/src/surface/terminal/filter.rs:1` | "promoted from `crates/tekstide-gui-spike/src/filter.rs`" |
| `crates/tekstide/src/surface/terminal/filter/tests.rs:9` | "Ported from `crates/tekstide-gui-spike/src/filter/tests.rs`" |
| `crates/tekstide/src/shell.rs:55` | spike named as the editor-shape precedent |
| `crates/tekstide/src/shell.rs:534` | spike's `while true` flood named as superseded precedent |

The first two are **provenance on security-critical code** — they record where the RFC-009 filter came from and when it was reviewed. Do not simply delete them to clear a dangling path; that loses the audit trail on the one module in this crate where origin matters most.

**My call:** keep all four, reworded so they read as history rather than as a live path — name the RFC and PR that promoted the code, and point at `rfcs/handoffs/014-.../` for the surviving evidence, rather than at a directory that no longer exists. Push back if you disagree once you have them in front of you.

### Decision 2 — do not rewrite the evidence files

Ten-plus documents mention the crate, including `qa-evidence.md` files for RFC-013, RFC-014, RFC-015, RFC-017 and RFC-021. **Leave every one of them alone.** Evidence files record what was true when the work was done; editing them to match a later state destroys their value as a record.

The one exception is RFC-014's own §"When the spike crate is deleted" — that section is a live instruction, not evidence. Append the deletion date and the commit, so a future reader sees it was discharged rather than skipped.

### Decision 3 — the gate line changes shape, and that is expected

Every gate line across this cycle reads `497 tekstide-core + 120 tekstide + 18 tekstide-gui-spike + 0 tekstide-pty-spike`. After deletion the 18 disappear.

**That is not a regression and should be stated as such in the commit message**, because the next person to compare gate output against RFC-017's evidence will otherwise be looking for 18 missing tests. Note it, do not quietly let the number change.

Also drop the workspace member entry in the root `Cargo.toml`.

## `tekstide-pty-spike` — a decision, not a task

RFC-007's PTY feasibility harness. Unlike the GUI spike it has **no recorded deletion condition at all**, which is exactly why it has outlived its purpose unnoticed. Nothing in `crates/tekstide/src` or `crates/tekstide-core/src` references it.

Do **one** of these, and say which and why:

- **Delete it**, if every property it proved has a product-code equivalent with its own tests and its evidence lives under `rfcs/handoffs/007-runtime-substrate-pty-feasibility/`. That is RFC-014's condition set applied to RFC-007, and it is the right test.
- **Write it a deletion condition** in RFC-007's handoff, naming what still needs reading and what event supersedes it — if something genuinely does.

What is **not** acceptable is leaving it as-is with no condition. That is the state that produced this document.

## Review gate

- `cargo tree -p tekstide` and `-p tekstide-core` compared before and after; no shift, or the shift reported.
- The four source references resolved deliberately, with the provenance on `filter.rs` preserved in some form.
- Evidence files untouched; RFC-014's deletion section annotated with the date.
- Gate-count change called out explicitly in the commit message.
- `tekstide-pty-spike` decided either way, with the reasoning stated.
