# RFC-035: Change Detection Coverage and Disclosure

Status: **Proposed — awaiting the human owner's acceptance.** Authored 2026-08-18.
Target milestone: M12
Date: 2026-08-18

Related baseline documents:

- `tekstide-security-threat-model-v0.md`
- `tekstide-requirements-v0.md`

Depends on:

- [RFC-012](../done/012-generated-change-review-foundations.md) — the detection model.
- `handoffs/change-detection-wiring.md` — the slice that made detection real (`0.11.0`) and
  disclosed every limitation this RFC exists to address.

## Summary

Close the gaps change detection shipped with, or decide deliberately to keep them.

## Why this is scheduled

`0.11.0` wired change detection and shipped four disclosed limitations. Disclosure was the
right call for that slice and is not a resting state for all four, because **one of them is
security-relevant and the product's premise is supervision.**

## The four, and they are not equal

### 1. `.git/` is excluded, so an agent's git hooks are invisible — the serious one

Detection skips `.git/`, `target/` and `node_modules/`. For build output and package caches
that is obviously right and this RFC does not revisit it.

`.git/` is different. **An agent that writes `.git/hooks/pre-commit` has installed code that
runs on the user's machine, and change review will never show it.** The explorer collapses
`.git/` too, so there is no second route by which a user would notice. This is the one item
here that is a supervision hole rather than a coverage limit.

Options: watch a narrow, high-consequence subset (`hooks/`, `config`) while continuing to
ignore the churn (refs, objects, index); watch none and say so much louder than a
`future-work` line; or make it configurable, which defers rather than decides.
**Recommend the narrow subset.** The churn argument is what justifies excluding `.git/`, and
`hooks/` and `config` do not churn.

### 2. `max_changed_paths` discards a list it already computed

When a scan exceeds `max_changed_paths`, `detect_filesystem_changes` returns an **empty**
`changed_paths` with a `Partial` status. For `max_entries` that behaviour is correct — a
truncated scan genuinely cannot distinguish "unchanged" from "not looked at."

**`max_changed_paths` is different**: the scan completed, the paths are known, and the code
throws them away. "We found 4,097 changes so we will show you none" is worse than showing the
first N and saying how many were omitted — which `ChangeSetSummary` already models with
`shown_changed_files` and `omitted_changed_file_count`, and which nothing populates.

### 3. Detection runs only at exit

A long-lived interactive Claude Code session — the product's headline use — reports nothing
until the user quits it. A mid-run trigger is real feature work with its own questions
(polling cost, what "since when" means, what a partial run's change set represents), which is
why the wiring slice correctly did not build one.

### 4. The baseline does not survive the application

`agent_run_change_baselines` is in-memory. If Tekstide closes while a run is live, that run
produces no change set — **indistinguishable, from outside, from an agent that changed
nothing.** That indistinguishability is the defect, more than the loss itself.

## Scope

Items 1 and 2. **Items 3 and 4 are recorded here and explicitly deferred**, because each is
its own design with its own cost, and bundling four unrelated fixes is how a slice gets
reviewed badly.

## Non-goals

- `.gitignore` parsing. Named as out of scope by the wiring handoff and belongs with RFC-030.
- Changing the exclusion list for `target/` or `node_modules/`.

## Decisions required

**D1 — the `.git/` subset.** Which paths, and is the list fixed or configurable? Recommend
fixed and short; configurability is RFC-023's and a security-relevant default should not
arrive as a setting first.

**D2 — what a partial-by-`max_changed_paths` change set renders.** Populating
`shown_changed_files`/`omitted_changed_file_count` is the model's answer; the surface must not
present a truncated list as complete. Same honesty rule as the scan-truncation case, which
`0.11.0` already got right.

## Risks

- **Watching `.git/hooks/` makes hook churn noisy** for users whose tooling writes hooks
  routinely. Mitigated by the subset being narrow, and by measuring against a real repository
  before shipping — the same discipline the wiring slice used for its scan cost.
- **A "showing 100 of 4,097" list implies the 100 are the important ones.** They are whatever
  order the scan produced. Say so.
