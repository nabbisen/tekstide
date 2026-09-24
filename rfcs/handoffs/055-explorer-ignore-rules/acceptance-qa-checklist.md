---
title: "RFC-055 — acceptance and QA checklist"
rfc: "RFC-055"
rfc_file: "../../accepted/055-ignore-rules-in-the-explorer.md"
source_rfc_status: "Accepted 2026-09-24 — M12 remainder tail"
target_milestone: "M12 remainder"
created: "2026-09-24"
---

# Acceptance and QA checklist

Tick a box when the evidence for it is in `qa-evidence.md`, not when the code looks right. Where a
box names a measurement, the number goes in the evidence file — **a box that says "measured" with no
number in the evidence is not ticked.**

## PR-055-A — the query, and the file that tries to silence it

- [x] A file named `:(glob)evil.log` sits in the fixture, and **every sibling of it still receives an
      ignore answer.** The test's own comment records that removing the `./` prefix makes it fail
      with exit 128.
- [x] **Ablation:** the `./` prefix is removed, the test above fails, and the evidence names the exit
      code and the `fatal:` line. Restored with `rfcs/handoffs/ablate.sh`, which refuses a dirty tree
      — never by hand.
- [x] Exit `1` (nothing ignored) and *unknown* are **different values in the returned type**, and a
      test would fail if they collapsed.
- [x] A forced failure reaches unknown. Not a repository, and a gate refusal, both reach unknown
      without running a subprocess to discover it.
      *(Not a repository runs nothing at all. A gate refusal necessarily read the configuration — that is
      the gate — and never reaches `check-ignore`; `qa-evidence.md` says which is which.)*
- [x] A tracked file matching `*.log` comes back **not ignored** — no `--no-index` anywhere.
- [x] A project root two levels inside its repository gets the repository-root `.gitignore` applied.
- [x] A non-UTF-8 filename survives the round trip; nothing decodes lossily before a comparison.
- [x] The gate is **not cached** (D9), and the evidence says where it is paid.

### Required at review 431

- [x] **R1:** a scan **inside** a submodule is pinned — expanding the poisoned submodule of the
      review-406 fixture and asking about *its* entries yields `Unknown(GateRefused)` and runs no
      marker. The existing test asks from the superproject; this is the case the dropped gitlink
      check used to cover.
- [x] **R2:** `vet_configuration`'s doc comment states one measurement's numbers. It currently says
      the whole gate is ~115 ms and the walk is ~117 ms **of it**.
- [x] **R3:** the home exclusion has its own `IgnoreUnknown` variant. It is a decision to decline,
      not a fact about the filesystem, and PR-055-B renders which rule it used.
- [ ] **R4 (travels to B and C):** `core.excludesFile` is **not** honoured — `GIT_CONFIG_GLOBAL` is
      `/dev/null` (R6, and right). `.gitignore`, `.git/info/exclude` and `$XDG_CONFIG_HOME/git/ignore`
      are. `REQ-FILE-005` is not marked complete without naming this, and the user-facing disclosure
      says it. Whether to honour it is ruled at PR-055-C.
- [x] **R5:** `MAX_IGNORE_QUERY_ENTRIES` and its comment agree — it is `1_024`, the comment calls it
      the explorer's 256 restated.

## PR-055-B — the scan carries it, and the floor keeps its job

- [x] `FileGitStatus::Ignored` exists and **no match arm on it is a catch-all** — grepped, with the
      count of sites in the evidence.
- [x] The query is asked about **at most `max_children_per_directory` paths per directory**, and the
      20 000-`*.log` fixture proves it: the number of paths asked about tracks the rows drawn, not
      the number of ignored files. **This is the test that falsifies D2 if D2 is wrong.**
- [x] Entries beyond the cap keep **unknown** ignore state, and nothing renders or counts them as
      ignored or as not ignored.
- [x] A repository whose `.gitignore` does **not** name `target/` shows `target/` as an ordinary
      expandable directory; one that does shows it collapsed with the `ignored` badge. Both captured.
- [x] A project that is not a repository shows the floor list **and the scan carries that it was the
      floor**, as a value, not as a render-time guess.
- [x] `IGNORED_DIRECTORY_NAMES`' doc comment states that its two consumers are no longer symmetrical.
- [x] **Budget, measured on RFC-052's 100 000-entry fixture:** expanding a directory pays the gate
      plus one query and stays within the frame budget. The number in the evidence is the **whole
      call**, not the query alone.
- [x] Nothing new runs on the render thread; the work stays on `explorer_scan_subscription`.

### Required at review 432

- [x] **Q1:** `{ $name }` moves to the front of `explorer-node-entry`, before the status selectors —
      `browse-node-entry` already has that shape. A capture shows an ignored directory whose name is
      whole, and a test holds the name ahead of the words. A clipped *name* invents a file that does
      not exist (`(collapsed) [ignored] t`); a clipped *word* is visibly damaged.
- [x] Recorded, not required: the sidebar has no overflow marker, so a long filename still clips
      silently. Pre-existing (RFC-052); the `0.26.0` changelog owns it as a limitation.

## PR-055-C — the setting, the badge, the words, and one invariant

- [x] `explorer.show_ignored` defaults to `false`; `true` draws ignored rows with the `ignored`
      badge. Both captured.
- [x] An unknown or refused value for it is **named on the Project Board** and the default stands, as
      RFC-054's mechanism already does.
- [x] **Dotfiles are still visible** with the setting at either value — `.gitignore`, `.env` and
      `.git-exclude` appear as ordinary rows. The captures show it.
- [x] The sidebar says where the ignore rule came from, and the staleness sentence is RFC-052's
      extended, not a second vocabulary for the same fact.
      *(There was no RFC-052 staleness sentence on screen to extend — it was in the book and the changelog only —
      so the on-screen form is new: `explorer-ignore-age`. `qa-evidence.md` says so.)*
- [x] The book's configuration page names the key, and the existing page/code invariants pass.
- [x] **Carried from RFC-053's closeout:** `rfc_docs_invariants` gains a check that an RFC the
      changelog names as released lives in `done/`. The evidence shows it **failing** against a
      planted violation and passing without it.
- [x] The `0.26.0` changelog section owns this release's own limitations in its own words, and
      corrects any predecessor it contradicts by name.

## Whole-RFC

- [x] **Review 431 R4:** `REQ-FILE-005` is not marked complete without naming that **`core.excludesFile` is not honoured**
      (`qa-evidence.md`); the user-facing disclosure says so in the user's words.
- [x] `REQ-FILE-005`, `REQ-FILE-006` and the `ignored` category of `REQ-FILE-002` move to
      implemented **with evidence they are reachable by a user**, not merely parsed.
- [ ] The colour-alone, i18n completeness and internal-identifier scans still pass.
- [ ] `cargo fmt`, `clippy --workspace --all-targets -D warnings`, `git diff --cached --check` after
      staging, `rfc_docs_invariants`, **three consecutive full-workspace runs with `--no-fail-fast`**
      to files, **0 fixture entries left** in a fresh short `TMPDIR`.
- [x] Every new intermittent failure has a dated row in `test-process-leak.md`.
- [ ] Commits are pushed once the gate is green.

## Final Acceptance Decision

- [ ] Accepted.
- [ ] Accepted with required follow-up.
- [ ] Requires re-review after changes.

Reviewer notes:

```text
```
