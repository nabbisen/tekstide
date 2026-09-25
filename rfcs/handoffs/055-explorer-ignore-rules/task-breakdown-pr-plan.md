---
title: "RFC-055 — task breakdown and PR plan"
rfc: "RFC-055"
rfc_file: "../../done/055-ignore-rules-in-the-explorer.md"
source_rfc_status: "Implemented and closed 2026-09-25 — M12 remainder tail"
target_milestone: "M12 remainder"
created: "2026-09-24"
---

# Task breakdown and PR plan

Three slices. **A is the whole security argument and changes no surface**; B wires it to the scan;
C is the setting, the badge and the words. Each lands green on its own.

## PR-055-A — the query, and the file that tries to silence it

`crates/tekstide-core/src/runtime/git.rs`, plus its tests. **No surface change, no explorer change.**

A new production entry beside `compute_summary`, taking a directory and the paths to ask about, and
returning three states — ignored set, nothing ignored, unknown — never two. Inside:

- The gate first, exactly as `compute_summary_with_environment` does it, and **not cached** (D9).
  `resolve_git_dir_from_filesystem` returning `None` is *not a repository*, which is its own outcome
  and costs no subprocess.
- `git check-ignore -z --stdin`, through `run_bounded_git`, with `cwd` the directory being asked
  about, **no `--no-index`**, and the existing `SUBPROCESS_TIMEOUT` and `MAX_OUTPUT_BYTES` bounds.
- **One function applies `./` to every path on the way in and strips it on the way out.** No caller
  can hand a raw path to the subprocess; make that true of the types, not of the reviewers.
- Exit `0`/`1`/other mapped per the risk document's table. `1` is an answer.

Tests, in this crate's own hostile-fixture style:

1. A file named `:(glob)evil.log` beside ordinary files — **every sibling still gets an answer**.
   Delete the `./` prefix and this test fails with exit 128; say so in the test's own comment.
2. Exit `1` (a directory with nothing ignored) is drawn as an ordinary directory, not as a failure.
3. A forced failure yields unknown, and unknown is distinguishable from "nothing ignored" in the
   returned type. If those two collapse to the same value, the type is wrong.
4. A tracked file matching `*.log` comes back **not** ignored.
5. A project root two levels inside its repository: the repository-root `.gitignore` still applies.
6. A non-UTF-8 filename survives the round trip.
7. Not a repository, and a gate refusal, both reach unknown without a subprocess claim.

## PR-055-B — the scan carries it, and the floor keeps its job

`project/root/explorer.rs`, `project/metadata.rs`, `project/ignored_directories.rs`, and the scan
wiring in `crates/tekstide/src/shell.rs`.

- `FileGitStatus` gains `Ignored` (D5). Every match arm on it is a compile error until handled —
  that is the point; do not add a catch-all.
- The worker that already produces `ExplorerDirectoryScan` asks PR-055-A about the entries it is
  about to return, at most `max_children_per_directory` of them, and records the answer on the nodes.
  It stays on that worker (`explorer_scan_subscription`); nothing new blocks the render thread.
- The scan carries **which rule it used**: git's answer, or the floor list. Not a boolean tucked in a
  render path — a value on the scan, because the sidebar has to say it (D6).
- Omitted entries beyond the cap keep **unknown** ignore state (risk document §3).
- `IGNORED_DIRECTORY_NAMES`' doc comment is updated to say its two consumers are no longer
  symmetrical: the explorer now prefers git's answer and keeps this list as a floor, while
  `GeneratedChangeDetectionPolicy` still uses it as the whole rule. A shared constant whose two
  readers mean different things must say so where it is defined.
- **The budget**: expanding a directory pays the gate plus one query, measured on the 100 000-entry
  fixture from RFC-052, with the number written down. The *whole call*, not the query alone — see
  *Decided on acceptance*.

## PR-055-C — the setting, the badge, the words, and one invariant

`config/`, the explorer surface, the catalog, the book, and `crates/tekstide/tests/rfc_docs_invariants.rs`.

- `explorer.show_ignored`, default `false`, through RFC-054's mechanism: an unknown or refused value
  is named on the Project Board and the default stands. **Dotfiles are untouched** — the setting
  governs ignored entries only (D7).
- The `ignored` badge word joins the six in `surface/explorer.rs`, through the catalog like the rest.
- The sidebar says where the rule came from, and extends RFC-052's existing staleness sentence rather
  than adding a second vocabulary for the same fact (D8).
- The book's configuration page gains the key; the existing invariant tests already require the page
  and the code to agree, so this is not optional.
- **Carried from RFC-053's closeout**: one new check in `rfc_docs_invariants` — an RFC the changelog
  names as released must live in `done/`. RFC-053 shipped in `0.23.0` and stayed in `accepted/` for
  three releases because nothing relates a release to a folder. Expect it to fail first; that is how
  you know it works.
- The `0.26.0` changelog section owns this release's own limitations in its own words, including the
  staleness, and corrects any predecessor it contradicts by name.

## Order, and why

A before B because A is where the security argument lives and it can be falsified alone. B before C
because a badge for a state the model cannot hold is not a slice. C last because the setting, the
words and the book move together — and the invariant test belongs with the release-shaped slice.
