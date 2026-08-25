---
title: "What watching .git/ must not become"
rfc: "RFC-035"
rfc_file: "../../accepted/035-change-detection-coverage-and-disclosure.md"
source_rfc_status: "Accepted 2026-08-18 — scheduled 2026-08-25"
status: "Required reading before any detector change"
created: "2026-08-25"
---

# What watching `.git/` must not become

Narrowing an exclusion is a security change in both directions: too narrow and the hole stays
open; too wide and detection drowns in churn and becomes useless, which closes the same hole a
different way.

## 1. The decision: `hooks/` and `config`, nothing else

RFC-035 recommends the narrow subset and its reasoning is the deciding one — **the churn argument
is what justifies excluding `.git/` at all, and `hooks/` and `config` do not churn.** `refs/`,
`objects/` and `index` change on every ordinary git operation and carry no code.

Decided: watch `.git/hooks/` and `.git/config`. Not configurable — RFC-035 names that option and
correctly calls it deferring rather than deciding.

## 2. The two watches are complementary, and neither alone is sufficient

This is not in RFC-035 and it decides the shape:

**`core.hooksPath` relocates the hooks directory.** A `.git/config` carrying
`[core] hooksPath = .githooks` means hooks no longer live in `.git/hooks/` at all — so watching
`.git/hooks/` alone can be stepped around by one line of config. `.git/config` is also its own
execution surface independently: `[alias]` entries beginning `!` run shell commands, and
`[filter]` `clean`/`smudge` commands run on checkout and commit.

So **`config` is not a lesser sibling of `hooks/` — it is the one that can redirect or replace
it.** Watch both, and if you find yourself dropping one for scope, drop neither: report instead.

**Do not chase `hooksPath` to its target.** Following it means reading config, resolving a path
that may be anywhere, and watching a second location that changes as config changes — real scope
with its own failure modes. Watching `.git/config` tells the user *the hook location changed*,
which is the fact that matters. Record following it as deferred, with this reason.

## 3. A watched path must not be silently reclassified

`.git/hooks/pre-commit` is a changed path like any other. It goes through the same
`DetectedChangedPath` shape, the same `ChangeSet`, the same surface. **Do not add a "security"
severity, a special icon, or a separate list** — this project has no such concept, and inventing
one here would be a policy decision smuggled in as a rendering detail.

If a reviewer cannot tell from the rendered path that `.git/hooks/pre-commit` matters more than
`src/main.rs`, that is a real finding — but it is RFC-020's surface's problem to solve
deliberately, not this slice's to pre-empt.

## 4. The exclusion is load-bearing elsewhere — check before changing it

The `.git/` exclusion is not only detection's. The **explorer collapses `.git/` too**, which
RFC-035 names as why there is no second route by which a user would notice a hook.

**Before changing any shared constant, enumerate who reads it.** If detection and the explorer
share one exclusion list, narrowing it for detection silently un-collapses `.git/` in the
explorer, and a user gets a tree full of `objects/` — the churn problem, arriving somewhere
nobody was looking. Two lists with one honest name each beats one list with two meanings.

This is the same shape as `scan_active_project_explorer_directory` doing two jobs, which cost
PR-038-F a slice to unpick.

## 5. `max_changed_paths` — show what was found, count what was not

The scan **completed**. The paths are known. Discarding them is the defect.

`ChangeSetSummary` already models the honest answer — `shown_changed_files`,
`omitted_changed_file_count` — and nothing populates it from the detector side. Populate it.

**`Partial { limit }` still applies** and must keep meaning what it means. A truncated *scan*
(`max_entries`) and a truncated *list* (`max_changed_paths`) are different facts, and RFC-020's
surface already renders them as two lines when both are true. Do not collapse them now that both
can be non-trivial at once — that distinction has been defended three times in this codebase and
this is the first slice that can actually make both true simultaneously.
