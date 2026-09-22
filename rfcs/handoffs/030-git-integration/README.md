---
title: "RFC-030: Git Integration — implementation handoff"
rfc: "RFC-030"
rfc_file: "../../accepted/030-git-integration.md"
source_rfc_status: "Accepted 2026-09-22 — M12"
target_milestone: "M12"
created: "2026-09-22"
---

# The evidence first, the feature second

Source RFC: [RFC-030](../../accepted/030-git-integration.md)

## What this is

The board says a project's branch is **"not available"**, because nothing produces
`ProjectGitSummary` — `set_git_summary` has no production caller. Filling it means reading a Git
repository, and **a repository can name programs that Git runs during an ordinary status read**. That
is the whole difficulty; the rest is rendering.

**So slice A ships no feature.** It builds a repository that tries to run something, measures both
mechanisms against it, and decides D1 from what it measured. **If neither is safe, the RFC stops and
the field keeps saying "not available"** — that is a permitted outcome, not a failure to work around.

## Read these first

1. [`what-git-integration-must-not-do.md`](./what-git-integration-must-not-do.md) — **required
   before writing code.**
2. RFC-012's **"Git Detector Safety"** section in [`../../done/012-generated-change-review-foundations.md`](../../done/012-generated-change-review-foundations.md).
   It is the gate, and it already says a safe library implementation may satisfy it.
3. The RFC's **"Decided on acceptance"** — especially **D1**'s rule and **D7**'s fixture.

## The plan

[`task-breakdown-pr-plan.md`](./task-breakdown-pr-plan.md): A evidence and the decision, B branch and
dirty state, C per-file status. Checklist:
[`acceptance-qa-checklist.md`](./acceptance-qa-checklist.md).

## Not in this slice

- **Any write.** No commit, stage, push, rebase or checkout, in any trust state (REQ-GIT-007).
- **The commit graph** (REQ-GIT-006, deferred) and **the two-sided diff** (its own RFC).
- **Credentials of any kind**, and non-Git version control.
