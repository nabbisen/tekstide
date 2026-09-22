# RFC-030: Git Integration

Status: **Accepted by the human owner 2026-09-22.** **D2–D8 decided by the architect on acceptance; D1 stays open by design and slice A decides it.** Proposed 2026-09-17. Reserved for M12 (renumbered from 024 on 2026-08-12). Scoped at the
owner's word **alongside** RFC-025, shipping when its safety evidence is done rather than on
`0.21.0`'s date.
Target milestone: **M12**
Date: 2026-09-17

Related RFCs:

- [RFC-012](../done/012-generated-change-review-foundations.md) — wrote the **Git Detector Safety**
  gate this RFC must meet, and allowed a library implementation to meet it.
- [RFC-024](../done/024-diff-preview-policy.md) — a two-sided diff is blocked on a before-source, and
  *"the only one this project has designed is Git-backed detection"*. Not delivered here.
- [RFC-004](../done/004-security-baseline-and-restricted-mode.md) — Restricted Mode, which
  REQ-GIT-005 binds.
- RFC-025 (proposed alongside) — renders Git state once this RFC produces it.

## Summary

A repository is content the user did not necessarily write, and **Git reads instructions out of it**.
The requirements permit calling the system `git`; RFC-012 wrote down what that would take to be safe.
This RFC decides how to meet that gate **before** any Git state is shown, and then shows it.

## What is true today, measured

- **The requirements** (`tekstide-requirements-v0.md` §6.x): REQ-GIT-001 detect a repository; **002**
  branch and dirty/clean; **003** per-file status in the explorer; **004** *may* call the system
  `git`; **005** automatic Git commands are **read-only in Restricted Mode** unless explicitly
  approved; **006** a graph may be deferred; **007** never auto-commit, push or rebase.
  NFR-PERF-006: refresh is debounced and never blocks input for more than a frame.
- **The domain type exists and nothing produces it.** `ProjectGitSummary` carries a provider state,
  branch, changed-file count, and ahead/behind counts. `set_git_summary` has **no production caller**,
  so the board reads *"branch: not available"* — truthfully.
- **RFC-012's gate, verbatim in substance:** a reviewed non-project-local executable; no shell; a
  deterministic argument vector, not aliases; no project-local `PATH`; a sanitised environment; **no
  workspace hooks or config-driven automation**; bounded time and output; diagnostics with no file
  contents. *"If these guarantees cannot be made, Git detection must report `unavailable`."* **A safe
  library implementation may satisfy it**, but review must still verify it reads metadata only.

## The problem, stated plainly

**Reading Git status is not obviously read-only.** A repository's own configuration can name programs
Git will run during an ordinary status read — a filesystem monitor is the well-known case — and the set
of such keys is not closed. Overriding the ones we know about with command-line settings leaves the
ones we do not. That is why this RFC decides the mechanism first.

## Decisions required

**D1 — Mechanism: a read-only library, or a hardened subprocess.** **Open, and the first slice
measures it before deciding.** A pure-Rust Git library could read `HEAD`, the index and the working
tree without executing anything a repository names — no subprocess boundary to harden at all — at the
cost of a large new dependency and its advisory load (`dependency-advisories.md`). A hardened
subprocess keeps the dependency surface small, but must neutralise repository-configured execution,
which is open-ended. **Recommended direction: prefer the library if slice A shows it executes nothing
repository-configured while computing status; otherwise the subprocess, with the gate met item by
item.** Either way RFC-012's review test applies: metadata only, no workspace automation.

**D2 — The safety evidence is its own slice, and nothing is shown before it.** Recommended. Slice A is
evidence and a decision, with an adversarial fixture: a repository configured to run a program during
a status read, and a test that the program never runs.

**D3 — Restricted Mode.** Recommended: status detection runs in Restricted projects **only because D1
guarantees it executes nothing the repository names** — that is what makes it read-only in REQ-GIT-005's
sense. If D1 cannot guarantee it, detection reports `unavailable` in Restricted projects.

**D4 — Off the UI thread, debounced (NFR-PERF-006).** Recommended; the provider state already models
*pending* and *unavailable*.

**D5 — Read-only, always.** No commit, stage, push, rebase, checkout or any write, in any trust state
(REQ-GIT-007). The graph is deferred (REQ-GIT-006).

**D6 — The two-sided diff is out.** RFC-024's before-source becomes possible once Git state is read
safely, and it is its own RFC.

## Non-goals

- Any Git write operation; the graph; the two-sided diff; credential handling of any kind.
- Recognising non-Git version control.

## Risks

- **The library executes something after all** — through a filter, a hook, or a config-named helper.
  Slice A's adversarial fixture exists to find this before anything ships.
- **A new large dependency.** Its advisories become ours; the register must say so.
- **Status on a large repository is slow.** D4 keeps it off the input path, and the provider state
  says *pending* rather than showing stale data as current.

## Acceptance criteria

- A repository configured to run a program during a status read **never runs it**, asserted against a
  real fixture, in both trust states.
- Branch, dirty/clean and per-file status shown for a real repository; *"not available"* when the
  project is not one, or when D1's guarantee does not hold.
- No write operation exists anywhere in the Git path — held by the API, not by grep.
- Input is never blocked by a refresh.


## Decided on acceptance (2026-09-22)

**D1 stays open, and that is the decision.** Choosing the mechanism now would be choosing it from the
shape of the two options rather than from what either one does against a hostile repository — the
exact error this project has made before. **Slice A decides it, and slice A is evidence.** The rule it
decides by, fixed now so the measurement cannot be argued backwards into a preference:

- **The library wins if, and only if, it executes nothing the repository names** while computing
  branch, dirty state and per-file status — no filter, no hook, no config-named helper.
- **Otherwise the hardened subprocess**, with RFC-012's *Git Detector Safety* gate met **item by
  item, each with its own evidence**, not as a summary claim.
- **If neither can be shown safe, the RFC stops there.** Git state keeps reading *"not available"* —
  which is what it says today, and what RFC-025's status bar will say. **Shipping nothing is an
  acceptable outcome of slice A**, and a better one than shipping a boundary we could not prove.

The decision is recorded in this RFC as **D1′** at slice A's review, with the measurement behind it.

**D2–D6 as recommended.**

**D7 — the adversarial fixture is built by the test, and is itself harmless.** The repository is
constructed under a temporary directory; the program it names **writes a marker file and nothing
else**, so "it ran" is observable without anything happening. The fixture **must not depend on the
developer's own Git configuration**: global and system config are pointed inside the fixture and the
environment sanitized, or a green run on one machine means nothing on another. It runs in **both
trust states**.

**D8 — a new dependency arrives with its advisory row.** If slice A chooses the library, the **same
commit** adds a dated row to `rfcs/handoffs/dependency-advisories.md` naming it, its version and its
own dependency load. Its advisories become ours; the register is where we find that out on time.

**Ship boundary.** RFC-025 renders the Git field as *"not available"*; this RFC fills it. It adds no
new surface of its own, and it ships when its evidence is done — `0.22.0` if that is where it lands,
not on `0.21.0`'s date.
