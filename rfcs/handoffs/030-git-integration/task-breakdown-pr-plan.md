---
title: "RFC-030 — task breakdown and PR plan"
rfc: "RFC-030"
rfc_file: "../../accepted/030-git-integration.md"
source_rfc_status: "Accepted 2026-09-22 — M12"
target_milestone: "M12"
created: "2026-09-22"
---

# Task breakdown and PR plan

**A decides whether B and C happen at all.**

## PR-030-A — the adversarial fixture, the measurement, the decision

**No user-visible change. Nothing renders Git state at the end of this slice.**

- **Build the fixture** (D7): a repository under a temporary directory configured to run a program
  during an ordinary status read — by as many of the mechanisms as apply (filter, hook, config-named
  helper, alias, and any the library documents). The program **writes a marker file**. Global and
  system config point inside the fixture; the environment is sanitized.
- **Measure both mechanisms** against it: the candidate read-only library, and `git` invoked as
  RFC-012's gate requires. Compute branch, dirty state and per-file status with each.
- **Decide D1 by the rule in the RFC**: the library only if it executes nothing repository-configured;
  otherwise the subprocess with the gate met **item by item, each with its own evidence**;
  **otherwise stop**, and say so in the request.
- **Record the decision as D1′ in the RFC**, with the measurement, in the same commit.
- **If the library is chosen: a dated row in `dependency-advisories.md`** in that same commit (D8).

**Required tests:** the marker file does not exist after a status read, **in both trust states**, per
mechanism measured. **Ablation:** neutralise whatever suppresses execution (or, for the library,
assert against a deliberately unsafe invocation) and the marker appears — this is the ablation that
proves the fixture is actually hostile. **A fixture that cannot be made to fail proves nothing**, so
run that ablation first and report it.

**Report the gate item by item** if the subprocess is chosen: reviewed non-project-local executable,
no shell, deterministic argv, no project-local `PATH`, sanitized environment, no workspace hooks or
config-driven automation, bounded time and output, bounded diagnostics.

## PR-030-B — branch and dirty state, behind D1′

- Produce `ProjectGitSummary` through the chosen mechanism and **call `set_git_summary`**, which has
  no production caller today. Branch name and changed-file count (REQ-GIT-002).
- **Off the UI thread, debounced** (D4). *Pending* while a read is in flight; *unavailable* when the
  project is not a repository, or when D1′'s guarantee does not hold.
- **No write exists in the API** (D5, §4).
- **Restricted projects** follow D3: detection runs only under §1's guarantee, otherwise
  `unavailable`.

**Required tests:** a real repository shows its branch and dirty state; a non-repository directory
shows *unavailable*, not an error; a read in flight shows *pending*, never the previous value; the
fixture's marker is still absent after the production path runs. **Ablation:** render the previous
summary while a read is in flight; the pending test fails alone.

## PR-030-C — per-file status

- Per-file status for the files the review surfaces already list (REQ-GIT-003), through the same
  mechanism, under the same debounce.

**Required tests:** each status a file can carry, from a real repository; a file outside the
repository carries none. **Evidence:** a live capture against a `mktemp -d` repository and state
root, with RFC-025's status bar showing a real Git state where it said "not available".

## After this

RFC-024's before-source becomes possible; the two-sided diff is its own RFC (D6). The graph stays
deferred (REQ-GIT-006).
