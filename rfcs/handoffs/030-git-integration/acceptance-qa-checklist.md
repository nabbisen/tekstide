---
title: "RFC-030 — acceptance and QA checklist"
rfc: "RFC-030"
rfc_file: "../../accepted/030-git-integration.md"
source_rfc_status: "Accepted 2026-09-22 — M12; D1′ decided 2026-09-22"
target_milestone: "M12"
created: "2026-09-22"
---

# Acceptance and QA checklist

**Rewritten 2026-09-22 at review 405, to match D1′.** Every box is a property. A box whose plan
assigns it elsewhere, or that cannot be satisfied as written, stays unticked with the contradiction
named — the reviewer's error to fix, not the implementer's to paper over.

## PR-030-A — the fixture and the gate

- [x] **The fixture is hostile, proven by ablation**: with the gate removed, the poisoned
      repository's marker file **appears**. Run this first; a fixture that cannot fail is not a
      fixture. — `hostile_fixtures_are_provably_hostile`, qa-evidence.md.
- [x] Every vector has its own row — clean filter, `core.fsmonitor`, textconv, **include-hidden**,
      attributes-named-but-undefined, and a control that names nothing. — qa-evidence.md's vector
      table.
- [x] After the gate runs, the marker file **does not exist**, for every vector, **in both trust
      states**. — every `*_is_refused_and_the_marker_never_runs`/`*_on_the_include_key_itself` test;
      trust-state coverage is structural (`evaluate` takes no trust parameter), exercised explicitly
      for one vector in `the_gate_does_not_depend_on_trust_state` — see qa-evidence.md for why
      re-running all four was judged redundant rather than skipped.
- [x] The poisoned repositories are **refused**; the control repository is **accepted** and still
      executes nothing. — `control_repository_is_accepted_and_executes_nothing` plus the four
      vector-specific refusal tests.
- [x] **The configuration read is `git config --list`**, includes expanded — not `--local`, not
      `--no-includes`. A test carries the include-hidden repository specifically, because that is the
      measured bypass. — `include_hidden_repository_is_refused_on_the_include_key_itself`.
- [x] **An unknown configuration key is a refusal**, and so is any `include`/`includeIf`.
      **Ablation:** turn the allowlist into a denylist of the known-bad keys; the include-hidden or
      unknown-key test fails. — manual ablation recorded in qa-evidence.md (reverted, checksum-verified
      clean).
- [x] A repository whose attributes name a `filter=` or `diff=` driver is refused **for dirty and
      per-file state** — no count is produced for it. —
      `attributes_naming_an_undefined_driver_is_accepted_branch_only` and its `diff=` counterpart.
- [x] The fixture pins its own configuration and sanitizes the environment; the result does not depend
      on the developer's machine. — `Fixture::new`'s isolated `HOME`/`GIT_CONFIG_GLOBAL`/
      `GIT_CONFIG_SYSTEM`.
- [x] The named programs are harmless: a marker file and nothing else. — `Fixture::marker_script`.
- [x] **RFC-012's *Git Detector Safety* gate reported item by item**, each with its own evidence. —
      qa-evidence.md.
- [x] `git` absent or too old is `unavailable` — no panic, no guess. —
      `git_not_found_reports_unavailable_not_a_panic`, `parse_git_version_reads_the_real_toolchain_output`,
      `a_version_below_the_minimum_is_too_old`.
- [x] **D1′ confirmed against the committed fixture** — including the measurement that a repository
      naming nothing executes nothing, which is what the whole decision rests on. —
      `control_repository_is_accepted_and_executes_nothing`.

## PR-030-B — branch, dirty state, ahead/behind

- [ ] `set_git_summary` has a production caller; an accepted repository shows branch, dirty state and
      ahead/behind (REQ-GIT-001, 002).
- [ ] A **refused** repository shows its branch and `unavailable` for dirty state — **with the branch
      read measured to execute nothing**, not assumed.
- [ ] *Unavailable* for a non-repository; *pending* while a read is in flight. **Ablation:** show the
      previous summary while pending; that test fails alone.
- [ ] **No write operation exists in the Git path — held by the API, not by grep** (REQ-GIT-007).
- [ ] Restricted projects are not treated differently, and the reason is the gate, not the trust
      grant.
- [ ] Input is never blocked by a refresh (NFR-PERF-006); the marker is still absent after the
      production path runs.
- [ ] **Carried from RFC-025 (review 404):** the status bar's project fields **reach the rendered
      row**, re-proved by this slice — by a live capture showing all of REQ-NOTIFY-002's fields
      together, at minimum.

## PR-030-C — per-file status

- [ ] Per-file status from an accepted repository (REQ-GIT-003); a file outside it carries none; a
      refused repository offers none.
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
