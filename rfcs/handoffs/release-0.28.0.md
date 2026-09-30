---
title: "Release 0.28.0: the editor tells you where you are, and lets you take it back"
status: "**Scoped 2026-09-30 by the architect**, after RFC-057 closed out at review 448. The candidate is the dev team's; the publish, the tag and the post-publish checks are the architect's."
rfc_file: "../accepted/057-editor-essentials.md — moved to done/ by the closure commit below"
target_milestone: "M10 remainder"
created: "2026-09-30"
---

# Release 0.28.0

## Scope — one RFC, five slices and a ruling

**RFC-057, Editor Essentials.** The editor drew the whole file as one string every frame, had no
line numbers, drew no caret, and could not take back a keystroke. It now draws the rows that fit,
numbers them from the file's own index, shows a caret that is an element rather than a character,
follows the cursor sideways on a line too long for the window, and undoes by operation. The owner's
two-row header came with it, and the owner's font ruling closed the one defect the slice measured in
itself.

`REQ-EDIT-002` is met. `NFR-REL-005` records undo. **`NFR-PERF-003` has a measured number for the
first time.**

## The order of the commits

**1. `Close RFC-057` — its own commit, before the candidate.** `rfcs/accepted/057-…` → `rfcs/done/`
with a *Closed* section; `rfcs/README.md` (accepted table empty, done row added, handoff row closed);
`delivery-plan.md`'s row; the pack's `rfc_file` paths. The lifecycle move belongs **inside** the
release sequence — that is the rule RFC-053 cost us three releases to learn, and
`an_rfc_a_release_names_lives_in_done` now enforces it.

The Closed section should carry what only this RFC can tell the next reader:

- **The baseline was measured before anything changed** (D11), and that is why "it got faster" is a
  claim with a number behind it: p95 14.285 ms → 7.876 ms at 100,000 lines, layout 9.896 → 0.050.
- **Three ablations that found nothing, and were reported as findings** — the worker that could have
  stopped asking git, the C3 ablation inside `view`, and the two modal guards that repeat a gate.
- **An approximation falsified rather than defended**: `W` overflows by 50.91 columns, `i` by zero,
  and the owner's ruling made the shipped default exact (0.0 % off).
- **What is left open**: the per-keystroke whole-document copy (now ~80 % of the remaining cost, and
  deliberately out of this release); the proportional-family approximation, disclosed with its
  number; `RFC-063` and `RFC-064`; and the requirements gap — no `REQ-EDIT` names undo.

**2. The candidate commit.** Version `0.27.0` → `0.28.0`, **and the core pin with it** —
`the_workspace_pins_tekstide_core_to_its_own_version` is red until it moves, which is the point.
`Cargo.lock`, and the changelog promoted under its title.

## The changelog — fold the superseded entries, do not annotate them

Two entries in `Unreleased` describe states that **never shipped** and are marked *"Corrected
above/below"*:

- *"A line wider than the editor is clipped at its right edge… there is still no horizontal
  scrolling"* — written at PR-057-B, untrue of the release.
- *"The editor shows about 48 lines… no line numbers and no visible caret"* — written at PR-057-A,
  untrue of the release.

**Fold each into its corrected successor and keep only what is still true**, rather than shipping the
contradiction with a pointer. The cross-release convention — correct `0.24.0` by name, never erase it
— exists because readers *acted on* `0.24.0`'s text. Nobody acted on a mid-development draft of an
unreleased section, and a limitations list that contains non-limitations costs a reader more than the
history is worth.

What must survive the fold, because it is true of `0.28.0`: **lines still do not wrap**; there is
**still no scrollbar and no mouse wheel**; navigation is still the four arrow keys.

## Suggested title

**"The Editor Knows Where You Are"** — line numbers, a caret, a window that follows the cursor on
both axes, and undo. Take it or better it; the convention is the thing the release makes true.

## The gate — two steps are new this cycle

Everything in `release-checklist.md`, and note the two that changed since `0.27.0`:

- **`cargo package --workspace --locked`**, *not* `cargo package -p tekstide`. The pin makes the
  single-crate step fail for every release, by design.
- **The `.rs` placement check**: every `.rs` in each archive under `src/`, `tests/`, `examples/` or
  `benches/`. Read it as a placement convention, not a deadness check — review 446.

Three consecutive full-workspace runs, `--no-fail-fast`, to files, **0 fixture entries left** in a
fresh short `TMPDIR` — and a **short fixed literal** (`/dev/shm/tk1`), not `mktemp`, or the
`AF_UNIX` 108-byte limit bites again (PR-057-D's own finding).

## What is deliberately not in it

Say these in the changelog's own words, because a reader who finds them should find them named first:
the per-keystroke document copy; soft wrap; a scrollbar or mouse wheel; selection, clipboard and
delete-forward (the vocabulary is four keys); syntax highlighting; and the proportional-family
approximation with its measured ~51 columns.

## Mine, after the candidate

The independent gate, the package diff, `cargo audit`, the publish, the tag, and
`post-publish-check.sh` for **`0.28.0` and `0.27.0`** — the release just published and the one before
it, because only an old app shows what a new core broke.
