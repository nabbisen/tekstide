---
title: "RFC-030 — task breakdown and PR plan"
rfc: "RFC-030"
rfc_file: "../../done/030-git-integration.md"
source_rfc_status: "Implemented and closed 2026-09-23 — M12"
target_milestone: "M12"
created: "2026-09-22"
---

# Task breakdown and PR plan

**Rewritten 2026-09-22, at review 405, once D1′ was decided.** The original plan had slice A choose
between a library and a subprocess. The measurement said **both execute a repository-named clean
filter**, and that **a repository naming nothing causes no execution at all** — so the choice moved
from *which mechanism* to *which repositories we read*. D1′ in
[the RFC](../../done/030-git-integration.md) is the decision; this plan implements it.

## PR-030-A — the fixture, and the gate

**No user-visible change.** Nothing renders Git state at the end of this slice.

### The committed fixture (D7)

One repository builder, under a temporary directory, with its own config pinned inside it and the
environment sanitized, so the result cannot depend on the developer's machine. Each vector is its
own row, because the allowlist has to be proven against each:

| Vector | Shape |
| --- | --- |
| clean filter | `filter.<name>.clean`, `required = true`, named by `.gitattributes` |
| fsmonitor | `core.fsmonitor` — a **fixed** key, the easy case |
| textconv | `diff.<name>.textconv` |
| **include-hidden** | the driver in a second file, pulled in by `include.path` — **measured: `git config --list --local` does not show it while `git status` still runs it** |
| attributes-only | `.gitattributes` naming a filter **no config defines** — measured: executes nothing |
| control | a repository that names nothing — measured: executes nothing |
| **submodule** | a gitlink whose submodule gitdir names a clean filter, with the **parent config entirely allowlisted** and no worktree `.gitattributes` — measured at review 406: `git status` in the parent **runs it** |
| **oversized tree** | more directory entries than the walk's budget — the walk must fail **closed** |
| **symlinked directory** | a symlink out of the project — refused at every level, as RFC-050's loader does |

Every named program **writes a marker file and nothing else**. Nothing destructive, nothing
networked, nothing outside the temporary directory.

### The gate

Before any worktree read, in this order:

1. **Sanitized environment and argv** — external configuration neutralized, a reviewed
   non-project-local `git`, no shell, deterministic argv, no project-local `PATH`, bounded time and
   output, bounded diagnostics. RFC-012's *Git Detector Safety* gate, **reported item by item**.
2. **Read the effective configuration with `git config --list`** — **not `--local`, and not
   `--no-includes`**: the include-hidden row above is exactly that bypass.
3. **Refuse unless every key is on a small allowlist** of keys that cannot name a program. **An
   unknown key is a refusal, not a warning**, and any `include`/`includeIf` key is a refusal.
4. **Refuse the dirty and per-file answer** for a repository whose attributes name a `filter=` or
   `diff=` driver. Nothing would execute, but the comparison would be against an index written
   through a filter we did not run — every Git LFS file would read as modified, and a wrong number is
   worse than "not available".
5. **Refusal is an outcome, not an error**: `unavailable`, with a reason.

**A gate reads one repository's configuration; git may consult another's.** Submodules are the
measured instance (review 406): the parent's configuration can be entirely allowlisted while the
program lives in a second repository's config, in a gitdir no worktree walk sees. Detect the gitlink
(`git ls-files -s`) or resolve the gitdir (`git rev-parse --git-dir`) — both measured to execute
nothing — and do not give the content answer for a repository that contains one.

**Required tests.** Per vector: the marker file **does not exist** after the gate runs, and the
poisoned repository is **refused**; the control repository is **accepted** and still executes nothing.
**In both trust states.**

**Run the falsifying ablation first**: with the gate removed, the poisoned repository's marker
**appears**. A fixture that cannot be made to fail proves nothing, so report that before anything else.

Also: `git` absent, or too old to answer, is `unavailable` — never a panic and never a guess.

## PR-030-B — branch, dirty state, ahead/behind

- Produce `ProjectGitSummary` for **accepted** repositories and **call `set_git_summary`**, which has
  no production caller today: branch name, changed-file count, ahead/behind (REQ-GIT-001, 002).
- **A refused repository may still show its branch** — measured, a branch read executes nothing even
  in a poisoned repository, and reading `.git/HEAD` directly executes nothing by construction — with
  dirty state `unavailable`. **Measure it in this slice; do not assume it from this sentence.**
- **Off the UI thread, debounced** (D4). *Pending* while a read is in flight; never the previous value
  shown as current.
- **No write exists in the API** (D5).
- **Restricted projects are not treated differently**: the gate is what makes this safe, not the trust
  grant (D1′ item 5).
- **Carried from RFC-025 (review 404):** this slice edits the status-bar row when it replaces the Git
  field, so it re-proves that the bar's project fields **reach the rendered row** — measured there:
  computing them and never pushing them fails no test.

**Required tests:** a control repository shows branch and dirty state; a poisoned one shows the branch
and `unavailable`, with the marker still absent; a non-repository shows `unavailable`, not an error; a
read in flight shows *pending*. **Ablation:** render the previous summary while pending; that test
fails alone.

## PR-030-C — per-file status

- Per-file status for accepted repositories only (REQ-GIT-003), same gate, same debounce.

**Required tests:** each status a file can carry, from a real repository; a file outside the
repository carries none; a refused repository offers none. **Evidence:** a live capture against a
`mktemp -d` repository and state root, with the status bar showing a real Git state where it said
"not available".

## After this

RFC-024's before-source becomes possible; the two-sided diff is its own RFC (D6). The graph stays
deferred (REQ-GIT-006).
