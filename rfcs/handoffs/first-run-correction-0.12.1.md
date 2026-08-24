---
title: "0.12.1: the first-run correction — evidence and corrections"
owning_rfcs: "none — a correction release; RFC-038 owns the actual fix"
status: "Released and published 2026-08-22; audited 2026-08-22 (request 288, response 289)"
created: "2026-08-22"
---

# What `0.12.1` was, and the two corrections it needs

This file exists because `0.12.1` shipped with **no handoff pack, no acceptance checklist and
no evidence file** — the architect implemented it directly, so none of the artifacts that
normally carry a slice's evidence were produced. Traceability between the release, its
implementation and its tests had no home. This is that home, written after the fact.

## The release

`0.12.0` shipped a window that told a new user nothing. Started with no argument — which the
published Quick Start told people to do — it showed an empty Project Board rendering
`Add Project` and `Open from path` as `text()` widgets: two actions with no button, no handler
and no in-app route behind either, inert since the surface landed on **2026-07-31** and shipped
in **twelve releases** (`0.4.0`–`0.12.0`). Nine keybindings were live and the string `Ctrl`
appeared zero times in the user-facing catalogue. `tekstide --help` printed
`folder does not exist: --help`.

`0.12.1` (`ca456c7`, published to crates.io 2026-08-22) corrected all of that: a truthful empty
state, the derived keyboard list on both board states, working `-h`/`--help`/`-V`/`--version`,
and a Quick Start that leads with the path. It did **not** add an in-app way to open a project.
RFC-038 owns that.

## Correction 1: the release claims five ablations; four were valid

`ca456c7`'s commit message and the `0.12.1` tag both state "Five ablations, each failing the
intended test." **Four were independent and valid. The fifth proved nothing.**

The ablation described as *"describing the palette"* changed two things at once: it gave
`OpenCommandPalette` a catalog key in `action_catalog_key`, **and** it widened
`advertised_bindings()`'s filter from `(Candidate, Some(binding))` to `(_, Some(binding))`.
Three tests failed, and the failures were recorded as evidence that describing a reserved
action is caught. They were produced by the filter change — which is ablation 1, already run
and already counted. The fifth ablation was a contaminated duplicate of the first.

The tell was written down at the time, in a comment on the ablation itself: *"also break the
core filter so the reserved rule reaches the GUI."* The correct reading of that sentence is
that the property is unreachable without a second change, and therefore untested. It was
instead treated as setup.

**Neither the commit message nor the tag is rewritten** — both are pushed, and this project does
not edit closed evidence to match a later state. The correction lives here. `CHANGELOG.md` does
not carry the claim, so nothing user-facing overstates it.

**The re-check this correction names:** anyone relying on `0.12.1`'s ablation record should
treat the `action_catalog_key` contract for non-`Candidate` actions as **unverified** until the
test below lands.

## Correction 2: a real property has no test

Found by the dev team's audit (request 288), reproduced here because it outlives the audit:

`action_catalog_key`'s return value for a non-`Candidate` action is unreachable from any test.
`keyboard_help_lines` calls `advertised_bindings()` first, which filters on `rule.status`, so
the `(action, binding)` pair for a `Reserved` or unbound action never reaches
`action_catalog_key` at all. Giving `OpenCommandPalette` a description today is dead code — and
the moment that action is promoted to `Candidate` with a real binding, its catalog key starts
mattering, with nothing having ever checked it.

Same shape as report 287's finding 4: a real property with no test that can fail on it.

**Required follow-up, assigned to the dev team (response 289):** a direct unit test of
`action_catalog_key`'s own contract, not routed through `keyboard_help_lines`, asserting the
biconditional —

> `action_catalog_key(a).is_some()` **iff** `a` is live in `linux_mvp()` (`Candidate` with
> `Some(binding)`).

The expected set derived from the policy rather than written as a literal list, and ablated by
changing exactly one thing.

**DISCHARGED 2026-08-22** — `7e8d8c2`, request 290, response 291.
`action_catalog_key_is_some_iff_the_action_is_live` iterates `linux_mvp().rules` and asserts
`key.is_some() == (status == Candidate && default_binding.is_some())` for every rule, with the
expected side taken from the policy so a status change cannot make it stale.

Ablated independently by the reviewer, one variable per run, `navigation.rs` untouched in both:

| Ablation | Result |
| --- | --- |
| `OpenCommandPalette` (Reserved) gains a key | **1 test fails** — the new one, uniquely. This is the gap that existed |
| `OpenTrustSettings` (live) loses its key | 3 tests fail, the new one among them |

The first is the one that matters: before `7e8d8c2` nothing in the suite failed on it, which is
what made the original ablation's two-variable result look like evidence. The second direction
was already covered by two composed-behaviour tests; the new test states it as the function's
own contract instead, so it survives a change of caller.

**The correction above is therefore closed.** `ca456c7`'s "five ablations" claim remains
inaccurate as written and is not rewritten; the property it failed to establish is now
established.

## Audit evidence (request 288)

Performed by the dev team on `ca456c7` as committed, after report 287 disclosed that the
architect had implemented the release himself.

- Full gates re-run: 318 + 714 tests, 0 failed; `fmt` clean; `clippy -D warnings` clean.
- Structural claims read against the code: single `linux_mvp()` constructor shared by the input
  dispatcher and the help builder; `action_catalog_key` exhaustive over all 14
  `NavigationAction` variants; `view()`'s branches mutually exclusive (a suspected double-render
  was checked against the full file and withdrawn); `status_bar` still one line, so
  `content_area_height`'s subtraction still holds.
- Live binary: `--help`, `-h`, `-V`, `--version` verified against the compiled binary.
- **Cold start**: `tekstide` with no arguments and a fresh `XDG_STATE_HOME`, screenshotted live.
  All nine bindings listed with their preconditions; no lying action labels. **This is the first
  cold-start evidence in this repository's history**, and it exists because report 287 asked for
  the rule and the dev team applied it to the next thing they touched.

## Why there was no pack

The architect implemented this release directly, which the operating instructions prohibit
absent explicit emergency authorization from the human owner, and which was reported to the
owner and to the dev team (report 287). The practical consequence was not only the missing
review — that was supplied afterwards by request 288 — but the missing artifacts: no task
breakdown, no acceptance checklist, no evidence file, and, until this document, no traceable
link from the release back to what was verified.

Recorded here rather than in a post-mortem because the artifact gap is the part most likely to
recur quietly: a review can be requested after the fact, but nobody notices absent paperwork
until someone goes looking for evidence that was never written.
