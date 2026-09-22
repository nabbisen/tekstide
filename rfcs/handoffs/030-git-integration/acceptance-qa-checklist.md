---
title: "RFC-030 — acceptance and QA checklist"
rfc: "RFC-030"
rfc_file: "../../accepted/030-git-integration.md"
source_rfc_status: "Accepted 2026-09-22 — M12"
target_milestone: "M12"
created: "2026-09-22"
---

# Acceptance and QA checklist

Every box is a property. A box whose plan assigns it elsewhere, or that cannot be satisfied as
written, stays unticked with the contradiction named — the reviewer's error to fix, not the
implementer's to paper over.

## PR-030-A — evidence and the decision

- [ ] **The fixture is hostile, proven by ablation**: with the protection removed, the marker file
      **appears**. Run this first; a fixture that cannot fail is not a fixture.
- [ ] After a status read, the marker file **does not exist** — per mechanism measured, **in both
      trust states**.
- [ ] The fixture points global and system Git config inside itself and sanitizes the environment
      (D7); the result does not depend on the developer's machine.
- [ ] The named program is harmless: it writes a marker and nothing else.
- [ ] **D1 decided by the RFC's rule**, recorded as D1′ **in the RFC, in this commit**, with the
      measurement behind it. If neither mechanism is safe, the request says the RFC stops here.
- [ ] If the subprocess is chosen: RFC-012's *Git Detector Safety* gate reported **item by item**,
      each with its own evidence.
- [ ] If a library is chosen: a dated row in `dependency-advisories.md` in the **same commit** (D8).

## PR-030-B — branch and dirty state

- [ ] `set_git_summary` has a production caller; a real repository shows branch and dirty state
      (REQ-GIT-001, 002).
- [ ] *Unavailable* for a non-repository, and when D1′'s guarantee does not hold; *pending* while a
      read is in flight. **Ablation:** show the previous summary while pending; that test fails alone.
- [ ] **No write operation exists in the Git path — held by the API, not by grep** (REQ-GIT-007).
- [ ] Restricted Mode follows D3, and the reason is §1's guarantee, not the trust state.
- [ ] Input is never blocked by a refresh (NFR-PERF-006); the marker is still absent after the
      production path runs.
- [ ] **Carried from RFC-025 (review 404):** the status bar's project fields **reach the rendered
      row**, not merely the producer. Measured there: computing them and never pushing them onto the
      `row!` fails no test. This slice edits that row when it replaces the Git field, so it re-proves
      the wiring — by a live capture showing all of REQ-NOTIFY-002's fields together, at minimum.

## PR-030-C — per-file status

- [ ] Per-file status from a real repository (REQ-GIT-003); a file outside it carries none.
- [ ] Live capture against `mktemp -d`, showing a real Git state where the status bar said "not
      available". Throwaway state only.

## Whole-RFC

- [ ] `cargo fmt`, `clippy --workspace --all-targets -D warnings`, `git diff --cached --check` after
      staging, `rfc_docs_invariants`, and **three consecutive full-workspace runs with
      `--no-fail-fast`**, output redirected to files.
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
